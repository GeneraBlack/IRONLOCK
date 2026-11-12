use super::StudioState;
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::SimWorld;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// One row in the **Inheritance & Override Diff Table** (`Base Value -> Active Value -> Source Layer`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InheritanceDiffRow {
    pub entity_kind: String,
    pub entity_id: String,
    pub field_path: String,
    pub base_value: String,
    pub active_value: String,
    pub source_layer: String,
}

/// Persistent UI state for the Mod Mutator & Delta-Overlay Wizard (`Task 246`).
#[derive(Debug, Clone)]
pub struct ModOverlayWizardState {
    pub overlay_id: String,
    pub overlay_name: String,
    pub overlay_version: String,
    pub base_mod_id: String,
    pub base_version_req: String,
    pub output_parent_dir: String,
    pub delta_only_mode: bool,
    pub include_sample_unit_delta: bool,
    pub selected_unit_id: String,
    pub delta_cost_override: u32,
    pub delta_append_tag: String,
    pub delta_remove_tag: String,
    pub diff_filter: String,
    pub last_created_status: Option<String>,
}

impl Default for ModOverlayWizardState {
    fn default() -> Self {
        Self {
            overlay_id: "balance-overhaul".to_string(),
            overlay_name: "Balance & Mutator Overlay".to_string(),
            overlay_version: "0.1.0".to_string(),
            base_mod_id: String::new(),
            base_version_req: ">=0.1.0".to_string(),
            output_parent_dir: String::new(),
            delta_only_mode: true,
            include_sample_unit_delta: false,
            selected_unit_id: String::new(),
            delta_cost_override: 650,
            delta_append_tag: "veteran_mutator".to_string(),
            delta_remove_tag: String::new(),
            diff_filter: String::new(),
            last_created_status: None,
        }
    }
}

/// Computes a minimal TOML delta patch table between `base_table` and `active_table`:
/// - Always preserves `"id"` when present.
/// - Emits `{ "$remove" = [...], "$append" = [...] }` for string-list fields (`tags`, `prerequisites`, etc.).
/// - Recurses into nested tables and emits ONLY modified keys.
pub fn compute_minimal_toml_delta(
    base_table: &toml::map::Map<String, toml::Value>,
    active_table: &toml::map::Map<String, toml::Value>,
) -> toml::map::Map<String, toml::Value> {
    let mut delta = toml::map::Map::new();
    if let Some(id_val) = active_table.get("id").or_else(|| base_table.get("id")) {
        delta.insert("id".to_string(), id_val.clone());
    }

    for (key, active_val) in active_table {
        if key == "id" {
            continue;
        }
        let Some(base_val) = base_table.get(key) else {
            if let toml::Value::Array(a_arr) = active_val {
                if !a_arr.is_empty() && a_arr.iter().all(|v| v.is_str()) {
                    let mut patch = toml::map::Map::new();
                    patch.insert("$append".to_string(), toml::Value::Array(a_arr.clone()));
                    delta.insert(key.clone(), toml::Value::Table(patch));
                    continue;
                }
            }
            delta.insert(key.clone(), active_val.clone());
            continue;
        };
        if base_val == active_val {
            continue;
        }

        match (base_val, active_val) {
            (toml::Value::Table(b_sub), toml::Value::Table(a_sub)) => {
                let sub_delta = compute_minimal_toml_delta_inner(b_sub, a_sub);
                if !sub_delta.is_empty() {
                    delta.insert(key.clone(), toml::Value::Table(sub_delta));
                }
            }
            (toml::Value::Array(b_arr), toml::Value::Array(a_arr))
                if (!b_arr.is_empty() || !a_arr.is_empty())
                    && b_arr.iter().all(|v| v.is_str())
                    && a_arr.iter().all(|v| v.is_str()) =>
            {
                let b_strs: Vec<String> = b_arr
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect();
                let a_strs: Vec<String> = a_arr
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect();
                let removed: Vec<toml::Value> = b_strs
                    .iter()
                    .filter(|s| !a_strs.contains(s))
                    .cloned()
                    .map(toml::Value::String)
                    .collect();
                let appended: Vec<toml::Value> = a_strs
                    .iter()
                    .filter(|s| !b_strs.contains(s))
                    .cloned()
                    .map(toml::Value::String)
                    .collect();
                if !removed.is_empty() || !appended.is_empty() {
                    let mut patch = toml::map::Map::new();
                    if !removed.is_empty() {
                        patch.insert("$remove".to_string(), toml::Value::Array(removed));
                    }
                    if !appended.is_empty() {
                        patch.insert("$append".to_string(), toml::Value::Array(appended));
                    }
                    delta.insert(key.clone(), toml::Value::Table(patch));
                }
            }
            _ => {
                delta.insert(key.clone(), active_val.clone());
            }
        }
    }
    for (key, base_val) in base_table {
        if key != "id" && !active_table.contains_key(key) {
            if let toml::Value::Array(b_arr) = base_val {
                if !b_arr.is_empty() && b_arr.iter().all(|v| v.is_str()) {
                    let mut patch = toml::map::Map::new();
                    patch.insert("$remove".to_string(), toml::Value::Array(b_arr.clone()));
                    delta.insert(key.clone(), toml::Value::Table(patch));
                }
            }
        }
    }
    delta
}

