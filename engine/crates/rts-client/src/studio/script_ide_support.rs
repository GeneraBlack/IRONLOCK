use bevy_egui::egui;
use egui::text::{LayoutJob, TextFormat};
use rts_simulation::world::SimWorld;

#[derive(Debug, Clone, Default)]
pub struct ScriptIdeState {
    pub show_api_reference: bool,
    pub api_search_filter: String,
    pub pending_jump_line: Option<usize>,
    pub new_watch_var_name: String,
    pub new_watch_var_val: i64,
    pub new_watch_flag_name: String,
}

pub struct ScriptApiDocEntry {
    pub symbol: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub rhai_snippet: &'static str,
    pub lua_snippet: &'static str,
}

pub const SCRIPT_API_CATALOG: &[ScriptApiDocEntry] = &[
    ScriptApiDocEntry {
        symbol: "on_spawn(ctx)",
        category: "Unit/Building Hook",
        description: "Invoked when an entity with `script_hooks.on_spawn` enters the world.",
        rhai_snippet: "fn on_spawn(ctx) {\n    ctx.set_flag(\"initialized\", true);\n    ctx\n}\n",
        lua_snippet: "function on_spawn(ctx)\n    ctx.custom_flags[\"initialized\"] = true\n    return ctx\nend\n",
    },
    ScriptApiDocEntry {
        symbol: "on_damaged(ctx)",
        category: "Unit/Building Hook",
        description: "Invoked whenever an entity receives combat damage (`ctx.health`, `ctx.max_health`).",
        rhai_snippet: "fn on_damaged(ctx) {\n    if ctx.health < ctx.max_health * 0.3 {\n        ctx.speed_multiplier = 1.25;\n    }\n    ctx\n}\n",
        lua_snippet: "function on_damaged(ctx)\n    if ctx.health < ctx.max_health * 0.3 then\n        ctx.speed_multiplier = 1.25\n    end\n    return ctx\nend\n",
    },
    ScriptApiDocEntry {
        symbol: "on_ability(ctx)",
        category: "Unit/Building Hook",
        description: "Custom active ability logic invoked when an `AbilityDef` fires.",
        rhai_snippet: "fn on_ability(ctx) {\n    ctx.heal(100.0);\n    ctx.damage_multiplier = 1.35;\n    ctx\n}\n",
        lua_snippet: "function on_ability(ctx)\n    ctx.health = math.min(ctx.max_health, ctx.health + 100.0)\n    return ctx\nend\n",
    },
    ScriptApiDocEntry {
        symbol: "on_trigger(ev)",
        category: "World Event / Mission Hook",
        description: "World trigger zone & mission hook with full access to `trigger_vars`, objectives, spawns & EVA.",
        rhai_snippet: "fn on_mission_wave(ev) {\n    ev.eva(\"EVA\", \"Enemy reinforcements approaching!\");\n    ev.spawn_units(2, \"grizzly_tank\", 3, 24, 24);\n    ev.set_var(\"wave_index\", ev.get_var(\"wave_index\") + 1);\n    ev\n}\n",
        lua_snippet: "function on_mission_wave(ev)\n    ev.eva_speaker = \"EVA\"\n    ev.eva_text = \"Enemy reinforcements approaching!\"\n    ev.spawn_player_id = 2\n    ev.spawn_unit_id = \"grizzly_tank\"\n    ev.spawn_count = 3\n    return ev\nend\n",
    },
    ScriptApiDocEntry {
        symbol: "ev.get_var / ev.set_var",
        category: "Trigger Variables",
        description: "Reads or writes deterministic integer variables in `world.trigger_vars`.",
        rhai_snippet: "let stage = ev.get_var(\"mission_stage\");\nev.set_var(\"mission_stage\", stage + 1);\n",
        lua_snippet: "local stage = ev.trigger_vars[\"mission_stage\"] or 0\nev.trigger_vars[\"mission_stage\"] = stage + 1\n",
    },
    ScriptApiDocEntry {
        symbol: "ev.grant_resource(player_id, res_id, delta)",
        category: "Economy & Credits",
        description: "Grants or deducts `credits` or custom `ResourceTypeDef` amounts for `player_id`.",
        rhai_snippet: "ev.grant_resource(1, \"credits\", 1500);\n",
        lua_snippet: "ev.bonus_credits = 1500\n",
    },
    ScriptApiDocEntry {
        symbol: "ev.focus_camera(tx, tz)",
        category: "Camera & Cutscenes",
        description: "Snaps the player camera to tile `(tx, tz)` and pings the tactical radar.",
        rhai_snippet: "ev.focus_camera(32, 32);\n",
        lua_snippet: "ev.camera_focus_x = 32\nev.camera_focus_z = 32\n",
    },
    ScriptApiDocEntry {
        symbol: "on_ai_tick(ev)",
        category: "AI Commander Hook",
        description: "Scriptable AI doctrine callback referenced by `AiProfileDef.on_ai_tick`.",
        rhai_snippet: "fn on_ai_tick(ev) {\n    if ev.tick % 400 == 0 {\n        ev.set_flag(\"ai_aggressive_push\", true);\n    }\n    ev\n}\n",
        lua_snippet: "function on_ai_tick(ev)\n    if ev.tick % 400 == 0 then\n        ev.custom_flags[\"ai_aggressive_push\"] = true\n    end\n    return ev\nend\n",
    },
];

