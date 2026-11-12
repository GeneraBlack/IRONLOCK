mod hotkeys;
mod keyboard;
mod keyboard_actions;
pub mod keymap;
mod mouse_left;
mod mouse_left_editor;
mod mouse_right;
mod mouse_right_targets;
mod picking;
mod save_envelope;
mod ui_cache;

pub use keyboard::{handle_rts_input, resolve_active_sidebar_cameo_page};
pub use keymap::{
    capture_rebind_key_string, conflicting_hotkey_slots, default_hotkey_map, parse_key_code,
    pressed_grid_cameo_slot, HotkeySlot, GRID_CAMEO_KEY_CODES, GRID_CAMEO_KEY_LABELS,
};
pub use picking::{
    collect_idle_economy_units, filter_entities_in_screen_box, ray_aabb_intersection,
    sample_polyline_formation_targets,
};
pub use save_envelope::{
    load_world_envelope_checked_from_path, load_world_envelope_from_path,
    save_world_envelope_to_path,
};
pub use ui_cache::{
    discover_compatible_mutators, find_mod_skirmish_index_for_faction, mod_has_custom_hud_skin,
    mod_skirmish_option_count, qualify_mod_asset_rel_path, resolve_active_faction_hud_skin,
    resolve_mod_campaign_loading_screen, resolve_mod_entity_cameo, resolve_mod_item_cameo,
    resolve_mod_skirmish_descriptor, resolve_mod_skirmish_sub_faction_id, DiscoveredMutatorEntry,
    UiAssetCache, UiInputBlocker,
};

use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::manifest::HudLayoutPreset;
use rts_simulation::ai::{AiDifficulty, AiPersonality};
use rts_simulation::command::AreaCommandKind;
pub use rts_simulation::world::{EnvironmentPreset, MapEditorBrushSize, MapSymmetryMode};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapEditorBrush {
    #[default]
    Off,
    PaintOre,
    PaintHighGround,
    PaintRamp,
    PaintWater,
    PaintBridge,
    PaintRock,
    PaintNeutralBunker,
    PaintCrate,
    SpawnUnit,
    PlaceBuilding,
    PlacePlayerSpawn,
    PlaceTriggerZone,
    ClearTile,
}

impl MapEditorBrush {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::PaintOre,
            Self::PaintOre => Self::PaintHighGround,
            Self::PaintHighGround => Self::PaintRamp,
            Self::PaintRamp => Self::PaintWater,
            Self::PaintWater => Self::PaintBridge,
            Self::PaintBridge => Self::PaintRock,
            Self::PaintRock => Self::PaintNeutralBunker,
            Self::PaintNeutralBunker => Self::PaintCrate,
            Self::PaintCrate => Self::SpawnUnit,
            Self::SpawnUnit => Self::PlaceBuilding,
            Self::PlaceBuilding => Self::PlacePlayerSpawn,
            Self::PlacePlayerSpawn => Self::PlaceTriggerZone,
            Self::PlaceTriggerZone => Self::ClearTile,
            Self::ClearTile => Self::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::PaintOre => "ORE",
            Self::PaintHighGround => "HIGH GROUND",
            Self::PaintRamp => "RAMP",
            Self::PaintWater => "WATER",
            Self::PaintBridge => "BRIDGE",
            Self::PaintRock => "ROCK",
            Self::PaintNeutralBunker => "NEUTRAL BUNKER",
            Self::PaintCrate => "CRATE",
            Self::SpawnUnit => "SPAWN UNIT",
            Self::PlaceBuilding => "PLACE BUILDING",
            Self::PlacePlayerSpawn => "PLAYER SPAWN",
            Self::PlaceTriggerZone => "TRIGGER ZONE",
            Self::ClearTile => "CLEAR",
        }
    }
}

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppPhase {
    #[default]
    MainMenu,
    LoadingScreen,
    InGame,
    PostGameScore,
}

impl AppPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::MainMenu => "MAIN MENU",
            Self::LoadingScreen => "LOADING SCREEN",
            Self::InGame => "IN GAME",
            Self::PostGameScore => "POST-GAME SCORE",
        }
    }
}

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MenuScreen {
    #[default]
    Root,
    Campaign,
    Skirmish,
    Multiplayer,
    MultiplayerStaging,
    ModWorkshop,
    Theater,
    Encyclopedia,
    Studio,
    Settings,
    PauseMenu,
    PostGameScore,
}

