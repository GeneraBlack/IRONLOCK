use super::history::EditorCommand;
use super::StudioState;
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::LocaleCatalog;
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct LocaleEditorState {
    pub primary_lang: String,
    pub secondary_lang: String,
    pub search_filter: String,
    pub missing_only: bool,
    pub new_key: String,
    pub new_primary_val: String,
    pub new_secondary_val: String,
    pub new_lang_code: String,
}

impl Default for LocaleEditorState {
    fn default() -> Self {
        Self {
            primary_lang: "en".to_string(),
            secondary_lang: "de".to_string(),
            search_filter: String::new(),
            missing_only: false,
            new_key: String::new(),
            new_primary_val: String::new(),
            new_secondary_val: String::new(),
            new_lang_code: String::new(),
        }
    }
}

pub fn ensure_locale_pair_initialized(loaded: &mut LoadedMod, primary: &str, secondary: &str) {
    for lang in [primary, secondary] {
        let code = lang.trim();
        if !code.is_empty() && !loaded.locales.contains_key(code) {
            loaded.locales.insert(
                code.to_string(),
                LocaleCatalog {
                    language: code.to_string(),
                    strings: Default::default(),
                },
            );
        }
    }
}

/// Collects the sorted union of translation keys across `primary` and `secondary`,
/// optionally filtered by `search_query` and `missing_only`.
pub fn collect_filtered_locale_keys(
    loaded: &LoadedMod,
    primary: &str,
    secondary: &str,
    search_query: &str,
    missing_only: bool,
) -> Vec<String> {
    let mut all_keys = BTreeSet::new();
    if let Some(cat) = loaded.locales.get(primary) {
        all_keys.extend(cat.strings.keys().cloned());
    }
    if let Some(cat) = loaded.locales.get(secondary) {
        all_keys.extend(cat.strings.keys().cloned());
    }

    let q = search_query.trim().to_lowercase();
    all_keys
        .into_iter()
        .filter(|k| {
            let v1 = loaded
                .locales
                .get(primary)
                .and_then(|c| c.strings.get(k))
                .map(String::as_str)
                .unwrap_or("");
            let v2 = loaded
                .locales
                .get(secondary)
                .and_then(|c| c.strings.get(k))
                .map(String::as_str)
                .unwrap_or("");
            if missing_only && !v1.trim().is_empty() && !v2.trim().is_empty() {
                return false;
            }
            if !q.is_empty()
                && !k.to_lowercase().contains(&q)
                && !v1.to_lowercase().contains(&q)
                && !v2.to_lowercase().contains(&q)
            {
                return false;
            }
            true
        })
        .collect()
}

pub fn upsert_locale_key_value(
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    lang: &str,
    key: &str,
    value: String,
) {
    let before = loaded
        .locales
        .get(lang)
        .cloned()
        .unwrap_or_else(|| LocaleCatalog {
            language: lang.to_string(),
            strings: Default::default(),
        });
    if before.strings.get(key) == Some(&value) {
        return;
    }
    let mut after = before.clone();
    after.language = lang.to_string();
    after.strings.insert(key.to_string(), value);
    loaded.locales.insert(lang.to_string(), after.clone());
    state.modified_locales.insert(lang.to_string());
    state.validation_dirty = true;
    state
        .history
        .push_executed(EditorCommand::EditLocaleCatalog {
            lang: lang.to_string(),
            before: Box::new(before),
            after: Box::new(after),
        });
}

pub fn remove_locale_key_both(
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    primary: &str,
    secondary: &str,
    key: &str,
) {
    for lang in [primary, secondary] {
        let Some(before) = loaded.locales.get(lang).cloned() else {
            continue;
        };
        if !before.strings.contains_key(key) {
            continue;
        }
        let mut after = before.clone();
        after.strings.remove(key);
        loaded.locales.insert(lang.to_string(), after.clone());
        state.modified_locales.insert(lang.to_string());
        state
            .history
            .push_executed(EditorCommand::EditLocaleCatalog {
                lang: lang.to_string(),
                before: Box::new(before),
                after: Box::new(after),
            });
    }
    state.validation_dirty = true;
}

