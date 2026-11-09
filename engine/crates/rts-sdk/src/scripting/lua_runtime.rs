use anyhow::{anyhow, bail, Context, Result};
use std::collections::BTreeMap;

use crate::schema::TriggerAction;
use super::{ScriptBackend, ScriptContext, ScriptLang, WorldEventContext};

/// Deterministic, sandboxed pure-Rust Lua hook runtime for RTS mods.
/// Supports `function <hook>(ctx) ... return ctx end`, `if ... then ... else ... end`,
/// boolean/numeric/string expressions, comparisons (`==`, `~=`, `>=`, `<=`, `>`, `<`, `and`, `or`),
/// modifying `ctx.<field>` properties, and queueing director `TriggerAction`s via `ctx:queue_*(...)`.
#[derive(Debug, Clone)]
enum LuaStmt {
    AssignField {
        field: String,
        expr: LuaExpr,
    },
    CallQueueMethod {
        method: String,
        args: Vec<LuaExpr>,
    },
    IfElse {
        cond: LuaExpr,
        then_body: Vec<LuaStmt>,
        else_body: Vec<LuaStmt>,
    },
    ReturnCtx,
}

#[derive(Debug, Clone)]
enum LuaExpr {
    Bool(bool),
    Number(f32),
    Str(String),
    Field(String),
    Not(Box<LuaExpr>),
    Add(Box<LuaExpr>, Box<LuaExpr>),
    Sub(Box<LuaExpr>, Box<LuaExpr>),
    Mul(Box<LuaExpr>, Box<LuaExpr>),
    Eq(Box<LuaExpr>, Box<LuaExpr>),
    Ne(Box<LuaExpr>, Box<LuaExpr>),
    Ge(Box<LuaExpr>, Box<LuaExpr>),
    Le(Box<LuaExpr>, Box<LuaExpr>),
    Gt(Box<LuaExpr>, Box<LuaExpr>),
    Lt(Box<LuaExpr>, Box<LuaExpr>),
    And(Box<LuaExpr>, Box<LuaExpr>),
    Or(Box<LuaExpr>, Box<LuaExpr>),
}

#[derive(Debug, Clone)]
struct LuaFunction {
    param_name: String,
    body: Vec<LuaStmt>,
}

#[derive(Default)]
pub struct LuaBackend {
    functions: BTreeMap<String, LuaFunction>,
}

#[derive(Debug, Clone, PartialEq)]
enum LuaValue {
    Bool(bool),
    Number(f32),
    Str(String),
}

impl LuaValue {
    fn as_bool(&self) -> bool {
        match self {
            LuaValue::Bool(b) => *b,
            LuaValue::Number(n) => *n != 0.0,
            LuaValue::Str(s) => !s.is_empty(),
        }
    }

    fn as_f32(&self) -> Result<f32> {
        match self {
            LuaValue::Number(n) => Ok(*n),
            _ => bail!("Expected numeric Lua value, got {self:?}"),
        }
    }

    fn as_i64(&self) -> Result<i64> {
        match self {
            LuaValue::Number(n) => Ok(n.round() as i64),
            _ => bail!("Expected integer Lua value, got {self:?}"),
        }
    }

    fn as_str(&self) -> Result<String> {
        match self {
            LuaValue::Str(s) => Ok(s.clone()),
            _ => bail!("Expected string Lua value, got {self:?}"),
        }
    }
}

/// Internal evaluation state used for full 1:1 `WorldEventContext` execution in Lua.
struct LuaWorldEvalState<'a> {
    ctx: &'a mut WorldEventContext,
    focus_x: Option<i32>,
    focus_z: Option<i32>,
    spawn_tx: Option<i32>,
    spawn_tz: Option<i32>,
    attack_x: Option<f32>,
    attack_z: Option<f32>,
    eva_speaker: Option<String>,
    eva_text: Option<String>,
}

impl LuaBackend {
    pub fn new() -> Self {
        Self::default()
    }

