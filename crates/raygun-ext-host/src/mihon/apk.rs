//! A Mihon extension's APK, read without running it: its manifest (package name, version code,
//! the source class Mihon loads) and its signing certificates, to check a downloaded APK against
//! the store that lists it before anything else is done with it.
//!
//! The manifest is Android's binary XML (a string pool, then element chunks); the signatures are
//! APK Signature Scheme v2/v3 blocks before the ZIP central directory, whose first certificate's
//! SHA-256 is what Mihon stores call `signingKeyFingerprint`.

use std::io::Read;

use sha2::{Digest, Sha256};

use super::index::StoreExtension;

/// What an APK says about itself.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ApkFacts {
    pub package: String,
    pub version_code: u64,
    pub version_name: Option<String>,
    /// `tachiyomi.extension.class`: the class (or factory) Mihon instantiates.
    pub source_class: Option<String>,
    /// Whether it declares the `tachiyomi.extension` feature, as Mihon's extensions do.
    pub is_extension: bool,
    /// SHA-256 of each signer's certificate, lowercase hex.
    pub signers: Vec<String>,
}

/// Reads an APK's manifest and signatures.
pub fn read(bytes: &[u8]) -> Result<ApkFacts, String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|error| format!("the APK isn't a ZIP archive: {error}"))?;
    let mut manifest = Vec::new();
    archive
        .by_name("AndroidManifest.xml")
        .map_err(|_| "the APK has no AndroidManifest.xml".to_owned())?
        .take(4 * 1024 * 1024)
        .read_to_end(&mut manifest)
        .map_err(|error| format!("the APK's manifest couldn't be read: {error}"))?;
    let mut facts = manifest_facts(&manifest)?;
    facts.signers = signers(bytes)?;
    Ok(facts)
}

/// Checks an APK against the store's entry: its package, its version code, and (when the store
/// names one) its signing key.
pub fn check(
    facts: &ApkFacts,
    extension: &StoreExtension,
    signing_key: Option<&str>,
) -> Result<(), String> {
    if facts.package != extension.pkg {
        return Err(format!(
            "the APK is {}, not {} as the store says",
            facts.package, extension.pkg
        ));
    }
    if i64::try_from(facts.version_code).ok() != Some(extension.version_code) {
        return Err(format!(
            "the APK is version {}, not {} as the store says",
            facts.version_code, extension.version_code
        ));
    }
    if !facts.is_extension || facts.source_class.is_none() {
        return Err("the APK isn't a Mihon extension".into());
    }
    let Some(key) = signing_key.map(|key| key.trim().to_ascii_lowercase().replace(':', "")) else {
        return Ok(());
    };
    if facts.signers.contains(&key) {
        Ok(())
    } else if facts.signers.is_empty() {
        Err("the APK's signature couldn't be read".into())
    } else {
        Err("the APK isn't signed with the store's key".into())
    }
}

// ---- Binary XML ----

const RES_STRING_POOL: u16 = 0x0001;
const RES_XML_START_ELEMENT: u16 = 0x0102;
const NO_ENTRY: u32 = 0xFFFF_FFFF;

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

