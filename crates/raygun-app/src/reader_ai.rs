//! The readers' AI layers (v1.3.0). Upscaling and restoring comic pages runs in the webview, on
//! the graphics card (Anime4K's shaders, read here from the installed model); this module reads
//! the words on comic pages (`import::ocr`, grouped into blocks) and translates texts for the
//! translation layers, on this device's models or with a translator from Settings → AI. What it
//! read and translated is kept in memory a while, so turning back a page or moving a layer about
//! doesn't redo it.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use raygun_ai::translate::Content;
use raygun_model::{PageTextBlock, ReaderAiSupport, ReaderAiTranslateRequest};
use raygun_net::ImageLinks;
use tauri::{AppHandle, Manager};

use crate::ai::AiService;
use crate::import::ocr;
use crate::lyrics_translation::LyricsTranslations;

/// Pages whose words are kept.
const PAGES_KEPT: usize = 200;
/// Translations kept (a novel's paragraphs, a page's bubbles).
const TRANSLATIONS_KEPT: usize = 20_000;
/// A model id from Settings → AI's translators starts with this.
const AI_PREFIX: &str = "ai:";
/// Pages larger than this aren't read.
const MAX_PAGE_BYTES: usize = 40 * 1024 * 1024;

/// A map that forgets its oldest entries past `cap`.
struct Kept<V> {
    cap: usize,
    order: VecDeque<String>,
    values: HashMap<String, V>,
}

impl<V: Clone> Kept<V> {
    fn new(cap: usize) -> Self {
        Self {
            cap,
            order: VecDeque::new(),
            values: HashMap::new(),
        }
    }

    fn get(&self, key: &str) -> Option<V> {
        self.values.get(key).cloned()
    }

    fn put(&mut self, key: String, value: V) {
        if self.values.insert(key.clone(), value).is_none() {
            self.order.push_back(key);
            while self.order.len() > self.cap {
                if let Some(old) = self.order.pop_front() {
                    self.values.remove(&old);
                }
            }
        }
    }
}

pub struct ReaderAi {
    app: AppHandle,
    client: reqwest::Client,
    pages: Mutex<Kept<Vec<PageTextBlock>>>,
    translations: Mutex<Kept<String>>,
    /// One page read at a time: the text reader uses every core.
    reading: tokio::sync::Mutex<()>,
}

impl ReaderAi {
    pub fn new(app: AppHandle, client: reqwest::Client) -> Arc<Self> {
        Arc::new(Self {
            app,
            client,
            pages: Mutex::new(Kept::new(PAGES_KEPT)),
            translations: Mutex::new(Kept::new(TRANSLATIONS_KEPT)),
            reading: tokio::sync::Mutex::new(()),
        })
    }

    pub fn support() -> ReaderAiSupport {
        ReaderAiSupport {
            text_reader_built_in: ocr::built_in(),
            text_reader_model: ocr::OCRS_MODEL.to_owned(),
        }
    }