/// Extracts a 1-based line number from Rhai (`(line 12, position 5)`) or Lua (`:12:`) error strings.
pub fn extract_error_line_number(err_msg: &str) -> Option<usize> {
    let lower = err_msg.to_ascii_lowercase();
    if let Some(idx) = lower.find("line ") {
        let rest = &lower[idx + 5..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(line) = digits.parse::<usize>() {
            if line >= 1 {
                return Some(line);
            }
        }
    }
    // Fallback for Lua `:14: unexpected symbol`
    let bytes = err_msg.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b':' {
            let tail = &err_msg[i + 1..];
            let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() && tail[digits.len()..].starts_with(':') {
                if let Ok(line) = digits.parse::<usize>() {
                    if line >= 1 {
                        return Some(line);
                    }
                }
            }
        }
    }
    None
}

/// Computes the character offset of 1-based `target_line` within `source`.
pub fn char_index_for_line(source: &str, target_line: usize) -> usize {
    if target_line <= 1 {
        return 0;
    }
    let mut current_line = 1usize;
    for (char_idx, ch) in source.chars().enumerate() {
        if ch == '\n' {
            current_line += 1;
            if current_line == target_line {
                return char_idx + 1;
            }
        }
    }
    source.chars().count()
}

fn is_script_keyword(word: &str, is_lua: bool) -> bool {
    if is_lua {
        matches!(
            word,
            "function"
                | "local"
                | "if"
                | "then"
                | "else"
                | "elseif"
                | "end"
                | "for"
                | "in"
                | "while"
                | "do"
                | "repeat"
                | "until"
                | "return"
                | "and"
                | "or"
                | "not"
                | "true"
                | "false"
                | "nil"
        )
    } else {
        matches!(
            word,
            "fn" | "let"
                | "const"
                | "if"
                | "else"
                | "for"
                | "in"
                | "while"
                | "loop"
                | "break"
                | "continue"
                | "return"
                | "switch"
                | "true"
                | "false"
                | "private"
                | "import"
                | "export"
        )
    }
}

fn is_engine_builtin(word: &str) -> bool {
    matches!(
        word,
        "ctx"
            | "ev"
            | "set_flag"
            | "get_flag"
            | "get_var"
            | "set_var"
            | "spawn_units"
            | "grant_resource"
            | "focus_camera"
            | "eva"
            | "heal"
            | "query_entities_in_radius"
            | "count_entities_in_radius"
    )
}

