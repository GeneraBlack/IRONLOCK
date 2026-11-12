use super::trigger_graph_check::validate_trigger_graph;
use super::{check_existing_asset_binary, ValidationReport};
use crate::loader::LoadedMod;
use semver::{Version, VersionReq};
use std::collections::BTreeSet;

pub(super) fn validate_manifest_resources_factions_and_vfx(
    loaded: &LoadedMod,
    require_model_files: bool,
    checked_binary_assets: &mut BTreeSet<String>,
    report: &mut ValidationReport,
) {
    // Validate SemVer on manifest.version and manifest.depends_on
    let ver_str = loaded.manifest.version.trim();
    if !ver_str.is_empty() && Version::parse(ver_str).is_err() {
        report.push_error(
            "INVALID_SEMVER",
            format!(
                "Mod '{}' has invalid SemVer version '{}'",
                loaded.manifest.id, loaded.manifest.version
            ),
        );
    }
    for dep in &loaded.manifest.depends_on {
        if dep.id.trim().is_empty() {
            report.push_error(
                "INVALID_DEPENDENCY",
                format!(
                    "Mod '{}' declares a depends_on entry with an empty id",
                    loaded.manifest.id
                ),
            );
        }
        let req_str = dep.version_req.trim();
        if !req_str.is_empty() && VersionReq::parse(req_str).is_err() {
            report.push_error(
                "INVALID_SEMVER",
                format!(
                    "Mod '{}' dependency '{}' has invalid SemVer version_req '{}'",
                    loaded.manifest.id, dep.id, dep.version_req
                ),
            );
        }
    }

    if loaded.factions.is_empty() {
        report.push_error("NO_FACTIONS", "Mod defines zero factions");
    }

    if loaded.resources.is_empty() {
        report.push_error("NO_RESOURCES", "Mod defines zero resource types");
    }
    for (r_id, r_def) in &loaded.resources {
        if r_def.id.trim().is_empty() || r_def.id != *r_id {
            report.push_error(
                "INVALID_RESOURCE_DEF",
                format!(
                    "Resource '{r_id}' has empty or mismatched id '{}'",
                    r_def.id
                ),
            );
        }
        if r_def.display_name.trim().is_empty() {
            report.push_error(
                "INVALID_RESOURCE_DEF",
                format!("Resource '{r_id}' has empty display_name"),
            );
        }
        if r_def
            .color_rgb
            .iter()
            .any(|c| !c.is_finite() || !(0.0..=1.0).contains(c))
        {
            report.push_error(
                "INVALID_RESOURCE_DEF",
                format!(
                    "Resource '{r_id}' color_rgb {:?} must have finite components in 0.0..=1.0",
                    r_def.color_rgb
                ),
            );
        }
        if let Some(icon_rel) = &r_def.icon {
            if !icon_rel.is_empty() {
                let icon_path = loaded.resolve_asset_path(icon_rel);
                if !loaded.asset_exists(icon_rel) {
                    let msg = format!(
                        "Resource '{r_id}' icon '{icon_rel}' does not exist at {}",
                        icon_path.display()
                    );
                    if require_model_files {
                        report.push_error("MISSING_ICON", msg);
                    } else {
                        report.push_warning("MISSING_ICON", msg);
                    }
                } else {
                    check_existing_asset_binary(
                        loaded,
                        icon_rel,
                        &format!("Resource '{r_id}' icon"),
                        checked_binary_assets,
                        report,
                    );
                }
            }
        }
    }

    // Validate faction HUD skin & loading screen assets
    for (f_id, f_def) in &loaded.factions {
        if let Some(ls) = &f_def.loading_screen {
            let full = loaded.resolve_asset_path(ls);
            if !loaded.asset_exists(ls) {
                report.push_error(
                    "MISSING_HUD_ASSET",
                    format!(
                        "Faction '{f_id}' loading_screen '{}' does not exist at {}",
                        ls,
                        full.display()
                    ),
                );
            } else {
                check_existing_asset_binary(
                    loaded,
                    ls,
                    &format!("Faction '{f_id}' loading_screen"),
                    checked_binary_assets,
                    report,
                );
            }
        }
        let hs = &f_def.hud_skin;
        for camp_ls in &f_def.campaign_loading_screens {
            if !camp_ls.is_empty() {
                let full = loaded.resolve_asset_path(camp_ls);
                if !loaded.asset_exists(camp_ls) {
                    report.push_error(
                        "MISSING_HUD_ASSET",
                        format!(
                            "Faction '{f_id}' campaign_loading_screens '{}' does not exist at {}",
                            camp_ls,
                            full.display()
                        ),
                    );
                } else {
                    check_existing_asset_binary(
                        loaded,
                        camp_ls,
                        &format!("Faction '{f_id}' campaign_loading_screens"),
                        checked_binary_assets,
                        report,
                    );
                }
            }
        }
        for sf in &f_def.sub_factions {
            if let Some(sf_ls) = &sf.loading_screen {
                if !sf_ls.is_empty() {
                    let full = loaded.resolve_asset_path(sf_ls);
                    if !loaded.asset_exists(sf_ls) {
                        report.push_error(
                            "MISSING_HUD_ASSET",
                            format!(
                                "Faction '{f_id}' sub_faction '{}' loading_screen '{}' does not exist at {}",
                                sf.id,
                                sf_ls,
                                full.display()
                            ),
                        );
                    } else {
                        check_existing_asset_binary(
                            loaded,
                            sf_ls,
                            &format!("Faction '{f_id}' sub_faction '{}' loading_screen", sf.id),
                            checked_binary_assets,
                            report,
                        );
                    }
                }
            }
        }
        for (field_name, opt_path) in [
            ("dialog_background", &hs.dialog_background),
            ("credits_panel", &hs.credits_panel),
            ("top_panel", &hs.top_panel),
            ("options_button", &hs.options_button),
            ("diplomacy_button", &hs.diplomacy_button),
            ("radar_frame", &hs.radar_frame),
            ("side_header", &hs.side_header),
            ("repair_button", &hs.repair_button),
            ("sell_button", &hs.sell_button),
            ("cameo_row_frame", &hs.cameo_row_frame),
            ("cameo_footer_frame", &hs.cameo_footer_frame),
            ("addon_panel", &hs.addon_panel),
            ("power_bar_icon", &hs.power_bar_icon),
        ] {
            if let Some(rel_path) = opt_path {
                let full = loaded.resolve_asset_path(rel_path);
                if !loaded.asset_exists(rel_path) {
                    report.push_error(
                        "MISSING_HUD_ASSET",
                        format!(
                            "Faction '{f_id}' hud_skin.{field_name} '{}' does not exist at {}",
                            rel_path,
                            full.display()
                        ),
                    );
                } else {
                    check_existing_asset_binary(
                        loaded,
                        rel_path,
                        &format!("Faction '{f_id}' hud_skin.{field_name}"),
                        checked_binary_assets,
                        report,
                    );
                }
            }
        }
        for tab_icon in &hs.tab_icons {
            let full = loaded.resolve_asset_path(tab_icon);
            if !loaded.asset_exists(tab_icon) {
                report.push_error(
                    "MISSING_HUD_ASSET",
                    format!(
                        "Faction '{f_id}' hud_skin.tab_icons '{}' does not exist at {}",
                        tab_icon,
                        full.display()
                    ),
                );
            } else {
                check_existing_asset_binary(
                    loaded,
                    tab_icon,
                    &format!("Faction '{f_id}' hud_skin.tab_icons"),
                    checked_binary_assets,
                    report,
                );
            }
        }
    }

    // Validate optional VFX flipbook catalog sprite sheets
    for (vfx_id, vfx_def) in &loaded.vfx_catalog {
        let full = loaded.resolve_asset_path(&vfx_def.sprite_sheet);
        if !loaded.asset_exists(&vfx_def.sprite_sheet) {
            report.push_error(
                "MISSING_VFX_SPRITE_SHEET",
                format!(
                    "VFX flipbook '{vfx_id}' sprite_sheet '{}' does not exist at {}",
                    vfx_def.sprite_sheet,
                    full.display()
                ),
            );
        } else {
            check_existing_asset_binary(
                loaded,
                &vfx_def.sprite_sheet,
                &format!("VFX flipbook '{vfx_id}' sprite_sheet"),
                checked_binary_assets,
                report,
            );
        }
    }

    // Validate Phase 28 TOML VfxCatalog (`vfx_effects`)
    for (eff_id, eff) in &loaded.vfx_effects {
        if eff.id.trim().is_empty()
            || !eff.duration_secs.is_finite()
            || eff.duration_secs <= 0.0
            || !eff.camera_shake_intensity.is_finite()
            || eff.camera_shake_intensity < 0.0
            || !eff.camera_shake_radius.is_finite()
            || eff.camera_shake_radius < 0.0
        {
            report.push_error(
                "INVALID_VFX_EFFECT",
                format!("VFX effect '{eff_id}' has invalid duration or camera shake values"),
            );
        }
        if let Some(decal) = &eff.ground_decal {
            if decal.decal_kind.trim().is_empty()
                || !decal.radius.is_finite()
                || decal.radius <= 0.0
                || !decal.fade_time_secs.is_finite()
                || decal.fade_time_secs <= 0.0
            {
                report.push_error(
                    "INVALID_VFX_EFFECT",
                    format!("VFX effect '{eff_id}' ground_decal has invalid parameters"),
                );
            }
        }
        for em in &eff.emitters {
            if em.id.trim().is_empty()
                || !em.duration_secs.is_finite()
                || em.duration_secs <= 0.0
                || !em.continuous_rate_per_sec.is_finite()
                || em.continuous_rate_per_sec < 0.0
                || em.lifetime_range_secs[0] <= 0.0
                || em.lifetime_range_secs[1] < em.lifetime_range_secs[0]
                || em.speed_range[0] < 0.0
                || em.speed_range[1] < em.speed_range[0]
            {
                report.push_error(
                    "INVALID_VFX_EMITTER",
                    format!(
                        "VFX effect '{eff_id}' emitter '{}' has invalid lifetime, speed, or rate",
                        em.id
                    ),
                );
            }
            if let Some(tex_rel) = em.texture_path.as_deref().filter(|s| !s.is_empty()) {
                let full = loaded.resolve_asset_path(tex_rel);
                if !loaded.asset_exists(tex_rel) {
                    let msg = format!(
                        "VFX effect '{eff_id}' emitter '{}' texture_path '{tex_rel}' does not exist at {}",
                        em.id,
                        full.display()
                    );
                    if require_model_files {
                        report.push_error("MISSING_VFX_TEXTURE", msg);
                    } else {
                        report.push_warning("MISSING_VFX_TEXTURE", msg);
                    }
                } else {
                    check_existing_asset_binary(
                        loaded,
                        tex_rel,
                        &format!("VFX effect '{eff_id}' emitter '{}'", em.id),
                        checked_binary_assets,
                        report,
                    );
                }
            }
        }
    }
}

