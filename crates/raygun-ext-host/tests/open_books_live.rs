//! The openly licensed textbook sources against their real sites (`open_books_live`): lists,
//! search with filters, details naming the licence, chapters, and that a chapter reads as real
//! paragraphs with its code and formulas kept and the site's navigation left out.
//!
//! `cargo test -p raygun-ext-host --test open_books_live -- --ignored --nocapture`, or one source
//! with its name after `open_books_live_`.

use std::collections::BTreeMap;
use std::path::Path;

use raygun_ext_host::manga::{MangaChapter, MangaList, MangaListing};
use raygun_ext_host::novel::{NOVEL_SOURCE, PassageBlock};
use raygun_ext_host::{ExtensionHost, HostConfig, discover_installed};
use raygun_model::{ExtensionStatus, FilterValue};

const WIKIBOOKS: &str = "dev.raygun.wikibooks/books";
const BCCAMPUS: &str = "dev.raygun.bccampus/textbooks";
const GNU_MANUALS: &str = "dev.raygun.gnu-manuals/manuals";
const GREEN_TEA_PRESS: &str = "dev.raygun.green-tea-press/books";
const SAYLOR: &str = "dev.raygun.saylor/textbooks";

/// Every first-party extension, loaded.
async fn host() -> ExtensionHost {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    for info in host.load(discover_installed(&dir)).await {
        assert_eq!(
            info.status,
            ExtensionStatus::Loaded,
            "{}: {:?}",
            info.id,
            info.error
        );
    }
    host
}

fn select(value: &str) -> FilterValue {
    FilterValue::Select(value.to_owned())
}

fn filters(pairs: &[(&str, &str)]) -> BTreeMap<String, FilterValue> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), select(value)))
        .collect()
}

fn assert_listing(label: &str, listing: &MangaListing) {
    println!(
        "{label}: {} titles (more: {}), first {:?}",
        listing.items.len(),
        listing.has_next_page,
        listing
            .items
            .iter()
            .take(4)
            .map(|item| (&item.id, &item.title))
            .collect::<Vec<_>>()
    );
    assert!(!listing.items.is_empty(), "{label} is empty");
    let mut ids: Vec<&str> = listing.items.iter().map(|item| item.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), listing.items.len(), "{label} repeats a title");
}

fn has(listing: &MangaListing, id: &str) -> bool {
    listing.items.iter().any(|item| item.id == id)
}

fn print_chapters(label: &str, chapters: &[MangaChapter]) {
    println!(
        "{label}: {} chapters, first {:?}, last {:?}",
        chapters.len(),
        chapters
            .iter()
            .take(4)
            .map(|chapter| &chapter.name)
            .collect::<Vec<_>>(),
        chapters.last().map(|chapter| &chapter.name)
    );
}

fn chapter<'a>(chapters: &'a [MangaChapter], name: &str) -> &'a MangaChapter {
    chapters
        .iter()
        .find(|chapter| chapter.name.contains(name))
        .unwrap_or_else(|| panic!("no chapter named {name:?}"))
}

fn paragraphs(blocks: &[PassageBlock]) -> usize {
    blocks
        .iter()
        .filter(|block| matches!(block, PassageBlock::Paragraph(text) if text.len() > 40))
        .count()
}

