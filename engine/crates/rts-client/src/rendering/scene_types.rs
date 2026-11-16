use super::{CachedArticulatedPart, UnitPbrTeamMaterial};
use bevy::pbr::{CascadeShadowConfig, CascadeShadowConfigBuilder};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Resource, Debug, Clone, Default)]
pub struct StudioDebugOverlayConfig {
    pub show_clearance_grid: bool,
    pub show_flow_fields: bool,
    pub show_ballistic_arcs: bool,
    pub show_ai_threat_heatmap: bool,
    pub show_ai_econ_heatmap: bool,
    pub show_sockets: bool,
    pub inspected_ai_player_id: u8,
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct MapTerrainDirty(pub bool);

/// Tracks fine-grained dirty tile rectangles so terrain brush strokes only rebuild affected
/// `16x16` `TerrainChunkEntity` meshes and splatmap sub-regions instead of despawning the world.
#[derive(Resource, Debug, Default, Clone)]
pub struct MapTerrainDirtyRects {
    pub rects: Vec<([i32; 2], [i32; 2])>,
    pub full_rebuild: bool,
}

impl MapTerrainDirtyRects {
    pub fn mark_rect(&mut self, min_tile: [i32; 2], max_tile: [i32; 2]) {
        if self.full_rebuild {
            return;
        }
        let lo = [min_tile[0].min(max_tile[0]), min_tile[1].min(max_tile[1])];
        let hi = [min_tile[0].max(max_tile[0]), min_tile[1].max(max_tile[1])];
        for (existing_lo, existing_hi) in &mut self.rects {
            let overlaps_or_adjacent = lo[0] <= existing_hi[0] + 1
                && hi[0] >= existing_lo[0] - 1
                && lo[1] <= existing_hi[1] + 1
                && hi[1] >= existing_lo[1] - 1;
            if overlaps_or_adjacent {
                existing_lo[0] = existing_lo[0].min(lo[0]);
                existing_lo[1] = existing_lo[1].min(lo[1]);
                existing_hi[0] = existing_hi[0].max(hi[0]);
                existing_hi[1] = existing_hi[1].max(hi[1]);
                return;
            }
        }
        self.rects.push((lo, hi));
    }

    pub fn mark_all(&mut self) {
        self.full_rebuild = true;
        self.rects.clear();
    }

    pub fn is_empty(&self) -> bool {
        !self.full_rebuild && self.rects.is_empty()
    }

    pub fn clear(&mut self) {
        self.full_rebuild = false;
        self.rects.clear();
    }
}

/// Screen-Space Ambient Occlusion quality setting (`Off`, `Medium`, `High`; defaults to `High`).
#[derive(
    Resource,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    Default,
)]
#[serde(rename_all = "snake_case")]
pub enum SsaoQualitySetting {
    Off,
    Medium,
    #[default]
    High,
}

impl SsaoQualitySetting {
    pub const ALL: [Self; 3] = [Self::Off, Self::Medium, Self::High];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }

    pub fn from_str_lossy(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "off" | "none" | "false" | "0" => Self::Off,
            "medium" | "med" | "1" => Self::Medium,
            _ => Self::High,
        }
    }
}

/// Directional cascade shadow map quality setting (`Off`, `Low`, `Medium`, `High`; defaults to `Medium`).
#[derive(
    Resource,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    Default,
)]
#[serde(rename_all = "snake_case")]
pub enum ShadowQualitySetting {
    Off,
    Low,
    #[default]
    Medium,
    High,
}

