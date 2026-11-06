//! Raygun backup files (M46.51): a header line, the manifest on one JSON line, then the backup
//! itself as gzipped JSON. A listing reads only the start of each file to describe it. Files
//! made before (version 1, gzipped JSON alone) are told apart by content and read in full.

use std::io::{Read, Write};

use raygun_db::backup::{self as db_backup, BackupData, Item, Part};
use raygun_model::{BackupManifest, BackupSection, DataCategory, DataCount, DataItem};

pub const EXTENSION: &str = "raygunbk";
/// The first line, before the format version.
const MAGIC: &[u8] = b"RAYGUN-BACKUP ";
/// How much of a file is read to find its manifest.
pub const HEADER_BYTES: usize = 256 * 1024;
/// Bigger files are refused before reading them in full (a library backup is far smaller).
pub const MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 512 * 1024 * 1024;

/// What the start of a file is.
#[derive(Debug, PartialEq)]
pub enum Sniffed {
    /// A backup that describes itself.
    Described(BackupManifest),
    /// Gzipped data: a version 1 backup, or another app's (Mihon's) backup.
    Packed,
    /// Neither.
    Other,
}

/// Reads what the start of a file says about it.
pub fn sniff(prefix: &[u8]) -> Result<Sniffed, String> {
    if let Some(rest) = prefix.strip_prefix(MAGIC) {
        let (version, rest) = line(rest).ok_or("The backup's header is cut short")?;
        let version: u32 = std::str::from_utf8(version)
            .ok()
            .and_then(|text| text.trim().parse().ok())
            .ok_or("The backup's header is damaged")?;
        if version > db_backup::VERSION {
            return Err("This backup is from a newer Raygun; update the app first".into());
        }
        let (manifest, _) = line(rest).ok_or("The backup's description is cut short")?;
        let manifest: BackupManifest = serde_json::from_slice(manifest)
            .map_err(|error| format!("The backup's description is damaged ({error})"))?;
        return Ok(Sniffed::Described(manifest));
    }
    if prefix.starts_with(&[0x1f, 0x8b]) {
        return Ok(Sniffed::Packed);
    }
    Ok(Sniffed::Other)
}

/// Splits at the first newline.
fn line(bytes: &[u8]) -> Option<(&[u8], &[u8])> {
    let end = bytes.iter().position(|byte| *byte == b'\n')?;
    Some((&bytes[..end], &bytes[end + 1..]))
}

/// The file for a backup.
pub fn encode(manifest: &BackupManifest, data: &BackupData) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(manifest.version.to_string().as_bytes());
    bytes.push(b'\n');
    serde_json::to_writer(&mut bytes, manifest).map_err(to_message)?;
    bytes.push(b'\n');
    let json = serde_json::to_vec(data).map_err(to_message)?;
    let mut encoder = flate2::write::GzEncoder::new(bytes, flate2::Compression::default());
    encoder.write_all(&json).map_err(to_message)?;
    encoder.finish().map_err(to_message)
}

/// What a whole file holds.
pub enum Decoded {
    Raygun {
        manifest: BackupManifest,
        data: Box<BackupData>,
    },
    /// Not a Raygun backup: the unpacked bytes, for the Mihon reader.
    Other(Vec<u8>),
}

/// Reads a whole file: a Raygun backup of either version, or another app's (unpacked).
pub fn decode(bytes: &[u8]) -> Result<Decoded, String> {
    if let Some(rest) = bytes.strip_prefix(MAGIC) {
        let manifest = match sniff(bytes)? {
            Sniffed::Described(manifest) => manifest,
            _ => unreachable!("the magic was read"),
        };
        let (_, rest) = line(rest).ok_or("The backup is cut short")?;
        let (_, body) = line(rest).ok_or("The backup is cut short")?;
        let data = parse_data(&gunzip(body)?)?;
        return Ok(Decoded::Raygun {
            manifest,
            data: Box::new(data),
        });
    }
    let unpacked = gunzip(bytes)?;
    if unpacked.first() != Some(&b'{') {
        return Ok(Decoded::Other(unpacked));
    }
    let data = parse_data(&unpacked)?;
    Ok(Decoded::Raygun {
        manifest: described(&data),
        data: Box::new(data),
    })
}