fn compute_minimal_toml_delta_inner(
    base_table: &toml::map::Map<String, toml::Value>,
    active_table: &toml::map::Map<String, toml::Value>,
) -> toml::map::Map<String, toml::Value> {
    let mut sub = compute_minimal_toml_delta(base_table, active_table);
    if !active_table.contains_key("id") {
        sub.remove("id");
    }
    sub
}

/// Scaffolds a thin overlay/mutator/addon mod directory with `depends_on` in `mod.toml`
/// and optional minimal delta-override `units/units.toml`.
#[allow(clippy::too_many_arguments)]
pub fn scaffold_thin_overlay_mod(
    parent_dir: &Path,
    overlay_id: &str,
    overlay_name: &str,
    overlay_version: &str,
    base_mod_id: &str,
    base_version_req: &str,
    unit_deltas: &[toml::map::Map<String, toml::Value>],
) -> anyhow::Result<PathBuf> {
    let clean_id = overlay_id.trim().to_lowercase().replace([' ', '_'], "-");
    anyhow::ensure!(!clean_id.is_empty(), "Overlay Mod ID must not be empty");
    anyhow::ensure!(
        !base_mod_id.trim().is_empty(),
        "Base Mod ID must not be empty"
    );

    let out_dir = parent_dir.join(&clean_id);
    for sub in ["units", "buildings", "weapons", "vfx"] {
        fs::create_dir_all(out_dir.join(sub))?;
    }

    let req = if base_version_req.trim().is_empty() {
        "*".to_string()
    } else {
        base_version_req.trim().to_string()
    };
    let manifest_toml = format!(
        "id = \"{clean_id}\"\nname = \"{}\"\nversion = \"{}\"\nengine_version = \"0.1.0\"\n\n[[depends_on]]\nid = \"{}\"\nversion_req = \"{req}\"\n",
        overlay_name.trim().replace('"', "'"),
        if overlay_version.trim().is_empty() {
            "0.1.0"
        } else {
            overlay_version.trim()
        },
        base_mod_id.trim(),
    );
    fs::write(out_dir.join("mod.toml"), manifest_toml)?;

    let non_empty_deltas: Vec<toml::Value> = unit_deltas
        .iter()
        .filter(|d| d.len() > 1)
        .cloned()
        .map(toml::Value::Table)
        .collect();
    if !non_empty_deltas.is_empty() {
        let mut root = toml::map::Map::new();
        root.insert("units".to_string(), toml::Value::Array(non_empty_deltas));
        let rendered = toml::to_string_pretty(&toml::Value::Table(root))?;
        fs::write(out_dir.join("units/units.toml"), rendered)?;
    }

    Ok(out_dir)
}

