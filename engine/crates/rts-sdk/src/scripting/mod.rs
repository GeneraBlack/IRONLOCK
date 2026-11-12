pub mod det_math;
pub mod lua_runtime;

use anyhow::{anyhow, Result};
use rhai::packages::{
    BasicArrayPackage, BasicMapPackage, BasicStringPackage, CorePackage, LogicPackage, Package,
};
use rhai::{Dynamic, Engine, Map, Scope, AST};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

pub use det_math::{
    det_abs, det_atan2, det_ceil, det_clamp, det_cos, det_dist, det_floor, det_hypot, det_round,
    det_sin, det_sqrt, f64_to_fixed, fixed_to_f64, DET_PI, DET_TAU,
};
pub use lua_runtime::LuaBackend;

fn default_script_stacks() -> u8 {
    1
}

fn dyn_as_f32(v: &Dynamic) -> Option<f32> {
    v.as_float()
        .ok()
        .map(|f| f as f32)
        .or_else(|| v.as_int().ok().map(|i| i as f32))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ScriptLang {
    Rhai,
    Lua,
}

impl ScriptLang {
    pub fn from_path(path: impl AsRef<Path>) -> Option<Self> {
        match path
            .as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .as_deref()
        {
            Some("rhai") => Some(Self::Rhai),
            Some("lua") => Some(Self::Lua),
            _ => None,
        }
    }
}

/// Deterministic command/effect action emitted by a unit or world script hook (`ScriptContext` 2.0).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScriptAction {
    ApplyBuff {
        target_entity_id: u64,
        status_id: String,
        duration_secs: f32,
        #[serde(default = "default_script_stacks")]
        stacks: u8,
    },
    SpawnProjectile {
        weapon_id: String,
        origin: [f32; 3],
        target_pos: [f32; 3],
        #[serde(default)]
        target_entity_id: Option<u64>,
    },
    SpawnEntity {
        def_id: String,
        player_id: u8,
        pos: [f32; 3],
        #[serde(default)]
        is_building: bool,
    },
    DealAreaDamage {
        center: [f32; 3],
        radius: f32,
        damage: f32,
        #[serde(default)]
        friendly_fire: bool,
    },
    IssueCommand {
        entity_id: u64,
        command: String,
        #[serde(default)]
        target_pos: Option<[f32; 3]>,
        #[serde(default)]
        target_entity_id: Option<u64>,
    },
}

/// Zero-clone spatial query callback interface used by `on_tick` and world-event scripts so
/// living entities never need to be eagerly cloned into `rhai::Map` / Lua tables every tick.
pub trait SpatialScriptQuery: Send + Sync {
    /// Returns deterministically sorted `entity_id`s within `radius_tiles` of `(tile_x, tile_z)`.
    /// `owner_filter <= 0` matches all players; `owner_filter > 0` matches `player_id == owner_filter`.
    fn query_ids_in_radius(
        &self,
        tile_x: i32,
        tile_z: i32,
        radius_tiles: i32,
        owner_filter: i32,
    ) -> Vec<u64>;

    fn count_in_radius(
        &self,
        tile_x: i32,
        tile_z: i32,
        radius_tiles: i32,
        owner_filter: i32,
    ) -> usize {
        self.query_ids_in_radius(tile_x, tile_z, radius_tiles, owner_filter)
            .len()
    }
}

#[derive(Clone)]
pub struct SpatialQueryHandle(pub Arc<dyn SpatialScriptQuery>);

impl PartialEq for SpatialQueryHandle {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for SpatialQueryHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SpatialQueryHandle(..)")
    }
}

type SpatialCellEntry = (u64, u8, i32, i32);

/// Compact cache-friendly spatial hash grid implementing [`SpatialScriptQuery`] without per-tick
/// `rhai::Map` or `String` allocations.
#[derive(Debug, Clone, Default)]
pub struct FlatSpatialScriptGrid {
    cell_shift: u32,
    cells: BTreeMap<(i32, i32), Vec<SpatialCellEntry>>,
}

impl FlatSpatialScriptGrid {
    pub fn new() -> Self {
        Self {
            cell_shift: 3, // 8x8 tile buckets
            cells: BTreeMap::new(),
        }
    }

    pub fn from_entries(entries: impl IntoIterator<Item = (u64, u8, [i32; 2])>) -> Self {
        let mut grid = Self::new();
        grid.rebuild(entries);
        grid
    }

    pub fn rebuild(&mut self, entries: impl IntoIterator<Item = (u64, u8, [i32; 2])>) {
        for bucket in self.cells.values_mut() {
            bucket.clear();
        }
        let shift = self.cell_shift;
        for (eid, pid, [tx, tz]) in entries {
            let key = (tx >> shift, tz >> shift);
            self.cells.entry(key).or_default().push((eid, pid, tx, tz));
        }
        self.cells.retain(|_, v| !v.is_empty());
    }
}

impl SpatialScriptQuery for FlatSpatialScriptGrid {
    fn query_ids_in_radius(
        &self,
        tile_x: i32,
        tile_z: i32,
        radius_tiles: i32,
        owner_filter: i32,
    ) -> Vec<u64> {
        let r = radius_tiles.max(0);
        let r2 = (r as i64).saturating_mul(r as i64);
        let shift = self.cell_shift;
        let min_cx = (tile_x.saturating_sub(r)) >> shift;
        let max_cx = (tile_x.saturating_add(r)) >> shift;
        let min_cz = (tile_z.saturating_sub(r)) >> shift;
        let max_cz = (tile_z.saturating_add(r)) >> shift;

        let mut ids = Vec::new();
        for cz in min_cz..=max_cz {
            for cx in min_cx..=max_cx {
                if let Some(bucket) = self.cells.get(&(cx, cz)) {
                    for &(eid, pid, ex, ez) in bucket {
                        if owner_filter > 0 && pid as i32 != owner_filter {
                            continue;
                        }
                        let dx = (ex - tile_x) as i64;
                        let dz = (ez - tile_z) as i64;
                        if dx.saturating_mul(dx).saturating_add(dz.saturating_mul(dz)) <= r2 {
                            ids.push(eid);
                        }
                    }
                }
            }
        }
        ids.sort_unstable();
        ids
    }