    fn eval_binary_op(op: &str, left: LuaValue, right: LuaValue) -> Result<LuaValue> {
        match op {
            "+" => Ok(LuaValue::Number(left.as_f32()? + right.as_f32()?)),
            "-" => Ok(LuaValue::Number(left.as_f32()? - right.as_f32()?)),
            "*" => Ok(LuaValue::Number(left.as_f32()? * right.as_f32()?)),
            "==" => Ok(LuaValue::Bool(left == right)),
            "~=" => Ok(LuaValue::Bool(left != right)),
            ">=" => Ok(LuaValue::Bool(left.as_f32()? >= right.as_f32()?)),
            "<=" => Ok(LuaValue::Bool(left.as_f32()? <= right.as_f32()?)),
            ">" => Ok(LuaValue::Bool(left.as_f32()? > right.as_f32()?)),
            "<" => Ok(LuaValue::Bool(left.as_f32()? < right.as_f32()?)),
            "and" => Ok(LuaValue::Bool(left.as_bool() && right.as_bool())),
            "or" => Ok(LuaValue::Bool(left.as_bool() || right.as_bool())),
            _ => bail!("Unsupported binary op '{op}'"),
        }
    }

    fn eval_expr_unit(expr: &LuaExpr, ctx: &ScriptContext) -> Result<LuaValue> {
        match expr {
            LuaExpr::Bool(b) => Ok(LuaValue::Bool(*b)),
            LuaExpr::Number(n) => Ok(LuaValue::Number(*n)),
            LuaExpr::Str(s) => Ok(LuaValue::Str(s.clone())),
            LuaExpr::Field(field) => match field.as_str() {
                "entity_id" => Ok(LuaValue::Number(ctx.entity_id as f32)),
                "unit_id" => Ok(LuaValue::Str(ctx.unit_id.clone())),
                "health" => Ok(LuaValue::Number(ctx.health)),
                "max_health" => Ok(LuaValue::Number(ctx.max_health)),
                "speed_multiplier" => Ok(LuaValue::Number(ctx.speed_multiplier)),
                "armor_multiplier" => Ok(LuaValue::Number(ctx.armor_multiplier)),
                "damage_multiplier" => Ok(LuaValue::Number(ctx.damage_multiplier)),
                "range_bonus" => Ok(LuaValue::Number(ctx.range_bonus)),
                "deployed" => Ok(LuaValue::Bool(ctx.deployed)),
                other => {
                    if let Some(flag) = other.strip_prefix("flag_") {
                        let val = ctx.custom_flags.get(flag).copied().unwrap_or(false);
                        Ok(LuaValue::Bool(val))
                    } else {
                        bail!("Unknown ctx field '{other}' in Lua script")
                    }
                }
            },
            LuaExpr::Not(inner) => Ok(LuaValue::Bool(!Self::eval_expr_unit(inner, ctx)?.as_bool())),
            LuaExpr::Add(a, b) => Self::eval_binary_op(
                "+",
                Self::eval_expr_unit(a, ctx)?,
                Self::eval_expr_unit(b, ctx)?,
            ),
            LuaExpr::Sub(a, b) => Self::eval_binary_op(
                "-",
                Self::eval_expr_unit(a, ctx)?,
                Self::eval_expr_unit(b, ctx)?,
            ),
            LuaExpr::Mul(a, b) => Self::eval_binary_op(
                "*",
                Self::eval_expr_unit(a, ctx)?,
                Self::eval_expr_unit(b, ctx)?,
            ),
            LuaExpr::Eq(a, b) => Self::eval_binary_op(
                "==",
                Self::eval_expr_unit(a, ctx)?,
                Self::eval_expr_unit(b, ctx)?,
            ),
            LuaExpr::Ne(a, b) => Self::eval_binary_op(
                "~=",
                Self::eval_expr_unit(a, ctx)?,
                Self::eval_expr_unit(b, ctx)?,
            ),
            LuaExpr::Ge(a, b) => Self::eval_binary_op(
                ">=",
                Self::eval_expr_unit(a, ctx)?,
                Self::eval_expr_unit(b, ctx)?,
            ),
            LuaExpr::Le(a, b) => Self::eval_binary_op(
                "<=",
                Self::eval_expr_unit(a, ctx)?,
                Self::eval_expr_unit(b, ctx)?,
            ),
            LuaExpr::Gt(a, b) => Self::eval_binary_op(
                ">",
                Self::eval_expr_unit(a, ctx)?,
                Self::eval_expr_unit(b, ctx)?,
            ),
            LuaExpr::Lt(a, b) => Self::eval_binary_op(
                "<",
                Self::eval_expr_unit(a, ctx)?,
                Self::eval_expr_unit(b, ctx)?,
            ),
            LuaExpr::And(a, b) => Self::eval_binary_op(
                "and",
                Self::eval_expr_unit(a, ctx)?,
                Self::eval_expr_unit(b, ctx)?,
            ),
            LuaExpr::Or(a, b) => Self::eval_binary_op(
                "or",
                Self::eval_expr_unit(a, ctx)?,
                Self::eval_expr_unit(b, ctx)?,
            ),
        }
    }

