mod client_api;
mod client_http;
mod models_core;
mod models_ext;
mod relay_hub;
mod server_http;
mod server_routes_core;
mod server_routes_v1;
mod state_automatch_workshop;
mod state_lobbies;
mod state_mmr_matches;
#[cfg(test)]
mod tests;

pub use crate::lobby_room::LobbyMatchRules;
pub use client_http::{resolve_redirect_url, LobbyClientTimeouts, LobbyRegistryClient};
pub use models_core::{
    is_cgnat_or_private_ip, now_unix_ms, CommanderRankEntry, CreateMatchRequest,
    LeaderboardResponse, LobbyAnnouncement, LobbyEntry, LobbyHeartbeatRequest, LobbyListResponse,
    LobbyStatus, MatchPlayerRecord, MatchRecord, PlayerRegistrationRequest, StunWhoamiResponse,
    DEFAULT_CENTRAL_LOBBY_URL, DEFAULT_RELAY_MUX_PORT, LOCAL_FALLBACK_LOBBY_URL,
    MAX_HTTP_BODY_BYTES, MAX_HTTP_HEADER_BYTES, MAX_WORKSHOP_UPLOAD_BYTES,
};
pub use models_ext::{
    canonical_workshop_publish_payload, is_safe_workshop_item_id, AutomatchAssignment,
    AutomatchCancelRequest, AutomatchEnqueueRequest, AutomatchMode, AutomatchPlayerAssignment,
    AutomatchPollResponse, AutomatchPollStatus, DedicatedMatchAllocation,
    DedicatedServerAllocateRequest, DedicatedServerAllocateResponse, DedicatedServerEntry,
    DedicatedServerHeartbeatRequest, DedicatedServerListResponse, DedicatedServerRegisterRequest,
    DedicatedServerRegisterResponse, DedicatedServerStatus, WorkshopItemKind,
    WorkshopItemListResponse, WorkshopItemMetadata, WorkshopPublishRequest, WorkshopQuery,
    WorkshopSortOrder,
};
pub use server_http::{resolve_trusted_client_and_server_host, LobbyRegistryServer};
pub use state_mmr_matches::glicko2_update_single;
