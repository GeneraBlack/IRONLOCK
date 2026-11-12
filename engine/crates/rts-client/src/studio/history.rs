use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    AiProfileDef, AudioCatalog, BuildingDef, CampaignDef, CommanderPowerTreeDef, FactionDef,
    LocaleCatalog, MapDef, ResourceTypeDef, UnitDef, UpgradeDef, VfxEffectDef, WeaponDef,
};
use rts_simulation::pathfinding::CollisionGrid;
use rts_simulation::world::{
    EnvironmentPreset, SimCivilianStructure, SimEntity, SimProp, SimWorld,
};
use std::collections::{BTreeMap, VecDeque};

pub const MAX_EDITOR_HISTORY_STEPS: usize = 64;

/// Snapshot of a single `SimEntity`'s transform (`id, position, tile_origin, facing_radians`).
pub type EntityTransformSnap = (u64, [f32; 3], [i32; 2], f32);

/// Full reversible snapshot of terrain grid, continuous heightmap, splatmap, entities, props, civilian structures, and resource nodes.
#[derive(Clone, Debug)]
pub struct TerrainStrokeState {
    pub grid: CollisionGrid,
    pub map_def: MapDef,
    pub resource_nodes: BTreeMap<String, BTreeMap<[i32; 2], u32>>,
    pub civilian_structures: Vec<SimCivilianStructure>,
    pub props: Vec<SimProp>,
    pub next_prop_id: u64,
    pub entities: BTreeMap<u64, SimEntity>,
    pub next_entity_id: u64,
    pub entity_tags: BTreeMap<u64, String>,
    pub playable_bounds: Option<([i32; 2], [i32; 2])>,
    pub environment_preset: EnvironmentPreset,
}

impl TerrainStrokeState {
    pub fn capture(world: &SimWorld, loaded: &LoadedMod) -> Self {
        Self {
            grid: world.grid.clone(),
            map_def: loaded.entry_map.clone(),
            resource_nodes: world.resource_nodes.clone(),
            civilian_structures: world.civilian_structures.clone(),
            props: world.props.clone(),
            next_prop_id: world.next_prop_id,
            entities: world.entities.clone(),
            next_entity_id: world.next_entity_id,
            entity_tags: world.entity_tags.clone(),
            playable_bounds: world.playable_bounds,
            environment_preset: world.environment_preset,
        }
    }

    pub fn restore(&self, world: &mut SimWorld, loaded: &mut LoadedMod) {
        world.grid = self.grid.clone();
        world.resource_nodes = self.resource_nodes.clone();
        world.civilian_structures = self.civilian_structures.clone();
        world.props = self.props.clone();
        world.next_prop_id = self.next_prop_id;
        world.entities = self.entities.clone();
        world.next_entity_id = self.next_entity_id;
        world.entity_tags = self.entity_tags.clone();
        world.playable_bounds = self.playable_bounds;
        world.environment_preset = self.environment_preset;
        world.trigger_zones = self.map_def.trigger_zones.clone();
        world.trigger_graphs = self.map_def.trigger_graphs.clone();
        loaded.entry_map = self.map_def.clone();
        loaded
            .maps
            .insert(loaded.entry_map.id.clone(), loaded.entry_map.clone());
    }
}

/// Reversible editor action recorded onto `EditorHistory` (`Ctrl+Z` / `Ctrl+Y`).
#[derive(Clone, Debug)]
pub enum EditorCommand {
    TerrainStroke {
        label: String,
        before: Box<TerrainStrokeState>,
        after: Box<TerrainStrokeState>,
    },
    SpawnEntity {
        entity: Box<SimEntity>,
    },
    DeleteEntity {
        entity: Box<SimEntity>,
    },
    MoveEntity {
        entity_id: u64,
        old_pos: [f32; 3],
        new_pos: [f32; 3],
    },
    TransformEntities {
        label: String,
        before: Vec<EntityTransformSnap>,
        after: Vec<EntityTransformSnap>,
    },
    EditUnitDef {
        id: String,
        before: Box<UnitDef>,
        after: Box<UnitDef>,
    },
    EditBuildingDef {
        id: String,
        before: Box<BuildingDef>,
        after: Box<BuildingDef>,
    },
    EditWeaponDef {
        id: String,
        before: Box<WeaponDef>,
        after: Box<WeaponDef>,
    },
    EditUpgradeDef {
        id: String,
        before: Box<UpgradeDef>,
        after: Box<UpgradeDef>,
    },
    EditCommanderTreeDef {
        id: String,
        before: Box<CommanderPowerTreeDef>,
        after: Box<CommanderPowerTreeDef>,
    },
    EditFactionDef {
        id: String,
        before: Box<FactionDef>,
        after: Box<FactionDef>,
    },
    EditResourceDef {
        id: String,
        before: Box<ResourceTypeDef>,
        after: Box<ResourceTypeDef>,
    },
    EditAiProfileDef {
        id: String,
        before: Box<AiProfileDef>,
        after: Box<AiProfileDef>,
    },
    EditCampaignDef {
        id: String,
        before: Box<CampaignDef>,
        after: Box<CampaignDef>,
    },
    EditAudioCatalog {
        before: Box<AudioCatalog>,
        after: Box<AudioCatalog>,
    },
    EditLocaleCatalog {
        lang: String,
        before: Box<LocaleCatalog>,
        after: Box<LocaleCatalog>,
    },
    EditVfxEffectDef {
        id: String,
        before: Option<Box<VfxEffectDef>>,
        after: Option<Box<VfxEffectDef>>,
    },
    EditMapDef {
        label: String,
        before: Box<MapDef>,
        after: Box<MapDef>,
    },
}

