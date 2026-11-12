//! Lockstep & Hybrid-Rollback Multiplayer Transport (`Tasks 247, 248, 249, 250`).
//!
//! Modularized from the legacy 4,353-line `transport.rs` monolith into focused `< 800`-line
//! submodules while preserving 100% of public types, constants, and re-exports.

mod client_connect;
mod client_staging;
mod client_step;
mod common;
mod server_poll_io;
mod server_poll_staging;
mod server_poll_ticks;
mod server_setup;
mod server_staging_setup;
mod server_stalls;

pub use crate::dedicated::{DedicatedMatchConfig, DedicatedMatchHost, SpectatorClient};
pub use client_connect::LockstepClient;
pub use common::{
    generate_crypto_token_u64, is_authorized_multiplayer_command, BroadcastBundleRecord,
    RelayEndpointInput, TwoTierSnapshotHistory, COARSE_ANCHOR_RING_CAPACITY,
    COARSE_ANCHOR_RING_INTERVAL_TICKS, DEFAULT_MIN_SPECTATOR_DELAY_TICKS,
    DEFAULT_STALL_BROADCAST_INTERVAL_MS, DEFAULT_STALL_DETECT_THRESHOLD_MS,
    DEFAULT_STALL_GRACE_TIMEOUT_MS, FINE_SNAPSHOT_RING_CAPACITY, FINE_SNAPSHOT_RING_INTERVAL_TICKS,
    MAX_SPECTATOR_HISTORY_TICKS,
};
pub use server_setup::LockstepServer;
