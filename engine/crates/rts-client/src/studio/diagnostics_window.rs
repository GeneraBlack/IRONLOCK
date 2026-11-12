use super::dock_layout::{focus_or_open_tab, StudioTab};
use super::{SdkInspectTarget, StudioState};
use crate::input::ClientInteractionState;
use crate::rendering::StudioDebugOverlayConfig;
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_sdk::validator::{validate_mod, Severity};
use rts_simulation::ai::AiCommanderState;
use rts_simulation::world::SimWorld;

pub fn show_diagnostics_and_profiler_window(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &LoadedMod,
    world: &mut SimWorld,
    interaction: &mut ClientInteractionState,
    overlay_cfg: &mut StudioDebugOverlayConfig,
) {
    if state.validation_dirty || state.cached_validation_report.is_none() {
        state.cached_validation_report = Some(validate_mod(loaded, true));
        state.validation_dirty = false;
    }

    // Record latest SimTickProfile sample
    let prof = &world.last_tick_profile;
    let sample = [
        prof.total_us,
        prof.commands_us,
        prof.movement_us,
        prof.combat_us,
        prof.projectiles_us,
        prof.triggers_fow_us,
    ];
    if state.tick_profile_history_us.back().copied() != Some(sample) {
        if state.tick_profile_history_us.len() >= 120 {
            state.tick_profile_history_us.pop_front();
        }
        state.tick_profile_history_us.push_back(sample);
    }

    egui::ScrollArea::vertical()
        .id_salt("studio_diagnostics_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // 1. Live `validate_mod` Console with Click-to-Jump
            egui::CollapsingHeader::new(
                "1. Live Mod Schema Validator (Click Issue -> Jump to Inspector)",
            )
            .default_open(true)
            .show(ui, |ui| {
                show_validation_console(ui, state, loaded);
            });

            ui.separator();

            // 2. SimTickProfile Graph & Deterministic Single-Tick Stepper
            egui::CollapsingHeader::new(
                "2. Deterministic 20 Hz Sim Profiler & Single-Tick Stepper",
            )
            .default_open(true)
            .show(ui, |ui| {
                show_sim_profiler_section(ui, state, world, interaction);
            });

            ui.separator();

            // 3. GPU & 3D Debug Overlays (`StudioDebugOverlayConfig`)
            egui::CollapsingHeader::new("3. 3D Battlefield Debug Overlays & Heatmaps")
                .default_open(true)
                .show(ui, |ui| {
                    show_overlay_toggles_section(ui, interaction, overlay_cfg);
                });

            ui.separator();

            // 4. Live `AiCommanderState` Intelligence & Counter-Weight Inspector
            egui::CollapsingHeader::new(
                "4. AI Commander Intelligence & Counter-Production Weights",
            )
            .default_open(true)
            .show(ui, |ui| {
                show_ai_commander_inspector(ui, loaded, world, overlay_cfg);
            });

            ui.separator();

            // 5. Live Playtest Script & Trigger Variable Watch Table
            egui::CollapsingHeader::new(
                "5. Live Playtest Watch Table (`trigger_vars`, Flags, Objectives & Timers)",
            )
            .default_open(true)
            .show(ui, |ui| {
                super::script_ide_support::show_live_playtest_watch_table(
                    ui,
                    world,
                    &mut state.script_ide,
                    "diagnostics_watch",
                );
            });
        });
}

fn show_validation_console(ui: &mut egui::Ui, state: &mut StudioState, loaded: &LoadedMod) {
    ui.horizontal_wrapped(|ui| {
        if ui.small_button("🔄 Re-Validate Mod").clicked() {
            state.cached_validation_report = Some(validate_mod(loaded, true));
            state.validation_dirty = false;
        }
        ui.label("Filter:");
        ui.text_edit_singleline(&mut state.diag_filter);
        ui.checkbox(&mut state.diag_errors_only, "Errors Only");
    });

    let Some(report) = state.cached_validation_report.clone() else {
        return;
    };

    if report.issues.is_empty() {
        ui.colored_label(
            egui::Color32::from_rgb(90, 225, 135),
            format!(
                "✔ 0 Validation Issues across {} units, {} buildings, {} weapons, {} maps.",
                loaded.units.len(),
                loaded.buildings.len(),
                loaded.weapons.len(),
                loaded.maps.len()
            ),
        );
        return;
    }

    let q = state.diag_filter.trim().to_lowercase();
    for iss in &report.issues {
        if state.diag_errors_only && iss.severity != Severity::Error {
            continue;
        }
        if !q.is_empty()
            && !iss.code.to_lowercase().contains(&q)
            && !iss.message.to_lowercase().contains(&q)
        {
            continue;
        }
        let col = match iss.severity {
            Severity::Error => egui::Color32::from_rgb(245, 95, 95),
            Severity::Warning => egui::Color32::from_rgb(245, 195, 85),
        };
        let row_lbl = format!("[{:?} {}] {}", iss.severity, iss.code, iss.message);
        if ui
            .add(egui::Button::new(egui::RichText::new(row_lbl).color(col)).frame(false))
            .on_hover_text("Click to jump directly to this definition in Property Inspector")
            .clicked()
        {
            if let Some(target) = resolve_diagnostic_sdk_target(loaded, &iss.message) {
                state.inspect_target = target;
                focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
            }
        }
    }
}

