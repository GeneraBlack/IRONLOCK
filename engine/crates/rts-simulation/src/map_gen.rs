use crate::world::MapSymmetryMode;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    BuildingRole, InitialBuildingSpawn, InitialUnitSpawn, MapDef, MapPropDef, ObstacleRectDef,
    OrePatchDef, PlayerSpawnDef, PropKind, UnitRole,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapGenConfig {
    pub id: String,
    pub name: String,
    pub seed: u64,
    pub width: u32,
    pub height: u32,
    pub player_count: u8,
    pub symmetry: MapSymmetryMode,
    pub ore_patches_per_player: u32,
}

impl Default for MapGenConfig {
    fn default() -> Self {
        Self {
            id: "proc_skirmish".to_string(),
            name: "Procedural Tournament Arena".to_string(),
            seed: 42,
            width: 64,
            height: 64,
            player_count: 2,
            symmetry: MapSymmetryMode::Rotational2Way,
            ore_patches_per_player: 2,
        }
    }
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_add(0x9e3779b97f4a7c15),
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut z = self.state;
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    fn range_i32(&mut self, min_incl: i32, max_incl: i32) -> i32 {
        if max_incl <= min_incl {
            return min_incl;
        }
        let span = (max_incl - min_incl + 1) as u64;
        min_incl + (self.next_u64() % span) as i32
    }
}

fn symmetric_points(
    pt: [i32; 2],
    w: i32,
    h: i32,
    symmetry: MapSymmetryMode,
    player_count: u8,
) -> Vec<[i32; 2]> {
    let mx = w - 1 - pt[0];
    let mz = h - 1 - pt[1];
    let mut out = vec![pt];
    match symmetry {
        MapSymmetryMode::MirrorX => out.push([mx, pt[1]]),
        MapSymmetryMode::MirrorZ => out.push([pt[0], mz]),
        MapSymmetryMode::Rotational4Way => {
            out.push([mx, mz]);
            out.push([pt[1].clamp(4, w - 5), (w - 1 - pt[0]).clamp(4, h - 5)]);
            out.push([
                (w - 1 - pt[1]).clamp(4, w - 5),
                pt[0].clamp(4, h - 5),
            ]);
        }
        MapSymmetryMode::Rotational2Way | MapSymmetryMode::Off => {
            out.push([mx, mz]);
            if player_count >= 4 {
                out.push([mx, pt[1]]);
                out.push([pt[0], mz]);
            }
        }
    }
    out.dedup();
    out
}

