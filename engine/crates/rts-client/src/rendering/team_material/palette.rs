use bevy::prelude::*;
use bevy::render::view::visibility::VisibilityRange;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::SimWorld;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const MAX_SHARED_TEAM_DAMAGE_MATERIALS: usize = 64;
pub const TEAM_COLOR_PALETTE_SLOTS: u8 = 17;
pub const PLAYER_TEAM_COLOR_COUNT: u8 = 16;

/// Canonical 16-player built-in sRGB palette (`1..=16`), overridden slot-for-slot by `ModManifest::player_colors`.
pub const DEFAULT_PLAYER_COLORS: [(&str, &str, [f32; 3]); 16] = [
    ("cobalt_blue", "Cobalt Blue", [0.06, 0.36, 1.00]),
    ("crimson_red", "Crimson Red", [0.94, 0.05, 0.06]),
    ("psi_purple", "Psi Purple", [0.58, 0.07, 0.96]),
    ("sovereign_gold", "Sovereign Gold", [1.00, 0.78, 0.04]),
    ("emerald_green", "Emerald Green", [0.06, 0.80, 0.20]),
    ("chrono_cyan", "Chrono Cyan", [0.04, 0.82, 0.92]),
    ("blaze_orange", "Blaze Orange", [1.00, 0.38, 0.03]),
    ("comandante_pink", "Comandante Pink", [0.94, 0.12, 0.60]),
    ("sky_ice_blue", "Sky Ice Blue", [0.42, 0.76, 1.00]),
    ("chartreuse_lime", "Chartreuse Lime", [0.62, 0.94, 0.10]),
    ("burgundy_maroon", "Burgundy Maroon", [0.52, 0.04, 0.14]),
    ("navy_indigo", "Navy Indigo", [0.10, 0.16, 0.62]),
    ("forest_olive", "Forest Olive", [0.26, 0.48, 0.14]),
    ("teal_turquoise", "Teal Turquoise", [0.04, 0.56, 0.52]),
    ("desert_amber", "Desert Amber", [0.84, 0.62, 0.30]),
    ("bronze_brown", "Bronze Brown", [0.50, 0.28, 0.10]),
];

/// Accessible colorblind & high-contrast team-color palette modes (`Task 240`).
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ColorblindPaletteMode {
    #[default]
    Off,
    Deuteranopia,
    Protanopia,
    Tritanopia,
    AllyEnemyHighContrast,
}

impl ColorblindPaletteMode {
    pub const ALL: [Self; 5] = [
        Self::Off,
        Self::Deuteranopia,
        Self::Protanopia,
        Self::Tritanopia,
        Self::AllyEnemyHighContrast,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Deuteranopia => "deuteranopia",
            Self::Protanopia => "protanopia",
            Self::Tritanopia => "tritanopia",
            Self::AllyEnemyHighContrast => "ally_enemy_high_contrast",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Deuteranopia => "Deuteranopia",
            Self::Protanopia => "Protanopia",
            Self::Tritanopia => "Tritanopia",
            Self::AllyEnemyHighContrast => "Ally/Enemy High Contrast",
        }
    }

    pub fn from_str_lossy(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "deuteranopia" | "deutan" => Self::Deuteranopia,
            "protanopia" | "protan" => Self::Protanopia,
            "tritanopia" | "tritan" => Self::Tritanopia,
            "ally_enemy_high_contrast" | "high_contrast" | "allyenemyhighcontrast" => {
                Self::AllyEnemyHighContrast
            }
            _ => Self::Off,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Deuteranopia,
            Self::Deuteranopia => Self::Protanopia,
            Self::Protanopia => Self::Tritanopia,
            Self::Tritanopia => Self::AllyEnemyHighContrast,
            Self::AllyEnemyHighContrast => Self::Off,
        }
    }
}

/// Global accessibility resource controlling 3D team shaders, strategic icons, health bars, and minimap dots.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorblindSettingsResource {
    pub mode: ColorblindPaletteMode,
}

const DEUTERANOPIA_16: [[f32; 3]; 16] = [
    [0.00, 0.45, 0.70],
    [0.84, 0.37, 0.00],
    [0.80, 0.47, 0.65],
    [0.94, 0.89, 0.26],
    [0.34, 0.71, 0.91],
    [0.90, 0.62, 0.00],
    [0.15, 0.25, 0.85],
    [0.98, 0.74, 0.48],
    [0.56, 0.82, 0.98],
    [0.78, 0.76, 0.14],
    [0.52, 0.20, 0.02],
    [0.08, 0.14, 0.52],
    [0.44, 0.52, 0.22],
    [0.18, 0.56, 0.68],
    [0.86, 0.66, 0.38],
    [0.48, 0.34, 0.18],
];

