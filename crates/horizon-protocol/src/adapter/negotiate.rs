//! Which release to speak to a server when its status ping alone does not settle it.
//!
//! A proxy (BungeeCord, Velocity and their forks) or ViaVersion answers a status ping with the
//! protocol the client asked with, when it takes that client: a server on 1.8 answers 26.3's
//! protocol to a 26.3 client. Many proxies answer every client that way and say in their version
//! name how far they really go ("CheatBungee 1.8.x - 1.20.x"); [`ceiling`] reads that. A server
//! whose ViaVersion does not know this game's protocol answers with its own, older one; asked again
//! with an older client's protocol, it echoes the newest it takes ([`newest_echoed`]).
//!
//! No dependencies: `tools/test_android_servers.py` runs these tests with `rustc --test` alone.

/// A Minecraft: Java Edition release number: `1.20.4` is `Release(1, 20, 4)`, `26.1` is
/// `Release(26, 1, 0)`. [`ANY`] stands for a patch (or minor) a version name leaves open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Release(pub u32, pub u32, pub u32);

/// Any number: `1.21.x` and an upper bound of `1.21` take every 1.21 patch.
pub const ANY: u32 = u32::MAX;

impl Release {
    /// `1.20.4`, `26.1` (patch 0), `1.21.x`; `None` for anything that is not a Minecraft release
    /// number (a proxy's own `3.4.0`, a build number).
    pub fn parse(text: &str) -> Option<Self> {
        let (release, _) = read(text, false)?;
        Some(release)
    }
}

/// Reads a release number at the start of `text`, and how many bytes it took. `bound`: a missing
/// patch is [`ANY`] (an upper bound's `1.21` takes 1.21.11) rather than 0.
fn read(text: &str, bound: bool) -> Option<(Release, usize)> {
    let bytes = text.as_bytes();
    let mut parts = [0u32; 3];
    let mut count = 0;
    let mut at = 0;
    loop {
        let start = at;
        let part = if bytes.get(at).is_some_and(|b| *b == b'x' || *b == b'X') {
            at += 1;
            ANY
        } else {
            while bytes.get(at).is_some_and(u8::is_ascii_digit) {
                at += 1;
            }
            if at == start || at - start > 2 {
                return None;
            }
            text[start..at].parse().ok()?
        };
        parts[count] = part;
        count += 1;
        // Another part follows only after a dot and a digit or an `x`.
        let more = count < 3
            && bytes.get(at) == Some(&b'.')
            && bytes
                .get(at + 1)
                .is_some_and(|b| b.is_ascii_digit() || *b == b'x' || *b == b'X');
        if !more {
            break;
        }
        at += 1;
    }
    if count < 2 || parts[0] == ANY {
        return None;
    }
    if parts[1] == ANY {
        parts[2] = ANY;
    } else if count == 2 && bound {
        parts[2] = ANY;
    }
    let release = Release(parts[0], parts[1], parts[2]);
    // 1.7 to 1.21.11 before the year numbering, then 26.1 on.
    let minecraft = match release.0 {
        1 => (7..=21).contains(&release.1) || release.1 == ANY,
        26..=99 => true,
        _ => false,
    };
    minecraft.then_some((release, at))
}

/// The newest release a server's version name says it takes: "Velocity 1.7.2-26.3" gives 26.3,
/// "CheatBungee 1.8.x - 1.20.x" any 1.20, "Requires MC 1.8 / 1.21" any 1.21. Formatting codes are
/// skipped. `None` when the name has no release number.
pub fn ceiling(name: &str) -> Option<Release> {
    let mut plain = String::with_capacity(name.len());
    let mut chars = name.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            plain.push(c);
        }
    }
    let bytes = plain.as_bytes();
    let mut newest = None;
    let mut at = 0;
    while at < bytes.len() {
        // A number starts after something that is not part of a word or another number.
        let starts = bytes[at].is_ascii_digit()
            && (at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'.'));
        if starts {
            let end = bytes[at..]
                .iter()
                .position(|b| !(b.is_ascii_alphanumeric() || *b == b'.'))
                .map_or(bytes.len(), |n| at + n);
            if let Some((release, used)) = read(&plain[at..end], true)
                && (at + used == end || !bytes[at + used].is_ascii_alphanumeric())
            {
                newest = newest.max(Some(release));
            }
            at = end;
        } else {
            at += 1;
        }
    }
    newest
}

/// The first of `oldest` (each protocol's oldest release, newest protocol first) that a server
/// taking releases up to `ceiling` takes, by its index.
pub fn newest_within(oldest: &[Release], ceiling: Release) -> Option<usize> {
    oldest.iter().position(|release| *release <= ceiling)
}

