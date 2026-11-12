//! Tables in book text. A table of data (rows and columns: a list of commands with what each
//! does, symbols and their names) becomes a [`Table`] of cells, each cell's text in parts (italics,
//! code, formulas); one that only lays its contents out (a code example beside its result, a box
//! around a note, a gallery) is read through as blocks, as before.

use ego_tree::NodeRef;
use scraper::Node;
use scraper::node::Element;
use serde::{Deserialize, Serialize};

use super::{Part, image_formula, plain_text};

/// Most rows a table keeps, and cells a row.
const MAX_ROWS: usize = 2_000;
const MAX_CELLS: usize = 64;
/// The most columns or rows a cell may span.
const MAX_SPAN: u16 = 64;
/// Elements in a cell that lay a table out rather than fill it: code set out line by line,
/// another table, a figure, a heading, a quotation.
const LAYOUT: [&str; 12] = [
    "pre",
    "listing",
    "xmp",
    "table",
    "figure",
    "blockquote",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
];

/// A [`super::PassageBlock::Table`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Table {
    /// Its caption (`<caption>`) in parts, when it has one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caption: Vec<Part>,
    /// Its rows in order, each its cells from the first column on.
    pub rows: Vec<Vec<TableCell>>,
}

/// One cell of a [`Table`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableCell {
    /// The cell's text in parts; `\n` in a text part breaks a line.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Part>,
    /// A header cell (`<th>`, or a cell of the table's head).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub header: bool,
    /// How many columns and rows the cell takes.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub colspan: u16,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub rowspan: u16,
}

fn one() -> u16 {
    1
}

fn is_one(value: &u16) -> bool {
    *value == 1
}

impl TableCell {
    /// The cell's text as plain text, its lines kept.
    pub fn plain(&self) -> String {
        plain_text(&self.parts)
    }
}

impl Table {
    /// The table as plain text: its caption, then each row on a line of its own, cells a tab
    /// apart (a cell's own line breaks become spaces).
    pub fn plain(&self) -> String {
        let caption = plain_text(&self.caption).replace('\n', " ");
        let rows = self.rows.iter().map(|row| {
            row.iter()
                .map(|cell| cell.plain().replace('\n', " "))
                .collect::<Vec<_>>()
                .join("\t")
        });
        std::iter::once(caption)
            .filter(|caption| !caption.is_empty())
            .chain(rows)
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(super) fn bytes(&self) -> usize {
        self.caption
            .iter()
            .chain(self.rows.iter().flatten().flat_map(|cell| &cell.parts))
            .map(Part::bytes)
            .sum()
    }
}

fn is_element(node: &NodeRef<'_, Node>, name: &str) -> bool {
    node.value()
        .as_element()
        .is_some_and(|element| element.name() == name)
}

/// The table's own rows in order, each with whether it's in the table's head (not the rows of
/// tables inside it).
fn rows(table: NodeRef<'_, Node>) -> Vec<(NodeRef<'_, Node>, bool)> {
    let mut rows = Vec::new();
    for child in table.children() {
        let Some(element) = child.value().as_element() else {
            continue;
        };
        match element.name() {
            "tr" => rows.push((child, false)),
            "thead" | "tbody" | "tfoot" => {
                let head = element.name() == "thead";
                rows.extend(
                    child
                        .children()
                        .filter(|row| is_element(row, "tr"))
                        .map(|row| (row, head)),
                );
            }
            _ => {}
        }
    }
    rows
}

/// A row's cells, each with its element.
fn cells(row: NodeRef<'_, Node>) -> impl Iterator<Item = (NodeRef<'_, Node>, &Element)> {
    row.children().filter_map(|cell| {
        let element = cell.value().as_element()?;
        matches!(element.name(), "td" | "th").then_some((cell, element))
    })
}

/// A cell's `colspan` or `rowspan`: 1 unless it says more.
fn span(element: &Element, name: &str) -> u16 {
    element
        .attr(name)
        .and_then(|value| value.trim().parse::<u16>().ok())
        .filter(|span| *span > 0)
        .unwrap_or(1)
        .min(MAX_SPAN)
}

/// Whether a `<table>` holds data, rows and columns to show as a table, rather than laying its
/// contents out: tables of one row or one column (a code example beside its result, a box around a
/// note), tables whose cells hold code blocks, tables or headings, galleries (pictures in half the
/// cells or more) and tables marked `role="presentation"` lay out.
pub(super) fn is_data_table(table: NodeRef<'_, Node>, element: &Element) -> bool {
    if matches!(
        element.attr("role").map(str::trim),
        Some("presentation" | "none")
    ) {
        return false;
    }
    let rows = rows(table);
    if rows.len() < 2 {
        return false;
    }
    let mut columns = 0;
    let mut count = 0;
    let mut pictures = 0;
    for (row, _) in &rows {
        let mut width = 0;
        for (cell, element) in cells(*row) {
            count += 1;
            width += usize::from(span(element, "colspan"));
            let mut picture = false;
            for inner in cell.descendants().skip(1) {
                let Some(inner) = inner.value().as_element() else {
                    continue;
                };
                if LAYOUT.contains(&inner.name()) {
                    return false;
                }
                picture |= inner.name() == "img" && image_formula(inner).is_none();
            }
            pictures += usize::from(picture);
        }
        columns = columns.max(width);
    }
    columns >= 2 && pictures * 2 < count
}

/// A data table's rows of cells, each cell's text read by `read`; `None` when it has no text.
/// Rows stop once `budget` bytes of text are read.
pub(super) fn build<'a>(
    table: NodeRef<'a, Node>,
    mut read: impl FnMut(NodeRef<'a, Node>) -> Vec<Part>,
    budget: usize,
) -> Option<Table> {
    let caption = table
        .children()
        .find(|child| is_element(child, "caption"))
        .map(&mut read)
        .unwrap_or_default();
    let mut bytes: usize = caption.iter().map(Part::bytes).sum();
    let mut kept = Vec::new();
    for (row, head) in rows(table).into_iter().take(MAX_ROWS) {
        let mut row_cells = Vec::new();
        for (cell, element) in cells(row).take(MAX_CELLS) {
            let parts = read(cell);
            bytes += parts.iter().map(Part::bytes).sum::<usize>();
            row_cells.push(TableCell {
                parts,
                header: head || element.name() == "th",
                colspan: span(element, "colspan"),
                rowspan: span(element, "rowspan"),
            });
        }
        if !row_cells.is_empty() {
            kept.push(row_cells);
        }
        if bytes >= budget {
            break;
        }
    }
    let has_text = !caption.is_empty() || kept.iter().flatten().any(|cell| !cell.parts.is_empty());
    has_text.then_some(Table {
        caption,
        rows: kept,
    })
}

/// The words a picture in a table's cell stands for (its `alt`), unless they only name its file.
pub(super) fn picture_words(element: &Element) -> Option<String> {
    let alt = element.attr("alt")?.trim();
    let file = alt.rsplit_once('.').is_some_and(|(_, extension)| {
        matches!(
            extension.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp"
        )
    });
    (!alt.is_empty() && alt.chars().count() <= 60 && !file).then(|| alt.to_owned())
}