impl MenuScreen {
    pub fn label(self) -> &'static str {
        match self {
            Self::Root => "MAIN MENU",
            Self::Campaign => "CAMPAIGN",
            Self::Skirmish => "SKIRMISH",
            Self::Multiplayer => "MULTIPLAYER (ONLINE & LAN)",
            Self::MultiplayerStaging => "MULTIPLAYER STAGING ROOM",
            Self::ModWorkshop => "MOD WORKSHOP",
            Self::Theater => "REPLAYS, SAVES & MEDIA ARCHIVE",
            Self::Encyclopedia | Self::Studio => "UNIT & FACTION ENCYCLOPEDIA",
            Self::Settings => "OPTIONS & CONTROLS",
            Self::PauseMenu => "TACTICAL PAUSE MENU",
            Self::PostGameScore => "POST-GAME SCORE & GRAPHS",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProductionSidebarTab {
    #[default]
    Structures,
    Defenses,
    Infantry,
    Vehicles,
}

impl ProductionSidebarTab {
    pub fn index(self) -> usize {
        match self {
            Self::Structures => 0,
            Self::Defenses => 1,
            Self::Infantry => 2,
            Self::Vehicles => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VehicleDomainFilter {
    #[default]
    All,
    Ground,
    Naval,
    Air,
}

impl VehicleDomainFilter {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Ground => "GND",
            Self::Naval => "NAV",
            Self::Air => "AIR",
        }
    }

    pub fn matches(self, u: &rts_sdk::schema::UnitDef) -> bool {
        match self {
            Self::All => true,
            Self::Ground => matches!(
                u.movement_domain,
                rts_sdk::schema::MovementDomain::Ground
                    | rts_sdk::schema::MovementDomain::Amphibious
            ),
            Self::Naval => matches!(
                u.movement_domain,
                rts_sdk::schema::MovementDomain::Naval
                    | rts_sdk::schema::MovementDomain::Amphibious
            ),
            Self::Air => matches!(
                u.movement_domain,
                rts_sdk::schema::MovementDomain::Air | rts_sdk::schema::MovementDomain::Jumpjet
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DebugGizmoFlags {
    pub show_paths: bool,
    pub show_weapon_ranges: bool,
    pub show_vision_and_stealth: bool,
    pub show_trigger_zones: bool,
    pub show_blocked_tiles: bool,
}

impl DebugGizmoFlags {
    pub fn any_active(&self) -> bool {
        self.show_paths
            || self.show_weapon_ranges
            || self.show_vision_and_stealth
            || self.show_trigger_zones
            || self.show_blocked_tiles
    }
}

#[derive(Resource, Debug, Default)]
pub struct ClientInteractionState {
    pub app_phase: AppPhase,
    pub active_menu_screen: MenuScreen,
    pub selected_campaign_faction: String,
    pub selected_encyclopedia_faction: String,
    pub selected_encyclopedia_entry: Option<String>,
    pub skirmish_country_index: usize,
    pub loading_target_title: String,
    pub loading_progress: f32,
    pub local_player_id: u8,
    pub is_multiplayer: bool,
    pub ai_enabled: bool,
    pub ai_difficulty: AiDifficulty,
    pub ai_personality: AiPersonality,
    pub environment_preset: EnvironmentPreset,
    pub fog_of_war_enabled: bool,
    pub game_paused: bool,
    pub sim_speed_multiplier: f32,
    pub camera_pan_speed: f32,
    pub active_subgroup_unit_id: Option<String>,
    pub map_editor_brush: MapEditorBrush,
    pub map_editor_brush_size: MapEditorBrushSize,
    pub map_editor_symmetry: MapSymmetryMode,
    pub sandbox_building_index: usize,
    pub environment_dirty: bool,
    pub menu_open: bool,
    pub studio_open: bool,
    pub studio_playtest_active: bool,
    pub ui_scale: f32,
    pub active_options_tab: u8,
    pub scoreboard_open: bool,
    pub tech_tree_open: bool,
    pub studio_status_message: String,
    pub attack_move_mode: bool,
    pub patrol_mode: bool,
    pub guard_mode: bool,
    pub reverse_move_mode: bool,
    pub repair_cursor_mode: bool,
    pub harvest_cursor_mode: bool,
    pub sell_cursor_mode: bool,
    pub follow_entity_id: Option<u64>,
    pub hovered_entity_id: Option<u64>,
    pub sandbox_unit_index: usize,
    pub targeted_ability_mode: Option<String>,
    pub active_support_power_id: Option<String>,
    pub chrono_shift_source_pos: Option<[f32; 3]>,
    pub worker_placing_building: Option<(Vec<u64>, String)>,
    pub active_map_id: String,
    pub active_campaign_id: Option<String>,
    pub active_mission_id: Option<String>,
    pub campaign_mission_page: usize,
    pub skirmish_map_page: usize,
    pub skirmish_show_all_maps: bool,
    pub procedural_map_seed: u64,
    pub active_video_preview_path: Option<String>,
    pub chat_input_open: bool,
    pub chat_team_only: bool,
    pub chat_input_buffer: String,
    pub pending_outgoing_chat: Option<(bool, String)>,
    pub chat_messages: Vec<(String, f32)>,
    pub ip_input_editing: bool,
    pub server_addr_input: String,
    pub lobby_server_url: String,
    pub active_lobby_id: Option<String>,
    pub online_lobbies: Vec<rts_network::lobby_registry::LobbyEntry>,
    pub observer_pov_player_id: u8,
    pub spectator_all_vision: bool,
    pub spectator_mode_active: bool,
    pub reconnect_token_cached: Option<u64>,
    pub custom_map_transfer_status: String,
    pub pending_replay_seek_tick: Option<u64>,
    pub replay_active: bool,
    pub replay_total_ticks: u64,
    pub dev_mode_enabled: bool,
    pub edge_scroll_enabled: bool,
    pub settings_opened_from_pause: bool,
    pub rebinding_hotkey_slot: Option<HotkeySlot>,
    pub selected_ability_index: usize,
    pub hovered_cameo_tooltip: Option<String>,
    pub active_sidebar_tab: ProductionSidebarTab,
    pub sidebar_tab_pages: [usize; 4],
    pub vehicle_domain_filter: VehicleDomainFilter,
    pub support_power_page: usize,
    pub upgrade_page: usize,
    pub encyclopedia_page: usize,
    pub encyclopedia_show_buildings: bool,
    pub online_lobby_page: usize,
    pub replay_list_page: usize,
    pub save_list_page: usize,
    pub jukebox_page: usize,
    pub dev_console_open: bool,
    pub dev_console_input: String,
    pub dev_console_history: Vec<String>,
    pub pending_single_steps: u32,
    pub debug_gizmo_flags: DebugGizmoFlags,
    pub net_status_line: String,
    pub selected_ids: BTreeSet<u64>,
    pub control_groups: [BTreeSet<u64>; 10],
    pub camera_bookmarks: [Option<Vec3>; 4],
    pub last_group_press_time: [f32; 10],
    pub last_click_time: f32,
    pub last_clicked_entity: Option<u64>,
    pub drag_start_screen: Option<Vec2>,
    pub drag_current_screen: Option<Vec2>,
    pub placing_building_id: Option<String>,
    pub placement_rotated: bool,
    pub wall_drag_start_tile: Option<[i32; 2]>,
    pub host_player_slots: u8,
    pub hovered_tile: [i32; 2],
    pub pending_command_rings: Vec<(Vec3, Color)>,
    pub battle_feed: Vec<(String, f32)>,
    pub eva_log_archive: Vec<(u32, String)>,
    pub hotkey_attack_move: String,
    pub hotkey_stop: String,
    pub hotkey_scatter: String,
    pub hotkey_activate_ability: String,
    pub hotkey_rotate_building: String,
    pub hotkeys: BTreeMap<HotkeySlot, String>,
    pub grid_hotkeys_enabled: bool,
    pub wasd_pan_enabled: bool,
    pub smart_cast_enabled: bool,
    pub hud_layout_override: Option<HudLayoutPreset>,
    pub colorblind_mode: String,
    pub auto_resync_on_desync: bool,
    pub active_mutators: Vec<String>,
    pub use_hybrid_rollback: bool,
    pub rmb_curve_points: Vec<[f32; 3]>,
    pub lmb_area_cmd_drag: Option<(AreaCommandKind, Vec3, Vec3)>,
    pub desync_forensics_path: Option<String>,
    pub virtual_shift_preview: bool,
    pub language: String,
    pub ssao_quality: String,
    pub lock_player_mode: bool,
    pub autosave_interval_secs: u32,
    pub autosave_elapsed_secs: f32,
    pub autosave_slot_cursor: u8,
    pub player_display_name: String,
    pub player_elo_rating: f64,
    pub player_wins: u32,
    pub player_losses: u32,
    pub leaderboard_entries: Vec<rts_network::lobby_registry::CommanderRankEntry>,
    pub lobby_password_input: String,
    pub lobby_password_editing: bool,
    pub active_lobby_room: Option<rts_network::lobby_room::LobbyRoomState>,
    pub custom_map_transfer_pct: f32,
    pub player_identity_short: String,
    pub staging_chat_input: String,
    pub staging_chat_editing: bool,
    pub staging_chat_history: Vec<String>,
    pub staging_custom_map_path: Option<PathBuf>,
    pub staging_map_index: usize,
    pub automatch_ticket_id: Option<String>,
    pub automatch_mode: Option<rts_network::lobby_registry::AutomatchMode>,
    pub automatch_status_text: String,
    pub automatch_poll_timer: f32,
    pub reconnect_in_progress: bool,
    pub reconnect_retry_timer: f32,
    pub reconnect_attempts: u32,
    pub reconnect_status_text: String,
    pub desync_banner_text: Option<String>,
    pub match_rated_reported: bool,
    pub save_slot_name_input: String,
    pub save_slot_editing: bool,
    pub selected_save_path: Option<PathBuf>,
    pub post_game_tab: u8,
    pub post_game_dismissed_winner: Option<u8>,
    pub rmb_drag_start_screen: Option<Vec2>,
    pub rmb_drag_start_world: Option<Vec3>,
    pub rmb_drag_active: bool,
    pub window_mode: String,
    pub resolution: [u32; 2],
    pub vsync_enabled: bool,
    pub shadow_quality: String,
    pub fps_cap: u16,
    pub ping_cursor_mode: bool,
    pub radar_event_ring: VecDeque<[f32; 3]>,
    pub radar_event_cursor: usize,
    pub last_synced_radar_event: Option<[f32; 3]>,
    pub last_control_group_click: Option<(usize, f32)>,
    pub objectives_hud_collapsed: bool,
    pub workshop_items: Vec<rts_network::lobby_registry::WorkshopItemMetadata>,
    pub workshop_search_query: String,
    pub workshop_search_editing: bool,
    pub workshop_kind_filter: Option<rts_network::lobby_registry::WorkshopItemKind>,
    pub workshop_sort: rts_network::lobby_registry::WorkshopSortOrder,
    pub workshop_status_text: String,
    pub workshop_page: usize,
    pub pending_missing_mod_prompt: Option<(String, String)>,
    pub selected_mp_resume_save: Option<PathBuf>,
    pub mp_resume_save_page: usize,
}

impl ClientInteractionState {
    pub fn new(
        local_player_id: u8,
        is_multiplayer: bool,
        studio_open: bool,
        net_status_line: String,
    ) -> Self {
        let cfg = crate::settings_store::UnifiedClientSettingsFile::load_or_default();
        let ai_diff = match cfg.default_ai_difficulty.to_ascii_lowercase().as_str() {
            "easy" => AiDifficulty::Easy,
            "hard" => AiDifficulty::Hard,
            "brutal" => AiDifficulty::Brutal,
            _ => AiDifficulty::Medium,
        };
        let launch_studio = studio_open && !is_multiplayer;
        let start_in_menu = !is_multiplayer && !launch_studio;
        let initial_phase = if start_in_menu {
            AppPhase::MainMenu
        } else {
            AppPhase::InGame
        };
        let identity = rts_network::player_identity::PlayerIdentityProfile::load_or_create(
            rts_network::player_identity::PlayerIdentityProfile::default_path(),
            &cfg.player_display_name,
        )
        .ok();
        let display_name = identity
            .as_ref()
            .map(|p| p.display_name.clone())
            .unwrap_or_else(|| cfg.player_display_name.clone());
        let identity_short = identity
            .as_ref()
            .map(|p| p.player_id.chars().take(8).collect::<String>())
            .unwrap_or_default();
        let slot_hotkeys = cfg.slot_hotkeys();
        let hud_override =
            crate::settings_store::parse_hud_layout_preset_opt(cfg.hud_layout_override.as_deref());
        let mut s = Self {
            app_phase: initial_phase,
            active_menu_screen: MenuScreen::Root,
            loading_target_title: "COMMAND & CONQUER: RED ALERT 2".to_string(),
            local_player_id,
            is_multiplayer,
            ai_enabled: !is_multiplayer,
            ai_difficulty: ai_diff,
            ai_personality: AiPersonality::Balanced,
            environment_preset: EnvironmentPreset::Day,
            fog_of_war_enabled: if is_multiplayer {
                true
            } else {
                cfg.default_fog_of_war
            },
            sim_speed_multiplier: 1.0,
            camera_pan_speed: cfg.camera_pan_speed,
            edge_scroll_enabled: cfg.edge_scroll_enabled,
            ui_scale: cfg.ui_scale.clamp(0.75, 1.50),
            host_player_slots: 2,
            server_addr_input: "127.0.0.1:7878".to_string(),
            lobby_server_url: rts_network::lobby_registry::DEFAULT_CENTRAL_LOBBY_URL.to_string(),
            custom_map_transfer_status: "Custom Map Sync: Ready (Chunked Transfer Enabled)"
                .to_string(),
            menu_open: start_in_menu,
            studio_open: launch_studio,
            dev_mode_enabled: launch_studio,
            active_sidebar_tab: ProductionSidebarTab::Structures,
            studio_status_message: if launch_studio {
                "IRONLOCK CREATOR WORKBENCH ACTIVE - Dockable IDE, World Builder & SDK Inspector ready."
                    .to_string()
            } else {
                "COMMAND CENTER READY - Select Campaign, Skirmish, Online Multiplayer, or Encyclopedia."
                    .to_string()
            },
            net_status_line,
            hotkey_attack_move: cfg.hotkey_attack_move,
            hotkey_stop: cfg.hotkey_stop,
            hotkey_scatter: cfg.hotkey_scatter,
            hotkey_activate_ability: cfg.hotkey_activate_ability,
            hotkey_rotate_building: cfg.hotkey_rotate_building,
            hotkeys: slot_hotkeys,
            grid_hotkeys_enabled: cfg.grid_hotkeys_enabled,
            wasd_pan_enabled: cfg.wasd_pan_enabled,
            smart_cast_enabled: cfg.smart_cast_enabled,
            hud_layout_override: hud_override,
            colorblind_mode: if cfg.colorblind_mode.trim().is_empty() {
                "off".to_string()
            } else {
                cfg.colorblind_mode
            },
            auto_resync_on_desync: cfg.auto_resync_on_desync,
            use_hybrid_rollback: true,
            language: if cfg.language.trim().is_empty() {
                "en".to_string()
            } else {
                cfg.language
            },
            ssao_quality: if cfg.ssao_quality.trim().is_empty() {
                "medium".to_string()
            } else {
                cfg.ssao_quality
            },
            window_mode: if cfg.window_mode.trim().is_empty() {
                "windowed".to_string()
            } else {
                cfg.window_mode
            },
            resolution: cfg.resolution,
            vsync_enabled: cfg.vsync_enabled,
            shadow_quality: if cfg.shadow_quality.trim().is_empty() {
                "medium".to_string()
            } else {
                cfg.shadow_quality
            },
            fps_cap: cfg.fps_cap,
            autosave_interval_secs: cfg.autosave_interval_secs,
            player_display_name: display_name,
            player_identity_short: identity_short,
            player_elo_rating: 1500.0,
            automatch_status_text: "Automatch Idle (1v1 / 2v2 Ranked Glicko-2)".to_string(),
            workshop_status_text:
                "Workshop Ready - Browse community .rtsmod packages & custom maps.".to_string(),
            save_slot_name_input: "slot_alpha".to_string(),
            ..Default::default()
        };
        s.push_eva_alert("EVA: Battlefield control online. Good luck, Commander!");
        s
    }

    pub fn phase(&self) -> AppPhase {
        self.app_phase
    }

    pub fn menu_screen(&self) -> MenuScreen {
        self.active_menu_screen
    }

    pub fn effective_hud_layout(&self, loaded: &LoadedMod) -> HudLayoutPreset {
        self.hud_layout_override
            .unwrap_or(loaded.manifest.default_hud_layout)
    }

    pub fn binding_for(&self, slot: HotkeySlot) -> &str {
        if let Some(val) = self.hotkeys.get(&slot) {
            if !val.trim().is_empty() {
                return val.as_str();
            }
        }
        match slot {
            HotkeySlot::AttackMove if !self.hotkey_attack_move.trim().is_empty() => {
                &self.hotkey_attack_move
            }
            HotkeySlot::Stop if !self.hotkey_stop.trim().is_empty() => &self.hotkey_stop,
            HotkeySlot::Scatter if !self.hotkey_scatter.trim().is_empty() => &self.hotkey_scatter,
            HotkeySlot::ActivateAbility if !self.hotkey_activate_ability.trim().is_empty() => {
                &self.hotkey_activate_ability
            }
            HotkeySlot::RotateBuilding if !self.hotkey_rotate_building.trim().is_empty() => {
                &self.hotkey_rotate_building
            }
            _ => slot.default_binding(),
        }
    }

    pub fn hotkey_code(&self, slot: HotkeySlot) -> KeyCode {
        parse_key_code(self.binding_for(slot), slot.default_keycode())
    }

    pub fn set_hotkey_binding(&mut self, slot: HotkeySlot, bound: impl Into<String>) {
        let val = bound.into();
        if self.hotkeys.is_empty() {
            self.hotkeys = default_hotkey_map();
        }
        self.hotkeys.insert(slot, val.clone());
        match slot {
            HotkeySlot::AttackMove => self.hotkey_attack_move = val,
            HotkeySlot::Stop => self.hotkey_stop = val,
            HotkeySlot::Scatter => self.hotkey_scatter = val,
            HotkeySlot::ActivateAbility => self.hotkey_activate_ability = val,
            HotkeySlot::RotateBuilding => self.hotkey_rotate_building = val,
            _ => {}
        }
    }

    pub fn reset_all_hotkeys_to_defaults(&mut self) {
        self.hotkeys = default_hotkey_map();
        self.hotkey_attack_move = HotkeySlot::AttackMove.default_binding().to_string();
        self.hotkey_stop = HotkeySlot::Stop.default_binding().to_string();
        self.hotkey_scatter = HotkeySlot::Scatter.default_binding().to_string();
        self.hotkey_activate_ability = HotkeySlot::ActivateAbility.default_binding().to_string();
        self.hotkey_rotate_building = HotkeySlot::RotateBuilding.default_binding().to_string();
    }

    pub fn persist_client_settings(&self) {
        let mut cfg = crate::settings_store::UnifiedClientSettingsFile::load_or_default();
        cfg.apply_from_interaction_only(self);
        let _ = cfg.save_default();
    }

    pub fn push_radar_event(&mut self, pos: [f32; 3]) {
        if self.radar_event_ring.iter().any(|p| {
            let dx = p[0] - pos[0];
            let dz = p[2] - pos[2];
            dx * dx + dz * dz < 36.0
        }) {
            return;
        }
        if self.radar_event_ring.len() >= 8 {
            self.radar_event_ring.pop_front();
        }
        self.radar_event_ring.push_back(pos);
        self.radar_event_cursor = self.radar_event_ring.len();
    }

    pub fn cycle_radar_event(&mut self, fallback_last_pos: Option<[f32; 3]>) -> Option<[f32; 3]> {
        if self.radar_event_ring.is_empty() {
            if let Some(p) = fallback_last_pos {
                self.push_radar_event(p);
            }
        }
        if self.radar_event_ring.is_empty() {
            return None;
        }
        let len = self.radar_event_ring.len();
        let idx = if self.radar_event_cursor == 0 || self.radar_event_cursor > len {
            len - 1
        } else {
            self.radar_event_cursor - 1
        };
        let chosen = self.radar_event_ring[idx];
        self.radar_event_cursor = if idx == 0 { len } else { idx };
        Some(chosen)
    }

    pub fn push_eva_alert(&mut self, msg: impl Into<String>) {
        let text = msg.into();
        if self
            .battle_feed
            .last()
            .map(|(m, _)| m == &text)
            .unwrap_or(false)
        {
            return;
        }
        let seq_idx = self.eva_log_archive.last().map(|(n, _)| n + 1).unwrap_or(1);
        self.eva_log_archive.push((seq_idx, text.clone()));
        if self.eva_log_archive.len() > 120 {
            self.eva_log_archive.remove(0);
        }
        self.battle_feed.push((text, 6.5));
        if self.battle_feed.len() > 4 {
            self.battle_feed.remove(0);
        }
    }

    pub fn push_chat_message(&mut self, msg: impl Into<String>) {
        self.chat_messages.push((msg.into(), 12.0));
        if self.chat_messages.len() > 6 {
            self.chat_messages.remove(0);
        }
    }
}