fn parse_data(json: &[u8]) -> Result<BackupData, String> {
    let data: BackupData = serde_json::from_slice(json)
        .map_err(|error| format!("This backup is damaged ({error})"))?;
    if data.format != db_backup::FORMAT {
        return Err("This isn't a Raygun backup".into());
    }
    if data.version > db_backup::VERSION {
        return Err("This backup is from a newer Raygun; update the app first".into());
    }
    Ok(data)
}

/// A manifest for a version 1 backup, from what it holds.
fn described(data: &BackupData) -> BackupManifest {
    let categories: Vec<DataCategory> = db_backup::parts_in(data)
        .into_iter()
        .map(category_of)
        .collect();
    BackupManifest {
        version: data.version,
        created_at: data.created_at as f64,
        device_id: data.device_id.clone(),
        device_name: String::new(),
        app_version: String::new(),
        platform: String::new(),
        secrets: false,
        sections: sections(data, &categories, &[]),
    }
}

/// Counts of the app's own values, added to what the database part counts.
pub type ExtraCounts = [(DataCategory, DataItem, u32)];

/// The sections of a backup holding `categories`: every one of them, counted.
pub fn sections(
    data: &BackupData,
    categories: &[DataCategory],
    extra: &ExtraCounts,
) -> Vec<BackupSection> {
    let index = db_backup::index(data);
    let mut sorted = categories.to_vec();
    sorted.sort();
    sorted.dedup();
    sorted
        .into_iter()
        .map(|category| {
            let mut counts: Vec<DataCount> = part_of(category)
                .and_then(|part| index.get(&part))
                .into_iter()
                .flatten()
                .map(|(item, count)| DataCount {
                    item: item_of(*item),
                    count: *count,
                    bytes: None,
                })
                .collect();
            for (_, item, count) in extra.iter().filter(|(of, _, _)| *of == category) {
                match counts.iter_mut().find(|known| known.item == *item) {
                    Some(known) => known.count = *count,
                    None if *count > 0 => counts.push(DataCount {
                        item: *item,
                        count: *count,
                        bytes: None,
                    }),
                    None => {}
                }
            }
            BackupSection { category, counts }
        })
        .collect()
}

/// The database part a category is kept in, if it's kept there.
pub fn part_of(category: DataCategory) -> Option<Part> {
    Some(match category {
        DataCategory::Comics => Part::Comics,
        DataCategory::Books => Part::Books,
        DataCategory::Videos => Part::Videos,
        DataCategory::Podcasts => Part::Podcasts,
        DataCategory::Audiobooks => Part::Audiobooks,
        DataCategory::Websites => Part::Websites,
        DataCategory::Playlists => Part::Playlists,
        DataCategory::Pins => Part::Pins,
        DataCategory::History => Part::History,
        DataCategory::Settings => Part::Settings,
        DataCategory::Sources => Part::Sources,
        DataCategory::Downloads => Part::Downloads,
        DataCategory::LocalFiles => Part::LocalFiles,
        DataCategory::Searches | DataCategory::AiModels | DataCategory::Caches => return None,
    })
}

pub fn category_of(part: Part) -> DataCategory {
    match part {
        Part::Comics => DataCategory::Comics,
        Part::Books => DataCategory::Books,
        Part::Videos => DataCategory::Videos,
        Part::Podcasts => DataCategory::Podcasts,
        Part::Audiobooks => DataCategory::Audiobooks,
        Part::Websites => DataCategory::Websites,
        Part::Playlists => DataCategory::Playlists,
        Part::Pins => DataCategory::Pins,
        Part::History => DataCategory::History,
        Part::Settings => DataCategory::Settings,
        Part::Sources => DataCategory::Sources,
        Part::Downloads => DataCategory::Downloads,
        Part::LocalFiles => DataCategory::LocalFiles,
    }
}

