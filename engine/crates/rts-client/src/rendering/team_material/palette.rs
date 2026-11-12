use bevy::prelude::*;
use bevy::render::view::visibility::VisibilityRange;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::SimWorld;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

/// Resolves an accessible `[r, g, b, a]` team color for `slot_idx` under the active `ColorblindPaletteMode`.
pub fn resolve_accessible_team_color(
    slot_idx: u8,
    is_local: bool,
    is_ally: bool,
    mode: ColorblindPaletteMode,
) -> [f32; 4] {
    if slot_idx == 0 {
        return [0.78, 0.80, 0.84, 1.0];
    }
    let idx = (slot_idx % TEAM_COLOR_PALETTE_SLOTS) as usize;
    let rgb = match mode {
        ColorblindPaletteMode::Off => team_color_rgb_for_slot(slot_idx),
        ColorblindPaletteMode::AllyEnemyHighContrast => {
            if is_local {
                [0.14, 0.72, 1.00]
            } else if is_ally {
                [0.98, 0.86, 0.18]
            } else {
                [1.00, 0.22, 0.14]
            }
        }
        ColorblindPaletteMode::Deuteranopia => match idx {
            1 => [0.00, 0.45, 0.70],
            2 => [0.84, 0.37, 0.00],
            3 => [0.80, 0.47, 0.65],
            4 => [0.94, 0.89, 0.26],
            5 => [0.34, 0.71, 0.91],
            6 => [0.90, 0.62, 0.00],
            7 => [0.15, 0.25, 0.85],
            8 => [0.98, 0.74, 0.48],
            _ => [0.68, 0.82, 0.96],
        },
        ColorblindPaletteMode::Protanopia => match idx {
            1 => [0.08, 0.48, 0.86],
            2 => [0.92, 0.62, 0.05],
            3 => [0.42, 0.76, 0.96],
            4 => [0.96, 0.90, 0.28],
            5 => [0.22, 0.30, 0.78],
            6 => [0.82, 0.46, 0.12],
            7 => [0.64, 0.56, 0.92],
            8 => [0.95, 0.78, 0.52],
            _ => [0.72, 0.86, 0.96],
        },
        ColorblindPaletteMode::Tritanopia => match idx {
            1 => [0.00, 0.66, 0.68],
            2 => [0.92, 0.18, 0.22],
            3 => [0.96, 0.54, 0.66],
            4 => [0.58, 0.88, 0.88],
            5 => [0.12, 0.42, 0.46],
            6 => [0.84, 0.14, 0.42],
            7 => [0.98, 0.76, 0.78],
            8 => [0.26, 0.78, 0.72],
            _ => [0.76, 0.86, 0.88],
        },
    };
    [rgb[0], rgb[1], rgb[2], 1.0]
}

pub const MAX_SHARED_TEAM_DAMAGE_MATERIALS: usize = 32;
pub const TEAM_COLOR_PALETTE_SLOTS: u8 = 10;

pub fn lod0_visibility_range() -> VisibilityRange {
    VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: 55.0..62.0,
        use_aabb: false,
    }
}

