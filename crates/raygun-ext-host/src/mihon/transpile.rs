//! Extensions built on no theme: their Kotlin, turned into JavaScript for the Mihon runtime.

use super::convert::Converted;
use super::index::StoreExtension;
use super::kotlin::ast::File;

/// Converts a theme-less extension; the error says why it can't be.
pub fn convert(
    _extension: &StoreExtension,
    _files: &[(String, File)],
) -> Result<Converted, String> {
    Err("it's written without a theme, which Raygun doesn't convert yet".into())
}
