mod hotkeys;
mod keyboard;
mod mouse_left;
mod mouse_right;

pub use keyboard::handle_rts_input;

use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_simulation::ai::{AiDifficulty, AiPersonality};
use rts_simulation::combat::distance_to_target_entity;
pub use rts_simulation::world::{
    EnvironmentPreset, MapEditorBrushSize, MapSymmetryMode,
};
use rts_simulation::world::{EntityKind, SimEntity, SimWorld};
use std::collections::BTreeSet;
use std::path::Path;

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

#[derive(Debug, Clone, PartialEq)]
pub struct ClientSettings {
    pub master_volume: f32,
    pub camera_pan_speed: f32,
    pub default_fog_of_war: bool,
    pub default_ai_difficulty: String,
    pub hotkey_attack_move: String,
    pub hotkey_stop: String,
    pub hotkey_scatter: String,
    pub hotkey_activate_ability: String,
    pub hotkey_rotate_building: String,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            master_volume: 0.85,
            camera_pan_speed: 32.0,
            default_fog_of_war: false,
            default_ai_difficulty: "Medium".to_string(),
            hotkey_attack_move: "X".to_string(),
            hotkey_stop: "H".to_string(),
            hotkey_scatter: "V".to_string(),
            hotkey_activate_ability: "G".to_string(),
            hotkey_rotate_building: "R".to_string(),
        }
    }
}

pub fn parse_key_code(s: &str, fallback: KeyCode) -> KeyCode {
    match s.trim().to_ascii_uppercase().as_str() {
        "A" => KeyCode::KeyA,
        "B" => KeyCode::KeyB,
        "C" => KeyCode::KeyC,
        "D" => KeyCode::KeyD,
        "E" => KeyCode::KeyE,
        "F" => KeyCode::KeyF,
        "G" => KeyCode::KeyG,
        "H" => KeyCode::KeyH,
        "I" => KeyCode::KeyI,
        "J" => KeyCode::KeyJ,
        "K" => KeyCode::KeyK,
        "L" => KeyCode::KeyL,
        "M" => KeyCode::KeyM,
        "N" => KeyCode::KeyN,
        "O" => KeyCode::KeyO,
        "P" => KeyCode::KeyP,
        "Q" => KeyCode::KeyQ,
        "R" => KeyCode::KeyR,
        "S" => KeyCode::KeyS,
        "T" => KeyCode::KeyT,
        "U" => KeyCode::KeyU,
        "V" => KeyCode::KeyV,
        "W" => KeyCode::KeyW,
        "X" => KeyCode::KeyX,
        "Y" => KeyCode::KeyY,
        "Z" => KeyCode::KeyZ,
        "SPACE" => KeyCode::Space,
        _ => fallback,
    }
}

impl ClientSettings {
    pub const DEFAULT_PATH: &'static str = "saves/settings.toml";

    pub fn load_or_default() -> Self {
        Self::load_from_file(Path::new(Self::DEFAULT_PATH)).unwrap_or_default()
    }

