use crate::studio::StudioState;
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::MapDef;
use rts_simulation::command::GameCommand;
use rts_simulation::world::SimWorld;

#[derive(Debug, Clone)]
pub struct DuelSimSummary {
    pub unit_a: String,
    pub count_a: u32,
    pub unit_b: String,
    pub count_b: u32,
    pub runs: u32,
    pub wins_a: u32,
    pub wins_b: u32,
    pub draws: u32,
    pub mean_ttk_secs: f32,
    pub mean_survivors_a: f32,
    pub mean_survivors_b: f32,
    pub cost_efficiency_a_vs_b: f32,
}

pub fn show_duel_simulator_header(ui: &mut egui::Ui, state: &mut StudioState, loaded: &LoadedMod) {
    egui::CollapsingHeader::new("⚔ Headless Deterministic Combat Duel Simulator")
        .default_open(false)
        .show(ui, |ui| {
            let unit_ids: Vec<String> = loaded.units.keys().cloned().collect();
            if state.duel_unit_a.is_empty() {
                if let Some(first) = unit_ids.first() {
                    state.duel_unit_a = first.clone();
                }
            }
            if state.duel_unit_b.is_empty() {
                if let Some(second) = unit_ids.get(1).or_else(|| unit_ids.first()) {
                    state.duel_unit_b = second.clone();
                }
            }

            ui.horizontal_wrapped(|ui| {
                ui.label("Force A:");
                ui.add(egui::DragValue::new(&mut state.duel_count_a).range(1..=32));
                egui::ComboBox::from_id_salt("duel_unit_a_cb")
                    .selected_text(&state.duel_unit_a)
                    .show_ui(ui, |ui| {
                        for uid in &unit_ids {
                            ui.selectable_value(&mut state.duel_unit_a, uid.clone(), uid);
                        }
                    });

                ui.strong("VS");

                ui.label("Force B:");
                ui.add(egui::DragValue::new(&mut state.duel_count_b).range(1..=32));
                egui::ComboBox::from_id_salt("duel_unit_b_cb")
                    .selected_text(&state.duel_unit_b)
                    .show_ui(ui, |ui| {
                        for uid in &unit_ids {
                            ui.selectable_value(&mut state.duel_unit_b, uid.clone(), uid);
                        }
                    });

                ui.label("Duels:");
                ui.add(egui::DragValue::new(&mut state.duel_runs).range(1..=25));

                if ui.button("▶ Run Headless Duels").clicked()
                    && !state.duel_unit_a.is_empty()
                    && !state.duel_unit_b.is_empty()
                {
                    let res = run_headless_combat_duel_sim(
                        loaded,
                        &state.duel_unit_a,
                        state.duel_count_a,
                        &state.duel_unit_b,
                        state.duel_count_b,
                        state.duel_runs,
                    );
                    state.status_banner = format!(
                        "Duel Sim: {}x {} vs {}x {} -> {:.0}% Win A | Mean TTK {:.1}s",
                        res.count_a,
                        res.unit_a,
                        res.count_b,
                        res.unit_b,
                        (res.wins_a as f32 / res.runs.max(1) as f32) * 100.0,
                        res.mean_ttk_secs
                    );
                    state.last_duel_result = Some(res);
                }
            });

            if let Some(res) = state.last_duel_result.as_ref() {
                let win_pct_a = (res.wins_a as f32 / res.runs.max(1) as f32) * 100.0;
                let win_pct_b = (res.wins_b as f32 / res.runs.max(1) as f32) * 100.0;
                ui.colored_label(
                    egui::Color32::from_rgb(130, 230, 175),
                    format!(
                        "Results ({} runs): Force A ('{}') Wins {:.0}% (avg {:.1} survivors) | Force B ('{}') Wins {:.0}% (avg {:.1} survivors) | Draws: {} | Mean TTK: {:.1}s | Cost-Eff A/B: {:.2}x",
                        res.runs,
                        res.unit_a,
                        win_pct_a,
                        res.mean_survivors_a,
                        res.unit_b,
                        win_pct_b,
                        res.mean_survivors_b,
                        res.draws,
                        res.mean_ttk_secs,
                        res.cost_efficiency_a_vs_b
                    ),
                );
            }
        });
}