const PROTANOPIA_16: [[f32; 3]; 16] = [
    [0.08, 0.48, 0.86],
    [0.92, 0.62, 0.05],
    [0.42, 0.76, 0.96],
    [0.96, 0.90, 0.28],
    [0.22, 0.30, 0.78],
    [0.82, 0.46, 0.12],
    [0.64, 0.56, 0.92],
    [0.95, 0.78, 0.52],
    [0.58, 0.86, 1.00],
    [0.80, 0.82, 0.16],
    [0.54, 0.32, 0.06],
    [0.10, 0.18, 0.56],
    [0.46, 0.54, 0.24],
    [0.16, 0.60, 0.74],
    [0.84, 0.68, 0.40],
    [0.46, 0.36, 0.20],
];

const TRITANOPIA_16: [[f32; 3]; 16] = [
    [0.00, 0.66, 0.68],
    [0.92, 0.18, 0.22],
    [0.96, 0.54, 0.66],
    [0.58, 0.88, 0.88],
    [0.12, 0.42, 0.46],
    [0.84, 0.14, 0.42],
    [0.98, 0.76, 0.78],
    [0.26, 0.78, 0.72],
    [0.44, 0.82, 0.86],
    [0.68, 0.92, 0.86],
    [0.56, 0.08, 0.18],
    [0.08, 0.26, 0.34],
    [0.24, 0.54, 0.52],
    [0.06, 0.56, 0.58],
    [0.86, 0.58, 0.62],
    [0.48, 0.26, 0.28],
];

/// Resolves an accessible `[r, g, b, a]` team color for `slot_idx` (`0..=16`), honoring `loaded.manifest.player_colors` when `mode == Off`.
pub fn resolve_accessible_team_color_in_mod(
    slot_idx: u8,
    is_local: bool,
    is_ally: bool,
    mode: ColorblindPaletteMode,
    loaded: Option<&LoadedMod>,
) -> [f32; 4] {
    let norm = slot_idx % TEAM_COLOR_PALETTE_SLOTS;
    if norm == 0 {
        return [0.78, 0.80, 0.84, 1.0];
    }
    let idx0 = (norm - 1) as usize;
    let rgb = match mode {
        ColorblindPaletteMode::Off => match loaded {
            Some(m) => team_color_rgb_for_slot_in_mod(norm, m),
            None => team_color_rgb_for_slot(norm),
        },
        ColorblindPaletteMode::AllyEnemyHighContrast => {
            if is_local {
                [0.14, 0.72, 1.00]
            } else if is_ally {
                [0.98, 0.86, 0.18]
            } else {
                [1.00, 0.22, 0.14]
            }
        }
        ColorblindPaletteMode::Deuteranopia => DEUTERANOPIA_16[idx0],
        ColorblindPaletteMode::Protanopia => PROTANOPIA_16[idx0],
        ColorblindPaletteMode::Tritanopia => TRITANOPIA_16[idx0],
    };
    [rgb[0], rgb[1], rgb[2], 1.0]
}

/// Resolves an accessible `[r, g, b, a]` team color for `slot_idx` under the active `ColorblindPaletteMode`.
pub fn resolve_accessible_team_color(
    slot_idx: u8,
    is_local: bool,
    is_ally: bool,
    mode: ColorblindPaletteMode,
) -> [f32; 4] {
    resolve_accessible_team_color_in_mod(slot_idx, is_local, is_ally, mode, None)
}

pub fn lod0_visibility_range() -> VisibilityRange {
    VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: 185.0..210.0,
        use_aabb: false,
    }
}

pub fn lod1_visibility_range() -> VisibilityRange {
    VisibilityRange {
        start_margin: 185.0..210.0,
        end_margin: 945.0..1015.0,
        use_aabb: false,
    }
}

