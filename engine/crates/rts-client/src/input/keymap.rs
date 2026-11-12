use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

/// 30 rebindable tactical, production, camera, and HUD actions persisted in `saves/settings.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HotkeySlot {
    AttackMove,
    Stop,
    Scatter,
    ActivateAbility,
    RotateBuilding,
    CycleStance,
    Patrol,
    Guard,
    ReverseMove,
    UnloadTransport,
    PlaceMine,
    CycleFormation,
    SetPrimaryBuilding,
    UpgradeStructure,
    TogglePower,
    SellStructure,
    PauseProduction,
    SelectHarvester,
    SelectDamaged,
    CenterSelection,
    JumpHomeBase,
    JumpLastEvent,
    ToggleTechTree,
    ToggleDiplomacy,
    ToggleLogbook,
    TogglePauseMatch,
    CycleGameSpeed,
    ToggleFogOfWar,
    QuickSave,
    QuickLoad,
}

impl HotkeySlot {
    pub const ALL: [Self; 30] = [
        Self::AttackMove,
        Self::Stop,
        Self::Scatter,
        Self::ActivateAbility,
        Self::RotateBuilding,
        Self::CycleStance,
        Self::Patrol,
        Self::Guard,
        Self::ReverseMove,
        Self::UnloadTransport,
        Self::PlaceMine,
        Self::CycleFormation,
        Self::SetPrimaryBuilding,
        Self::UpgradeStructure,
        Self::TogglePower,
        Self::SellStructure,
        Self::PauseProduction,
        Self::SelectHarvester,
        Self::SelectDamaged,
        Self::CenterSelection,
        Self::JumpHomeBase,
        Self::JumpLastEvent,
        Self::ToggleTechTree,
        Self::ToggleDiplomacy,
        Self::ToggleLogbook,
        Self::TogglePauseMatch,
        Self::CycleGameSpeed,
        Self::ToggleFogOfWar,
        Self::QuickSave,
        Self::QuickLoad,
    ];