/// Runs `runs` deterministic headless arena combat duels of `count_a` x `unit_a` vs `count_b` x `unit_b`.
pub fn run_headless_combat_duel_sim(
    loaded: &LoadedMod,
    unit_a: &str,
    count_a: u32,
    unit_b: &str,
    count_b: u32,
    runs: u32,
) -> DuelSimSummary {
    let arena_map = MapDef {
        id: "studio_duel_arena".to_string(),
        name: "Studio Duel Arena".to_string(),
        width: 40,
        height: 40,
        tile_size: 2.0,
        mission_objective: None,
        ore_regen_per_sec: 0.0,
        ore_patches: Vec::new(),
        gem_patches: Vec::new(),
        resource_patches: Vec::new(),
        obstacles: Vec::new(),
        water_rects: Vec::new(),
        bridge_rects: Vec::new(),
        elevation_rects: Vec::new(),
        ramp_rects: Vec::new(),
        heightmap: None,
        splatmap: None,
        civilian_structures: Vec::new(),
        preplaced_entities: Vec::new(),
        props: Vec::new(),
        trigger_zones: Vec::new(),
        spawns: Vec::new(),
        ..Default::default()
    };

    let runs = runs.clamp(1, 25);
    let count_a = count_a.clamp(1, 32);
    let count_b = count_b.clamp(1, 32);

    let mut wins_a = 0u32;
    let mut wins_b = 0u32;
    let mut draws = 0u32;
    let mut total_ticks = 0u64;
    let mut total_surv_a = 0usize;
    let mut total_surv_b = 0usize;

    for run_idx in 0..runs {
        let mut sim = SimWorld::from_mod_map(loaded, &arena_map)
            .with_seed(0xD0E1_2026_u64.wrapping_add(run_idx as u64 * 7919));

        let mut ids_a = Vec::new();
        let mut ids_b = Vec::new();
        for i in 0..count_a {
            let tz = 12 + (i as i32 % 12);
            let tx = 10 + (i as i32 / 12) * 2;
            if let Some(id) = sim.spawn_unit(1, unit_a, [tx, tz], loaded) {
                ids_a.push(id);
            }
        }
        for i in 0..count_b {
            let tz = 12 + (i as i32 % 12);
            let tx = 28 - (i as i32 / 12) * 2;
            if let Some(id) = sim.spawn_unit(2, unit_b, [tx, tz], loaded) {
                ids_b.push(id);
            }
        }

        let center_world = [40.0, 0.0, 36.0];
        sim.push_command(GameCommand::AttackMove {
            player_id: 1,
            entity_ids: ids_a,
            target_pos: center_world,
        });
        sim.push_command(GameCommand::AttackMove {
            player_id: 2,
            entity_ids: ids_b,
            target_pos: center_world,
        });

        for _ in 0..1200 {
            sim.step(loaded);
            let alive_a = sim
                .entities
                .values()
                .filter(|e| e.player_id == 1 && e.health > 0.0)
                .count();
            let alive_b = sim
                .entities
                .values()
                .filter(|e| e.player_id == 2 && e.health > 0.0)
                .count();
            if alive_a == 0 || alive_b == 0 {
                break;
            }
        }

        let surv_a = sim
            .entities
            .values()
            .filter(|e| e.player_id == 1 && e.health > 0.0)
            .count();
        let surv_b = sim
            .entities
            .values()
            .filter(|e| e.player_id == 2 && e.health > 0.0)
            .count();
        if surv_a > surv_b {
            wins_a += 1;
        } else if surv_b > surv_a {
            wins_b += 1;
        } else {
            draws += 1;
        }
        total_ticks += sim.tick;
        total_surv_a += surv_a;
        total_surv_b += surv_b;
    }

    let unit_cost_a = loaded
        .units
        .get(unit_a)
        .map(|u| u.cost)
        .unwrap_or(100)
        .max(1) as f32;
    let unit_cost_b = loaded
        .units
        .get(unit_b)
        .map(|u| u.cost)
        .unwrap_or(100)
        .max(1) as f32;
    let mean_surv_a = total_surv_a as f32 / runs as f32;
    let mean_surv_b = total_surv_b as f32 / runs as f32;
    let lost_cost_a = ((count_a as f32 - mean_surv_a).max(0.1)) * unit_cost_a;
    let killed_cost_b = ((count_b as f32 - mean_surv_b).max(0.0)) * unit_cost_b;

    DuelSimSummary {
        unit_a: unit_a.to_string(),
        count_a,
        unit_b: unit_b.to_string(),
        count_b,
        runs,
        wins_a,
        wins_b,
        draws,
        mean_ttk_secs: (total_ticks as f32 / runs as f32) / 20.0,
        mean_survivors_a: mean_surv_a,
        mean_survivors_b: mean_surv_b,
        cost_efficiency_a_vs_b: killed_cost_b / lost_cost_b_denom(lost_cost_a),
    }
}

#[inline]
fn lost_cost_b_denom(lost_cost_a: f32) -> f32 {
    lost_cost_a.max(0.1)
}