    fn eval_expr_world(expr: &LuaExpr, state: &LuaWorldEvalState<'_>) -> Result<LuaValue> {
        match expr {
            LuaExpr::Bool(b) => Ok(LuaValue::Bool(*b)),
            LuaExpr::Number(n) => Ok(LuaValue::Number(*n)),
            LuaExpr::Str(s) => Ok(LuaValue::Str(s.clone())),
            LuaExpr::Field(field) => match field.as_str() {
                "event_type" => Ok(LuaValue::Str(state.ctx.event_type.clone())),
                "tick" => Ok(LuaValue::Number(state.ctx.tick as f32)),
                "entity_id" => Ok(LuaValue::Number(state.ctx.entity_id as f32)),
                "player_id" => Ok(LuaValue::Number(state.ctx.player_id as f32)),
                "health" | "bonus_credits" => Ok(LuaValue::Number(state.ctx.bonus_credits as f32)),
                "spawn_unit_id" => Ok(LuaValue::Str(
                    state.ctx.spawn_unit_id.clone().unwrap_or_default(),
                )),
                "spawn_count" => Ok(LuaValue::Number(state.ctx.spawn_count as f32)),
                "spawn_player_id" => Ok(LuaValue::Number(
                    state.ctx.spawn_player_id.unwrap_or(state.ctx.player_id) as f32,
                )),
                "objective_text" => Ok(LuaValue::Str(
                    state.ctx.objective_text.clone().unwrap_or_default(),
                )),
                "declared_winner" => Ok(LuaValue::Number(
                    state.ctx.declared_winner.unwrap_or(0) as f32,
                )),
                other => {
                    if let Some(flag) = other.strip_prefix("flag_") {
                        let val = state.ctx.custom_flags.get(flag).copied().unwrap_or(false);
                        Ok(LuaValue::Bool(val))
                    } else if let Some(var_name) = other.strip_prefix("var_") {
                        let val = state.ctx.trigger_vars.get(var_name).copied().unwrap_or(0);
                        Ok(LuaValue::Number(val as f32))
                    } else {
                        bail!("Unknown world ctx field '{other}' in Lua script")
                    }
                }
            },
            LuaExpr::Not(inner) => Ok(LuaValue::Bool(
                !Self::eval_expr_world(inner, state)?.as_bool(),
            )),
            LuaExpr::Add(a, b) => Self::eval_binary_op(
                "+",
                Self::eval_expr_world(a, state)?,
                Self::eval_expr_world(b, state)?,
            ),
            LuaExpr::Sub(a, b) => Self::eval_binary_op(
                "-",
                Self::eval_expr_world(a, state)?,
                Self::eval_expr_world(b, state)?,
            ),
            LuaExpr::Mul(a, b) => Self::eval_binary_op(
                "*",
                Self::eval_expr_world(a, state)?,
                Self::eval_expr_world(b, state)?,
            ),
            LuaExpr::Eq(a, b) => Self::eval_binary_op(
                "==",
                Self::eval_expr_world(a, state)?,
                Self::eval_expr_world(b, state)?,
            ),
            LuaExpr::Ne(a, b) => Self::eval_binary_op(
                "~=",
                Self::eval_expr_world(a, state)?,
                Self::eval_expr_world(b, state)?,
            ),
            LuaExpr::Ge(a, b) => Self::eval_binary_op(
                ">=",
                Self::eval_expr_world(a, state)?,
                Self::eval_expr_world(b, state)?,
            ),
            LuaExpr::Le(a, b) => Self::eval_binary_op(
                "<=",
                Self::eval_expr_world(a, state)?,
                Self::eval_expr_world(b, state)?,
            ),
            LuaExpr::Gt(a, b) => Self::eval_binary_op(
                ">",
                Self::eval_expr_world(a, state)?,
                Self::eval_expr_world(b, state)?,
            ),
            LuaExpr::Lt(a, b) => Self::eval_binary_op(
                "<",
                Self::eval_expr_world(a, state)?,
                Self::eval_expr_world(b, state)?,
            ),
            LuaExpr::And(a, b) => Self::eval_binary_op(
                "and",
                Self::eval_expr_world(a, state)?,
                Self::eval_expr_world(b, state)?,
            ),
            LuaExpr::Or(a, b) => Self::eval_binary_op(
                "or",
                Self::eval_expr_world(a, state)?,
                Self::eval_expr_world(b, state)?,
            ),
        }
    }

