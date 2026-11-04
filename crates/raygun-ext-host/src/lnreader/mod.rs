//! LNReader compatibility: LNReader's novel plugins (plugins built from LNReader/lnreader-plugins,
//! MIT, for the LNReader app, MIT) run in Raygun's QuickJS runtime as `novel.source` providers.
//!
//! - `index` reads LNReader repositories (`plugins.min.json`: an array of `{ id, name, site,
//!   lang, version, url, iconUrl }`), `package` turns a downloaded plugin into an extension
//!   folder (runtime `lnreader`).
//! - `runtime` wraps the plugin as LNReader evaluates it (CommonJS, with a `require`) and installs
//!   the natives; `prelude.js` builds what `require` hands out (cheerio, htmlparser2, dayjs,
//!   urlencode and LNReader's `@libs`) and presents the plugin as a Raygun provider.
//! - `dom` keeps the documents cheerio and htmlparser2 read, queried with Raygun's
//!   Jsoup-compatible selector engine.

pub(crate) mod dom;
pub mod index;
pub mod package;
pub(crate) mod runtime;

#[cfg(test)]
mod tests;