    pub fn load_from_file(path: &Path) -> Result<Self, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        let mut cfg = Self::default();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = trimmed.split_once('=') {
                let key = k.trim();
                let val = v.trim().trim_matches('"');
                match key {
                    "master_volume" => {
                        if let Ok(parsed) = val.parse::<f32>() {
                            cfg.master_volume = parsed.clamp(0.0, 1.0);
                        }
                    }
                    "camera_pan_speed" => {
                        if let Ok(parsed) = val.parse::<f32>() {
                            cfg.camera_pan_speed = parsed.clamp(12.0, 96.0);
                        }
                    }
                    "default_fog_of_war" => {
                        if let Ok(parsed) = val.parse::<bool>() {
                            cfg.default_fog_of_war = parsed;
                        }
                    }
                    "default_ai_difficulty" => {
                        cfg.default_ai_difficulty = val.to_string();
                    }
                    "hotkey_attack_move" => {
                        cfg.hotkey_attack_move = val.to_string();
                    }
                    "hotkey_stop" => {
                        cfg.hotkey_stop = val.to_string();
                    }
                    "hotkey_scatter" => {
                        cfg.hotkey_scatter = val.to_string();
                    }
                    "hotkey_activate_ability" => {
                        cfg.hotkey_activate_ability = val.to_string();
                    }
                    "hotkey_rotate_building" => {
                        cfg.hotkey_rotate_building = val.to_string();
                    }
                    _ => {}
                }
            }
        }
        Ok(cfg)
    }

    pub fn save_to_file(&self, path: &Path) -> Result<(), std::io::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let toml_text = format!(
            "# Rust-RTS Engine Persistent Client Configuration\nmaster_volume = {:.2}\ncamera_pan_speed = {:.1}\ndefault_fog_of_war = {}\ndefault_ai_difficulty = \"{}\"\nhotkey_attack_move = \"{}\"\nhotkey_stop = \"{}\"\nhotkey_scatter = \"{}\"\nhotkey_activate_ability = \"{}\"\nhotkey_rotate_building = \"{}\"\n",
            self.master_volume,
            self.camera_pan_speed,
            self.default_fog_of_war,
            self.default_ai_difficulty,
            self.hotkey_attack_move,
            self.hotkey_stop,
            self.hotkey_scatter,
            self.hotkey_activate_ability,
            self.hotkey_rotate_building,
        );
        std::fs::write(path, toml_text)
    }

    pub fn save_to_default_file(&self) -> Result<(), std::io::Error> {
        self.save_to_file(Path::new(Self::DEFAULT_PATH))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppPhase {
    #[default]
    MainMenu,
    LoadingScreen,
    InGame,
}

impl AppPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::MainMenu => "MAIN MENU",
            Self::LoadingScreen => "LOADING SCREEN",
            Self::InGame => "IN GAME",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuScreen {
    #[default]
    Root,
    Campaign,
    Skirmish,
    Multiplayer,
    Theater,
    Studio,
    Settings,
    PauseMenu,
}

impl MenuScreen {
    pub fn label(self) -> &'static str {
        match self {
            Self::Root => "MAIN MENU",
            Self::Campaign => "CAMPAIGN",
            Self::Skirmish => "SKIRMISH",
            Self::Multiplayer => "MULTIPLAYER (ONLINE & LAN)",
            Self::Theater => "REPLAYS, SAVES & MEDIA ARCHIVE",
            Self::Studio => "MOD STUDIO, EDITOR & ENCYCLOPEDIA",
            Self::Settings => "OPTIONS & CONTROLS",
            Self::PauseMenu => "TACTICAL PAUSE MENU",
        }
    }
}

fn collect_skirmish_sub_factions(
    loaded: &rts_sdk::loader::LoadedMod,
) -> Vec<(&rts_sdk::schema::FactionDef, &rts_sdk::schema::SubFactionDef)> {
    let mut order: Vec<&rts_sdk::schema::FactionDef> = loaded.factions.values().collect();
    // Preserve canonical faction display order if starting_credits / id order is used
    order.sort_by_key(|f| f.id.as_str());
    let mut out = Vec::new();
    for fac in order {
        for sub in &fac.sub_factions {
            out.push((fac, sub));
        }
    }
    out
}

pub fn qualify_mod_asset_rel_path(loaded: &rts_sdk::loader::LoadedMod, rel: &str) -> String {
    let trimmed = rel.trim_start_matches('/');
    for dir in &loaded.asset_search_dirs {
        if dir.join(trimmed).exists() {
            if let Some(folder_name) = dir.file_name().and_then(|s| s.to_str()) {
                return format!("{folder_name}/{trimmed}");
            }
        }
    }
    let prefix = format!("{}/", loaded.manifest.id);
    if rel.starts_with(&prefix) {
        rel.to_string()
    } else {
        format!("{}/{}", loaded.manifest.id, trimmed)
    }
}

