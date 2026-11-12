//! Text set apart (italics, bold, code), list items and tables in book text.

use super::*;
use PassageBlock::*;

fn text(text: &str) -> Part {
    Part::Text(text.into())
}

fn styled(text: &str, italic: bool, bold: bool, code: bool) -> Part {
    Part::Styled(Styled {
        text: text.into(),
        italic,
        bold,
        code,
    })
}

fn italic(text: &str) -> Part {
    styled(text, true, false, false)
}

fn bold(text: &str) -> Part {
    styled(text, false, true, false)
}

fn code(text: &str) -> Part {
    styled(text, false, false, true)
}

fn cell(parts: Vec<Part>) -> TableCell {
    TableCell {
        parts,
        header: false,
        colspan: 1,
        rowspan: 1,
    }
}

fn header(parts: Vec<Part>) -> TableCell {
    TableCell {
        header: true,
        ..cell(parts)
    }
}

fn item(marker: &str, depth: u8, parts: Vec<Part>) -> PassageBlock {
    Item(ListItem {
        marker: marker.into(),
        depth,
        parts,
    })
}

fn only_table(blocks: &[PassageBlock]) -> &tables::Table {
    let tables: Vec<&tables::Table> = blocks
        .iter()
        .filter_map(|block| match block {
            Table(table) => Some(table),
            _ => None,
        })
        .collect();
    let [table] = tables.as_slice() else {
        panic!("one table: {blocks:#?}");
    };
    table
}

#[test]
fn italics_bold_and_code_are_parts_of_a_paragraph() {
    let html = r#"<p>It was a <i>dreary</i> night, <b>very <em>very</em></b> cold; type <code>ls  -l</code> or <span style="font-family: monospace">cd</span>.</p>"#;
    assert_eq!(
        html_blocks(html, None),
        [Rich(vec![
            text("It was a "),
            italic("dreary"),
            text(" night, "),
            bold("very"),
            text(" "),
            styled("very", true, true, false),
            text(" cold; type "),
            code("ls -l"),
            text(" or "),
            code("cd"),
            text("."),
        ])]
    );
    // Without anything set apart, a paragraph is plain text as before.
    assert_eq!(
        html_blocks("<p>Just <span>words</span> here</p>", None),
        [Paragraph("Just words here".into())]
    );
}

#[test]
fn spaces_at_the_edges_of_set_apart_text_belong_to_the_text_around_it() {
    let html = "<p><i> lead</i>and<b>trail </b>then <i>two</i> <i>runs</i></p>";
    let blocks = html_blocks(html, None);
    assert_eq!(
        blocks,
        [Rich(vec![
            italic("lead"),
            text("and"),
            bold("trail"),
            text(" then "),
            italic("two"),
            text(" "),
            italic("runs"),
        ])]
    );
    assert_eq!(
        blocks[0].plain().as_deref(),
        Some("leadandtrail then two runs")
    );
    // Styles from `style`, and runs set alike that touch, join.
    let html = r#"<p><span style="font-style: italic">one</span><em> two</em> <span style="font-weight:700">three</span><span style="font-weight: normal">four</span></p>"#;
    assert_eq!(
        html_blocks(html, None),
        [Rich(vec![
            italic("one two"),
            text(" "),
            bold("three"),
            text("four"),
        ])]
    );
}

/// Wikibooks' `LaTeX/Document_Structure`: the sectioning commands, a placeholder in italics
/// inside each command (the extension turns MediaWiki's `''part''` into `<i>part</i>`).
const SECTIONING: &str = r#"
<p>LaTeX provides 7 levels of depth for defining sections (see table below).
</p>
<table class="wikitable">
<tbody><tr>
<th>Command
</th>
<th width="10%">Level
</th>
<th>Comment
</th></tr>
<tr>
<td><code class="mw-highlight mw-highlight-lang-latex mw-content-ltr" dir="ltr"><span class="k">\part</span><span class="nb">{</span><i>part</i><span class="nb">}</span></code>
</td>
<td style="text-align: center">-1
</td>
<td>not in letters
</td></tr>
<tr>
<td><code class="mw-highlight mw-highlight-lang-latex mw-content-ltr" dir="ltr"><span class="k">\chapter</span><span class="nb">{</span><i>chapter</i><span class="nb">}</span></code>
</td>
<td style="text-align: center">0
</td>
<td>only books and reports
</td></tr></tbody></table>
<p>All the titles of the sections are added automatically to the table of contents.</p>
"#;

