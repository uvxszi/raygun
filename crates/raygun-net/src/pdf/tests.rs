//! `fixtures/sample.pdf`: three 300×400 pt pages set in Helvetica (not embedded, so the built-in
//! standard fonts draw it). Page 1: the heading "Chapter One", a paragraph over three lines with a
//! word split by a hyphen, a second paragraph, the page number. Page 2: "Chapter Two" and a
//! paragraph. Page 3: "Section 2.1", a line and a blue box. The outline: Chapter One (an explicit
//! destination), Chapter Two (a GoTo action) and under it Section 2.1 (a named destination, its
//! title in UTF-16). The document's title is "Sample Book" by "Raygun Tests".

use std::collections::BTreeMap;

use super::*;

const SAMPLE: &[u8] = include_bytes!("fixtures/sample.pdf");

fn sample() -> Document {
    Document::from_bytes(SAMPLE.to_vec()).expect("the fixture opens")
}

#[test]
fn documents_say_what_they_hold() {
    let document = sample();
    assert_eq!(document.page_count(), 3);
    assert_eq!(document.page_size(0), Some((300.0, 400.0)));
    assert_eq!(
        document.info(),
        Info {
            title: Some("Sample Book".into()),
            author: Some("Raygun Tests".into()),
            subject: Some("A fixture for the PDF reader".into()),
        }
    );
    assert!(Document::from_bytes(b"<html>not a pdf</html>".to_vec()).is_err());
}

#[test]
fn the_outline_leads_to_pages() {
    let entries = sample().outline();
    let shown: Vec<(&str, usize, u32)> = entries
        .iter()
        .map(|entry| (entry.title.as_str(), entry.page, entry.level))
        .collect();
    assert_eq!(
        shown,
        [
            ("Chapter One", 0, 0),
            ("Chapter Two", 1, 0),
            ("Section 2.1", 2, 1)
        ]
    );
}

#[test]
fn pages_are_drawn_at_the_width_asked() {
    let document = sample();
    let png = document.render_png(2, 300).unwrap();
    let picture = image::load_from_memory(&png).unwrap().to_rgb8();
    assert_eq!(picture.dimensions(), (300, 400));
    // The blue box (30..270 × 100..160 pt from the bottom) and the white page around it.
    let blue = picture.get_pixel(150, 270).0;
    assert!(blue[2] > 150 && blue[0] < 100, "{blue:?}");
    assert_eq!(picture.get_pixel(5, 5).0, [255, 255, 255]);
    // The heading's text is drawn (dark pixels where it's set).
    let page = image::load_from_memory(&document.render_png(0, 600).unwrap())
        .unwrap()
        .to_luma8();
    let dark = (0..60)
        .flat_map(|y| (0..600).map(move |x| (x, y + 80)))
        .filter(|(x, y)| page.get_pixel(*x, *y).0[0] < 100)
        .count();
    assert!(
        dark > 200,
        "the heading should be drawn, {dark} dark pixels"
    );
}

#[test]
fn page_text_reads_as_paragraphs() {
    let document = sample();
    let first = document.page_text(0);
    let shown: Vec<(&str, bool)> = first
        .blocks
        .iter()
        .map(|block| (block.text.as_str(), block.heading))
        .collect();
    assert_eq!(
        shown,
        [
            ("Chapter One", true),
            (
                "It was a bright cold day in April, and the clocks were striking thirteen. \
                 Winston hurried home.",
                false
            ),
            ("A second paragraph begins here.", false),
        ]
    );
    assert_eq!(
        document.page_text(1).plain(),
        "Chapter Two\nThe keeper climbed the stairs of the old lighthouse every night."
    );
}

#[test]
fn marked_addresses_carry_documents_and_page_runs() {
    let headers = BTreeMap::from([("Referer".to_owned(), "https://site/".to_owned())]);
    let whole = PdfRef {
        url: "https://site/book.pdf".into(),
        headers: headers.clone(),
        first_page: None,
        last_page: None,
    };
    assert_eq!(whole.marked(), "https://site/book.pdf#raygun-pdf");
    assert_eq!(
        PdfRef::from_marked(&whole.marked(), &headers),
        Some(whole.clone())
    );
    let part = PdfRef {
        first_page: Some(3),
        last_page: Some(9),
        ..whole
    };
    assert_eq!(part.marked(), "https://site/book.pdf#raygun-pdf=3-9");
    assert_eq!(
        PdfRef::from_marked(&part.marked(), &headers),
        Some(part.clone())
    );
    assert_eq!(part.pages(20), 2..9);
    assert_eq!(part.pages(5), 2..5);
    assert_eq!(page_range(Some(9), Some(3), 20), 8..9);
    assert_eq!(page_range(None, None, 0), 0..0);
    assert!(PdfRef::from_marked("https://site/a.png", &headers).is_none());
}

