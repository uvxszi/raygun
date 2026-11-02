//! Who may use the remote: one-time pairing codes and the devices they admitted. Pure, with the
//! time passed in, so expiry and lockout are testable.

use serde::{Deserialize, Serialize};

/// How long a pairing code works.
pub const CODE_LIFETIME_MS: i64 = 10 * 60_000;
/// Wrong codes tolerated before the current code is cancelled.
pub const MAX_FAILURES: u32 = 5;
const CODE_LENGTH: usize = 10;
/// No 0/O or 1/I/l, so a code read aloud or typed survives.
const CODE_ALPHABET: &[u8] = b"23456789abcdefghjkmnpqrstuvwxyz";
const MAX_NAME_CHARS: usize = 40;

/// A browser that paired. Stored device-local by the app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairedDevice {
    pub id: String,
    pub name: String,
    pub token: String,
    /// Unix ms.
    pub paired_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairError {
    /// No code is open, it expired, or it was cancelled after too many failures.
    NoCode,
    WrongCode,
}

#[derive(Debug, Default)]
pub struct Pairing {
    devices: Vec<PairedDevice>,
    code: Option<(String, i64)>,
    failures: u32,
}

impl Pairing {
    pub fn new(devices: Vec<PairedDevice>) -> Self {
        Self {
            devices,
            ..Self::default()
        }
    }

    pub fn devices(&self) -> &[PairedDevice] {
        &self.devices
    }

    /// Opens a new code, replacing any earlier one.
    pub fn new_code(&mut self, now_ms: i64) -> String {
        let code = random_code();
        self.code = Some((code.clone(), now_ms + CODE_LIFETIME_MS));
        self.failures = 0;
        code
    }

    /// The open code, unless it expired.
    pub fn current_code(&self, now_ms: i64) -> Option<&str> {
        self.code
            .as_ref()
            .filter(|(_, expires)| now_ms < *expires)
            .map(|(code, _)| code.as_str())
    }

    /// Admits a device with the open code. The code works once.
    pub fn pair(&mut self, code: &str, name: &str, now_ms: i64) -> Result<PairedDevice, PairError> {
        let Some(open) = self.current_code(now_ms) else {
            return Err(PairError::NoCode);
        };
        if !constant_time_eq(open.as_bytes(), code.trim().to_lowercase().as_bytes()) {
            self.failures += 1;
            if self.failures >= MAX_FAILURES {
                self.code = None;
            }
            return Err(PairError::WrongCode);
        }
        self.code = None;
        self.failures = 0;
        let device = new_device(name, now_ms);
        self.devices.push(device.clone());
        Ok(device)
    }

    /// Admits a device this device's user allowed (Connect, M46.15): no code is needed.
    pub fn admit(&mut self, name: &str, now_ms: i64) -> PairedDevice {
        let device = new_device(name, now_ms);
        self.devices.push(device.clone());
        device
    }

    /// The paired device holding this token.
    pub fn authorize(&self, token: &str) -> Option<&PairedDevice> {
        if token.is_empty() {
            return None;
        }
        self.devices
            .iter()
            .find(|device| constant_time_eq(device.token.as_bytes(), token.as_bytes()))
    }

    /// Removes a device; its token stops working at once.
    pub fn forget(&mut self, id: &str) -> bool {
        let before = self.devices.len();
        self.devices.retain(|device| device.id != id);
        self.devices.len() != before
    }
}

fn new_device(name: &str, now_ms: i64) -> PairedDevice {
    let name: String = name.trim().chars().take(MAX_NAME_CHARS).collect();
    PairedDevice {
        id: uuid::Uuid::now_v7().to_string(),
        name: if name.is_empty() {
            "Browser".into()
        } else {
            name
        },
        token: format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        ),
        paired_at: now_ms,
    }
}

fn random_code() -> String {
    let bytes = uuid::Uuid::new_v4().into_bytes();
    bytes
        .iter()
        .take(CODE_LENGTH)
        .map(|byte| CODE_ALPHABET[*byte as usize % CODE_ALPHABET.len()] as char)
        .collect()
}

pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_admits_one_device_once() {
        let mut pairing = Pairing::default();
        let code = pairing.new_code(0);
        assert_eq!(code.len(), CODE_LENGTH);
        assert!(code.bytes().all(|byte| CODE_ALPHABET.contains(&byte)));

        let device = pairing
            .pair(
                &format!(" {} ", code.to_uppercase()),
                "  Kitchen tablet ",
                1,
            )
            .unwrap();
        assert_eq!(device.name, "Kitchen tablet");
        assert_eq!(device.token.len(), 64);
        assert_eq!(pairing.authorize(&device.token), Some(&device));
        assert_eq!(
            pairing.pair(&code, "Again", 2),
            Err(PairError::NoCode),
            "single use"
        );
    }

    #[test]
    fn codes_expire_and_lock_after_too_many_failures() {
        let mut pairing = Pairing::default();
        let code = pairing.new_code(0);
        assert_eq!(
            pairing.pair(&code, "Late", CODE_LIFETIME_MS),
            Err(PairError::NoCode)
        );

        let code = pairing.new_code(0);
        for _ in 0..MAX_FAILURES - 1 {
            assert_eq!(pairing.pair("wrong", "x", 1), Err(PairError::WrongCode));
        }
        assert!(pairing.current_code(1).is_some());
        assert_eq!(pairing.pair("wrong", "x", 1), Err(PairError::WrongCode));
        assert_eq!(
            pairing.pair(&code, "x", 1),
            Err(PairError::NoCode),
            "cancelled"
        );

        let fresh = pairing.new_code(2);
        assert!(
            pairing.pair(&fresh, "", 3).is_ok(),
            "a new code resets the count"
        );
        assert_eq!(pairing.devices()[0].name, "Browser");
    }

    #[test]
    fn forgotten_devices_lose_access() {
        let mut pairing = Pairing::default();
        let code = pairing.new_code(0);
        let device = pairing.pair(&code, "Phone", 1).unwrap();
        assert!(pairing.forget(&device.id));
        assert!(!pairing.forget(&device.id));
        assert_eq!(pairing.authorize(&device.token), None);
        assert_eq!(pairing.authorize(""), None);
    }
}
