//! `novel.source` providers (M31): the catalogue calls of `manga.source` (same shapes, same
//! checks) plus `passage`, a chapter's text. Source HTML never reaches the app: it becomes plain
//! blocks here (paragraphs, headings, separators, images). Formulas (MathML or TeX, [`crate::math`])
//! and raised or lowered text make a paragraph's parts; a formula on its own line is a block. A
//! passage may instead be a PDF document (`{ pdf }`), which the readers show as pages.

use raygun_net::pdf::PdfRef;
use scraper::{Html, Node};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ExtensionHost;
use crate::math::{Formula, TextPiece, is_math, split_tex};

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
    /// A paragraph holding formulas or raised or lowered text, in parts.
    Rich(Vec<Part>),
    /// A formula on a line of its own.
    Math(Formula),
    /// The chapter is a PDF document, or some of its pages: the only block of its passage.
    Pdf(PdfRef),
}

/// One part of a [`PassageBlock::Rich`] paragraph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum Part {
    Text(String),
    /// Raised text (`<sup>`): an exponent, a footnote mark.
    Sup(String),
    /// Lowered text (`<sub>`): an index, a chemical formula's count.
    Sub(String),
    Math(Formula),
}

impl Part {
    /// The part as plain text: raised and lowered digits as such (`x²`, `H₂O`), a formula as
    /// its text.
    pub fn plain(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Sup(text) => crate::math::raised_text(text),
            Self::Sub(text) => crate::math::lowered_text(text),
            Self::Math(formula) => formula.text.clone(),
        }
    }
}

/// A rich paragraph's plain text.
pub fn plain_text(parts: &[Part]) -> String {
    parts.iter().map(Part::plain).collect()
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
            return Err(format!(
                "{} has no text for this chapter",
                self.provider_name(provider)
            ));
        }
        Ok(blocks)
    }
}

