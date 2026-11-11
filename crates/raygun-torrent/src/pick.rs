//! Which of a torrent's files plays: the one the link names, else the wanted episode's, else the
//! largest video. Samples and extras (much smaller videos named so) come after the film.

use raygun_resolver::video_match::{self, EpisodeNumber};

use crate::link::FileHint;

/// One file of a torrent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorrentFile {
    /// Its index among the torrent's files.
    pub index: usize,
    /// Its path inside the torrent, `/` between folders.
    pub path: String,
    pub len: u64,
}

impl TorrentFile {
    /// The file's own name, without its folders.
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

const VIDEO_EXTENSIONS: [&str; 17] = [
    "mkv", "mp4", "m4v", "webm", "avi", "mov", "ts", "m2ts", "mts", "wmv", "flv", "mpg", "mpeg",
    "ogv", "3gp", "divx", "vob",
];

/// Whether a file name is a video's, by its extension.
pub fn is_video(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, extension)| {
        VIDEO_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
    })
}

/// Words that mark a video as an extra rather than the film or episode.
const EXTRA_WORDS: [&str; 6] = [
    "sample",
    "trailer",
    "teaser",
    "extras",
    "featurette",
    "preview",
];

pub(crate) fn is_extra(file: &TorrentFile) -> bool {
    file.path
        .to_ascii_lowercase()
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|word| EXTRA_WORDS.contains(&word))
}

/// Whether the file is the episode `wanted` numbers (any of its numberings), by its name or,
/// when that names no season, by its folders as well ("Season 2/03.mkv").
fn is_episode(file: &TorrentFile, wanted: &[EpisodeNumber]) -> bool {
    let fits = |info: &video_match::EpisodeInfo| {
        let Some(first) = info.episode.filter(|_| !info.weak) else {
            return false;
        };
        let last = info.last_episode.unwrap_or(first);
        wanted.iter().any(|number| {
            let episode = f64::from(number.episode);
            episode >= first - 0.01
                && episode <= last + 0.01
                && match (info.season, number.season) {
                    (Some(found), Some(asked)) => found == asked,
                    _ => true,
                }
        })
    };
    let own = video_match::parse_episode(file.name());
    if fits(&own) && own.season.is_some() {
        return true;
    }
    // The season from the folders: "Show/Season 2/Show - 03.mkv".
    let whole = video_match::parse_episode(&file.path.replace('/', " "));
    if fits(&whole) {
        return true;
    }
    own.season.is_none() && fits(&own) && wanted.iter().all(|number| number.season.is_none())
}

/// The index of the file to play, or `None` when the torrent has no video (and the link named
/// none).
pub fn pick_file(files: &[TorrentFile], hint: &FileHint) -> Option<usize> {
    if let Some(index) = hint.index
        && files.iter().any(|file| file.index == index)
    {
        return Some(index);
    }
    let videos: Vec<&TorrentFile> = files.iter().filter(|file| is_video(&file.path)).collect();
    let largest = videos.iter().map(|file| file.len).max()?;
    // Extras much smaller than the largest video stay out while anything else is left.
    let main: Vec<&TorrentFile> = videos
        .iter()
        .copied()
        .filter(|file| !(is_extra(file) && file.len < largest / 3))
        .collect();
    let videos = if main.is_empty() { videos } else { main };
    if !hint.episodes.is_empty() {
        let episode = videos
            .iter()
            .filter(|file| is_episode(file, &hint.episodes))
            .max_by_key(|file| file.len);
        if let Some(file) = episode {
            return Some(file.index);
        }
    }
    videos
        .iter()
        .max_by_key(|file| file.len)
        .map(|file| file.index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(index: usize, path: &str, mb: u64) -> TorrentFile {
        TorrentFile {
            index,
            path: path.into(),
            len: mb * 1024 * 1024,
        }
    }

    fn episode(season: Option<u32>, episode: u32) -> EpisodeNumber {
        EpisodeNumber { season, episode }
    }

    #[test]
    fn a_film_is_its_largest_video_not_its_sample_or_extras() {
        let files = [
            file(0, "Sintel/Sintel.2010.1080p.srt", 1),
            file(1, "Sintel/Sample/sintel-sample.mkv", 40),
            file(2, "Sintel/Sintel.2010.1080p.mkv", 1200),
            file(3, "Sintel/poster.jpg", 2),
            file(4, "Sintel/Extras/Making of.mp4", 300),
        ];
        assert_eq!(pick_file(&files, &FileHint::default()), Some(2));
    }

    #[test]
    fn the_index_the_link_names_wins() {
        let files = [file(0, "a.mkv", 900), file(1, "b.mkv", 100)];
        let hint = FileHint {
            index: Some(1),
            episodes: Vec::new(),
        };
        assert_eq!(pick_file(&files, &hint), Some(1));
        // An index the torrent doesn't have is ignored.
        let hint = FileHint {
            index: Some(7),
            episodes: Vec::new(),
        };
        assert_eq!(pick_file(&files, &hint), Some(0));
    }

    #[test]
    fn a_seasons_torrent_opens_the_episode_wanted() {
        let files = [
            file(0, "Show S01/Show.S01E01.1080p.mkv", 700),
            file(1, "Show S01/Show.S01E02.1080p.mkv", 650),
            file(2, "Show S01/Show.S01E03.1080p.mkv", 720),
        ];
        let hint = FileHint {
            index: None,
            episodes: vec![episode(Some(1), 2)],
        };
        assert_eq!(pick_file(&files, &hint), Some(1));
        // Another season's episode isn't taken for it: the largest plays.
        let hint = FileHint {
            index: None,
            episodes: vec![episode(Some(2), 2)],
        };
        assert_eq!(pick_file(&files, &hint), Some(2));
    }

    #[test]
    fn folders_can_name_the_season_and_numbers_across_seasons_count() {
        let files = [
            file(0, "Show/Season 1/Show - 03.mkv", 500),
            file(1, "Show/Season 2/Show - 03.mkv", 500),
            file(2, "Show/Season 2/Show - 04.mkv", 600),
        ];
        let hint = FileHint {
            index: None,
            episodes: vec![episode(Some(2), 3)],
        };
        assert_eq!(pick_file(&files, &hint), Some(1));

        let anime = [
            file(0, "[Group] Title - 13 (1080p).mkv", 400),
            file(1, "[Group] Title - 14 (1080p).mkv", 380),
        ];
        let hint = FileHint {
            index: None,
            episodes: vec![episode(Some(2), 1), episode(None, 14)],
        };
        assert_eq!(pick_file(&anime, &hint), Some(1));
    }

    #[test]
    fn a_torrent_without_video_has_nothing_to_play() {
        let files = [file(0, "book.epub", 2), file(1, "cover.jpg", 1)];
        assert_eq!(pick_file(&files, &FileHint::default()), None);
        assert!(is_video("A.MKV"));
        assert!(!is_video("a.mkv.part"));
    }
}