pub fn mod_has_custom_hud_skin(loaded: &rts_sdk::loader::LoadedMod) -> bool {
    loaded.factions.values().any(|f| {
        f.hud_skin.dialog_background.is_some()
            || f.hud_skin.radar_frame.is_some()
            || f.hud_skin.credits_panel.is_some()
            || f.hud_skin.side_header.is_some()
    })
}

pub fn resolve_active_faction_hud_skin<'a>(
    loaded: &'a rts_sdk::loader::LoadedMod,
    faction_id: &str,
) -> Option<&'a rts_sdk::schema::FactionHudSkinDef> {
    loaded
        .factions
        .get(faction_id)
        .or_else(|| {
            loaded
                .factions
                .values()
                .find(|f| f.id.eq_ignore_ascii_case(faction_id) || f.id.starts_with(faction_id))
        })
        .or_else(|| loaded.factions.values().next())
        .map(|f| &f.hud_skin)
}

/// Returns how many selectable factions or sub-factions the currently loaded mod offers.
pub fn mod_skirmish_option_count(loaded: &rts_sdk::loader::LoadedMod) -> usize {
    let subs = collect_skirmish_sub_factions(loaded);
    if !subs.is_empty() {
        subs.len()
    } else {
        loaded.factions.len().max(1)
    }
}

/// Returns the sub-faction identifier at `index` if the loaded mod defines `sub_factions`.
pub fn resolve_mod_skirmish_sub_faction_id(
    loaded: &rts_sdk::loader::LoadedMod,
    index: usize,
) -> Option<String> {
    let subs = collect_skirmish_sub_factions(loaded);
    if subs.is_empty() {
        None
    } else {
        Some(subs[index % subs.len()].1.id.clone())
    }
}

/// Returns the first skirmish option index matching `faction_id`.
pub fn find_mod_skirmish_index_for_faction(
    loaded: &rts_sdk::loader::LoadedMod,
    faction_id: &str,
) -> usize {
    let subs = collect_skirmish_sub_factions(loaded);
    if !subs.is_empty() {
        subs.iter()
            .position(|(fac, sub)| {
                fac.id.eq_ignore_ascii_case(faction_id)
                    || sub.id.eq_ignore_ascii_case(faction_id)
                    || fac.id.starts_with(faction_id)
            })
            .unwrap_or(0)
    } else {
        loaded
            .factions
            .keys()
            .position(|fid| fid.eq_ignore_ascii_case(faction_id) || fid.starts_with(faction_id))
            .unwrap_or(0)
    }
}

/// Mod-neutral Skirmish faction/sub-faction descriptor:
/// Returns `(display_name, faction_id, specialty_description, optional_4k_artwork_path)`.
pub fn resolve_mod_skirmish_descriptor(
    loaded: &rts_sdk::loader::LoadedMod,
    index: usize,
) -> (String, String, String, Option<String>) {
    let subs = collect_skirmish_sub_factions(loaded);
    if !subs.is_empty() {
        let (fac, sub) = subs[index % subs.len()];
        let screen = sub
            .loading_screen
            .as_deref()
            .or(fac.loading_screen.as_deref())
            .map(|rel| qualify_mod_asset_rel_path(loaded, rel));
        let desc = if sub.specialty.trim().is_empty() {
            fac.description.clone()
        } else {
            sub.specialty.clone()
        };
        return (sub.name.clone(), fac.id.clone(), desc, screen);
    }

    let factions: Vec<_> = loaded.factions.values().collect();
    if factions.is_empty() {
        return (
            "Default Command".to_string(),
            "default".to_string(),
            "Modular RTS Faction".to_string(),
            None,
        );
    }
    let fac = factions[index % factions.len()];
    let desc = if fac.description.trim().is_empty() {
        format!("Starting Credits: ${}", fac.starting_credits)
    } else {
        fac.description.clone()
    };
    let screen_opt = if let Some(rel) = fac.loading_screen.as_deref() {
        Some(qualify_mod_asset_rel_path(loaded, rel))
    } else {
        let fallback_rel = format!("assets/ui/loading_screens/faction_{}.png", fac.id);
        loaded
            .root_dir
            .join(&fallback_rel)
            .exists()
            .then(|| qualify_mod_asset_rel_path(loaded, &fallback_rel))
    };
    (fac.name.clone(), fac.id.clone(), desc, screen_opt)
}

