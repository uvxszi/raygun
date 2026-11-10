use std::io::{Cursor, Read};

use raygun_model::TextBlock;

use super::book::{Book, BookChapter, text, write_epub};
use super::cbz::{self, ComicInfo};
use super::pdf::PdfWriter;

/// A tiny valid JPEG frame header: enough for the PDF writer, which passes the bytes through.
fn jpeg(width: u16, height: u16) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00];
    bytes.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08]);
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&[0x03, 0x01, 0x11, 0x00]);
    bytes.extend_from_slice(&[0xFF, 0xD9]);
    bytes
}

fn png_rgba(width: u32, height: u32, alpha: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        let pixels: Vec<u8> = (0..width * height)
            .flat_map(|_| [200, 10, 10, alpha])
            .collect();
        writer.write_image_data(&pixels).unwrap();
    }
    bytes
}

#[test]
fn cbz_files_carry_their_pages_and_comic_info() {
    let dir = tempfile::tempdir().unwrap();
    let pages: Vec<_> = (1..=3)
        .map(|index| {
            let path = dir.path().join(format!("{index:03}.jpg"));
            std::fs::write(&path, jpeg(10, 10)).unwrap();
            path
        })
        .collect();
    let info = ComicInfo {
        series: "Frieren & Co".into(),
        title: "Chapter 12.5".into(),
        number: Some(12.5),
        summary: Some("An elf <mage>".into()),
        writers: vec!["Yamada".into()],
        pencillers: vec!["Abe".into()],
        translator: Some("Group".into()),
        genres: vec!["Fantasy".into(), "Drama".into()],
        ..ComicInfo::default()
    };
    let path = dir.path().join("out.cbz");
    let mut file = std::fs::File::options()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    cbz::write(&mut file, &pages, &info).unwrap();
    drop(file);

    let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    let names: Vec<String> = archive
        .file_names()
        .map(|name| name.unwrap().into_owned())
        .collect();
    assert!(names.contains(&"0001.jpg".to_owned()) && names.contains(&"0003.jpg".to_owned()));
    let mut xml = String::new();
    archive
        .by_name("ComicInfo.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    // Raygun's own import reads the file back.
    let read = crate::comics::ComicInfo::parse(&xml);
    assert_eq!(read.series.as_deref(), Some("Frieren & Co"));
    assert_eq!(read.number, Some(12.5));
    assert_eq!(read.summary.as_deref(), Some("An elf <mage>"));
    assert_eq!(read.genre.as_deref(), Some("Fantasy, Drama"));
    assert!(xml.contains("<PageCount>3</PageCount>"));
    assert_eq!(
        archive.by_name("0001.jpg").unwrap().compression(),
        zip::CompressionMethod::Stored
    );
}

#[test]
fn pdfs_hold_jpeg_png_and_split_tall_pages() {
    let mut pdf = PdfWriter::new(Vec::new()).unwrap();
    pdf.bookmark("Chapter 1");
    assert!(pdf.add_image(&jpeg(800, 1200)).unwrap());
    assert!(pdf.add_image(&png_rgba(4, 4, 128)).unwrap());
    pdf.bookmark("Chapter 2");
    // A webtoon strip taller than a PDF page may be.
    assert!(pdf.add_image(&jpeg(800, 30_000)).unwrap());
    assert!(!pdf.add_image(b"GIF89a....").unwrap());
    assert_eq!(pdf.skipped(), 1);
    let bytes = pdf.finish("Frieren").unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.starts_with("%PDF-1.7"));
    assert!(text.contains("/Count 5"));
    assert!(text.contains("/Filter /DCTDecode"));
    assert!(
        text.contains("/SMask"),
        "the half-transparent PNG keeps its alpha"
    );
    assert!(text.contains("/Type /Outlines") && text.contains("/Count 2 >>"));
    assert!(text.contains("/MediaBox [0 0 800.00 14400.00]"));
    assert!(text.trim_end().ends_with("%%EOF"));

    // Every cross-reference offset points at its object.
    let xref = text.rfind("\nxref\n").unwrap() + 1;
    let table = &text[xref..];
    for (number, line) in table
        .lines()
        .skip(3)
        .take_while(|line| line.ends_with(" n"))
        .enumerate()
    {
        let offset: usize = line[..10].parse().unwrap();
        assert!(
            text[offset..].starts_with(&format!("{} 0 obj", number + 1)),
            "object {} isn't at {offset}",
            number + 1
        );
    }

    // An opaque PNG has no mask.
    let mut opaque = PdfWriter::new(Vec::new()).unwrap();
    opaque.add_image(&png_rgba(2, 2, 255)).unwrap();
    let bytes = opaque.finish("x").unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("/SMask"));
    assert!(PdfWriter::new(Vec::new()).unwrap().finish("empty").is_err());
}

fn book() -> Book {
    Book {
        id: "urn:raygun:t1".into(),
        title: "Frankenstein".into(),
        authors: vec!["Mary Shelley".into()],
        description: Some("A <monster> story".into()),
        language: "und".into(),
        genres: vec!["Gothic".into()],
        cover: Some((vec![0xFF, 0xD8, 0xFF], "jpg".into())),
        chapters: vec![
            BookChapter {
                title: "Letter 1".into(),
                blocks: vec![
                    TextBlock::Heading {
                        text: "To Mrs. Saville".into(),
                    },
                    TextBlock::Paragraph {
                        text: "You will rejoice & be glad.".into(),
                    },
                    TextBlock::Separator,
                    TextBlock::Image {
                        url: "http://127.0.0.1/x".into(),
                    },
                ],
            },
            BookChapter {
                title: "Letter 2".into(),
                blocks: vec![
                    // Repeats the chapter's title: left out.
                    TextBlock::Heading {
                        text: "letter 2".into(),
                    },
                    TextBlock::Paragraph {
                        text: "How slowly the time passes".into(),
                    },
                ],
            },
        ],
        modified: "2026-11-01T12:00:00Z".into(),
    }
}

