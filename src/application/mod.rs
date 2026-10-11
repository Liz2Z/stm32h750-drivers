//! Application orchestration: sensor acquisition and refresh scheduling.
pub mod acquisition;
#[cfg(target_arch = "arm")]
pub mod config;
pub mod connection;
pub mod presentation;
#[cfg(target_arch = "arm")]
mod runtime;
pub mod scheduling;
#[cfg(target_arch = "arm")]
pub use runtime::run;