/// A string pool chunk's strings (UTF-8 or UTF-16).
fn string_pool(chunk: &[u8]) -> Option<Vec<String>> {
    let count = u32_at(chunk, 8)? as usize;
    let flags = u32_at(chunk, 16)?;
    let strings_start = u32_at(chunk, 20)? as usize;
    let header = u16_at(chunk, 2)? as usize;
    let utf8 = flags & 0x100 != 0;
    let mut strings = Vec::with_capacity(count.min(100_000));
    for index in 0..count {
        let offset = u32_at(chunk, header + index * 4)? as usize;
        let at = strings_start + offset;
        let text = if utf8 {
            // Two lengths (UTF-16 units, then bytes), each one byte or two.
            let mut at = at;
            let skip_length = |at: &mut usize| -> Option<usize> {
                let first = *chunk.get(*at)? as usize;
                *at += 1;
                if first & 0x80 != 0 {
                    let second = *chunk.get(*at)? as usize;
                    *at += 1;
                    Some(((first & 0x7F) << 8) | second)
                } else {
                    Some(first)
                }
            };
            skip_length(&mut at)?;
            let length = skip_length(&mut at)?;
            String::from_utf8_lossy(chunk.get(at..at + length)?).into_owned()
        } else {
            let mut length = u16_at(chunk, at)? as usize;
            let mut at = at + 2;
            if length & 0x8000 != 0 {
                length = ((length & 0x7FFF) << 16) | u16_at(chunk, at)? as usize;
                at += 2;
            }
            let units: Vec<u16> = (0..length)
                .map(|unit| u16_at(chunk, at + unit * 2))
                .collect::<Option<_>>()?;
            String::from_utf16_lossy(&units)
        };
        strings.push(text);
    }
    Some(strings)
}