    pub fn settings_key(self) -> &'static str {
        match self {
            Self::AttackMove => "hotkey_attack_move",
            Self::Stop => "hotkey_stop",
            Self::Scatter => "hotkey_scatter",
            Self::ActivateAbility => "hotkey_activate_ability",
            Self::RotateBuilding => "hotkey_rotate_building",
            Self::CycleStance => "hotkey_cycle_stance",
            Self::Patrol => "hotkey_patrol",
            Self::Guard => "hotkey_guard",
            Self::ReverseMove => "hotkey_reverse_move",
            Self::UnloadTransport => "hotkey_unload_transport",
            Self::PlaceMine => "hotkey_place_mine",
            Self::CycleFormation => "hotkey_cycle_formation",
            Self::SetPrimaryBuilding => "hotkey_set_primary_building",
            Self::UpgradeStructure => "hotkey_upgrade_structure",
            Self::TogglePower => "hotkey_toggle_power",
            Self::SellStructure => "hotkey_sell_structure",
            Self::PauseProduction => "hotkey_pause_production",
            Self::SelectHarvester => "hotkey_select_harvester",
            Self::SelectDamaged => "hotkey_select_damaged",
            Self::CenterSelection => "hotkey_center_selection",
            Self::JumpHomeBase => "hotkey_jump_home_base",
            Self::JumpLastEvent => "hotkey_jump_last_event",
            Self::ToggleTechTree => "hotkey_toggle_tech_tree",
            Self::ToggleDiplomacy => "hotkey_toggle_diplomacy",
            Self::ToggleLogbook => "hotkey_toggle_logbook",
            Self::TogglePauseMatch => "hotkey_toggle_pause_match",
            Self::CycleGameSpeed => "hotkey_cycle_game_speed",
            Self::ToggleFogOfWar => "hotkey_toggle_fog_of_war",
            Self::QuickSave => "hotkey_quick_save",
            Self::QuickLoad => "hotkey_quick_load",
        }
    }

    pub fn from_settings_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.settings_key() == key)
    }

    pub fn default_binding(self) -> &'static str {
        match self {
            Self::AttackMove => "X",
            Self::Stop => "H",
            Self::Scatter => "V",
            Self::ActivateAbility => "G",
            Self::RotateBuilding => "R",
            Self::CycleStance => "T",
            Self::Patrol => "Z",
            Self::Guard => "F",
            Self::ReverseMove => "K",
            Self::UnloadTransport => "U",
            Self::PlaceMine => "M",
            Self::CycleFormation => "J",
            Self::SetPrimaryBuilding => "B",
            Self::UpgradeStructure => "N",
            Self::TogglePower => "O",
            Self::SellStructure => "DELETE",
            Self::PauseProduction => "I",
            Self::SelectHarvester => "PERIOD",
            Self::SelectDamaged => "COMMA",
            Self::CenterSelection => "C",
            Self::JumpHomeBase => "HOME",
            Self::JumpLastEvent => "BACKSPACE",
            Self::ToggleTechTree => "Y",
            Self::ToggleDiplomacy => "D",
            Self::ToggleLogbook => "TAB",
            Self::TogglePauseMatch => "P",
            Self::CycleGameSpeed => "F6",
            Self::ToggleFogOfWar => "F4",
            Self::QuickSave => "F5",
            Self::QuickLoad => "F8",
        }
    }

    pub fn default_keycode(self) -> KeyCode {
        parse_key_code(self.default_binding(), KeyCode::KeyX)
    }

    pub fn category(self) -> &'static str {
        match self {
            Self::AttackMove
            | Self::Stop
            | Self::Scatter
            | Self::ActivateAbility
            | Self::CycleStance
            | Self::Patrol
            | Self::Guard
            | Self::ReverseMove
            | Self::UnloadTransport
            | Self::PlaceMine
            | Self::CycleFormation => "COMBAT & ORDERS",
            Self::RotateBuilding
            | Self::SetPrimaryBuilding
            | Self::UpgradeStructure
            | Self::TogglePower
            | Self::SellStructure
            | Self::PauseProduction => "BASE & PRODUCTION",
            Self::SelectHarvester
            | Self::SelectDamaged
            | Self::CenterSelection
            | Self::JumpHomeBase
            | Self::JumpLastEvent => "SELECTION & CAMERA",
            Self::ToggleTechTree
            | Self::ToggleDiplomacy
            | Self::ToggleLogbook
            | Self::TogglePauseMatch
            | Self::CycleGameSpeed
            | Self::ToggleFogOfWar
            | Self::QuickSave
            | Self::QuickLoad => "HUD & SYSTEM",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::AttackMove => "Attack-Move",
            Self::Stop => "Stop / Hold",
            Self::Scatter => "Scatter Units",
            Self::ActivateAbility => "Special Ability",
            Self::RotateBuilding => "Rotate / Repair",
            Self::CycleStance => "Cycle Unit Stance",
            Self::Patrol => "Patrol Order",
            Self::Guard => "Guard / Escort",
            Self::ReverseMove => "Reverse Move",
            Self::UnloadTransport => "Unload Transport",
            Self::PlaceMine => "Deploy Landmine",
            Self::CycleFormation => "Cycle Formation",
            Self::SetPrimaryBuilding => "Primary Factory",
            Self::UpgradeStructure => "Upgrade Structure",
            Self::TogglePower => "Toggle Building Power",
            Self::SellStructure => "Sell Structure",
            Self::PauseProduction => "Pause Production",
            Self::SelectHarvester => "Select Harvester / MCV",
            Self::SelectDamaged => "Select Damaged Units",
            Self::CenterSelection => "Center on Selection",
            Self::JumpHomeBase => "Jump to Home Base",
            Self::JumpLastEvent => "Jump to Last Alert",
            Self::ToggleTechTree => "Toggle Tech Tree",
            Self::ToggleDiplomacy => "Toggle Diplomacy",
            Self::ToggleLogbook => "Subgroup / Logbook",
            Self::TogglePauseMatch => "Pause Match",
            Self::CycleGameSpeed => "Cycle Sim Speed",
            Self::ToggleFogOfWar => "Toggle Fog of War",
            Self::QuickSave => "Quick-Save Match",
            Self::QuickLoad => "Quick-Load Match",
        }
    }
}