#[tokio::test]
async fn the_store_draws_and_searches_local_documents() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("books/abc")).unwrap();
    std::fs::write(root.path().join("books/abc/document.pdf"), SAMPLE).unwrap();
    let cache = tempfile::tempdir().unwrap();
    let store = PdfStore::new(
        "127.0.0.1:9".parse().unwrap(),
        crate::http_client().unwrap(),
        Some(root.path().to_path_buf()),
        Some(cache.path().join("pdf")),
        Some(crate::PageCache::new(cache.path().join("pages"), 1 << 30)),
    );
    let token = store.local_token("books/abc/document.pdf");
    let png = store.page_png(&token, 1, Some(500)).await.unwrap();
    let picture = image::load_from_memory(&png).unwrap();
    assert_eq!(picture.width(), 720, "widths round up to the next of a few");
    // Drawn once, then read from the page cache.
    assert_eq!(store.page_png(&token, 1, Some(700)).await.unwrap(), png);
    assert_eq!(
        store.page_png(&token, 9, None).await,
        Err(PdfError::Missing)
    );

    let hits = store
        .search(&token, "LIGHTHOUSE every", 0..3)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].page, 1);
    assert_eq!(hits[0].matched, "lighthouse every");
    assert!(
        store
            .search(&token, "lighthouse", 2..3)
            .await
            .unwrap()
            .is_empty()
    );
    let texts = store.texts(&token, 2..3).await.unwrap();
    assert_eq!(texts[0].blocks[0].text, "Section 2.1");

    assert_eq!(
        store
            .page_png(&store.local_token("../secret.pdf"), 0, None)
            .await,
        Err(PdfError::Missing)
    );
    assert_eq!(
        store.page_png("wdeadbeef", 0, None).await,
        Err(PdfError::Missing),
        "a web document never registered"
    );
}

/// A PDF of `pages` blank 300×400 pt pages whose outline is `outline` (title, page from 0, level),
/// shaped as LaTeX's hyperref writes a book's: each bookmark a GoTo action to a named destination,
/// the names in the catalog's `/Dests` name tree, split into two leaves with their `/Limits`.
fn book_pdf(pages: usize, outline: &[(&str, usize, u32)]) -> Vec<u8> {
    // 1 catalog, 2 page tree, 3 outline root, 4 name tree root, 5 and 6 its leaves, then the
    // pages, then the bookmarks.
    let page_id = |page: usize| 7 + page;
    let item_id = |index: usize| 7 + pages + index;
    let name = |index: usize| format!("section.{index:03}");
    let parent = |index: usize| {
        (0..index)
            .rev()
            .find(|before| outline[*before].2 < outline[index].2)
    };
    let children = |of: Option<usize>| -> Vec<usize> {
        (0..outline.len())
            .filter(|index| parent(*index) == of)
            .collect()
    };
    let links = |of: Option<usize>| {
        let kids = children(of);
        match (kids.first(), kids.last()) {
            (Some(first), Some(last)) => format!(
                "/First {} 0 R /Last {} 0 R /Count {}",
                item_id(*first),
                item_id(*last),
                kids.len()
            ),
            _ => String::new(),
        }
    };
    let leaf = |range: std::ops::Range<usize>| {
        let names: String = range
            .clone()
            .map(|index| {
                format!(
                    "({}) [{} 0 R /XYZ 0 400 null] ",
                    name(index),
                    page_id(outline[index].1)
                )
            })
            .collect();
        format!(
            "<< /Names [{names}] /Limits [({}) ({})] >>",
            name(range.start),
            name(range.end - 1)
        )
    };
    let middle = outline.len() / 2;
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R /Outlines 3 0 R /Names << /Dests 4 0 R >> >>".to_owned(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {pages} >>",
            (0..pages)
                .map(|page| format!("{} 0 R", page_id(page)))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        format!("<< /Type /Outlines {} >>", links(None)),
        "<< /Kids [5 0 R 6 0 R] >>".to_owned(),
        leaf(0..middle),
        leaf(middle..outline.len()),
    ];
    objects.extend(
        (0..pages).map(|_| "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] >>".to_owned()),
    );
    for (index, (title, _, _)) in outline.iter().enumerate() {
        let siblings = children(parent(index));
        let at = siblings
            .iter()
            .position(|sibling| *sibling == index)
            .unwrap();
        let mut item = format!(
            "<< /Title ({title}) /Parent {} 0 R /A << /S /GoTo /D ({}) >> {}",
            parent(index).map_or(3, item_id),
            name(index),
            links(Some(index))
        );
        if at > 0 {
            item.push_str(&format!(" /Prev {} 0 R", item_id(siblings[at - 1])));
        }
        if let Some(next) = siblings.get(at + 1) {
            item.push_str(&format!(" /Next {} 0 R", item_id(*next)));
        }
        item.push_str(" >>");
        objects.push(item);
    }
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}

/// A book's outline as a textbook typeset with LaTeX has it (Think Python's: chapters and their
/// sections, pages of front matter before the first), its destinations found in a name tree that
/// is split into leaves.
#[test]
fn a_books_bookmarks_lead_through_a_split_name_tree() {
    let outline = [
        ("Preface", 2, 0),
        ("The way of the program", 4, 0),
        ("What is a program?", 4, 1),
        ("Running Python", 5, 1),
        ("Variables, expressions and statements", 7, 0),
        ("Assignment statements", 7, 1),
        ("Order of operations", 8, 1),
        ("Glossary", 9, 2),
    ];
    let document = Document::from_bytes(book_pdf(10, &outline)).expect("the generated book opens");
    assert_eq!(document.page_count(), 10);
    let read: Vec<(String, usize, u32)> = document
        .outline()
        .into_iter()
        .map(|entry| (entry.title, entry.page, entry.level))
        .collect();
    let wanted: Vec<(String, usize, u32)> = outline
        .iter()
        .map(|(title, page, level)| ((*title).to_owned(), *page, *level))
        .collect();
    assert_eq!(read, wanted);
}