/// Collects `Base Value -> Active Value -> Source Layer` diff rows across stacked `ModVfs` layers
/// (and in-memory Workbench edits relative to the base layer on disk).
pub fn collect_inheritance_diff_rows(loaded: &LoadedMod) -> Vec<InheritanceDiffRow> {
    let mut rows = Vec::new();
    let active_layer_name = loaded.manifest.id.clone();

    let Some(base_layer) = loaded.vfs_layers.last() else {
        return rows;
    };
    let Ok(base_loaded) = LoadedMod::load_from_path(base_layer.root_path()) else {
        return rows;
    };
    let source_label = if loaded.vfs_layers.len() > 1 {
        format!("Overlay '{}'", active_layer_name)
    } else {
        format!("Unsaved Edit ({})", active_layer_name)
    };

    for (uid, active_u) in &loaded.units {
        if let Some(base_u) = base_loaded.units.get(uid) {
            if base_u != active_u {
                append_value_diff_rows(
                    "Unit",
                    uid,
                    "",
                    &toml::Value::try_from(base_u).unwrap_or(toml::Value::String(String::new())),
                    &toml::Value::try_from(active_u).unwrap_or(toml::Value::String(String::new())),
                    &source_label,
                    &mut rows,
                );
            }
        } else {
            rows.push(InheritanceDiffRow {
                entity_kind: "Unit".to_string(),
                entity_id: uid.clone(),
                field_path: "<added>".to_string(),
                base_value: "(absent)".to_string(),
                active_value: active_u.name.clone(),
                source_layer: source_label.clone(),
            });
        }
    }

    for (bid, active_b) in &loaded.buildings {
        if let Some(base_b) = base_loaded.buildings.get(bid) {
            if base_b != active_b {
                append_value_diff_rows(
                    "Building",
                    bid,
                    "",
                    &toml::Value::try_from(base_b).unwrap_or(toml::Value::String(String::new())),
                    &toml::Value::try_from(active_b).unwrap_or(toml::Value::String(String::new())),
                    &source_label,
                    &mut rows,
                );
            }
        }
    }

    for (wid, active_w) in &loaded.weapons {
        if let Some(base_w) = base_loaded.weapons.get(wid) {
            if base_w != active_w {
                append_value_diff_rows(
                    "Weapon",
                    wid,
                    "",
                    &toml::Value::try_from(base_w).unwrap_or(toml::Value::String(String::new())),
                    &toml::Value::try_from(active_w).unwrap_or(toml::Value::String(String::new())),
                    &source_label,
                    &mut rows,
                );
            }
        }
    }

    rows
}

fn append_value_diff_rows(
    entity_kind: &str,
    entity_id: &str,
    prefix: &str,
    base: &toml::Value,
    active: &toml::Value,
    source_layer: &str,
    out: &mut Vec<InheritanceDiffRow>,
) {
    if base == active {
        return;
    }
    if let (toml::Value::Table(bt), toml::Value::Table(at)) = (base, active) {
        let mut keys: BTreeMap<&str, ()> = BTreeMap::new();
        for k in bt.keys().chain(at.keys()) {
            keys.insert(k.as_str(), ());
        }
        for k in keys.keys() {
            if *k == "id" {
                continue;
            }
            let path = if prefix.is_empty() {
                (*k).to_string()
            } else {
                format!("{prefix}.{k}")
            };
            match (bt.get(*k), at.get(*k)) {
                (Some(bv), Some(av)) => {
                    append_value_diff_rows(entity_kind, entity_id, &path, bv, av, source_layer, out)
                }
                (None, Some(av)) => out.push(InheritanceDiffRow {
                    entity_kind: entity_kind.to_string(),
                    entity_id: entity_id.to_string(),
                    field_path: path,
                    base_value: "(none)".to_string(),
                    active_value: compact_toml_val(av),
                    source_layer: source_layer.to_string(),
                }),
                (Some(bv), None) => out.push(InheritanceDiffRow {
                    entity_kind: entity_kind.to_string(),
                    entity_id: entity_id.to_string(),
                    field_path: path,
                    base_value: compact_toml_val(bv),
                    active_value: "(removed)".to_string(),
                    source_layer: source_layer.to_string(),
                }),
                (None, None) => {}
            }
        }
        return;
    }

    out.push(InheritanceDiffRow {
        entity_kind: entity_kind.to_string(),
        entity_id: entity_id.to_string(),
        field_path: prefix.to_string(),
        base_value: compact_toml_val(base),
        active_value: compact_toml_val(active),
        source_layer: source_layer.to_string(),
    });
}

fn compact_toml_val(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => format!("\"{s}\""),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => format!("{f:.2}"),
        toml::Value::Boolean(b) => b.to_string(),
        _ => v.to_string(),
    }
}