#[test]
fn a_table_of_data_is_a_table_with_its_cells_in_parts() {
    let blocks = html_blocks(SECTIONING, None);
    assert_eq!(blocks.len(), 3, "{blocks:#?}");
    assert!(matches!(&blocks[0], Paragraph(text) if text.starts_with("LaTeX provides")));
    assert!(matches!(&blocks[2], Paragraph(text) if text.starts_with("All the titles")));
    let table = only_table(&blocks);
    assert!(table.caption.is_empty());
    assert_eq!(
        table.rows,
        [
            vec![
                header(vec![text("Command")]),
                header(vec![text("Level")]),
                header(vec![text("Comment")]),
            ],
            vec![
                cell(vec![
                    code("\\part{"),
                    styled("part", true, false, true),
                    code("}")
                ]),
                cell(vec![text("-1")]),
                cell(vec![text("not in letters")]),
            ],
            vec![
                cell(vec![
                    code("\\chapter{"),
                    styled("chapter", true, false, true),
                    code("}")
                ]),
                cell(vec![text("0")]),
                cell(vec![text("only books and reports")]),
            ],
        ]
    );
    assert_eq!(
        table.plain(),
        "Command\tLevel\tComment\n\\part{part}\t-1\tnot in letters\n\\chapter{chapter}\t0\tonly books and reports"
    );
    // Nothing of it runs together into paragraphs.
    assert!(
        blocks
            .iter()
            .all(|block| !matches!(block, Paragraph(text) if text.contains("not in letters")))
    );
}

#[test]
fn tables_that_lay_out_are_read_through() {
    // `{{LaTeX/Example}}`: the code beside what it typesets, one row.
    let example = r#"<table style="margin-top: 1.5em; margin-bottom: 1.5em"><tbody><tr>
        <td style="border-right:2px black solid;padding-right:3em;"><div class="mw-highlight mw-highlight-lang-latex mw-content-ltr" dir="ltr"><pre><span class="k">\begin</span><span class="nb">{</span>tabular<span class="nb">}{</span> l c r <span class="nb">}</span>
  1 <span class="nb">&amp;</span> 2 <span class="nb">&amp;</span> 3 <span class="k">\\</span>
<span class="k">\end</span><span class="nb">{</span>tabular<span class="nb">}</span>
</pre></div></td>
        <td style="padding-left:3em;"><p><math class="mwe-math-element mwe-math-element-inline" xmlns="http://www.w3.org/1998/Math/MathML"><mrow><mtable><mtr><mtd><mn>1</mn></mtd><mtd><mn>2</mn></mtd><mtd><mn>3</mn></mtd></mtr></mtable></mrow></math></p></td>
        </tr></tbody></table>"#;
    let blocks = html_blocks(example, None);
    assert!(
        matches!(&blocks[..], [Code(code), Math(formula)]
            if code.language.as_deref() == Some("latex")
                && code.text == "\\begin{tabular}{ l c r }\n  1 & 2 & 3 \\\\\n\\end{tabular}"
                && formula.display),
        "{blocks:#?}"
    );
    // One column, a box around a note, a gallery of pictures, a table marked as layout.
    let one_column = "<table><tr><td>First note</td></tr><tr><td>Second note</td></tr></table>";
    let gallery = r#"<table><tr><td><img src="https://img.example/a.jpg"> A</td><td><img src="https://img.example/b.jpg"> B</td></tr><tr><td><img src="https://img.example/c.jpg"> C</td><td>D</td></tr></table>"#;
    let marked = r#"<table role="presentation"><tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></table>"#;
    let with_code =
        "<table><tr><td>a</td><td><pre>x = 1</pre></td></tr><tr><td>b</td><td>c</td></tr></table>";
    for html in [one_column, gallery, marked, with_code] {
        let blocks = html_blocks(html, None);
        assert!(
            !blocks.iter().any(|block| matches!(block, Table(_))),
            "{html}: {blocks:#?}"
        );
    }
    assert_eq!(
        html_blocks(one_column, None),
        [
            Paragraph("First note".into()),
            Paragraph("Second note".into())
        ]
    );
    assert_eq!(
        html_blocks(gallery, None)
            .iter()
            .filter(|block| matches!(block, Image(_)))
            .count(),
        3
    );
}

