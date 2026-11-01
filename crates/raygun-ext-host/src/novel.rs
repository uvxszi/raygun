//! `novel.source` providers (M31): the catalogue calls of `manga.source` (same shapes, same
//! checks) plus `passage`, a chapter's text. Source HTML never reaches the app: it becomes plain
//! blocks here (paragraphs, headings, separators, images).

use scraper::{Html, Node};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ExtensionHost;

pub const NOVEL_SOURCE: &str = "novel.source";

/// Most blocks and text one passage may hold.
const MAX_BLOCKS: usize = 20_000;
const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;

/// Elements dropped with everything inside them.
const SKIPPED: [&str; 17] = [
    "script", "style", "noscript", "iframe", "object", "embed", "svg", "head", "template", "form",
    "button", "select", "textarea", "canvas", "title", "link", "meta",
];
/// Elements that start and end a paragraph.
const BLOCKS: [&str; 26] = [
    "p",
    "div",
    "section",
    "article",
    "blockquote",
    "li",
    "ul",
    "ol",
    "dl",
    "dt",
    "dd",
    "table",
    "tr",
    "td",
    "th",
    "pre",
    "figure",
    "figcaption",
    "center",
    "header",
    "footer",
    "main",
    "aside",
    "address",
    "body",
    "html",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum PassageBlock {
    Paragraph(String),
    Heading(String),
    Separator,
    Image(String),
}

impl ExtensionHost {
    /// A chapter's text as blocks.
    pub async fn novel_passage(
        &self,
        provider: &str,
        chapter_id: &str,
    ) -> Result<Vec<PassageBlock>, String> {
        let value = self
            .call(
                provider,
                "passage",
                serde_json::json!({ "chapterId": chapter_id }),
            )
            .await
            .map_err(|error| error.to_string())?;
        let blocks = parse_passage(&value);
        if blocks.is_empty() {
            return Err("This chapter has no text".into());
        }
        Ok(blocks)
    }
}

/// Blocks from a `passage` answer: `html` (with `baseUrl` for relative images) or `text`.
pub fn parse_passage(value: &Value) -> Vec<PassageBlock> {
    let base = value
        .get("baseUrl")
        .and_then(Value::as_str)
        .and_then(|base| url::Url::parse(base).ok())
        .filter(|base| matches!(base.scheme(), "http" | "https"));
    if let Some(html) = value.get("html").and_then(Value::as_str) {
        html_blocks(html, base.as_ref())
    } else if let Some(text) = value.get("text").and_then(Value::as_str) {
        text_blocks(text)
    } else {
        Vec::new()
    }
}

/// Paragraphs of plain text: split at blank lines, hard-wrapped lines joined.
pub fn text_blocks(text: &str) -> Vec<PassageBlock> {
    let mut builder = Builder::default();
    for line in text.lines() {
        if line.trim().is_empty() {
            builder.flush();
        } else {
            builder.push_text(line);
            // Hard-wrapped lines join with a space.
            builder.push_text(" ");
        }
    }
    builder.finish()
}

/// Blocks from HTML, keeping only text structure and http(s) images.
pub fn html_blocks(html: &str, base: Option<&url::Url>) -> Vec<PassageBlock> {
    let document = Html::parse_fragment(html);
    let mut builder = Builder::default();
    walk(document.tree.root(), base, &mut builder);
    builder.finish()
}

fn walk(node: ego_tree::NodeRef<'_, Node>, base: Option<&url::Url>, builder: &mut Builder) {
    for child in node.children() {
        if builder.full() {
            return;
        }
        match child.value() {
            Node::Text(text) => builder.push_text(text),
            Node::Element(element) => {
                let name = element.name();
                if SKIPPED.contains(&name) {
                    continue;
                }
                match name {
                    "br" => builder.flush(),
                    "hr" => {
                        builder.flush();
                        builder.push(PassageBlock::Separator);
                    }
                    "img" => {
                        let source = element.attr("src").and_then(|src| resolve(src, base));
                        if let Some(source) = source {
                            builder.flush();
                            builder.push(PassageBlock::Image(source));
                        }
                    }
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        builder.flush();
                        let mut heading = Builder::default();
                        walk(child, base, &mut heading);
                        // A heading's images stay; its text becomes one heading.
                        let mut text = Vec::new();
                        for block in heading.finish() {
                            match block {
                                PassageBlock::Paragraph(line) | PassageBlock::Heading(line) => {
                                    text.push(line)
                                }
                                other => builder.push(other),
                            }
                        }
                        if !text.is_empty() {
                            builder.push(PassageBlock::Heading(text.join(" ")));
                        }
                    }
                    _ if BLOCKS.contains(&name) => {
                        builder.flush();
                        walk(child, base, builder);
                        builder.flush();
                    }
                    _ => walk(child, base, builder),
                }
            }
            _ => walk(child, base, builder),
        }
    }
}

