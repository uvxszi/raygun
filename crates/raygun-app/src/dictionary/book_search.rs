//! Where a word appears in a book, for the lookup's "Search in book": the chapters on this
//! device are read (every chapter of an imported EPUB; a source's downloaded chapters and the
//! one open) and their paragraphs searched, case and accents aside.

use raygun_model::{BookSearch, BookSearchHit, DownloadState, TextBlock};
use tauri::{AppHandle, Manager};

/// Places listed at most.
const MAX_HITS: usize = 200;
/// Characters shown on each side of a match.
const CONTEXT: usize = 48;

/// A character as compared: lower case, without its accent.
fn folded(c: char) -> char {
    let lower = c.to_lowercase().next().unwrap_or(c);
    match lower {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => 'a',
        'ç' | 'ć' | 'č' => 'c',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => 'e',
        'ì' | 'í' | 'î' | 'ï' | 'ī' => 'i',
        'ñ' | 'ń' | 'ň' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ō' | 'ø' => 'o',
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' => 'u',
        'ý' | 'ÿ' => 'y',
        'ś' | 'š' => 's',
        'ź' | 'ż' | 'ž' => 'z',
        '’' | 'ʼ' => '\'',
        other => other,
    }
}

/// Where `query` appears in `text`: each match's start and end, as character places.
pub fn matches_in(text: &str, query: &str) -> Vec<(usize, usize)> {
    let haystack: Vec<char> = text.chars().map(folded).collect();
    let needle: Vec<char> = query.chars().map(folded).collect();
    if needle.is_empty() || needle.len() > haystack.len() {
        return Vec::new();
    }
    let word_char = |c: char| c.is_alphanumeric();
    let mut found = Vec::new();
    let mut at = 0;
    while at + needle.len() <= haystack.len() {
        if haystack[at..at + needle.len()] == needle[..] {
            // Whole words only: "art" isn't found in "start".
            let before = at.checked_sub(1).map(|place| haystack[place]);
            let after = haystack.get(at + needle.len()).copied();
            let starts = !needle[0].is_alphanumeric() || !before.is_some_and(word_char);
            let ends = !needle[needle.len() - 1].is_alphanumeric() || !after.is_some_and(word_char);
            if starts && ends {
                found.push((at, at + needle.len()));
                at += needle.len();
                continue;
            }
        }
        at += 1;
    }
    found
}

/// A match with the text around it, cut at word edges.
fn hit(
    chapter_id: &str,
    name: &str,
    block: usize,
    text: &str,
    start: usize,
    end: usize,
) -> BookSearchHit {
    let chars: Vec<char> = text.chars().collect();
    let from = start.saturating_sub(CONTEXT);
    let to = (end + CONTEXT).min(chars.len());
    let mut before: String = chars[from..start].iter().collect();
    let mut after: String = chars[end..to].iter().collect();
    if from > 0 {
        before = format!(
            "…{}",
            before
                .split_once(' ')
                .map_or(before.as_str(), |(_, rest)| rest)
        );
    }
    if to < chars.len() {
        after = format!(
            "{}…",
            after
                .rsplit_once(' ')
                .map_or(after.as_str(), |(rest, _)| rest)
        );
    }
    BookSearchHit {
        chapter_id: chapter_id.to_owned(),
        chapter_name: name.to_owned(),
        block: block as u32,
        before,
        matched: chars[start..end].iter().collect(),
        after,
    }
}

/// The hits in one chapter's blocks.
pub fn search_blocks(
    chapter_id: &str,
    name: &str,
    blocks: &[TextBlock],
    query: &str,
    hits: &mut Vec<BookSearchHit>,
) -> bool {
    for (index, block) in blocks.iter().enumerate() {
        let text = match block {
            TextBlock::Paragraph { text, .. }
            | TextBlock::Heading { text }
            | TextBlock::Code { text, .. }
            | TextBlock::Table { text, .. } => text,
            _ => continue,
        };
        for (start, end) in matches_in(text, query) {
            if hits.len() >= MAX_HITS {
                return true;
            }
            hits.push(hit(chapter_id, name, index, text, start, end));
        }
    }
    false
}

pub async fn search(
    app: &AppHandle,
    title_id: &str,
    open_chapter: Option<&str>,
    query: &str,
) -> Result<BookSearch, String> {
    let query = query.split_whitespace().collect::<Vec<_>>().join(" ");
    if query.is_empty() {
        return Err("Type something to search for".to_owned());
    }
    let db = app.state::<crate::startup::AppDb>().inner().0.clone();
    let title = raygun_db::titles::title(&db, title_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or("This book was removed")?;
    let mut units = raygun_db::titles::units(&db, title_id)
        .await
        .map_err(|error| error.to_string())?;
    // In reading order: by number where chapters have one, else as the source lists them.
    units.sort_by(|a, b| {
        let number = |unit: &raygun_db::titles::UnitRow| unit.fields.number.unwrap_or(f64::MAX);
        number(a)
            .total_cmp(&number(b))
            .then(a.source_order.cmp(&b.source_order))
    });
    let imported = title.fields.source_id == crate::novel::LOCAL_EPUB;
    let downloaded = if imported {
        Default::default()
    } else {
        app.state::<std::sync::Arc<crate::manga_downloads::MangaDownloadService>>()
            .for_title(title_id)
            .await
    };
    let novels = app.state::<crate::novel::NovelService>();
    let mut hits = Vec::new();
    let mut searched = 0u32;
    let mut more = false;
    for unit in &units {
        let on_device = imported
            || open_chapter == Some(unit.id.as_str())
            || downloaded
                .get(&unit.id)
                .is_some_and(|download| download.state == DownloadState::Done);
        if !on_device || more {
            continue;
        }
        let Ok(chapter) = novels.chapter(title_id, &unit.id).await else {
            continue;
        };
        searched += 1;
        more = search_blocks(
            &unit.id,
            &unit.fields.name,
            &chapter.blocks,
            &query,
            &mut hits,
        );
    }
    Ok(BookSearch {
        hits,
        searched,
        chapters: units.len() as u32,
        more,
    })
}