/// Resolves any SDK schema definition ID mentioned in a `ValidationIssue` message.
pub fn resolve_diagnostic_sdk_target(
    loaded: &LoadedMod,
    message: &str,
) -> Option<SdkInspectTarget> {
    // Check quoted tokens first (`'grizzly_tank'`), then substring matches
    for part in message.split('\'').skip(1).step_by(2) {
        if loaded.units.contains_key(part) {
            return Some(SdkInspectTarget::Unit(part.to_string()));
        }
        if loaded.buildings.contains_key(part) {
            return Some(SdkInspectTarget::Building(part.to_string()));
        }
        if loaded.weapons.contains_key(part) {
            return Some(SdkInspectTarget::Weapon(part.to_string()));
        }
        if loaded.upgrades.contains_key(part) {
            return Some(SdkInspectTarget::Upgrade(part.to_string()));
        }
        if loaded.commander_trees.contains_key(part) {
            return Some(SdkInspectTarget::CommanderTree(part.to_string()));
        }
        if loaded.factions.contains_key(part) {
            return Some(SdkInspectTarget::Faction(part.to_string()));
        }
        if loaded.resources.contains_key(part) {
            return Some(SdkInspectTarget::Resource(part.to_string()));
        }
        if loaded.ai_profiles.contains_key(part) {
            return Some(SdkInspectTarget::AiProfile(part.to_string()));
        }
        if loaded.campaigns.contains_key(part) {
            return Some(SdkInspectTarget::Campaign(part.to_string()));
        }
    }
    for uid in loaded.units.keys() {
        if message.contains(uid.as_str()) {
            return Some(SdkInspectTarget::Unit(uid.clone()));
        }
    }
    for bid in loaded.buildings.keys() {
        if message.contains(bid.as_str()) {
            return Some(SdkInspectTarget::Building(bid.clone()));
        }
    }
    for wid in loaded.weapons.keys() {
        if message.contains(wid.as_str()) {
            return Some(SdkInspectTarget::Weapon(wid.clone()));
        }
    }
    for upid in loaded.upgrades.keys() {
        if message.contains(upid.as_str()) {
            return Some(SdkInspectTarget::Upgrade(upid.clone()));
        }
    }
    for ctid in loaded.commander_trees.keys() {
        if message.contains(ctid.as_str()) {
            return Some(SdkInspectTarget::CommanderTree(ctid.clone()));
        }
    }
    for fid in loaded.factions.keys() {
        if message.contains(fid.as_str()) {
            return Some(SdkInspectTarget::Faction(fid.clone()));
        }
    }
    for rid in loaded.resources.keys() {
        if message.contains(rid.as_str()) {
            return Some(SdkInspectTarget::Resource(rid.clone()));
        }
    }
    for apid in loaded.ai_profiles.keys() {
        if message.contains(apid.as_str()) {
            return Some(SdkInspectTarget::AiProfile(apid.clone()));
        }
    }
    for cid in loaded.campaigns.keys() {
        if message.contains(cid.as_str()) {
            return Some(SdkInspectTarget::Campaign(cid.clone()));
        }
    }
    if message.to_lowercase().contains("audio") {
        return Some(SdkInspectTarget::Audio);
    }
    None
}

fn show_sim_profiler_section(
    ui: &mut egui::Ui,
    state: &StudioState,
    world: &SimWorld,
    interaction: &mut ClientInteractionState,
) {
    let p = &world.last_tick_profile;
    ui.horizontal_wrapped(|ui| {
        let pause_lbl = if interaction.studio_playtest_active && !interaction.game_paused {
            "⏸ Pause Sim"
        } else {
            "▶ Resume Sim"
        };
        if ui.button(pause_lbl).clicked() {
            if !interaction.studio_playtest_active {
                interaction.studio_playtest_active = true;
                interaction.game_paused = false;
            } else {
                interaction.game_paused = !interaction.game_paused;
            }
        }
        if ui.button("⏭ Step +1 Tick").clicked() {
            interaction.game_paused = true;
            interaction.pending_single_steps += 1;
        }
        if ui.button("⏭ Step +10 Ticks").clicked() {
            interaction.game_paused = true;
            interaction.pending_single_steps += 10;
        }
        ui.label(format!(
            "Tick: {} | Hash: 0x{:08x} | Entities: {} | Projectiles: {} | Cache Hits: {}",
            world.tick,
            world.state_hash() as u32,
            p.entity_count,
            p.projectile_count,
            p.path_cache_hits
        ));
    });

    ui.label(format!(
        "Last Tick Breakdown: Total {} µs (Cmd {} µs | Move {} µs | Combat {} µs | Proj {} µs | Trig/FoW {} µs)",
        p.total_us, p.commands_us, p.movement_us, p.combat_us, p.projectiles_us, p.triggers_fow_us
    ));

    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(240.0), 72.0),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(12, 18, 26));

    let max_us = state
        .tick_profile_history_us
        .iter()
        .map(|s| s[0])
        .max()
        .unwrap_or(1000)
        .max(500) as f32;
    let count = state.tick_profile_history_us.len();
    if count > 1 {
        let step_x = rect.width() / (count.saturating_sub(1) as f32).max(1.0);
        let mut pts = Vec::with_capacity(count);
        for (i, sample) in state.tick_profile_history_us.iter().enumerate() {
            let x = rect.min.x + i as f32 * step_x;
            let norm = (sample[0] as f32 / max_us).clamp(0.0, 1.0);
            let y = rect.max.y - 4.0 - norm * (rect.height() - 8.0);
            pts.push(egui::pos2(x, y));
        }
        painter.add(egui::Shape::line(
            pts,
            egui::Stroke::new(1.8, egui::Color32::from_rgb(90, 220, 155)),
        ));
    }
}