/// Blocks from a `passage` answer: `pdf` (the chapter is a PDF document), `html` (with `baseUrl`
/// for relative images) or `text`.
pub fn parse_passage(value: &Value) -> Vec<PassageBlock> {
    if let Some(pdf) = crate::manga::parse_pdf(value) {
        return vec![PassageBlock::Pdf(pdf)];
    }
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

/// Blocks from HTML, keeping only text structure, formulas and http(s) images.
pub fn html_blocks(html: &str, base: Option<&url::Url>) -> Vec<PassageBlock> {
    let document = Html::parse_fragment(html);
    let mut builder = Builder::default();
    walk(document.tree.root(), base, &mut builder);
    builder.finish()
}

/// Classes of the boxes math renderers draw a formula in: the formula is the MathML inside (or
/// the TeX script after it), the rest is its drawing.
const MATH_BOXES: [&str; 8] = [
    "katex",
    "katex-display",
    "mwe-math-element",
    "MathJax",
    "MathJax_Display",
    "MathJax_CHTML",
    "MathJax_SVG",
    "MathJax_SVG_Display",
];
/// Inline elements whose text is kept as written: TeX there is code, not a formula.
const CODE: [&str; 4] = ["code", "kbd", "samp", "var"];

fn has_class(element: &scraper::node::Element, wanted: &str) -> bool {
    element.classes().any(|class| class == wanted)
}

/// A `<script type="math/tex">` (MathJax 2's source of a formula) or `math/mml`.
fn script_formula(node: ego_tree::NodeRef<'_, Node>) -> Option<Formula> {
    let Node::Element(element) = node.value() else {
        return None;
    };
    let kind = element.attr("type")?.to_ascii_lowercase();
    let source: String = node
        .children()
        .filter_map(|child| child.value().as_text().map(|text| String::from(&**text)))
        .collect();
    if kind.starts_with("math/tex") {
        Formula::from_tex(&source, kind.contains("mode=display"))
    } else if kind.starts_with("math/mml") {
        Formula::from_markup(&source)
    } else {
        None
    }
}

/// Whether the next element after `node` is a math script, which then says the formula.
fn script_follows(node: ego_tree::NodeRef<'_, Node>) -> bool {
    node.next_siblings()
        .find(|sibling| match sibling.value() {
            Node::Text(text) => !text.trim().is_empty(),
            Node::Element(_) => true,
            _ => false,
        })
        .is_some_and(|sibling| {
            matches!(sibling.value(), Node::Element(element)
                if element.name() == "script"
                    && element.attr("type").is_some_and(|kind| kind.to_ascii_lowercase().starts_with("math/")))
        })
}

/// A formula box's display: a block box, or a display formula inside.
fn boxed_display(element: &scraper::node::Element, formula: &Formula) -> bool {
    formula.display
        || has_class(element, "katex-display")
        || has_class(element, "MathJax_Display")
        || has_class(element, "MathJax_SVG_Display")
        || element.attr("display") == Some("true")
}

fn walk(node: ego_tree::NodeRef<'_, Node>, base: Option<&url::Url>, builder: &mut Builder) {
    for child in node.children() {
        if builder.full() {
            return;
        }
        match child.value() {
            Node::Text(text) => builder.push_text_with_tex(text),
            Node::Element(element) => {
                let name = element.name();
                if is_math(child) {
                    match Formula::from_element(child) {
                        Some(formula) => builder.push_formula(formula),
                        None => walk(child, base, builder),
                    }
                    continue;
                }
                if name == "script" {
                    if let Some(formula) = script_formula(child) {
                        builder.push_formula(formula);
                    }
                    continue;
                }
                if SKIPPED.contains(&name) {
                    continue;
                }
                // A renderer's box: the formula once, not its drawing as letters.
                if name == "mjx-container"
                    || MATH_BOXES.iter().any(|class| has_class(element, class))
                {
                    if name != "mjx-container" && script_follows(child) {
                        continue;
                    }
                    let inner = child.descendants().find(|inner| is_math(*inner));
                    if let Some(mut formula) = inner.and_then(Formula::from_element) {
                        formula.display = boxed_display(element, &formula);
                        builder.push_formula(formula);
                        continue;
                    }
                }
                // `<span class="math">x^2</span>`: TeX without delimiters.
                if (has_class(element, "math") || has_class(element, "tex"))
                    && let Some(formula) = bare_tex(child, element)
                {
                    builder.push_formula(formula);
                    continue;
                }
                match name {
                    "br" => builder.flush(),
                    "hr" => {
                        builder.flush();
                        builder.push(PassageBlock::Separator);
                    }
                    "img" => {
                        if let Some(formula) = image_formula(element) {
                            builder.push_formula(formula);
                            continue;
                        }
                        let source = element.attr("src").and_then(|src| resolve(src, base));
                        if let Some(source) = source {
                            builder.flush();
                            builder.push(PassageBlock::Image(source));
                        }
                    }
                    "sup" | "sub" => {
                        let text: String = child
                            .descendants()
                            .filter_map(|inner| {
                                inner.value().as_text().map(|text| String::from(&**text))
                            })
                            .collect();
                        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                        if !text.is_empty() {
                            builder.push_part(if name == "sup" {
                                Part::Sup(text)
                            } else {
                                Part::Sub(text)
                            });
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
                                PassageBlock::Rich(parts) => text.push(plain_text(&parts)),
                                PassageBlock::Math(formula) => text.push(formula.text),
                                other => builder.push(other),
                            }
                        }
                        if !text.is_empty() {
                            builder.push(PassageBlock::Heading(text.join(" ")));
                        }
                    }
                    "pre" => {
                        builder.flush();
                        builder.code += 1;
                        walk(child, base, builder);
                        builder.code -= 1;
                        builder.flush();
                    }
                    _ if BLOCKS.contains(&name) => {
                        builder.flush();
                        walk(child, base, builder);
                        builder.flush();
                    }
                    _ if CODE.contains(&name) => {
                        builder.code += 1;
                        walk(child, base, builder);
                        builder.code -= 1;
                    }
                    _ => walk(child, base, builder),
                }
            }
            _ => walk(child, base, builder),
        }
    }
}

/// A formula drawn as a picture whose `alt` is its TeX (MediaWiki's fallback pictures, LaTeX
/// plugins' `class="latex"`): the formula itself, drawn as MathML. Other pictures stay pictures.
fn image_formula(element: &scraper::node::Element) -> Option<Formula> {
    let tex_picture = element.classes().any(|class| {
        class.starts_with("mwe-math-fallback-image") || matches!(class, "latex" | "tex")
    });
    if !tex_picture {
        return None;
    }
    let display = element
        .classes()
        .any(|class| class.ends_with("-display") || class == "displaymath");
    Formula::from_tex(element.attr("alt")?, display)
}