fn text_of(blocks: &[PassageBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            PassageBlock::Paragraph(text) | PassageBlock::Heading(text) => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A chapter as Raygun keeps it, and the HTML the source answered (maths and code are checked
/// there: the reader's blocks are text).
async fn read(
    host: &ExtensionHost,
    provider: &str,
    chapter: &MangaChapter,
) -> (Vec<PassageBlock>, String) {
    let blocks = host.novel_passage(provider, &chapter.id).await.unwrap();
    let answer = host
        .call(
            provider,
            "passage",
            serde_json::json!({ "chapterId": chapter.id }),
        )
        .await
        .unwrap();
    let html = answer["html"].as_str().unwrap_or_default().to_owned();
    println!(
        "{:?}: {} blocks ({} paragraphs), {} bytes of HTML, first {:?}",
        chapter.name,
        blocks.len(),
        paragraphs(&blocks),
        html.len(),
        blocks.first()
    );
    (blocks, html)
}

#[tokio::test]
#[ignore = "needs network access to en.wikibooks.org and wikimedia.org"]
async fn open_books_live_wikibooks() {
    let host = host().await;
    assert!(
        host.providers(NOVEL_SOURCE)
            .await
            .iter()
            .any(|provider| provider.id == WIKIBOOKS)
    );
    let popular = host
        .manga_list(WIKIBOOKS, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("most read last month", &popular);
    assert!(popular.has_next_page);
    assert!(popular.items.iter().all(|item| !item.id.contains('/')));
    let second = host
        .manga_list(WIKIBOOKS, MangaList::Popular, 2)
        .await
        .unwrap();
    assert_listing("most read, page 2", &second);
    assert!(
        second.items.iter().all(|item| !has(&popular, &item.id)),
        "page 2 repeats page 1"
    );
    let latest = host
        .manga_list(WIKIBOOKS, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_listing("recently completed", &latest);
    let found = host.manga_search(WIKIBOOKS, "python", 1).await.unwrap();
    assert_listing("search python", &found);
    assert!(has(&found, "Python Programming"));
    assert!(
        found.items.iter().all(|item| !item.id.contains('/')),
        "books, not their pages"
    );
    assert_eq!(host.manga_filters(WIKIBOOKS).await.unwrap().len(), 3);
    let featured = host
        .manga_search_filtered(
            WIKIBOOKS,
            "",
            1,
            &filters(&[("subject", "Mathematics"), ("status", "Featured books")]),
        )
        .await
        .unwrap();
    assert_listing("featured mathematics", &featured);
    assert!(has(&featured, "Linear Algebra"));

    let details = host
        .manga_details(WIKIBOOKS, "Linear Algebra")
        .await
        .unwrap();
    println!(
        "details: {:?}, {:?}, {}, {:?}",
        details.title, details.genres, details.status, details.description
    );
    assert_eq!(details.title, "Linear Algebra");
    assert!(
        details
            .description
            .as_deref()
            .is_some_and(|text| text.contains("Licence: CC BY-SA 4.0"))
    );
    // A saved collection, with its chapters' headings.
    let chapters = host
        .manga_chapters(WIKIBOOKS, "Linear Algebra")
        .await
        .unwrap();
    print_chapters("Linear Algebra", &chapters);
    assert!(chapters.len() >= 50);
    assert!(
        chapters
            .iter()
            .any(|chapter| chapter.name.starts_with("Chapter I · "))
    );
    let (blocks, html) = read(&host, WIKIBOOKS, chapter(&chapters, "Gauss")).await;
    assert!(paragraphs(&blocks) > 10);
    assert!(html.contains("<math"), "formulas stay MathML");
    assert!(!html.contains("noprint"), "navigation is left out");

    // No collection: the main page's links, its own text first.
    let haskell = host.manga_chapters(WIKIBOOKS, "Haskell").await.unwrap();
    print_chapters("Haskell", &haskell);
    assert!(haskell.len() > 40);
    assert!(
        haskell
            .iter()
            .all(|chapter| !chapter.name.to_lowercase().contains("print"))
    );
    let (blocks, html) = read(&host, WIKIBOOKS, chapter(&haskell, "Recursion")).await;
    assert!(paragraphs(&blocks) > 10);
    assert!(html.contains("<pre"), "code blocks stay");
}

#[tokio::test]
#[ignore = "needs network access to opentextbc.ca"]
async fn open_books_live_bccampus() {
    let host = host().await;
    let popular = host
        .manga_list(BCCAMPUS, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("catalogue", &popular);
    assert_eq!(popular.items.len(), 30);
    assert!(popular.has_next_page);
    assert!(popular.items.iter().any(|item| item.cover_url.is_some()));
    assert!(popular.items.iter().any(|item| !item.authors.is_empty()));
    let latest = host
        .manga_list(BCCAMPUS, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_listing("newest", &latest);
    let found = host.manga_search(BCCAMPUS, "psychology", 1).await.unwrap();
    assert_listing("search psychology", &found);
    assert!(has(&found, "introductiontopsychology"));
    let algebra = host
        .manga_search_filtered(
            BCCAMPUS,
            "",
            1,
            &filters(&[("subject", "P"), ("licence", "CC BY-NC-SA")]),
        )
        .await
        .unwrap();
    assert_listing("mathematics and science, CC BY-NC-SA", &algebra);
    assert!(has(&algebra, "intermediatealgebraberg"));

    let details = host
        .manga_details(BCCAMPUS, "intermediatealgebraberg")
        .await
        .unwrap();
    println!(
        "details: {:?} by {:?}, {:?}",
        details.title, details.authors, details.description
    );
    assert_eq!(details.title, "Intermediate Algebra");
    assert!(!details.authors.is_empty());
    assert!(
        details
            .description
            .as_deref()
            .is_some_and(|text| text.contains("Licence: CC BY-NC-SA 4.0"))
    );
    let chapters = host
        .manga_chapters(BCCAMPUS, "intermediatealgebraberg")
        .await
        .unwrap();
    print_chapters("Intermediate Algebra", &chapters);
    assert!(chapters.len() > 100);
    assert!(
        chapters
            .iter()
            .all(|chapter| !chapter.name.contains("Accessibility Statement"))
    );
    let (blocks, html) = read(&host, BCCAMPUS, chapter(&chapters, "1.2 Fractions")).await;
    assert!(paragraphs(&blocks) > 5);
    assert!(html.contains("\\("), "LaTeX in the usual delimiters");
    assert!(!html.contains("[latex]"));
}

#[tokio::test]
#[ignore = "needs network access to www.gnu.org"]
async fn open_books_live_gnu_manuals() {
    let host = host().await;
    let popular = host
        .manga_list(GNU_MANUALS, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("best known", &popular);
    assert_eq!(popular.items[0].id, "bash");
    assert!(popular.has_next_page);
    let found = host.manga_search(GNU_MANUALS, "awk", 1).await.unwrap();
    assert_listing("search awk", &found);
    assert!(has(&found, "gawk"));
    let editors = host
        .manga_search_filtered(GNU_MANUALS, "", 1, &filters(&[("category", "Editors")]))
        .await
        .unwrap();
    assert_listing("editors", &editors);
    assert!(has(&editors, "emacs-emacs"));

    let details = host.manga_details(GNU_MANUALS, "gzip").await.unwrap();
    println!("details: {:?}, {:?}", details.title, details.description);
    assert!(details.description.as_deref().is_some_and(|text| {
        text.contains("Licence: GNU Free Documentation License 1.3 or later")
    }));
    let chapters = host.manga_chapters(GNU_MANUALS, "gzip").await.unwrap();
    print_chapters("gzip", &chapters);
    assert!(chapters.len() >= 7);
    let (blocks, html) = read(&host, GNU_MANUALS, chapter(&chapters, "Invoking gzip")).await;
    assert!(paragraphs(&blocks) > 5);
    assert!(html.contains("<pre"), "examples stay");
    assert!(
        !text_of(&blocks).contains("Next:"),
        "navigation is left out"
    );

    // A manual on one page: each chapter is its part of the page.
    let emms = host.manga_chapters(GNU_MANUALS, "emms").await.unwrap();
    print_chapters("EMMS", &emms);
    assert!(emms.len() > 30);
    let (blocks, _) = read(&host, GNU_MANUALS, &emms[1]).await;
    assert!(!blocks.is_empty());
    assert!(blocks.len() < 200, "one part, not the whole manual");

    // Texinfo 4's contents list its sections without names.
    let sharutils = host.manga_chapters(GNU_MANUALS, "sharutils").await.unwrap();
    print_chapters("sharutils", &sharutils);
    assert!(sharutils.len() > 20);
}

#[tokio::test]
#[ignore = "needs network access to greenteapress.com and two GitHub Pages sites"]
async fn open_books_live_green_tea_press() {
    let host = host().await;
    let popular = host
        .manga_list(GREEN_TEA_PRESS, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("books", &popular);
    assert!(popular.items.iter().any(|item| item.cover_url.is_some()));
    assert!(popular.items.iter().all(|item| !item.authors.is_empty()));
    let found = host
        .manga_search(GREEN_TEA_PRESS, "bayes", 1)
        .await
        .unwrap();
    assert_listing("search bayes", &found);
    assert!(has(&found, "think-bayes-2e"));

    let details = host
        .manga_details(GREEN_TEA_PRESS, "think-python-3e")
        .await
        .unwrap();
    println!("details: {:?}, {:?}", details.title, details.description);
    assert!(
        details
            .description
            .as_deref()
            .is_some_and(|text| text.contains("Licence: CC BY-NC-SA 4.0"))
    );

    // A Jupyter Book: code, its output and formulas stay; notices and licence lines don't.
    let python = host
        .manga_chapters(GREEN_TEA_PRESS, "think-python-3e")
        .await
        .unwrap();
    print_chapters("Think Python", &python);
    assert!(python.len() >= 19);
    let (blocks, html) = read(&host, GREEN_TEA_PRESS, chapter(&python, "2. Variables")).await;
    assert!(paragraphs(&blocks) > 10);
    assert!(html.contains("<pre"));
    let text = text_of(&blocks);
    for left_out in ["Colab", "Copyright 2024", "Bookshop", "Show code cell"] {
        assert!(!text.contains(left_out), "{left_out:?} is left out");
    }
    let bayes = host
        .manga_chapters(GREEN_TEA_PRESS, "think-bayes-2e")
        .await
        .unwrap();
    let (_, html) = read(&host, GREEN_TEA_PRESS, chapter(&bayes, "2. Bayes")).await;
    assert!(html.contains("\\["), "MathJax's formulas stay");

    // HeVeA's HTML, in both of its layouts.
    for id in ["think-os", "think-python-2e"] {
        let chapters = host.manga_chapters(GREEN_TEA_PRESS, id).await.unwrap();
        print_chapters(id, &chapters);
        assert!(chapters.len() > 10);
        let (blocks, _) = read(&host, GREEN_TEA_PRESS, &chapters[2]).await;
        assert!(paragraphs(&blocks) > 10);
        let text = text_of(&blocks);
        assert!(!text.contains("PDF version") && !text.contains("Buy this book"));
    }

    // Quarto (Think Java).
    let java = host
        .manga_chapters(GREEN_TEA_PRESS, "think-java-2e")
        .await
        .unwrap();
    print_chapters("Think Java", &java);
    let (blocks, html) = read(&host, GREEN_TEA_PRESS, &java[2]).await;
    assert!(paragraphs(&blocks) > 10);
    assert!(html.contains("<pre"));
    assert!(!text_of(&blocks).contains("Notices"));
}

#[tokio::test]
#[ignore = "needs network access to saylordotorg.github.io"]
async fn open_books_live_saylor() {
    let host = host().await;
    let popular = host
        .manga_list(SAYLOR, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("textbooks", &popular);
    assert!(popular.has_next_page);
    let found = host.manga_search(SAYLOR, "chemistry", 1).await.unwrap();
    assert_listing("search chemistry", &found);
    assert!(has(&found, "introductory-chemistry"));

    let details = host
        .manga_details(SAYLOR, "introductory-statistics")
        .await
        .unwrap();
    println!("details: {:?}, {:?}", details.title, details.description);
    assert!(
        details
            .description
            .as_deref()
            .is_some_and(|text| text.contains("Licence: CC BY-NC-SA 3.0"))
    );
    let chapters = host
        .manga_chapters(SAYLOR, "introductory-statistics")
        .await
        .unwrap();
    print_chapters("Introductory Statistics", &chapters);
    assert!(chapters.len() > 10);
    assert!(
        chapters
            .iter()
            .all(|chapter| !chapter.name.contains("Licensing"))
    );
    let (blocks, html) = read(
        &host,
        SAYLOR,
        chapter(&chapters, "Chapter 2: Descriptive Statistics"),
    )
    .await;
    assert!(paragraphs(&blocks) > 50, "a chapter holds its sections");
    assert!(html.contains("<math"), "formulas stay MathML");

    // Footnotes become notes at the end; margin definitions are left out.
    let business = host
        .manga_chapters(SAYLOR, "exploring-business")
        .await
        .unwrap();
    let (blocks, html) = read(&host, SAYLOR, chapter(&business, "Chapter 1:")).await;
    assert!(paragraphs(&blocks) > 50);
    assert!(html.contains("<sup>[1]</sup>") && html.contains("<h3>Notes</h3>"));
    assert!(!html.contains("glossdef"));
}