impl ShadowQualitySetting {
    pub const ALL: [Self; 4] = [Self::Off, Self::Low, Self::Medium, Self::High];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }

    pub fn from_str_lossy(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "off" | "none" | "false" | "0" => Self::Off,
            "low" | "1" => Self::Low,
            "high" | "ultra" | "3" => Self::High,
            _ => Self::Medium,
        }
    }

    pub fn shadows_enabled(self) -> bool {
        !matches!(self, Self::Off)
    }

    pub fn build_cascade_config(self) -> CascadeShadowConfig {
        self.build_cascade_config_for_altitude(32.0)
    }

    /// Scales directional shadow cascade bounds smoothly with camera altitude (`12m..550m`, `Task 237`)
    /// so orbital strategic zoom (`250m..550m`) retains crisp terrain/structure shadows without cutoff.
    pub fn build_cascade_config_for_altitude(self, camera_distance: f32) -> CascadeShadowConfig {
        let alt_scale = if camera_distance <= 55.0 {
            1.0
        } else {
            (1.0 + (camera_distance - 55.0) / 145.0).clamp(1.0, 4.4)
        };
        match self {
            Self::Off => CascadeShadowConfigBuilder {
                num_cascades: 1,
                minimum_distance: 1.0,
                first_cascade_far_bound: 22.0 * alt_scale,
                maximum_distance: 45.0 * alt_scale,
                overlap_proportion: 0.15,
            }
            .build(),
            Self::Low => CascadeShadowConfigBuilder {
                num_cascades: 2,
                minimum_distance: 1.0,
                first_cascade_far_bound: 24.0 * alt_scale,
                maximum_distance: (85.0 * alt_scale).max(camera_distance * 1.25),
                overlap_proportion: 0.15,
            }
            .build(),
            Self::Medium => CascadeShadowConfigBuilder {
                num_cascades: 3,
                minimum_distance: 1.0,
                first_cascade_far_bound: 28.0 * alt_scale,
                maximum_distance: (135.0 * alt_scale).max(camera_distance * 1.35),
                overlap_proportion: 0.20,
            }
            .build(),
            Self::High => CascadeShadowConfigBuilder {
                num_cascades: 4,
                minimum_distance: 1.0,
                first_cascade_far_bound: 34.0 * alt_scale,
                maximum_distance: (210.0 * alt_scale).max(camera_distance * 1.45),
                overlap_proportion: 0.22,
            }
            .build(),
        }
    }
}

/// Live RMB-hold formation drag preview state rendered by `world_overlays`.
#[derive(Resource, Debug, Clone, Default)]
pub struct FormationDragPreviewState {
    pub active: bool,
    pub anchor_world: [f32; 3],
    pub facing_radians: f32,
    pub line_spacing: f32,
    pub slot_positions: Vec<[f32; 3]>,
    pub attack_move: bool,
}

/// Live RMB freehand curve-formation preview state (`Task 239`, populated by `input/mouse_right.rs`).
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct CurveFormationPreviewResource {
    pub active: bool,
    pub curve_points: Vec<[f32; 3]>,
    pub sample_targets: Vec<([f32; 3], f32)>,
}

/// Interactive Studio PBR & Team-Mask override applied to the 3D Preview Stage (`Task 245`).
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct MaterialPreviewOverrideResource {
    pub enabled: bool,
    pub visual_override: Option<rts_sdk::schema::VisualDef>,
    pub preview_team_slot: u8,
}

#[derive(Resource, Clone)]
pub struct FogOfWarOverlayImage(pub Handle<Image>);

/// Legacy marker kept for backwards-compatible `Without<FogOfWarPlaneEntity>` query filters;
/// floating `+0.52m` FoW plane mesh is no longer spawned (`Task 238` direct World-XZ shader FoW).
#[derive(Component)]
pub struct FogOfWarPlaneEntity;

#[derive(Component)]
pub struct MapEnvironmentEntity;

#[derive(Component)]
pub struct BuildingPointLight;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArticulatedPartKind {
    Turret,
    Barrel,
    Rotor,
}