fn resolve(src: &str, base: Option<&url::Url>) -> Option<String> {
    let resolved = match base {
        Some(base) => base.join(src.trim()).ok()?,
        None => url::Url::parse(src.trim()).ok()?,
    };
    matches!(resolved.scheme(), "http" | "https").then(|| resolved.to_string())
}

#[derive(Default)]
struct Builder {
    blocks: Vec<PassageBlock>,
    current: String,
    bytes: usize,
}

impl Builder {
    fn full(&self) -> bool {
        self.blocks.len() >= MAX_BLOCKS || self.bytes >= MAX_TEXT_BYTES
    }

    /// Adds text, collapsing whitespace runs to one space.
    fn push_text(&mut self, text: &str) {
        for (index, word) in text.split_whitespace().enumerate() {
            let starts_with_space = index > 0 || text.starts_with(char::is_whitespace);
            if starts_with_space && !self.current.is_empty() && !self.current.ends_with(' ') {
                self.current.push(' ');
            }
            self.current.push_str(word);
        }
        if text.ends_with(char::is_whitespace) && !self.current.is_empty() {
            self.current.push(' ');
        }
    }

    fn flush(&mut self) {
        let text = self.current.trim();
        if !text.is_empty() {
            let text = text.to_owned();
            self.bytes += text.len();
            self.blocks.push(PassageBlock::Paragraph(text));
        }
        self.current.clear();
    }

    fn push(&mut self, block: PassageBlock) {
        if !self.full() {
            self.blocks.push(block);
        }
    }

    fn finish(mut self) -> Vec<PassageBlock> {
        self.flush();
        self.blocks.truncate(MAX_BLOCKS);
        self.blocks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PassageBlock::*;

    fn paragraph(text: &str) -> PassageBlock {
        Paragraph(text.into())
    }

    #[test]
    fn html_becomes_plain_blocks() {
        let base = url::Url::parse("https://www.gutenberg.org/cache/epub/84/pg84-images.html").ok();
        let html = r#"
            <h2><a id="ch1"></a>Chapter  1</h2>
            <p>It was a <i>dreary</i> night of
               November.<script>alert(1)</script></p>
            <style>p { color: red }</style>
            <p>Line one<br>Line two<br/><br/></p>
            <div class="poem"><span>Inline</span> text <b>joined</b></div>
            <hr>
            <img src="images/fig1.jpg" alt="x">
            <img src="javascript:alert(1)">
            <img src="data:image/png;base64,AAAA">
            <p>   </p>
            <ul><li>First</li><li>Second</li></ul>
        "#;
        assert_eq!(
            html_blocks(html, base.as_ref()),
            [
                Heading("Chapter 1".into()),
                paragraph("It was a dreary night of November."),
                paragraph("Line one"),
                paragraph("Line two"),
                paragraph("Inline text joined"),
                Separator,
                Image("https://www.gutenberg.org/cache/epub/84/images/fig1.jpg".into()),
                paragraph("First"),
                paragraph("Second"),
            ]
        );
    }

    #[test]
    fn relative_images_need_a_base() {
        assert_eq!(
            html_blocks(r#"<img src="a.jpg"><p>x</p>"#, None),
            [paragraph("x")]
        );
        assert_eq!(
            html_blocks(r#"<img src="https://img.example/a.jpg">"#, None),
            [Image("https://img.example/a.jpg".into())]
        );
    }

    #[test]
    fn plain_text_splits_at_blank_lines() {
        let text = "First line\nwrapped here.\n\n\n  Second paragraph.  \n";
        assert_eq!(
            text_blocks(text),
            [
                paragraph("First line wrapped here."),
                paragraph("Second paragraph.")
            ]
        );
    }

    #[test]
    fn answers_pick_html_or_text() {
        let html = serde_json::json!({ "html": "<p>Hi</p>", "baseUrl": "javascript:x" });
        assert_eq!(parse_passage(&html), [paragraph("Hi")]);
        let text = serde_json::json!({ "text": "Plain" });
        assert_eq!(parse_passage(&text), [paragraph("Plain")]);
        assert!(parse_passage(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn huge_passages_are_cut() {
        let html = "<p>x</p>".repeat(MAX_BLOCKS + 10);
        assert_eq!(html_blocks(&html, None).len(), MAX_BLOCKS);
    }
}