pub fn item_of(item: Item) -> DataItem {
    match item {
        Item::Titles => DataItem::Titles,
        Item::Chapters => DataItem::Chapters,
        Item::Read => DataItem::Read,
        Item::Episodes => DataItem::Episodes,
        Item::Watched => DataItem::Watched,
        Item::Categories => DataItem::Categories,
        Item::Shows => DataItem::Shows,
        Item::Books => DataItem::Books,
        Item::Sites => DataItem::Sites,
        Item::Playlists => DataItem::Playlists,
        Item::PlaylistSongs => DataItem::PlaylistSongs,
        Item::Favorites => DataItem::Favorites,
        Item::Pins => DataItem::Pins,
        Item::Played => DataItem::Played,
        Item::Settings => DataItem::Settings,
        Item::Repositories => DataItem::Repositories,
        Item::Extensions => DataItem::Extensions,
        Item::Addons => DataItem::Addons,
        Item::Plays => DataItem::Plays,
        Item::Songs => DataItem::Songs,
        Item::Videos => DataItem::Videos,
        Item::Waiting => DataItem::Waiting,
        Item::Folders => DataItem::Folders,
    }
}

/// `raygun-<yyyymmdd-hhmmss>.raygunbk`, its time in UTC.
pub fn file_name(now_ms: i64) -> String {
    format!("raygun-{}.{EXTENSION}", stamp(now_ms))
}

/// Whether a file's name looks like a backup's: `….raygunbk` (Android may add `.bin` or a
/// ` (1)` to a name it was given), never a file being written.
pub fn is_backup_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains(&format!(".{EXTENSION}")) && !lower.ends_with(".part")
}

/// A name inside the data folder's backups: `raygun-<digits and dashes>.raygunbk`, so it never
/// leaves the folder.
pub fn is_own_name(name: &str) -> bool {
    name.strip_prefix("raygun-")
        .and_then(|rest| rest.strip_suffix(&format!(".{EXTENSION}")))
        .is_some_and(|stamp| {
            !stamp.is_empty()
                && stamp
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || byte == b'-')
        })
}

/// Decompresses a gzip stream (bounded, against decompression bombs); other bytes pass through.
pub fn gunzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if !bytes.starts_with(&[0x1f, 0x8b]) {
        return Ok(bytes.to_vec());
    }
    let mut unpacked = Vec::new();
    flate2::read::MultiGzDecoder::new(bytes)
        .take(MAX_UNPACKED_BYTES + 1)
        .read_to_end(&mut unpacked)
        .map_err(|_| "The file is damaged".to_owned())?;
    if unpacked.len() as u64 > MAX_UNPACKED_BYTES {
        return Err("This file is too large to be a backup".into());
    }
    Ok(unpacked)
}

