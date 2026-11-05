//! Android's fonts: the named families of the system's font configuration (`font_fallback.xml`
//! since Android 15, `fonts.xml` before it, a maker's own in `fonts_customization.xml`), each
//! called by its regular file's own name (`serif` is Noto Serif, `casual` Coming Soon) and served
//! file by file at the stream proxy's `/font/` route. Unnamed families are fallbacks for scripts
//! and emoji, left out.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use raygun_model::{SystemFont, SystemFontFace};
use raygun_net::FontLinks;

/// A maker's own named families first, then the system's; the first file of a family wins.
const CONFIGS: [(&str, &str); 3] = [
    ("/product/etc/fonts_customization.xml", "/product/fonts"),
    ("/system/etc/font_fallback.xml", "/system/fonts"),
    ("/system/etc/fonts.xml", "/system/fonts"),
];

pub(super) fn installed(links: &FontLinks) -> Vec<SystemFont> {
    let configs: Vec<_> = CONFIGS
        .iter()
        .filter_map(|(file, folder)| {
            let text = std::fs::read_to_string(file).ok()?;
            Some((parse_fonts_xml(&text), PathBuf::from(folder)))
        })
        .collect();
    named_families(&configs, read_file, |path| links.link(path))
}

/// What a font file says about itself.
#[derive(Debug, Clone, PartialEq)]
struct FileFacts {
    family: String,
    latin: bool,
    /// A variable font's weight axis.
    weights: Option<(u16, u16)>,
}

fn read_file(path: &Path) -> Option<FileFacts> {
    let data = std::fs::read(path).ok()?;
    let face = ttf_parser::Face::parse(&data, 0).ok()?;
    Some(FileFacts {
        family: family_name(&face)?,
        latin: super::draws_latin(&face),
        weights: weight_axis(&face),
    })
}

/// A face's family as `font-family` names it: the typographic family (name ID 16) when there is
/// one, else the family (ID 1), in US English when the font has it.
fn family_name(face: &ttf_parser::Face<'_>) -> Option<String> {
    use ttf_parser::{Language, name_id};
    let names: Vec<_> = face
        .names()
        .into_iter()
        .filter(|name| name.is_unicode())
        .collect();
    [name_id::TYPOGRAPHIC_FAMILY, name_id::FAMILY]
        .into_iter()
        .find_map(|id| {
            let of_id: Vec<_> = names.iter().filter(|name| name.name_id == id).collect();
            of_id
                .iter()
                .find(|name| name.language() == Language::English_UnitedStates)
                .or_else(|| of_id.first())
                .and_then(|name| name.to_string())
                .map(|name| name.trim().to_owned())
                .filter(|name| !name.is_empty())
        })
}

/// The weights a variable face draws: its weight axis.
fn weight_axis(face: &ttf_parser::Face<'_>) -> Option<(u16, u16)> {
    let weight = |value: f32| value.round().clamp(1.0, 1000.0) as u16;
    face.variation_axes()
        .into_iter()
        .find(|axis| axis.tag == ttf_parser::Tag::from_bytes(b"wght"))
        .map(|axis| (weight(axis.min_value), weight(axis.max_value)))
}

/// A family of a font configuration file: named ones are what apps ask for by name
/// (`sans-serif`, `serif`, `casual`…), unnamed ones are fallbacks for scripts and emoji.
#[derive(Debug, Default, PartialEq)]
struct XmlFamily {
    name: Option<String>,
    fonts: Vec<XmlFont>,
}

#[derive(Debug, PartialEq)]
struct XmlFont {
    /// A file name in the configuration's font folder.
    file: String,
    weight: u16,
    italic: bool,
    /// The face in a collection (`.ttc`).
    index: u32,
}

fn attribute(element: &BytesStart<'_>, key: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.as_ref() == key)
        .and_then(|attribute| {
            attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(|value| value.trim().to_owned())
        })
        .filter(|value| !value.is_empty())
}

