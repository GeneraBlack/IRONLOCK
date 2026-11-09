pub mod lua_runtime;

use anyhow::{anyhow, Result};
use rhai::{Dynamic, Engine, Map, Scope, AST};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub use lua_runtime::LuaBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScriptContext {
    pub entity_id: u64,
    pub unit_id: String,
    pub health: f32,
    pub max_health: f32,
    pub speed_multiplier: f32,
    pub armor_multiplier: f32,
    pub damage_multiplier: f32,
    pub range_bonus: f32,
    pub deployed: bool,
    pub custom_flags: BTreeMap<String, bool>,
}

impl Default for ScriptContext {
    fn default() -> Self {
        Self {
            entity_id: 0,
            unit_id: String::new(),
            health: 100.0,
            max_health: 100.0,
            speed_multiplier: 1.0,
            armor_multiplier: 1.0,
            damage_multiplier: 1.0,
            range_bonus: 0.0,
            deployed: false,
            custom_flags: BTreeMap::new(),
        }
    }
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
        let mut engine = Engine::new();
        engine.set_max_operations(50_000);

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

        engine.register_fn("queue_set_variable", |ctx: &mut Map, name: &str, value: i64| {
            let mut m = Map::new();
            m.insert("type".into(), Dynamic::from("set_variable".to_string()));
            m.insert("name".into(), Dynamic::from(name.to_string()));
            m.insert("value".into(), Dynamic::from_int(value));
            push_action_to_map(ctx, m);
        });
        engine.register_fn("queue_add_variable", |ctx: &mut Map, name: &str, delta: i64| {
            let mut m = Map::new();
            m.insert("type".into(), Dynamic::from("add_variable".to_string()));
            m.insert("name".into(), Dynamic::from(name.to_string()));
            m.insert("delta".into(), Dynamic::from_int(delta));
            push_action_to_map(ctx, m);
        });
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
        engine.register_fn("queue_focus_camera", |ctx: &mut Map, tile_x: i64, tile_z: i64| {
            let mut m = Map::new();
            m.insert("type".into(), Dynamic::from("focus_camera".to_string()));
            m.insert("tile_x".into(), Dynamic::from_int(tile_x));
            m.insert("tile_z".into(), Dynamic::from_int(tile_z));
            push_action_to_map(ctx, m);
        });
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

        Self {
            engine,
            asts: BTreeMap::new(),
            hook_to_script: BTreeMap::new(),
        }
    }
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
            self.hook_to_script.insert(fn_name.clone(), name.to_string());
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
        map.insert("health".into(), Dynamic::from_float(ctx.health as rhai::FLOAT));
        map.insert(
            "max_health".into(),
            Dynamic::from_float(ctx.max_health as rhai::FLOAT),
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

        let mut scope = Scope::new();
        let result: Map = self
            .engine
            .call_fn(&mut scope, ast, hook_name, (map,))
            .map_err(|e| anyhow!("Error executing Rhai hook '{hook_name}': {e}"))?;

        if let Some(v) = result.get("health").and_then(|v| v.as_float().ok()) {
            ctx.health = v as f32;
        }
        if let Some(v) = result
            .get("speed_multiplier")
            .and_then(|v| v.as_float().ok())
        {
            ctx.speed_multiplier = v as f32;
        }
        if let Some(v) = result
            .get("armor_multiplier")
            .and_then(|v| v.as_float().ok())
        {
            ctx.armor_multiplier = v as f32;
        }
        if let Some(v) = result
            .get("damage_multiplier")
            .and_then(|v| v.as_float().ok())
        {
            ctx.damage_multiplier = v as f32;
        }
        if let Some(v) = result.get("range_bonus").and_then(|v| v.as_float().ok()) {
            ctx.range_bonus = v as f32;
        }
        if let Some(v) = result.get("deployed").and_then(|v| v.as_bool().ok()) {
            ctx.deployed = v;
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
        let lang = self
            .hook_languages
            .get(hook_name)
            .copied()
            .ok_or_else(|| anyhow!("Script hook '{hook_name}' is not registered in Rhai or Lua"))?;
        match lang {
            ScriptLang::Rhai => self.rhai.invoke_hook(hook_name, ctx),
            ScriptLang::Lua => self.lua.invoke_hook(hook_name, ctx),
        }
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

        assert_eq!(rhai_ctx, lua_ctx, "Rhai and Lua must produce identical WorldEventContext");
        assert_eq!(lua_ctx.trigger_vars.get("stage"), Some(&2));
        assert_eq!(lua_ctx.bonus_credits, 400);
        assert_eq!(lua_ctx.camera_focus_tile, Some([18, 22]));
        assert_eq!(
            lua_ctx.eva_transmission,
            Some(("Tanya".to_string(), "Bridge secured!".to_string()))
        );
        assert_eq!(lua_ctx.queued_actions.len(), 3);
    }
}