fn show_overlay_toggles_section(
    ui: &mut egui::Ui,
    interaction: &mut ClientInteractionState,
    overlay_cfg: &mut StudioDebugOverlayConfig,
) {
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(
            &mut overlay_cfg.show_clearance_grid,
            "Clearance Classes (1x1..3x3)",
        );
        ui.checkbox(&mut overlay_cfg.show_flow_fields, "Flow-Field Vectors");
        ui.checkbox(&mut overlay_cfg.show_ballistic_arcs, "3D Ballistic Arcs");
        ui.checkbox(&mut overlay_cfg.show_sockets, "3D Weapon/VFX Sockets");
    });
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(
            &mut overlay_cfg.show_ai_threat_heatmap,
            "AI Threat Heatmap (`InfluenceMap`)",
        );
        ui.checkbox(
            &mut overlay_cfg.show_ai_econ_heatmap,
            "AI Economy Opportunity Heatmap",
        );
        ui.checkbox(
            &mut interaction.debug_gizmo_flags.show_paths,
            "A* Waypoints",
        );
        ui.checkbox(
            &mut interaction.debug_gizmo_flags.show_weapon_ranges,
            "Weapon Ranges",
        );
        ui.checkbox(
            &mut interaction.debug_gizmo_flags.show_trigger_zones,
            "Trigger Zone Bounds",
        );
    });
}

fn show_ai_commander_inspector(
    ui: &mut egui::Ui,
    loaded: &LoadedMod,
    world: &SimWorld,
    overlay_cfg: &mut StudioDebugOverlayConfig,
) {
    ui.horizontal(|ui| {
        ui.label("Inspected AI Slot:");
        for pid in 1u8..=8u8 {
            if world.players.contains_key(&pid) || pid <= 2 {
                ui.selectable_value(
                    &mut overlay_cfg.inspected_ai_player_id,
                    pid,
                    format!("Player {pid}"),
                );
            }
        }
    });

    let pid = overlay_cfg.inspected_ai_player_id.max(1);
    let live_or_computed: AiCommanderState = if let Some(cmd) = world.ai_commanders.get(&pid) {
        cmd.clone()
    } else {
        let mut fresh = AiCommanderState::new(pid, world.grid.width, world.grid.height);
        fresh.refresh_intelligence(world, loaded);
        fresh
    };

    ui.label(format!(
        "Scouted Enemy Ratios — Infantry: {:.0}% | Armor: {:.0}% | Air: {:.0}% | Defenses: {}",
        live_or_computed.enemy_infantry_ratio * 100.0,
        live_or_computed.enemy_armor_ratio * 100.0,
        live_or_computed.enemy_air_ratio * 100.0,
        live_or_computed.enemy_defense_count
    ));
    ui.label(format!(
        "Priority Counter Unit: {:?} | Saving For Building: {:?} | Planned Flank Waypoints: {}",
        live_or_computed.counter_unit_priority,
        live_or_computed.saving_for_building,
        live_or_computed.planned_attack_route.len()
    ));

    let faction = world
        .players
        .get(&pid)
        .map(|p| p.faction.clone())
        .unwrap_or_default();
    ui.strong("Dynamic Counter-Production Weights:");
    for u in loaded
        .units
        .values()
        .filter(|u| faction.is_empty() || u.faction == faction)
        .take(8)
    {
        let w_pct = live_or_computed.counter_weight_percent(u, loaded);
        ui.horizontal(|ui| {
            ui.label(format!("{:<18}", u.name));
            ui.add(
                egui::ProgressBar::new((w_pct as f32 / 250.0).clamp(0.0, 1.0))
                    .desired_width(140.0)
                    .text(format!("{w_pct}%")),
            );
        });
    }
}