pub fn default_hotkey_map() -> BTreeMap<HotkeySlot, String> {
    let mut map = BTreeMap::new();
    for slot in HotkeySlot::ALL {
        map.insert(slot, slot.default_binding().to_string());
    }
    map
}

/// Returns the set of `HotkeySlot`s whose normalized key binding collides with another slot.
pub fn conflicting_hotkey_slots(bindings: &BTreeMap<HotkeySlot, String>) -> BTreeSet<HotkeySlot> {
    let mut by_key: BTreeMap<String, Vec<HotkeySlot>> = BTreeMap::new();
    for slot in HotkeySlot::ALL {
        let key = bindings
            .get(&slot)
            .map(|s| s.trim().to_ascii_uppercase())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| slot.default_binding().to_string());
        by_key.entry(key).or_default().push(slot);
    }
    let mut conflicts = BTreeSet::new();
    for (_, group) in by_key {
        if group.len() > 1 {
            for slot in group {
                conflicts.insert(slot);
            }
        }
    }
    conflicts
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
        "TAB" => KeyCode::Tab,
        "DELETE" | "DEL" => KeyCode::Delete,
        "BACKSPACE" | "BKSP" => KeyCode::Backspace,
        "HOME" => KeyCode::Home,
        "END" => KeyCode::End,
        "PERIOD" | "." => KeyCode::Period,
        "COMMA" | "," => KeyCode::Comma,
        "SLASH" | "/" => KeyCode::Slash,
        "SEMICOLON" | ";" => KeyCode::Semicolon,
        "F1" => KeyCode::F1,
        "F2" => KeyCode::F2,
        "F3" => KeyCode::F3,
        "F4" => KeyCode::F4,
        "F5" => KeyCode::F5,
        "F6" => KeyCode::F6,
        "F7" => KeyCode::F7,
        "F8" => KeyCode::F8,
        "F10" => KeyCode::F10,
        "F11" => KeyCode::F11,
        _ => fallback,
    }
}

pub fn capture_rebind_key_string(keys: &ButtonInput<KeyCode>) -> Option<&'static str> {
    let candidates = [
        (KeyCode::KeyA, "A"),
        (KeyCode::KeyB, "B"),
        (KeyCode::KeyC, "C"),
        (KeyCode::KeyD, "D"),
        (KeyCode::KeyE, "E"),
        (KeyCode::KeyF, "F"),
        (KeyCode::KeyG, "G"),
        (KeyCode::KeyH, "H"),
        (KeyCode::KeyI, "I"),
        (KeyCode::KeyJ, "J"),
        (KeyCode::KeyK, "K"),
        (KeyCode::KeyL, "L"),
        (KeyCode::KeyM, "M"),
        (KeyCode::KeyN, "N"),
        (KeyCode::KeyO, "O"),
        (KeyCode::KeyP, "P"),
        (KeyCode::KeyQ, "Q"),
        (KeyCode::KeyR, "R"),
        (KeyCode::KeyS, "S"),
        (KeyCode::KeyT, "T"),
        (KeyCode::KeyU, "U"),
        (KeyCode::KeyV, "V"),
        (KeyCode::KeyW, "W"),
        (KeyCode::KeyX, "X"),
        (KeyCode::KeyY, "Y"),
        (KeyCode::KeyZ, "Z"),
        (KeyCode::Space, "SPACE"),
        (KeyCode::Tab, "TAB"),
        (KeyCode::Delete, "DELETE"),
        (KeyCode::Backspace, "BACKSPACE"),
        (KeyCode::Home, "HOME"),
        (KeyCode::End, "END"),
        (KeyCode::Period, "PERIOD"),
        (KeyCode::Comma, "COMMA"),
        (KeyCode::Slash, "SLASH"),
        (KeyCode::Semicolon, "SEMICOLON"),
        (KeyCode::F1, "F1"),
        (KeyCode::F2, "F2"),
        (KeyCode::F3, "F3"),
        (KeyCode::F4, "F4"),
        (KeyCode::F5, "F5"),
        (KeyCode::F6, "F6"),
        (KeyCode::F7, "F7"),
        (KeyCode::F8, "F8"),
        (KeyCode::F10, "F10"),
        (KeyCode::F11, "F11"),
    ];
    for (kc, name) in candidates {
        if keys.just_pressed(kc) {
            return Some(name);
        }
    }
    None
}