    /// The words on a reader's page (one of the image proxy's links), in blocks.
    pub async fn page_text(&self, url: &str) -> Result<Vec<PageTextBlock>, String> {
        if !self.app.state::<ImageLinks>().is_page(url) {
            return Err("That isn't one of the reader's pages".into());
        }
        if let Some(blocks) = self
            .pages
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(url)
        {
            return Ok(blocks);
        }
        let models = if ocr::built_in() {
            None
        } else {
            Some(
                self.app
                    .state::<AiService>()
                    .installed_dir(ocr::OCRS_MODEL)
                    .ok_or("Download the text reader first")?,
            )
        };
        let _turn = self.reading.lock().await;
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            return Err(format!(
                "The page couldn't be loaded (HTTP {})",
                response.status().as_u16()
            ));
        }
        let bytes = response.bytes().await.map_err(|error| error.to_string())?;
        if bytes.len() > MAX_PAGE_BYTES {
            return Err("The page is too large to read".into());
        }
        let lines = tauri::async_runtime::spawn_blocking(move || {
            ocr::read_boxes(&bytes, models.as_deref())
        })
        .await
        .map_err(|error| error.to_string())??;
        let blocks = blocks_of(&lines);
        tracing::info!(
            lines = lines.len(),
            blocks = blocks.len(),
            "read a page's words"
        );
        self.pages
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .put(url.to_owned(), blocks.clone());
        Ok(blocks)
    }

    /// Translates texts for a translation layer, in order; texts without words come back as they
    /// were. Ones translated before with the same layer come from memory.
    pub async fn translate(
        &self,
        request: ReaderAiTranslateRequest,
    ) -> Result<Vec<String>, String> {
        let ReaderAiTranslateRequest {
            texts,
            from,
            to,
            model,
        } = request;
        if to.trim().is_empty() {
            return Err("Pick a language to translate into".into());
        }
        let key = |text: &str| format!("{model}\u{1f}{from}\u{1f}{to}\u{1f}{text}");
        let mut out: Vec<Option<String>> = Vec::with_capacity(texts.len());
        let mut wanted: Vec<String> = Vec::new();
        {
            let kept = self.translations.lock().unwrap_or_else(|p| p.into_inner());
            for text in &texts {
                if !has_words(text) {
                    out.push(Some(text.clone()));
                } else if let Some(done) = kept.get(&key(text)) {
                    out.push(Some(done));
                } else {
                    out.push(None);
                    if !wanted.contains(text) {
                        wanted.push(text.clone());
                    }
                }
            }
        }
        if !wanted.is_empty() {
            let translated = self.translate_now(&wanted, &from, &to, &model).await?;
            let mut kept = self.translations.lock().unwrap_or_else(|p| p.into_inner());
            for (text, done) in wanted.iter().zip(translated) {
                kept.put(key(text), done);
            }
            for (slot, text) in out.iter_mut().zip(&texts) {
                if slot.is_none() {
                    *slot = kept.get(&key(text));
                }
            }
        }
        Ok(out
            .into_iter()
            .zip(texts)
            .map(|(done, text)| done.unwrap_or(text))
            .collect())
    }

    async fn translate_now(
        &self,
        texts: &[String],
        from: &str,
        to: &str,
        model: &str,
    ) -> Result<Vec<String>, String> {
        // Text already in the language wanted stays as it is.
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        if same_language(from, to) || (from.is_empty() && is_english(to) && looks_english(&refs)) {
            return Ok(texts.to_vec());
        }
        if let Some(provider) = model.strip_prefix(AI_PREFIX) {
            let from = Some(from).filter(|from| !from.is_empty());
            let (_, translated) = self
                .app
                .state::<AiService>()
                .translate_texts_from(provider, texts, from, to, Content::Text)
                .await?;
            return Ok(translated);
        }
        // This device's models translate a sentence at a time; a paragraph goes in as its
        // sentences and comes back joined.
        let mut sentences: Vec<String> = Vec::new();
        let mut parts: Vec<usize> = Vec::with_capacity(texts.len());
        for text in texts {
            let split = sentences_of(text);
            parts.push(split.len());
            sentences.extend(split);
        }
        let preferred = Some(model.to_owned()).filter(|model| !model.is_empty());
        let translated = self
            .app
            .state::<Arc<LyricsTranslations>>()
            .translate_on_device_with(to, &sentences, preferred, is_english(from), |_, _| {})
            .await?;
        let mut rest = translated.into_iter();
        Ok(parts
            .into_iter()
            .map(|count| {
                rest.by_ref()
                    .take(count)
                    .map(|sentence| sentence.trim().to_owned())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect())
    }
}

/// Whether a text has anything to translate.
fn has_words(text: &str) -> bool {
    text.chars().filter(|c| c.is_alphabetic()).count() >= 2
}

/// A paragraph's sentences: split after `.`, `!`, `?` (and their full-width forms) followed by
/// a space, or after the full-width ones alone; whitespace inside is kept as one space.
fn sentences_of(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            if !current.is_empty() && !current.ends_with(' ') {
                current.push(' ');
            }
            continue;
        }
        current.push(c);
        let ends = matches!(c, '.' | '!' | '?' | '…')
            && chars.peek().is_some_and(|next| next.is_whitespace());
        let full_width = matches!(c, '。' | '！' | '？');
        if (ends || full_width) && current.chars().filter(|c| c.is_alphanumeric()).count() > 0 {
            out.push(current.trim().to_owned());
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_owned());
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// Lines read on a page grouped into blocks (a speech bubble, a caption): a line joins the block
/// above it when it starts close under it and the two overlap sideways. A word broken over two
/// lines with a hyphen is joined again.
fn blocks_of(lines: &[ocr::TextBox]) -> Vec<PageTextBlock> {
    let mut lines: Vec<&ocr::TextBox> = lines
        .iter()
        .filter(|line| !line.text.trim().is_empty())
        .collect();
    lines.sort_by(|a, b| a.top.total_cmp(&b.top).then(a.left.total_cmp(&b.left)));
    struct Block {
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        last_height: f32,
        text: String,
    }
    let mut blocks: Vec<Block> = Vec::new();
    for line in lines {
        let height = (line.bottom - line.top).max(0.001);
        let text = line.text.trim();
        let fitting = blocks.iter_mut().rev().find(|block| {
            let gap = line.top - block.bottom;
            let overlap = line.right.min(block.right) - line.left.max(block.left);
            gap < block.last_height.max(height) * 0.9 && gap > -height * 0.5 && overlap > 0.0
        });
        match fitting {
            Some(block) => {
                if let Some(stem) = block.text.strip_suffix('-')
                    && text.chars().next().is_some_and(char::is_lowercase)
                {
                    block.text = format!("{stem}{text}");
                } else {
                    block.text.push(' ');
                    block.text.push_str(text);
                }
                block.left = block.left.min(line.left);
                block.right = block.right.max(line.right);
                block.bottom = block.bottom.max(line.bottom);
                block.last_height = height;
            }
            None => blocks.push(Block {
                left: line.left,
                top: line.top,
                right: line.right,
                bottom: line.bottom,
                last_height: height,
                text: text.to_owned(),
            }),
        }
    }
    blocks
        .into_iter()
        .filter(|block| has_words(&block.text))
        .map(|block| PageTextBlock {
            left: f64::from(block.left),
            top: f64::from(block.top),
            right: f64::from(block.right),
            bottom: f64::from(block.bottom),
            text: block.text,
        })
        .collect()
}

/// The text of an installed model the readers run themselves: an Anime4K shader (one file),
/// which runs in the webview.
pub fn model_text(app: &AppHandle, id: &str) -> Result<String, String> {
    let model = raygun_ai::models::find(id).map_err(|error| error.to_string())?;
    let [file] = model.files.as_slice() else {
        return Err("That model doesn't run in the reader".into());
    };
    if model.engine.as_deref() != Some("anime4k") {
        return Err("That model doesn't run in the reader".into());
    }
    let dir = app
        .state::<AiService>()
        .installed_dir(id)
        .ok_or("Download the model first")?;
    std::fs::read_to_string(dir.join(&file.name)).map_err(|error| error.to_string())
}

/// The primary subtag of a language tag (`pt-BR` → `pt`), lower-case.
fn primary(tag: &str) -> String {
    tag.split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn is_english(tag: &str) -> bool {
    primary(tag) == "en"
}

/// Whether a layer translates from a language into the same one.
fn same_language(from: &str, to: &str) -> bool {
    !from.is_empty() && primary(from) == primary(to)
}

/// Whether texts read as English, as the on-device translators judge it.
fn looks_english(texts: &[&str]) -> bool {
    raygun_ai::marian::looks_english(texts)
}

#[cfg(test)]
mod tests {
    #[test]
    fn text_in_the_wanted_language_is_left_alone() {
        assert!(super::same_language("pt-BR", "pt"));
        assert!(!super::same_language("", "en"));
        assert!(!super::same_language("ja", "en"));
        assert!(super::is_english("en-GB"));
        assert!(super::looks_english(&[
            "It was a dreary night of November."
        ]));
    }

    use super::*;

    fn line(left: f32, top: f32, right: f32, bottom: f32, text: &str) -> ocr::TextBox {
        ocr::TextBox {
            left,
            top,
            right,
            bottom,
            text: text.into(),
        }
    }

    #[test]
    fn lines_close_together_make_one_block() {
        let lines = [
            line(0.10, 0.10, 0.30, 0.13, "WHERE ARE"),
            line(0.11, 0.135, 0.29, 0.165, "YOU GO-"),
            line(0.12, 0.17, 0.28, 0.20, "ing?"),
            // Far to the right: another bubble.
            line(0.60, 0.11, 0.80, 0.14, "HOME."),
            // Far below the first: another one again.
            line(0.10, 0.50, 0.30, 0.53, "Wait!"),
            line(0.40, 0.90, 0.45, 0.92, "7"),
        ];
        let blocks = blocks_of(&lines);
        let texts: Vec<&str> = blocks.iter().map(|block| block.text.as_str()).collect();
        assert_eq!(texts, ["WHERE ARE YOU GOing?", "HOME.", "Wait!"]);
        let first = &blocks[0];
        assert!((first.left - 0.10).abs() < 1e-6 && (first.bottom - 0.20).abs() < 1e-6);
    }

    #[test]
    fn paragraphs_split_into_sentences() {
        assert_eq!(
            sentences_of("It rained.  Then it stopped!\nDid it? Yes"),
            ["It rained.", "Then it stopped!", "Did it?", "Yes"]
        );
        assert_eq!(sentences_of("雨だ。晴れた。"), ["雨だ。", "晴れた。"]);
        assert_eq!(sentences_of(""), [""]);
    }

    #[test]
    fn kept_forgets_the_oldest() {
        let mut kept = Kept::new(2);
        kept.put("a".into(), 1);
        kept.put("b".into(), 2);
        kept.put("c".into(), 3);
        assert_eq!(kept.get("a"), None);
        assert_eq!(kept.get("c"), Some(3));
    }
}
