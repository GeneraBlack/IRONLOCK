use super::*;
use crate::audio::{SoundEffectKind, SoundSettingsResource};
use crate::{ModResource, SimWorldResource};
use rts_sdk::schema::{BuildingRole, CrateKind};
use rts_simulation::command::GameCommand;

pub(super) fn apply_live_sandbox_brush_click(
    shift_held: bool,
    interaction: &mut ClientInteractionState,
    sim_res: &mut SimWorldResource,
    sound_res: &mut SoundSettingsResource,
    mod_res: &ModResource,
) {
    let t = interaction.hovered_tile;
    if !sim_res.0.grid.in_bounds(t) {
        return;
    }
    let target_tiles = sim_res.0.expand_editor_brush_tiles(
        t,
        interaction.map_editor_brush_size,
        interaction.map_editor_symmetry,
    );
    let center_sym_tiles = sim_res.0.expand_editor_brush_tiles(
        t,
        MapEditorBrushSize::Size1x1,
        interaction.map_editor_symmetry,
    );
    match interaction.map_editor_brush {
        MapEditorBrush::Off => {}
        MapEditorBrush::PaintOre => {
            for pt in target_tiles {
                sim_res.0.grid.ore_tiles.insert(pt, 1500);
            }
        }
        MapEditorBrush::PaintHighGround => {
            for pt in target_tiles {
                sim_res.0.grid.elevation_tiles.insert(pt);
            }
        }
        MapEditorBrush::PaintRamp => {
            for pt in target_tiles {
                sim_res.0.grid.ramp_tiles.insert(pt);
            }
        }
        MapEditorBrush::PaintWater => {
            for pt in target_tiles {
                sim_res.0.grid.water_tiles.insert(pt);
                sim_res.0.grid.blocked_tiles.insert(pt);
            }
        }
        MapEditorBrush::PaintBridge => {
            for pt in target_tiles {
                sim_res.0.grid.bridge_tiles.insert(pt);
                sim_res.0.grid.blocked_tiles.remove(&pt);
            }
        }
        MapEditorBrush::PaintRock => {
            for pt in target_tiles {
                sim_res.0.grid.blocked_tiles.insert(pt);
            }
        }
        MapEditorBrush::PaintNeutralBunker => {
            let neutral_b_id = mod_res
                .0
                .buildings
                .values()
                .find(|b| b.container.capacity > 0 && b.role == BuildingRole::Defense)
                .or_else(|| {
                    mod_res
                        .0
                        .buildings
                        .values()
                        .find(|b| b.container.capacity > 0)
                })
                .or_else(|| mod_res.0.buildings.values().next())
                .map(|b| b.id.clone());
            if let Some(bid) = neutral_b_id {
                for pt in center_sym_tiles {
                    if !sim_res.0.grid.blocked_tiles.contains(&pt) {
                        sim_res.0.spawn_building(0, &bid, pt, &mod_res.0, false);
                    }
                }
            }
        }
        MapEditorBrush::PaintCrate => {
            let kinds = [
                CrateKind::BonusCredits,
                CrateKind::HealAll,
                CrateKind::VeterancyPromo,
                CrateKind::FirepowerBoost,
                CrateKind::SpeedBoost,
                CrateKind::RevealMap,
                CrateKind::SalvageScrap,
                CrateKind::CloakStealth,
            ];
            for pt in center_sym_tiles {
                let k = kinds[(sim_res.0.next_crate_id as usize) % kinds.len()];
                sim_res.0.spawn_crate(pt, k);
            }
        }
        MapEditorBrush::SpawnUnit => {
            let mut unit_keys: Vec<String> = mod_res.0.units.keys().cloned().collect();
            unit_keys.sort();
            if !unit_keys.is_empty() {
                let uid = unit_keys[interaction.sandbox_unit_index % unit_keys.len()].clone();
                for (idx, pt) in center_sym_tiles.into_iter().enumerate() {
                    let spawn_pid = if shift_held || idx > 0 {
                        if interaction.local_player_id == 1 {
                            2
                        } else {
                            1
                        }
                    } else {
                        interaction.local_player_id
                    };
                    sim_res.0.push_command(GameCommand::SpawnSandboxUnit {
                        player_id: spawn_pid,
                        unit_id: uid.clone(),
                        tile_pos: pt,
                    });
                }
            }
        }
        MapEditorBrush::PlaceBuilding => {
            let mut b_keys: Vec<String> = mod_res.0.buildings.keys().cloned().collect();
            b_keys.sort();
            if !b_keys.is_empty() {
                let bid = b_keys[interaction.sandbox_building_index % b_keys.len()].clone();
                for (idx, pt) in center_sym_tiles.into_iter().enumerate() {
                    let spawn_pid = if shift_held {
                        0
                    } else if idx > 0 {
                        if interaction.local_player_id == 1 {
                            2
                        } else {
                            1
                        }
                    } else {
                        interaction.local_player_id
                    };
                    sim_res
                        .0
                        .spawn_building(spawn_pid, &bid, pt, &mod_res.0, false);
                }
            }
        }
        MapEditorBrush::PlacePlayerSpawn => {
            let conyard_id = mod_res
                .0
                .buildings
                .values()
                .find(|b| b.role == BuildingRole::ConstructionYard)
                .map(|b| b.id.clone());
            if let Some(cid) = conyard_id {
                for (idx, pt) in center_sym_tiles.into_iter().enumerate() {
                    let pid = ((idx as u8) % 8) + 1;
                    sim_res.0.spawn_building(pid, &cid, pt, &mod_res.0, false);
                }
            }
        }
        MapEditorBrush::PlaceTriggerZone => {
            let zone_id = format!("editor_zone_{}", sim_res.0.trigger_zones.len() + 1);
            sim_res
                .0
                .trigger_zones
                .push(rts_sdk::schema::TriggerZoneDef {
                    id: zone_id.clone(),
                    min: [t[0] - 2, t[1] - 2],
                    max: [t[0] + 2, t[1] + 2],
                    once: true,
                    script_hook: String::new(),
                    condition: None,
                    actions: vec![rts_sdk::schema::TriggerAction::SetObjective {
                        text: format!("EVA: Trigger zone '{zone_id}' reached!"),
                    }],
                });
            interaction.push_eva_alert(format!(
                "EVA: Trigger zone '{zone_id}' placed at [{}, {}]!",
                t[0], t[1]
            ));
        }
        MapEditorBrush::ClearTile => {
            for pt in target_tiles {
                sim_res.0.grid.ore_tiles.remove(&pt);
                sim_res.0.grid.elevation_tiles.remove(&pt);
                sim_res.0.grid.ramp_tiles.remove(&pt);
                sim_res.0.grid.water_tiles.remove(&pt);
                sim_res.0.grid.bridge_tiles.remove(&pt);
                sim_res.0.grid.blocked_tiles.remove(&pt);
                sim_res.0.crates.retain(|c| c.tile != pt);
            }
        }
    }
    interaction.environment_dirty = true;
    sound_res.trigger(SoundEffectKind::UiClick);
}