/// The formula of a `math`-classed element holding TeX without delimiters.
fn bare_tex(
    node: ego_tree::NodeRef<'_, Node>,
    element: &scraper::node::Element,
) -> Option<Formula> {
    // Only text inside: an element holding markup is walked as usual.
    if node
        .children()
        .any(|child| matches!(child.value(), Node::Element(_)))
    {
        return None;
    }
    let text: String = node
        .children()
        .filter_map(|child| child.value().as_text().map(|text| String::from(&**text)))
        .collect();
    let trimmed = text.trim();
    // Delimited TeX is found in the text itself; words with a `math` class stay words.
    let delimited = split_tex(trimmed)
        .iter()
        .any(|piece| matches!(piece, TextPiece::Tex { .. }));
    let mathy =
        trimmed.chars().count() == 1 || trimmed.contains(['\\', '^', '_', '{', '=', '+', '<', '>']);
    if trimmed.is_empty() || delimited || !mathy {
        return None;
    }
    let display = has_class(element, "display") || element.name() == "div";
    Formula::from_tex(trimmed, display)
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
    /// Text since the paragraph's last part.
    current: String,
    /// The paragraph's parts so far, when it has more than text.
    parts: Vec<Part>,
    bytes: usize,
    /// Inside code: TeX stays as written.
    code: u32,
}

impl Builder {
    fn full(&self) -> bool {
        self.blocks.len() >= MAX_BLOCKS || self.bytes >= MAX_TEXT_BYTES
    }

    /// Whether a space may come next: after something, and not after a space.
    fn wants_space(&self) -> bool {
        match self.current.chars().next_back() {
            Some(last) => last != ' ',
            None => !self.parts.is_empty(),
        }
    }

    /// Adds text, collapsing whitespace runs to one space.
    fn push_text(&mut self, text: &str) {
        for (index, word) in text.split_whitespace().enumerate() {
            let starts_with_space = index > 0 || text.starts_with(char::is_whitespace);
            if starts_with_space && self.wants_space() {
                self.current.push(' ');
            }
            self.current.push_str(word);
        }
        if text.ends_with(char::is_whitespace) && self.wants_space() {
            self.current.push(' ');
        }
    }

    /// Adds text, with formulas where it has TeX between delimiters (not in code).
    fn push_text_with_tex(&mut self, text: &str) {
        if self.code > 0 || !text.contains(['$', '\\']) {
            self.push_text(text);
            return;
        }
        for piece in split_tex(text) {
            match piece {
                TextPiece::Text(text) => self.push_text(text),
                TextPiece::Tex {
                    tex,
                    display,
                    source,
                } => match Formula::from_tex(tex, display) {
                    Some(formula) => self.push_formula(formula),
                    // Not TeX after all: the text as it was.
                    None => self.push_text(source),
                },
            }
        }
    }

    /// A formula: in the line, or on a line of its own.
    fn push_formula(&mut self, formula: Formula) {
        if formula.display {
            self.flush();
            self.bytes += formula.mathml.len();
            self.push(PassageBlock::Math(formula));
        } else {
            self.push_part(Part::Math(formula));
        }
    }

    /// A part of the paragraph that isn't plain text.
    fn push_part(&mut self, part: Part) {
        if !self.current.is_empty() {
            self.parts
                .push(Part::Text(std::mem::take(&mut self.current)));
        }
        self.parts.push(part);
    }

