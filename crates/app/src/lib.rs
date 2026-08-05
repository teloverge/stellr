//! Shared application assembly for Stellr hosts.

#[cfg(debug_assertions)]
pub mod acceptance;
pub mod auth_activation;
pub mod cli;
pub mod desktop;
pub mod entrypoints;
pub mod route_state;
pub mod runtime;
pub mod target;
pub mod theme;