#[test]
fn epubs_are_well_formed_books() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("book.epub");
    let mut file = std::fs::File::create(&path).unwrap();
    write_epub(&mut file, &book()).unwrap();
    drop(file);

    let bytes = std::fs::read(&path).unwrap();
    // The mimetype is the first entry, stored, so readers can sniff it at a fixed offset.
    assert_eq!(&bytes[30..38], b"mimetype");
    assert_eq!(&bytes[38..58], b"application/epub+zip");
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut read = |name: &str| {
        let mut text = String::new();
        archive
            .by_name(name)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        text
    };
    let opf = read("OEBPS/content.opf");
    assert!(opf.contains("<dc:title>Frankenstein</dc:title>"));
    assert!(opf.contains("properties=\"cover-image\"") && !opf.contains("cover.xhtml"));
    assert!(opf.contains("<itemref idref=\"c2\"/>"));
    assert!(opf.contains("dcterms:modified"));
    let chapter = read("OEBPS/chapters/0001.xhtml");
    assert!(chapter.contains("<p>You will rejoice &amp; be glad.</p>"));
    assert!(chapter.contains("<hr/>") && !chapter.contains("<img"));
    assert!(read("OEBPS/nav.xhtml").contains("Letter 2"));
    assert!(!read("OEBPS/chapters/0002.xhtml").contains("<h3>"));
    assert!(read("OEBPS/toc.ncx").contains("playOrder=\"2\""));
    // Raygun's own EPUB import reads it back.
    let imported = crate::epub::Epub::open(&std::fs::read(&path).unwrap())
        .and_then(|epub| epub.book())
        .unwrap();
    assert_eq!(imported.title, "Frankenstein");
    assert_eq!(imported.authors, vec!["Mary Shelley".to_owned()]);
    assert_eq!(
        imported
            .chapters
            .iter()
            .map(|chapter| chapter.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Letter 1", "Letter 2"]
    );

    let plain = text(&book());
    assert!(plain.starts_with("Frankenstein\nMary Shelley\n"));
    assert!(plain.contains("\n\nLetter 2\n\nHow slowly the time passes\n"));
    assert!(plain.contains("* * *"));
}

#[test]
fn a_folder_chosen_for_the_export_wins_over_the_settings_one() {
    use raygun_model::{
        ExportDestination, ExportFolder, ExportFormat, ExportRequest, ExportScope, SavePrefs,
    };

    use super::output::Place;
    use super::place_for_request;

    let picked = ExportFolder {
        uri: "content://picked".into(),
        name: "Picked".into(),
    };
    let prefs = SavePrefs {
        folder_uri: Some("content://settings".into()),
        folder_name: Some("Comics".into()),
        ..SavePrefs::default()
    };
    let mut request = ExportRequest {
        medium: "manga".into(),
        title_ids: vec!["t".into()],
        scope: ExportScope::All,
        format: ExportFormat::Cbz,
        destination: ExportDestination::Folder,
        folder: Some(picked.clone()),
    };
    assert!(matches!(
        place_for_request(&prefs, &request),
        Ok(Place::Folder { uri, name }) if uri == "content://picked" && name == "Picked"
    ));
    request.folder = None;
    assert!(matches!(
        place_for_request(&prefs, &request),
        Ok(Place::Folder { uri, .. }) if uri == "content://settings"
    ));
    assert!(place_for_request(&SavePrefs::default(), &request).is_err());
    request.destination = ExportDestination::Downloads;
    request.folder = Some(picked);
    assert!(matches!(
        place_for_request(&prefs, &request),
        Ok(Place::Downloads)
    ));
}

#[test]
fn a_cancelled_export_reads_cancelled_at_once_and_stays_so() {
    use raygun_model::{ExportDestination, ExportFormat, ExportJob, ExportState};
    let mut job = ExportJob {
        id: "j".into(),
        label: "Show".into(),
        medium: "video".into(),
        format: ExportFormat::Video,
        destination: ExportDestination::Downloads,
        state: ExportState::Downloading,
        downloaded: 0,
        to_download: 3,
        written: 0,
        to_write: 3,
        files: Vec::new(),
        failed_chapters: 0,
        skipped_pages: 0,
        message: Some("Downloads are paused".into()),
        started_at: 0.0,
        progress: None,
    };
    assert!(super::cancel_job(&mut job));
    assert_eq!(job.state, ExportState::Cancelled);
    assert_eq!(job.message, None);
    // Work still under way reports once more before it sees the flag: it stays cancelled.
    super::change_job(&mut job, true, |job| {
        job.state = ExportState::Writing;
        job.downloaded = 1;
    });
    assert_eq!((job.state, job.downloaded), (ExportState::Cancelled, 1));
    assert!(
        !super::cancel_job(&mut job),
        "a finished job isn't cancelled again"
    );
}