    fn flush(&mut self) {
        if self.parts.is_empty() {
            let text = self.current.trim();
            if !text.is_empty() {
                let text = text.to_owned();
                self.bytes += text.len();
                self.blocks.push(PassageBlock::Paragraph(text));
            }
            self.current.clear();
            return;
        }
        if !self.current.is_empty() {
            self.parts
                .push(Part::Text(std::mem::take(&mut self.current)));
        }
        let mut parts = std::mem::take(&mut self.parts);
        if let Some(Part::Text(first)) = parts.first_mut() {
            *first = first.trim_start().to_owned();
        }
        if let Some(Part::Text(last)) = parts.last_mut() {
            *last = last.trim_end().to_owned();
        }
        parts.retain(|part| !matches!(part, Part::Text(text) if text.is_empty()));
        if parts.is_empty() {
            return;
        }
        self.bytes += parts
            .iter()
            .map(|part| match part {
                Part::Math(formula) => formula.mathml.len() + formula.text.len(),
                Part::Text(text) | Part::Sup(text) | Part::Sub(text) => text.len(),
            })
            .sum::<usize>();
        self.blocks.push(PassageBlock::Rich(parts));
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
    fn formulas_and_scripts_become_parts_and_blocks() {
        let html = r##"
            <p>Let $x^2$ be <i>the</i> square of H<sub>2</sub>O<sup><a href="#n1">1</a></sup>.</p>
            <p>$$\int_0^1 f$$</p>
            <p><span class="mwe-math-element"><span class="mwe-math-mathml-inline" style="display:none"><math><mi>y</mi></math></span><img src="https://wiki.example/y.svg" class="mwe-math-fallback-image-inline"></span> stays</p>
            <p>Price $5 and $6.</p>
            <pre>$x$ in code</pre>
        "##;
        let blocks = html_blocks(html, None);
        assert_eq!(blocks.len(), 5, "{blocks:#?}");
        let Rich(parts) = &blocks[0] else {
            panic!("a paragraph in parts: {:?}", blocks[0]);
        };
        assert!(matches!(&parts[0], Part::Text(text) if text == "Let "));
        assert!(
            matches!(&parts[1], Part::Math(formula) if formula.text == "x^2" && !formula.display)
        );
        assert_eq!(
            parts[2..],
            [
                Part::Text(" be the square of H".into()),
                Part::Sub("2".into()),
                Part::Text("O".into()),
                Part::Sup("1".into()),
                Part::Text(".".into()),
            ]
        );
        assert_eq!(plain_text(parts), "Let x^2 be the square of H₂O¹.");
        assert!(
            matches!(&blocks[1], Math(formula) if formula.display && formula.mathml.contains("<msubsup>"))
        );
        // MediaWiki's MathML, not its fallback picture.
        let Rich(parts) = &blocks[2] else {
            panic!("a formula in a paragraph: {:?}", blocks[2]);
        };
        assert!(matches!(&parts[0], Part::Math(formula) if formula.text == "y"));
        assert_eq!(parts[1], Part::Text(" stays".into()));
        assert_eq!(blocks[3], paragraph("Price $5 and $6."));
        assert_eq!(blocks[4], paragraph("$x$ in code"));
    }

    #[test]
    fn renderers_boxes_give_their_formula_once() {
        let katex = r#"<p><span class="katex"><span class="katex-mathml"><math><semantics><mrow><mi>a</mi></mrow><annotation encoding="application/x-tex">a</annotation></semantics></math></span><span class="katex-html" aria-hidden="true"><span class="base">a</span></span></span></p>"#;
        let mathjax = r#"<p><span class="MathJax" id="MathJax-Element-1-Frame"><nobr>x2</nobr></span><script type="math/tex" id="MathJax-Element-1">x^2</script></p>"#;
        let bare =
            r#"<p><span class="math">\alpha + 1</span> and <span class="math">Menu</span></p>"#;
        for (html, text) in [(katex, "a"), (mathjax, "x^2"), (bare, r"\alpha + 1")] {
            let blocks = html_blocks(html, None);
            let [Rich(parts)] = blocks.as_slice() else {
                panic!("one paragraph in parts: {blocks:?}");
            };
            let formulas: Vec<&str> = parts
                .iter()
                .filter_map(|part| match part {
                    Part::Math(formula) => Some(formula.text.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(formulas, [text], "{html}");
        }
        assert_eq!(
            plain_text(match &html_blocks(bare, None)[0] {
                Rich(parts) => parts,
                other => panic!("{other:?}"),
            }),
            r"\alpha + 1 and Menu"
        );
    }

    #[test]
    fn formula_pictures_with_tex_become_formulas() {
        let html = r#"<p>Area <img class="mwe-math-fallback-image-inline" alt="{\displaystyle \pi r^{2}}" src="https://wiki.example/a.svg"> and <img src="https://wiki.example/b.png" alt="a photo"></p>"#;
        let blocks = html_blocks(html, None);
        let Rich(parts) = &blocks[0] else {
            panic!("a formula in a paragraph: {blocks:?}");
        };
        assert!(matches!(&parts[1], Part::Math(formula) if formula.text == r"\pi r^{2}"));
        // A picture that isn't a formula stays a picture.
        assert_eq!(blocks[1], Image("https://wiki.example/b.png".into()));
    }

    #[test]
    fn passages_may_be_pdf_documents() {
        let answer = serde_json::json!({
            "pdf": { "url": "https://books.example/a.pdf", "headers": { "Referer": "https://books.example/" }, "firstPage": 3, "lastPage": 5 },
            "html": "<p>ignored</p>"
        });
        let blocks = parse_passage(&answer);
        let [Pdf(pdf)] = blocks.as_slice() else {
            panic!("a PDF passage: {blocks:?}");
        };
        assert_eq!(pdf.url, "https://books.example/a.pdf");
        assert_eq!(pdf.headers.len(), 1);
        assert_eq!((pdf.first_page, pdf.last_page), (Some(3), Some(5)));
        let plain = serde_json::json!({ "pdf": "https://books.example/b.pdf" });
        assert!(matches!(&parse_passage(&plain)[..], [Pdf(pdf)] if pdf.first_page.is_none()));
        // Not a web address: no PDF, and nothing else to read.
        assert!(parse_passage(&serde_json::json!({ "pdf": "javascript:x" })).is_empty());
    }

    #[test]
    fn huge_passages_are_cut() {
        let html = "<p>x</p>".repeat(MAX_BLOCKS + 10);
        assert_eq!(html_blocks(&html, None).len(), MAX_BLOCKS);
    }
}