#[test]
fn table_cells_keep_their_lines_spans_and_formulas() {
    let html = r#"<table>
        <caption>Greek <i>letters</i></caption>
        <thead><tr><th colspan="2">Letter</th><th rowspan="2">Note</th></tr></thead>
        <tbody>
        <tr><td><code>\alpha</code></td><td><math><mi>α</mi></math></td></tr>
        <tr><td>Line one<br>Line two</td><td><p>First</p><p>Second</p></td><td><ul><li>a</li><li>b</li></ul></td></tr>
        <tr><td><img src="check.png" alt="Yes"></td><td>See <img src="https://img.example/dots.png" alt="dots.png"></td><td>$x^2$ costs $5</td></tr>
        </tbody></table>"#;
    let blocks = html_blocks(html, None);
    let table = only_table(&blocks);
    assert_eq!(table.caption, [text("Greek "), italic("letters")]);
    assert_eq!(
        table.rows[0],
        [
            TableCell {
                colspan: 2,
                ..header(vec![text("Letter")])
            },
            TableCell {
                rowspan: 2,
                ..header(vec![text("Note")])
            },
        ]
    );
    assert_eq!(table.rows[1][0], cell(vec![code("\\alpha")]));
    assert!(matches!(&table.rows[1][1].parts[..], [Part::Math(formula)] if formula.text == "α"));
    assert_eq!(table.rows[2][0], cell(vec![text("Line one\nLine two")]));
    assert_eq!(table.rows[2][1], cell(vec![text("First\nSecond")]));
    assert_eq!(table.rows[2][2], cell(vec![text("• a\n• b")]));
    // A picture stays in its cell; one that can't be loaded leaves its words (not a file name).
    // TeX outside code is a formula.
    assert_eq!(table.rows[3][0], cell(vec![text("Yes")]));
    assert_eq!(
        table.rows[3][1],
        cell(vec![
            text("See "),
            Part::Image("https://img.example/dots.png".into())
        ])
    );
    assert!(matches!(
        &table.rows[3][2].parts[..],
        [Part::Math(formula), Part::Text(rest)] if formula.text == "x^2" && rest == " costs $5"
    ));
    assert_eq!(
        table.plain().lines().next(),
        Some("Greek letters"),
        "the caption leads"
    );
    // A table without any text is left out.
    let empty = "<table><tr><td> </td><td></td></tr><tr><td></td><td><img src=\"a.png\" alt=\"\"></td></tr></table>";
    assert!(html_blocks(empty, None).is_empty());
}

/// Wikibooks' `LaTeX/Mathematics`: row headers beside a header row, code in the cells.
#[test]
fn row_headers_and_code_with_tex_in_cells() {
    let html = r#"<table class="wikitable"><tbody><tr>
        <th width="15%">Type</th><th>Inline (within text) formulas</th><th>Displayed equations</th></tr>
        <tr><th>Environment</th><td><code class="mw-highlight mw-highlight-lang-latex">math</code></td><td><code class="mw-highlight mw-highlight-lang-latex">displaymath</code></td></tr>
        <tr><th width="15%">TeX shorthand</th><td><code class="mw-highlight mw-highlight-lang-latex"><span class="s">$</span><span class="nb">...</span><span class="s">$</span></code></td><td><code class="mw-highlight mw-highlight-lang-latex"><span class="sb">$$</span><span class="nb">...</span><span class="s">$$</span></code></td></tr>
        </tbody></table>"#;
    let blocks = html_blocks(html, None);
    let table = only_table(&blocks);
    assert_eq!(table.rows.len(), 3);
    assert!(table.rows[0].iter().all(|cell| cell.header));
    assert!(table.rows[1][0].header && !table.rows[1][1].header);
    assert_eq!(table.rows[2][1], cell(vec![code("$...$")]));
    assert_eq!(table.rows[2][2], cell(vec![code("$$...$$")]));
}

#[test]
fn list_items_carry_their_marks_and_depth() {
    let html = r#"
        <ul><li>First</li><li><i>Second</i> item<ul><li>Inner</li></ul> and more</li></ul>
        <ol start="3"><li>Three</li><li value="7">Seven</li><li><p>Eight</p><p>Still eight</p></li></ol>
        <ol type="a"><li>a</li><li>b</li></ol>
        <ol style="list-style-type: upper-roman"><li>one</li><li>two</li></ol>
        <ul style="list-style: none"><li>Plain</li></ul>
        <dl><dt>Term</dt><dd>Its meaning</dd></dl>
        <blockquote><p>Quoted</p></blockquote>
    "#;
    assert_eq!(
        html_blocks(html, None),
        [
            item("•", 1, vec![text("First")]),
            item("•", 1, vec![italic("Second"), text(" item")]),
            item("◦", 2, vec![text("Inner")]),
            item("", 1, vec![text("and more")]),
            item("3.", 1, vec![text("Three")]),
            item("7.", 1, vec![text("Seven")]),
            item("8.", 1, vec![text("Eight")]),
            item("", 1, vec![text("Still eight")]),
            item("a.", 1, vec![text("a")]),
            item("b.", 1, vec![text("b")]),
            item("I.", 1, vec![text("one")]),
            item("II.", 1, vec![text("two")]),
            item("", 1, vec![text("Plain")]),
            Rich(vec![bold("Term")]),
            item("", 1, vec![text("Its meaning")]),
            item("", 1, vec![text("Quoted")]),
        ]
    );
    assert_eq!(
        (letters(27), roman(1994)),
        ("aa".to_owned(), "mcmxciv".to_owned())
    );
}