#[inline]
pub fn is_lod1_node_name(node_name: &str) -> bool {
    node_name.ends_with("_LOD1") || node_name.contains("_LOD1.")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum DamageVisualTier {
    #[default]
    Intact,
    Yellow50,
    Red25,
}

impl DamageVisualTier {
    #[inline]
    pub fn from_hp_ratio(hp_ratio: f32) -> Self {
        let clamped = hp_ratio.clamp(0.0, 1.0);
        if clamped < 0.25 {
            Self::Red25
        } else if clamped < 0.50 {
            Self::Yellow50
        } else {
            Self::Intact
        }
    }
}

pub fn team_color_rgb_for_slot(slot: u8) -> [f32; 3] {
    let norm = slot % TEAM_COLOR_PALETTE_SLOTS;
    if norm == 0 {
        [0.76, 0.78, 0.82]
    } else {
        DEFAULT_PLAYER_COLORS[(norm - 1) as usize].2
    }
}

pub fn team_color_name_for_slot(slot: u8) -> &'static str {
    let norm = slot % TEAM_COLOR_PALETTE_SLOTS;
    if norm == 0 {
        "Neutral"
    } else {
        DEFAULT_PLAYER_COLORS[(norm - 1) as usize].1
    }
}

pub fn team_color_rgb_for_slot_in_mod(slot: u8, loaded: &LoadedMod) -> [f32; 3] {
    let norm = slot % TEAM_COLOR_PALETTE_SLOTS;
    if norm == 0 {
        return [0.76, 0.78, 0.82];
    }
    if let Some(def) = loaded.manifest.player_colors.get((norm - 1) as usize) {
        return def.rgb;
    }
    DEFAULT_PLAYER_COLORS[(norm - 1) as usize].2
}

pub fn team_color_name_for_slot_in_mod(slot: u8, loaded: &LoadedMod) -> &str {
    let norm = slot % TEAM_COLOR_PALETTE_SLOTS;
    if norm == 0 {
        return "Neutral";
    }
    if let Some(def) = loaded.manifest.player_colors.get((norm - 1) as usize) {
        if !def.name.trim().is_empty() {
            return def.name.as_str();
        }
    }
    DEFAULT_PLAYER_COLORS[(norm - 1) as usize].1
}

/// Resolves `(label, preview_rgb)` for a UI `color_index` (`0` = `"Auto"`, `1..=16` = explicit palette slot).
pub fn resolve_ui_color_swatch<'a>(
    color_index: u8,
    faction_hint: Option<&str>,
    loaded: Option<&'a LoadedMod>,
) -> (&'a str, [f32; 3]) {
    if color_index == 0 {
        let preview_rgb = faction_hint
            .map(str::trim)
            .filter(|f| !f.is_empty() && !f.eq_ignore_ascii_case("random"))
            .map(|f| {
                let slot = default_faction_slot(f);
                match loaded {
                    Some(m) => team_color_rgb_for_slot_in_mod(slot, m),
                    None => team_color_rgb_for_slot(slot),
                }
            })
            .unwrap_or([0.66, 0.74, 0.86]);
        return ("Auto", preview_rgb);
    }
    let slot = 1 + ((color_index - 1) % PLAYER_TEAM_COLOR_COUNT);
    match loaded {
        Some(m) => (
            team_color_name_for_slot_in_mod(slot, m),
            team_color_rgb_for_slot_in_mod(slot, m),
        ),
        None => (team_color_name_for_slot(slot), team_color_rgb_for_slot(slot)),
    }
}

pub fn default_faction_slot(faction_id: &str) -> u8 {
    let low = faction_id.to_ascii_lowercase();
    if low.contains("allies") || low.contains("allied") || low.contains("gdi") {
        1
    } else if low.contains("soviet") || low.contains("nod") || low.contains("red") {
        2
    } else if low.contains("yuri") || low.contains("psi") || low.contains("epsilon") {
        3
    } else {
        4
    }
}

fn default_faction_slot_in_mod(faction_id: &str, loaded: &LoadedMod) -> u8 {
    let builtin = default_faction_slot(faction_id);
    if builtin != 4 {
        return builtin;
    }
    if let Some(fac) = loaded.factions.get(faction_id) {
        let [fr, fg, fb] = fac.color;
        let mut best: Option<(u8, f32)> = None;
        for slot in 1..=PLAYER_TEAM_COLOR_COUNT {
            let [pr, pg, pb] = team_color_rgb_for_slot_in_mod(slot, loaded);
            let dist_sq = (fr - pr).powi(2) + (fg - pg).powi(2) + (fb - pb).powi(2);
            if dist_sq <= 0.08 && best.is_none_or(|(_, b_d)| dist_sq < b_d) {
                best = Some((slot, dist_sq));
            }
        }
        if let Some((slot, _)) = best {
            return slot;
        }
    }
    builtin
}