/// The families of an Android font configuration (`fonts.xml` and the files like it), as far as
/// it reads. A `<family-list>` names the families inside it.
fn parse_fonts_xml(text: &str) -> Vec<XmlFamily> {
    let mut reader = Reader::from_str(text);
    let mut families = Vec::new();
    let mut list_names: Vec<Option<String>> = Vec::new();
    let mut family: Option<XmlFamily> = None;
    let mut font: Option<XmlFont> = None;
    loop {
        let event = match reader.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(event) => event,
        };
        match event {
            Event::Start(element) => match element.local_name().as_ref() {
                "family-list" => list_names.push(attribute(&element, "name")),
                "family" => {
                    let name = attribute(&element, "name")
                        .or_else(|| list_names.iter().rev().find_map(Clone::clone));
                    family = Some(XmlFamily {
                        name,
                        fonts: Vec::new(),
                    });
                }
                "font" => {
                    font = Some(XmlFont {
                        file: String::new(),
                        weight: attribute(&element, "weight")
                            .and_then(|weight| weight.parse().ok())
                            .unwrap_or(400),
                        italic: attribute(&element, "style").as_deref() == Some("italic"),
                        index: attribute(&element, "index")
                            .and_then(|index| index.parse().ok())
                            .unwrap_or(0),
                    });
                }
                _ => {}
            },
            // The file name comes first; the axes after it are elements of their own.
            Event::Text(content) => {
                if let Some(font) = font.as_mut() {
                    font.file.push_str(&content);
                }
            }
            Event::End(element) => match element.local_name().as_ref() {
                "font" => {
                    if let (Some(mut done), Some(family)) = (font.take(), family.as_mut()) {
                        done.file = done.file.trim().to_owned();
                        if !done.file.is_empty() {
                            family.fonts.push(done);
                        }
                    }
                }
                "family" => families.extend(family.take()),
                "family-list" => {
                    list_names.pop();
                }
                _ => {}
            },
            _ => {}
        }
    }
    families
}

