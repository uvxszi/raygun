//! Nuvio compatibility: the providers of Nuvio repositories (tapframe/NuvioStreaming, GPL-3.0)
//! run in Raygun's QuickJS runtime as `video.streams` providers: given a movie or an episode by
//! its TMDB (or IMDb) id, they answer with streams that play sessions offer beside the Stremio
//! addons' (`raygun-app`'s `stremio::StremioService`).
//!
//! - `index` reads a repository's `manifest.json` (its `scrapers`), `package` turns a downloaded
//!   provider into an extension folder (runtime `nuvio`, one extension per provider, so each has
//!   its own switch).
//! - `runtime` wraps the provider as Nuvio evaluates it (a CommonJS module) and installs the
//!   natives; `prelude.js` gives the globals Nuvio gives (`fetch`, cheerio, CryptoJS,
//!   `crypto.subtle`, `TextEncoder`, timers, `SCRAPER_SETTINGS`, `TMDB_API_KEY`) and calls its
//!   `getStreams(tmdbId, mediaType, season, episode)`.

pub mod index;
pub mod package;
pub(crate) mod runtime;

#[cfg(test)]
mod tests;
