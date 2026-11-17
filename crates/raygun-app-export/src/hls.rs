//! Saved HLS playlists joined into one stream: init sections once, AES-128 segments decrypted.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use aes::cipher::BlockDecryptMut;
use aes::cipher::KeyIvInit;
use aes::cipher::block_padding::Pkcs7;

use crate::remux;

type Decryptor = cbc::Decryptor<aes::Aes128>;

/// One file of a joined stream.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    /// An init section, written once before the segments that use it.
    Map(String),
    Segment {
        name: String,
        /// The key file and IV, for AES-128.
        key: Option<(String, [u8; 16])>,
    },
}

/// A saved media playlist, ready to join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Join {
    parts: Vec<Part>,
    /// How long its segments play (`#EXTINF`), ms.
    length_ms: u64,
}

impl Join {
    /// Reads a saved playlist. A master playlist (separate audio) can't be joined: converting
    /// (`remux`) reads its two playlists instead.
    pub fn parse(playlist: &str) -> Result<Self, String> {
        if playlist.contains("#EXT-X-STREAM-INF") {
            return Err(
                "This video's sound is a separate stream: save it as MP4 or MKV to join them"
                    .into(),
            );
        }
        let mut parts = Vec::new();
        let mut sequence: u128 = 0;
        let mut key: Option<(String, Option<[u8; 16]>)> = None;
        let mut map: Option<String> = None;
        let mut length = 0.0f64;
        for line in playlist
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            if let Some(value) = line.strip_prefix("#EXTINF:") {
                let seconds = value.split(',').next().unwrap_or_default().trim();
                length += seconds.parse::<f64>().unwrap_or(0.0).max(0.0);
            } else if let Some(value) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
                sequence = value.trim().parse().unwrap_or(0);
            } else if let Some(attributes) = line.strip_prefix("#EXT-X-KEY:") {
                key = match attribute(attributes, "METHOD").as_deref() {
                    Some("AES-128") => {
                        let uri =
                            attribute(attributes, "URI").ok_or("An encryption key is missing")?;
                        let iv = attribute(attributes, "IV")
                            .map(|iv| parse_iv(&iv))
                            .transpose()?;
                        Some((uri, iv))
                    }
                    Some("NONE") | None => None,
                    Some(_) => return Err("This video is protected".into()),
                };
            } else if let Some(attributes) = line.strip_prefix("#EXT-X-MAP:") {
                let uri = attribute(attributes, "URI").ok_or("An init section is missing")?;
                if map.as_deref() != Some(uri.as_str()) {
                    parts.push(Part::Map(uri.clone()));
                    map = Some(uri);
                }
            } else if !line.starts_with('#') {
                let key = key.as_ref().map(|(file, iv)| {
                    // Without an IV, the segment's sequence number is the IV (RFC 8216 §5.2).
                    (file.clone(), iv.unwrap_or_else(|| sequence.to_be_bytes()))
                });
                parts.push(Part::Segment {
                    name: line.to_owned(),
                    key,
                });
                sequence += 1;
            }
        }
        if !parts
            .iter()
            .any(|part| matches!(part, Part::Segment { .. }))
        {
            return Err("The saved stream has no segments".into());
        }
        Ok(Self {
            parts,
            length_ms: (length * 1000.0).round() as u64,
        })
    }

    /// How long its segments play, when the playlist says.
    pub fn seconds(&self) -> Option<f64> {
        (self.length_ms > 0).then(|| self.length_ms as f64 / 1000.0)
    }

    /// `mp4` for fragmented MP4, else `ts`.
    pub fn extension(&self) -> &'static str {
        if self.parts.iter().any(|part| matches!(part, Part::Map(_))) {
            "mp4"
        } else {
            "ts"
        }
    }

    /// Writes the joined stream, reading the parts from `dir`.
    pub fn write(
        &self,
        dir: &Path,
        out: &mut impl Write,
        cancel: &AtomicBool,
    ) -> std::io::Result<()> {
        for segment in self.segments(dir) {
            if cancel.load(Ordering::SeqCst) {
                return Err(std::io::Error::other("Cancelled"));
            }
            let segment = segment?;
            if let Some(init) = &segment.init {
                out.write_all(init)?;
            }
            out.write_all(&segment.data)?;
        }
        Ok(())
    }

    /// The segments in order, decrypted, each with the init section it starts (fragmented MP4).
    pub fn segments<'a>(&'a self, dir: &'a Path) -> remux::Segments<'a> {
        let mut keys: Vec<(String, [u8; 16])> = Vec::new();
        let mut init: Option<Vec<u8>> = None;
        let mut parts = self.parts.iter();
        Box::new(std::iter::from_fn(move || {
            loop {
                let part = parts.next()?;
                let read = (|| -> std::io::Result<Option<remux::Segment>> {
                    match part {
                        Part::Map(name) => {
                            init = Some(std::fs::read(safe_join(dir, name)?)?);
                            Ok(None)
                        }
                        Part::Segment { name, key } => {
                            let mut data = std::fs::read(safe_join(dir, name)?)?;
                            if let Some((file, iv)) = key {
                                let key = match keys.iter().find(|(known, _)| known == file) {
                                    Some((_, key)) => *key,
                                    None => {
                                        let mut key = [0u8; 16];
                                        File::open(safe_join(dir, file)?)?.read_exact(&mut key)?;
                                        keys.push((file.clone(), key));
                                        key
                                    }
                                };
                                let length = Decryptor::new(&key.into(), &(*iv).into())
                                    .decrypt_padded_mut::<Pkcs7>(&mut data)
                                    .map_err(|_| {
                                        std::io::Error::other("A segment couldn't be decrypted")
                                    })?
                                    .len();
                                data.truncate(length);
                            }
                            Ok(Some(remux::Segment {
                                init: init.take(),
                                data,
                            }))
                        }
                    }
                })();
                match read {
                    Ok(Some(segment)) => return Some(Ok(segment)),
                    Ok(None) => {}
                    Err(error) => return Some(Err(error)),
                }
            }
        }))
    }
}