#[derive(Component)]
pub struct SimVisualEntity {
    pub sim_id: u64,
    pub entity_type_id: String,
    pub building_footprint: Option<[u32; 2]>,
    pub world_pos: Vec3,
    pub spawn_progress: f32,
    pub deconstruct_progress: Option<f32>,
    pub death_anim_progress: Option<f32>,
    pub is_infantry: bool,
    pub cached_articulated_parts: Option<Vec<CachedArticulatedPart>>,
    pub has_3d_turret_node: bool,
}

pub fn player_team_color(player_id: u8) -> Color {
    match player_id {
        1 => Color::srgba(0.20, 0.58, 1.0, 0.78),
        2 => Color::srgba(1.0, 0.22, 0.20, 0.78),
        3 => Color::srgba(0.18, 0.92, 0.42, 0.78),
        4 => Color::srgba(1.0, 0.82, 0.16, 0.78),
        5 => Color::srgba(0.75, 0.32, 1.0, 0.78),
        6 => Color::srgba(0.18, 0.92, 0.96, 0.78),
        7 => Color::srgba(1.0, 0.52, 0.14, 0.78),
        8 => Color::srgba(1.0, 0.25, 0.78, 0.78),
        _ => Color::srgba(0.88, 0.92, 0.98, 0.65),
    }
}

/// Faction variant for `v3_ultra` 3D PBR scaffolding & slewing crane rigs (`assets/models/fx/*_construction_rig.glb`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ConstructionRigFaction {
    #[default]
    Allies,
    Soviets,
    Yuri,
}

impl ConstructionRigFaction {
    pub fn model_rel_path(self) -> &'static str {
        match self {
            Self::Allies => "assets/models/fx/allied_construction_rig.glb",
            Self::Soviets => "assets/models/fx/soviet_construction_rig.glb",
            Self::Yuri => "assets/models/fx/yuri_construction_rig.glb",
        }
    }

    pub fn from_faction_or_entity_id(faction_opt: Option<&str>, entity_type_id: &str) -> Self {
        if let Some(f) = faction_opt {
            let low = f.trim().to_ascii_lowercase();
            if low.contains("yuri") {
                return Self::Yuri;
            }
            if low.contains("soviet")
                || matches!(low.as_str(), "russia" | "iraq" | "libya" | "cuba")
            {
                return Self::Soviets;
            }
            if low.contains("allie")
                || matches!(
                    low.as_str(),
                    "usa" | "america" | "uk" | "france" | "germany" | "korea"
                )
            {
                return Self::Allies;
            }
        }
        let eid = entity_type_id.to_ascii_lowercase();
        if eid.starts_with("yuri_") || eid.starts_with("psychic_") {
            Self::Yuri
        } else if eid.starts_with("soviet_") {
            Self::Soviets
        } else {
            Self::Allies
        }
    }
}

/// Discovered articulated node handles inside a spawned `*_construction_rig.glb` hierarchy.
#[derive(Debug, Clone, Default)]
pub struct DiscoveredRigNodes {
    pub corner_nw: Option<Entity>,
    pub corner_ne: Option<Entity>,
    pub corner_se: Option<Entity>,
    pub corner_sw: Option<Entity>,
    pub span_n: Option<Entity>,
    pub span_s: Option<Entity>,
    pub span_e: Option<Entity>,
    pub span_w: Option<Entity>,
    pub crane_mast: Option<Entity>,
    pub crane_jib: Option<Entity>,
    pub spark_muzzle: Option<Entity>,
}

/// Live 3D construction scaffolding & crane entity tracking a building's buildup or sell/undeploy fold-down.
#[derive(Component, Debug, Clone)]
pub struct BuildingConstructionRig {
    pub sim_id: u64,
    pub rig_faction: ConstructionRigFaction,
    pub cached_player_id: u8,
    pub cached_color_slot: u8,
    pub nodes_discovered: bool,
    pub nodes: DiscoveredRigNodes,
    pub mesh_entities: Vec<(Entity, bool)>,
    pub orig_std_materials: Vec<Option<Handle<StandardMaterial>>>,
    pub material_handles: Vec<Handle<UnitPbrTeamMaterial>>,
    pub applied_color_key: Option<([u8; 3], bool)>,
    pub spark_timer: f32,
}