/// The newest of `count` protocols (newest first) a server echoes, by its index. `echoes(i)` asks
/// the server with protocol `i`: whether it answered with it, or `None` when the ask failed (which
/// ends the search). A server takes every protocol older than one it takes, as ViaVersion does, so
/// after the newest the search halves what is left: at most five asks for fourteen protocols.
pub fn newest_echoed(count: usize, mut echoes: impl FnMut(usize) -> Option<bool>) -> Option<usize> {
    if count == 0 {
        return None;
    }
    if echoes(0)? {
        return Some(0);
    }
    let (mut low, mut high) = (1, count);
    while low < high {
        let middle = (low + high) / 2;
        if echoes(middle)? {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    (low < count).then_some(low)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_numbers_read_as_minecraft_writes_them() {
        assert_eq!(Release::parse("1.20.4"), Some(Release(1, 20, 4)));
        assert_eq!(Release::parse("26.1"), Some(Release(26, 1, 0)));
        assert_eq!(Release::parse("1.21.x"), Some(Release(1, 21, ANY)));
        assert_eq!(Release::parse("3.4.0"), None);
        assert_eq!(Release::parse("1.6.4"), None);
        assert_eq!(Release::parse("20"), None);
    }

    #[test]
    fn version_names_give_the_newest_release_they_name() {
        assert_eq!(ceiling("Velocity 1.7.2-26.3"), Some(Release(26, 3, ANY)));
        assert_eq!(
            ceiling("CheatBungee 1.8.x - 1.20.x"),
            Some(Release(1, 20, ANY))
        );
        assert_eq!(ceiling("Requires MC 1.8 / 1.21"), Some(Release(1, 21, ANY)));
        assert_eq!(ceiling("BotFilter 1.8.x-26.x"), Some(Release(26, ANY, ANY)));
        assert_eq!(
            ceiling("§f§f§fWe support: 1.20-1.21"),
            Some(Release(1, 21, ANY))
        );
        assert_eq!(
            ceiling("§c❤ §b§lMustery§6§lWorld§r §7>>> §61.12 §7- §61.21 §c❤"),
            Some(Release(1, 21, ANY))
        );
        assert_eq!(ceiling("Paper 1.21.4"), Some(Release(1, 21, 4)));
        assert_eq!(ceiling("1.20.1"), Some(Release(1, 20, 1)));
    }

    #[test]
    fn numbers_that_are_not_releases_are_not_ceilings() {
        assert_eq!(ceiling("CubeCraft"), None);
        assert_eq!(ceiling("Velocity 3.4.0-SNAPSHOT"), None);
        assert_eq!(
            ceiling("BungeeCord 1.21-R0.4-SNAPSHOT"),
            Some(Release(1, 21, ANY))
        );
        assert_eq!(ceiling("Waterfall build 1.20.512"), None);
        assert_eq!(ceiling("v2.1.21.4"), None);
    }

    #[test]
    fn the_newest_protocol_within_a_ceiling_is_chosen() {
        // 26.2, 26.1-26.1.2, 1.21.11, 1.21.9-1.21.10, 1.21.2-1.21.3, 1.20.2.
        let oldest = [
            Release(26, 2, 0),
            Release(26, 1, 0),
            Release(1, 21, 11),
            Release(1, 21, 9),
            Release(1, 21, 2),
            Release(1, 20, 2),
        ];
        assert_eq!(newest_within(&oldest, Release(1, 21, ANY)), Some(2));
        assert_eq!(newest_within(&oldest, Release(1, 21, 10)), Some(3));
        assert_eq!(newest_within(&oldest, Release(1, 21, 3)), Some(4));
        assert_eq!(newest_within(&oldest, Release(26, 1, 2)), Some(1));
        assert_eq!(newest_within(&oldest, Release(1, 20, ANY)), Some(5));
        assert_eq!(newest_within(&oldest, Release(1, 16, 5)), None);
    }

    #[test]
    fn the_search_finds_the_newest_echoed_protocol_in_few_asks() {
        for count in 1..=14 {
            for newest in 0..=count {
                let mut asks = 0;
                let found = newest_echoed(count, |i| {
                    asks += 1;
                    Some(i >= newest)
                });
                assert_eq!(
                    found,
                    (newest < count).then_some(newest),
                    "{count} {newest}"
                );
                assert!(asks <= 5, "{count} {newest}: {asks} asks");
            }
        }
    }

    #[test]
    fn a_failed_ask_ends_the_search() {
        let mut asks = 0;
        assert_eq!(
            newest_echoed(14, |_| {
                asks += 1;
                None
            }),
            None
        );
        assert_eq!(asks, 1);
    }
}
