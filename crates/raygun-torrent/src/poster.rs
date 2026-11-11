//! A torrent's own poster: a picture that comes with the video ("Sintel/poster.jpg"), fetched
//! with it so a link played on its own has a picture on its page and in Continue watching.

use crate::pick::TorrentFile;

/// The largest picture taken for a poster.
pub const MAX_POSTER_BYTES: u64 = 8 * 1024 * 1024;

const IMAGE_EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "png", "webp"];

/// Names a poster goes by, best first.
const POSTER_NAMES: [&str; 5] = ["poster", "cover", "folder", "front", "thumb"];

/// Whether a file name is a picture's, by its extension.
pub fn is_image(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, extension)| {
        IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
    })
}

/// A picture's format for its saved copy: `jpg`, `png` or `webp`.
pub fn image_extension(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "png"
    } else if lower.ends_with(".webp") {
        "webp"
    } else {
        "jpg"
    }
}

/// The poster among a torrent's `files`: a picture named as one ("poster.jpg", "cover.png",
/// "folder.jpg"), else its only picture; none when it holds several pictures that aren't named so
/// (a comic's pages, screenshots) or the only ones are too large.
pub fn poster_file(files: &[TorrentFile]) -> Option<&TorrentFile> {
    let pictures: Vec<&TorrentFile> = files
        .iter()
        .filter(|file| is_image(file.name()) && file.len > 0)
        .collect();
    let rank = |file: &TorrentFile| {
        let name = file.name().to_ascii_lowercase();
        let stem = name
            .rsplit_once('.')
            .map_or(name.as_str(), |(stem, _)| stem);
        POSTER_NAMES
            .iter()
            .position(|known| stem == *known || stem.ends_with(&format!("-{known}")))
    };
    let named = pictures
        .iter()
        .copied()
        .filter(|file| file.len <= MAX_POSTER_BYTES)
        .filter_map(|file| rank(file).map(|rank| (rank, file)))
        .min_by_key(|(rank, file)| (*rank, file.path.matches('/').count(), file.index))
        .map(|(_, file)| file);
    let lone = match pictures.as_slice() {
        [only] if only.len <= MAX_POSTER_BYTES => Some(*only),
        _ => None,
    };
    named.or(lone)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(index: usize, path: &str, len: u64) -> TorrentFile {
        TorrentFile {
            index,
            path: path.into(),
            len,
        }
    }

    #[test]
    fn a_picture_named_as_a_poster_is_the_poster() {
        let sintel = [
            file(0, "Sintel/Sintel.de.srt", 1_000),
            file(1, "Sintel/Sintel.mp4", 129_000_000),
            file(2, "Sintel/poster.jpg", 46_000),
            file(3, "Sintel/Sintel.en.srt", 1_000),
        ];
        assert_eq!(poster_file(&sintel).map(|found| found.index), Some(2));
        // "poster" before "cover", and the one nearer the top.
        let several = [
            file(0, "Film/Extras/cover.jpg", 10_000),
            file(1, "Film/Extras/poster.png", 10_000),
            file(2, "Film/poster.jpg", 10_000),
        ];
        assert_eq!(poster_file(&several).map(|found| found.index), Some(2));
        assert_eq!(image_extension("Film/poster.PNG"), "png");
        assert_eq!(image_extension("poster.jpeg"), "jpg");
    }

    #[test]
    fn a_lone_picture_counts_and_many_unnamed_ones_dont() {
        let lone = [file(0, "a.mkv", 900), file(1, "Film (2010).jpg", 300)];
        assert_eq!(poster_file(&lone).map(|found| found.index), Some(1));
        let pages = [
            file(0, "a.mkv", 900),
            file(1, "shots/001.jpg", 300),
            file(2, "shots/002.jpg", 300),
        ];
        assert_eq!(poster_file(&pages), None);
        let huge = [file(0, "a.mkv", 900), file(1, "poster.png", 40_000_000)];
        assert_eq!(poster_file(&huge), None);
        assert_eq!(poster_file(&[file(0, "a.mkv", 900)]), None);
    }
}