/// Renders the `+ New Overlay / Balance / Addon Mod` Wizard & Inheritance Diff Table (`Task 246`).
pub fn show_mod_overlay_wizard_window(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) {
    if state.mod_overlay_wizard.base_mod_id.is_empty() {
        state.mod_overlay_wizard.base_mod_id = loaded
            .dependency_chain
            .first()
            .cloned()
            .unwrap_or_else(|| loaded.manifest.id.clone());
    }
    if state.mod_overlay_wizard.output_parent_dir.is_empty() {
        let default_parent = loaded
            .root_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| loaded.root_dir.clone());
        state.mod_overlay_wizard.output_parent_dir = default_parent.display().to_string();
    }
    if state.mod_overlay_wizard.selected_unit_id.is_empty() {
        if let Some((uid, u)) = loaded.units.iter().next() {
            state.mod_overlay_wizard.selected_unit_id = uid.clone();
            state.mod_overlay_wizard.delta_cost_override = u.cost;
        }
    }

    egui::ScrollArea::vertical()
        .id_salt("studio_mod_overlay_wizard_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Active DAG Stack Banner
            ui.horizontal_wrapped(|ui| {
                ui.strong("Active Mod DAG Stack:");
                let chain = if loaded.dependency_chain.is_empty() {
                    loaded.manifest.id.clone()
                } else {
                    loaded.dependency_chain.join(" ➡ ")
                };
                ui.colored_label(egui::Color32::from_rgb(130, 220, 255), chain);
                ui.weak(format!(
                    "| VFS Layers: {} | Stack Hash: {:016x}",
                    loaded.vfs_layers.len(),
                    loaded.stack_content_hash
                ));
            });

            ui.separator();

            // 1. + New Overlay / Balance / Addon Mod Wizard
            egui::CollapsingHeader::new("1. 🧩 + New Overlay / Balance / Addon Mod Wizard")
                .default_open(true)
                .show(ui, |ui| {
                    show_scaffold_overlay_form(ui, state, loaded, world);
                });

            ui.separator();

            // 2. Inheritance & Override Diff Table
            egui::CollapsingHeader::new(
                "2. 📊 Inheritance & Override Diff Table (Base Value ➡ Active Value ➡ Source Layer)",
            )
            .default_open(true)
            .show(ui, |ui| {
                show_inheritance_diff_table(ui, state, loaded);
            });
        });
}