/// Mod-neutral Campaign loading screen resolver: reads `MissionDef.loading_screen` or
/// `FactionDef.campaign_loading_screens` from the active mod catalog.
pub fn resolve_mod_campaign_loading_screen(
    loaded: &rts_sdk::loader::LoadedMod,
    faction: &str,
    mission_index: usize,
) -> Option<String> {
    if let Some(camp) = loaded.campaigns.values().find(|c| {
        c.faction.eq_ignore_ascii_case(faction) || c.faction.starts_with(faction)
    }) {
        if let Some(miss) = camp.missions.get(mission_index) {
            if let Some(rel) = miss.loading_screen.as_deref() {
                return Some(qualify_mod_asset_rel_path(loaded, rel));
            }
        }
    }
    if let Some(fac) = loaded.factions.get(faction).or_else(|| {
        loaded
            .factions
            .values()
            .find(|f| f.id.eq_ignore_ascii_case(faction) || f.id.starts_with(faction))
    }) {
        if !fac.campaign_loading_screens.is_empty() {
            let rel = &fac.campaign_loading_screens[mission_index % fac.campaign_loading_screens.len()];
            return Some(qualify_mod_asset_rel_path(loaded, rel));
        }
    }
    let candidate = format!(
        "assets/ui/loading_screens/{}_{:02}.png",
        faction,
        mission_index + 1
    );
    if loaded.root_dir.join(&candidate).exists() {
        Some(qualify_mod_asset_rel_path(loaded, &candidate))
    } else {
        None
    }
}

