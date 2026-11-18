//! Messages between two devices, and the pairing text one device shows the other.

use raygun_db::sync::{SyncKind, SyncRow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Message {
    /// The joiner's first message when pairing; sent only after it checked the host's key.
    #[serde(rename_all = "camelCase")]
    Pair {
        code: String,
        device_id: String,
        name: String,
        /// The joiner's own sync port, so the host can reach it later.
        port: Option<u16>,
    },
    #[serde(rename_all = "camelCase")]
    Paired {
        device_id: String,
        name: String,
    },
    /// Opens a sync session; both sides send one.
    #[serde(rename_all = "camelCase")]
    Hello {
        device_id: String,
        name: String,
        kinds: Vec<SyncKind>,
        port: Option<u16>,
    },
    Rows {
        rows: Vec<SyncRow>,
    },
    Done {
        upto: i64,
    },
    Ack {
        upto: i64,
    },
    Refused {
        reason: String,
    },
    /// Opens a file transfer (`files`): the sender's first message, and the receiver's answer.
    #[serde(rename_all = "camelCase")]
    Files {
        device_id: String,
        name: String,
    },
    /// What the sender has for the receiver.
    FilesOffered {
        items: Vec<FileItem>,
    },
    /// What the receiver wants of them, and how much of each file it already holds.
    FilesWanted {
        wanted: Vec<WantedItem>,
    },
    /// A file's bytes follow from `offset` as raw frames, up to its size.
    FileStart {
        item: String,
        file: u32,
        offset: u64,
    },
    /// A file is complete: its SHA-256 (hex), checked by the receiver.
    FileEnd {
        item: String,
        file: u32,
        sha256: String,
    },
    /// The sender can't send an item after all (it changed or went).
    ItemSkipped {
        item: String,
        reason: String,
    },
    /// The receiver kept an item (or says why not).
    ItemStored {
        item: String,
        error: Option<String>,
    },
    /// Nothing more this time (all sent, or cancelled).
    FilesDone,
}

/// Something one device sends another as files: a downloaded song, a chapter's pages, an
/// episode, a picked cover. `meta` is what the receiver needs to keep it as its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileItem {
    pub id: String,
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub meta: serde_json::Value,
    pub files: Vec<ItemFile>,
}

impl FileItem {
    pub fn size(&self) -> u64 {
        self.files.iter().map(|file| file.size).sum()
    }
}

/// One file of an item: its name in the item (`/`-separated) and size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemFile {
    pub name: String,
    pub size: u64,
}

/// An item the receiver wants: per file, the bytes it already holds (a transfer cut off before).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WantedItem {
    pub id: String,
    pub from: Vec<u64>,
}

/// What a joiner needs to pair: where the host is, the one-time code, and the host's key, which
/// the joiner checks before it sends the code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub host: String,
    pub port: u16,
    pub code: String,
    pub device_id: String,
    pub public_key: Vec<u8>,
}

const OFFER_PREFIX: &str = "raygun-pair:";

impl Offer {
    pub fn to_text(&self) -> String {
        format!(
            "{OFFER_PREFIX}{}:{}:{}:{}:{}",
            self.host,
            self.port,
            self.code,
            self.device_id,
            crate::channel::to_hex(&self.public_key)
        )
    }

    /// Reads a pairing text. The host may be an IPv6 address, so fields are taken from the right.
    pub fn parse(text: &str) -> Option<Self> {
        // Phone keyboards capitalise the first letter of typed text.
        let text = text.trim();
        let body = text
            .get(..OFFER_PREFIX.len())
            .filter(|prefix| prefix.eq_ignore_ascii_case(OFFER_PREFIX))
            .and_then(|_| text.get(OFFER_PREFIX.len()..))?;
        let mut parts = body.rsplitn(5, ':');
        let key = parts.next()?;
        let device_id = parts.next()?;
        let code = parts.next()?;
        let port = parts.next()?.parse().ok()?;
        let host = parts.next()?.trim_matches(['[', ']']);
        let public_key = crate::channel::from_hex(key).filter(|key| key.len() == 32)?;
        (!host.is_empty() && !code.is_empty() && !device_id.is_empty()).then(|| Self {
            host: host.to_owned(),
            port,
            code: code.to_owned(),
            device_id: device_id.to_owned(),
            public_key,
        })
    }

    /// `host:port`, bracketing IPv6 hosts.
    pub fn address(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_round_trip_and_reject_damage() {
        let offer = Offer {
            host: "192.168.1.20".into(),
            port: 8830,
            code: "abcd2345".into(),
            device_id: "01a190e3-66dc-73c1-89fc-4a6e6a9bc78b".into(),
            public_key: vec![7; 32],
        };
        let text = offer.to_text();
        assert!(text.starts_with("raygun-pair:192.168.1.20:8830:abcd2345:01a190e3-"));
        assert_eq!(Offer::parse(&format!("  {text}\n")), Some(offer.clone()));
        assert_eq!(
            Offer::parse(&text.replacen("raygun", "Raygun", 1)),
            Some(offer.clone()),
            "a capitalised prefix still reads"
        );
        assert_eq!(offer.address(), "192.168.1.20:8830");

        let v6 = Offer {
            host: "fe80::1".into(),
            ..offer.clone()
        };
        let parsed = Offer::parse(&v6.to_text()).unwrap();
        assert_eq!(parsed.host, "fe80::1");
        assert_eq!(parsed.address(), "[fe80::1]:8830");

        for broken in [
            "hello",
            "raygun-pair:host:notaport:code:id:00",
            &text.replace("raygun-pair:", "raygun-pear:"),
            &text[..text.len() - 2],
        ] {
            assert_eq!(Offer::parse(broken), None, "{broken}");
        }
    }

    #[test]
    fn messages_are_tagged_json() {
        let json = serde_json::to_value(Message::Done { upto: 9 }).unwrap();
        assert_eq!(json, serde_json::json!({ "type": "done", "upto": 9 }));
        let hello: Message = serde_json::from_value(serde_json::json!({
            "type": "hello", "deviceId": "d", "name": "Phone", "kinds": ["library"], "port": null
        }))
        .unwrap();
        assert!(matches!(hello, Message::Hello { kinds, .. } if kinds == [SyncKind::Library]));
    }
}
