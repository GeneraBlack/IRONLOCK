use crate::loader::LoadedMod;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub severity: Severity,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        !self.issues.iter().any(|i| i.severity == Severity::Error)
    }

    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count()
    }

    fn push_error(&mut self, code: impl Into<String>, message: impl Into<String>) {
        self.issues.push(ValidationIssue {
            severity: Severity::Error,
            code: code.into(),
            message: message.into(),
        });
    }

    fn push_warning(&mut self, code: impl Into<String>, message: impl Into<String>) {
        self.issues.push(ValidationIssue {
            severity: Severity::Warning,
            code: code.into(),
            message: message.into(),
        });
    }
}

pub fn validate_mod(loaded: &LoadedMod, require_model_files: bool) -> ValidationReport {
    let mut report = ValidationReport::default();

    for dup in &loaded.duplicate_ids {
        report.push_error("DUPLICATE_ID", format!("Duplicate catalog ID '{dup}'"));
    }

    if loaded.factions.is_empty() {
        report.push_error("NO_FACTIONS", "Mod defines zero factions");
    }

    // Validate faction HUD skin & loading screen assets
    for (f_id, f_def) in &loaded.factions {
        if let Some(ls) = &f_def.loading_screen {
            let full = loaded.resolve_asset_path(ls);
            if !full.exists() {
                report.push_error(
                    "MISSING_HUD_ASSET",
                    format!(
                        "Faction '{f_id}' loading_screen '{}' does not exist at {}",
                        ls,
                        full.display()
                    ),
                );
            }
        }
        let hs = &f_def.hud_skin;
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
                if !full.exists() {
                    report.push_error(
                        "MISSING_HUD_ASSET",
                        format!(
                            "Faction '{f_id}' hud_skin.{field_name} '{}' does not exist at {}",
                            rel_path,
                            full.display()
                        ),
                    );
                }
            }
        }
        for tab_icon in &hs.tab_icons {
            let full = loaded.resolve_asset_path(tab_icon);
            if !full.exists() {
                report.push_error(
                    "MISSING_HUD_ASSET",
                    format!(
                        "Faction '{f_id}' hud_skin.tab_icons '{}' does not exist at {}",
                        tab_icon,
                        full.display()
                    ),
                );
            }
        }
    }

    // Validate optional VFX catalog sprite sheets
    for (vfx_id, vfx_def) in &loaded.vfx_catalog {
        let full = loaded.resolve_asset_path(&vfx_def.sprite_sheet);
        if !full.exists() {
            report.push_error(
                "MISSING_VFX_SPRITE_SHEET",
                format!(
                    "VFX flipbook '{vfx_id}' sprite_sheet '{}' does not exist at {}",
                    vfx_def.sprite_sheet,
                    full.display()
                ),
            );
        }
    }

    // Validate buildings
    for (b_id, b) in &loaded.buildings {
        if !loaded.factions.contains_key(&b.faction) && !b.is_available_to_faction("") {
            report.push_error(
                "UNKNOWN_FACTION",
                format!("Building '{b_id}' references unknown faction '{}'", b.faction),
            );
        }
        if let Some(w_id) = &b.primary_weapon {
            if !loaded.weapons.contains_key(w_id) {
                report.push_error(
                    "UNKNOWN_WEAPON",
                    format!("Building '{b_id}' references unknown weapon '{w_id}'"),
                );
            }
        }
        for prereq in &b.prerequisites {
            if !loaded.buildings.contains_key(prereq) {
                report.push_error(
                    "UNKNOWN_PREREQUISITE",
                    format!("Building '{b_id}' requires unknown building '{prereq}'"),
                );
            }
        }
        if let Some(spawn_unit) = &b.spawns_unit_on_complete {
            if !loaded.units.contains_key(spawn_unit) {
                report.push_error(
                    "UNKNOWN_SPAWN_UNIT",
                    format!(
                        "Building '{b_id}' spawns unknown unit '{spawn_unit}' on completion"
                    ),
                );
            }
        }
        for sp in &b.support_powers {
            if let Some(uid) = &sp.unit_id {
                if !loaded.units.contains_key(uid) {
                    report.push_error(
                        "UNKNOWN_SUPPORT_POWER_UNIT",
                        format!(
                            "Building '{b_id}' support power '{}' references unknown unit '{uid}'",
                            sp.id
                        ),
                    );
                }
            }
        }
        for unlocked_uid in &b.unlocks_units {
            if !loaded.units.contains_key(unlocked_uid) {
                report.push_error(
                    "UNKNOWN_UNLOCK_UNIT",
                    format!(
                        "Building '{b_id}' unlocks_units references unknown unit '{unlocked_uid}'"
                    ),
                );
            }
        }
        for stolen_uid in &b.stolen_tech_units {
            if !loaded.units.contains_key(stolen_uid) {
                report.push_error(
                    "UNKNOWN_STOLEN_TECH_UNIT",
                    format!(
                        "Building '{b_id}' stolen_tech_units references unknown unit '{stolen_uid}'"
                    ),
                );
            }
        }
        if let Some(trans_unit) = &b.transforms_into_unit {
            if !loaded.units.contains_key(trans_unit) {
                report.push_error(
                    "UNKNOWN_TRANSFORM_UNIT",
                    format!(
                        "Building '{b_id}' transforms into unknown unit '{trans_unit}'"
                    ),
                );
            }
        }
        if let Some(up_b) = &b.upgrades_to {
            if !loaded.buildings.contains_key(up_b) {
                report.push_error(
                    "UNKNOWN_UPGRADE_BUILDING",
                    format!("Building '{b_id}' upgrades to unknown building '{up_b}'"),
                );
            }
        }
        if let Some(surv_u) = b.death_payload_spec().survivor_unit.as_ref() {
            if !loaded.units.contains_key(surv_u) {
                report.push_error(
                    "UNKNOWN_SURVIVOR_UNIT",
                    format!("Building '{b_id}' references unknown survivor unit '{surv_u}'"),
                );
            }
        }
        for (event, hook_fn) in &b.script_hooks {
            if !loaded.script_host.has_hook(hook_fn) {
                report.push_error(
                    "UNKNOWN_SCRIPT_HOOK",
                    format!(
                        "Building '{b_id}' hook '{event}' references undefined script function '{hook_fn}'"
                    ),
                );
            }
        }
        let model_path = loaded.resolve_asset_path(&b.visual.model);
        if !model_path.exists() {
            let msg = format!(
                "Building '{b_id}' 3D model '{}' does not exist at {}",
                b.visual.model,
                model_path.display()
            );
            if require_model_files {
                report.push_error("MISSING_MODEL", msg);
            } else {
                report.push_warning("MISSING_MODEL", msg);
            }
        }
    }

    // Detect prerequisite cycles in buildings
    for b_id in loaded.buildings.keys() {
        let mut visited = BTreeSet::new();
        let mut stack = vec![b_id.as_str()];
        while let Some(curr) = stack.pop() {
            if !visited.insert(curr) {
                continue;
            }
            if let Some(def) = loaded.buildings.get(curr) {
                for p in &def.prerequisites {
                    if p == b_id {
                        report.push_error(
                            "TECH_TREE_CYCLE",
                            format!("Cyclic building prerequisite detected involving '{b_id}'"),
                        );
                        break;
                    }
                    stack.push(p.as_str());
                }
            }
        }
    }

    // Validate units
    for (u_id, u) in &loaded.units {
        if !loaded.factions.contains_key(&u.faction) && !u.is_available_to_faction("") {
            report.push_error(
                "UNKNOWN_FACTION",
                format!("Unit '{u_id}' references unknown faction '{}'", u.faction),
            );
        }
        for w_id in [
            &u.primary_weapon,
            &u.secondary_weapon,
            &u.elite_weapon,
            &u.deploy_weapon,
        ]
        .into_iter()
        .flatten()
        {
            if !loaded.weapons.contains_key(w_id) {
                report.push_error(
                    "UNKNOWN_WEAPON",
                    format!("Unit '{u_id}' references unknown weapon '{w_id}'"),
                );
            }
        }
        if let Some(trans_b) = &u.transforms_into_building {
            if !loaded.buildings.contains_key(trans_b) {
                report.push_error(
                    "UNKNOWN_TRANSFORM_BUILDING",
                    format!("Unit '{u_id}' transforms into unknown building '{trans_b}'"),
                );
            }
        }
        if let Some(surv_u) = u.death_payload_spec().survivor_unit.as_ref() {
            if !loaded.units.contains_key(surv_u) {
                report.push_error(
                    "UNKNOWN_SURVIVOR_UNIT",
                    format!("Unit '{u_id}' references unknown survivor unit '{surv_u}'"),
                );
            }
        }
        if let Some(drone_u) = &u.spawns_drone_unit {
            if !loaded.units.contains_key(drone_u) {
                report.push_error(
                    "UNKNOWN_DRONE_UNIT",
                    format!("Unit '{u_id}' references unknown drone unit '{drone_u}'"),
                );
            }
        }
        for prereq in &u.prerequisites {
            if !loaded.buildings.contains_key(prereq) {
                report.push_error(
                    "UNKNOWN_PREREQUISITE",
                    format!("Unit '{u_id}' requires unknown building '{prereq}'"),
                );
            }
        }
        for (event, hook_fn) in &u.script_hooks {
            if !loaded.script_host.has_hook(hook_fn) {
                report.push_error(
                    "UNKNOWN_SCRIPT_HOOK",
                    format!(
                        "Unit '{u_id}' hook '{event}' references undefined script function '{hook_fn}'"
                    ),
                );
            }
        }
        let model_path = loaded.resolve_asset_path(&u.visual.model);
        if !model_path.exists() {
            let msg = format!(
                "Unit '{u_id}' 3D model '{}' does not exist at {}",
                u.visual.model,
                model_path.display()
            );
            if require_model_files {
                report.push_error("MISSING_MODEL", msg);
            } else {
                report.push_warning("MISSING_MODEL", msg);
            }
        }
    }

    // Validate map spawns
    for spawn in &loaded.entry_map.spawns {
        if !loaded.factions.contains_key(&spawn.faction) {
            report.push_error(
                "UNKNOWN_MAP_SPAWN_FACTION",
                format!(
                    "Map '{}' player {} references unknown faction '{}'",
                    loaded.entry_map.id, spawn.player_id, spawn.faction
                ),
            );
        }
        for ib in &spawn.initial_buildings {
            if !loaded.buildings.contains_key(&ib.building_id) {
                report.push_error(
                    "UNKNOWN_MAP_SPAWN_BUILDING",
                    format!(
                        "Map '{}' spawns unknown building '{}'",
                        loaded.entry_map.id, ib.building_id
                    ),
                );
            }
        }
        for iu in &spawn.initial_units {
            if !loaded.units.contains_key(&iu.unit_id) {
                report.push_error(
                    "UNKNOWN_MAP_SPAWN_UNIT",
                    format!(
                        "Map '{}' spawns unknown unit '{}'",
                        loaded.entry_map.id, iu.unit_id
                    ),
                );
            }
        }
    }

    // Validate declarative map triggers
    fn validate_trigger_condition(
        cond: &crate::schema::TriggerCondition,
        map_id: &str,
        tz_id: &str,
        loaded: &LoadedMod,
        report: &mut ValidationReport,
    ) {
        match cond {
            crate::schema::TriggerCondition::BuildingCaptured { building_id, .. }
            | crate::schema::TriggerCondition::BuildingDestroyed { building_id, .. }
                if !loaded.buildings.contains_key(building_id) =>
            {
                report.push_error(
                    "UNKNOWN_TRIGGER_BUILDING",
                    format!(
                        "Map '{map_id}' trigger '{tz_id}' references unknown building_id '{building_id}'"
                    ),
                );
            }
            crate::schema::TriggerCondition::All { conditions }
            | crate::schema::TriggerCondition::Any { conditions } => {
                for sub in conditions {
                    validate_trigger_condition(sub, map_id, tz_id, loaded, report);
                }
            }
            crate::schema::TriggerCondition::Not { condition } => {
                validate_trigger_condition(condition, map_id, tz_id, loaded, report);
            }
            _ => {}
        }
    }

    for map_def in loaded.maps.values() {
        for tz in &map_def.trigger_zones {
            if let Some(cond) = &tz.condition {
                validate_trigger_condition(cond, &map_def.id, &tz.id, loaded, &mut report);
            }
            for act in &tz.actions {
                match act {
                    crate::schema::TriggerAction::SpawnReinforcements { unit_id, .. }
                        if !loaded.units.contains_key(unit_id) =>
                    {
                        report.push_error(
                            "UNKNOWN_TRIGGER_UNIT",
                            format!(
                                "Map '{}' trigger '{}' spawns unknown unit_id '{unit_id}'",
                                map_def.id, tz.id
                            ),
                        );
                    }
                    crate::schema::TriggerAction::SpawnBuilding { building_id, .. }
                        if !loaded.buildings.contains_key(building_id) =>
                    {
                        report.push_error(
                            "UNKNOWN_TRIGGER_BUILDING",
                            format!(
                                "Map '{}' trigger '{}' spawns unknown building_id '{building_id}'",
                                map_def.id, tz.id
                            ),
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    // Validate campaigns
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
        let mission_ids: BTreeSet<&str> =
            campaign.missions.iter().map(|m| m.id.as_str()).collect();
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
                    if !full.exists() {
                        report.push_error(
                            "MISSING_MISSION_MEDIA",
                            format!(
                                "Campaign '{c_id}' mission '{}' {field_name} '{}' does not exist at {}",
                                mission.id,
                                rel_path,
                                full.display()
                            ),
                        );
                    }
                }
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
        if !full.exists() {
            report.push_error(
                "MISSING_AUDIO_ASSET",
                format!(
                    "Weapon SFX '{w_id}' audio file '{}' does not exist at {}",
                    rel_path,
                    full.display()
                ),
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
            if !full.exists() {
                report.push_error(
                    "MISSING_AUDIO_ASSET",
                    format!(
                        "Unit '{u_id}' voice audio file '{}' does not exist at {}",
                        rel_path,
                        full.display()
                    ),
                );
            }
        }
    }
    for track in &loaded.audio.music_tracks {
        let full = loaded.resolve_asset_path(&track.path);
        if !full.exists() {
            report.push_error(
                "MISSING_AUDIO_ASSET",
                format!(
                    "Music track '{}' audio file '{}' does not exist at {}",
                    track.id,
                    track.path,
                    full.display()
                ),
            );
        }
        for f_id in &track.factions {
            if !loaded.factions.contains_key(f_id) {
                report.push_error(
                    "UNKNOWN_MUSIC_FACTION",
                    format!(
                        "Music track '{}' references unknown faction '{f_id}'",
                        track.id
                    ),
                );
            }
        }
    }
    for (cue_id, rel_path) in &loaded.audio.eva_cues {
        let full = loaded.resolve_asset_path(rel_path);
        if !full.exists() {
            report.push_error(
                "MISSING_AUDIO_ASSET",
                format!(
                    "EVA cue '{cue_id}' audio file '{}' does not exist at {}",
                    rel_path,
                    full.display()
                ),
            );
        }
    }

    // Validate AI profiles
    for (p_id, prof) in &loaded.ai_profiles {
        if !loaded.factions.contains_key(&prof.faction) {
            report.push_error(
                "UNKNOWN_AI_PROFILE_FACTION",
                format!(
                    "AI profile '{p_id}' references unknown faction '{}'",
                    prof.faction
                ),
            );
        }
        for b_id in &prof.build_order {
            if !loaded.buildings.contains_key(b_id) {
                report.push_error(
                    "UNKNOWN_AI_BUILD_ORDER_BUILDING",
                    format!(
                        "AI profile '{p_id}' build_order references unknown building '{b_id}'"
                    ),
                );
            }
        }
        for u_id in prof.unit_weights.keys() {
            if !loaded.units.contains_key(u_id) {
                report.push_error(
                    "UNKNOWN_AI_UNIT_WEIGHT",
                    format!(
                        "AI profile '{p_id}' unit_weights references unknown unit '{u_id}'"
                    ),
                );
            }
        }
        if let Some(hook_fn) = &prof.on_ai_tick {
            if !loaded.script_host.has_hook(hook_fn) {
                report.push_error(
                    "UNKNOWN_SCRIPT_HOOK",
                    format!(
                        "AI profile '{p_id}' on_ai_tick references undefined script function '{hook_fn}'"
                    ),
                );
            }
        }
    }

    // Semantic Lint 1: ZERO_EFFECTIVE_DAMAGE on weapons with positive damage but all armor multipliers <= 0
    for (w_id, w) in &loaded.weapons {
        if w.damage > 0.0
            && !w.armor_multipliers.is_empty()
            && w.armor_multipliers.values().all(|&m| m <= 0.0)
        {
            report.push_error(
                "ZERO_EFFECTIVE_DAMAGE",
                format!(
                    "Weapon '{w_id}' has damage {} > 0.0, but all armor_multipliers are <= 0.0",
                    w.damage
                ),
            );
        }
    }

    // Semantic Lint 2: UNREACHABLE_PREREQUISITE (non-stolen-tech unit/building requiring a building from a different faction)
    let all_stolen_tech_units: BTreeSet<&str> = loaded
        .buildings
        .values()
        .flat_map(|b| b.stolen_tech_units.iter().map(|s| s.as_str()))
        .collect();

    for (b_id, b) in &loaded.buildings {
        if b.is_available_to_faction("") {
            continue;
        }
        for prereq in &b.prerequisites {
            if let Some(prereq_b) = loaded.buildings.get(prereq) {
                if !prereq_b.is_available_to_faction("")
                    && !prereq_b.is_available_to_faction(&b.faction)
                {
                    report.push_error(
                        "UNREACHABLE_PREREQUISITE",
                        format!(
                            "Building '{b_id}' (faction '{}') requires unreachable cross-faction prerequisite '{prereq}' (faction '{}')",
                            b.faction, prereq_b.faction
                        ),
                    );
                }
            }
        }
    }
    for (u_id, u) in &loaded.units {
        if u.is_available_to_faction("") || all_stolen_tech_units.contains(u_id.as_str()) {
            continue;
        }
        for prereq in &u.prerequisites {
            if let Some(prereq_b) = loaded.buildings.get(prereq) {
                if !prereq_b.is_available_to_faction("")
                    && !prereq_b.is_available_to_faction(&u.faction)
                {
                    report.push_error(
                        "UNREACHABLE_PREREQUISITE",
                        format!(
                            "Unit '{u_id}' (faction '{}') requires unreachable cross-faction prerequisite '{prereq}' (faction '{}')",
                            u.faction, prereq_b.faction
                        ),
                    );
                }
            }
        }
    }

    // Semantic Lint 3: ORPHANED_WEAPON (warnings for weapons never referenced by any unit or building)
    let mut referenced_weapons: BTreeSet<&str> = BTreeSet::new();
    for u in loaded.units.values() {
        for w in [
            &u.primary_weapon,
            &u.secondary_weapon,
            &u.elite_weapon,
            &u.deploy_weapon,
        ]
        .into_iter()
        .flatten()
        {
            referenced_weapons.insert(w.as_str());
        }
        for w in u.passenger_weapon_by_unit.values() {
            referenced_weapons.insert(w.as_str());
        }
        for w in u.passenger_weapon_by_role.values() {
            referenced_weapons.insert(w.as_str());
        }
    }
    for b in loaded.buildings.values() {
        if let Some(w) = &b.primary_weapon {
            referenced_weapons.insert(w.as_str());
        }
    }
    for (w_id, w) in &loaded.weapons {
        if !referenced_weapons.contains(w_id.as_str()) {
            report.push_warning(
                "ORPHANED_WEAPON",
                format!("Weapon '{w_id}' is not referenced by any unit or building"),
            );
        }
        for se in w.iter_on_hit_statuses() {
            if se.duration_secs <= 0.0 {
                report.push_warning(
                    "ORPHANED_STATUS_EFFECT",
                    format!(
                        "Status effect '{}' on weapon '{w_id}' has zero or negative duration ({})",
                        se.id, se.duration_secs
                    ),
                );
            }
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{ModCatalogPaths, ModManifest};
    use crate::schema::*;
    use crate::scripting::ScriptHost;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn sample_loaded_mod() -> LoadedMod {
        let mut factions = BTreeMap::new();
        factions.insert(
            "allies".to_string(),
            FactionDef {
                id: "allies".to_string(),
                name: "Allies".to_string(),
                color: [0.2, 0.4, 0.9],
                starting_credits: 10000,
                description: String::new(),
                ..Default::default()
            },
        );

        let mut weapons = BTreeMap::new();
        weapons.insert(
            "m60".to_string(),
            WeaponDef {
                id: "m60".to_string(),
                name: "M60".to_string(),
                damage: 15.0,
                range: 10.0,
                cooldown_secs: 1.0,
                projectile_speed: 30.0,
                armor_multipliers: BTreeMap::new(),
                ..Default::default()
            },
        );

        let mut buildings = BTreeMap::new();
        buildings.insert(
            "allied_barracks".to_string(),
            BuildingDef {
                id: "allied_barracks".to_string(),
                name: "Allied Barracks".to_string(),
                faction: "allies".to_string(),
                role: BuildingRole::Barracks,
                cost: 500,
                build_time_secs: 5.0,
                health: 800.0,
                armor: ArmorType::Structure,
                footprint: [2, 2],
                power_delta: -20,
                vision_range: 16.0,
                primary_weapon: None,
                prerequisites: vec![],
                spawns_unit_on_complete: None,
                script_hooks: BTreeMap::new(),
                visual: VisualDef {
                    model: "assets/models/allied_barracks.glb".to_string(),
                    icon: None,
                    archetype: VisualArchetype::Barracks,
                    scale: [2.0, 2.0, 2.0],
                    primary_color: [0.2, 0.4, 0.9],
                    team_color_tint: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut script_hooks = BTreeMap::new();
        script_hooks.insert("on_ability".to_string(), "on_gi_deploy".to_string());

        let mut units = BTreeMap::new();
        units.insert(
            "allied_gi".to_string(),
            UnitDef {
                id: "allied_gi".to_string(),
                name: "GI".to_string(),
                faction: "allies".to_string(),
                role: UnitRole::Infantry,
                cost: 200,
                build_time_secs: 3.0,
                health: 125.0,
                armor: ArmorType::Light,
                speed: 4.0,
                vision_range: 14.0,
                harvest_capacity: 0,
                primary_weapon: Some("m60".to_string()),
                prerequisites: vec!["allied_barracks".to_string()],
                ability_id: Some("deploy".to_string()),
                script_hooks,
                visual: VisualDef {
                    model: "assets/models/allied_gi.glb".to_string(),
                    icon: None,
                    archetype: VisualArchetype::Infantry,
                    scale: [1.0, 1.0, 1.0],
                    primary_color: [0.2, 0.4, 0.9],
                    team_color_tint: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut script_host = ScriptHost::new();
        script_host
            .load_script_file("scripts/rules.rhai", "fn on_gi_deploy(ctx) { ctx }")
            .unwrap();

        LoadedMod {
            root_dir: PathBuf::from("."),
            asset_search_dirs: vec![PathBuf::from(".")],
            manifest: ModManifest {
                id: "test".to_string(),
                name: "Test".to_string(),
                version: "0.1.0".to_string(),
                extends: None,
                authors: vec![],
                description: String::new(),
                engine_version: "0.1.0".to_string(),
                entry_map: "maps/test.toml".to_string(),
                catalogs: ModCatalogPaths {
                    factions: String::new(),
                    weapons: String::new(),
                    buildings: String::new(),
                    units: String::new(),
                },
                scripts: vec!["scripts/rules.rhai".to_string()],
                rules: Default::default(),
            },
            factions,
            weapons,
            buildings,
            units,
            upgrades: BTreeMap::new(),
            campaigns: BTreeMap::new(),
            ai_profiles: BTreeMap::new(),
            audio: AudioCatalog::default(),
            locales: BTreeMap::new(),
            vfx_catalog: BTreeMap::new(),
            entry_map: MapDef {
                id: "test_map".to_string(),
                name: "Test Map".to_string(),
                width: 64,
                height: 64,
                tile_size: 2.0,
                ..Default::default()
            },
            maps: BTreeMap::new(),
            raw_scripts: BTreeMap::new(),
            script_host,
            duplicate_ids: vec![],
        }
    }

    #[test]
    fn validator_accepts_valid_mod_and_catches_broken_references_and_cycles() {
        let mut loaded = sample_loaded_mod();
        let report = validate_mod(&loaded, false);
        assert!(report.is_valid(), "Expected valid mod, got {:?}", report.issues);

        // Introduce broken weapon, cycle, and missing script hook
        let gi = loaded.units.get_mut("allied_gi").unwrap();
        gi.primary_weapon = Some("non_existent_laser".to_string());
        gi.script_hooks
            .insert("on_spawn".to_string(), "missing_hook_fn".to_string());

        let barracks = loaded.buildings.get_mut("allied_barracks").unwrap();
        barracks.prerequisites = vec!["allied_barracks".to_string()];

        let bad_report = validate_mod(&loaded, false);
        assert!(!bad_report.is_valid());
        assert!(bad_report.issues.iter().any(|i| i.code == "UNKNOWN_WEAPON"));
        assert!(bad_report
            .issues
            .iter()
            .any(|i| i.code == "UNKNOWN_SCRIPT_HOOK"));
        assert!(bad_report
            .issues
            .iter()
            .any(|i| i.code == "TECH_TREE_CYCLE"));
    }

    #[test]
    fn semantic_linter_detects_unreachable_prereqs_zero_damage_and_orphaned_weapons() {
        let mut loaded = sample_loaded_mod();
        loaded.factions.insert(
            "soviets".to_string(),
            FactionDef {
                id: "soviets".to_string(),
                name: "Soviets".to_string(),
                color: [0.9, 0.2, 0.2],
                starting_credits: 10000,
                ..Default::default()
            },
        );
        loaded.buildings.insert(
            "soviet_radar".to_string(),
            BuildingDef {
                id: "soviet_radar".to_string(),
                name: "Soviet Radar".to_string(),
                faction: "soviets".to_string(),
                role: BuildingRole::TechCenter,
                cost: 1000,
                build_time_secs: 8.0,
                health: 1000.0,
                armor: ArmorType::Structure,
                footprint: [2, 2],
                power_delta: -50,
                vision_range: 20.0,
                ..Default::default()
            },
        );
        // Allied GI requiring a Soviet Radar without being a stolen-tech unit -> UNREACHABLE_PREREQUISITE
        loaded
            .units
            .get_mut("allied_gi")
            .unwrap()
            .prerequisites
            .push("soviet_radar".to_string());

        // Orphaned weapon with 0.0 armor multipliers despite positive damage -> ZERO_EFFECTIVE_DAMAGE & ORPHANED_WEAPON & ORPHANED_STATUS_EFFECT
        loaded.weapons.insert(
            "broken_peashooter".to_string(),
            WeaponDef {
                id: "broken_peashooter".to_string(),
                name: "Broken Peashooter".to_string(),
                damage: 25.0,
                range: 8.0,
                cooldown_secs: 1.0,
                projectile_speed: 20.0,
                armor_multipliers: BTreeMap::from([(ArmorType::None, 0.0), (ArmorType::Heavy, 0.0)]),
                on_hit_status: Some(StatusEffectDef {
                    id: "noop_debuff".to_string(),
                    duration_secs: 0.0,
                    ..Default::default()
                }),
                ..Default::default()
            },
        );

        let report = validate_mod(&loaded, false);
        assert!(!report.is_valid());
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.code == "UNREACHABLE_PREREQUISITE"),
            "Expected UNREACHABLE_PREREQUISITE in {:?}",
            report.issues
        );
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.code == "ZERO_EFFECTIVE_DAMAGE"),
            "Expected ZERO_EFFECTIVE_DAMAGE in {:?}",
            report.issues
        );
        assert!(
            report.issues.iter().any(|i| i.code == "ORPHANED_WEAPON"),
            "Expected ORPHANED_WEAPON warning in {:?}",
            report.issues
        );
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.code == "ORPHANED_STATUS_EFFECT"),
            "Expected ORPHANED_STATUS_EFFECT warning in {:?}",
            report.issues
        );
    }
}