/// Resolves deterministic palette slot `1..=16` (`0` for neutral `player_id == 0`).
/// - `color_index == 0`: `"Auto (Faction Default)"` (`allies -> 1`, `soviets -> 2`, `yuri -> 3`).
/// - `color_index == 1..=16`: explicit palette slot `1..=16`.
///
/// Explicit choices claim their requested slot first; remaining collisions resolve deterministically across `1..=16`.
pub fn resolve_player_team_color_slot(world: &SimWorld, loaded: &LoadedMod, player_id: u8) -> u8 {
    if player_id == 0 {
        return 0;
    }
    if !world.players.contains_key(&player_id) {
        return 1 + ((player_id.saturating_sub(1)) % PLAYER_TEAM_COLOR_COUNT);
    }

    let mut used = [false; TEAM_COLOR_PALETTE_SLOTS as usize];
    let mut assigned: Option<u8> = None;

    // Pass 1: Explicit `color_index > 0` claims (`1..=16`) in ascending deterministic `pid` order
    for (&pid, econ) in &world.players {
        if pid == 0 || econ.color_index == 0 {
            continue;
        }
        let base = 1 + ((econ.color_index - 1) % PLAYER_TEAM_COLOR_COUNT);
        let mut chosen = base;
        for step in 0..PLAYER_TEAM_COLOR_COUNT {
            let probe = 1 + ((base - 1 + step) % PLAYER_TEAM_COLOR_COUNT);
            if !used[probe as usize] {
                chosen = probe;
                break;
            }
        }
        used[chosen as usize] = true;
        if pid == player_id {
            return chosen;
        }
    }

    // Pass 2: `color_index == 0` Auto (Faction Default) claims across remaining unused `1..=16` slots
    for (&pid, econ) in &world.players {
        if pid == 0 || econ.color_index > 0 {
            continue;
        }
        let base = default_faction_slot_in_mod(&econ.faction, loaded);
        let mut chosen = base;
        for step in 0..PLAYER_TEAM_COLOR_COUNT {
            let probe = 1 + ((base - 1 + step) % PLAYER_TEAM_COLOR_COUNT);
            if !used[probe as usize] {
                chosen = probe;
                break;
            }
        }
        used[chosen as usize] = true;
        if pid == player_id {
            assigned = Some(chosen);
            break;
        }
    }

    assigned.unwrap_or_else(|| 1 + ((player_id.saturating_sub(1)) % PLAYER_TEAM_COLOR_COUNT))
}

pub fn team_color_rgb_for_player(world: &SimWorld, loaded: &LoadedMod, player_id: u8) -> [f32; 3] {
    let slot = resolve_player_team_color_slot(world, loaded, player_id);
    team_color_rgb_for_slot_in_mod(slot, loaded)
}

/// Resolves an accessible `[r, g, b]` team color for `player_id` relative to `local_pid` under `cb_mode`.
pub fn accessible_team_color_rgb_for_player(
    world: &SimWorld,
    loaded: &LoadedMod,
    player_id: u8,
    local_pid: u8,
    cb_mode: ColorblindPaletteMode,
) -> [f32; 3] {
    let slot = resolve_player_team_color_slot(world, loaded, player_id);
    let is_local = player_id == local_pid;
    let is_ally = player_id != 0 && !world.are_hostile(local_pid, player_id);
    let rgba = resolve_accessible_team_color_in_mod(slot, is_local, is_ally, cb_mode, Some(loaded));
    [rgba[0], rgba[1], rgba[2]]
}

pub fn build_team_damage_standard_material_in_mod(
    slot: u8,
    tier: DamageVisualTier,
    loaded: Option<&LoadedMod>,
) -> StandardMaterial {
    let [tr, tg, tb] = match loaded {
        Some(m) => team_color_rgb_for_slot_in_mod(slot, m),
        None => team_color_rgb_for_slot(slot),
    };
    let tint_strength = if slot.is_multiple_of(TEAM_COLOR_PALETTE_SLOTS) {
        0.15
    } else {
        0.46
    };
    let base_r = 1.0 - tint_strength + tr * tint_strength;
    let base_g = 1.0 - tint_strength + tg * tint_strength;
    let base_b = 1.0 - tint_strength + tb * tint_strength;

    match tier {
        DamageVisualTier::Intact => StandardMaterial {
            base_color: Color::srgb(base_r, base_g, base_b),
            emissive: LinearRgba::BLACK,
            perceptual_roughness: 0.56,
            metallic: 0.30,
            ..default()
        },
        DamageVisualTier::Yellow50 => StandardMaterial {
            base_color: Color::srgb(base_r * 0.64, base_g * 0.60, base_b * 0.56),
            emissive: LinearRgba::new(0.12, 0.05, 0.01, 1.0),
            perceptual_roughness: 0.78,
            metallic: 0.18,
            ..default()
        },
        DamageVisualTier::Red25 => StandardMaterial {
            base_color: Color::srgb(base_r * 0.38, base_g * 0.34, base_b * 0.32),
            emissive: LinearRgba::new(0.46, 0.11, 0.03, 1.0),
            perceptual_roughness: 0.92,
            metallic: 0.10,
            ..default()
        },
    }
}