/// Caches preloaded rig scene handles and last-known building owners so deconstructing (`sold` /
/// `undeployed`) buildings retain their exact 16-color player palette after `SimEntity` removal.
#[derive(Resource, Default)]
pub struct ConstructionRigCache {
    pub loaded_mod_root: Option<std::path::PathBuf>,
    pub scene_handles: HashMap<ConstructionRigFaction, Handle<Scene>>,
    pub known_building_owners: HashMap<u64, (u8, u8, ConstructionRigFaction)>,
}

/// Computes canonical world-meter half-extents `(hw, hd)` for a building footprint `[fw, fh]`.
#[inline]
pub fn footprint_rig_half_extents(footprint: [u32; 2]) -> (f32, f32) {
    let hw = if footprint[0] <= 1 {
        0.875
    } else {
        (footprint[0] as f32) * 0.90
    };
    let hd = if footprint[1] <= 1 {
        0.875
    } else {
        (footprint[1] as f32) * 0.90
    };
    (hw, hd)
}

/// Computes `(y_scaf_local, y_crane_local, crane_retract_damp)` for `progress` in `0.0..1.0`.
#[inline]
pub fn compute_rig_vertical_profile(progress_01: f32) -> (f32, f32, f32) {
    let p = progress_01.clamp(0.0, 1.0);
    if p <= 0.86 {
        let lin = (p / 0.72).clamp(0.0, 1.0);
        (-(1.0 - lin) * 3.2, -(1.0 - lin) * 3.6, 1.0)
    } else {
        let t = ((p - 0.86) / (0.996 - 0.86)).clamp(0.0, 1.0);
        let smooth = t * t * (3.0 - 2.0 * t);
        (-smooth * 3.6, -smooth * 4.4, 1.0 - smooth)
    }
}

#[cfg(test)]
mod rig_math_tests {
    use super::*;

    #[test]
    fn footprint_half_extents_and_vertical_profile_follow_ra2_contract() {
        let (hx1, hz1) = footprint_rig_half_extents([1, 1]);
        assert!((hx1 - 0.875).abs() < 1e-5 && (hz1 - 0.875).abs() < 1e-5);
        let (hx2, hz2) = footprint_rig_half_extents([2, 2]);
        assert!((hx2 - 1.80).abs() < 1e-5 && (hz2 - 1.80).abs() < 1e-5);
        let (hx3, hz3) = footprint_rig_half_extents([3, 3]);
        assert!((hx3 - 2.70).abs() < 1e-5 && (hz3 - 2.70).abs() < 1e-5);
        let (hx32, hz32) = footprint_rig_half_extents([3, 2]);
        assert!((hx32 - 2.70).abs() < 1e-5 && (hz32 - 1.80).abs() < 1e-5);

        let (y0, yc0, d0) = compute_rig_vertical_profile(0.0);
        assert!((y0 + 3.2).abs() < 1e-4 && (yc0 + 3.6).abs() < 1e-4 && d0 == 1.0);
        let (yfull, ycfull, dfull) = compute_rig_vertical_profile(0.76);
        assert!(yfull.abs() < 1e-5 && ycfull.abs() < 1e-5 && dfull == 1.0);
        let (yret, ycret, dret) = compute_rig_vertical_profile(0.996);
        assert!((yret + 3.6).abs() < 1e-3 && (ycret + 4.4).abs() < 1e-3 && dret < 1e-3);

        assert_eq!(
            ConstructionRigFaction::from_faction_or_entity_id(Some("yuri"), "yuri_conyard"),
            ConstructionRigFaction::Yuri
        );
        assert_eq!(
            ConstructionRigFaction::from_faction_or_entity_id(Some("soviets"), "soviet_conyard"),
            ConstructionRigFaction::Soviets
        );
    }
}
