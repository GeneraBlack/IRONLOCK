//! Authoritative Headless `rts-dedicated-server` Host & Delayed Live Spectator (`Task 249`).

mod fleet_reporting;
mod host;
mod spectator;

pub use host::{DedicatedMatchConfig, DedicatedMatchHost};
pub use spectator::{DelayedBundleEntry, SpectatorClient};
