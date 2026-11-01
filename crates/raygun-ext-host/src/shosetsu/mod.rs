//! Shosetsu compatibility (M32): Shosetsu's Lua novel sources run in Raygun as `novel.source`
//! providers. `select` and `dom` stand in for Jsoup, `http` for OkHttp, and `runtime` for
//! kotlin-lib's `LuaExtension`.

pub mod dom;
pub mod http;
pub mod package;
pub mod runtime;
pub mod select;

pub use runtime::ShosetsuExtension;

#[cfg(test)]
mod tests;