/// Builds a syntax-highlighted `egui::text::LayoutJob` for `.rhai` and `.lua` source code.
pub fn build_script_highlight_layout_job(source: &str, is_lua: bool, wrap_width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;

    let font_id = egui::FontId::monospace(12.5);
    let fmt_default = TextFormat::simple(font_id.clone(), egui::Color32::from_rgb(220, 228, 238));
    let fmt_kw = TextFormat::simple(font_id.clone(), egui::Color32::from_rgb(198, 138, 255));
    let fmt_builtin = TextFormat::simple(font_id.clone(), egui::Color32::from_rgb(95, 205, 250));
    let fmt_str = TextFormat::simple(font_id.clone(), egui::Color32::from_rgb(155, 228, 145));
    let fmt_num = TextFormat::simple(font_id.clone(), egui::Color32::from_rgb(248, 182, 108));
    let fmt_comment = TextFormat::simple(font_id, egui::Color32::from_rgb(122, 142, 160));

    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;

    while i < len {
        // Line comments: `//` (Rhai) or `--` (Lua)
        if (!is_lua && i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'/')
            || (is_lua && i + 1 < len && bytes[i] == b'-' && bytes[i + 1] == b'-')
        {
            let start = i;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            job.append(&source[start..i], 0.0, fmt_comment.clone());
            continue;
        }

        // String literals (`"..."` or `'...'`)
        if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            let start = i;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 2;
                    continue;
                }
                if bytes[i] == quote {
                    i += 1;
                    break;
                }
                if bytes[i] == b'\n' {
                    break;
                }
                i += 1;
            }
            job.append(&source[start..i], 0.0, fmt_str.clone());
            continue;
        }

        // Identifiers / Keywords
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &source[start..i];
            let fmt = if is_script_keyword(word, is_lua) {
                &fmt_kw
            } else if is_engine_builtin(word) {
                &fmt_builtin
            } else {
                &fmt_default
            };
            job.append(word, 0.0, fmt.clone());
            continue;
        }

        // Numbers
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'.' || bytes[i] == b'_') {
                i += 1;
            }
            job.append(&source[start..i], 0.0, fmt_num.clone());
            continue;
        }

        // Single UTF-8 character fallback
        let ch = source[i..].chars().next().unwrap_or(' ');
        let next = i + ch.len_utf8();
        job.append(&source[i..next], 0.0, fmt_default.clone());
        i = next;
    }

    job
}

/// Renders a syntax-highlighted `.rhai`/`.lua` code editor with a synchronized line-number gutter
/// and support for jumping the text cursor directly to `ide_state.pending_jump_line`.
pub fn show_highlighted_code_editor(
    ui: &mut egui::Ui,
    source_buf: &mut String,
    is_lua: bool,
    error_line_opt: Option<usize>,
    ide_state: &mut ScriptIdeState,
) -> bool {
    let mut changed = false;
    let line_count = source_buf.lines().count().max(1);
    let editor_id = ui.make_persistent_id("studio_script_code_editor_te");

    egui::ScrollArea::vertical()
        .id_salt("studio_script_ide_scroll_area")
        .max_height(260.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                // Line-number gutter
                let mut gutter_job = LayoutJob::default();
                let gutter_font = egui::FontId::monospace(12.5);
                for line_no in 1..=line_count {
                    let is_err = error_line_opt == Some(line_no);
                    let col = if is_err {
                        egui::Color32::from_rgb(255, 105, 105)
                    } else {
                        egui::Color32::from_rgb(115, 130, 150)
                    };
                    let prefix = if is_err { "▶" } else { " " };
                    let suffix = if line_no < line_count { "\n" } else { "" };
                    gutter_job.append(
                        &format!("{prefix}{line_no:>3}{suffix}"),
                        0.0,
                        TextFormat::simple(gutter_font.clone(), col),
                    );
                }
                ui.add(egui::Label::new(gutter_job).selectable(false));
                ui.separator();

                let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                    let job = build_script_highlight_layout_job(text, is_lua, wrap_width);
                    ui.fonts(|f| f.layout_job(job))
                };

                let out = egui::TextEdit::multiline(source_buf)
                    .id(editor_id)
                    .code_editor()
                    .desired_rows(12)
                    .desired_width(f32::INFINITY)
                    .layouter(&mut layouter)
                    .show(ui);

                if out.response.changed() {
                    changed = true;
                }

                if let Some(jump_line) = ide_state.pending_jump_line.take() {
                    let char_idx = char_index_for_line(source_buf, jump_line);
                    let mut te_state = out.state;
                    te_state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(
                            egui::text::CCursor::new(char_idx),
                        )));
                    te_state.store(ui.ctx(), editor_id);
                    out.response.request_focus();
                }
            });
        });

    changed
}

