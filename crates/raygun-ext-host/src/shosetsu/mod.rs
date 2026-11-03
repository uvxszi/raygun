//! Shosetsu compatibility (M32): Shosetsu's Lua novel sources run in Raygun as `novel.source`
//! providers. `select` and `dom` stand in for Jsoup, `http` for OkHttp, `lualib` for the parts of
//! LuaJ's library the sandbox writes itself, and `runtime` for kotlin-lib's `LuaExtension`;
//! `filters` maps a script's filters onto Raygun's, and `errors` says what went wrong in plain
//! words.

pub mod dom;
pub mod errors;
pub mod filters;
pub mod http;
pub mod lualib;
pub mod package;
pub mod runtime;
pub mod select;

pub use runtime::ShosetsuExtension;

#[cfg(test)]
mod tests;
