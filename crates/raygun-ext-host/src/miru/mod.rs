//! Miru compatibility: Miru's JavaScript extensions (miru-project/miru-app, AGPL-3.0; scripts
//! from repositories such as miru-project/repo, MIT) run in Raygun's QuickJS runtime as video
//! (`bangumi`), manga and novel (`fikushon`) sources.
//!
//! - `index` reads Miru repositories (`index.json`), `header` a script's metadata block and
//!   `package` turns a downloaded script into an extension folder (runtime `miru`).
//! - `runtime` prepares the script as Miru does and installs the natives behind Miru's API;
//!   `prelude.js` builds that API (`Extension`, `Element`, `this.request`, settings, timers) and
//!   presents the script as a Raygun provider.
//! - `dom` answers Miru's HTML queries with Raygun's Jsoup-compatible selector engine, `xpath`
//!   its `queryXPath`.

mod dom;
pub mod header;
pub mod index;
pub mod package;
pub(crate) mod runtime;
mod xpath;

#[cfg(test)]
mod tests;
