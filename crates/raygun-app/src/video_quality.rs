//! Picture sizes of video links and stream variants: read from labels, file names and playlist
//! attributes, and the one rule that picks among a video's sizes for playing and downloading
//! (`VideoPlayerPrefs::preferred_resolution`). The players cap adaptive streams the same way
//! (`PlayerTracks.kt`, `video/mediaEngine.ts`).

use raygun_model::VideoResolution;

/// The height a picture counts as: what a 16:9 picture with the same long side has, so 1920×800
/// (a wide film) and 1080×1920 (an upright video) are both 1080p. Rounded, so 854×480 is 480p.
pub fn picture_height(width: u32, height: u32) -> u32 {
    let long = u64::from(width.max(height));
    ((long * 9 + 8) / 16) as u32
}

/// A height from an HLS `RESOLUTION` value ("1920x1080").
pub fn resolution_height(value: &str) -> Option<u32> {
    let (width, height) = value.trim().split_once(['x', 'X'])?;
    let (width, height) = (width.parse().ok()?, height.parse().ok()?);
    (width > 0 && height > 0).then(|| picture_height(width, height))
}

/// A rough height for a stream known only by its bit rate (an HLS variant without
/// `RESOLUTION`), from common H.264 ladders.
pub fn bandwidth_height(bits_per_second: u64) -> u32 {
    match bits_per_second {
        12_000_000.. => 2160,
        7_000_000.. => 1440,
        3_500_000.. => 1080,
        1_800_000.. => 720,
        900_000.. => 480,
        _ => 360,
    }
}

/// Heights a bare number in a label can mean.
const KNOWN_HEIGHTS: [u32; 9] = [4320, 2160, 1440, 1080, 720, 576, 480, 360, 240];

/// The height a label, name or file name gives: "1080p", "720i", "1920x1080", "4K", "UHD",
/// "Full HD", "FHD", "QHD", then a bare "1080" or "720", then "HD" (720) or "SD" (480).
pub fn label_height(text: &str) -> Option<u32> {
    let tokens: Vec<String> = text
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let explicit = |index: usize, token: &str| -> Option<u32> {
        if let Some(number) = token
            .strip_suffix(['p', 'i'])
            .and_then(|number| number.parse::<u32>().ok())
        {
            return (100..=4320).contains(&number).then_some(number);
        }
        if let Some(height) = resolution_height(token) {
            return Some(height);
        }
        match token {
            "8k" | "4320" => Some(4320),
            "4k" | "uhd" | "2160" => Some(2160),
            "2k" | "qhd" | "1440" => Some(1440),
            "fhd" | "fullhd" => Some(1080),
            "hd" if index > 0 && tokens[index - 1] == "full" => Some(1080),
            _ => None,
        }
    };
    let bare = |token: &str| {
        token
            .parse::<u32>()
            .ok()
            .filter(|number| KNOWN_HEIGHTS.contains(number))
    };
    let loose = |token: &str| match token {
        "hd" => Some(720),
        "sd" => Some(480),
        _ => None,
    };
    let first = |find: &dyn Fn(usize, &str) -> Option<u32>| {
        tokens
            .iter()
            .enumerate()
            .find_map(|(index, token)| find(index, token))
    };
    first(&explicit)
        .or_else(|| first(&|_, token| bare(token)))
        .or_else(|| first(&|_, token| loose(token)))
}

/// The first height any of `texts` gives.
pub fn texts_height<'a>(texts: impl IntoIterator<Item = Option<&'a str>>) -> Option<u32> {
    texts.into_iter().flatten().find_map(label_height)
}

/// How well a height suits `preferred`, lower first: with Highest the highest; otherwise the
/// highest at or below the preferred height, then the lowest above it; unknown sizes last.
pub fn rank(height: Option<u32>, preferred: VideoResolution) -> (u8, u32) {
    match (height, preferred.max_height()) {
        (None, _) => (2, 0),
        (Some(height), None) => (0, u32::MAX - height),
        (Some(height), Some(limit)) if height <= limit => (0, limit - height),
        (Some(height), Some(limit)) => (1, height - limit),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heights_come_from_labels_names_and_files() {
        let cases = [
            ("1080p", Some(1080)),
            ("720", Some(720)),
            ("Server 2 · 480p", Some(480)),
            ("Torrentio\n4k", Some(2160)),
            ("4K HDR", Some(2160)),
            ("UHD BluRay", Some(2160)),
            ("FHD", Some(1080)),
            ("Full HD", Some(1080)),
            ("QHD", Some(1440)),
            ("HD", Some(720)),
            ("SD", Some(480)),
            ("Movie.2019.1080i.HDTV.x264", Some(1080)),
            ("Show.S01E02.2160p.WEB-DL.DDP5.1.HDR.H.265", Some(2160)),
            ("1920x800", Some(1080)),
            ("HD 1080p", Some(1080)),
            ("torrentio|720p|BluRay", Some(720)),
            ("HDRip x265 10bit", None),
            ("Auto", None),
            ("Episode 12", None),
        ];
        for (text, height) in cases {
            assert_eq!(label_height(text), height, "{text}");
        }
        assert_eq!(
            texts_height([None, Some("Addon"), Some("Film.720p.mkv")]),
            Some(720)
        );
    }

    #[test]
    fn pictures_count_by_their_long_side() {
        assert_eq!(picture_height(1920, 1080), 1080);
        assert_eq!(picture_height(1920, 800), 1080);
        assert_eq!(picture_height(1080, 1920), 1080);
        assert_eq!(picture_height(854, 480), 480);
        assert_eq!(picture_height(3840, 1600), 2160);
        assert_eq!(resolution_height("1280x720"), Some(720));
        assert_eq!(resolution_height("0x0"), None);
        assert_eq!(bandwidth_height(5_000_000), 1080);
        assert_eq!(bandwidth_height(800_000), 360);
    }

    fn pick(heights: &[Option<u32>], preferred: VideoResolution) -> Option<u32> {
        heights
            .iter()
            .copied()
            .min_by_key(|height| rank(*height, preferred))
            .flatten()
    }

    #[test]
    fn the_highest_at_or_below_the_preference_else_the_lowest_above() {
        let all = [
            Some(360),
            None,
            Some(2160),
            Some(720),
            Some(1080),
            Some(480),
        ];
        assert_eq!(pick(&all, VideoResolution::Highest), Some(2160));
        assert_eq!(pick(&all, VideoResolution::P2160), Some(2160));
        assert_eq!(pick(&all, VideoResolution::P1440), Some(1080));
        assert_eq!(pick(&all, VideoResolution::P1080), Some(1080));
        assert_eq!(pick(&all, VideoResolution::P720), Some(720));
        assert_eq!(
            pick(&[Some(1080), Some(2160)], VideoResolution::P720),
            Some(1080)
        );
        assert_eq!(
            pick(&[None, Some(480)], VideoResolution::Highest),
            Some(480)
        );
        assert_eq!(VideoResolution::default(), VideoResolution::P1080);
    }
}