/// Canonical 3x4 `QWER / ASDF / ZXCV` positional Grid-Hotkey labels for up to 12 sidebar cameos.
pub const GRID_CAMEO_KEY_LABELS: [&str; 12] =
    ["Q", "W", "E", "R", "A", "S", "D", "F", "Z", "X", "C", "V"];

pub const GRID_CAMEO_KEY_CODES: [KeyCode; 12] = [
    KeyCode::KeyQ,
    KeyCode::KeyW,
    KeyCode::KeyE,
    KeyCode::KeyR,
    KeyCode::KeyA,
    KeyCode::KeyS,
    KeyCode::KeyD,
    KeyCode::KeyF,
    KeyCode::KeyZ,
    KeyCode::KeyX,
    KeyCode::KeyC,
    KeyCode::KeyV,
];

/// Returns which `0..12` cameo grid slot was just pressed when `grid_hotkeys_enabled` is active.
/// If `wasd_pan_enabled` is also active, `W/A/S/D` are reserved for camera panning.
pub fn pressed_grid_cameo_slot(
    keys: &ButtonInput<KeyCode>,
    wasd_pan_enabled: bool,
) -> Option<usize> {
    for (idx, &kc) in GRID_CAMEO_KEY_CODES.iter().enumerate() {
        if wasd_pan_enabled
            && matches!(
                kc,
                KeyCode::KeyW | KeyCode::KeyA | KeyCode::KeyS | KeyCode::KeyD
            )
        {
            continue;
        }
        if keys.just_pressed(kc) {
            return Some(idx);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_30_hotkey_map_has_zero_conflicts_and_resolves_t_y_collision() {
        let defaults = default_hotkey_map();
        assert_eq!(defaults.len(), 30);
        assert_eq!(defaults.get(&HotkeySlot::CycleStance).unwrap(), "T");
        assert_eq!(defaults.get(&HotkeySlot::ToggleTechTree).unwrap(), "Y");
        assert_eq!(defaults.get(&HotkeySlot::QuickSave).unwrap(), "F5");
        assert_eq!(defaults.get(&HotkeySlot::QuickLoad).unwrap(), "F8");
        let conflicts = conflicting_hotkey_slots(&defaults);
        assert!(
            conflicts.is_empty(),
            "Default 30-slot keymap must have zero collisions: {conflicts:?}"
        );
    }

    #[test]
    fn conflicting_hotkey_slots_flags_duplicate_bindings() {
        let mut map = default_hotkey_map();
        map.insert(HotkeySlot::Patrol, "X".to_string()); // collides with AttackMove ("X")
        let conflicts = conflicting_hotkey_slots(&map);
        assert!(conflicts.contains(&HotkeySlot::AttackMove));
        assert!(conflicts.contains(&HotkeySlot::Patrol));
        assert_eq!(conflicts.len(), 2);
    }
}
