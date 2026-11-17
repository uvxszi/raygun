//! Raygun's sources (M46.56): extensions and their host, catalogues, repositories, servers, the AI service, crash records and safe starts.

pub mod ai;
pub mod browser_check;
pub mod catalog;
pub mod comics;
pub mod crash_log;
pub mod extension_installs;
pub mod extensions;
pub mod genres;
pub mod hidden;
pub mod mihon;
pub mod mihon_container;
pub mod recent_searches;
pub mod search;
pub mod search_filter;
pub mod servers;
pub mod source_detect;
pub mod source_queue;
pub mod stremio_addons;
pub mod uploads;
pub mod web_views;

#[allow(unused_imports)]
pub(crate) use raygun_app_base::{
    device_state, events, local_network, network, picker, privacy, settings,
};

/// The start-up's handles, at the paths the moved code names them by.
#[allow(unused_imports)]
pub(crate) mod startup {
    pub use raygun_app_base::{AppDb, AppPaths};
}

/// Exports' files, at the paths the moved code names them by.
#[allow(unused_imports)]
pub(crate) mod export {
    pub use raygun_app_export::output::{self, Outputs, Place};
}

#[allow(unused_imports)]
pub(crate) use raygun_app_reading::{dictionary, epub};