/// Renders the searchable Script API Reference & 1-click Snippet Inserter.
pub fn show_script_api_reference_panel(
    ui: &mut egui::Ui,
    is_lua: bool,
    ide_state: &mut ScriptIdeState,
    source_buf: &mut String,
) -> bool {
    let mut inserted = false;
    egui::CollapsingHeader::new("📚 Script API Reference & 1-Click Snippets")
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Search API:");
                ui.add(
                    egui::TextEdit::singleline(&mut ide_state.api_search_filter)
                        .desired_width(180.0)
                        .hint_text("spawn, trigger_vars, on_damaged..."),
                );
                if !ide_state.api_search_filter.is_empty() && ui.small_button("✖").clicked() {
                    ide_state.api_search_filter.clear();
                }
            });

            let q = ide_state.api_search_filter.trim().to_lowercase();
            for entry in SCRIPT_API_CATALOG {
                if !q.is_empty()
                    && !entry.symbol.to_lowercase().contains(&q)
                    && !entry.category.to_lowercase().contains(&q)
                    && !entry.description.to_lowercase().contains(&q)
                {
                    continue;
                }
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .small_button("➕ Insert")
                        .on_hover_text(if is_lua {
                            entry.lua_snippet
                        } else {
                            entry.rhai_snippet
                        })
                        .clicked()
                    {
                        if !source_buf.ends_with('\n') && !source_buf.is_empty() {
                            source_buf.push('\n');
                        }
                        source_buf.push_str(if is_lua {
                            entry.lua_snippet
                        } else {
                            entry.rhai_snippet
                        });
                        inserted = true;
                    }
                    ui.monospace(
                        egui::RichText::new(entry.symbol)
                            .color(egui::Color32::from_rgb(105, 210, 250)),
                    );
                    ui.weak(format!("[{}] — {}", entry.category, entry.description));
                });
            }
        });
    inserted
}

