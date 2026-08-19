//! Shared application assembly for Stellr hosts.

#[cfg(all(debug_assertions, feature = "desktop"))]
pub mod acceptance;
#[cfg(feature = "desktop")]
pub mod auth_activation;
pub mod cli;
#[cfg(feature = "desktop")]
pub mod desktop;
pub mod entrypoints;
pub mod route_state;
pub mod runtime;
pub mod target;
pub mod theme;