impl EditorCommand {
    pub fn label(&self) -> String {
        match self {
            Self::TerrainStroke { label, .. } => label.clone(),
            Self::SpawnEntity { entity } => format!("Spawn Entity #{}", entity.id),
            Self::DeleteEntity { entity } => format!("Delete Entity #{}", entity.id),
            Self::MoveEntity { entity_id, .. } => format!("Move Entity #{entity_id}"),
            Self::TransformEntities { label, .. } => label.clone(),
            Self::EditUnitDef { id, .. } => format!("Edit Unit '{id}'"),
            Self::EditBuildingDef { id, .. } => format!("Edit Building '{id}'"),
            Self::EditWeaponDef { id, .. } => format!("Edit Weapon '{id}'"),
            Self::EditUpgradeDef { id, .. } => format!("Edit Upgrade '{id}'"),
            Self::EditCommanderTreeDef { id, .. } => format!("Edit Commander Tree '{id}'"),
            Self::EditFactionDef { id, .. } => format!("Edit Faction '{id}'"),
            Self::EditResourceDef { id, .. } => format!("Edit Resource '{id}'"),
            Self::EditAiProfileDef { id, .. } => format!("Edit AI Profile '{id}'"),
            Self::EditCampaignDef { id, .. } => format!("Edit Campaign '{id}'"),
            Self::EditAudioCatalog { .. } => "Edit Audio Catalog".to_string(),
            Self::EditLocaleCatalog { lang, .. } => format!("Edit Locale '{lang}'"),
            Self::EditVfxEffectDef { id, .. } => format!("Edit VFX Effect '{id}'"),
            Self::EditMapDef { label, .. } => label.clone(),
        }
    }

    /// Applies the inverse of `self`, returning `(terrain_dirty, sdk_dirty)`.
    pub fn apply_undo(&self, world: &mut SimWorld, loaded: &mut LoadedMod) -> (bool, bool) {
        match self {
            Self::TerrainStroke { before, .. } => {
                before.restore(world, loaded);
                (true, false)
            }
            Self::SpawnEntity { entity } => {
                world.entities.remove(&entity.id);
                (false, false)
            }
            Self::DeleteEntity { entity } => {
                world.entities.insert(entity.id, (**entity).clone());
                (false, false)
            }
            Self::MoveEntity {
                entity_id, old_pos, ..
            } => {
                if let Some(ent) = world.entities.get_mut(entity_id) {
                    ent.position = *old_pos;
                }
                (false, false)
            }
            Self::TransformEntities { before, .. } => {
                for &(eid, pos, tile, facing) in before {
                    if let Some(ent) = world.entities.get_mut(&eid) {
                        ent.position = pos;
                        ent.tile_origin = tile;
                        ent.facing_radians = facing;
                        ent.turret_facing_radians = facing;
                    }
                }
                (false, false)
            }
            Self::EditUnitDef { id, before, .. } => {
                loaded.units.insert(id.clone(), (**before).clone());
                let _ = world.apply_mod_hot_reload(loaded);
                (false, true)
            }
            Self::EditBuildingDef { id, before, .. } => {
                loaded.buildings.insert(id.clone(), (**before).clone());
                let _ = world.apply_mod_hot_reload(loaded);
                (false, true)
            }
            Self::EditWeaponDef { id, before, .. } => {
                loaded.weapons.insert(id.clone(), (**before).clone());
                let _ = world.apply_mod_hot_reload(loaded);
                (false, true)
            }
            Self::EditUpgradeDef { id, before, .. } => {
                loaded.upgrades.insert(id.clone(), (**before).clone());
                (false, true)
            }
            Self::EditCommanderTreeDef { id, before, .. } => {
                loaded
                    .commander_trees
                    .insert(id.clone(), (**before).clone());
                (false, true)
            }
            Self::EditFactionDef { id, before, .. } => {
                loaded.factions.insert(id.clone(), (**before).clone());
                (false, true)
            }
            Self::EditResourceDef { id, before, .. } => {
                loaded.resources.insert(id.clone(), (**before).clone());
                (false, true)
            }
            Self::EditAiProfileDef { id, before, .. } => {
                loaded.ai_profiles.insert(id.clone(), (**before).clone());
                (false, true)
            }
            Self::EditCampaignDef { id, before, .. } => {
                loaded.campaigns.insert(id.clone(), (**before).clone());
                (false, true)
            }
            Self::EditAudioCatalog { before, .. } => {
                loaded.audio = (**before).clone();
                (false, true)
            }
            Self::EditLocaleCatalog { lang, before, .. } => {
                loaded.locales.insert(lang.clone(), (**before).clone());
                (false, true)
            }
            Self::EditVfxEffectDef { id, before, .. } => {
                if let Some(prev) = before {
                    loaded.vfx_effects.insert(id.clone(), (**prev).clone());
                } else {
                    loaded.vfx_effects.remove(id);
                }
                (false, true)
            }
            Self::EditMapDef { before, .. } => {
                loaded.entry_map = (**before).clone();
                loaded
                    .maps
                    .insert(loaded.entry_map.id.clone(), loaded.entry_map.clone());
                world.trigger_zones = loaded.entry_map.trigger_zones.clone();
                world.trigger_graphs = loaded.entry_map.trigger_graphs.clone();
                world.playable_bounds = loaded.entry_map.playable_bounds;
                (false, true)
            }
        }
    }

