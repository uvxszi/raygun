//! A `.torrent` file's trackers, filtered: the file is rewritten with only the trackers the
//! engine can announce to (on Android, HTTPS trackers' certificates can't be checked by the
//! engine's own HTTP client). Everything else is copied byte for byte, the `info` dictionary
//! included, so the torrent's info hash stays the same.

use crate::link::usable_tracker;

/// The end of the bencoded value starting at `at` (one past its last byte).
fn value_end(bytes: &[u8], at: usize) -> Option<usize> {
    match *bytes.get(at)? {
        b'i' => {
            let end = bytes[at..].iter().position(|byte| *byte == b'e')?;
            Some(at + end + 1)
        }
        b'l' | b'd' => {
            let mut position = at + 1;
            while *bytes.get(position)? != b'e' {
                position = value_end(bytes, position)?;
            }
            Some(position + 1)
        }
        b'0'..=b'9' => string_at(bytes, at).map(|(_, end)| end),
        _ => None,
    }
}

/// The byte string starting at `at` and where it ends.
fn string_at(bytes: &[u8], at: usize) -> Option<(&[u8], usize)> {
    let colon = at + bytes.get(at..)?.iter().position(|byte| *byte == b':')?;
    let len: usize = std::str::from_utf8(&bytes[at..colon]).ok()?.parse().ok()?;
    let start = colon + 1;
    let end = start.checked_add(len)?;
    Some((bytes.get(start..end)?, end))
}

fn push_string(out: &mut Vec<u8>, text: &[u8]) {
    out.extend_from_slice(text.len().to_string().as_bytes());
    out.push(b':');
    out.extend_from_slice(text);
}

fn usable(tracker: &[u8], https_trackers: bool) -> bool {
    std::str::from_utf8(tracker).is_ok_and(|tracker| usable_tracker(tracker, https_trackers))
}

/// `announce-list`'s tiers, keeping only usable trackers and tiers left with any.
fn filtered_tiers(bytes: &[u8], at: usize, end: usize, https_trackers: bool) -> Option<Vec<u8>> {
    let mut out = vec![b'l'];
    let mut tiers = 0;
    let mut position = at + 1;
    while position < end - 1 {
        let tier_end = value_end(bytes, position)?;
        if bytes[position] == b'l' {
            let mut tier = vec![b'l'];
            let mut kept = 0;
            let mut inner = position + 1;
            while inner < tier_end - 1 {
                let (tracker, next) = string_at(bytes, inner)?;
                if usable(tracker, https_trackers) {
                    push_string(&mut tier, tracker);
                    kept += 1;
                }
                inner = next;
            }
            if kept > 0 {
                tier.push(b'e');
                out.extend_from_slice(&tier);
                tiers += 1;
            }
        }
        position = tier_end;
    }
    out.push(b'e');
    (tiers > 0).then_some(out)
}

/// The torrent file with only the trackers the engine can use; `None` when it doesn't read as a
/// torrent file (it's handed over as it is then, and the engine says what's wrong).
pub fn with_usable_trackers(bytes: &[u8], https_trackers: bool) -> Option<Vec<u8>> {
    if bytes.first() != Some(&b'd') || value_end(bytes, 0)? != bytes.len() {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len());
    out.push(b'd');
    let mut position = 1;
    while bytes[position] != b'e' {
        let (key, value_at) = string_at(bytes, position)?;
        let end = value_end(bytes, value_at)?;
        match key {
            b"announce" => {
                let (tracker, _) = string_at(bytes, value_at)?;
                if usable(tracker, https_trackers) {
                    out.extend_from_slice(&bytes[position..end]);
                }
            }
            b"announce-list" => {
                if let Some(tiers) = filtered_tiers(bytes, value_at, end, https_trackers) {
                    push_string(&mut out, key);
                    out.extend_from_slice(&tiers);
                }
            }
            _ => out.extend_from_slice(&bytes[position..end]),
        }
        position = end;
    }
    out.push(b'e');
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bencoded byte string.
    fn text(value: &str) -> String {
        format!("{}:{value}", value.len())
    }

    #[test]
    fn https_trackers_go_and_the_rest_stays_byte_for_byte() {
        let (secure, udp, plain) = (
            "https://t.example/announce",
            "udp://u.example:1337/announce",
            "http://h.example/announce",
        );
        let info = format!(
            "d{}i5e{}{}{}i16384e{}{}e",
            text("length"),
            text("name"),
            text("a.mkv"),
            text("piece length"),
            text("pieces"),
            text("aaaaaaaaaaaaaaaaaaaa"),
        );
        let torrent = format!(
            "d{}{}{}ll{}{}el{}ee{}{}{}{info}e",
            text("announce"),
            text(secure),
            text("announce-list"),
            text(secure),
            text(udp),
            text(plain),
            text("comment"),
            text("hi"),
            text("info"),
        );
        let expected = format!(
            "d{}ll{}el{}ee{}{}{}{info}e",
            text("announce-list"),
            text(udp),
            text(plain),
            text("comment"),
            text("hi"),
            text("info"),
        );
        let kept = with_usable_trackers(torrent.as_bytes(), false).unwrap();
        assert_eq!(String::from_utf8(kept).unwrap(), expected);

        // Where HTTPS trackers can be used, nothing changes.
        assert_eq!(
            with_usable_trackers(torrent.as_bytes(), true).unwrap(),
            torrent.as_bytes()
        );
    }

    #[test]
    fn what_isnt_a_torrent_file_is_left_alone() {
        assert_eq!(with_usable_trackers(b"<html>", false), None);
        assert_eq!(with_usable_trackers(b"d4:infod", false), None);
        assert_eq!(with_usable_trackers(b"", false), None);
    }
}
