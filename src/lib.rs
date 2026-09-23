//! MultiSignal: manage several Signal Desktop profiles.
//!
//! Everything except `ui` and `system` is free of GTK so it can be tested
//! against a temporary directory.

pub mod launcher;
pub mod names;
pub mod paths;
pub mod procs;
pub mod profiles;
pub mod store;
pub mod system;
pub mod units;