    /// Re-applies `self`, returning `(terrain_dirty, sdk_dirty)`.
    pub fn apply_redo(&self, world: &mut SimWorld, loaded: &mut LoadedMod) -> (bool, bool) {
        match self {
            Self::TerrainStroke { after, .. } => {
                after.restore(world, loaded);
                (true, false)
            }
            Self::SpawnEntity { entity } => {
                world.entities.insert(entity.id, (**entity).clone());
                (false, false)
            }
            Self::DeleteEntity { entity } => {
                world.entities.remove(&entity.id);
                (false, false)
            }
            Self::MoveEntity {
                entity_id, new_pos, ..
            } => {
                if let Some(ent) = world.entities.get_mut(entity_id) {
                    ent.position = *new_pos;
                }
                (false, false)
            }
            Self::TransformEntities { after, .. } => {
                for &(eid, pos, tile, facing) in after {
                    if let Some(ent) = world.entities.get_mut(&eid) {
                        ent.position = pos;
                        ent.tile_origin = tile;
                        ent.facing_radians = facing;
                        ent.turret_facing_radians = facing;
                    }
                }
                (false, false)
            }
            Self::EditUnitDef { id, after, .. } => {
                loaded.units.insert(id.clone(), (**after).clone());
                let _ = world.apply_mod_hot_reload(loaded);
                (false, true)
            }
            Self::EditBuildingDef { id, after, .. } => {
                loaded.buildings.insert(id.clone(), (**after).clone());
                let _ = world.apply_mod_hot_reload(loaded);
                (false, true)
            }
            Self::EditWeaponDef { id, after, .. } => {
                loaded.weapons.insert(id.clone(), (**after).clone());
                let _ = world.apply_mod_hot_reload(loaded);
                (false, true)
            }
            Self::EditUpgradeDef { id, after, .. } => {
                loaded.upgrades.insert(id.clone(), (**after).clone());
                (false, true)
            }
            Self::EditCommanderTreeDef { id, after, .. } => {
                loaded.commander_trees.insert(id.clone(), (**after).clone());
                (false, true)
            }
            Self::EditFactionDef { id, after, .. } => {
                loaded.factions.insert(id.clone(), (**after).clone());
                (false, true)
            }
            Self::EditResourceDef { id, after, .. } => {
                loaded.resources.insert(id.clone(), (**after).clone());
                (false, true)
            }
            Self::EditAiProfileDef { id, after, .. } => {
                loaded.ai_profiles.insert(id.clone(), (**after).clone());
                (false, true)
            }
            Self::EditCampaignDef { id, after, .. } => {
                loaded.campaigns.insert(id.clone(), (**after).clone());
                (false, true)
            }
            Self::EditAudioCatalog { after, .. } => {
                loaded.audio = (**after).clone();
                (false, true)
            }
            Self::EditLocaleCatalog { lang, after, .. } => {
                loaded.locales.insert(lang.clone(), (**after).clone());
                (false, true)
            }
            Self::EditVfxEffectDef { id, after, .. } => {
                if let Some(next) = after {
                    loaded.vfx_effects.insert(id.clone(), (**next).clone());
                } else {
                    loaded.vfx_effects.remove(id);
                }
                (false, true)
            }
            Self::EditMapDef { after, .. } => {
                loaded.entry_map = (**after).clone();
                loaded
                    .maps
                    .insert(loaded.entry_map.id.clone(), loaded.entry_map.clone());
                world.trigger_zones = loaded.entry_map.trigger_zones.clone();
                world.trigger_graphs = loaded.entry_map.trigger_graphs.clone();
                world.playable_bounds = loaded.entry_map.playable_bounds;
                (false, true)
            }
        }
    }
}