    fn count_in_radius(
        &self,
        tile_x: i32,
        tile_z: i32,
        radius_tiles: i32,
        owner_filter: i32,
    ) -> usize {
        let r = radius_tiles.max(0);
        let r2 = (r as i64).saturating_mul(r as i64);
        let shift = self.cell_shift;
        let min_cx = (tile_x.saturating_sub(r)) >> shift;
        let max_cx = (tile_x.saturating_add(r)) >> shift;
        let min_cz = (tile_z.saturating_sub(r)) >> shift;
        let max_cz = (tile_z.saturating_add(r)) >> shift;

        let mut count = 0_usize;
        for cz in min_cz..=max_cz {
            for cx in min_cx..=max_cx {
                if let Some(bucket) = self.cells.get(&(cx, cz)) {
                    for &(_, pid, ex, ez) in bucket {
                        if owner_filter > 0 && pid as i32 != owner_filter {
                            continue;
                        }
                        let dx = (ex - tile_x) as i64;
                        let dz = (ez - tile_z) as i64;
                        if dx.saturating_mul(dx).saturating_add(dz.saturating_mul(dz)) <= r2 {
                            count += 1;
                        }
                    }
                }
            }
        }
        count
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ScriptContext {
    pub entity_id: u64,
    pub unit_id: String,
    #[serde(default)]
    pub player_id: u8,
    #[serde(default)]
    pub self_pos: [f32; 3],
    pub health: f32,
    pub max_health: f32,
    #[serde(default)]
    pub energy: f32,
    #[serde(default)]
    pub max_energy: f32,
    pub speed_multiplier: f32,
    pub armor_multiplier: f32,
    pub damage_multiplier: f32,
    pub range_bonus: f32,
    pub deployed: bool,
    #[serde(default)]
    pub target_pos: Option<[f32; 3]>,
    #[serde(default)]
    pub target_entity_id: Option<u64>,
    #[serde(default)]
    pub attacker_id: Option<u64>,
    pub custom_flags: BTreeMap<String, bool>,
    #[serde(default)]
    pub emitted_actions: Vec<ScriptAction>,
}

impl Default for ScriptContext {
    fn default() -> Self {
        Self {
            entity_id: 0,
            unit_id: String::new(),
            player_id: 0,
            self_pos: [0.0, 0.0, 0.0],
            health: 100.0,
            max_health: 100.0,
            energy: 0.0,
            max_energy: 0.0,
            speed_multiplier: 1.0,
            armor_multiplier: 1.0,
            damage_multiplier: 1.0,
            range_bonus: 0.0,
            deployed: false,
            target_pos: None,
            target_entity_id: None,
            attacker_id: None,
            custom_flags: BTreeMap::new(),
            emitted_actions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EntityQuerySnapshot {
    pub entity_id: u64,
    pub def_id: String,
    pub player_id: u8,
    pub is_building: bool,
    pub tile_pos: [i32; 2],
    pub health: f32,
    pub max_health: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlayerResourceSnapshot {
    pub player_id: u8,
    pub credits: u32,
    pub power_balance: i32,
    #[serde(default)]
    pub resources: BTreeMap<String, u32>,
    pub unit_count: u32,
    pub building_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ScriptResourceGrant {
    pub player_id: u8,
    pub resource_id: String,
    pub delta: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldEventContext {
    pub event_type: String,
    pub tick: u64,
    pub entity_id: u64,
    pub player_id: u8,
    pub bonus_credits: i32,
    pub spawn_unit_id: Option<String>,
    pub spawn_count: u32,
    pub spawn_tile: Option<[i32; 2]>,
    pub spawn_player_id: Option<u8>,
    pub attack_move_target: Option<[f32; 3]>,
    pub objective_text: Option<String>,
    pub declared_winner: Option<u8>,
    pub custom_flags: BTreeMap<String, bool>,
    #[serde(default)]
    pub trigger_vars: BTreeMap<String, i64>,
    #[serde(default)]
    pub camera_focus_tile: Option<[i32; 2]>,
    #[serde(default)]
    pub eva_transmission: Option<(String, String)>,
    #[serde(default)]
    pub queued_actions: Vec<crate::schema::TriggerAction>,
    #[serde(default)]
    pub emitted_actions: Vec<ScriptAction>,
    #[serde(default)]
    pub player_snapshots: BTreeMap<u8, PlayerResourceSnapshot>,
    #[serde(default)]
    pub entity_snapshots: Vec<EntityQuerySnapshot>,
    #[serde(default)]
    pub resource_grants: Vec<ScriptResourceGrant>,
    #[serde(skip)]
    pub spatial_query: Option<SpatialQueryHandle>,
}

pub trait ScriptBackend: Send + Sync {
    fn language(&self) -> ScriptLang;
    fn load_script(&mut self, name: &str, source: &str) -> Result<Vec<String>>;
    fn has_hook(&self, hook_name: &str) -> bool;
    fn invoke_hook(&self, hook_name: &str, ctx: &mut ScriptContext) -> Result<()>;
}

pub struct RhaiBackend {
    pub(crate) engine: Engine,
    pub(crate) asts: BTreeMap<String, AST>,
    pub(crate) hook_to_script: BTreeMap<String, String>,
}

impl Default for RhaiBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl RhaiBackend {
    pub fn new() -> Self {
        let mut engine = Engine::new_raw();
        engine.register_global_module(CorePackage::new().as_shared_module());
        engine.register_global_module(LogicPackage::new().as_shared_module());
        engine.register_global_module(BasicArrayPackage::new().as_shared_module());
        engine.register_global_module(BasicMapPackage::new().as_shared_module());
        engine.register_global_module(BasicStringPackage::new().as_shared_module());
        register_deterministic_rhai_math(&mut engine);
        engine.disable_symbol("eval");
        engine.set_max_operations(50_000);
        engine.set_max_string_size(16_384);
        engine.set_max_array_size(512);
        engine.set_max_map_size(256);
        engine.set_max_call_levels(32);
        engine.set_max_expr_depths(32, 16);

        fn push_action_to_map(ctx: &mut Map, action_map: Map) {
            if let Some(arr_dyn) = ctx.get_mut("queued_actions") {
                if let Some(mut arr) = arr_dyn.write_lock::<rhai::Array>() {
                    arr.push(Dynamic::from(action_map));
                    return;
                }
            }
            ctx.insert(
                "queued_actions".into(),
                Dynamic::from(vec![Dynamic::from(action_map)]),
            );
        }

        engine.register_fn(
            "queue_set_variable",
            |ctx: &mut Map, name: &str, value: i64| {
                let mut m = Map::new();
                m.insert("type".into(), Dynamic::from("set_variable".to_string()));
                m.insert("name".into(), Dynamic::from(name.to_string()));
                m.insert("value".into(), Dynamic::from_int(value));
                push_action_to_map(ctx, m);
            },
        );
        engine.register_fn(
            "queue_add_variable",
            |ctx: &mut Map, name: &str, delta: i64| {
                let mut m = Map::new();
                m.insert("type".into(), Dynamic::from("add_variable".to_string()));
                m.insert("name".into(), Dynamic::from(name.to_string()));
                m.insert("delta".into(), Dynamic::from_int(delta));
                push_action_to_map(ctx, m);
            },
        );
        engine.register_fn(
            "queue_grant_credits",
            |ctx: &mut Map, player_id: i64, amount: i64| {
                let mut m = Map::new();
                m.insert("type".into(), Dynamic::from("grant_credits".to_string()));
                m.insert("player_id".into(), Dynamic::from_int(player_id));
                m.insert("amount".into(), Dynamic::from_int(amount));
                push_action_to_map(ctx, m);
            },
        );
        engine.register_fn(
            "queue_spawn_reinforcements",
            |ctx: &mut Map, player_id: i64, unit_id: &str, count: i64, tile_x: i64, tile_z: i64| {
                let mut m = Map::new();
                m.insert(
                    "type".into(),
                    Dynamic::from("spawn_reinforcements".to_string()),
                );
                m.insert("player_id".into(), Dynamic::from_int(player_id));
                m.insert("unit_id".into(), Dynamic::from(unit_id.to_string()));
                m.insert("count".into(), Dynamic::from_int(count));
                m.insert("tile_x".into(), Dynamic::from_int(tile_x));
                m.insert("tile_z".into(), Dynamic::from_int(tile_z));
                push_action_to_map(ctx, m);
            },
        );
        engine.register_fn(
            "queue_spawn_building",
            |ctx: &mut Map, player_id: i64, building_id: &str, tile_x: i64, tile_z: i64| {
                let mut m = Map::new();
                m.insert("type".into(), Dynamic::from("spawn_building".to_string()));
                m.insert("player_id".into(), Dynamic::from_int(player_id));
                m.insert("building_id".into(), Dynamic::from(building_id.to_string()));
                m.insert("tile_x".into(), Dynamic::from_int(tile_x));
                m.insert("tile_z".into(), Dynamic::from_int(tile_z));
                push_action_to_map(ctx, m);
            },
        );
        engine.register_fn(
            "queue_focus_camera",
            |ctx: &mut Map, tile_x: i64, tile_z: i64| {
                let mut m = Map::new();
                m.insert("type".into(), Dynamic::from("focus_camera".to_string()));
                m.insert("tile_x".into(), Dynamic::from_int(tile_x));
                m.insert("tile_z".into(), Dynamic::from_int(tile_z));
                push_action_to_map(ctx, m);
            },
        );
        engine.register_fn("queue_eva", |ctx: &mut Map, speaker: &str, text: &str| {
            let mut m = Map::new();
            m.insert("type".into(), Dynamic::from("eva_transmission".to_string()));
            m.insert("speaker".into(), Dynamic::from(speaker.to_string()));
            m.insert("text".into(), Dynamic::from(text.to_string()));
            push_action_to_map(ctx, m);
        });
        engine.register_fn(
            "queue_eva_transmission",
            |ctx: &mut Map, speaker: &str, text: &str| {
                let mut m = Map::new();
                m.insert("type".into(), Dynamic::from("eva_transmission".to_string()));
                m.insert("speaker".into(), Dynamic::from(speaker.to_string()));
                m.insert("text".into(), Dynamic::from(text.to_string()));
                push_action_to_map(ctx, m);
            },
        );
        engine.register_fn("queue_set_objective", |ctx: &mut Map, text: &str| {
            let mut m = Map::new();
            m.insert("type".into(), Dynamic::from("set_objective".to_string()));
            m.insert("text".into(), Dynamic::from(text.to_string()));
            push_action_to_map(ctx, m);
        });
        engine.register_fn("queue_set_weather", |ctx: &mut Map, preset: &str| {
            let mut m = Map::new();
            m.insert("type".into(), Dynamic::from("set_weather".to_string()));
            m.insert("preset".into(), Dynamic::from(preset.to_string()));
            push_action_to_map(ctx, m);
        });
        engine.register_fn("queue_declare_winner", |ctx: &mut Map, player_id: i64| {
            let mut m = Map::new();
            m.insert("type".into(), Dynamic::from("declare_winner".to_string()));
            m.insert("player_id".into(), Dynamic::from_int(player_id));
            push_action_to_map(ctx, m);
        });
        engine.register_fn(
            "queue_reveal_area",
            |ctx: &mut Map, tile_x: i64, tile_z: i64, radius: i64| {
                let mut m = Map::new();
                m.insert("type".into(), Dynamic::from("reveal_area".to_string()));
                m.insert("tile_x".into(), Dynamic::from_int(tile_x));
                m.insert("tile_z".into(), Dynamic::from_int(tile_z));
                m.insert("radius".into(), Dynamic::from_int(radius));
                push_action_to_map(ctx, m);
            },
        );
        engine.register_fn(
            "queue_change_owner",
            |ctx: &mut Map, from_player_id: i64, to_player_id: i64| {
                let mut m = Map::new();
                m.insert("type".into(), Dynamic::from("change_owner".to_string()));
                m.insert("from_player_id".into(), Dynamic::from_int(from_player_id));
                m.insert("to_player_id".into(), Dynamic::from_int(to_player_id));
                push_action_to_map(ctx, m);
            },
        );
        engine.register_fn(
            "queue_change_building_owner",
            |ctx: &mut Map, from_player_id: i64, to_player_id: i64, building_id: &str| {
                let mut m = Map::new();
                m.insert("type".into(), Dynamic::from("change_owner".to_string()));
                m.insert("from_player_id".into(), Dynamic::from_int(from_player_id));
                m.insert("to_player_id".into(), Dynamic::from_int(to_player_id));
                m.insert("building_id".into(), Dynamic::from(building_id.to_string()));
                push_action_to_map(ctx, m);
            },
        );
        engine.register_fn(
            "queue_fire_support_power",
            |ctx: &mut Map, player_id: i64, power_id: &str, target_x: i64, target_z: i64| {
                let mut m = Map::new();
                m.insert(
                    "type".into(),
                    Dynamic::from("fire_support_power".to_string()),
                );
                m.insert("player_id".into(), Dynamic::from_int(player_id));
                m.insert("power_id".into(), Dynamic::from(power_id.to_string()));
                m.insert("target_x".into(), Dynamic::from_int(target_x));
                m.insert("target_z".into(), Dynamic::from_int(target_z));
                push_action_to_map(ctx, m);
            },
        );

        fn push_grant_to_map(ctx: &mut Map, grant_map: Map) {
            if let Some(arr_dyn) = ctx.get_mut("resource_grants") {
                if let Some(mut arr) = arr_dyn.write_lock::<rhai::Array>() {
                    arr.push(Dynamic::from(grant_map));
                    return;
                }
            }
            ctx.insert(
                "resource_grants".into(),
                Dynamic::from(vec![Dynamic::from(grant_map)]),
            );
        }

        engine.register_fn(
            "queue_grant_resource",
            |ctx: &mut Map, player_id: i64, resource_id: &str, delta: i64| {
                let mut m = Map::new();
                m.insert("player_id".into(), Dynamic::from_int(player_id));
                m.insert("resource_id".into(), Dynamic::from(resource_id.to_string()));
                m.insert("delta".into(), Dynamic::from_int(delta));
                push_grant_to_map(ctx, m);
            },
        );
        engine.register_fn(
            "query_player_resource",
            |ctx: &mut Map, player_id: i64, resource_id: &str| -> i64 {
                let pid_key = player_id.to_string();
                if let Some(ps_dyn) = ctx.get("player_snapshots") {
                    if let Some(ps_map) = ps_dyn.read_lock::<Map>() {
                        if let Some(p_dyn) = ps_map.get(pid_key.as_str()) {
                            if let Some(p_map) = p_dyn.read_lock::<Map>() {
                                if let Some(res_dyn) = p_map.get("resources") {
                                    if let Some(res_map) = res_dyn.read_lock::<Map>() {
                                        if let Some(v) =
                                            res_map.get(resource_id).and_then(|v| v.as_int().ok())
                                        {
                                            return v;
                                        }
                                    }
                                }
                                if resource_id == "credits" {
                                    return p_map
                                        .get("credits")
                                        .and_then(|v| v.as_int().ok())
                                        .unwrap_or(0);
                                }
                                if resource_id == "power" {
                                    return p_map
                                        .get("power_balance")
                                        .and_then(|v| v.as_int().ok())
                                        .unwrap_or(0);
                                }
                            }
                        }
                    }
                }
                0
            },
        );
        engine.register_fn("query_unit_count", |ctx: &mut Map, player_id: i64| -> i64 {
            let pid_key = player_id.to_string();
            if let Some(ps_dyn) = ctx.get("player_snapshots") {
                if let Some(ps_map) = ps_dyn.read_lock::<Map>() {
                    if let Some(p_dyn) = ps_map.get(pid_key.as_str()) {
                        if let Some(p_map) = p_dyn.read_lock::<Map>() {
                            if let Some(v) = p_map.get("unit_count").and_then(|v| v.as_int().ok()) {
                                return v;
                            }
                        }
                    }
                }
            }
            0
        });
        engine.register_fn(
            "query_building_count",
            |ctx: &mut Map, player_id: i64| -> i64 {
                let pid_key = player_id.to_string();
                if let Some(ps_dyn) = ctx.get("player_snapshots") {
                    if let Some(ps_map) = ps_dyn.read_lock::<Map>() {
                        if let Some(p_dyn) = ps_map.get(pid_key.as_str()) {
                            if let Some(p_map) = p_dyn.read_lock::<Map>() {
                                if let Some(v) =
                                    p_map.get("building_count").and_then(|v| v.as_int().ok())
                                {
                                    return v;
                                }
                            }
                        }
                    }
                }
                0
            },
        );
        engine.register_fn(
            "query_entities_in_radius",
            |ctx: &mut Map,
             tile_x: i64,
             tile_z: i64,
             radius_tiles: i64,
             owner_filter: i64|
             -> rhai::Array {
                if let Some(sp_dyn) = ctx.get("__spatial") {
                    if let Some(handle) = sp_dyn.read_lock::<SpatialQueryHandle>() {
                        let ids = handle.0.query_ids_in_radius(
                            tile_x as i32,
                            tile_z as i32,
                            radius_tiles as i32,
                            owner_filter as i32,
                        );
                        return ids
                            .into_iter()
                            .map(|id| Dynamic::from_int(id as i64))
                            .collect();
                    }
                }
                let mut ids: Vec<i64> = Vec::new();
                let r2 = radius_tiles.max(0).saturating_mul(radius_tiles.max(0));
                if let Some(ent_dyn) = ctx.get("entity_snapshots") {
                    if let Some(ent_arr) = ent_dyn.read_lock::<rhai::Array>() {
                        for item in ent_arr.iter() {
                            if let Some(em) = item.read_lock::<Map>() {
                                let pid = em
                                    .get("player_id")
                                    .and_then(|v| v.as_int().ok())
                                    .unwrap_or(0);
                                if owner_filter > 0 && pid != owner_filter {
                                    continue;
                                }
                                let ex =
                                    em.get("tile_x").and_then(|v| v.as_int().ok()).unwrap_or(0);
                                let ez =
                                    em.get("tile_z").and_then(|v| v.as_int().ok()).unwrap_or(0);
                                let dx = ex - tile_x;
                                let dz = ez - tile_z;
                                if dx.saturating_mul(dx).saturating_add(dz.saturating_mul(dz)) <= r2
                                {
                                    if let Some(eid) =
                                        em.get("entity_id").and_then(|v| v.as_int().ok())
                                    {
                                        ids.push(eid);
                                    }
                                }
                            }
                        }
                    }
                }
                ids.sort_unstable();
                ids.into_iter().map(Dynamic::from_int).collect()
            },
        );
        engine.register_fn(
            "count_entities_in_radius",
            |ctx: &mut Map,
             tile_x: i64,
             tile_z: i64,
             radius_tiles: i64,
             owner_filter: i64|
             -> i64 {
                if let Some(sp_dyn) = ctx.get("__spatial") {
                    if let Some(handle) = sp_dyn.read_lock::<SpatialQueryHandle>() {
                        return handle.0.count_in_radius(
                            tile_x as i32,
                            tile_z as i32,
                            radius_tiles as i32,
                            owner_filter as i32,
                        ) as i64;
                    }
                }
                let mut count: i64 = 0;
                let r2 = radius_tiles.max(0).saturating_mul(radius_tiles.max(0));
                if let Some(ent_dyn) = ctx.get("entity_snapshots") {
                    if let Some(ent_arr) = ent_dyn.read_lock::<rhai::Array>() {
                        for item in ent_arr.iter() {
                            if let Some(em) = item.read_lock::<Map>() {
                                let pid = em
                                    .get("player_id")
                                    .and_then(|v| v.as_int().ok())
                                    .unwrap_or(0);
                                if owner_filter > 0 && pid != owner_filter {
                                    continue;
                                }
                                let ex =
                                    em.get("tile_x").and_then(|v| v.as_int().ok()).unwrap_or(0);
                                let ez =
                                    em.get("tile_z").and_then(|v| v.as_int().ok()).unwrap_or(0);
                                let dx = ex - tile_x;
                                let dz = ez - tile_z;
                                if dx.saturating_mul(dx).saturating_add(dz.saturating_mul(dz)) <= r2
                                {
                                    count += 1;
                                }
                            }
                        }
                    }
                }
                count
            },
        );

        fn push_script_action(ctx: &mut Map, act: ScriptAction) {
            if let Some(arr_dyn) = ctx.get_mut("emitted_actions") {
                if let Some(mut arr) = arr_dyn.write_lock::<rhai::Array>() {
                    arr.push(Dynamic::from(act));
                    return;
                }
            }
            ctx.insert(
                "emitted_actions".into(),
                Dynamic::from(vec![Dynamic::from(act)]),
            );
        }

        engine.register_fn(
            "apply_buff",
            |ctx: &mut Map, target_entity_id: i64, status_id: &str, duration_secs: Dynamic| {
                let dur = dyn_as_f32(&duration_secs).unwrap_or(5.0);
                push_script_action(
                    ctx,
                    ScriptAction::ApplyBuff {
                        target_entity_id: target_entity_id.max(0) as u64,
                        status_id: status_id.to_string(),
                        duration_secs: dur,
                        stacks: 1,
                    },
                );
            },
        );
        engine.register_fn(
            "apply_buff",
            |ctx: &mut Map,
             target_entity_id: i64,
             status_id: &str,
             duration_secs: Dynamic,
             stacks: i64| {
                let dur = dyn_as_f32(&duration_secs).unwrap_or(5.0);
                push_script_action(
                    ctx,
                    ScriptAction::ApplyBuff {
                        target_entity_id: target_entity_id.max(0) as u64,
                        status_id: status_id.to_string(),
                        duration_secs: dur,
                        stacks: stacks.clamp(1, 255) as u8,
                    },
                );
            },
        );
        engine.register_fn(
            "spawn_projectile",
            |ctx: &mut Map,
             weapon_id: &str,
             ox: Dynamic,
             oy: Dynamic,
             oz: Dynamic,
             tx: Dynamic,
             ty: Dynamic,
             tz: Dynamic| {
                push_script_action(
                    ctx,
                    ScriptAction::SpawnProjectile {
                        weapon_id: weapon_id.to_string(),
                        origin: [
                            dyn_as_f32(&ox).unwrap_or(0.0),
                            dyn_as_f32(&oy).unwrap_or(0.0),
                            dyn_as_f32(&oz).unwrap_or(0.0),
                        ],
                        target_pos: [
                            dyn_as_f32(&tx).unwrap_or(0.0),
                            dyn_as_f32(&ty).unwrap_or(0.0),
                            dyn_as_f32(&tz).unwrap_or(0.0),
                        ],
                        target_entity_id: None,
                    },
                );
            },
        );
        engine.register_fn(
            "spawn_entity",
            |ctx: &mut Map, def_id: &str, player_id: i64, x: Dynamic, y: Dynamic, z: Dynamic| {
                push_script_action(
                    ctx,
                    ScriptAction::SpawnEntity {
                        def_id: def_id.to_string(),
                        player_id: player_id.clamp(0, 8) as u8,
                        pos: [
                            dyn_as_f32(&x).unwrap_or(0.0),
                            dyn_as_f32(&y).unwrap_or(0.0),
                            dyn_as_f32(&z).unwrap_or(0.0),
                        ],
                        is_building: false,
                    },
                );
            },
        );
        engine.register_fn(
            "deal_area_damage",
            |ctx: &mut Map,
             x: Dynamic,
             y: Dynamic,
             z: Dynamic,
             radius: Dynamic,
             damage: Dynamic,
             friendly_fire: bool| {
                push_script_action(
                    ctx,
                    ScriptAction::DealAreaDamage {
                        center: [
                            dyn_as_f32(&x).unwrap_or(0.0),
                            dyn_as_f32(&y).unwrap_or(0.0),
                            dyn_as_f32(&z).unwrap_or(0.0),
                        ],
                        radius: dyn_as_f32(&radius).unwrap_or(1.0),
                        damage: dyn_as_f32(&damage).unwrap_or(0.0),
                        friendly_fire,
                    },
                );
            },
        );
        engine.register_fn(
            "issue_command",
            |ctx: &mut Map,
             entity_id: i64,
             command: &str,
             tx: Dynamic,
             ty: Dynamic,
             tz: Dynamic| {
                push_script_action(
                    ctx,
                    ScriptAction::IssueCommand {
                        entity_id: entity_id.max(0) as u64,
                        command: command.to_string(),
                        target_pos: Some([
                            dyn_as_f32(&tx).unwrap_or(0.0),
                            dyn_as_f32(&ty).unwrap_or(0.0),
                            dyn_as_f32(&tz).unwrap_or(0.0),
                        ]),
                        target_entity_id: None,
                    },
                );
            },
        );

        Self {
            engine,
            asts: BTreeMap::new(),
            hook_to_script: BTreeMap::new(),
        }
    }
}

fn register_deterministic_rhai_math(engine: &mut Engine) {
    let mut const_mod = rhai::Module::new();
    const_mod.set_var("PI", DET_PI);
    const_mod.set_var("TAU", DET_TAU);
    let shared_consts = rhai::Shared::new(const_mod);
    engine.register_global_module(shared_consts.clone());
    engine.register_static_module("math", shared_consts);

    engine.register_fn("PI", || -> f64 { DET_PI });
    engine.register_fn("TAU", || -> f64 { DET_TAU });

    for name in ["det_sin", "sin"] {
        engine.register_fn(name, det_sin);
        engine.register_fn(name, |x: i64| -> f64 { det_sin(x as f64) });
    }
    for name in ["det_cos", "cos"] {
        engine.register_fn(name, det_cos);
        engine.register_fn(name, |x: i64| -> f64 { det_cos(x as f64) });
    }
    for name in ["det_sqrt", "sqrt"] {
        engine.register_fn(name, det_sqrt);
        engine.register_fn(name, |x: i64| -> f64 { det_sqrt(x as f64) });
    }
    for name in ["det_atan2", "atan2"] {
        engine.register_fn(name, det_atan2);
        engine.register_fn(name, |y: i64, x: i64| -> f64 {
            det_atan2(y as f64, x as f64)
        });
        engine.register_fn(name, |y: i64, x: f64| -> f64 { det_atan2(y as f64, x) });
        engine.register_fn(name, |y: f64, x: i64| -> f64 { det_atan2(y, x as f64) });
    }
    for name in ["det_hypot", "hypot"] {
        engine.register_fn(name, det_hypot);
        engine.register_fn(name, |dx: i64, dz: i64| -> f64 {
            det_hypot(dx as f64, dz as f64)
        });
        engine.register_fn(name, |dx: i64, dz: f64| -> f64 { det_hypot(dx as f64, dz) });
        engine.register_fn(name, |dx: f64, dz: i64| -> f64 { det_hypot(dx, dz as f64) });
    }
    for name in ["det_dist", "dist"] {
        engine.register_fn(name, det_dist);
        engine.register_fn(name, |x1: i64, z1: i64, x2: i64, z2: i64| -> f64 {
            det_dist(x1 as f64, z1 as f64, x2 as f64, z2 as f64)
        });
        engine.register_fn(name, det_hypot);
        engine.register_fn(name, |dx: i64, dz: i64| -> f64 {
            det_hypot(dx as f64, dz as f64)
        });
    }
    for name in ["det_clamp", "clamp"] {
        engine.register_fn(name, det_clamp);
        engine.register_fn(name, |v: i64, lo: i64, hi: i64| -> i64 {
            let actual_lo = lo.min(hi);
            let actual_hi = lo.max(hi);
            v.clamp(actual_lo, actual_hi)
        });
    }
    for name in ["det_abs", "abs"] {
        engine.register_fn(name, det_abs);
        engine.register_fn(name, |x: i64| -> i64 { x.saturating_abs() });
    }
    engine.register_fn("floor", det_floor);
    engine.register_fn("floor", |x: i64| -> i64 { x });
    engine.register_fn("ceil", det_ceil);
    engine.register_fn("ceil", |x: i64| -> i64 { x });
    engine.register_fn("round", det_round);
    engine.register_fn("round", |x: i64| -> i64 { x });
    engine.register_fn("min", |a: f64, b: f64| -> f64 {
        if f64_to_fixed(a) <= f64_to_fixed(b) {
            a
        } else {
            b
        }
    });
    engine.register_fn("min", |a: i64, b: i64| -> i64 { a.min(b) });
    engine.register_fn("max", |a: f64, b: f64| -> f64 {
        if f64_to_fixed(a) >= f64_to_fixed(b) {
            a
        } else {
            b
        }
    });
    engine.register_fn("max", |a: i64, b: i64| -> i64 { a.max(b) });
}

impl ScriptBackend for RhaiBackend {
    fn language(&self) -> ScriptLang {
        ScriptLang::Rhai
    }

    fn load_script(&mut self, name: &str, source: &str) -> Result<Vec<String>> {
        let ast = self
            .engine
            .compile(source)
            .map_err(|e| anyhow!("Rhai compile error in '{name}': {e}"))?;
        let mut hooks = Vec::new();
        for fn_def in ast.iter_functions() {
            let fn_name = fn_def.name.to_string();
            self.hook_to_script
                .insert(fn_name.clone(), name.to_string());
            hooks.push(fn_name);
        }
        self.asts.insert(name.to_string(), ast);
        Ok(hooks)
    }

    fn has_hook(&self, hook_name: &str) -> bool {
        self.hook_to_script.contains_key(hook_name)
    }

    fn invoke_hook(&self, hook_name: &str, ctx: &mut ScriptContext) -> Result<()> {
        let script_name = self
            .hook_to_script
            .get(hook_name)
            .ok_or_else(|| anyhow!("Rhai hook '{hook_name}' not found"))?;
        let ast = self
            .asts
            .get(script_name)
            .ok_or_else(|| anyhow!("Rhai AST '{script_name}' missing"))?;

        let mut map = Map::new();
        map.insert("entity_id".into(), Dynamic::from_int(ctx.entity_id as i64));
        map.insert("unit_id".into(), Dynamic::from(ctx.unit_id.clone()));
        map.insert("player_id".into(), Dynamic::from_int(ctx.player_id as i64));
        map.insert(
            "pos_x".into(),
            Dynamic::from_float(ctx.self_pos[0] as rhai::FLOAT),
        );
        map.insert(
            "pos_y".into(),
            Dynamic::from_float(ctx.self_pos[1] as rhai::FLOAT),
        );
        map.insert(
            "pos_z".into(),
            Dynamic::from_float(ctx.self_pos[2] as rhai::FLOAT),
        );
        map.insert(
            "health".into(),
            Dynamic::from_float(ctx.health as rhai::FLOAT),
        );
        map.insert(
            "max_health".into(),
            Dynamic::from_float(ctx.max_health as rhai::FLOAT),
        );
        map.insert(
            "energy".into(),
            Dynamic::from_float(ctx.energy as rhai::FLOAT),
        );
        map.insert(
            "max_energy".into(),
            Dynamic::from_float(ctx.max_energy as rhai::FLOAT),
        );
        map.insert(
            "speed_multiplier".into(),
            Dynamic::from_float(ctx.speed_multiplier as rhai::FLOAT),
        );
        map.insert(
            "armor_multiplier".into(),
            Dynamic::from_float(ctx.armor_multiplier as rhai::FLOAT),
        );
        map.insert(
            "damage_multiplier".into(),
            Dynamic::from_float(ctx.damage_multiplier as rhai::FLOAT),
        );
        map.insert(
            "range_bonus".into(),
            Dynamic::from_float(ctx.range_bonus as rhai::FLOAT),
        );
        map.insert("deployed".into(), Dynamic::from_bool(ctx.deployed));
        let tpos = ctx.target_pos.unwrap_or([0.0, 0.0, 0.0]);
        map.insert(
            "has_target_pos".into(),
            Dynamic::from_bool(ctx.target_pos.is_some()),
        );
        map.insert(
            "target_x".into(),
            Dynamic::from_float(tpos[0] as rhai::FLOAT),
        );
        map.insert(
            "target_y".into(),
            Dynamic::from_float(tpos[1] as rhai::FLOAT),
        );
        map.insert(
            "target_z".into(),
            Dynamic::from_float(tpos[2] as rhai::FLOAT),
        );
        map.insert(
            "target_entity_id".into(),
            Dynamic::from_int(ctx.target_entity_id.unwrap_or(0) as i64),
        );
        map.insert(
            "attacker_id".into(),
            Dynamic::from_int(ctx.attacker_id.unwrap_or(0) as i64),
        );
        map.insert("emitted_actions".into(), Dynamic::from(rhai::Array::new()));
        for (k, v) in &ctx.custom_flags {
            map.insert(format!("flag_{k}").into(), Dynamic::from_bool(*v));
        }

        let mut scope = Scope::new();
        let result: Map = self
            .engine
            .call_fn(&mut scope, ast, hook_name, (map,))
            .map_err(|e| anyhow!("Error executing Rhai hook '{hook_name}': {e}"))?;

        if let Some(v) = result.get("health").and_then(dyn_as_f32) {
            ctx.health = v;
        }
        if let Some(v) = result.get("energy").and_then(dyn_as_f32) {
            ctx.energy = v;
        }
        if let Some(v) = result.get("speed_multiplier").and_then(dyn_as_f32) {
            ctx.speed_multiplier = v;
        }
        if let Some(v) = result.get("armor_multiplier").and_then(dyn_as_f32) {
            ctx.armor_multiplier = v;
        }
        if let Some(v) = result.get("damage_multiplier").and_then(dyn_as_f32) {
            ctx.damage_multiplier = v;
        }
        if let Some(v) = result.get("range_bonus").and_then(dyn_as_f32) {
            ctx.range_bonus = v;
        }
        if let Some(v) = result.get("deployed").and_then(|v| v.as_bool().ok()) {
            ctx.deployed = v;
        }
        if let Some(actions_dyn) = result.get("emitted_actions") {
            if let Some(arr) = actions_dyn.read_lock::<rhai::Array>() {
                for item in arr.iter() {
                    if let Some(act) = item.clone().try_cast::<ScriptAction>() {
                        ctx.emitted_actions.push(act);
                    }
                }
            }
        }
        for (k, v) in result {
            if let Some(flag_name) = k.as_str().strip_prefix("flag_") {
                if let Ok(b) = v.as_bool() {
                    ctx.custom_flags.insert(flag_name.to_string(), b);
                }
            }
        }
        Ok(())
    }
}

/// Unified dual-language script host managing both `.rhai` and `.lua` scripts.
#[derive(Default)]
pub struct ScriptHost {
    rhai: RhaiBackend,
    lua: LuaBackend,
    hook_languages: BTreeMap<String, ScriptLang>,
}

impl ScriptHost {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_script_file(&mut self, relative_path: &str, source: &str) -> Result<Vec<String>> {
        let lang = ScriptLang::from_path(relative_path).ok_or_else(|| {
            anyhow!("Unsupported script extension for '{relative_path}' (expected .rhai or .lua)")
        })?;
        let hooks = match lang {
            ScriptLang::Rhai => self.rhai.load_script(relative_path, source)?,
            ScriptLang::Lua => self.lua.load_script(relative_path, source)?,
        };
        for h in &hooks {
            self.hook_languages.insert(h.clone(), lang);
        }
        Ok(hooks)
    }

    pub fn has_hook(&self, hook_name: &str) -> bool {
        self.hook_languages.contains_key(hook_name)
    }

    pub fn hook_language(&self, hook_name: &str) -> Option<ScriptLang> {
        self.hook_languages.get(hook_name).copied()
    }

    pub fn registered_hooks(&self) -> Vec<(String, ScriptLang)> {
        self.hook_languages
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    pub fn invoke_hook(&self, hook_name: &str, ctx: &mut ScriptContext) -> Result<()> {
        let lang =
            self.hook_languages.get(hook_name).copied().ok_or_else(|| {
                anyhow!("Script hook '{hook_name}' is not registered in Rhai or Lua")
            })?;
        match lang {
            ScriptLang::Rhai => self.rhai.invoke_hook(hook_name, ctx),
            ScriptLang::Lua => self.lua.invoke_hook(hook_name, ctx),
        }
    }

    /// Invokes a global world-event hook with an optional zero-clone [`SpatialScriptQuery`]
    /// backend so `on_tick` never needs to clone living entities into Rhai/Lua maps.
    pub fn invoke_world_event_with_spatial(
        &self,
        hook_name: &str,
        event_ctx: &mut WorldEventContext,
        spatial: Option<Arc<dyn SpatialScriptQuery>>,
    ) -> Result<bool> {
        if let Some(sp) = spatial {
            event_ctx.spatial_query = Some(SpatialQueryHandle(sp));
        }
        self.invoke_world_event(hook_name, event_ctx)
    }

    /// Invokes a global world-event hook (`on_tick`, `on_entity_spawned`, `on_entity_destroyed`, `on_building_captured`, `on_trigger_zone`)
    /// if the active mod defines a script function with that name.
    pub fn invoke_world_event(
        &self,
        hook_name: &str,
        event_ctx: &mut WorldEventContext,
    ) -> Result<bool> {
        if !self.has_hook(hook_name) {
            return Ok(false);
        }
        match self.hook_language(hook_name) {
            Some(ScriptLang::Rhai) => {
                if let Some(script_name) = self.rhai.hook_to_script.get(hook_name) {
                    if let Some(ast) = self.rhai.asts.get(script_name) {
                        let mut map = Map::new();
                        map.insert(
                            "event_type".into(),
                            Dynamic::from(event_ctx.event_type.clone()),
                        );
                        map.insert("tick".into(), Dynamic::from_int(event_ctx.tick as i64));
                        map.insert(
                            "entity_id".into(),
                            Dynamic::from_int(event_ctx.entity_id as i64),
                        );
                        map.insert(
                            "player_id".into(),
                            Dynamic::from_int(event_ctx.player_id as i64),
                        );
                        map.insert(
                            "health".into(),
                            Dynamic::from_float(event_ctx.bonus_credits as rhai::FLOAT),
                        );
                        map.insert(
                            "bonus_credits".into(),
                            Dynamic::from_int(event_ctx.bonus_credits as i64),
                        );
                        map.insert(
                            "spawn_count".into(),
                            Dynamic::from_int(event_ctx.spawn_count as i64),
                        );
                        map.insert("queued_actions".into(), Dynamic::from(rhai::Array::new()));
                        map.insert("emitted_actions".into(), Dynamic::from(rhai::Array::new()));
                        map.insert("resource_grants".into(), Dynamic::from(rhai::Array::new()));
                        if let Some(sp) = &event_ctx.spatial_query {
                            map.insert("__spatial".into(), Dynamic::from(sp.clone()));
                        }
                        let mut ps_map = Map::new();
                        for (pid, snap) in &event_ctx.player_snapshots {
                            let mut p_entry = Map::new();
                            p_entry.insert("player_id".into(), Dynamic::from_int(*pid as i64));
                            p_entry
                                .insert("credits".into(), Dynamic::from_int(snap.credits as i64));
                            p_entry.insert(
                                "power_balance".into(),
                                Dynamic::from_int(snap.power_balance as i64),
                            );
                            p_entry.insert(
                                "unit_count".into(),
                                Dynamic::from_int(snap.unit_count as i64),
                            );
                            p_entry.insert(
                                "building_count".into(),
                                Dynamic::from_int(snap.building_count as i64),
                            );
                            let mut res_m = Map::new();
                            for (rk, rv) in &snap.resources {
                                res_m.insert(rk.as_str().into(), Dynamic::from_int(*rv as i64));
                            }
                            p_entry.insert("resources".into(), Dynamic::from(res_m));
                            ps_map.insert(pid.to_string().into(), Dynamic::from(p_entry));
                        }
                        map.insert("player_snapshots".into(), Dynamic::from(ps_map));

                        if !event_ctx.entity_snapshots.is_empty() {
                            let mut ent_arr =
                                rhai::Array::with_capacity(event_ctx.entity_snapshots.len());
                            for es in &event_ctx.entity_snapshots {
                                let mut em = Map::new();
                                em.insert(
                                    "entity_id".into(),
                                    Dynamic::from_int(es.entity_id as i64),
                                );
                                em.insert("def_id".into(), Dynamic::from(es.def_id.clone()));
                                em.insert(
                                    "player_id".into(),
                                    Dynamic::from_int(es.player_id as i64),
                                );
                                em.insert(
                                    "tile_x".into(),
                                    Dynamic::from_int(es.tile_pos[0] as i64),
                                );
                                em.insert(
                                    "tile_z".into(),
                                    Dynamic::from_int(es.tile_pos[1] as i64),
                                );
                                em.insert(
                                    "health".into(),
                                    Dynamic::from_float(es.health as rhai::FLOAT),
                                );
                                em.insert(
                                    "max_health".into(),
                                    Dynamic::from_float(es.max_health as rhai::FLOAT),
                                );
                                em.insert("is_building".into(), Dynamic::from_bool(es.is_building));
                                ent_arr.push(Dynamic::from(em));
                            }
                            map.insert("entity_snapshots".into(), Dynamic::from(ent_arr));
                        }
                        for (k, v) in &event_ctx.custom_flags {
                            map.insert(format!("flag_{k}").into(), Dynamic::from_bool(*v));
                        }
                        let mut vars_map = Map::new();
                        for (k, v) in &event_ctx.trigger_vars {
                            map.insert(format!("var_{k}").into(), Dynamic::from_int(*v));
                            vars_map.insert(k.as_str().into(), Dynamic::from_int(*v));
                        }
                        map.insert("vars".into(), Dynamic::from(vars_map));

                        let mut scope = Scope::new();
                        let result: Map = self
                            .rhai
                            .engine
                            .call_fn(&mut scope, ast, hook_name, (map,))
                            .map_err(|e| {
                                anyhow!("Error executing Rhai world hook '{hook_name}': {e}")
                            })?;

                        if let Some(v) = result
                            .get("bonus_credits")
                            .and_then(|v| v.as_int().ok())
                            .filter(|&v| v as i32 != event_ctx.bonus_credits)
                        {
                            event_ctx.bonus_credits = v as i32;
                        } else if let Some(v) = result.get("health").and_then(|v| v.as_float().ok())
                        {
                            event_ctx.bonus_credits = v.round() as i32;
                        }
                        if let Some(v) = result
                            .get("spawn_unit_id")
                            .and_then(|v| v.clone().into_string().ok())
                        {
                            if !v.is_empty() {
                                event_ctx.spawn_unit_id = Some(v);
                            }
                        }
                        if let Some(v) = result.get("spawn_count").and_then(|v| v.as_int().ok()) {
                            event_ctx.spawn_count = v.max(0) as u32;
                        }
                        if let (Some(tx), Some(tz)) = (
                            result.get("spawn_tile_x").and_then(|v| v.as_int().ok()),
                            result.get("spawn_tile_z").and_then(|v| v.as_int().ok()),
                        ) {
                            event_ctx.spawn_tile = Some([tx as i32, tz as i32]);
                        }
                        if let Some(v) = result.get("spawn_player_id").and_then(|v| v.as_int().ok())
                        {
                            event_ctx.spawn_player_id = Some(v.clamp(0, 8) as u8);
                        }
                        if let (Some(ax), Some(az)) = (
                            result.get("attack_move_x").and_then(|v| v.as_float().ok()),
                            result.get("attack_move_z").and_then(|v| v.as_float().ok()),
                        ) {
                            event_ctx.attack_move_target = Some([ax as f32, 0.0, az as f32]);
                        }
                        if let Some(v) = result
                            .get("objective_text")
                            .and_then(|v| v.clone().into_string().ok())
                        {
                            if !v.is_empty() {
                                event_ctx.objective_text = Some(v);
                            }
                        }
                        if let Some(v) = result.get("declared_winner").and_then(|v| v.as_int().ok())
                        {
                            if v > 0 {
                                event_ctx.declared_winner = Some(v as u8);
                            }
                        }
                        let fx = result
                            .get("focus_tile_x")
                            .or_else(|| result.get("camera_focus_x"))
                            .and_then(|v| v.as_int().ok());
                        let fz = result
                            .get("focus_tile_z")
                            .or_else(|| result.get("camera_focus_z"))
                            .and_then(|v| v.as_int().ok());
                        if let (Some(tx), Some(tz)) = (fx, fz) {
                            event_ctx.camera_focus_tile = Some([tx as i32, tz as i32]);
                        }
                        if let Some(eva_txt) = result
                            .get("eva_text")
                            .and_then(|v| v.clone().into_string().ok())
                        {
                            if !eva_txt.is_empty() {
                                let speaker = result
                                    .get("eva_speaker")
                                    .and_then(|v| v.clone().into_string().ok())
                                    .unwrap_or_else(|| "EVA".to_string());
                                event_ctx.eva_transmission = Some((speaker, eva_txt));
                            }
                        }
                        if let Some(vars_dyn) = result.get("vars") {
                            if let Some(vars_m) = vars_dyn.read_lock::<Map>() {
                                for (vk, vv) in vars_m.iter() {
                                    if let Ok(iv) = vv.as_int() {
                                        event_ctx.trigger_vars.insert(vk.to_string(), iv);
                                    }
                                }
                            }
                        }
                        if let Some(actions_dyn) = result.get("queued_actions") {
                            if let Some(arr) = actions_dyn.read_lock::<rhai::Array>() {
                                for item in arr.iter() {
                                    if let Some(act_map) = item.read_lock::<Map>() {
                                        if let Some(parsed) = parse_rhai_trigger_action(&act_map) {
                                            event_ctx.queued_actions.push(parsed);
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(emitted_dyn) = result.get("emitted_actions") {
                            if let Some(arr) = emitted_dyn.read_lock::<rhai::Array>() {
                                for item in arr.iter() {
                                    if let Some(act) = item.clone().try_cast::<ScriptAction>() {
                                        event_ctx.emitted_actions.push(act);
                                    }
                                }
                            }
                        }
                        if let Some(grants_dyn) = result.get("resource_grants") {
                            if let Some(arr) = grants_dyn.read_lock::<rhai::Array>() {
                                for item in arr.iter() {
                                    if let Some(gm) = item.read_lock::<Map>() {
                                        if let (Some(pid), Some(res_id), Some(delta)) = (
                                            gm.get("player_id").and_then(|v| v.as_int().ok()),
                                            gm.get("resource_id")
                                                .and_then(|v| v.clone().into_string().ok()),
                                            gm.get("delta").and_then(|v| v.as_int().ok()),
                                        ) {
                                            if !res_id.is_empty() {
                                                event_ctx.resource_grants.push(
                                                    ScriptResourceGrant {
                                                        player_id: pid.clamp(0, 8) as u8,
                                                        resource_id: res_id,
                                                        delta,
                                                    },
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        for (k, v) in result {
                            if let Some(flag_name) = k.as_str().strip_prefix("flag_") {
                                if let Ok(b) = v.as_bool() {
                                    event_ctx.custom_flags.insert(flag_name.to_string(), b);
                                }
                            } else if let Some(var_name) = k.as_str().strip_prefix("var_") {
                                if let Ok(iv) = v.as_int() {
                                    event_ctx.trigger_vars.insert(var_name.to_string(), iv);
                                }
                            }
                        }
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Some(ScriptLang::Lua) => {
                self.lua.invoke_world_event(hook_name, event_ctx)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

fn parse_rhai_trigger_action(m: &Map) -> Option<crate::schema::TriggerAction> {
    let kind = m.get("type")?.clone().into_string().ok()?;
    let get_i64 = |key: &str| m.get(key).and_then(|v| v.as_int().ok());
    let get_str = |key: &str| m.get(key).and_then(|v| v.clone().into_string().ok());
    match kind.as_str() {
        "set_variable" => Some(crate::schema::TriggerAction::SetVariable {
            name: get_str("name")?,
            value: get_i64("value")?,
        }),
        "add_variable" => Some(crate::schema::TriggerAction::AddVariable {
            name: get_str("name")?,
            delta: get_i64("delta")?,
        }),
        "grant_credits" => Some(crate::schema::TriggerAction::GrantCredits {
            player_id: get_i64("player_id").unwrap_or(1).clamp(0, 8) as u8,
            amount: get_i64("amount")?.max(0) as u32,
        }),
        "spawn_reinforcements" => Some(crate::schema::TriggerAction::SpawnReinforcements {
            player_id: get_i64("player_id").unwrap_or(1).clamp(0, 8) as u8,
            unit_id: get_str("unit_id")?,
            count: get_i64("count").unwrap_or(1).max(1) as u32,
            tile: [
                get_i64("tile_x").unwrap_or(10) as i32,
                get_i64("tile_z").unwrap_or(10) as i32,
            ],
        }),
        "spawn_building" => Some(crate::schema::TriggerAction::SpawnBuilding {
            player_id: get_i64("player_id").unwrap_or(1).clamp(0, 8) as u8,
            building_id: get_str("building_id")?,
            tile: [
                get_i64("tile_x").unwrap_or(10) as i32,
                get_i64("tile_z").unwrap_or(10) as i32,
            ],
        }),
        "focus_camera" => Some(crate::schema::TriggerAction::FocusCamera {
            tile: [get_i64("tile_x")? as i32, get_i64("tile_z")? as i32],
        }),
        "eva_transmission" => Some(crate::schema::TriggerAction::EvaTransmission {
            speaker: get_str("speaker").unwrap_or_else(|| "EVA".to_string()),
            text: get_str("text")?,
            portrait: get_str("portrait").filter(|s| !s.is_empty()),
            audio_cue: get_str("audio_cue").filter(|s| !s.is_empty()),
            radar_video: get_str("radar_video").filter(|s| !s.is_empty()),
        }),
        "set_objective" => Some(crate::schema::TriggerAction::SetObjective {
            text: get_str("text")?,
        }),
        "set_weather" => Some(crate::schema::TriggerAction::SetWeather {
            preset: get_str("preset")?,
        }),
        "declare_winner" => Some(crate::schema::TriggerAction::DeclareWinner {
            player_id: get_i64("player_id")?.clamp(0, 8) as u8,
        }),
        "reveal_area" => Some(crate::schema::TriggerAction::RevealArea {
            tile: [get_i64("tile_x")? as i32, get_i64("tile_z")? as i32],
            radius: get_i64("radius").unwrap_or(6) as i32,
        }),
        "change_owner" => Some(crate::schema::TriggerAction::ChangeOwner {
            from_player_id: get_i64("from_player_id")?.clamp(0, 8) as u8,
            to_player_id: get_i64("to_player_id")?.clamp(0, 8) as u8,
            building_id: get_str("building_id").filter(|s| !s.is_empty()),
        }),
        "fire_support_power" => Some(crate::schema::TriggerAction::FireSupportPower {
            player_id: get_i64("player_id").unwrap_or(1).clamp(0, 8) as u8,
            power_id: get_str("power_id")?,
            target_tile: [get_i64("target_x")? as i32, get_i64("target_z")? as i32],
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executes_both_rhai_and_lua_hooks_on_script_context() {
        let mut host = ScriptHost::new();

        let rhai_src = r#"
fn on_gi_deploy(ctx) {
    if ctx.deployed {
        ctx.deployed = false;
        ctx.speed_multiplier = 1.0;
        ctx.armor_multiplier = 1.0;
        ctx.range_bonus = 0.0;
    } else {
        ctx.deployed = true;
        ctx.speed_multiplier = 0.0;
        ctx.armor_multiplier = 1.5;
        ctx.range_bonus = 4.0;
        ctx.flag_sandbagged = true;
    }
    ctx
}
"#;

        let lua_src = r#"
-- Soviet Rhino Tank Overcharge ability in Lua
function on_soviet_overcharge(ctx)
    if ctx.deployed then
        ctx.deployed = false
        ctx.damage_multiplier = 1.0
        ctx.speed_multiplier = 1.0
    else
        ctx.deployed = true
        ctx.damage_multiplier = 1.35
        ctx.speed_multiplier = 0.8
        ctx.flag_overcharged = true
    end
    return ctx
end
"#;

        host.load_script_file("scripts/allied_rules.rhai", rhai_src)
            .expect("Rhai script should load");
        host.load_script_file("scripts/soviet_rules.lua", lua_src)
            .expect("Lua script should load");

        assert_eq!(host.hook_language("on_gi_deploy"), Some(ScriptLang::Rhai));
        assert_eq!(
            host.hook_language("on_soviet_overcharge"),
            Some(ScriptLang::Lua)
        );

        let mut gi_ctx = ScriptContext {
            entity_id: 1,
            unit_id: "allied_gi".to_string(),
            ..Default::default()
        };
        host.invoke_hook("on_gi_deploy", &mut gi_ctx)
            .expect("Rhai hook should execute");
        assert!(gi_ctx.deployed);
        assert_eq!(gi_ctx.speed_multiplier, 0.0);
        assert_eq!(gi_ctx.armor_multiplier, 1.5);
        assert_eq!(gi_ctx.range_bonus, 4.0);
        assert_eq!(gi_ctx.custom_flags.get("sandbagged"), Some(&true));

        let mut rhino_ctx = ScriptContext {
            entity_id: 2,
            unit_id: "soviet_rhino_tank".to_string(),
            ..Default::default()
        };
        host.invoke_hook("on_soviet_overcharge", &mut rhino_ctx)
            .expect("Lua hook should execute");
        assert!(rhino_ctx.deployed);
        assert!((rhino_ctx.damage_multiplier - 1.35).abs() < 1e-4);
        assert!((rhino_ctx.speed_multiplier - 0.8).abs() < 1e-4);
        assert_eq!(rhino_ctx.custom_flags.get("overcharged"), Some(&true));
    }

    #[test]
    fn executes_global_world_event_hooks_and_loads_upgrades_and_maps() {
        let mut host = ScriptHost::new();
        let rhai_world_src = r#"
fn on_building_captured(ctx) {
    ctx.health = 250.0;
    ctx.flag_capture_reward = true;
    ctx
}
"#;
        host.load_script_file("scripts/world_events.rhai", rhai_world_src)
            .expect("World event Rhai script should load");

        let mut ev_ctx = WorldEventContext {
            tick: 120,
            event_type: "building_captured".to_string(),
            entity_id: 42,
            player_id: 1,
            ..Default::default()
        };
        let ran = host
            .invoke_world_event("on_building_captured", &mut ev_ctx)
            .expect("World event should invoke");
        assert!(ran);
        assert_eq!(ev_ctx.bonus_credits, 250);
        assert_eq!(ev_ctx.custom_flags.get("capture_reward"), Some(&true));

        let loaded = crate::loader::LoadedMod::load_from_dir("rust-ra2")
            .expect("rust-ra2 mod should load with upgrades and maps");
        assert!(loaded.upgrades.contains_key("composite_armor"));
        assert!(loaded.upgrades.contains_key("heavy_reactive_plating"));
        assert!(loaded.maps.contains_key("skirmish_valley"));
        assert!(!loaded.entry_map.ramp_rects.is_empty());
    }

    #[test]
    fn rhai_and_lua_world_event_hooks_have_1_to_1_parity_for_vars_and_queued_actions() {
        let mut host = ScriptHost::new();

        let rhai_director = r#"
fn on_rhai_director(ctx) {
    if ctx.var_stage == 1 {
        ctx.var_stage = 2;
        ctx.bonus_credits = 400;
        ctx.focus_tile_x = 18;
        ctx.focus_tile_z = 22;
        ctx.eva_speaker = "Tanya";
        ctx.eva_text = "Bridge secured!";
        ctx.queue_grant_credits(1, 750);
        ctx.queue_spawn_reinforcements(1, "allied_gi", 3, 12, 14);
        ctx.queue_add_variable("kills", 5);
    }
    ctx
}
"#;

        let lua_director = r#"
function on_lua_director(ctx)
    if ctx.var_stage == 1 then
        ctx.var_stage = 2
        ctx.bonus_credits = 400
        ctx.focus_tile_x = 18
        ctx.focus_tile_z = 22
        ctx.eva_speaker = "Tanya"
        ctx.eva_text = "Bridge secured!"
        ctx:queue_grant_credits(1, 750)
        ctx:queue_spawn_reinforcements(1, "allied_gi", 3, 12, 14)
        ctx:queue_add_variable("kills", 5)
    end
    return ctx
end
"#;

        host.load_script_file("scripts/director.rhai", rhai_director)
            .expect("Rhai director should load");
        host.load_script_file("scripts/director.lua", lua_director)
            .expect("Lua director should load");

        let mut rhai_ctx = WorldEventContext {
            event_type: "mission_step".to_string(),
            tick: 100,
            player_id: 1,
            trigger_vars: BTreeMap::from([("stage".to_string(), 1)]),
            ..Default::default()
        };
        let mut lua_ctx = rhai_ctx.clone();

        host.invoke_world_event("on_rhai_director", &mut rhai_ctx)
            .expect("Rhai director should execute");
        host.invoke_world_event("on_lua_director", &mut lua_ctx)
            .expect("Lua director should execute");

        assert_eq!(
            rhai_ctx, lua_ctx,
            "Rhai and Lua must produce identical WorldEventContext"
        );
        assert_eq!(lua_ctx.trigger_vars.get("stage"), Some(&2));
        assert_eq!(lua_ctx.bonus_credits, 400);
        assert_eq!(lua_ctx.camera_focus_tile, Some([18, 22]));
        assert_eq!(
            lua_ctx.eva_transmission,
            Some(("Tanya".to_string(), "Bridge secured!".to_string()))
        );
        assert_eq!(lua_ctx.queued_actions.len(), 3);
    }

    #[test]
    fn rhai_sandbox_blocks_wall_clock_eval_and_unbounded_memory_growth() {
        let mut host = ScriptHost::new();

        let clock_err = host.load_script_file(
            "scripts/bad_clock.rhai",
            r#"
            fn on_clock(ctx) {
                let t = timestamp();
                ctx
            }
            "#,
        );
        if clock_err.is_ok() {
            let mut ctx = ScriptContext::default();
            assert!(
                host.invoke_hook("on_clock", &mut ctx).is_err(),
                "Host wall-clock timestamp() must not be callable in sandboxed Rhai"
            );
        }

        assert!(
            host.load_script_file(
                "scripts/bad_eval.rhai",
                r#"
                fn on_eval(ctx) {
                    eval("ctx.health = 9999.0;");
                    ctx
                }
                "#,
            )
            .is_err(),
            "eval() must be disabled at compile time in sandboxed Rhai"
        );

        host.load_script_file(
            "scripts/oom_string.rhai",
            r#"
            fn on_oom(ctx) {
                let s = "a";
                for i in 0..30 {
                    s += s;
                }
                ctx
            }
            "#,
        )
        .expect("OOM test script compiles");
        let mut oom_ctx = ScriptContext::default();
        assert!(
            host.invoke_hook("on_oom", &mut oom_ctx).is_err(),
            "Exponential string doubling must be stopped by max_string_size(16_384)"
        );
    }

    #[test]
    fn rhai_invoke_hook_preserves_and_toggles_existing_custom_flags() {
        let mut host = ScriptHost::new();
        host.load_script_file(
            "scripts/flag_toggle.rhai",
            r#"
            fn on_sandbag_toggle(ctx) {
                if ctx.flag_sandbagged == true {
                    ctx.flag_sandbagged = false;
                    ctx.armor_multiplier = 1.0;
                } else {
                    ctx.flag_sandbagged = true;
                    ctx.armor_multiplier = 1.5;
                }
                ctx
            }
            "#,
        )
        .expect("flag_toggle.rhai should compile");

        let mut ctx = ScriptContext::default();
        host.invoke_hook("on_sandbag_toggle", &mut ctx)
            .expect("first toggle");
        assert_eq!(ctx.custom_flags.get("sandbagged"), Some(&true));
        assert_eq!(ctx.armor_multiplier, 1.5);

        host.invoke_hook("on_sandbag_toggle", &mut ctx)
            .expect("second toggle reads existing flag_sandbagged");
        assert_eq!(ctx.custom_flags.get("sandbagged"), Some(&false));
        assert_eq!(ctx.armor_multiplier, 1.0);
    }

    #[test]
    fn lua_runtime_supports_locals_elseif_loops_parens_left_assoc_exact_i64_and_isolated_globals() {
        let mut host = ScriptHost::new();
        let lua_src = r#"
shared_counter = 0 -- module baseline global

local function compute_bonus(base)
    return (base + 10) / 2 -- parenthesized + division
end

function on_lua_advanced(ctx)
    shared_counter = shared_counter + 1 -- must NOT leak to subsequent calls
    local sub_check = 10 - 3 - 2 -- left-associative: must be 5, not 9!
    local compact_sub = 10-3-2
    local rem = 17 % 5 -- modulo: 2
    local acc = 0
    for i = 1, 4 do
        acc = acc + i -- 10
    end
    local w = 0
    while w < 3 do
        w = w + 1
    end

    if sub_check ~= 5 or compact_sub ~= 5 then
        ctx.var_error = 1
    elseif rem == 2 and acc == 10 and w == 3 then
        ctx.var_math_ok = compute_bonus(30) + shared_counter -- (30+10)/2 + 1 = 21
    else
        ctx.var_math_ok = -1
    end

    -- Exact 64-bit integer arithmetic above 2^24 (no f32 truncation!)
    ctx.var_exact_id = ctx.entity_id + 1
    ctx.eva_speaker = 'EVA'
    ctx.eva_text = 'Tick ' .. tostring(ctx.tick)
    return ctx
end

function on_lua_infinite(ctx)
    while true do
        ctx.var_spin = 1
    end
    return ctx
end
"#;
        host.load_script_file("scripts/advanced.lua", lua_src)
            .expect("advanced.lua should parse");

        let large_id: u64 = 16_777_219; // > 2^24, where f32 loses precision
        let mut ev1 = WorldEventContext {
            entity_id: large_id,
            tick: 33_554_433,
            ..Default::default()
        };
        host.invoke_world_event("on_lua_advanced", &mut ev1)
            .expect("first Lua invocation");
        assert_eq!(ev1.trigger_vars.get("error"), None);
        assert_eq!(ev1.trigger_vars.get("math_ok"), Some(&21));
        assert_eq!(
            ev1.trigger_vars.get("exact_id"),
            Some(&(large_id as i64 + 1))
        );
        assert_eq!(
            ev1.eva_transmission,
            Some(("EVA".to_string(), "Tick 33554433".to_string()))
        );

        // Second invocation must start with isolated `shared_counter == 0` -> math_ok is still 21 (not 22)
        let mut ev2 = WorldEventContext {
            entity_id: 9_007_199_254_740_993,
            tick: 1,
            ..Default::default()
        };
        host.invoke_world_event("on_lua_advanced", &mut ev2)
            .expect("second Lua invocation");
        assert_eq!(
            ev2.trigger_vars.get("math_ok"),
            Some(&21),
            "Per-call globals must be isolated across Lua hook invocations"
        );
        assert_eq!(
            ev2.trigger_vars.get("exact_id"),
            Some(&9_007_199_254_740_994),
            "Exact i64 arithmetic must preserve integers even above 2^53"
        );

        let mut inf_ctx = WorldEventContext::default();
        assert!(
            host.invoke_world_event("on_lua_infinite", &mut inf_ctx)
                .is_err(),
            "Infinite while loop must terminate deterministically on MAX_LUA_OPS"
        );
    }

    #[test]
    fn script_api_v2_emits_actions_and_runs_zero_clone_spatial_queries_in_rhai_and_lua() {
        let mut host = ScriptHost::new();
        host.load_script_file(
            "scripts/v2_hooks.rhai",
            r#"
            fn on_hero_cast_rhai(ctx) {
                ctx.energy = ctx.energy - 25.0;
                ctx.apply_buff(ctx.target_entity_id, "solar_flare_burn", 6.0, 2);
                ctx.spawn_projectile("solar_lance", ctx.pos_x, ctx.pos_y, ctx.pos_z, ctx.target_x, ctx.target_y, ctx.target_z);
                ctx.spawn_entity("sun_wisp", ctx.player_id, ctx.target_x, ctx.target_y, ctx.target_z);
                ctx.deal_area_damage(ctx.target_x, ctx.target_y, ctx.target_z, 4.5, 120.0, false);
                ctx.issue_command(ctx.entity_id, "attack_move", ctx.target_x, ctx.target_y, ctx.target_z);
                ctx
            }

            fn on_tick_spatial_rhai(ctx) {
                let ids = ctx.query_entities_in_radius(10, 10, 5, 2);
                let cnt = ctx.count_entities_in_radius(10, 10, 5, 2);
                ctx.var_spatial_cnt = cnt;
                if ids.len() > 0 {
                    ctx.var_first_id = ids[0];
                    ctx.apply_buff(ids[0], "marked", 3.0);
                }
                ctx
            }
            "#,
        )
        .expect("rhai v2 script");

        host.load_script_file(
            "scripts/v2_hooks.lua",
            r#"
            function on_hero_cast_lua(ctx)
                ctx.energy = ctx.energy - 25.0
                apply_buff(ctx.target_entity_id, 'solar_flare_burn', 6.0, 2)
                spawn_projectile('solar_lance', ctx.pos_x, ctx.pos_y, ctx.pos_z, ctx.target_x, ctx.target_y, ctx.target_z)
                spawn_entity('sun_wisp', ctx.player_id, ctx.target_x, ctx.target_y, ctx.target_z)
                deal_area_damage(ctx.target_x, ctx.target_y, ctx.target_z, 4.5, 120.0, false)
                issue_command(ctx.entity_id, 'attack_move', ctx.target_x, ctx.target_y, ctx.target_z)
                return ctx
            end

            function on_tick_spatial_lua(ctx)
                local cnt = count_entities_in_radius(10, 10, 5, 2)
                local ids = query_entities_in_radius(10, 10, 5, 2)
                ctx.var_spatial_cnt = cnt
                ctx.var_first_id = ids[1]
                apply_buff(ids[1], 'marked', 3.0)
                return ctx
            end
            "#,
        )
        .expect("lua v2 script");

        for hook in ["on_hero_cast_rhai", "on_hero_cast_lua"] {
            let mut ctx = ScriptContext {
                entity_id: 7,
                player_id: 1,
                self_pos: [2.0, 0.0, 3.0],
                energy: 100.0,
                max_energy: 100.0,
                target_pos: Some([12.0, 0.0, 15.0]),
                target_entity_id: Some(42),
                attacker_id: Some(9),
                ..Default::default()
            };
            host.invoke_hook(hook, &mut ctx).expect(hook);
            assert!((ctx.energy - 75.0).abs() < 1e-4);
            assert_eq!(
                ctx.emitted_actions.len(),
                5,
                "{hook} should emit all 5 v2 actions"
            );
            assert!(matches!(
                &ctx.emitted_actions[0],
                ScriptAction::ApplyBuff { target_entity_id: 42, status_id, stacks: 2, .. }
                if status_id == "solar_flare_burn"
            ));
        }

        let spatial = Arc::new(FlatSpatialScriptGrid::from_entries(vec![
            (101, 2, [11, 12]),
            (102, 2, [13, 10]),
            (103, 1, [10, 10]),
            (104, 2, [40, 40]),
        ]));
        for hook in ["on_tick_spatial_rhai", "on_tick_spatial_lua"] {
            let mut ev = WorldEventContext::default();
            // Intentionally leave `entity_snapshots` empty to prove zero-clone spatial callback works!
            assert!(ev.entity_snapshots.is_empty());
            host.invoke_world_event_with_spatial(hook, &mut ev, Some(spatial.clone()))
                .expect(hook);
            assert_eq!(ev.trigger_vars.get("spatial_cnt"), Some(&2));
            assert_eq!(ev.trigger_vars.get("first_id"), Some(&101));
            assert_eq!(ev.emitted_actions.len(), 1);
        }
    }
}