pub fn build_team_damage_standard_material(slot: u8, tier: DamageVisualTier) -> StandardMaterial {
    build_team_damage_standard_material_in_mod(slot, tier, None)
}

#[derive(Resource, Debug, Default, Clone)]
pub struct TeamDamageMaterialPalette {
    entries: HashMap<(u8, DamageVisualTier), Handle<StandardMaterial>>,
}

impl TeamDamageMaterialPalette {
    pub fn get_or_create(
        &mut self,
        materials: &mut Assets<StandardMaterial>,
        slot: u8,
        tier: DamageVisualTier,
    ) -> Handle<StandardMaterial> {
        self.get_or_create_in_mod(materials, slot, tier, None)
    }

    pub fn get_or_create_in_mod(
        &mut self,
        materials: &mut Assets<StandardMaterial>,
        slot: u8,
        tier: DamageVisualTier,
        loaded: Option<&LoadedMod>,
    ) -> Handle<StandardMaterial> {
        let norm_slot = slot % TEAM_COLOR_PALETTE_SLOTS;
        let key = (norm_slot, tier);
        if let Some(existing) = self.entries.get(&key) {
            return existing.clone();
        }
        let handle = materials.add(build_team_damage_standard_material_in_mod(
            norm_slot, tier, loaded,
        ));
        self.entries.insert(key, handle.clone());
        handle
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ra2_faction_palette_slots_deliver_punchy_allied_blue_soviet_red_and_yuri_purple() {
        let allied = team_color_rgb_for_slot(default_faction_slot("allies"));
        let soviet = team_color_rgb_for_slot(default_faction_slot("soviet"));
        let yuri = team_color_rgb_for_slot(default_faction_slot("yuri"));

        assert!(allied[2] >= 0.95 && allied[0] <= 0.12 && allied[2] > allied[1] * 2.0);
        assert!(soviet[0] >= 0.90 && soviet[1] <= 0.10 && soviet[2] <= 0.10);
        assert!(yuri[2] >= 0.90 && yuri[0] >= 0.50 && yuri[1] <= 0.12);
    }

    #[test]
    fn all_16_player_palette_slots_and_colorblind_modes_are_distinct() {
        let mut seen_default = std::collections::BTreeSet::new();
        for slot in 1..=PLAYER_TEAM_COLOR_COUNT {
            let rgb = team_color_rgb_for_slot(slot);
            let q = [
                (rgb[0] * 255.0).round() as u8,
                (rgb[1] * 255.0).round() as u8,
                (rgb[2] * 255.0).round() as u8,
            ];
            assert!(seen_default.insert(q), "duplicate default slot {slot}: {q:?}");
        }
        for mode in [
            ColorblindPaletteMode::Deuteranopia,
            ColorblindPaletteMode::Protanopia,
            ColorblindPaletteMode::Tritanopia,
        ] {
            let mut seen_cb = std::collections::BTreeSet::new();
            for slot in 1..=PLAYER_TEAM_COLOR_COUNT {
                let rgba = resolve_accessible_team_color(slot, false, false, mode);
                let q = [
                    (rgba[0] * 255.0).round() as u8,
                    (rgba[1] * 255.0).round() as u8,
                    (rgba[2] * 255.0).round() as u8,
                ];
                assert!(seen_cb.insert(q), "duplicate {mode:?} slot {slot}: {q:?}");
            }
        }
    }

    #[test]
    fn lod_visibility_ranges_cover_telephoto_default_eye_and_orbital_crossfade() {
        let mut rig = crate::camera::RtsCameraRig::new(128.0, 128.0);
        let default_eye = (rig.compute_transform().translation - rig.focus).length();
        let lod0 = lod0_visibility_range();
        let lod1 = lod1_visibility_range();
        assert!(default_eye > 75.0 && default_eye < lod0.end_margin.start);

        rig.set_zoom_distance(185.0);
        let orbital_eye = (rig.compute_transform().translation - rig.focus).length();
        assert!(orbital_eye > lod1.start_margin.end && orbital_eye < lod1.end_margin.start);
    }
}