/// Bounded (`>= 64` step) undo/redo command history for the `IRONLOCK Creator Workbench`.
#[derive(Debug)]
pub struct EditorHistory {
    undo_stack: VecDeque<EditorCommand>,
    redo_stack: Vec<EditorCommand>,
    max_steps: usize,
}

impl Default for EditorHistory {
    fn default() -> Self {
        Self::new(MAX_EDITOR_HISTORY_STEPS)
    }
}

impl EditorHistory {
    pub fn new(max_steps: usize) -> Self {
        Self {
            undo_stack: VecDeque::with_capacity(max_steps.max(MAX_EDITOR_HISTORY_STEPS)),
            redo_stack: Vec::new(),
            max_steps: max_steps.max(MAX_EDITOR_HISTORY_STEPS),
        }
    }

    pub fn push_executed(&mut self, cmd: EditorCommand) {
        if self.undo_stack.len() >= self.max_steps {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(cmd);
        self.redo_stack.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }

    pub fn peek_undo_label(&self) -> Option<String> {
        self.undo_stack.back().map(EditorCommand::label)
    }

    pub fn peek_redo_label(&self) -> Option<String> {
        self.redo_stack.last().map(EditorCommand::label)
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn undo(
        &mut self,
        world: &mut SimWorld,
        loaded: &mut LoadedMod,
    ) -> Option<(String, bool, bool)> {
        let cmd = self.undo_stack.pop_back()?;
        let label = cmd.label();
        let (terrain_dirty, sdk_dirty) = cmd.apply_undo(world, loaded);
        self.redo_stack.push(cmd);
        Some((label, terrain_dirty, sdk_dirty))
    }

    pub fn redo(
        &mut self,
        world: &mut SimWorld,
        loaded: &mut LoadedMod,
    ) -> Option<(String, bool, bool)> {
        let cmd = self.redo_stack.pop()?;
        let label = cmd.label();
        let (terrain_dirty, sdk_dirty) = cmd.apply_redo(world, loaded);
        self.undo_stack.push_back(cmd);
        Some((label, terrain_dirty, sdk_dirty))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn load_sample_mod() -> LoadedMod {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mod_dir = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .expect("workspace root")
            .join("rust-ra2");
        LoadedMod::load_from_dir(&mod_dir).expect("rust-ra2 loads")
    }

    #[test]
    fn editor_history_supports_64_step_ring_and_reversible_sdk_and_entity_commands() {
        let mut loaded = load_sample_mod();
        let mut world = SimWorld::from_mod(&loaded);
        let mut history = EditorHistory::default();

        let first_uid = loaded.units.keys().next().expect("unit exists").clone();
        let orig_unit = loaded.units.get(&first_uid).expect("unit").clone();
        let orig_cost = orig_unit.cost;

        for step in 1..=70u32 {
            let before = loaded.units.get(&first_uid).expect("unit").clone();
            let mut after = before.clone();
            after.cost = orig_cost + step * 10;
            loaded.units.insert(first_uid.clone(), after.clone());
            history.push_executed(EditorCommand::EditUnitDef {
                id: first_uid.clone(),
                before: Box::new(before),
                after: Box::new(after),
            });
        }
        assert_eq!(history.undo_len(), MAX_EDITOR_HISTORY_STEPS);
        assert!(history.can_undo());
        assert!(!history.can_redo());

        // Undo one step and redo it back
        let (_, _, sdk_dirty) = history.undo(&mut world, &mut loaded).expect("undo");
        assert!(sdk_dirty);
        assert_eq!(loaded.units[&first_uid].cost, orig_cost + 690);
        assert!(history.can_redo());

        let (_, _, sdk_dirty_redo) = history.redo(&mut world, &mut loaded).expect("redo");
        assert!(sdk_dirty_redo);
        assert_eq!(loaded.units[&first_uid].cost, orig_cost + 700);
    }
}
