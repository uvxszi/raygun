//! File and folder names for saved and exported files, following Mihon's rules
//! (`core/common/.../storage/DiskUtil.kt`, Apache-2.0): every file system Android writes to
//! accepts them.

/// Longest name, in UTF-8 bytes (most file systems allow 255).
const MAX_BYTES: usize = 240;

/// `name` made safe as one file or folder name: no path separators or characters Windows and
/// FAT refuse, no leading or trailing dots or spaces, at most 240 bytes.
pub fn safe_name(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|c| match c {
            '"' | '*' | '/' | ':' | '<' | '>' | '?' | '\\' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = replaced.trim_matches(|c: char| c == '.' || c.is_whitespace());
    let mut cut = String::new();
    for c in trimmed.chars() {
        if cut.len() + c.len_utf8() > MAX_BYTES {
            break;
        }
        cut.push(c);
    }
    let cut = cut.trim_end_matches(|c: char| c == '.' || c.is_whitespace());
    if cut.is_empty() {
        "(untitled)".to_owned()
    } else {
        cut.to_owned()
    }
}

/// `stem.extension`, keeping the extension whole when the stem is cut.
pub fn file_name(stem: &str, extension: &str) -> String {
    let budget = MAX_BYTES.saturating_sub(extension.len() + 1);
    let mut stem = safe_name(stem);
    while stem.len() > budget {
        stem.pop();
    }
    format!("{}.{extension}", stem.trim_end())
}

/// A chapter's file name: its number first (padded, so files sort in reading order), then its
/// name. Sources without numbers (or with -1 for "unknown") get the name alone.
pub fn chapter_stem(number: Option<f64>, name: &str) -> String {
    let name = name.trim();
    let Some(number) = number.filter(|number| *number >= 0.0) else {
        return name.to_owned();
    };
    let whole = number.trunc();
    let fraction = number - whole;
    let shown = if fraction.abs() < 1e-6 {
        format!("{:04}", whole as u64)
    } else {
        // 12.5 → "0012.5"
        let decimals = format!("{fraction:.3}");
        let decimals = decimals.trim_start_matches('0').trim_end_matches('0');
        format!("{:04}{decimals}", whole as u64)
    };
    if name.is_empty() {
        shown
    } else {
        format!("{shown} {name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_lose_what_file_systems_refuse() {
        assert_eq!(safe_name("Re:Zero / Part 2?"), "Re_Zero _ Part 2_");
        assert_eq!(safe_name("  ..Hidden.. "), "Hidden");
        assert_eq!(safe_name("a\u{7}b"), "a_b");
        assert_eq!(safe_name("..."), "(untitled)");
        let long = "é".repeat(200);
        let safe = safe_name(&long);
        assert!(safe.len() <= 240 && safe.chars().all(|c| c == 'é'));
        let file = file_name(&"x".repeat(300), "cbz");
        assert!(file.len() <= 240 && file.ends_with(".cbz"));
    }

    #[test]
    fn chapter_names_sort_by_number() {
        assert_eq!(chapter_stem(Some(12.0), "Chapter 12"), "0012 Chapter 12");
        assert_eq!(chapter_stem(Some(3.0), "The Start"), "0003 The Start");
        assert_eq!(chapter_stem(Some(12.5), "Extra"), "0012.5 Extra");
        assert_eq!(chapter_stem(None, "Oneshot"), "Oneshot");
    }
}