/// Generates a deterministic, tournament-balanced RTS skirmish `MapDef` from `config`
/// that is guaranteed to pass `SimWorld::validate_map_playability()`.
pub fn generate_procedural_map(config: &MapGenConfig, loaded: &LoadedMod) -> MapDef {
    let w = config.width.clamp(40, 160) as i32;
    let h = config.height.clamp(40, 160) as i32;
    let mut rng = SplitMix64::new(config.seed);

    let faction_ids: Vec<String> = if loaded.factions.is_empty() {
        vec!["allies".to_string(), "soviets".to_string()]
    } else {
        loaded.factions.keys().cloned().collect()
    };

    // Candidate symmetric spawn coordinates around the arena perimeter
    let margin_x = 8 + rng.range_i32(0, 3);
    let margin_z = 8 + rng.range_i32(0, 3);
    let mid_x = w / 2;
    let mid_z = h / 2;
    let spawn_coords: Vec<[i32; 2]> = vec![
        [margin_x, margin_z],
        [w - 1 - margin_x, h - 1 - margin_z],
        [w - 1 - margin_x, margin_z],
        [margin_x, h - 1 - margin_z],
        [mid_x, margin_z],
        [mid_x, h - 1 - margin_z],
        [margin_x, mid_z],
        [w - 1 - margin_x, mid_z],
    ];

    let count = (config.player_count.clamp(2, 8) as usize).min(spawn_coords.len());
    let mut spawns = Vec::with_capacity(count);
    let mut ore_patches = Vec::new();
    let mut gem_patches = Vec::new();

    for (idx, &base_tile) in spawn_coords.iter().take(count).enumerate() {
        let pid = (idx + 1) as u8;
        let faction = faction_ids[idx % faction_ids.len()].clone();

        let conyard_id = loaded
            .buildings
            .values()
            .find(|b| {
                b.role == BuildingRole::ConstructionYard && b.is_available_to_faction(&faction)
            })
            .or_else(|| {
                loaded
                    .buildings
                    .values()
                    .find(|b| b.role == BuildingRole::ConstructionYard)
            })
            .map(|b| b.id.clone());

        let harvester_id = loaded
            .units
            .values()
            .find(|u| u.role == UnitRole::Harvester && u.is_available_to_faction(&faction))
            .or_else(|| {
                loaded
                    .units
                    .values()
                    .find(|u| u.role == UnitRole::Harvester)
            })
            .map(|u| u.id.clone());

        let scout_id = loaded
            .units
            .values()
            .find(|u| u.role == UnitRole::Infantry && u.is_available_to_faction(&faction))
            .map(|u| u.id.clone());

        let mut initial_buildings = Vec::new();
        if let Some(cid) = conyard_id {
            initial_buildings.push(InitialBuildingSpawn {
                building_id: cid,
                tile_pos: base_tile,
            });
        }

        let mut initial_units = Vec::new();
        if let Some(hid) = harvester_id {
            initial_units.push(InitialUnitSpawn {
                unit_id: hid,
                tile_pos: [
                    (base_tile[0] + 4).clamp(2, w - 3),
                    base_tile[1].clamp(2, h - 3),
                ],
            });
        }
        if let Some(sid) = scout_id {
            initial_units.push(InitialUnitSpawn {
                unit_id: sid,
                tile_pos: [
                    base_tile[0].clamp(2, w - 3),
                    (base_tile[1] + 4).clamp(2, h - 3),
                ],
            });
        }

        spawns.push(PlayerSpawnDef {
            player_id: pid,
            faction,
            sub_faction: None,
            base_tile,
            team_id: 0,
            initial_buildings,
            initial_units,
        });

        // Natural starting ore patch near each player's base
        let dir_x = if base_tile[0] < mid_x { 6 } else { -6 };
        let dir_z = if base_tile[1] < mid_z { 5 } else { -5 };
        let natural_ore = [
            (base_tile[0] + dir_x).clamp(5, w - 6),
            (base_tile[1] + dir_z).clamp(5, h - 6),
        ];
        ore_patches.push(OrePatchDef {
            position: natural_ore,
            radius: 3,
            amount_per_tile: 750,
        });

        if config.ore_patches_per_player >= 2 {
            let exp_ore = [
                (base_tile[0] + dir_x * 2).clamp(6, w - 7),
                base_tile[1].clamp(6, h - 7),
            ];
            ore_patches.push(OrePatchDef {
                position: exp_ore,
                radius: 2,
                amount_per_tile: 650,
            });
        }
    }

    // Central high-yield Gem expansion
    gem_patches.push(OrePatchDef {
        position: [mid_x, mid_z],
        radius: 3,
        amount_per_tile: 1100,
    });

    // Symmetric tactical obstacles & props (avoiding spawn areas and center corridor)
    let mut obstacles = Vec::new();
    let mut props = Vec::new();
    let seed_obs = [
        (mid_x - 7).clamp(14, w - 15),
        (mid_z - 5).clamp(14, h - 15),
    ];
    for pt in symmetric_points(seed_obs, w, h, config.symmetry, config.player_count) {
        let too_close_to_spawn = spawns.iter().any(|s| {
            (s.base_tile[0] - pt[0]).abs() <= 9 && (s.base_tile[1] - pt[1]).abs() <= 9
        });
        if !too_close_to_spawn && ((pt[0] - mid_x).abs() > 4 || (pt[1] - mid_z).abs() > 4) {
            obstacles.push(ObstacleRectDef {
                min: pt,
                max: [(pt[0] + 1).min(w - 3), (pt[1] + 1).min(h - 3)],
            });
            props.push(MapPropDef {
                kind: PropKind::Tree,
                tile_pos: [(pt[0] - 1).max(2), pt[1]],
            });
        }
    }

    MapDef {
        id: config.id.clone(),
        name: format!("{} (Seed {})", config.name, config.seed),
        width: w as u32,
        height: h as u32,
        tile_size: 2.0,
        mission_objective: None,
        ore_regen_per_sec: 4.0,
        ore_patches,
        gem_patches,
        obstacles,
        water_rects: Vec::new(),
        bridge_rects: Vec::new(),
        elevation_rects: Vec::new(),
        ramp_rects: Vec::new(),
        preplaced_entities: Vec::new(),
        props,
        trigger_zones: Vec::new(),
        spawns,
    }
}
