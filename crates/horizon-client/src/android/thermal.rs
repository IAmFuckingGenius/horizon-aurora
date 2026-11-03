//! The system's thermal state for Horizon Boost's governor: `AThermal_*` from libandroid
//! (status from API 30, headroom from API 31), resolved at run time so older Android
//! loads; Android 10 (API 29) reads `PowerManager.getCurrentThermalStatus` instead.
//! No sysfs, root or device identifiers.
use horizon_client::boost::Thermal;
use std::{
    cell::RefCell,
    ffi::{c_char, c_int, c_void},
    time::{Duration, Instant},
};
#[link(name = "dl")]
unsafe extern "C" {
    fn dlopen(n: *const c_char, f: c_int) -> *mut c_void;
    fn dlsym(h: *mut c_void, n: *const c_char) -> *mut c_void;
}
type Acquire = unsafe extern "C" fn() -> *mut c_void;
type Status = unsafe extern "C" fn(*mut c_void) -> c_int;
type Headroom = unsafe extern "C" fn(*mut c_void, c_int) -> f32;

/// Status is cheap; the platform rate-limits headroom (more often than about once a
/// second returns NaN), and JNI costs more.
const STATUS_EVERY: Duration = Duration::from_secs(1);
const HEADROOM_EVERY: Duration = Duration::from_secs(2);
const JNI_EVERY: Duration = Duration::from_secs(5);
/// How far ahead the headroom forecast looks.
const FORECAST_SECONDS: c_int = 10;

struct Reader {
    manager: *mut c_void,
    status: Option<Status>,
    headroom: Option<Headroom>,
    /// No AThermal (API 29): PowerManager through JNI.
    jni: bool,
    status_at: Option<Instant>,
    headroom_at: Option<Instant>,
    reading: Thermal,
}
impl Reader {
    fn new() -> Self {
        let mut r = Self {
            manager: std::ptr::null_mut(),
            status: None,
            headroom: None,
            jni: false,
            status_at: None,
            headroom_at: None,
            reading: Thermal::default(),
        };
        unsafe {
            let lib = dlopen(c"libandroid.so".as_ptr(), 2);
            if !lib.is_null() {
                let acquire = dlsym(lib, c"AThermal_acquireManager".as_ptr());
                let status = dlsym(lib, c"AThermal_getCurrentThermalStatus".as_ptr());
                let headroom = dlsym(lib, c"AThermal_getThermalHeadroom".as_ptr());
                if !acquire.is_null() && !status.is_null() {
                    let acquire: Acquire = std::mem::transmute(acquire);
                    r.manager = acquire();
                    if !r.manager.is_null() {
                        r.status = Some(std::mem::transmute::<*mut c_void, Status>(status));
                        r.headroom = (!headroom.is_null())
                            .then(|| std::mem::transmute::<*mut c_void, Headroom>(headroom));
                    }
                }
            }
        }
        r.jni = r.status.is_none();
        r
    }
    fn read(&mut self, now: Instant) -> Thermal {
        let due = |at: Option<Instant>, every| at.is_none_or(|t| now.duration_since(t) >= every);
        if let Some(status) = self.status {
            if due(self.status_at, STATUS_EVERY) {
                self.status_at = Some(now);
                let s = unsafe { status(self.manager) };
                self.reading.status = (s >= 0).then_some(s);
            }
        } else if self.jni && due(self.status_at, JNI_EVERY) {
            self.status_at = Some(now);
            let power = crate::platform::performance_power();
            self.reading.status = power["thermal_status"]
                .as_i64()
                .map(|s| s as i32)
                .filter(|s| *s >= 0);
            // Before API 29 there is nothing to ask.
            if power["android_sdk"].as_i64().is_some_and(|sdk| sdk < 29) {
                self.jni = false;
            }
        }
        if let Some(headroom) = self.headroom
            && due(self.headroom_at, HEADROOM_EVERY)
        {
            self.headroom_at = Some(now);
            let h = unsafe { headroom(self.manager, FORECAST_SECONDS) };
            // NaN while the platform has no forecast yet: keep the last one.
            if h.is_finite() {
                self.reading.headroom = Some(h.max(0.0));
            }
        }
        self.reading
    }
}

thread_local! {static READER: RefCell<Option<Reader>> = const { RefCell::new(None) };}

/// The latest reading, refreshed when due. Call from the game thread.
pub fn read() -> Thermal {
    READER.with(|r| {
        r.borrow_mut()
            .get_or_insert_with(Reader::new)
            .read(Instant::now())
    })
}

/// What the performance report and status say about the thermal source.
pub fn source() -> &'static str {
    READER.with(|r| match r.borrow().as_ref() {
        None => "unread",
        Some(r) if r.headroom.is_some() => "athermal_headroom",
        Some(r) if r.status.is_some() => "athermal_status",
        Some(r) if r.jni => "power_manager",
        Some(_) => "unavailable",
    })
}