fn show_scaffold_overlay_form(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) {
    let wiz = &mut state.mod_overlay_wizard;
    egui::Grid::new("mod_overlay_form_grid")
        .num_columns(2)
        .spacing([10.0, 5.0])
        .show(ui, |ui| {
            ui.label("Overlay Mod ID:");
            ui.text_edit_singleline(&mut wiz.overlay_id);
            ui.end_row();

            ui.label("Display Name:");
            ui.text_edit_singleline(&mut wiz.overlay_name);
            ui.end_row();

            ui.label("Overlay Version:");
            ui.text_edit_singleline(&mut wiz.overlay_version);
            ui.end_row();

            ui.label("Depends On Base Mod (`depends_on.id`):");
            ui.text_edit_singleline(&mut wiz.base_mod_id);
            ui.end_row();

            ui.label("SemVer Constraint (`version_req`):");
            ui.text_edit_singleline(&mut wiz.base_version_req);
            ui.end_row();

            ui.label("Output Parent Directory:");
            ui.text_edit_singleline(&mut wiz.output_parent_dir);
            ui.end_row();
        });

    ui.checkbox(
        &mut wiz.delta_only_mode,
        "Save Minimal Delta-Override Only (`id` + modified fields / `$append` / `$remove`)",
    );
    ui.checkbox(
        &mut wiz.include_sample_unit_delta,
        "Include Quick Unit Balance / `$append` Tag Override in `units/units.toml`",
    );

    if wiz.include_sample_unit_delta {
        ui.horizontal_wrapped(|ui| {
            ui.label("Unit:");
            egui::ComboBox::from_id_salt("overlay_wiz_unit_pick")
                .selected_text(&wiz.selected_unit_id)
                .width(150.0)
                .show_ui(ui, |ui| {
                    for (uid, u) in &loaded.units {
                        if ui
                            .selectable_value(&mut wiz.selected_unit_id, uid.clone(), uid)
                            .clicked()
                        {
                            wiz.delta_cost_override = u.cost;
                        }
                    }
                });
            ui.label("New Cost:");
            ui.add(egui::DragValue::new(&mut wiz.delta_cost_override).speed(10.0));
            ui.label("`$append` Tag:");
            ui.add(egui::TextEdit::singleline(&mut wiz.delta_append_tag).desired_width(115.0));
            ui.label("`$remove` Tag:");
            ui.add(egui::TextEdit::singleline(&mut wiz.delta_remove_tag).desired_width(115.0));
        });
    }

    ui.horizontal_wrapped(|ui| {
        let create_only = ui.button("📁 Scaffold Thin Overlay Mod").clicked();
        let create_and_mount = ui
            .button(
                egui::RichText::new("🚀 Scaffold & Hot-Mount Overlay Stack")
                    .color(egui::Color32::from_rgb(115, 240, 185)),
            )
            .clicked();

        if create_only || create_and_mount {
            let deltas = build_requested_unit_deltas(state, loaded);
            let parent = PathBuf::from(state.mod_overlay_wizard.output_parent_dir.trim());
            match scaffold_thin_overlay_mod(
                &parent,
                &state.mod_overlay_wizard.overlay_id,
                &state.mod_overlay_wizard.overlay_name,
                &state.mod_overlay_wizard.overlay_version,
                &state.mod_overlay_wizard.base_mod_id,
                &state.mod_overlay_wizard.base_version_req,
                &deltas,
            ) {
                Ok(out_dir) => {
                    let mut msg = format!("✔ Created thin overlay mod -> {}", out_dir.display());
                    if create_and_mount {
                        match LoadedMod::load_from_dir(&out_dir) {
                            Ok(stacked) => {
                                *loaded = stacked;
                                let _ = world.apply_mod_hot_reload(loaded);
                                state.validation_dirty = true;
                                msg.push_str(" (Mounted live onto ModVfs DAG!)");
                            }
                            Err(err) => {
                                msg.push_str(&format!(" (Created, mount note: {err})"));
                            }
                        }
                    }
                    state.status_banner = msg.clone();
                    state.mod_overlay_wizard.last_created_status = Some(msg);
                }
                Err(err) => {
                    let msg = format!("✖ Overlay scaffold failed: {err}");
                    state.status_banner = msg.clone();
                    state.mod_overlay_wizard.last_created_status = Some(msg);
                }
            }
        }
    });

    if let Some(ref st) = state.mod_overlay_wizard.last_created_status {
        ui.colored_label(egui::Color32::from_rgb(135, 230, 175), st);
    }
}

fn build_requested_unit_deltas(
    state: &StudioState,
    loaded: &LoadedMod,
) -> Vec<toml::map::Map<String, toml::Value>> {
    let mut out = Vec::new();
    let wiz = &state.mod_overlay_wizard;
    if wiz.include_sample_unit_delta {
        if let Some(base_u) = loaded.units.get(&wiz.selected_unit_id) {
            let mut modified = base_u.clone();
            modified.cost = wiz.delta_cost_override;
            let rem = wiz.delta_remove_tag.trim();
            if !rem.is_empty() {
                modified.tags.retain(|t| t != rem);
            }
            let app = wiz.delta_append_tag.trim();
            if !app.is_empty() && !modified.tags.iter().any(|t| t == app) {
                modified.tags.push(app.to_string());
            }
            if let (Ok(toml::Value::Table(bt)), Ok(toml::Value::Table(at))) = (
                toml::Value::try_from(base_u),
                toml::Value::try_from(&modified),
            ) {
                let d = if wiz.delta_only_mode {
                    compute_minimal_toml_delta(&bt, &at)
                } else {
                    at
                };
                out.push(d);
            }
        }
    }
    out
}