/// The manifest's package, versions and Mihon's metadata.
fn manifest_facts(xml: &[u8]) -> Result<ApkFacts, String> {
    let broken = || "the APK's manifest is damaged".to_owned();
    if u16_at(xml, 0) != Some(0x0003) {
        return Err("the APK's manifest isn't Android's binary XML".into());
    }
    let mut at = u16_at(xml, 2).ok_or_else(broken)? as usize;
    let mut strings: Vec<String> = Vec::new();
    let mut facts = ApkFacts::default();
    while at + 8 <= xml.len() {
        let kind = u16_at(xml, at).ok_or_else(broken)?;
        let size = u32_at(xml, at + 4).ok_or_else(broken)? as usize;
        if size < 8 || at + size > xml.len() {
            return Err(broken());
        }
        let chunk = &xml[at..at + size];
        match kind {
            RES_STRING_POOL => strings = string_pool(chunk).ok_or_else(broken)?,
            RES_XML_START_ELEMENT => {
                let header = u16_at(chunk, 2).ok_or_else(broken)? as usize;
                let name = string(&strings, u32_at(chunk, header + 4).ok_or_else(broken)?);
                let attribute_start = u16_at(chunk, header + 8).ok_or_else(broken)? as usize;
                let attribute_size = u16_at(chunk, header + 10).ok_or_else(broken)? as usize;
                let count = u16_at(chunk, header + 12).ok_or_else(broken)? as usize;
                let mut attributes = Vec::new();
                for index in 0..count {
                    let base = header + attribute_start + index * attribute_size;
                    let key = string(&strings, u32_at(chunk, base + 4).ok_or_else(broken)?);
                    let raw = u32_at(chunk, base + 8).ok_or_else(broken)?;
                    let data_type = *chunk.get(base + 15).ok_or_else(broken)?;
                    let data = u32_at(chunk, base + 16).ok_or_else(broken)?;
                    let value = if raw != NO_ENTRY {
                        string(&strings, raw)
                    } else {
                        match data_type {
                            0x03 => string(&strings, data),
                            0x12 => (data != 0).to_string(),
                            _ => data.to_string(),
                        }
                    };
                    attributes.push((key, value));
                }
                let get = |wanted: &str| {
                    attributes
                        .iter()
                        .find(|(key, _)| key == wanted)
                        .map(|(_, value)| value.clone())
                };
                match name.as_str() {
                    "manifest" => {
                        facts.package = get("package").unwrap_or_default();
                        facts.version_code = get("versionCode")
                            .and_then(|code| code.parse().ok())
                            .unwrap_or(0);
                        facts.version_name = get("versionName");
                    }
                    "uses-feature" if get("name").as_deref() == Some("tachiyomi.extension") => {
                        facts.is_extension = true;
                    }
                    "meta-data" if get("name").as_deref() == Some("tachiyomi.extension.class") => {
                        facts.source_class = get("value");
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        at += size;
    }
    if facts.package.is_empty() {
        return Err("the APK's manifest names no package".into());
    }
    Ok(facts)
}

fn string(strings: &[String], index: u32) -> String {
    strings.get(index as usize).cloned().unwrap_or_default()
}

// ---- Signatures ----

const SIGNATURE_V2: u32 = 0x7109_871A;
const SIGNATURE_V3: u32 = 0xF053_68C0;
const SIGNATURE_V31: u32 = 0x1B93_AD61;

/// A length-prefixed piece (`u32` little-endian length, then the bytes) at `at`, and where the
/// next one starts.
fn prefixed(bytes: &[u8], at: usize) -> Option<(&[u8], usize)> {
    let length = u32_at(bytes, at)? as usize;
    let piece = bytes.get(at + 4..at + 4 + length)?;
    Some((piece, at + 4 + length))
}

/// The SHA-256 of each signer's first certificate, from the APK Signing Block (v3 first, then
/// v2); none when the APK has only the old JAR signature.
fn signers(apk: &[u8]) -> Result<Vec<String>, String> {
    let Some(block) = signing_block(apk) else {
        return Ok(Vec::new());
    };
    let mut pairs = Vec::new();
    let mut at = 0;
    while at + 12 <= block.len() {
        let Some(length) = u64_at(block, at).map(|length| length as usize) else {
            break;
        };
        if length < 4 || at + 8 + length > block.len() {
            break;
        }
        let id = u32_at(block, at + 8).unwrap_or_default();
        pairs.push((id, &block[at + 12..at + 8 + length]));
        at += 8 + length;
    }
    for wanted in [SIGNATURE_V31, SIGNATURE_V3, SIGNATURE_V2] {
        let Some((_, value)) = pairs.iter().find(|(id, _)| *id == wanted) else {
            continue;
        };
        let mut found = Vec::new();
        let (signer_list, _) = prefixed(value, 0).ok_or("the APK's signature block is damaged")?;
        let mut at = 0;
        while let Some((signer, next)) = prefixed(signer_list, at) {
            at = next;
            let Some((signed_data, _)) = prefixed(signer, 0) else {
                continue;
            };
            // Digests, then certificates.
            let Some((_, after_digests)) = prefixed(signed_data, 0) else {
                continue;
            };
            let Some((certificates, _)) = prefixed(signed_data, after_digests) else {
                continue;
            };
            if let Some((certificate, _)) = prefixed(certificates, 0) {
                found.push(hex(&Sha256::digest(certificate)));
            }
        }
        if !found.is_empty() {
            return Ok(found);
        }
    }
    Ok(Vec::new())
}

/// The APK Signing Block's ID-value pairs, found before the ZIP central directory.
fn signing_block(apk: &[u8]) -> Option<&[u8]> {
    // The end of central directory record, within the last 64 KiB and its own 22 bytes.
    let search_from = apk.len().saturating_sub(22 + 65_535);
    let end = (search_from..apk.len().saturating_sub(21))
        .rev()
        .find(|at| apk.get(*at..*at + 4) == Some(&[0x50, 0x4B, 0x05, 0x06]))?;
    let directory = u32_at(apk, end + 16)? as usize;
    if directory < 24 || apk.get(directory - 16..directory)? != b"APK Sig Block 42" {
        return None;
    }
    let size = u64_at(apk, directory - 24)? as usize;
    let start = directory.checked_sub(size + 8)?;
    if u64_at(apk, start)? as usize != size {
        return None;
    }
    apk.get(start + 8..directory - 24)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A binary XML document of the given elements (name, attributes as `(name, value)`; a value
    /// that parses as a number is written as one).
    fn binary_xml(elements: &[(&str, &[(&str, &str)])]) -> Vec<u8> {
        let mut strings: Vec<String> = Vec::new();
        let index = |text: &str, strings: &mut Vec<String>| -> u32 {
            if let Some(at) = strings.iter().position(|known| known == text) {
                return at as u32;
            }
            strings.push(text.to_owned());
            (strings.len() - 1) as u32
        };
        let mut nodes = Vec::new();
        for (name, attributes) in elements {
            let name = index(name, &mut strings);
            let mut node = Vec::new();
            node.extend_from_slice(&RES_XML_START_ELEMENT.to_le_bytes());
            node.extend_from_slice(&16u16.to_le_bytes());
            node.extend_from_slice(&0u32.to_le_bytes()); // size, set below
            node.extend_from_slice(&1u32.to_le_bytes());
            node.extend_from_slice(&NO_ENTRY.to_le_bytes());
            node.extend_from_slice(&NO_ENTRY.to_le_bytes());
            node.extend_from_slice(&name.to_le_bytes());
            node.extend_from_slice(&20u16.to_le_bytes());
            node.extend_from_slice(&20u16.to_le_bytes());
            node.extend_from_slice(&(attributes.len() as u16).to_le_bytes());
            node.extend_from_slice(&[0; 6]);
            for (key, value) in *attributes {
                let key = index(key, &mut strings);
                node.extend_from_slice(&NO_ENTRY.to_le_bytes());
                node.extend_from_slice(&key.to_le_bytes());
                match value.parse::<u32>() {
                    Ok(number) => {
                        node.extend_from_slice(&NO_ENTRY.to_le_bytes());
                        node.extend_from_slice(&[8, 0, 0, 0x10]);
                        node.extend_from_slice(&number.to_le_bytes());
                    }
                    Err(_) => {
                        let text = index(value, &mut strings);
                        node.extend_from_slice(&text.to_le_bytes());
                        node.extend_from_slice(&[8, 0, 0, 0x03]);
                        node.extend_from_slice(&text.to_le_bytes());
                    }
                }
            }
            let size = node.len() as u32;
            node[4..8].copy_from_slice(&size.to_le_bytes());
            nodes.push(node);
        }
        // A UTF-16 string pool.
        let mut data = Vec::new();
        let mut offsets = Vec::new();
        for text in &strings {
            offsets.push(data.len() as u32);
            let units: Vec<u16> = text.encode_utf16().collect();
            data.extend_from_slice(&(units.len() as u16).to_le_bytes());
            for unit in units {
                data.extend_from_slice(&unit.to_le_bytes());
            }
            data.extend_from_slice(&[0, 0]);
        }
        let header = 28usize;
        let strings_start = header + offsets.len() * 4;
        let mut pool = Vec::new();
        pool.extend_from_slice(&RES_STRING_POOL.to_le_bytes());
        pool.extend_from_slice(&(header as u16).to_le_bytes());
        pool.extend_from_slice(&((strings_start + data.len()) as u32).to_le_bytes());
        pool.extend_from_slice(&(strings.len() as u32).to_le_bytes());
        pool.extend_from_slice(&0u32.to_le_bytes());
        pool.extend_from_slice(&0u32.to_le_bytes());
        pool.extend_from_slice(&(strings_start as u32).to_le_bytes());
        pool.extend_from_slice(&0u32.to_le_bytes());
        for offset in offsets {
            pool.extend_from_slice(&offset.to_le_bytes());
        }
        pool.extend_from_slice(&data);
        let mut xml = Vec::new();
        xml.extend_from_slice(&0x0003u16.to_le_bytes());
        xml.extend_from_slice(&8u16.to_le_bytes());
        let total = 8 + pool.len() + nodes.iter().map(Vec::len).sum::<usize>();
        xml.extend_from_slice(&(total as u32).to_le_bytes());
        xml.extend_from_slice(&pool);
        for node in nodes {
            xml.extend_from_slice(&node);
        }
        xml
    }

    #[test]
    fn reads_a_manifest_and_checks_it_against_the_store() {
        let xml = binary_xml(&[
            (
                "manifest",
                &[
                    ("package", "eu.kanade.tachiyomi.extension.en.fixture"),
                    ("versionCode", "16"),
                    ("versionName", "1.4.16"),
                ],
            ),
            ("uses-feature", &[("name", "tachiyomi.extension")]),
            (
                "meta-data",
                &[("name", "tachiyomi.extension.class"), ("value", ".Fixture")],
            ),
        ]);
        let facts = manifest_facts(&xml).unwrap();
        assert_eq!(facts.package, "eu.kanade.tachiyomi.extension.en.fixture");
        assert_eq!(facts.version_code, 16);
        assert_eq!(facts.version_name.as_deref(), Some("1.4.16"));
        assert_eq!(facts.source_class.as_deref(), Some(".Fixture"));
        assert!(facts.is_extension);

        let mut extension = StoreExtension {
            name: "Fixture".into(),
            pkg: "eu.kanade.tachiyomi.extension.en.fixture".into(),
            apk: "apk/fixture.apk".into(),
            icon: None,
            lib_version: "1.4".into(),
            version_code: 16,
            version_name: "1.4.16".into(),
            content: super::super::index::ContentWarning::Safe,
            sources: Vec::new(),
        };
        let signed = ApkFacts {
            signers: vec!["ab".repeat(32)],
            ..facts.clone()
        };
        assert_eq!(check(&signed, &extension, Some(&"AB".repeat(32))), Ok(()));
        assert!(
            check(&signed, &extension, Some(&"cd".repeat(32)))
                .unwrap_err()
                .contains("key")
        );
        assert!(
            check(&facts, &extension, Some(&"ab".repeat(32)))
                .unwrap_err()
                .contains("signature")
        );
        extension.version_code = 17;
        assert!(
            check(&signed, &extension, None)
                .unwrap_err()
                .contains("version")
        );
    }

    #[test]
    fn finds_the_signers_in_the_signing_block() {
        // An APK's tail: the signing block (one v2 signer, one certificate), then an empty
        // central directory and its end record.
        let certificate = b"a certificate";
        let mut certificates = Vec::new();
        certificates.extend_from_slice(&(certificate.len() as u32).to_le_bytes());
        certificates.extend_from_slice(certificate);
        let mut signed_data = Vec::new();
        signed_data.extend_from_slice(&0u32.to_le_bytes()); // no digests
        signed_data.extend_from_slice(&(certificates.len() as u32).to_le_bytes());
        signed_data.extend_from_slice(&certificates);
        let mut signer = Vec::new();
        signer.extend_from_slice(&(signed_data.len() as u32).to_le_bytes());
        signer.extend_from_slice(&signed_data);
        let mut signer_list = Vec::new();
        signer_list.extend_from_slice(&(signer.len() as u32).to_le_bytes());
        signer_list.extend_from_slice(&signer);
        let mut value = Vec::new();
        value.extend_from_slice(&(signer_list.len() as u32).to_le_bytes());
        value.extend_from_slice(&signer_list);
        let mut pairs = Vec::new();
        pairs.extend_from_slice(&((value.len() + 4) as u64).to_le_bytes());
        pairs.extend_from_slice(&SIGNATURE_V2.to_le_bytes());
        pairs.extend_from_slice(&value);
        let size = (pairs.len() + 24) as u64;
        let mut apk = b"local files".to_vec();
        apk.extend_from_slice(&size.to_le_bytes());
        apk.extend_from_slice(&pairs);
        apk.extend_from_slice(&size.to_le_bytes());
        apk.extend_from_slice(b"APK Sig Block 42");
        let directory = apk.len() as u32;
        apk.extend_from_slice(&[0x50, 0x4B, 0x05, 0x06]);
        apk.extend_from_slice(&[0; 12]);
        apk.extend_from_slice(&directory.to_le_bytes());
        apk.extend_from_slice(&[0, 0]);
        assert_eq!(signers(&apk).unwrap(), [hex(&Sha256::digest(certificate))]);
        assert!(signers(b"not an apk").unwrap().is_empty());
    }
}
