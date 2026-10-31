use std::fs;

use tempfile::TempDir;

use super::fixtures::{FixtureTags, tiny_png, write_mp3};
use super::*;

fn entry_for(path: &Path) -> FileEntry {
    discover(&[path.parent().unwrap().to_path_buf()])
        .into_iter()
        .find(|entry| entry.path == path)
        .expect("fixture is discovered")
}

#[test]
fn discovers_audio_files_recursively_and_skips_the_rest() {
    let temp = TempDir::new().unwrap();
    let music = temp.path().join("Music");
    fs::create_dir_all(music.join("Artist/Album")).unwrap();
    fs::create_dir_all(music.join(".thumbnails")).unwrap();
    write_mp3(
        &music.join("Artist/Album/01.mp3"),
        1,
        &FixtureTags::default(),
    )
    .unwrap();
    fs::write(music.join("Artist/Album/cover.jpg"), b"jpg").unwrap();
    fs::write(music.join("Artist/Album/02.FLAC"), b"not really flac").unwrap();
    fs::write(music.join(".thumbnails/hidden.mp3"), b"x").unwrap();

    let found = discover(std::slice::from_ref(&music));

    let names: Vec<_> = found
        .iter()
        .map(|entry| entry.path.strip_prefix(&music).unwrap().to_path_buf())
        .collect();
    assert_eq!(
        names,
        vec![
            PathBuf::from("Artist/Album/01.mp3"),
            PathBuf::from("Artist/Album/02.FLAC")
        ]
    );
    assert!(found[0].size > 0);
    assert!(found[0].modified > 0);
}

#[test]
fn missing_roots_are_ignored() {
    assert!(discover(&[PathBuf::from("/definitely/not/here")]).is_empty());
}

#[test]
fn reads_tags_duration_and_artwork() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("song.mp3");
    let png = tiny_png(200, 30, 30);
    write_mp3(
        &path,
        3,
        &FixtureTags {
            title: Some("Laser Beam"),
            artist: Some("The Raygunners"),
            album_artist: Some("Various Artists"),
            album: Some("Pew Pew"),
            track_no: Some(7),
            year: Some(2026),
            art_png: Some(&png),
        },
    )
    .unwrap();
    let art_dir = temp.path().join("art");

    let track = read_track(entry_for(&path), &art_dir).unwrap();

    assert_eq!(track.title, "Laser Beam");
    assert_eq!(track.artist.as_deref(), Some("The Raygunners"));
    assert_eq!(track.album_artist.as_deref(), Some("Various Artists"));
    assert_eq!(track.album.as_deref(), Some("Pew Pew"));
    assert_eq!(track.track_no, Some(7));
    assert_eq!(track.year, Some(2026));
    assert_eq!(track.group_artist(), "Various Artists");
    let duration = track.duration_ms.unwrap();
    assert!(
        (2_500..=3_500).contains(&duration),
        "duration was {duration}"
    );
    let art = track.art.unwrap();
    assert!(is_artwork_name(&art));
    assert_eq!(fs::read(art_dir.join(&art)).unwrap(), png);
}

#[test]
fn untagged_files_are_titled_after_the_file_name() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("Untitled Demo.mp3");
    write_mp3(&path, 1, &FixtureTags::default()).unwrap();

    let track = read_track(entry_for(&path), &temp.path().join("art")).unwrap();

    assert_eq!(track.title, "Untitled Demo");
    assert_eq!(track.group_artist(), UNKNOWN_ARTIST);
    assert_eq!(track.group_album(), UNKNOWN_ALBUM);
    assert_eq!(track.art, None);
}

#[test]
fn identical_artwork_is_stored_once() {
    let temp = TempDir::new().unwrap();
    let png = tiny_png(10, 120, 200);
    let art_dir = temp.path().join("art");
    let mut names = Vec::new();
    for index in 1..=3 {
        let path = temp.path().join(format!("{index}.mp3"));
        write_mp3(
            &path,
            1,
            &FixtureTags {
                album: Some("Same Cover"),
                art_png: Some(&png),
                ..FixtureTags::default()
            },
        )
        .unwrap();
        names.push(read_track(entry_for(&path), &art_dir).unwrap().art.unwrap());
    }

    assert!(names.iter().all(|name| name == &names[0]));
    assert_eq!(fs::read_dir(&art_dir).unwrap().count(), 1);
}

#[test]
fn unparseable_files_are_errors() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("broken.flac");
    fs::write(&path, b"this is not a flac file").unwrap();

    assert!(read_track(entry_for(&path), &temp.path().join("art")).is_err());
}

#[test]
fn artwork_names_are_strictly_validated() {
    let hash = "a".repeat(64);
    assert!(is_artwork_name(&format!("{hash}.jpg")));
    assert!(!is_artwork_name("../../etc/passwd"));
    assert!(!is_artwork_name(&format!("{hash}.exe")));
    assert!(!is_artwork_name("abc.png"));
}
