//! A page's text, read back from what it draws: each glyph with the character it stands for
//! (its font's `ToUnicode` map or glyph name), where it sits and how big it is. Glyphs join into
//! lines in the order the page draws them (which follows columns, unlike sorting by position),
//! lines into paragraphs where the gap between them grows, a first line is indented or the size
//! changes; a hyphen ending a line joins the word it split. Larger lines are headings, and a lone
//! page number at the top or bottom is left out. Invisible text counts: it's what a scanned book's
//! recognised text is drawn as.

use hayro::hayro_interpret::font::{Glyph, GlyphRun};
use hayro::hayro_interpret::hayro_cmap::BfString;
use hayro::hayro_interpret::util::TransformExt;
use hayro::hayro_interpret::{
    BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image, ImageDrawProps,
    InterpreterCache, InterpreterSettings, SoftMask, interpret_page,
};
use hayro::hayro_syntax::page::Page;
use hayro::kurbo::{BezPath, Point, Rect};
use serde::{Deserialize, Serialize};

/// Characters read from one page at most.
const MAX_CHARS: usize = 200_000;

/// One paragraph or heading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextBlock {
    pub text: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub heading: bool,
}

/// A page's text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageText {
    pub blocks: Vec<TextBlock>,
}

impl PageText {
    /// The page's text as one string, its blocks on lines of their own.
    pub fn plain(&self) -> String {
        self.blocks
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// One glyph's character(s) and place, in points from the page's top left.
#[derive(Debug, Clone)]
struct Piece {
    text: String,
    x0: f64,
    x1: f64,
    /// The baseline.
    y: f64,
    size: f64,
}

#[derive(Default)]
struct Collector {
    pieces: Vec<Piece>,
    chars: usize,
}

impl<'a> Device<'a> for Collector {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_image(&mut self, _: Image<'a, '_>, _: ImageDrawProps<'a>) {}
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}

    fn draw_glyph_run(&mut self, run: &GlyphRun<'_, 'a>, props: DrawProps<'a>, _: &DrawMode) {
        for glyph in run.glyphs() {
            if self.chars >= MAX_CHARS {
                return;
            }
            let Some(text) = glyph.as_unicode() else {
                continue;
            };
            let text = match text {
                BfString::Char(char) => char.to_string(),
                BfString::String(text) => text,
            };
            // Control characters some fonts map their glyphs to say nothing.
            let text: String = text.chars().filter(|char| !char.is_control()).collect();
            if text.is_empty() {
                continue;
            }
            let transform = props.transform * glyph.transform();
            let [a, b, c, d, _, _] = transform.as_coeffs();
            // Glyph space has 1000 units to the em.
            let size = (a * d - b * c).abs().sqrt() * 1000.0;
            if !size.is_finite() || size <= 0.0 {
                continue;
            }
            let advance = match &**glyph {
                Glyph::Outline(outline) => outline.advance_width(),
                Glyph::Type3(type3) => type3.advance_width(),
            }
            .filter(|advance| advance.is_finite() && *advance > 0.0)
            .unwrap_or(500.0);
            let start = transform * Point::ZERO;
            let end = transform * Point::new(f64::from(advance), 0.0);
            self.chars += text.chars().count();
            self.pieces.push(Piece {
                text,
                x0: start.x.min(end.x),
                x1: start.x.max(end.x),
                y: start.y,
                size,
            });
        }
    }
}

/// A run of pieces on one baseline.
#[derive(Debug, Clone)]
struct Line {
    text: String,
    x0: f64,
    x1: f64,
    y: f64,
    size: f64,
}

pub(super) fn read(page: &Page<'_>) -> PageText {
    let (width, height) = page.render_dimensions();
    let cache = InterpreterCache::new();
    let mut context = Context::new(
        page.initial_transform(true).to_kurbo(),
        Rect::new(0.0, 0.0, f64::from(width), f64::from(height)),
        &cache,
        page.xref(),
        InterpreterSettings::default(),
    );
    let mut collector = Collector::default();
    interpret_page(page, &mut context, &mut collector);
    blocks_of(collector.pieces, f64::from(height))
}

/// Pieces in drawing order → lines → paragraphs and headings.
fn blocks_of(pieces: Vec<Piece>, page_height: f64) -> PageText {
    let mut lines = lines_of(pieces);
    drop_page_numbers(&mut lines, page_height);
    if lines.is_empty() {
        return PageText::default();
    }
    let body = body_size(&lines);
    let spacing = line_spacing(&lines);
    let left = lines
        .iter()
        .map(|line| line.x0)
        .fold(f64::INFINITY, f64::min);
    let right = lines.iter().map(|line| line.x1).fold(0.0, f64::max);

    let mut blocks: Vec<TextBlock> = Vec::new();
    let mut current: Option<(String, f64, usize)> = None;
    let mut previous: Option<&Line> = None;
    for line in &lines {
        let starts = match previous {
            None => true,
            Some(before) => starts_paragraph(before, line, spacing, left, right),
        };
        if starts && let Some((text, size, count)) = current.take() {
            blocks.push(block(text, size, count, body));
        }
        current = Some(match current.take() {
            None => (line.text.trim().to_owned(), line.size, 1),
            Some((text, size, count)) => (join_lines(&text, line.text.trim()), size, count + 1),
        });
        previous = Some(line);
    }
    if let Some((text, size, count)) = current {
        blocks.push(block(text, size, count, body));
    }
    blocks.retain(|block| !block.text.is_empty());
    PageText { blocks }
}

fn block(text: String, size: f64, lines: usize, body: f64) -> TextBlock {
    let heading = size >= body * 1.2 && lines <= 3 && text.chars().count() <= 200;
    TextBlock { text, heading }
}

fn lines_of(pieces: Vec<Piece>) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    for piece in pieces {
        let joins = lines.last().is_some_and(|line| {
            let size = line.size.max(piece.size);
            (piece.y - line.y).abs() <= size * 0.4 && piece.x0 >= line.x1 - size * 1.0
        });
        if !joins {
            lines.push(Line {
                text: piece.text.clone(),
                x0: piece.x0,
                x1: piece.x1,
                y: piece.y,
                size: piece.size,
            });
            continue;
        }
        let line = lines.last_mut().expect("checked above");
        let gap = piece.x0 - line.x1;
        let spaced = line.text.ends_with(' ') || piece.text.starts_with(' ');
        if !spaced && gap > line.size.min(piece.size) * 0.15 {
            line.text.push(' ');
        }
        line.text.push_str(&piece.text);
        line.x1 = line.x1.max(piece.x1);
        line.x0 = line.x0.min(piece.x0);
        // A line's size is its body text's: a raised footnote mark doesn't change it.
        if piece.text.chars().count() > 1 || piece.size > line.size {
            line.size = line.size.max(piece.size);
        }
    }
    for line in &mut lines {
        line.text = line.text.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    lines.retain(|line| !line.text.is_empty());
    lines
}

/// A page number alone at the top or bottom of the page.
fn drop_page_numbers(lines: &mut Vec<Line>, page_height: f64) {
    let is_number = |line: &Line| {
        let text = line.text.trim();
        !text.is_empty()
            && text.len() <= 8
            && (text.chars().all(|char| char.is_ascii_digit())
                || text
                    .chars()
                    .all(|char| matches!(char.to_ascii_lowercase(), 'i' | 'v' | 'x' | 'l' | 'c')))
    };
    let near_edge = |line: &Line| line.y < page_height * 0.1 || line.y > page_height * 0.9;
    if lines
        .last()
        .is_some_and(|line| is_number(line) && near_edge(line))
    {
        lines.pop();
    }
    if lines.len() > 1
        && lines
            .first()
            .is_some_and(|line| is_number(line) && near_edge(line))
    {
        lines.remove(0);
    }
}

/// The size most of the page's text is set in.
fn body_size(lines: &[Line]) -> f64 {
    let mut sizes: Vec<(f64, usize)> = Vec::new();
    for line in lines {
        let chars = line.text.chars().count();
        match sizes
            .iter_mut()
            .find(|(size, _)| (size - line.size).abs() < 0.5)
        {
            Some((_, count)) => *count += chars,
            None => sizes.push((line.size, chars)),
        }
    }
    sizes
        .iter()
        .max_by_key(|(_, count)| *count)
        .map_or(12.0, |(size, _)| *size)
}

/// The usual distance between two lines' baselines in a paragraph: a low quartile of the gaps, as
/// gaps between paragraphs and around headings are the larger ones.
fn line_spacing(lines: &[Line]) -> f64 {
    let mut gaps: Vec<f64> = lines
        .windows(2)
        .map(|pair| pair[1].y - pair[0].y)
        .filter(|gap| *gap > 0.0)
        .collect();
    if gaps.is_empty() {
        return lines.first().map_or(14.0, |line| line.size * 1.2);
    }
    gaps.sort_by(f64::total_cmp);
    gaps[gaps.len() / 4]
}

fn starts_paragraph(before: &Line, line: &Line, spacing: f64, left: f64, right: f64) -> bool {
    let gap = line.y - before.y;
    // Up the page: a new column or a box beside the text.
    if gap <= 0.0 {
        return true;
    }
    if gap > spacing.max(before.size) * 1.45 {
        return true;
    }
    let size = before.size.max(line.size);
    if (line.size - before.size).abs() > size * 0.15 {
        return true;
    }
    // A first line indented under a line that wasn't.
    if line.x0 - left > size * 0.8 && before.x0 - left <= size * 0.8 {
        return true;
    }
    // A short line that ends a sentence ends its paragraph.
    let width = (right - left).max(1.0);
    let short = before.x1 < left + width * 0.75;
    short
        && before
            .text
            .trim_end()
            .ends_with(['.', '!', '?', ':', '"', '”', '’'])
}

/// Two lines of one paragraph: a hyphen that split a word goes, otherwise a space between.
fn join_lines(text: &str, next: &str) -> String {
    let continues = next.chars().next().is_some_and(char::is_lowercase);
    match text.strip_suffix('-') {
        Some(word) if continues && word.chars().last().is_some_and(char::is_alphabetic) => {
            format!("{word}{next}")
        }
        _ => format!("{text} {next}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(text: &str, x0: f64, y: f64, size: f64) -> Piece {
        Piece {
            text: text.into(),
            x0,
            x1: x0 + text.chars().count() as f64 * size * 0.5,
            y,
            size,
        }
    }

    #[test]
    fn lines_join_into_paragraphs_and_headings() {
        let pieces = vec![
            piece("Chapter", 30.0, 50.0, 20.0),
            piece("One", 112.0, 50.0, 20.0),
            piece("It was a bright cold day", 30.0, 82.0, 11.0),
            piece("and the clocks were thir-", 30.0, 96.0, 11.0),
            piece("teen.", 30.0, 110.0, 11.0),
            piece("Next one.", 30.0, 150.0, 11.0),
            piece("7", 146.0, 380.0, 9.0),
        ];
        let text = blocks_of(pieces, 400.0);
        assert_eq!(
            text.blocks,
            [
                TextBlock {
                    text: "Chapter One".into(),
                    heading: true
                },
                TextBlock {
                    text: "It was a bright cold day and the clocks were thirteen.".into(),
                    heading: false
                },
                TextBlock {
                    text: "Next one.".into(),
                    heading: false
                },
            ]
        );
    }

    #[test]
    fn indented_first_lines_start_paragraphs() {
        let pieces = vec![
            piece("The end of one paragraph that runs on", 30.0, 50.0, 10.0),
            piece("to here.", 30.0, 62.0, 10.0),
            piece("Another begins indented and", 45.0, 74.0, 10.0),
            piece("goes on.", 30.0, 86.0, 10.0),
        ];
        let text = blocks_of(pieces, 400.0);
        assert_eq!(text.blocks.len(), 2);
        assert_eq!(text.blocks[1].text, "Another begins indented and goes on.");
    }

    #[test]
    fn words_part_where_glyphs_leave_a_gap() {
        let mut pieces = Vec::new();
        let mut x = 10.0;
        for char in "ab".chars() {
            pieces.push(piece(&char.to_string(), x, 20.0, 10.0));
            x += 5.0;
        }
        // A gap of a third of the size: a space.
        x += 3.3;
        pieces.push(piece("c", x, 20.0, 10.0));
        assert_eq!(blocks_of(pieces, 400.0).plain(), "ab c");
    }
}