/// A part's file inside the download folder: names are the saved ones, never paths.
pub fn safe_join(dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(std::io::Error::other(
            "The saved playlist points outside its folder",
        ));
    }
    Ok(dir.join(name))
}

pub fn attribute(list: &str, name: &str) -> Option<String> {
    let mut rest = list;
    while !rest.is_empty() {
        let (key, after) = rest.split_once('=')?;
        let (value, next) = if let Some(quoted) = after.strip_prefix('"') {
            let end = quoted.find('"')?;
            (&quoted[..end], quoted[end + 1..].trim_start_matches(','))
        } else {
            match after.split_once(',') {
                Some((value, next)) => (value, next),
                None => (after, ""),
            }
        };
        if key.trim() == name {
            return Some(value.to_owned());
        }
        rest = next;
    }
    None
}

pub fn parse_iv(value: &str) -> Result<[u8; 16], String> {
    let hex = value
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    let number = u128::from_str_radix(hex, 16).map_err(|_| "An encryption IV is unreadable")?;
    Ok(number.to_be_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_playlists_join_with_their_keys() {
        let playlist = "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:7\n#EXT-X-KEY:METHOD=AES-128,URI=\"key1.key\"\n\
                        00001.ts\n#EXT-X-KEY:METHOD=AES-128,URI=\"key1.key\",IV=0x0000000000000000000000000000000A\n\
                        00002.ts\n#EXT-X-KEY:METHOD=NONE\n00003.ts\n#EXT-X-ENDLIST\n";
        let join = Join::parse(playlist).unwrap();
        assert_eq!(join.extension(), "ts");
        let mut seven = [0u8; 16];
        seven[15] = 7;
        let mut ten = [0u8; 16];
        ten[15] = 10;
        assert_eq!(
            join.parts,
            vec![
                Part::Segment {
                    name: "00001.ts".into(),
                    key: Some(("key1.key".into(), seven))
                },
                Part::Segment {
                    name: "00002.ts".into(),
                    key: Some(("key1.key".into(), ten))
                },
                Part::Segment {
                    name: "00003.ts".into(),
                    key: None
                },
            ]
        );
        let fmp4 =
            Join::parse("#EXTM3U\n#EXT-X-MAP:URI=\"init1.mp4\"\n00001.m4s\n00002.m4s\n").unwrap();
        assert_eq!(fmp4.extension(), "mp4");
        assert!(Join::parse("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\nvideo.m3u8\n").is_err());
        assert!(Join::parse("#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"k\"\n1.ts\n").is_err());
    }

    #[test]
    fn segments_are_decrypted_in_order() {
        use aes::cipher::BlockEncryptMut;
        type Encryptor = cbc::Encryptor<aes::Aes128>;
        let dir = tempfile::tempdir().unwrap();
        let key = [3u8; 16];
        std::fs::write(dir.path().join("key1.key"), key).unwrap();
        let mut iv = [0u8; 16];
        iv[15] = 1;
        let text = b"second segment";
        let mut buffer = [0u8; 32];
        buffer[..text.len()].copy_from_slice(text);
        let encrypted = Encryptor::new(&key.into(), &iv.into())
            .encrypt_padded_mut::<Pkcs7>(&mut buffer, text.len())
            .unwrap()
            .to_vec();
        std::fs::write(dir.path().join("00001.ts"), b"first segment ").unwrap();
        std::fs::write(dir.path().join("00002.ts"), encrypted).unwrap();
        let join = Join::parse(
            "#EXTM3U\n00001.ts\n#EXT-X-KEY:METHOD=AES-128,URI=\"key1.key\"\n00002.ts\n#EXT-X-ENDLIST\n",
        )
        .unwrap();
        let mut out = Vec::new();
        join.write(dir.path(), &mut out, &AtomicBool::new(false))
            .unwrap();
        assert_eq!(out, b"first segment second segment");
        assert!(
            Join::parse("#EXTM3U\n../x.ts\n")
                .unwrap()
                .write(dir.path(), &mut Vec::new(), &AtomicBool::new(false))
                .is_err()
        );
    }
}