#[test]
fn a_formula_alone_in_its_paragraph_is_set_on_its_own_line() {
    // MediaWiki's `:<math>…</math>`: an indented line holding a formula.
    let html = r#"<dl><dd><math><mi>E</mi><mo>=</mo><mi>m</mi><msup><mi>c</mi><mn>2</mn></msup></math></dd></dl><p>Text with <math><mi>x</mi></math> in it.</p><ul><li><math><mi>y</mi></math></li></ul>"#;
    let blocks = html_blocks(html, None);
    assert!(
        matches!(&blocks[0], Math(formula) if formula.display && formula.text.contains("mc")),
        "{blocks:#?}"
    );
    assert!(matches!(&blocks[1], Rich(parts) if parts.len() == 3));
    // A formula that is a list item keeps its mark.
    assert!(
        matches!(&blocks[2], Item(item) if item.marker == "•" && matches!(&item.parts[..], [Part::Math(_)]))
    );
}

#[test]
fn tables_lists_and_styles_keep_their_shape_in_downloads() {
    let blocks = html_blocks(SECTIONING, None);
    let json = serde_json::to_value(&blocks).unwrap();
    assert_eq!(
        json[1]["value"]["rows"][1][0],
        serde_json::json!({ "parts": [
            { "kind": "styled", "value": { "text": "\\part{", "code": true } },
            { "kind": "styled", "value": { "text": "part", "italic": true, "code": true } },
            { "kind": "styled", "value": { "text": "}", "code": true } },
        ] })
    );
    assert_eq!(json[1]["value"]["rows"][0][0]["header"], true);
    assert_eq!(
        serde_json::from_value::<Vec<PassageBlock>>(json).unwrap(),
        blocks
    );
    let spans = serde_json::json!({ "kind": "table", "value": { "rows": [[{ "parts": [{ "kind": "text", "value": "a" }], "colspan": 2 }]] } });
    let Table(table) = serde_json::from_value::<PassageBlock>(spans).unwrap() else {
        panic!("a table");
    };
    assert_eq!((table.rows[0][0].colspan, table.rows[0][0].rowspan), (2, 1));
    let listed = item("2.", 1, vec![text("x")]);
    assert_eq!(
        serde_json::to_value(&listed).unwrap(),
        serde_json::json!({ "kind": "item", "value": { "marker": "2.", "depth": 1, "parts": [{ "kind": "text", "value": "x" }] } })
    );
}

#[test]
fn a_download_keeps_what_its_blocks_are_read_from() {
    let answer = serde_json::json!({ "html": SECTIONING, "baseUrl": "https://en.wikibooks.org/wiki/LaTeX", "title": "x" });
    let kept = passage_source(&answer).unwrap();
    assert_eq!(kept["baseUrl"], "https://en.wikibooks.org/wiki/LaTeX");
    assert!(kept.get("title").is_none());
    assert_eq!(parse_passage(&kept), parse_passage(&answer));
    let text = serde_json::json!({ "text": "Plain words" });
    assert_eq!(passage_source(&text), Some(text.clone()));
    // A PDF chapter keeps its document instead.
    let pdf = serde_json::json!({ "pdf": "https://books.example/a.pdf", "html": "<p>x</p>" });
    assert!(passage_source(&pdf).is_none());
    assert!(passage_source(&serde_json::json!({})).is_none());
}

#[test]
fn huge_tables_are_cut() {
    let row = "<tr><td>a</td><td>b</td></tr>";
    let html = format!("<table>{}</table>", row.repeat(3_000));
    let blocks = html_blocks(&html, None);
    assert_eq!(only_table(&blocks).rows.len(), 2_000);
}