/// Mod-neutral Cameo Icon resolver: checks `VisualDef.icon` on the unit/building definition first,
/// then checks `<active_mod>/assets/ui/cameos/<entity_id>.png`.
pub fn resolve_mod_entity_cameo(
    loaded: &rts_sdk::loader::LoadedMod,
    entity_id: &str,
) -> Option<String> {
    let catalog_icon = loaded
        .units
        .get(entity_id)
        .and_then(|u| u.visual.icon.as_deref())
        .or_else(|| {
            loaded
                .buildings
                .get(entity_id)
                .and_then(|b| b.visual.icon.as_deref())
        });
    if let Some(icon_rel) = catalog_icon {
        let stripped = icon_rel
            .strip_prefix(&format!("{}/", loaded.manifest.id))
            .unwrap_or(icon_rel);
        if loaded.resolve_asset_path(stripped).exists() {
            return Some(qualify_mod_asset_rel_path(loaded, stripped));
        }
    }
    let direct_rel = format!("assets/ui/cameos/{entity_id}.png");
    if loaded.resolve_asset_path(&direct_rel).exists() {
        return Some(qualify_mod_asset_rel_path(loaded, &direct_rel));
    }
    None
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
    pub studio_open: bool,
    pub scoreboard_open: bool,
    pub tech_tree_open: bool,
    pub studio_status_message: String,
    pub attack_move_mode: bool,
    pub patrol_mode: bool,
    pub guard_mode: bool,
    pub reverse_move_mode: bool,
    pub follow_entity_id: Option<u64>,
    pub sandbox_unit_index: usize,
    pub targeted_ability_mode: Option<String>,
    pub active_support_power_id: Option<String>,
    pub chrono_shift_source_pos: Option<[f32; 3]>,
    pub active_map_id: String,
    pub active_campaign_id: Option<String>,
    pub active_mission_id: Option<String>,
    pub campaign_mission_page: usize,
    pub skirmish_map_page: usize,
    pub skirmish_show_all_maps: bool,
    pub procedural_map_seed: u64,
    pub active_video_preview_path: Option<String>,
    pub chat_input_open: bool,
    pub chat_input_buffer: String,
    pub pending_outgoing_chat: Option<String>,
    pub chat_messages: Vec<(String, f32)>,
    pub ip_input_editing: bool,
    pub server_addr_input: String,
    pub lobby_server_url: String,
    pub active_lobby_id: Option<String>,
    pub online_lobbies: Vec<rts_network::lobby_registry::LobbyEntry>,
    pub observer_pov_player_id: u8,
    pub spectator_all_vision: bool,
    pub pending_replay_seek_tick: Option<u64>,
    pub replay_active: bool,
    pub replay_total_ticks: u64,
    pub dev_mode_enabled: bool,
    pub active_sidebar_tab: ProductionSidebarTab,
    pub dev_console_open: bool,
    pub dev_console_input: String,
    pub dev_console_history: Vec<String>,
    pub pending_single_steps: u32,
    pub debug_gizmo_flags: DebugGizmoFlags,
    pub net_status_line: String,
    pub selected_ids: BTreeSet<u64>,
    pub control_groups: [BTreeSet<u64>; 5],
    pub camera_bookmarks: [Option<Vec3>; 4],
    pub last_group_press_time: [f32; 5],
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
    pub hotkey_attack_move: String,
    pub hotkey_stop: String,
    pub hotkey_scatter: String,
    pub hotkey_activate_ability: String,
    pub hotkey_rotate_building: String,
}

impl ClientInteractionState {
    pub fn new(
        local_player_id: u8,
        is_multiplayer: bool,
        studio_open: bool,
        net_status_line: String,
    ) -> Self {
        let cfg = ClientSettings::load_or_default();
        let ai_diff = match cfg.default_ai_difficulty.to_ascii_lowercase().as_str() {
            "easy" => AiDifficulty::Easy,
            "hard" => AiDifficulty::Hard,
            "brutal" => AiDifficulty::Brutal,
            _ => AiDifficulty::Medium,
        };
        let start_in_menu = !is_multiplayer || studio_open;
        let initial_phase = if start_in_menu {
            AppPhase::MainMenu
        } else {
            AppPhase::InGame
        };
        let initial_screen = if studio_open {
            MenuScreen::Studio
        } else {
            MenuScreen::Root
        };
        let mut s = Self {
            app_phase: initial_phase,
            active_menu_screen: initial_screen,
            selected_campaign_faction: String::new(),
            selected_encyclopedia_faction: String::new(),
            selected_encyclopedia_entry: None,
            skirmish_country_index: 0,
            loading_target_title: "COMMAND & CONQUER: RED ALERT 2".to_string(),
            local_player_id,
            is_multiplayer,
            ai_enabled: !is_multiplayer,
            ai_difficulty: ai_diff,
            ai_personality: AiPersonality::Balanced,
            environment_preset: EnvironmentPreset::Day,
            fog_of_war_enabled: cfg.default_fog_of_war,
            game_paused: false,
            sim_speed_multiplier: 1.0,
            camera_pan_speed: cfg.camera_pan_speed,
            active_subgroup_unit_id: None,
            host_player_slots: 2,
            server_addr_input: "127.0.0.1:7878".to_string(),
            lobby_server_url: rts_network::lobby_registry::DEFAULT_CENTRAL_LOBBY_URL.to_string(),
            active_lobby_id: None,
            online_lobbies: Vec::new(),
            studio_open: start_in_menu,
            dev_mode_enabled: studio_open,
            active_sidebar_tab: ProductionSidebarTab::Structures,
            studio_status_message: if studio_open {
                "ENGINE STUDIO & DEVELOPER MODE ACTIVE - Map Editor, Asset Pipeline, Hot-Reload & Benchmark ready."
                    .to_string()
            } else {
                "COMMAND CENTER READY - Select Campaign (40 Missions), Skirmish, Online Multiplayer, or 3D Encyclopedia."
                    .to_string()
            },
            net_status_line,
            hotkey_attack_move: cfg.hotkey_attack_move,
            hotkey_stop: cfg.hotkey_stop,
            hotkey_scatter: cfg.hotkey_scatter,
            hotkey_activate_ability: cfg.hotkey_activate_ability,
            hotkey_rotate_building: cfg.hotkey_rotate_building,
            ..Default::default()
        };
        s.push_eva_alert("EVA: Battlefield control online. Good luck, Commander!");
        s
    }

