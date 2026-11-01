//! Raygun's AI subsystem (ARCHITECTURE §8): the encrypted secrets store, the model manager and the
//! translation providers. Pure logic; the app wires it to settings and commands.

pub mod models;
pub mod secrets;
pub mod translate;