    fn exec_stmts_unit(stmts: &[LuaStmt], ctx: &mut ScriptContext) -> Result<bool> {
        for stmt in stmts {
            match stmt {
                LuaStmt::ReturnCtx => return Ok(true),
                LuaStmt::CallQueueMethod { method, .. } => {
                    bail!("Queue method '{method}' is only supported in world-event hooks")
                }
                LuaStmt::AssignField { field, expr } => {
                    let val = Self::eval_expr_unit(expr, ctx)?;
                    match (field.as_str(), val) {
                        ("health", LuaValue::Number(n)) => ctx.health = n,
                        ("speed_multiplier", LuaValue::Number(n)) => ctx.speed_multiplier = n,
                        ("armor_multiplier", LuaValue::Number(n)) => ctx.armor_multiplier = n,
                        ("damage_multiplier", LuaValue::Number(n)) => ctx.damage_multiplier = n,
                        ("range_bonus", LuaValue::Number(n)) => ctx.range_bonus = n,
                        ("deployed", LuaValue::Bool(b)) => ctx.deployed = b,
                        (other, LuaValue::Bool(b)) if other.starts_with("flag_") => {
                            let flag_name = other.trim_start_matches("flag_").to_string();
                            ctx.custom_flags.insert(flag_name, b);
                        }
                        (other, _) => {
                            bail!("Invalid assignment to ctx.{other} in Lua unit script")
                        }
                    }
                }
                LuaStmt::IfElse {
                    cond,
                    then_body,
                    else_body,
                } => {
                    let cond_val = Self::eval_expr_unit(cond, ctx)?.as_bool();
                    if cond_val {
                        if Self::exec_stmts_unit(then_body, ctx)? {
                            return Ok(true);
                        }
                    } else if Self::exec_stmts_unit(else_body, ctx)? {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }

    fn exec_stmts_world(stmts: &[LuaStmt], state: &mut LuaWorldEvalState<'_>) -> Result<bool> {
        for stmt in stmts {
            match stmt {
                LuaStmt::ReturnCtx => return Ok(true),
                LuaStmt::AssignField { field, expr } => {
                    let val = Self::eval_expr_world(expr, state)?;
                    match (field.as_str(), val) {
                        ("bonus_credits" | "health", LuaValue::Number(n)) => {
                            state.ctx.bonus_credits = n.round() as i32;
                        }
                        ("spawn_unit_id", LuaValue::Str(s)) => {
                            if !s.is_empty() {
                                state.ctx.spawn_unit_id = Some(s);
                            }
                        }
                        ("spawn_count", LuaValue::Number(n)) => {
                            state.ctx.spawn_count = n.max(0.0).round() as u32;
                        }
                        ("spawn_tile_x", LuaValue::Number(n)) => {
                            state.spawn_tx = Some(n.round() as i32);
                        }
                        ("spawn_tile_z", LuaValue::Number(n)) => {
                            state.spawn_tz = Some(n.round() as i32);
                        }
                        ("spawn_player_id", LuaValue::Number(n)) => {
                            state.ctx.spawn_player_id = Some((n.round() as i32).clamp(0, 8) as u8);
                        }
                        ("attack_move_x", LuaValue::Number(n)) => {
                            state.attack_x = Some(n);
                        }
                        ("attack_move_z", LuaValue::Number(n)) => {
                            state.attack_z = Some(n);
                        }
                        ("objective_text", LuaValue::Str(s)) => {
                            if !s.is_empty() {
                                state.ctx.objective_text = Some(s);
                            }
                        }
                        ("declared_winner", LuaValue::Number(n)) => {
                            let w = n.round() as i32;
                            if w > 0 {
                                state.ctx.declared_winner = Some(w as u8);
                            }
                        }
                        ("focus_tile_x" | "camera_focus_x", LuaValue::Number(n)) => {
                            state.focus_x = Some(n.round() as i32);
                        }
                        ("focus_tile_z" | "camera_focus_z", LuaValue::Number(n)) => {
                            state.focus_z = Some(n.round() as i32);
                        }
                        ("eva_speaker", LuaValue::Str(s)) => {
                            state.eva_speaker = Some(s);
                        }
                        ("eva_text", LuaValue::Str(s)) => {
                            state.eva_text = Some(s);
                        }
                        (other, LuaValue::Bool(b)) if other.starts_with("flag_") => {
                            let flag_name = other.trim_start_matches("flag_").to_string();
                            state.ctx.custom_flags.insert(flag_name, b);
                        }
                        (other, LuaValue::Number(n)) if other.starts_with("var_") => {
                            let var_name = other.trim_start_matches("var_").to_string();
                            state.ctx.trigger_vars.insert(var_name, n.round() as i64);
                        }
                        (other, _) => {
                            bail!("Invalid assignment to world ctx.{other} in Lua script")
                        }
                    }
                }
                LuaStmt::CallQueueMethod { method, args } => {
                    let mut eval_args = Vec::with_capacity(args.len());
                    for a in args {
                        eval_args.push(Self::eval_expr_world(a, state)?);
                    }
                    let action = match method.as_str() {
                        "queue_set_variable" => {
                            if eval_args.len() != 2 {
                                bail!("queue_set_variable expects (name, value)");
                            }
                            TriggerAction::SetVariable {
                                name: eval_args[0].as_str()?,
                                value: eval_args[1].as_i64()?,
                            }
                        }
                        "queue_add_variable" => {
                            if eval_args.len() != 2 {
                                bail!("queue_add_variable expects (name, delta)");
                            }
                            TriggerAction::AddVariable {
                                name: eval_args[0].as_str()?,
                                delta: eval_args[1].as_i64()?,
                            }
                        }
                        "queue_grant_credits" => {
                            if eval_args.len() != 2 {
                                bail!("queue_grant_credits expects (player_id, amount)");
                            }
                            TriggerAction::GrantCredits {
                                player_id: eval_args[0].as_i64()?.clamp(0, 8) as u8,
                                amount: eval_args[1].as_i64()?.max(0) as u32,
                            }
                        }
                        "queue_spawn_reinforcements" => {
                            if eval_args.len() != 5 {
                                bail!(
                                    "queue_spawn_reinforcements expects (player_id, unit_id, count, tile_x, tile_z)"
                                );
                            }
                            TriggerAction::SpawnReinforcements {
                                player_id: eval_args[0].as_i64()?.clamp(0, 8) as u8,
                                unit_id: eval_args[1].as_str()?,
                                count: eval_args[2].as_i64()?.max(1) as u32,
                                tile: [
                                    eval_args[3].as_i64()? as i32,
                                    eval_args[4].as_i64()? as i32,
                                ],
                            }
                        }
                        "queue_spawn_building" => {
                            if eval_args.len() != 4 {
                                bail!(
                                    "queue_spawn_building expects (player_id, building_id, tile_x, tile_z)"
                                );
                            }
                            TriggerAction::SpawnBuilding {
                                player_id: eval_args[0].as_i64()?.clamp(0, 8) as u8,
                                building_id: eval_args[1].as_str()?,
                                tile: [
                                    eval_args[2].as_i64()? as i32,
                                    eval_args[3].as_i64()? as i32,
                                ],
                            }
                        }
                        "queue_focus_camera" => {
                            if eval_args.len() != 2 {
                                bail!("queue_focus_camera expects (tile_x, tile_z)");
                            }
                            TriggerAction::FocusCamera {
                                tile: [
                                    eval_args[0].as_i64()? as i32,
                                    eval_args[1].as_i64()? as i32,
                                ],
                            }
                        }
                        "queue_eva" | "queue_eva_transmission" => {
                            if eval_args.len() != 2 {
                                bail!("queue_eva_transmission expects (speaker, text)");
                            }
                            TriggerAction::EvaTransmission {
                                speaker: eval_args[0].as_str()?,
                                text: eval_args[1].as_str()?,
                            }
                        }
                        "queue_set_objective" => {
                            if eval_args.len() != 1 {
                                bail!("queue_set_objective expects (text)");
                            }
                            TriggerAction::SetObjective {
                                text: eval_args[0].as_str()?,
                            }
                        }
                        "queue_set_weather" => {
                            if eval_args.len() != 1 {
                                bail!("queue_set_weather expects (preset)");
                            }
                            TriggerAction::SetWeather {
                                preset: eval_args[0].as_str()?,
                            }
                        }
                        "queue_declare_winner" => {
                            if eval_args.len() != 1 {
                                bail!("queue_declare_winner expects (player_id)");
                            }
                            TriggerAction::DeclareWinner {
                                player_id: eval_args[0].as_i64()?.clamp(0, 8) as u8,
                            }
                        }
                        "queue_reveal_area" => {
                            if eval_args.len() != 3 {
                                bail!("queue_reveal_area expects (tile_x, tile_z, radius)");
                            }
                            TriggerAction::RevealArea {
                                tile: [
                                    eval_args[0].as_i64()? as i32,
                                    eval_args[1].as_i64()? as i32,
                                ],
                                radius: eval_args[2].as_i64()? as i32,
                            }
                        }
                        other => bail!("Unknown Lua world queue method '{other}'"),
                    };
                    state.ctx.queued_actions.push(action);
                }
                LuaStmt::IfElse {
                    cond,
                    then_body,
                    else_body,
                } => {
                    let cond_val = Self::eval_expr_world(cond, state)?.as_bool();
                    if cond_val {
                        if Self::exec_stmts_world(then_body, state)? {
                            return Ok(true);
                        }
                    } else if Self::exec_stmts_world(else_body, state)? {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }

    fn split_outside_quotes<'a>(s: &'a str, pat: &str) -> Option<(&'a str, &'a str)> {
        let mut in_quotes = false;
        let bytes = s.as_bytes();
        let pat_bytes = pat.as_bytes();
        if pat_bytes.is_empty() || bytes.len() < pat_bytes.len() {
            return None;
        }
        for i in 0..=(bytes.len() - pat_bytes.len()) {
            if bytes[i] == b'"' {
                in_quotes = !in_quotes;
            }
            if !in_quotes && &bytes[i..i + pat_bytes.len()] == pat_bytes {
                return Some((&s[..i], &s[i + pat_bytes.len()..]));
            }
        }
        None
    }

    fn split_args_outside_quotes(s: &str) -> Vec<&str> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }
        let mut parts = Vec::new();
        let mut in_quotes = false;
        let mut start = 0;
        for (i, ch) in trimmed.char_indices() {
            if ch == '"' {
                in_quotes = !in_quotes;
            } else if ch == ',' && !in_quotes {
                parts.push(trimmed[start..i].trim());
                start = i + 1;
            }
        }
        parts.push(trimmed[start..].trim());
        parts
    }

    fn parse_expr(raw: &str, param_name: &str) -> Result<LuaExpr> {
        let s = raw.trim();
        if s == "true" {
            return Ok(LuaExpr::Bool(true));
        }
        if s == "false" {
            return Ok(LuaExpr::Bool(false));
        }
        if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
            return Ok(LuaExpr::Str(s[1..s.len() - 1].to_string()));
        }
        if let Some((l, r)) = Self::split_outside_quotes(s, " or ") {
            return Ok(LuaExpr::Or(
                Box::new(Self::parse_expr(l, param_name)?),
                Box::new(Self::parse_expr(r, param_name)?),
            ));
        }
        if let Some((l, r)) = Self::split_outside_quotes(s, " and ") {
            return Ok(LuaExpr::And(
                Box::new(Self::parse_expr(l, param_name)?),
                Box::new(Self::parse_expr(r, param_name)?),
            ));
        }
        if let Some(rest) = s.strip_prefix("not ") {
            return Ok(LuaExpr::Not(Box::new(Self::parse_expr(rest, param_name)?)));
        }
        if let Some((l, r)) = Self::split_outside_quotes(s, "==") {
            return Ok(LuaExpr::Eq(
                Box::new(Self::parse_expr(l, param_name)?),
                Box::new(Self::parse_expr(r, param_name)?),
            ));
        }
        if let Some((l, r)) = Self::split_outside_quotes(s, "~=") {
            return Ok(LuaExpr::Ne(
                Box::new(Self::parse_expr(l, param_name)?),
                Box::new(Self::parse_expr(r, param_name)?),
            ));
        }
        if let Some((l, r)) = Self::split_outside_quotes(s, ">=") {
            return Ok(LuaExpr::Ge(
                Box::new(Self::parse_expr(l, param_name)?),
                Box::new(Self::parse_expr(r, param_name)?),
            ));
        }
        if let Some((l, r)) = Self::split_outside_quotes(s, "<=") {
            return Ok(LuaExpr::Le(
                Box::new(Self::parse_expr(l, param_name)?),
                Box::new(Self::parse_expr(r, param_name)?),
            ));
        }
        if let Some((l, r)) = Self::split_outside_quotes(s, ">") {
            return Ok(LuaExpr::Gt(
                Box::new(Self::parse_expr(l, param_name)?),
                Box::new(Self::parse_expr(r, param_name)?),
            ));
        }
        if let Some((l, r)) = Self::split_outside_quotes(s, "<") {
            return Ok(LuaExpr::Lt(
                Box::new(Self::parse_expr(l, param_name)?),
                Box::new(Self::parse_expr(r, param_name)?),
            ));
        }
        if let Some((l, r)) = Self::split_outside_quotes(s, "+") {
            return Ok(LuaExpr::Add(
                Box::new(Self::parse_expr(l, param_name)?),
                Box::new(Self::parse_expr(r, param_name)?),
            ));
        }
        if let Some((l, r)) = Self::split_outside_quotes(s, " - ") {
            return Ok(LuaExpr::Sub(
                Box::new(Self::parse_expr(l, param_name)?),
                Box::new(Self::parse_expr(r, param_name)?),
            ));
        }
        if let Some((l, r)) = Self::split_outside_quotes(s, "*") {
            return Ok(LuaExpr::Mul(
                Box::new(Self::parse_expr(l, param_name)?),
                Box::new(Self::parse_expr(r, param_name)?),
            ));
        }
        let prefix = format!("{param_name}.");
        if let Some(field) = s.strip_prefix(&prefix) {
            return Ok(LuaExpr::Field(field.trim().to_string()));
        }
        if let Ok(num) = s.parse::<f32>() {
            return Ok(LuaExpr::Number(num));
        }
        bail!("Unsupported Lua expression: '{s}'")
    }

    fn parse_block(lines: &[&str], idx: &mut usize, param_name: &str) -> Result<Vec<LuaStmt>> {
        let mut stmts = Vec::new();
        while *idx < lines.len() {
            let line = lines[*idx].trim();
            if line.is_empty() || line.starts_with("--") {
                *idx += 1;
                continue;
            }
            if line == "end" || line == "else" {
                break;
            }
            if let Some(rest) = line.strip_prefix("if ") {
                let cond_str = rest
                    .strip_suffix(" then")
                    .ok_or_else(|| anyhow!("Lua 'if' missing 'then' in line: '{line}'"))?;
                let cond = Self::parse_expr(cond_str, param_name)?;
                *idx += 1;
                let then_body = Self::parse_block(lines, idx, param_name)?;
                let mut else_body = Vec::new();
                if *idx < lines.len() && lines[*idx].trim() == "else" {
                    *idx += 1;
                    else_body = Self::parse_block(lines, idx, param_name)?;
                }
                if *idx >= lines.len() || lines[*idx].trim() != "end" {
                    bail!("Lua 'if' block missing closing 'end'");
                }
                *idx += 1;
                stmts.push(LuaStmt::IfElse {
                    cond,
                    then_body,
                    else_body,
                });
                continue;
            }
            if let Some(ret_val) = line.strip_prefix("return ") {
                if ret_val.trim() != param_name {
                    bail!("Lua hook must return '{param_name}', got '{line}'");
                }
                stmts.push(LuaStmt::ReturnCtx);
                *idx += 1;
                continue;
            }
            let colon_prefix = format!("{param_name}:");
            let dot_prefix = format!("{param_name}.");
            if (line.starts_with(&colon_prefix) || line.starts_with(&dot_prefix))
                && line.ends_with(')')
                && !line.contains('=')
            {
                let after_receiver = &line[param_name.len() + 1..];
                if let Some((method_name, rest_args)) = after_receiver.split_once('(') {
                    let inner_args = rest_args.strip_suffix(')').unwrap_or(rest_args);
                    let raw_args = Self::split_args_outside_quotes(inner_args);
                    let mut parsed_args = Vec::with_capacity(raw_args.len());
                    for a in raw_args {
                        parsed_args.push(Self::parse_expr(a, param_name)?);
                    }
                    stmts.push(LuaStmt::CallQueueMethod {
                        method: method_name.trim().to_string(),
                        args: parsed_args,
                    });
                    *idx += 1;
                    continue;
                }
            }
            if let Some((lhs, rhs)) = Self::split_outside_quotes(line, "=") {
                let prefix = format!("{param_name}.");
                let field = lhs.trim().strip_prefix(&prefix).ok_or_else(|| {
                    anyhow!("Lua assignment must target '{param_name}.<field>' in '{line}'")
                })?;
                let expr = Self::parse_expr(rhs, param_name)?;
                stmts.push(LuaStmt::AssignField {
                    field: field.trim().to_string(),
                    expr,
                });
                *idx += 1;
                continue;
            }
            bail!("Invalid Lua statement: '{line}'");
        }
        Ok(stmts)
    }

    /// Executes a world-event hook with full 1:1 `WorldEventContext` parity.
    pub fn invoke_world_event(&self, hook_name: &str, ctx: &mut WorldEventContext) -> Result<()> {
        let func = self
            .functions
            .get(hook_name)
            .ok_or_else(|| anyhow!("Lua world hook '{hook_name}' not found"))?;
        let mut state = LuaWorldEvalState {
            focus_x: ctx.camera_focus_tile.map(|t| t[0]),
            focus_z: ctx.camera_focus_tile.map(|t| t[1]),
            spawn_tx: ctx.spawn_tile.map(|t| t[0]),
            spawn_tz: ctx.spawn_tile.map(|t| t[1]),
            attack_x: ctx.attack_move_target.map(|p| p[0]),
            attack_z: ctx.attack_move_target.map(|p| p[2]),
            eva_speaker: ctx.eva_transmission.as_ref().map(|(s, _)| s.clone()),
            eva_text: ctx.eva_transmission.as_ref().map(|(_, t)| t.clone()),
            ctx,
        };
        Self::exec_stmts_world(&func.body, &mut state)?;
        if let (Some(tx), Some(tz)) = (state.spawn_tx, state.spawn_tz) {
            state.ctx.spawn_tile = Some([tx, tz]);
        }
        if let (Some(ax), Some(az)) = (state.attack_x, state.attack_z) {
            state.ctx.attack_move_target = Some([ax, 0.0, az]);
        }
        if let (Some(fx), Some(fz)) = (state.focus_x, state.focus_z) {
            state.ctx.camera_focus_tile = Some([fx, fz]);
        }
        if let Some(text) = state.eva_text {
            if !text.is_empty() {
                let speaker = state.eva_speaker.unwrap_or_else(|| "EVA".to_string());
                state.ctx.eva_transmission = Some((speaker, text));
            }
        }
        Ok(())
    }
}

impl ScriptBackend for LuaBackend {
    fn language(&self) -> ScriptLang {
        ScriptLang::Lua
    }

    fn load_script(&mut self, name: &str, source: &str) -> Result<Vec<String>> {
        let lines: Vec<&str> = source.lines().collect();
        let mut idx = 0;
        let mut hooks = Vec::new();

        while idx < lines.len() {
            let line = lines[idx].trim();
            if line.is_empty() || line.starts_with("--") {
                idx += 1;
                continue;
            }
            if let Some(rest) = line.strip_prefix("function ") {
                let (fn_name, param_part) = rest
                    .split_once('(')
                    .ok_or_else(|| anyhow!("Invalid Lua function header in '{name}': '{line}'"))?;
                let param_name = param_part
                    .strip_suffix(')')
                    .ok_or_else(|| {
                        anyhow!("Missing ')' in Lua function header in '{name}': '{line}'")
                    })?
                    .trim()
                    .to_string();
                let fn_name = fn_name.trim().to_string();
                if fn_name.is_empty() || param_name.is_empty() {
                    bail!("Empty Lua function name or parameter in '{name}'");
                }
                idx += 1;
                let body = Self::parse_block(&lines, &mut idx, &param_name)
                    .with_context(|| format!("In Lua function '{fn_name}' of '{name}'"))?;
                if idx >= lines.len() || lines[idx].trim() != "end" {
                    bail!("Lua function '{fn_name}' in '{name}' missing closing 'end'");
                }
                idx += 1;
                hooks.push(fn_name.clone());
                self.functions
                    .insert(fn_name, LuaFunction { param_name, body });
            } else {
                bail!("Unexpected top-level Lua line in '{name}': '{line}'");
            }
        }
        Ok(hooks)
    }

    fn has_hook(&self, hook_name: &str) -> bool {
        self.functions.contains_key(hook_name)
    }

    fn invoke_hook(&self, hook_name: &str, ctx: &mut ScriptContext) -> Result<()> {
        let func = self
            .functions
            .get(hook_name)
            .ok_or_else(|| anyhow!("Lua hook '{hook_name}' not found"))?;
        let _ = &func.param_name;
        Self::exec_stmts_unit(&func.body, ctx)?;
        Ok(())
    }
}