/// The named families of font configurations (each with the folder its files are in), by their
/// files' own names, with a face per file and style. Families whose files are unreadable, that
/// draw no Latin letters or that repeat a family already listed (`sans-serif-condensed` is
/// Roboto again) are left out.
fn named_families(
    configs: &[(Vec<XmlFamily>, PathBuf)],
    read: impl Fn(&Path) -> Option<FileFacts>,
    link: impl Fn(&Path) -> String,
) -> Vec<SystemFont> {
    let mut known: HashMap<PathBuf, Option<FileFacts>> = HashMap::new();
    let mut facts = |path: &Path| {
        known
            .entry(path.to_path_buf())
            .or_insert_with(|| read(path))
            .clone()
    };
    let mut fonts = BTreeMap::new();
    for (families, folder) in configs {
        for family in families {
            if family.name.is_none() {
                continue;
            }
            // The weights each file is listed at, upright and italic apart. A collection's later
            // faces can't be picked with `@font-face`, and only plain names are files of the
            // folder.
            let mut files: BTreeMap<(&str, bool), (u16, u16)> = BTreeMap::new();
            for font in &family.fonts {
                let plain = Path::new(&font.file).file_name() == Some(font.file.as_ref());
                if font.index != 0 || !plain {
                    continue;
                }
                let range = files
                    .entry((font.file.as_str(), font.italic))
                    .or_insert((font.weight, font.weight));
                *range = (range.0.min(font.weight), range.1.max(font.weight));
            }
            // Named by its regular face: the upright file nearest weight 400.
            let regular = files
                .iter()
                .filter(|((_, italic), _)| !italic)
                .min_by_key(|(_, (low, high))| {
                    if (*low..=*high).contains(&400) {
                        0
                    } else {
                        low.abs_diff(400).min(high.abs_diff(400))
                    }
                })
                .map(|((file, _), _)| *file);
            let Some(own) = regular.and_then(|file| facts(&folder.join(file))) else {
                continue;
            };
            let key = own.family.to_lowercase();
            if !own.latin || fonts.contains_key(&key) {
                continue;
            }
            let mut faces: Vec<SystemFontFace> = files
                .into_iter()
                .filter_map(|((file, italic), (low, high))| {
                    let path = folder.join(file);
                    // A variable file draws its whole weight axis.
                    let (low, high) = match facts(&path)?.weights {
                        Some((axis_low, axis_high)) => (low.min(axis_low), high.max(axis_high)),
                        None => (low, high),
                    };
                    Some(SystemFontFace {
                        url: link(&path),
                        weight_min: low,
                        weight_max: high,
                        italic,
                    })
                })
                .collect();
            faces.sort_by_key(|face| (face.italic, face.weight_min));
            fonts.insert(
                key,
                SystemFont {
                    family: own.family,
                    faces,
                },
            );
        }
    }
    fonts.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shaped like AOSP's `fonts.xml`: named families (one variable file listed at several
    /// weights, one of static files), aliases, script fallbacks and a collection.
    const FONTS_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<!-- first font is default -->
<familyset version="23">
    <family name="sans-serif" supportedAxes="wght,ital">
        <font weight="100" style="normal">Roboto-Regular.ttf
            <axis tag="wdth" stylevalue="100" />
            <axis tag="wght" stylevalue="100" />
        </font>
        <font weight="400" style="normal">Roboto-Regular.ttf
            <axis tag="wdth" stylevalue="100" />
            <axis tag="wght" stylevalue="400" />
        </font>
        <font weight="900" style="normal">Roboto-Regular.ttf
            <axis tag="wdth" stylevalue="100" />
            <axis tag="wght" stylevalue="900" />
        </font>
        <font weight="400" style="italic">Roboto-Italic.ttf
            <axis tag="wdth" stylevalue="100" />
            <axis tag="wght" stylevalue="400" />
        </font>
    </family>
    <alias name="sans-serif-thin" to="sans-serif" weight="100" />
    <alias name="arial" to="sans-serif" />
    <family name="sans-serif-condensed">
        <font weight="400" style="normal">Roboto-Regular.ttf
            <axis tag="wdth" stylevalue="75" />
        </font>
    </family>
    <family name="serif">
        <font weight="400" style="normal" postScriptName="NotoSerif">NotoSerif-Regular.ttf</font>
        <font weight="700" style="normal">NotoSerif-Bold.ttf</font>
        <font weight="400" style="italic">NotoSerif-Italic.ttf</font>
        <font weight="700" style="italic">NotoSerif-BoldItalic.ttf</font>
    </family>
    <alias name="times new roman" to="serif" />
    <family name="casual">
        <font weight="400" style="normal">ComingSoon.ttf</font>
    </family>
    <family name="symbols">
        <font weight="400" style="normal">AndroidClock.ttf</font>
    </family>
    <family lang="und-Arab" variant="elegant">
        <font weight="400" style="normal" postScriptName="NotoNaskhArabic">
            NotoNaskhArabic-Regular.ttf
        </font>
    </family>
    <family lang="zh-Hans">
        <font weight="400" style="normal" index="2" postScriptName="NotoSansCJKjp-Regular">
            NotoSansCJK-Regular.ttc
        </font>
    </family>
    <family-list name="cursive">
        <family>
            <font weight="400" style="normal">DancingScript-Regular.ttf</font>
        </family>
    </family-list>
</familyset>
"#;

    fn font(file: &str, weight: u16, italic: bool) -> XmlFont {
        XmlFont {
            file: file.into(),
            weight,
            italic,
            index: 0,
        }
    }

    #[test]
    fn reads_named_families_and_fallbacks_from_fonts_xml() {
        let families = parse_fonts_xml(FONTS_XML);
        let names: Vec<_> = families
            .iter()
            .map(|family| family.name.as_deref())
            .collect();
        assert_eq!(
            names,
            [
                Some("sans-serif"),
                Some("sans-serif-condensed"),
                Some("serif"),
                Some("casual"),
                Some("symbols"),
                None,
                None,
                Some("cursive"),
            ],
            "aliases aren't families; a family list names its families"
        );
        assert_eq!(
            families[0].fonts,
            [
                font("Roboto-Regular.ttf", 100, false),
                font("Roboto-Regular.ttf", 400, false),
                font("Roboto-Regular.ttf", 900, false),
                font("Roboto-Italic.ttf", 400, true),
            ],
            "the file name, without the axes after it"
        );
        assert_eq!(families[2].fonts[1], font("NotoSerif-Bold.ttf", 700, false));
        assert_eq!(
            families[6].fonts,
            [XmlFont {
                file: "NotoSansCJK-Regular.ttc".into(),
                weight: 400,
                italic: false,
                index: 2,
            }]
        );
    }

    #[test]
    fn a_broken_file_gives_what_was_read() {
        let families = parse_fonts_xml(
            r#"<familyset><family name="serif"><font>Serif.ttf</font></family><family"#,
        );
        assert_eq!(families.len(), 1);
        assert_eq!(families[0].fonts, [font("Serif.ttf", 400, false)]);
        assert!(parse_fonts_xml("not xml at all").is_empty());
    }

    /// Font files as an Android system has them, by name.
    fn facts(path: &Path) -> Option<FileFacts> {
        let name = path.file_name()?.to_str()?;
        let (family, latin, weights) = match name {
            "Roboto-Regular.ttf" | "Roboto-Italic.ttf" => ("Roboto", true, Some((100, 900))),
            name if name.starts_with("NotoSerif-") => ("Noto Serif", true, None),
            "ComingSoon.ttf" => ("Coming Soon", true, None),
            "AndroidClock.ttf" => ("Android Clock", false, None),
            "DancingScript-Regular.ttf" => ("Dancing Script", true, None),
            "GoogleSans-Regular.ttf" => ("Google Sans", true, None),
            _ => return None,
        };
        Some(FileFacts {
            family: family.into(),
            latin,
            weights,
        })
    }

    fn link(path: &Path) -> String {
        format!("font:{}", path.display()).replace('\\', "/")
    }

    #[test]
    fn named_families_are_listed_by_their_files_names_with_a_face_per_file() {
        let configs = vec![(parse_fonts_xml(FONTS_XML), PathBuf::from("/system/fonts"))];
        let fonts = named_families(&configs, facts, link);
        let names: Vec<_> = fonts.iter().map(|font| font.family.as_str()).collect();
        assert_eq!(
            names,
            ["Coming Soon", "Dancing Script", "Noto Serif", "Roboto"],
            "no fallbacks, no repeats, nothing without Latin letters, by name"
        );

        assert_eq!(
            fonts[3].faces,
            [
                SystemFontFace {
                    url: "font:/system/fonts/Roboto-Regular.ttf".into(),
                    weight_min: 100,
                    weight_max: 900,
                    italic: false,
                },
                SystemFontFace {
                    url: "font:/system/fonts/Roboto-Italic.ttf".into(),
                    weight_min: 100,
                    weight_max: 900,
                    italic: true,
                },
            ],
            "a variable file draws its whole weight axis"
        );
        let serif: Vec<_> = fonts[2]
            .faces
            .iter()
            .map(|face| (face.weight_min, face.weight_max, face.italic))
            .collect();
        assert_eq!(
            serif,
            [
                (400, 400, false),
                (700, 700, false),
                (400, 400, true),
                (700, 700, true)
            ]
        );
    }

    #[test]
    fn a_makers_families_come_first() {
        let customization = r#"<fonts-modification version="1">
            <family customizationType="new-named-family" name="google-sans">
                <font weight="400" style="normal">GoogleSans-Regular.ttf</font>
            </family>
            <family customizationType="new-named-family" name="maker-serif">
                <font weight="400" style="normal">NotoSerif-Regular.ttf</font>
            </family>
            <family customizationType="new-named-family" name="gone">
                <font weight="400" style="normal">Missing.ttf</font>
            </family>
        </fonts-modification>"#;
        let configs = vec![
            (
                parse_fonts_xml(customization),
                PathBuf::from("/product/fonts"),
            ),
            (parse_fonts_xml(FONTS_XML), PathBuf::from("/system/fonts")),
        ];
        let fonts = named_families(&configs, facts, link);
        let urls: Vec<_> = fonts
            .iter()
            .map(|font| (font.family.as_str(), font.faces[0].url.as_str()))
            .collect();
        assert_eq!(
            urls,
            [
                ("Coming Soon", "font:/system/fonts/ComingSoon.ttf"),
                (
                    "Dancing Script",
                    "font:/system/fonts/DancingScript-Regular.ttf"
                ),
                ("Google Sans", "font:/product/fonts/GoogleSans-Regular.ttf"),
                ("Noto Serif", "font:/product/fonts/NotoSerif-Regular.ttf"),
                ("Roboto", "font:/system/fonts/Roboto-Regular.ttf"),
            ],
            "the maker's Noto Serif over the system's; a missing file lists nothing"
        );
    }

    #[test]
    fn files_outside_the_font_folder_are_never_linked() {
        let families = vec![XmlFamily {
            name: Some("serif".into()),
            fonts: vec![font("../../data/secret.ttf", 400, false)],
        }];
        let fonts = named_families(&[(families, PathBuf::from("/system/fonts"))], facts, link);
        assert!(fonts.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn reads_a_font_files_own_name() {
        let facts = read_file(Path::new(r"C:\Windows\Fonts\arial.ttf")).unwrap();
        assert_eq!(facts.family, "Arial");
        assert!(facts.latin);
        assert_eq!(facts.weights, None);
    }
}