pub(super) fn validate_campaigns_and_audio(
    loaded: &LoadedMod,
    defined_support_powers: &BTreeSet<&str>,
    checked_binary_assets: &mut BTreeSet<String>,
    report: &mut ValidationReport,
) {
    let empty_set: BTreeSet<&str> = BTreeSet::new();
    for (c_id, campaign) in &loaded.campaigns {
        if !loaded.factions.contains_key(&campaign.faction) {
            report.push_error(
                "UNKNOWN_CAMPAIGN_FACTION",
                format!(
                    "Campaign '{c_id}' references unknown faction '{}'",
                    campaign.faction
                ),
            );
        }
        let mission_ids: BTreeSet<&str> = campaign.missions.iter().map(|m| m.id.as_str()).collect();
        for mission in &campaign.missions {
            if !loaded.maps.contains_key(&mission.map_id) {
                report.push_error(
                    "UNKNOWN_CAMPAIGN_MAP",
                    format!(
                        "Campaign '{c_id}' mission '{}' references unknown map '{}'",
                        mission.id, mission.map_id
                    ),
                );
            }
            if let Some(next_id) = &mission.next_mission_id {
                if !mission_ids.contains(next_id.as_str()) {
                    report.push_error(
                        "UNKNOWN_NEXT_MISSION",
                        format!(
                            "Campaign '{c_id}' mission '{}' references unknown next_mission_id '{next_id}'",
                            mission.id
                        ),
                    );
                }
            }
            for (field_name, opt_path) in [
                ("loading_screen", &mission.loading_screen),
                ("intro_video", &mission.intro_video),
                ("victory_video", &mission.victory_video),
                ("radar_video", &mission.radar_video),
            ] {
                if let Some(rel_path) = opt_path {
                    let full = loaded.resolve_asset_path(rel_path);
                    if !loaded.asset_exists(rel_path) {
                        report.push_error(
                            "MISSING_MISSION_MEDIA",
                            format!(
                                "Campaign '{c_id}' mission '{}' {field_name} '{}' does not exist at {}",
                                mission.id,
                                rel_path,
                                full.display()
                            ),
                        );
                    } else {
                        check_existing_asset_binary(
                            loaded,
                            rel_path,
                            &format!("Campaign '{c_id}' mission '{}' {field_name}", mission.id),
                            checked_binary_assets,
                            report,
                        );
                    }
                }
            }
            let map_ref = loaded.maps.get(&mission.map_id);
            for graph in &mission.trigger_graphs {
                validate_trigger_graph(
                    graph,
                    &format!("Campaign '{c_id}' mission '{}'", mission.id),
                    map_ref,
                    &empty_set,
                    &empty_set,
                    &empty_set,
                    defined_support_powers,
                    loaded,
                    checked_binary_assets,
                    report,
                );
            }
        }
    }

    // Validate audio catalog
    for (w_id, rel_path) in &loaded.audio.weapon_sfx {
        if !loaded.weapons.contains_key(w_id) {
            report.push_warning(
                "UNKNOWN_AUDIO_WEAPON",
                format!("AudioCatalog weapon_sfx references unknown weapon '{w_id}'"),
            );
        }
        let full = loaded.resolve_asset_path(rel_path);
        if !loaded.asset_exists(rel_path) {
            report.push_error(
                "MISSING_AUDIO_ASSET",
                format!(
                    "Weapon SFX '{w_id}' audio file '{}' does not exist at {}",
                    rel_path,
                    full.display()
                ),
            );
        } else {
            check_existing_asset_binary(
                loaded,
                rel_path,
                &format!("Weapon SFX '{w_id}'"),
                checked_binary_assets,
                report,
            );
        }
    }
    for (u_id, voice_spec) in &loaded.audio.unit_voices {
        if !loaded.units.contains_key(u_id) {
            report.push_warning(
                "UNKNOWN_VOICE_UNIT",
                format!("AudioCatalog unit_voices references unknown unit '{u_id}'"),
            );
        }
        for rel_path in voice_spec
            .select
            .iter()
            .chain(voice_spec.move_order.iter())
            .chain(voice_spec.attack_order.iter())
            .chain(voice_spec.death.iter())
        {
            let full = loaded.resolve_asset_path(rel_path);
            if !loaded.asset_exists(rel_path) {
                report.push_error(
                    "MISSING_AUDIO_ASSET",
                    format!(
                        "Unit '{u_id}' voice audio file '{}' does not exist at {}",
                        rel_path,
                        full.display()
                    ),
                );
            } else {
                check_existing_asset_binary(
                    loaded,
                    rel_path,
                    &format!("Unit '{u_id}' voice"),
                    checked_binary_assets,
                    report,
                );
            }
        }
    }
    for track in &loaded.audio.music_tracks {
        let full = loaded.resolve_asset_path(&track.path);
        if !loaded.asset_exists(&track.path) {
            report.push_error(
                "MISSING_AUDIO_ASSET",
                format!(
                    "Music track '{}' audio file '{}' does not exist at {}",
                    track.id,
                    track.path,
                    full.display()
                ),
            );
        } else {
            check_existing_asset_binary(
                loaded,
                &track.path,
                &format!("Music track '{}'", track.id),
                checked_binary_assets,
                report,
            );
        }
        for f_id in track.factions.iter().chain(track.faction_tags.iter()) {
            let trimmed = f_id.trim();
            if !trimmed.is_empty()
                && !matches!(trimmed, "*" | "any" | "neutral" | "all")
                && !loaded.factions.contains_key(trimmed)
            {
                report.push_error(
                    "UNKNOWN_MUSIC_FACTION",
                    format!(
                        "Music track '{}' references unknown faction '{trimmed}'",
                        track.id
                    ),
                );
            }
        }
    }
    for (cue_id, rel_path) in &loaded.audio.eva_cues {
        let full = loaded.resolve_asset_path(rel_path);
        if !loaded.asset_exists(rel_path) {
            report.push_error(
                "MISSING_AUDIO_ASSET",
                format!(
                    "EVA cue '{cue_id}' audio file '{}' does not exist at {}",
                    rel_path,
                    full.display()
                ),
            );
        } else {
            check_existing_asset_binary(
                loaded,
                rel_path,
                &format!("EVA cue '{cue_id}'"),
                checked_binary_assets,
                report,
            );
        }
    }
    for (f_id, cues) in &loaded.audio.faction_eva_cues {
        if !loaded.factions.contains_key(f_id) {
            report.push_error(
                "UNKNOWN_AUDIO_FACTION",
                format!("AudioCatalog faction_eva_cues references unknown faction '{f_id}'"),
            );
        }
        for (cue_key, rel_path) in cues {
            let full = loaded.resolve_asset_path(rel_path);
            if !loaded.asset_exists(rel_path) {
                report.push_error(
                    "MISSING_AUDIO_ASSET",
                    format!(
                        "Faction '{f_id}' EVA cue '{cue_key}' audio file '{}' does not exist at {}",
                        rel_path,
                        full.display()
                    ),
                );
            } else {
                check_existing_asset_binary(
                    loaded,
                    rel_path,
                    &format!("Faction '{f_id}' EVA cue '{cue_key}'"),
                    checked_binary_assets,
                    report,
                );
            }
        }
    }
}