fn show_inheritance_diff_table(ui: &mut egui::Ui, state: &mut StudioState, loaded: &LoadedMod) {
    ui.horizontal(|ui| {
        ui.label("Filter Entity / Field:");
        ui.text_edit_singleline(&mut state.mod_overlay_wizard.diff_filter);
    });

    let rows = collect_inheritance_diff_rows(loaded);
    let q = state.mod_overlay_wizard.diff_filter.trim().to_lowercase();
    let filtered: Vec<&InheritanceDiffRow> = rows
        .iter()
        .filter(|r| {
            q.is_empty()
                || r.entity_id.to_lowercase().contains(&q)
                || r.field_path.to_lowercase().contains(&q)
                || r.source_layer.to_lowercase().contains(&q)
        })
        .collect();

    if filtered.is_empty() {
        ui.weak(
            "No overridden fields detected between the Base Layer and Active Mod state (0 diffs).",
        );
        return;
    }

    ui.label(format!("Showing {} override diff(s):", filtered.len()));
    egui::Grid::new("mod_inheritance_diff_grid")
        .num_columns(4)
        .striped(true)
        .spacing([14.0, 4.0])
        .show(ui, |ui| {
            ui.strong("Entity :: Field");
            ui.strong("Base Value");
            ui.strong("Active Value");
            ui.strong("Source Layer");
            ui.end_row();

            for row in filtered.into_iter().take(120) {
                ui.monospace(format!(
                    "[{}] {}.{}",
                    row.entity_kind, row.entity_id, row.field_path
                ));
                ui.colored_label(egui::Color32::from_rgb(235, 155, 130), &row.base_value);
                ui.colored_label(egui::Color32::from_rgb(125, 240, 175), &row.active_value);
                ui.colored_label(egui::Color32::from_rgb(145, 205, 255), &row.source_layer);
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_thin_overlay_and_compute_minimal_delta_roundtrips_dag_merge() {
        let base_mod = LoadedMod::load_from_dir("rust-ra2").expect("load rust-ra2");
        let (first_uid, base_unit) = base_mod.units.iter().next().expect("at least one unit");
        let mut active_unit = base_unit.clone();
        active_unit.cost = base_unit.cost + 175;
        let removed_tag = active_unit.tags.first().cloned();
        if let Some(ref rt) = removed_tag {
            active_unit.tags.retain(|t| t != rt);
        }
        active_unit.tags.push("mutator_reactive_armor".to_string());

        let bt = match toml::Value::try_from(base_unit).expect("serialize base unit") {
            toml::Value::Table(t) => t,
            _ => panic!("expected table"),
        };
        let at = match toml::Value::try_from(&active_unit).expect("serialize active unit") {
            toml::Value::Table(t) => t,
            _ => panic!("expected table"),
        };

        let delta = compute_minimal_toml_delta(&bt, &at);
        assert_eq!(
            delta.get("id").and_then(|v| v.as_str()),
            Some(first_uid.as_str())
        );
        assert_eq!(
            delta.get("cost").and_then(|v| v.as_integer()),
            Some(i64::from(active_unit.cost))
        );
        assert!(
            !delta.contains_key("max_hp"),
            "unchanged scalar fields must be omitted from minimal delta"
        );
        let tags_patch = delta
            .get("tags")
            .and_then(|v| v.as_table())
            .expect("tags delta uses $append/$remove patch table");
        assert!(tags_patch.contains_key("$append"));
        if removed_tag.is_some() {
            assert!(tags_patch.contains_key("$remove"));
        }

        // Scaffold thin overlay mod next to a temp copy or pointing to rust-ra2 via load_mod_with_overlays
        let temp_dir = std::env::temp_dir().join(format!(
            "rust_rts_overlay_wizard_test_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let overlay_dir = scaffold_thin_overlay_mod(
            &temp_dir,
            "ra2-balance-mutator",
            "RA2 Balance Mutator",
            "0.2.0",
            &base_mod.manifest.id,
            ">=0.1.0",
            &[delta],
        )
        .expect("scaffold thin overlay");

        let stacked = LoadedMod::load_mod_with_overlays(&base_mod.root_dir, &[&overlay_dir])
            .expect("stack base + thin overlay");
        assert_eq!(stacked.units[first_uid].cost, base_unit.cost + 175);
        assert!(stacked.units[first_uid]
            .tags
            .contains(&"mutator_reactive_armor".to_string()));

        let diffs = collect_inheritance_diff_rows(&stacked);
        assert!(
            diffs
                .iter()
                .any(|r| r.entity_id == *first_uid && r.field_path == "cost"),
            "inheritance diff table must surface overridden unit cost"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