pub fn save_active_locales_to_disk(state: &mut StudioState, loaded: &mut LoadedMod) {
    let mut langs: BTreeSet<String> = state.modified_locales.clone();
    langs.insert(state.locale_editor.primary_lang.clone());
    langs.insert(state.locale_editor.secondary_lang.clone());

    let mut saved_paths = Vec::new();
    let mut errors = Vec::new();
    for lang in langs {
        if lang.trim().is_empty() {
            continue;
        }
        let Some(cat) = loaded.locales.get(&lang).cloned() else {
            continue;
        };
        match loaded.save_locale_catalog(cat) {
            Ok(p) => {
                state.modified_locales.remove(&lang);
                saved_paths.push(format!("{lang}.toml ({})", p.display()));
            }
            Err(err) => {
                errors.push(format!("{lang}: {err}"));
            }
        }
    }

    if errors.is_empty() {
        state.status_banner = format!("💾 Saved locales: {}", saved_paths.join(", "));
    } else {
        state.status_banner = format!("⚠ Locale save errors: {}", errors.join(" | "));
    }
}

pub fn show_locale_editor_window(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
) {
    let primary = state.locale_editor.primary_lang.clone();
    let secondary = state.locale_editor.secondary_lang.clone();
    ensure_locale_pair_initialized(loaded, &primary, &secondary);

    let available_langs: Vec<String> = loaded.locales.keys().cloned().collect();

    ui.horizontal_wrapped(|ui| {
        ui.strong("🌐 Bilingual Locale Catalog Editor");
        ui.separator();

        ui.label("Left:");
        egui::ComboBox::from_id_salt("locale_primary_cb")
            .selected_text(format!("{}.toml", state.locale_editor.primary_lang))
            .width(90.0)
            .show_ui(ui, |ui| {
                for l in &available_langs {
                    ui.selectable_value(
                        &mut state.locale_editor.primary_lang,
                        l.clone(),
                        format!("{l}.toml"),
                    );
                }
            });

        ui.label("<-> Right:");
        egui::ComboBox::from_id_salt("locale_secondary_cb")
            .selected_text(format!("{}.toml", state.locale_editor.secondary_lang))
            .width(90.0)
            .show_ui(ui, |ui| {
                for l in &available_langs {
                    ui.selectable_value(
                        &mut state.locale_editor.secondary_lang,
                        l.clone(),
                        format!("{l}.toml"),
                    );
                }
            });

        ui.separator();
        ui.label("Filter:");
        ui.add(
            egui::TextEdit::singleline(&mut state.locale_editor.search_filter)
                .desired_width(150.0)
                .hint_text("Search key or text..."),
        );
        if !state.locale_editor.search_filter.is_empty() && ui.small_button("✖").clicked() {
            state.locale_editor.search_filter.clear();
        }

        ui.checkbox(
            &mut state.locale_editor.missing_only,
            "⚠ Missing Translations Only",
        );

        ui.separator();
        let dirty_tag = if state.modified_locales.is_empty() {
            "💾 Save Locales"
        } else {
            "💾 Save Locales *"
        };
        if ui
            .button(egui::RichText::new(dirty_tag).color(egui::Color32::from_rgb(115, 235, 160)))
            .clicked()
        {
            save_active_locales_to_disk(state, loaded);
        }
    });

    // Add New Key / New Language Bar
    ui.horizontal_wrapped(|ui| {
        ui.label("+ Key:");
        ui.add(
            egui::TextEdit::singleline(&mut state.locale_editor.new_key)
                .desired_width(145.0)
                .hint_text("unit.hover_tank.name"),
        );
        ui.label(format!("{}:", state.locale_editor.primary_lang));
        ui.add(
            egui::TextEdit::singleline(&mut state.locale_editor.new_primary_val)
                .desired_width(155.0)
                .hint_text("English text"),
        );
        ui.label(format!("{}:", state.locale_editor.secondary_lang));
        ui.add(
            egui::TextEdit::singleline(&mut state.locale_editor.new_secondary_val)
                .desired_width(155.0)
                .hint_text("Deutscher Text"),
        );
        if ui.button("➕ Add Key").clicked() {
            let clean_key = state.locale_editor.new_key.trim().to_string();
            if !clean_key.is_empty() {
                let p_lang = state.locale_editor.primary_lang.clone();
                let s_lang = state.locale_editor.secondary_lang.clone();
                let p_val = state.locale_editor.new_primary_val.clone();
                let s_val = state.locale_editor.new_secondary_val.clone();
                upsert_locale_key_value(state, loaded, &p_lang, &clean_key, p_val);
                if p_lang != s_lang {
                    upsert_locale_key_value(state, loaded, &s_lang, &clean_key, s_val);
                }
                state.locale_editor.new_key.clear();
                state.locale_editor.new_primary_val.clear();
                state.locale_editor.new_secondary_val.clear();
                state.status_banner = format!("Added locale key '{clean_key}'.");
            }
        }

        ui.separator();
        ui.add(
            egui::TextEdit::singleline(&mut state.locale_editor.new_lang_code)
                .desired_width(50.0)
                .hint_text("fr"),
        );
        if ui.small_button("+ Lang").clicked() {
            let code: String = state
                .locale_editor
                .new_lang_code
                .trim()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            if !code.is_empty() {
                loaded
                    .locales
                    .entry(code.clone())
                    .or_insert_with(|| LocaleCatalog {
                        language: code.clone(),
                        strings: Default::default(),
                    });
                state.locale_editor.secondary_lang = code.clone();
                state.modified_locales.insert(code);
                state.locale_editor.new_lang_code.clear();
            }
        }
    });

    let p_lang = state.locale_editor.primary_lang.clone();
    let s_lang = state.locale_editor.secondary_lang.clone();
    let keys = collect_filtered_locale_keys(
        loaded,
        &p_lang,
        &s_lang,
        &state.locale_editor.search_filter,
        state.locale_editor.missing_only,
    );

    let total_missing = collect_filtered_locale_keys(loaded, &p_lang, &s_lang, "", true).len();
    ui.weak(format!(
        "Showing {} key(s) | Missing in {}.toml or {}.toml: {}",
        keys.len(),
        p_lang,
        s_lang,
        total_missing
    ));
    ui.separator();

    let mut pending_edits: Vec<(String, String, String)> = Vec::new();
    let mut pending_delete_key: Option<String> = None;

    egui::ScrollArea::vertical()
        .id_salt("studio_locale_editor_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("studio_locale_bilingual_grid")
                .striped(true)
                .num_columns(4)
                .min_col_width(120.0)
                .show(ui, |ui| {
                    ui.strong("String Key");
                    ui.strong(format!(
                        "{} (locales/{}.toml)",
                        p_lang.to_uppercase(),
                        p_lang
                    ));
                    ui.strong(format!(
                        "{} (locales/{}.toml)",
                        s_lang.to_uppercase(),
                        s_lang
                    ));
                    ui.strong("Del");
                    ui.end_row();

                    for key in &keys {
                        let cur_p = loaded
                            .locales
                            .get(&p_lang)
                            .and_then(|c| c.strings.get(key))
                            .cloned()
                            .unwrap_or_default();
                        let cur_s = loaded
                            .locales
                            .get(&s_lang)
                            .and_then(|c| c.strings.get(key))
                            .cloned()
                            .unwrap_or_default();

                        let is_missing = cur_p.trim().is_empty() || cur_s.trim().is_empty();
                        if is_missing {
                            ui.colored_label(
                                egui::Color32::from_rgb(250, 185, 75),
                                format!("⚠ {key}"),
                            );
                        } else {
                            ui.monospace(key);
                        }

                        let mut edit_p = cur_p.clone();
                        let resp_p = ui.add(
                            egui::TextEdit::singleline(&mut edit_p)
                                .id_salt(("loc_p", key.as_str()))
                                .desired_width(260.0),
                        );
                        if resp_p.changed() {
                            pending_edits.push((p_lang.clone(), key.clone(), edit_p));
                        }

                        let mut edit_s = cur_s.clone();
                        let resp_s = ui.add(
                            egui::TextEdit::singleline(&mut edit_s)
                                .id_salt(("loc_s", key.as_str()))
                                .desired_width(260.0),
                        );
                        if resp_s.changed() && p_lang != s_lang {
                            pending_edits.push((s_lang.clone(), key.clone(), edit_s));
                        }

                        if ui
                            .small_button("🗑")
                            .on_hover_text("Delete key from both locales")
                            .clicked()
                        {
                            pending_delete_key = Some(key.clone());
                        }
                        ui.end_row();
                    }
                });
        });

    for (lang, key, val) in pending_edits {
        upsert_locale_key_value(state, loaded, &lang, &key, val);
    }
    if let Some(del_k) = pending_delete_key {
        remove_locale_key_both(state, loaded, &p_lang, &s_lang, &del_k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_asset_pipeline::scaffold::scaffold_new_mod_project;
    use rts_simulation::world::SimWorld;

    #[test]
    fn bilingual_locale_editor_filters_missing_undo_redo_and_saves_to_disk() {
        let tmp =
            std::env::temp_dir().join(format!("rts_studio_locale_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        scaffold_new_mod_project(&tmp, "locale-test", "Locale Test", None).expect("scaffold");

        let mut loaded = LoadedMod::load_from_dir(&tmp).expect("load mod");
        let mut world = SimWorld::from_mod(&loaded);
        let mut state = StudioState::default();

        ensure_locale_pair_initialized(&mut loaded, "en", "de");
        upsert_locale_key_value(
            &mut state,
            &mut loaded,
            "en",
            "mission.obj.alpha",
            "Capture Radar Outpost".to_string(),
        );
        upsert_locale_key_value(
            &mut state,
            &mut loaded,
            "de",
            "mission.obj.alpha",
            String::new(),
        );

        let missing = collect_filtered_locale_keys(&loaded, "en", "de", "", true);
        assert!(missing.contains(&"mission.obj.alpha".to_string()));

        upsert_locale_key_value(
            &mut state,
            &mut loaded,
            "de",
            "mission.obj.alpha",
            "Radar-Aussenposten erobern".to_string(),
        );
        let missing_after = collect_filtered_locale_keys(&loaded, "en", "de", "", true);
        assert!(!missing_after.contains(&"mission.obj.alpha".to_string()));

        // Undo & Redo
        let _ = state.history.undo(&mut world, &mut loaded);
        assert_eq!(loaded.locales["de"].strings["mission.obj.alpha"], "");
        let _ = state.history.redo(&mut world, &mut loaded);
        assert_eq!(
            loaded.locales["de"].strings["mission.obj.alpha"],
            "Radar-Aussenposten erobern"
        );

        save_active_locales_to_disk(&mut state, &mut loaded);
        let reloaded = LoadedMod::load_from_dir(&tmp).expect("reload");
        assert_eq!(
            reloaded.locales["en"]
                .strings
                .get("mission.obj.alpha")
                .map(String::as_str),
            Some("Capture Radar Outpost")
        );
        assert_eq!(
            reloaded.locales["de"]
                .strings
                .get("mission.obj.alpha")
                .map(String::as_str),
            Some("Radar-Aussenposten erobern")
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
