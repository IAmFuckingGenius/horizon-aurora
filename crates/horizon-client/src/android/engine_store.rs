//! Engines from Aurora's releases: the lists published beside the APKs
//! (`engines::LISTS`) and their `.hengine` files, downloaded on a worker thread,
//! checked against the list (size, SHA-256) and installed like an imported engine.
use crate::engines::{self, Manifest, Offer};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

/// The largest list read, in bytes.
const LIST_LIMIT: u64 = 1 << 20;

enum Reply {
    Offers(Result<Vec<Offer>, String>),
    Installed(Result<Manifest, String>),
}

/// What finished, for the launcher to say.
pub enum Done {
    /// The lists were read (the offers are in [`Store::offers`]).
    Checked,
    Installed(Manifest),
    Failed(String),
}

#[derive(Default)]
pub struct Store {
    /// The engines offered for this device, once checked.
    pub offers: Vec<Offer>,
    pub checked: bool,
    /// Why the lists could not be read.
    pub error: Option<String>,
    /// The offer being downloaded, and how many of its bytes have arrived.
    pub downloading: Option<String>,
    progress: Arc<AtomicU64>,
    worker: Option<mpsc::Receiver<Reply>>,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .https_only(true)
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(90))
        .build()
}

impl Store {
    pub fn busy(&self) -> bool {
        self.worker.is_some()
    }
    /// The download's progress, 0 to 1.
    pub fn progress(&self) -> f32 {
        let total = self
            .downloading
            .as_ref()
            .and_then(|id| self.offers.iter().find(|o| &o.id == id))
            .map_or(0, |o| o.size);
        if total == 0 {
            return 0.0;
        }
        (self.progress.load(Ordering::Relaxed) as f32 / total as f32).min(1.0)
    }

    /// Reads the lists of the latest release and of the dev builds.
    pub fn check(&mut self) {
        if self.busy() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.worker = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(Reply::Offers(read_lists()));
        });
    }

    /// Downloads `offer` into `home`'s engines and installs it.
    pub fn download(&mut self, home: PathBuf, offer: Offer) {
        if self.busy() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.worker = Some(rx);
        self.downloading = Some(offer.id.clone());
        self.progress.store(0, Ordering::Relaxed);
        let progress = self.progress.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Reply::Installed(fetch(&home, &offer, &progress)));
        });
    }

    /// What the worker finished, once.
    pub fn poll(&mut self) -> Option<Done> {
        let reply = self.worker.as_ref()?.try_recv();
        let reply = match reply {
            Ok(reply) => reply,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Reply::Installed(Err("The download stopped unexpectedly.".into()))
            }
        };
        self.worker = None;
        self.downloading = None;
        Some(match reply {
            Reply::Offers(Ok(offers)) => {
                self.offers = offers;
                self.checked = true;
                self.error = None;
                Done::Checked
            }
            Reply::Offers(Err(error)) => {
                self.checked = true;
                self.error = Some(error);
                Done::Checked
            }
            Reply::Installed(Ok(manifest)) => Done::Installed(manifest),
            Reply::Installed(Err(error)) => Done::Failed(error),
        })
    }
}

/// The offers of every list; a list that does not exist yet (no release has engines)
/// adds none.
fn read_lists() -> Result<Vec<Offer>, String> {
    let agent = agent();
    let mut offers: Vec<Offer> = Vec::new();
    let mut failure = None;
    let mut read_any = false;
    for url in engines::LISTS {
        let response = match agent.get(url).call() {
            Ok(response) => response,
            Err(ureq::Error::Status(404, _)) => {
                read_any = true;
                continue;
            }
            Err(error) => {
                failure.get_or_insert_with(|| error.to_string());
                continue;
            }
        };
        let mut bytes = Vec::new();
        if let Err(error) = response
            .into_reader()
            .take(LIST_LIMIT)
            .read_to_end(&mut bytes)
        {
            failure.get_or_insert_with(|| error.to_string());
            continue;
        }
        match engines::offers(&bytes) {
            Ok(found) => {
                read_any = true;
                for offer in found {
                    if !offers.iter().any(|o| o.id == offer.id) {
                        offers.push(offer);
                    }
                }
            }
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
    }
    match failure {
        Some(error) if !read_any => Err(error),
        _ => Ok(offers),
    }
}

/// Downloads `offer`'s `.hengine` beside the engines, checks it against the list and
/// installs it.
fn fetch(home: &std::path::Path, offer: &Offer, progress: &AtomicU64) -> Result<Manifest, String> {
    let folder = engines::folder(home);
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let partial = folder.join(".download.hengine");
    let result = (|| {
        let response = agent().get(&offer.url).call().map_err(|e| e.to_string())?;
        let mut reader = response.into_reader().take(offer.size + 1);
        let mut file = std::fs::File::create(&partial).map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut buffer = vec![0u8; 256 << 10];
        let mut total = 0u64;
        loop {
            let n = reader.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > offer.size {
                return Err("The engine file is larger than its list says.".to_owned());
            }
            hash.update(&buffer[..n]);
            file.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
            progress.store(total, Ordering::Relaxed);
        }
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        if total != offer.size {
            return Err("The download is incomplete. Try again.".to_owned());
        }
        let digest: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if !digest.eq_ignore_ascii_case(&offer.sha256) {
            return Err("The engine file is damaged: its checksum does not match.".to_owned());
        }
        let file = std::fs::File::open(&partial).map_err(|e| e.to_string())?;
        let manifest = engines::install(home, file)?;
        if manifest.id != offer.id {
            let _ = engines::remove(home, &manifest.id);
            return Err("The engine file is not the engine the list names.".to_owned());
        }
        Ok(manifest)
    })();
    let _ = std::fs::remove_file(&partial);
    result
}
