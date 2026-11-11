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