/// `yyyymmdd-hhmmss` (UTC) for a unix ms time.
fn stamp(now_ms: i64) -> String {
    let seconds = now_ms.div_euclid(1000);
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn data(rows: Vec<(&str, serde_json::Value)>) -> BackupData {
        let mut tables: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();
        for (table, row) in rows {
            tables.entry(table.to_owned()).or_default().push(row);
        }
        BackupData {
            format: db_backup::FORMAT.into(),
            version: db_backup::VERSION,
            created_at: 1_793_464_215_000,
            device_id: "device-a".into(),
            tables,
            values: BTreeMap::new(),
        }
    }

    fn sample() -> BackupData {
        data(vec![
            (
                "title",
                serde_json::json!({"id": "t1", "kind": "manga", "user_in_library_at": 5, "deleted": 0}),
            ),
            (
                "unit",
                serde_json::json!({"id": "u1", "title_id": "t1", "deleted": 0}),
            ),
            (
                "setting",
                serde_json::json!({"key": "theme", "value": "\"dark\"", "deleted": 0}),
            ),
        ])
    }

    fn manifest(data: &BackupData) -> BackupManifest {
        let categories = [
            DataCategory::Comics,
            DataCategory::Settings,
            DataCategory::Searches,
        ];
        BackupManifest {
            version: db_backup::VERSION,
            created_at: data.created_at as f64,
            device_id: data.device_id.clone(),
            device_name: "Pixel".into(),
            app_version: "1.2.0".into(),
            platform: "android".into(),
            secrets: false,
            sections: sections(
                data,
                &categories,
                &[(DataCategory::Searches, DataItem::Searches, 3)],
            ),
        }
    }

    #[test]
    fn the_manifest_is_read_from_the_start_of_the_file() {
        let data = sample();
        let manifest = manifest(&data);
        let bytes = encode(&manifest, &data).unwrap();
        // A listing reads only the head of a file.
        let head = &bytes[..bytes.len().min(HEADER_BYTES)];
        assert_eq!(sniff(head).unwrap(), Sniffed::Described(manifest.clone()));
        let section = |category| {
            manifest
                .sections
                .iter()
                .find(|section| section.category == category)
                .unwrap()
                .counts
                .iter()
                .map(|count| (count.item, count.count))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            section(DataCategory::Comics),
            [(DataItem::Titles, 1), (DataItem::Chapters, 1)]
        );
        assert_eq!(section(DataCategory::Settings), [(DataItem::Settings, 1)]);
        assert_eq!(section(DataCategory::Searches), [(DataItem::Searches, 3)]);

        match decode(&bytes).unwrap() {
            Decoded::Raygun {
                manifest: read,
                data: inside,
            } => {
                assert_eq!(read, manifest);
                assert_eq!(*inside, data);
            }
            Decoded::Other(_) => panic!("a Raygun backup"),
        }
    }

    #[test]
    fn a_version_1_backup_describes_itself_from_its_contents() {
        let mut old = sample();
        old.version = 1;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder
            .write_all(&serde_json::to_vec(&old).unwrap())
            .unwrap();
        let bytes = encoder.finish().unwrap();
        assert_eq!(sniff(&bytes).unwrap(), Sniffed::Packed);
        let Decoded::Raygun { manifest, .. } = decode(&bytes).unwrap() else {
            panic!("a Raygun backup");
        };
        assert_eq!(
            (manifest.version, manifest.device_id.as_str()),
            (1, "device-a")
        );
        let categories: Vec<_> = manifest
            .sections
            .iter()
            .map(|section| section.category)
            .collect();
        assert_eq!(categories, [DataCategory::Comics, DataCategory::Settings]);
    }

    #[test]
    fn other_files_are_told_apart_by_content() {
        assert_eq!(sniff(b"PK\x03\x04 a zip").unwrap(), Sniffed::Other);
        assert_eq!(sniff(b"").unwrap(), Sniffed::Other);
        assert!(sniff(b"RAYGUN-BACKUP 2\n{not json").is_err());
        assert!(
            sniff(b"RAYGUN-BACKUP 99\n{}\n")
                .unwrap_err()
                .contains("newer")
        );
        // Mihon's backups are gzipped protobuf: handed on unpacked.
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(b"\x0a\x02hi").unwrap();
        let mihon = encoder.finish().unwrap();
        assert!(matches!(decode(&mihon).unwrap(), Decoded::Other(bytes) if bytes == b"\x0a\x02hi"));
    }

    #[test]
    fn names_stay_in_the_folder() {
        assert!(is_own_name("raygun-20261031-174500.raygunbk"));
        assert!(!is_own_name("raygun-.raygunbk"));
        assert!(!is_own_name("raygun-../../x.raygunbk"));
        assert!(!is_own_name("other.raygunbk"));
        assert!(!is_own_name("raygun-1.raygunbk.part"));
        assert!(is_backup_name("raygun-20261031-174500 (1).raygunbk"));
        assert!(is_backup_name("Raygun-20261031.RAYGUNBK.bin"));
        assert!(!is_backup_name("raygun-1.raygunbk.part"));
        assert!(!is_backup_name("notes.txt"));
    }

    #[test]
    fn stamps_read_as_utc_dates() {
        assert_eq!(stamp(0), "19700101-000000");
        assert_eq!(stamp(1_793_464_215_000), "20261031-163015");
        assert_eq!(stamp(951_782_400_000), "20000229-000000", "leap day");
        assert_eq!(
            file_name(1_793_464_215_000),
            "raygun-20261031-163015.raygunbk"
        );
    }

    #[test]
    fn gzip_bounds_and_plain_bytes() {
        assert_eq!(gunzip(b"\x0a\x02hi").unwrap(), b"\x0a\x02hi");
        assert!(gunzip(&[0x1f, 0x8b, 0, 1, 2]).is_err());
    }
}