/// Renders the Live Playtest Watch Table (`world.trigger_vars`, `world.world_custom_flags`,
/// `world.mission_objectives`, and `world.countdown_timers`) for Trigger Studio & Diagnostics.
pub fn show_live_playtest_watch_table(
    ui: &mut egui::Ui,
    world: &mut SimWorld,
    ide_state: &mut ScriptIdeState,
    salt: &str,
) {
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!(
            "🔭 Live Watch — Tick {} | Vars: {} | Flags: {} | Objectives: {} | Timers: {}",
            world.tick,
            world.trigger_vars.len(),
            world.world_custom_flags.len(),
            world.mission_objectives.len(),
            world.countdown_timers.len()
        ));
    });

    ui.columns(2, |cols| {
        // Left column: Trigger Variables & Custom Flags
        cols[0].strong("Trigger Variables (`world.trigger_vars`)");
        let var_keys: Vec<String> = world.trigger_vars.keys().cloned().collect();
        if var_keys.is_empty() {
            cols[0].weak("No trigger_vars set yet.");
        } else {
            let mut var_remove: Option<String> = None;
            for k in var_keys {
                cols[0].horizontal(|ui| {
                    ui.monospace(&k);
                    if let Some(v) = world.trigger_vars.get_mut(&k) {
                        ui.add(egui::DragValue::new(v).speed(1));
                    }
                    if ui.small_button("✖").clicked() {
                        var_remove = Some(k);
                    }
                });
            }
            if let Some(rem) = var_remove {
                world.trigger_vars.remove(&rem);
            }
        }
        cols[0].horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut ide_state.new_watch_var_name)
                    .id_salt((salt, "watch_new_var"))
                    .desired_width(95.0)
                    .hint_text("var_name"),
            );
            ui.add(egui::DragValue::new(&mut ide_state.new_watch_var_val).speed(1));
            if ui.small_button("+ Var").clicked() {
                let k = ide_state.new_watch_var_name.trim().to_string();
                if !k.is_empty() {
                    world.trigger_vars.insert(k, ide_state.new_watch_var_val);
                    ide_state.new_watch_var_name.clear();
                }
            }
        });

        cols[0].separator();
        cols[0].strong("World Custom Flags (`world.world_custom_flags`)");
        let flag_keys: Vec<String> = world.world_custom_flags.keys().cloned().collect();
        if flag_keys.is_empty() {
            cols[0].weak("No world_custom_flags set yet.");
        } else {
            let mut flag_remove: Option<String> = None;
            for fk in flag_keys {
                cols[0].horizontal(|ui| {
                    if let Some(val) = world.world_custom_flags.get_mut(&fk) {
                        ui.checkbox(val, &fk);
                    }
                    if ui.small_button("✖").clicked() {
                        flag_remove = Some(fk);
                    }
                });
            }
            if let Some(rem) = flag_remove {
                world.world_custom_flags.remove(&rem);
            }
        }
        cols[0].horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut ide_state.new_watch_flag_name)
                    .id_salt((salt, "watch_new_flag"))
                    .desired_width(110.0)
                    .hint_text("flag_name"),
            );
            if ui.small_button("+ Flag").clicked() {
                let fk = ide_state.new_watch_flag_name.trim().to_string();
                if !fk.is_empty() {
                    world.world_custom_flags.insert(fk, true);
                    ide_state.new_watch_flag_name.clear();
                }
            }
        });

        // Right column: Mission Objectives & Countdown Timers
        cols[1].strong("Active Mission Objectives (`world.mission_objectives`)");
        if world.mission_objectives.is_empty() {
            cols[1].weak("No structured objectives active.");
        } else {
            for obj in &world.mission_objectives {
                let col = match obj.status {
                    rts_sdk::schema::ObjectiveStatus::Completed => {
                        egui::Color32::from_rgb(95, 230, 140)
                    }
                    rts_sdk::schema::ObjectiveStatus::Failed => {
                        egui::Color32::from_rgb(250, 95, 95)
                    }
                    rts_sdk::schema::ObjectiveStatus::Active => {
                        egui::Color32::from_rgb(245, 210, 95)
                    }
                    rts_sdk::schema::ObjectiveStatus::Pending => egui::Color32::GRAY,
                };
                let prog = if obj.def.target_count > 0 {
                    format!(" [{}/{}]", obj.current_count, obj.def.target_count)
                } else {
                    String::new()
                };
                cols[1].colored_label(
                    col,
                    format!(
                        "[{:?} / {:?}] {}: {}{}",
                        obj.def.tier, obj.status, obj.def.id, obj.def.title, prog
                    ),
                );
            }
        }

        cols[1].separator();
        cols[1].strong("Countdown Timers (`world.countdown_timers`)");
        if world.countdown_timers.is_empty() {
            cols[1].weak("No active countdown timers.");
        } else {
            let dt = world.dt;
            for timer in &world.countdown_timers {
                cols[1].colored_label(
                    egui::Color32::from_rgb(125, 215, 255),
                    format!(
                        "⏱ {} ('{}') — {} ({} / {} ticks)",
                        timer.label,
                        timer.timer_id,
                        timer.format_mm_ss(dt),
                        timer.remaining_ticks,
                        timer.total_ticks
                    ),
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rhai_and_lua_error_lines_and_highlights_tokens() {
        assert_eq!(
            extract_error_line_number("Syntax error: Unexpected token (line 14, position 8)"),
            Some(14)
        );
        assert_eq!(
            extract_error_line_number(
                "runtime error: [string \"mission.lua\"]:27: unexpected symbol"
            ),
            Some(27)
        );
        assert_eq!(
            extract_error_line_number("generic message without digits"),
            None
        );

        let src = "fn on_spawn(ctx) {\n    // comment\n    let hp = 250.0;\n    ctx.set_flag(\"ready\", true);\n}\n";
        assert_eq!(char_index_for_line(src, 1), 0);
        assert_eq!(
            &src[char_index_for_line(src, 3)..char_index_for_line(src, 3) + 8],
            "    let "
        );

        let job = build_script_highlight_layout_job(src, false, 600.0);
        assert_eq!(job.text, src);
        assert!(
            job.sections.len() >= 8,
            "Expected multiple syntax highlight sections, got {}",
            job.sections.len()
        );
    }
}