    pub fn push_eva_alert(&mut self, msg: impl Into<String>) {
        let text = msg.into();
        if self.battle_feed.last().map(|(m, _)| m == &text).unwrap_or(false) {
            return;
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

/// Pure helper that returns all entity IDs whose 2D screen positions fall inside
/// the selection rectangle `[min_p, max_p]`.
pub fn filter_entities_in_screen_box(
    candidates: &[(u64, Vec2)],
    box_a: Vec2,
    box_b: Vec2,
) -> BTreeSet<u64> {
    let min_p = box_a.min(box_b);
    let max_p = box_a.max(box_b);
    candidates
        .iter()
        .filter(|(_, p)| p.x >= min_p.x && p.x <= max_p.x && p.y >= min_p.y && p.y <= max_p.y)
        .map(|(id, _)| *id)
        .collect()
}

/// Tests whether a 3D ray intersects an axis-aligned bounding box `[min, max]`, returning the entry distance.
pub fn ray_aabb_intersection(
    ray_origin: Vec3,
    ray_dir: Vec3,
    box_min: Vec3,
    box_max: Vec3,
) -> Option<f32> {
    let mut t_min = 0.0_f32;
    let mut t_max = f32::MAX;

    for axis in 0..3 {
        let orig = ray_origin[axis];
        let dir = ray_dir[axis];
        let b_min = box_min[axis];
        let b_max = box_max[axis];

        if dir.abs() < 1e-6 {
            if orig < b_min || orig > b_max {
                return None;
            }
        } else {
            let inv_d = 1.0 / dir;
            let mut t1 = (b_min - orig) * inv_d;
            let mut t2 = (b_max - orig) * inv_d;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            t_min = t_min.max(t1);
            t_max = t_max.min(t2);
            if t_min > t_max {
                return None;
            }
        }
    }
    Some(t_min)
}

pub(super) fn entity_hit_score(
    world: &SimWorld,
    loaded: &LoadedMod,
    ent: &SimEntity,
    ray: Option<Ray3d>,
    ground_hit: Vec3,
) -> Option<f32> {
    if ent.transported_by.is_some() {
        return None;
    }
    // 1. Check 3D Ray-AABB intersection against the 3D volume of the building/unit
    if let Some(r) = ray {
        let (box_min, box_max) = match &ent.kind {
            EntityKind::Building { .. } => {
                let fp = world.entity_footprint(ent, loaded);
                let ts = world.grid.tile_size;
                let min_x = ent.tile_origin[0] as f32 * ts - 0.35;
                let max_x = (ent.tile_origin[0] + fp[0] as i32) as f32 * ts + 0.35;
                let min_z = ent.tile_origin[1] as f32 * ts - 0.35;
                let max_z = (ent.tile_origin[1] + fp[1] as i32) as f32 * ts + 0.35;
                (
                    Vec3::new(min_x, 0.0, min_z),
                    Vec3::new(max_x, 3.4, max_z),
                )
            }
            EntityKind::Unit { .. } => (
                Vec3::new(ent.position[0] - 1.5, 0.0, ent.position[2] - 1.5),
                Vec3::new(ent.position[0] + 1.5, 2.2, ent.position[2] + 1.5),
            ),
        };
        if let Some(_t) = ray_aabb_intersection(r.origin, *r.direction, box_min, box_max) {
            return Some(0.0);
        }
    }

    // 2. Check footprint edge / ground distance fallback
    let edge_dist =
        distance_to_target_entity(world, loaded, [ground_hit.x, 0.0, ground_hit.z], ent);
    let max_edge_dist = match ent.kind {
        EntityKind::Building { .. } => 1.8,
        EntityKind::Unit { .. } => 2.8,
    };
    if edge_dist <= max_edge_dist {
        Some(edge_dist)
    } else {
        None
    }
}

pub(super) fn append_typed_keys(keys: &ButtonInput<KeyCode>, buf: &mut String, allow_letters: bool) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let digits = [
        (KeyCode::Digit0, '0'),
        (KeyCode::Digit1, '1'),
        (KeyCode::Digit2, '2'),
        (KeyCode::Digit3, '3'),
        (KeyCode::Digit4, '4'),
        (KeyCode::Digit5, '5'),
        (KeyCode::Digit6, '6'),
        (KeyCode::Digit7, '7'),
        (KeyCode::Digit8, '8'),
        (KeyCode::Digit9, '9'),
    ];
    for (k, ch) in digits {
        if keys.just_pressed(k) {
            if shift && ch == '1' && allow_letters {
                buf.push('!');
            } else {
                buf.push(ch);
            }
        }
    }
    if keys.just_pressed(KeyCode::Period) {
        buf.push(if shift { ':' } else { '.' });
    }
    if keys.just_pressed(KeyCode::Semicolon) && shift {
        buf.push(':');
    }
    if keys.just_pressed(KeyCode::Minus) {
        buf.push('-');
    }
    if keys.just_pressed(KeyCode::Space) && allow_letters {
        buf.push(' ');
    }
    if keys.just_pressed(KeyCode::Backspace) {
        buf.pop();
    }
    if allow_letters {
        let letters = [
            (KeyCode::KeyA, 'a'),
            (KeyCode::KeyB, 'b'),
            (KeyCode::KeyC, 'c'),
            (KeyCode::KeyD, 'd'),
            (KeyCode::KeyE, 'e'),
            (KeyCode::KeyF, 'f'),
            (KeyCode::KeyG, 'g'),
            (KeyCode::KeyH, 'h'),
            (KeyCode::KeyI, 'i'),
            (KeyCode::KeyJ, 'j'),
            (KeyCode::KeyK, 'k'),
            (KeyCode::KeyL, 'l'),
            (KeyCode::KeyM, 'm'),
            (KeyCode::KeyN, 'n'),
            (KeyCode::KeyO, 'o'),
            (KeyCode::KeyP, 'p'),
            (KeyCode::KeyQ, 'q'),
            (KeyCode::KeyR, 'r'),
            (KeyCode::KeyS, 's'),
            (KeyCode::KeyT, 't'),
            (KeyCode::KeyU, 'u'),
            (KeyCode::KeyV, 'v'),
            (KeyCode::KeyW, 'w'),
            (KeyCode::KeyX, 'x'),
            (KeyCode::KeyY, 'y'),
            (KeyCode::KeyZ, 'z'),
        ];
        for (k, ch) in letters {
            if keys.just_pressed(k) {
                buf.push(if shift { ch.to_ascii_uppercase() } else { ch });
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_box_selection_filters_entities_inside_rectangle() {
        let candidates = vec![
            (1, Vec2::new(50.0, 60.0)),
            (2, Vec2::new(120.0, 90.0)),
            (3, Vec2::new(300.0, 250.0)),
        ];
        let selected = filter_entities_in_screen_box(
            &candidates,
            Vec2::new(150.0, 110.0),
            Vec2::new(40.0, 40.0),
        );
        assert!(selected.contains(&1));
        assert!(selected.contains(&2));
        assert!(!selected.contains(&3));
    }

    #[test]
    fn ray_aabb_intersects_3d_building_volume() {
        let hit = ray_aabb_intersection(
            Vec3::new(10.0, 20.0, 25.0),
            Vec3::new(0.0, -0.707, -0.707),
            Vec3::new(8.0, 0.0, 4.0),
            Vec3::new(14.0, 3.4, 10.0),
        );
        assert!(hit.is_some(), "Angled camera ray should hit 3D building AABB");
    }

    #[test]
    fn client_settings_toml_roundtrip_works() {
        let temp_path = std::env::temp_dir().join("rust_rts_test_settings.toml");
        let original = ClientSettings {
            master_volume: 0.65,
            camera_pan_speed: 48.0,
            default_fog_of_war: true,
            default_ai_difficulty: "Hard".to_string(),
            hotkey_attack_move: "A".to_string(),
            hotkey_stop: "S".to_string(),
            hotkey_scatter: "V".to_string(),
            hotkey_activate_ability: "G".to_string(),
            hotkey_rotate_building: "R".to_string(),
        };
        original.save_to_file(&temp_path).expect("save settings");
        let loaded = ClientSettings::load_from_file(&temp_path).expect("load settings");
        assert!((loaded.master_volume - 0.65).abs() < 1e-3);
        assert!((loaded.camera_pan_speed - 48.0).abs() < 1e-3);
        assert!(loaded.default_fog_of_war);
        assert_eq!(loaded.default_ai_difficulty, "Hard");
        assert_eq!(loaded.hotkey_attack_move, "A");
        assert_eq!(loaded.hotkey_stop, "S");
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn main_menu_screens_and_skirmish_countries_are_complete() {
        assert_eq!(AppPhase::default(), AppPhase::MainMenu);
        assert_eq!(MenuScreen::default(), MenuScreen::Root);
        let screens = [
            MenuScreen::Root,
            MenuScreen::Campaign,
            MenuScreen::Skirmish,
            MenuScreen::Multiplayer,
            MenuScreen::Theater,
            MenuScreen::Studio,
            MenuScreen::Settings,
            MenuScreen::PauseMenu,
        ];
        for screen in screens {
            assert!(!screen.label().is_empty());
        }
        let loaded = rts_sdk::loader::LoadedMod::load_from_dir(
            rts_sdk::loader::LoadedMod::resolve_mod_dir("rust-ra2"),
        )
        .expect("should load rust-ra2 mod");
        assert_eq!(mod_skirmish_option_count(&loaded), 10);
        for idx in 0..10 {
            let (country, faction, special, loading_png) =
                resolve_mod_skirmish_descriptor(&loaded, idx);
            assert!(!country.is_empty());
            assert!(matches!(faction.as_str(), "allies" | "soviets" | "yuri"));
            assert!(!special.is_empty());
            let png = loading_png.expect("sub-faction should have loading_screen");
            assert!(png.starts_with("rust-ra2/assets/ui/loading_screens/country_"));
            assert!(png.ends_with(".png"));
        }
        assert!(
            resolve_mod_campaign_loading_screen(&loaded, "allies", 0)
                .unwrap()
                .starts_with("rust-ra2/assets/ui/loading_screens/ra2_allied_")
        );
        assert!(
            resolve_mod_campaign_loading_screen(&loaded, "soviets", 0)
                .unwrap()
                .starts_with("rust-ra2/assets/ui/loading_screens/ra2_soviet_")
        );
        assert!(
            resolve_mod_entity_cameo(&loaded, "grizzly_tank")
                .unwrap()
                .starts_with("rust-ra2/assets/ui/cameos/")
        );
    }
}
