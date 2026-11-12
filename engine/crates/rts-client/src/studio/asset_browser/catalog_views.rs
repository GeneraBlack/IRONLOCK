use super::{collect_mod_video_paths, AssetBrowserCategory, PendingSchemaDelete, SchemaKind};
use crate::audio::SoundSettingsResource;
use crate::input::ClientInteractionState;
use crate::rendering::{MapTerrainDirty, VideoPlaybackResource};
use crate::studio::dock_layout::{focus_or_open_tab, StudioTab};
use crate::studio::{SdkInspectTarget, StudioState};
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::PropKind;
use rts_simulation::world::SimWorld;

#[allow(clippy::too_many_arguments)]
pub(super) fn show_browser_category_entries(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    interaction: &mut ClientInteractionState,
    sound_res: &mut SoundSettingsResource,
    video_res: Option<&mut VideoPlaybackResource>,
    terrain_dirty: &mut MapTerrainDirty,
) {
    let query = state.browser_search.trim().to_lowercase();
    egui::ScrollArea::vertical()
        .id_salt("studio_asset_browser_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| match state.browser_category {
            AssetBrowserCategory::Units => {
                let ids: Vec<String> = loaded.units.keys().cloned().collect();
                for uid in ids {
                    let Some(u) = loaded.units.get(&uid) else {
                        continue;
                    };
                    if !query.is_empty()
                        && !uid.to_lowercase().contains(&query)
                        && !u.name.to_lowercase().contains(&query)
                        && !u.faction.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    let is_sel = state.inspect_target == SdkInspectTarget::Unit(uid.clone());
                    let dirty_mark = if state.modified_units.contains(&uid) {
                        " *"
                    } else {
                        ""
                    };
                    ui.horizontal(|ui| {
                        draw_cameo_or_badge(ui, state, &uid, u.visual.primary_color);
                        let label = format!(
                            "{}{} [{}] — ${} | {:.0} HP",
                            u.name, dirty_mark, u.faction, u.cost, u.health
                        );
                        if ui.selectable_label(is_sel, label).clicked() {
                            state.inspect_target = SdkInspectTarget::Unit(uid.clone());
                            state.selected_unit_brush_id = uid.clone();
                            focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                        }
                        if ui.small_button("🗑").on_hover_text("Delete Unit").clicked() {
                            state.pending_delete_confirm = Some(PendingSchemaDelete {
                                kind: SchemaKind::Unit,
                                references: loaded.find_id_references(&uid),
                                id: uid.clone(),
                            });
                        }
                    });
                }
            }
            AssetBrowserCategory::Buildings => {
                let ids: Vec<String> = loaded.buildings.keys().cloned().collect();
                for bid in ids {
                    let Some(b) = loaded.buildings.get(&bid) else {
                        continue;
                    };
                    if !query.is_empty()
                        && !bid.to_lowercase().contains(&query)
                        && !b.name.to_lowercase().contains(&query)
                        && !b.faction.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    let is_sel = state.inspect_target == SdkInspectTarget::Building(bid.clone());
                    let dirty_mark = if state.modified_buildings.contains(&bid) {
                        " *"
                    } else {
                        ""
                    };
                    ui.horizontal(|ui| {
                        draw_cameo_or_badge(ui, state, &bid, b.visual.primary_color);
                        let label = format!(
                            "{}{} [{}] — ${} | {}x{}",
                            b.name, dirty_mark, b.faction, b.cost, b.footprint[0], b.footprint[1]
                        );
                        if ui.selectable_label(is_sel, label).clicked() {
                            state.inspect_target = SdkInspectTarget::Building(bid.clone());
                            state.selected_building_brush_id = bid.clone();
                            focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                        }
                        if ui
                            .small_button("🗑")
                            .on_hover_text("Delete Building")
                            .clicked()
                        {
                            state.pending_delete_confirm = Some(PendingSchemaDelete {
                                kind: SchemaKind::Building,
                                references: loaded.find_id_references(&bid),
                                id: bid.clone(),
                            });
                        }
                    });
                }
            }
            AssetBrowserCategory::Weapons => {
                let ids: Vec<String> = loaded.weapons.keys().cloned().collect();
                for wid in ids {
                    let Some(w) = loaded.weapons.get(&wid) else {
                        continue;
                    };
                    if !query.is_empty()
                        && !wid.to_lowercase().contains(&query)
                        && !w.name.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    let is_sel = state.inspect_target == SdkInspectTarget::Weapon(wid.clone());
                    let dirty_mark = if state.modified_weapons.contains(&wid) {
                        " *"
                    } else {
                        ""
                    };
                    ui.horizontal(|ui| {
                        let label = format!(
                            "{}{} ({}) — {:.0} dmg | {:.1} rng | {:?}",
                            w.name, dirty_mark, wid, w.damage, w.range, w.delivery
                        );
                        if ui.selectable_label(is_sel, label).clicked() {
                            state.inspect_target = SdkInspectTarget::Weapon(wid.clone());
                            focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                        }
                        if ui
                            .small_button("🗑")
                            .on_hover_text("Delete Weapon")
                            .clicked()
                        {
                            state.pending_delete_confirm = Some(PendingSchemaDelete {
                                kind: SchemaKind::Weapon,
                                references: loaded.find_id_references(&wid),
                                id: wid.clone(),
                            });
                        }
                    });
                }
            }
            AssetBrowserCategory::Vfx => {
                let ids: Vec<String> = loaded.vfx_effects.keys().cloned().collect();
                if ids.is_empty() {
                    ui.weak("No VfxEffectDef entries in vfx/vfx.toml yet. Click '+ New VFX Effect' or open Tab 11 VFX Studio!");
                }
                for vid in ids {
                    let Some(eff) = loaded.vfx_effects.get(&vid) else {
                        continue;
                    };
                    if !query.is_empty()
                        && !vid.to_lowercase().contains(&query)
                        && !eff.name.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    let is_sel = state.vfx_studio_ui.selected_effect_id == vid;
                    ui.horizontal(|ui| {
                        let lbl = format!(
                            "✨ {} ({}) — {} emitter(s) | {:.1}s",
                            if eff.name.is_empty() { &vid } else { &eff.name },
                            vid,
                            eff.emitters.len(),
                            eff.duration_secs
                        );
                        if ui.selectable_label(is_sel, lbl).clicked() {
                            state.vfx_studio_ui.selected_effect_id = vid.clone();
                            state.vfx_studio_ui.working_effect = Some(eff.clone());
                            state.vfx_studio_ui.pending_showcase_stage_sync = true;
                            focus_or_open_tab(&mut state.dock_state, StudioTab::VfxStudio);
                        }
                        if ui.small_button("🗑").clicked() {
                            state.pending_delete_confirm = Some(PendingSchemaDelete {
                                kind: SchemaKind::VfxEffect,
                                references: loaded.find_id_references(&vid),
                                id: vid.clone(),
                            });
                        }
                    });
                }
            }
            AssetBrowserCategory::Upgrades => {
                let ids: Vec<String> = loaded.upgrades.keys().cloned().collect();
                for uid in ids {
                    let Some(up) = loaded.upgrades.get(&uid) else {
                        continue;
                    };
                    if !query.is_empty()
                        && !uid.to_lowercase().contains(&query)
                        && !up.name.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    let is_sel = state.inspect_target == SdkInspectTarget::Upgrade(uid.clone());
                    let dirty = if state.modified_upgrades.contains(&uid) {
                        " *"
                    } else {
                        ""
                    };
                    ui.horizontal(|ui| {
                        let lbl = format!(
                            "⬆ {}{} ({}) [{}] — ${} | {:.1}s",
                            up.name, dirty, uid, up.faction, up.cost, up.research_time_secs
                        );
                        if ui.selectable_label(is_sel, lbl).clicked() {
                            state.inspect_target = SdkInspectTarget::Upgrade(uid.clone());
                            focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                        }
                        if ui.small_button("🗑").clicked() {
                            state.pending_delete_confirm = Some(PendingSchemaDelete {
                                kind: SchemaKind::Upgrade,
                                references: loaded.find_id_references(&uid),
                                id: uid.clone(),
                            });
                        }
                    });
                }
            }
            AssetBrowserCategory::CommanderTrees => {
                let ids: Vec<String> = loaded.commander_trees.keys().cloned().collect();
                for tid in ids {
                    let Some(t) = loaded.commander_trees.get(&tid) else {
                        continue;
                    };
                    if !query.is_empty()
                        && !tid.to_lowercase().contains(&query)
                        && !t.display_name.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    let is_sel =
                        state.inspect_target == SdkInspectTarget::CommanderTree(tid.clone());
                    ui.horizontal(|ui| {
                        let lbl = format!(
                            "🎖 {} ({}) [{}] — {} node(s)",
                            t.display_name,
                            tid,
                            t.faction,
                            t.nodes.len()
                        );
                        if ui.selectable_label(is_sel, lbl).clicked() {
                            state.inspect_target = SdkInspectTarget::CommanderTree(tid.clone());
                            focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                        }
                        if ui.small_button("🗑").clicked() {
                            state.pending_delete_confirm = Some(PendingSchemaDelete {
                                kind: SchemaKind::CommanderTree,
                                references: loaded.find_id_references(&tid),
                                id: tid.clone(),
                            });
                        }
                    });
                }
            }
            AssetBrowserCategory::Factions => {
                let ids: Vec<String> = loaded.factions.keys().cloned().collect();
                for fid in ids {
                    let Some(f) = loaded.factions.get(&fid) else {
                        continue;
                    };
                    if !query.is_empty()
                        && !fid.to_lowercase().contains(&query)
                        && !f.name.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    let is_sel = state.inspect_target == SdkInspectTarget::Faction(fid.clone());
                    ui.horizontal(|ui| {
                        draw_cameo_or_badge(ui, state, &fid, f.color);
                        let lbl = format!(
                            "{} ({}) — ${} | {} sub-faction(s)",
                            f.name,
                            fid,
                            f.starting_credits,
                            f.sub_factions.len()
                        );
                        if ui.selectable_label(is_sel, lbl).clicked() {
                            state.inspect_target = SdkInspectTarget::Faction(fid.clone());
                            focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                        }
                        if ui.small_button("🗑").clicked() {
                            state.pending_delete_confirm = Some(PendingSchemaDelete {
                                kind: SchemaKind::Faction,
                                references: loaded.find_id_references(&fid),
                                id: fid.clone(),
                            });
                        }
                    });
                }
            }
            AssetBrowserCategory::Resources => {
                let ids: Vec<String> = loaded.resources.keys().cloned().collect();
                for rid in ids {
                    let Some(r) = loaded.resources.get(&rid) else {
                        continue;
                    };
                    if !query.is_empty()
                        && !rid.to_lowercase().contains(&query)
                        && !r.display_name.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    let is_sel = state.inspect_target == SdkInspectTarget::Resource(rid.clone());
                    ui.horizontal(|ui| {
                        draw_cameo_or_badge(ui, state, &rid, r.color_rgb);
                        let lbl = format!(
                            "{} ({}) — Start: {}{}",
                            r.display_name,
                            rid,
                            r.starting_amount,
                            if r.is_capacity_pool { " [Pool]" } else { "" }
                        );
                        if ui.selectable_label(is_sel, lbl).clicked() {
                            state.inspect_target = SdkInspectTarget::Resource(rid.clone());
                            focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                        }
                        if ui.small_button("🗑").clicked() {
                            state.pending_delete_confirm = Some(PendingSchemaDelete {
                                kind: SchemaKind::Resource,
                                references: loaded.find_id_references(&rid),
                                id: rid.clone(),
                            });
                        }
                    });
                }
            }
            AssetBrowserCategory::AiProfiles => {
                let ids: Vec<String> = loaded.ai_profiles.keys().cloned().collect();
                for aid in ids {
                    let Some(ai) = loaded.ai_profiles.get(&aid) else {
                        continue;
                    };
                    if !query.is_empty()
                        && !aid.to_lowercase().contains(&query)
                        && !ai.personality.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    let is_sel = state.inspect_target == SdkInspectTarget::AiProfile(aid.clone());
                    ui.horizontal(|ui| {
                        let lbl = format!(
                            "🤖 {} [{}] — {} ({} BO steps)",
                            aid,
                            ai.faction,
                            ai.personality,
                            ai.build_order.len()
                        );
                        if ui.selectable_label(is_sel, lbl).clicked() {
                            state.inspect_target = SdkInspectTarget::AiProfile(aid.clone());
                            focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                        }
                        if ui.small_button("🌳 BT").on_hover_text("Open AI Behavior Graph").clicked() {
                            state.ai_graph_ui.selected_profile_id = aid.clone();
                            focus_or_open_tab(&mut state.dock_state, StudioTab::AiBehaviorGraph);
                        }
                        if ui.small_button("🗑").clicked() {
                            state.pending_delete_confirm = Some(PendingSchemaDelete {
                                kind: SchemaKind::AiProfile,
                                references: loaded.find_id_references(&aid),
                                id: aid.clone(),
                            });
                        }
                    });
                }
            }
            AssetBrowserCategory::Maps => {
                let map_ids: Vec<String> = loaded.maps.keys().cloned().collect();
                for mid in map_ids {
                    let Some(m) = loaded.maps.get(&mid).cloned() else {
                        continue;
                    };
                    if !query.is_empty()
                        && !mid.to_lowercase().contains(&query)
                        && !m.name.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    let is_active = loaded.entry_map.id == mid;
                    ui.horizontal(|ui| {
                        let label = format!(
                            "{} ({}) — {}x{} | {} spawns",
                            m.name,
                            mid,
                            m.width,
                            m.height,
                            m.spawns.len()
                        );
                        if ui.selectable_label(is_active, label).clicked() {
                            loaded.entry_map = m.clone();
                            *world = SimWorld::from_mod(loaded);
                            interaction.active_map_id = mid.clone();
                            terrain_dirty.0 = true;
                            state.history.clear();
                            state.status_banner =
                                format!("Loaded map '{}' ({mid}) into 3D viewport.", m.name);
                            focus_or_open_tab(&mut state.dock_state, StudioTab::WorldBuilder);
                        }
                        if ui.small_button("🗑").on_hover_text("Delete Map").clicked() {
                            state.pending_delete_confirm = Some(PendingSchemaDelete {
                                kind: SchemaKind::Map,
                                references: loaded.find_id_references(&mid),
                                id: mid.clone(),
                            });
                        }
                    });
                }
            }
            AssetBrowserCategory::Campaigns => {
                let camp_ids: Vec<String> = loaded.campaigns.keys().cloned().collect();
                for cid in camp_ids {
                    let Some(c) = loaded.campaigns.get(&cid) else {
                        continue;
                    };
                    if !query.is_empty()
                        && !cid.to_lowercase().contains(&query)
                        && !c.display_name.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        let label = format!(
                            "{} [{}] — {} mission(s)",
                            c.display_name,
                            c.faction,
                            c.missions.len()
                        );
                        let is_sel =
                            state.inspect_target == SdkInspectTarget::Campaign(cid.clone())
                                || state.selected_campaign_id == cid;
                        if ui.selectable_label(is_sel, label).clicked() {
                            state.selected_campaign_id = cid.clone();
                            state.selected_mission_idx = 0;
                            state.inspect_target = SdkInspectTarget::Campaign(cid.clone());
                            focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                        }
                        if ui.small_button("Triggers").clicked() {
                            state.selected_campaign_id = cid.clone();
                            state.selected_mission_idx = 0;
                            focus_or_open_tab(&mut state.dock_state, StudioTab::TriggerStudio);
                        }
                        if ui.small_button("🗑").clicked() {
                            state.pending_delete_confirm = Some(PendingSchemaDelete {
                                kind: SchemaKind::Campaign,
                                references: loaded.find_id_references(&cid),
                                id: cid.clone(),
                            });
                        }
                    });
                }
            }
            AssetBrowserCategory::Prefabs => {
                if state.spline_tools.cached_prefabs.is_empty() {
                    ui.weak("No MapPrefabDef files in prefabs/*.prefab.toml yet. Capture a selection in Tab 4 World Builder!");
                }
                let prefabs = state.spline_tools.cached_prefabs.clone();
                for (idx, pf) in prefabs.iter().enumerate() {
                    if !query.is_empty()
                        && !pf.id.to_lowercase().contains(&query)
                        && !pf.display_name.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        let sel = state.spline_tools.selected_prefab_idx == idx
                            && state.spline_tools.mode
                                == crate::studio::spline_and_prefab_tools::ActiveAdvancedTerrainMode::PrefabStamp;
                        if ui
                            .selectable_label(
                                sel,
                                format!(
                                    "🧩 {} ({}) [{}] — {}x{} ({} ents, {} props)",
                                    pf.display_name,
                                    pf.id,
                                    pf.category,
                                    pf.footprint_size[0],
                                    pf.footprint_size[1],
                                    pf.entities.len(),
                                    pf.props.len()
                                ),
                            )
                            .clicked()
                        {
                            state.spline_tools.selected_prefab_idx = idx;
                            state.spline_tools.mode =
                                crate::studio::spline_and_prefab_tools::ActiveAdvancedTerrainMode::PrefabStamp;
                            state.active_tool = crate::studio::world_builder::WorldBuilderTool::SelectAndMove;
                            focus_or_open_tab(&mut state.dock_state, StudioTab::WorldBuilder);
                        }
                        if ui.small_button("🗑").clicked() {
                            let _ = loaded.delete_prefab_def(&pf.id);
                            state.spline_tools.reload_prefabs(loaded);
                        }
                    });
                }
            }
            AssetBrowserCategory::Scripts => {
                let paths: Vec<String> = loaded.raw_scripts.keys().cloned().collect();
                if paths.is_empty() {
                    ui.weak("No .rhai / .lua scripts in this mod yet. Open Tab 6 to create one.");
                }
                for rel in paths {
                    if !query.is_empty() && !rel.to_lowercase().contains(&query) {
                        continue;
                    }
                    if ui
                        .selectable_label(state.active_script_rel_path == rel, format!("📜 {rel}"))
                        .clicked()
                    {
                        if let Some(src) = loaded.raw_scripts.get(&rel).cloned() {
                            state.active_script_rel_path = rel.clone();
                            state.script_editor_buffer = src.clone();
                            state.script_validation_status = rts_sdk::scripting::ScriptHost::new()
                                .load_script_file(&rel, &src)
                                .map_err(|e| e.to_string());
                            state.trigger_studio_graph_mode = false;
                            focus_or_open_tab(&mut state.dock_state, StudioTab::TriggerStudio);
                        }
                    }
                }
            }
            AssetBrowserCategory::Props => {
                let props = [
                    (0usize, PropKind::Tree, "Destructible Tree / Foliage"),
                    (1usize, PropKind::Streetlight, "Urban Streetlight Post"),
                    (2usize, PropKind::ExplosiveBarrel, "Volatile Explosive Barrel"),
                ];
                for (idx, kind, desc) in props {
                    if !query.is_empty() && !desc.to_lowercase().contains(&query) {
                        continue;
                    }
                    let sel = state.selected_prop_kind_index == idx
                        && state.active_tool == crate::studio::world_builder::WorldBuilderTool::PlaceProp;
                    if ui
                        .selectable_label(sel, format!("🌲 {desc} ({kind:?})"))
                        .clicked()
                    {
                        state.selected_prop_kind_index = idx;
                        state.active_tool = crate::studio::world_builder::WorldBuilderTool::PlaceProp;
                        focus_or_open_tab(&mut state.dock_state, StudioTab::WorldBuilder);
                    }
                }
            }
            AssetBrowserCategory::Audio => {
                ui.horizontal(|ui| {
                    ui.strong("Soundtrack & Jukebox");
                    if ui.small_button("⚙ Edit AudioCatalog in Inspector").clicked() {
                        state.inspect_target = SdkInspectTarget::Audio;
                        focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                    }
                });
                for track in &loaded.audio.music_tracks {
                    if !query.is_empty()
                        && !track.id.to_lowercase().contains(&query)
                        && !track.display_name.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        if ui.small_button("▶").clicked() {
                            sound_res.play_music_track(&track.id);
                            state.status_banner = format!(
                                "Playing music track '{}' ({})",
                                track.display_name, track.id
                            );
                        }
                        ui.label(format!("{} ({})", track.display_name, track.path));
                    });
                }
                ui.separator();
                ui.strong("EVA Announcer Cues");
                for (cue_id, file_rel) in &loaded.audio.eva_cues {
                    if !query.is_empty() && !cue_id.to_lowercase().contains(&query) {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        if ui.small_button("▶").clicked() {
                            sound_res.push_eva_announcement("shared", cue_id);
                            state.status_banner = format!("Triggered EVA cue '{cue_id}'");
                        }
                        ui.label(format!("{cue_id} -> {file_rel}"));
                    });
                }
                ui.separator();
                ui.strong("Weapon SFX");
                for (wid, sfx_rel) in &loaded.audio.weapon_sfx {
                    if !query.is_empty() && !wid.to_lowercase().contains(&query) {
                        continue;
                    }
                    ui.label(format!("🔊 {wid} -> {sfx_rel}"));
                }
            }
            AssetBrowserCategory::Videos => {
                let clips = collect_mod_video_paths(loaded);
                if clips.is_empty() {
                    ui.weak("No FMV clips configured in campaigns; synthetic test signal available in Tab 7.");
                }
                let mut play_clip: Option<String> = None;
                for rel in clips {
                    if !query.is_empty() && !rel.to_lowercase().contains(&query) {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        if ui.small_button("▶ Preview").clicked() {
                            play_clip = Some(rel.clone());
                        }
                        ui.label(&rel);
                    });
                }
                if let Some(rel) = play_clip {
                    if let Some(vres) = video_res {
                        let full = loaded.resolve_asset_path(&rel);
                        vres.play_file(&full, &rel);
                    }
                    interaction.active_video_preview_path = Some(rel);
                    focus_or_open_tab(&mut state.dock_state, StudioTab::AssetPreviewer);
                }
            }
        });
}

fn draw_cameo_or_badge(ui: &mut egui::Ui, state: &StudioState, entity_id: &str, rgb: [f32; 3]) {
    if let Some(&tex_id) = state.cameo_egui_textures.get(entity_id) {
        ui.image(egui::load::SizedTexture::new(
            tex_id,
            egui::vec2(22.0, 18.0),
        ));
    } else {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
        let col = egui::Color32::from_rgb(
            (rgb[0].clamp(0.0, 1.0) * 255.0) as u8,
            (rgb[1].clamp(0.0, 1.0) * 255.0) as u8,
            (rgb[2].clamp(0.0, 1.0) * 255.0) as u8,
        );
        ui.painter().rect_filled(rect, 3.0, col);
    }
}
