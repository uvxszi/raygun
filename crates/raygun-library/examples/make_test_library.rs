//! Generates a large tagged test library: `cargo run -p raygun-library --example make_test_library -- <dir> [artists] [albums] [tracks] [seconds] [album-prefix]`.
//! Defaults to 50 artists × 10 albums × 10 tracks = 5,000 one-second MP3s, each album with its
//! own single-colour cover.

use std::path::PathBuf;

use raygun_library::fixtures::{FixtureTags, tiny_png, write_mp3};

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(
        args.next()
            .expect("usage: make_test_library <dir> [artists] [albums] [tracks]"),
    );
    let mut number = |default: u32| {
        args.next()
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    };
    let (artists, albums, tracks, seconds) = (number(50), number(10), number(10), number(1));
    let prefix = args.next().unwrap_or_default();

    let mut written = 0;
    for artist in 1..=artists {
        let artist_name = format!("Artist {artist:02}");
        for album in 1..=albums {
            let album_name = format!("{prefix}Album {album:02} of {artist_name}");
            let dir = out.join(&artist_name).join(&album_name);
            std::fs::create_dir_all(&dir)?;
            let seed = artist * 31 + album * 17;
            let cover = tiny_png(
                (seed * 7 % 256) as u8,
                (seed * 13 % 256) as u8,
                (seed * 29 % 256) as u8,
            );
            for track in 1..=tracks {
                let title = format!("Song {track:02}");
                write_mp3(
                    &dir.join(format!("{track:02} {title}.mp3")),
                    seconds,
                    &FixtureTags {
                        title: Some(&title),
                        artist: Some(&artist_name),
                        album_artist: None,
                        album: Some(&album_name),
                        track_no: Some(track),
                        year: Some(2000 + (album as u16 % 27)),
                        art_png: Some(&cover),
                    },
                )?;
                written += 1;
            }
        }
    }
    println!("wrote {written} tracks to {}", out.display());
    Ok(())
}