pub fn lod1_visibility_range() -> VisibilityRange {
    VisibilityRange {
        start_margin: 55.0..62.0,
        end_margin: 280.0..300.0,
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
    match slot % TEAM_COLOR_PALETTE_SLOTS {
        0 => [0.78, 0.80, 0.84],
        1 => [0.22, 0.56, 1.00],
        2 => [0.98, 0.20, 0.18],
        3 => [0.76, 0.24, 0.98],
        4 => [0.98, 0.80, 0.16],
        5 => [0.18, 0.88, 0.38],
        6 => [0.16, 0.90, 0.94],
        7 => [1.00, 0.48, 0.12],
        8 => [0.96, 0.26, 0.70],
        _ => [0.64, 0.84, 0.98],
    }
}

fn default_faction_slot(faction_id: &str) -> u8 {
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

pub fn resolve_player_team_color_slot(world: &SimWorld, loaded: &LoadedMod, player_id: u8) -> u8 {
    if player_id == 0 {
        return 0;
    }
    let _ = loaded;
    let Some(econ) = world.players.get(&player_id) else {
        return 1 + ((player_id.saturating_sub(1)) % 9);
    };

    let candidate_for = |pid: u8, p: &rts_simulation::world::PlayerEconomy| -> u8 {
        if p.color_index > 0 {
            1 + ((p.color_index.saturating_sub(1)) % 9)
        } else {
            let f_slot = default_faction_slot(&p.faction);
            if f_slot > 0 {
                f_slot
            } else {
                1 + ((pid.saturating_sub(1)) % 9)
            }
        }
    };

    let mut chosen = candidate_for(player_id, econ);
    let mut used_by_earlier = [false; TEAM_COLOR_PALETTE_SLOTS as usize];
    for (&other_pid, other_econ) in &world.players {
        if other_pid == 0 || other_pid >= player_id {
            continue;
        }
        let mut earlier_slot = candidate_for(other_pid, other_econ);
        for step in 0..9u8 {
            let probe = 1 + ((earlier_slot.saturating_sub(1) + step) % 9);
            if !used_by_earlier[probe as usize] {
                earlier_slot = probe;
                break;
            }
        }
        used_by_earlier[earlier_slot as usize] = true;
    }

    if used_by_earlier[chosen as usize] {
        for step in 1..=9u8 {
            let probe = 1 + ((chosen.saturating_sub(1) + step) % 9);
            if !used_by_earlier[probe as usize] {
                chosen = probe;
                break;
            }
        }
    }
    chosen
}

pub fn team_color_rgb_for_player(world: &SimWorld, loaded: &LoadedMod, player_id: u8) -> [f32; 3] {
    let slot = resolve_player_team_color_slot(world, loaded, player_id);
    team_color_rgb_for_slot(slot)
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
    let rgba = resolve_accessible_team_color(slot, is_local, is_ally, cb_mode);
    [rgba[0], rgba[1], rgba[2]]
}

pub fn build_team_damage_standard_material(slot: u8, tier: DamageVisualTier) -> StandardMaterial {
    let [tr, tg, tb] = team_color_rgb_for_slot(slot);
    let tint_strength = if slot == 0 { 0.15 } else { 0.46 };
    let base_r = 1.0 - tint_strength + tr * tint_strength;
    let base_g = 1.0 - tint_strength + tg * tint_strength;
    let base_b = 1.0 - tint_strength + tb * tint_strength;

    match tier {
        DamageVisualTier::Intact => StandardMaterial {
            base_color: Color::srgb(base_r, base_g, base_b),
            emissive: LinearRgba::new(tr * 0.16, tg * 0.16, tb * 0.16, 1.0),
            perceptual_roughness: 0.56,
            metallic: 0.30,
            ..default()
        },
        DamageVisualTier::Yellow50 => StandardMaterial {
            base_color: Color::srgb(base_r * 0.64, base_g * 0.60, base_b * 0.56),
            emissive: LinearRgba::new(tr * 0.08 + 0.12, tg * 0.08 + 0.05, tb * 0.08 + 0.01, 1.0),
            perceptual_roughness: 0.78,
            metallic: 0.18,
            ..default()
        },
        DamageVisualTier::Red25 => StandardMaterial {
            base_color: Color::srgb(base_r * 0.38, base_g * 0.34, base_b * 0.32),
            emissive: LinearRgba::new(0.46 + tr * 0.06, 0.11 + tg * 0.04, 0.03 + tb * 0.04, 1.0),
            perceptual_roughness: 0.92,
            metallic: 0.10,
            ..default()
        },
    }
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
        let norm_slot = slot % TEAM_COLOR_PALETTE_SLOTS;
        let key = (norm_slot, tier);
        if let Some(existing) = self.entries.get(&key) {
            return existing.clone();
        }
        let handle = materials.add(build_team_damage_standard_material(norm_slot, tier));
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
