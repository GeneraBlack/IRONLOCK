use super::LoadedMod;
use crate::schema::{EffectPayload, TriggerAction, TriggerCondition};
use anyhow::{bail, Result};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotReloadSummary {
    pub units_count: usize,
    pub buildings_count: usize,
    pub weapons_count: usize,
    pub scripts_count: usize,
    pub content_hash: u64,
}

impl LoadedMod {
    /// Scans all catalogs, rules, campaigns, maps, triggers, and audio bindings in `self` for
    /// references to `target_id`, returning sorted human-readable reference locations so Studio
    /// `Delete` wizards can warn or block breaking deletions.
    pub fn find_id_references(&self, target_id: &str) -> Vec<String> {
        let needle = target_id.trim();
        if needle.is_empty() {
            return Vec::new();
        }
        let mut refs = BTreeSet::new();

        // 1. ModManifest rules & eras
        for (fac, uid) in &self.manifest.rules.free_unit_by_faction {
            if fac == needle || uid == needle {
                refs.insert(format!("mod.rules.free_unit_by_faction[{fac}]"));
            }
        }
        for (fac, uid) in &self.manifest.rules.default_infantry_by_faction {
            if fac == needle || uid == needle {
                refs.insert(format!("mod.rules.default_infantry_by_faction[{fac}]"));
            }
        }
        for (pair, uid) in &self.manifest.rules.stolen_tech_commando_by_pair {
            if uid == needle {
                refs.insert(format!("mod.rules.stolen_tech_commando_by_pair[{pair}]"));
            }
        }
        if self.manifest.rules.default_liberated_worker_unit == needle {
            refs.insert("mod.rules.default_liberated_worker_unit".to_string());
        }
        if self.manifest.rules.default_mutant_unit == needle {
            refs.insert("mod.rules.default_mutant_unit".to_string());
        }
        for era in &self.manifest.rules.eras {
            if era.prerequisite_buildings.iter().any(|b| b == needle)
                || era.prerequisite_upgrades.iter().any(|u| u == needle)
            {
                refs.insert(format!("mod.rules.eras[{}]", era.id));
            }
        }

        // 2. Units
        for u in self.units.values() {
            if u.id == needle {
                continue;
            }
            let pfx = format!("unit:{}", u.id);
            if u.faction == needle || u.sub_factions.iter().any(|s| s == needle) {
                refs.insert(format!("{pfx}.faction"));
            }
            for (slot, w) in [
                ("primary_weapon", &u.primary_weapon),
                ("secondary_weapon", &u.secondary_weapon),
                ("elite_weapon", &u.elite_weapon),
                ("deploy_weapon", &u.deploy_weapon),
            ] {
                if w.as_deref() == Some(needle) {
                    refs.insert(format!("{pfx}.{slot}"));
                }
            }
            if u.prerequisites.iter().any(|p| p == needle)
                || u.prerequisite_upgrades.iter().any(|p| p == needle)
            {
                refs.insert(format!("{pfx}.prerequisites"));
            }
            if u.required_era.as_deref() == Some(needle) {
                refs.insert(format!("{pfx}.required_era"));
            }
            if u.ambient_vfx_id.as_deref() == Some(needle)
                || u.death_vfx_id.as_deref() == Some(needle)
            {
                refs.insert(format!("{pfx}.vfx"));
            }
            if u.transforms_into_building.as_deref() == Some(needle) {
                refs.insert(format!("{pfx}.transforms_into_building"));
            }
            if u.spawns_drone_unit.as_deref() == Some(needle) {
                refs.insert(format!("{pfx}.spawns_drone_unit"));
            }
            if u.requires_powered_building.as_deref() == Some(needle) {
                refs.insert(format!("{pfx}.requires_powered_building"));
            }
            if u.death_effect.survivor_unit.as_deref() == Some(needle) {
                refs.insert(format!("{pfx}.death_effect.survivor_unit"));
            }
            for (p_uid, w_id) in &u.passenger_weapon_by_unit {
                if p_uid == needle || w_id == needle {
                    refs.insert(format!("{pfx}.passenger_weapon_by_unit"));
                }
            }
            if u.passenger_weapon_by_role.values().any(|w| w == needle) {
                refs.insert(format!("{pfx}.passenger_weapon_by_role"));
            }
            if u.resource_costs.contains_key(needle)
                || u.resource_trickle_per_sec.contains_key(needle)
            {
                refs.insert(format!("{pfx}.resources"));
            }
            if let Some(bt) = &u.builder_trait {
                if bt.buildable_buildings.iter().any(|b| b == needle) {
                    refs.insert(format!("{pfx}.builder_trait.buildable_buildings"));
                }
            }
            if let Some(gt) = &u.gatherer_trait {
                if gt.allowed_resources.iter().any(|r| r == needle)
                    || gt.dropoff_buildings.iter().any(|b| b == needle)
                {
                    refs.insert(format!("{pfx}.gatherer_trait"));
                }
            }
            if let Some(st) = &u.squad_trait {
                if st.available_weapon_upgrades.iter().any(|up| up == needle) {
                    refs.insert(format!("{pfx}.squad_trait.available_weapon_upgrades"));
                }
            }
            for ab in &u.abilities {
                if ab.cast_vfx_id.as_deref() == Some(needle)
                    || ab.impact_vfx_id.as_deref() == Some(needle)
                {
                    refs.insert(format!("{pfx}.abilities[{}].vfx", ab.id));
                }
                for pl in &ab.payloads {
                    match pl {
                        EffectPayload::SpawnUnits {
                            unit_id: Some(uid), ..
                        } if uid == needle => {
                            refs.insert(format!("{pfx}.abilities[{}].spawn_units", ab.id));
                        }
                        EffectPayload::MutateOrganicInfantry {
                            into_unit_id: Some(uid),
                            ..
                        } if uid == needle => {
                            refs.insert(format!("{pfx}.abilities[{}].mutate", ab.id));
                        }
                        _ => {}
                    }
                }
            }
        }

        // 3. Buildings
        for b in self.buildings.values() {
            if b.id == needle {
                continue;
            }
            let pfx = format!("building:{}", b.id);
            if b.faction == needle || b.sub_factions.iter().any(|s| s == needle) {
                refs.insert(format!("{pfx}.faction"));
            }
            if b.primary_weapon.as_deref() == Some(needle) {
                refs.insert(format!("{pfx}.primary_weapon"));
            }
            if b.prerequisites.iter().any(|p| p == needle)
                || b.prerequisite_upgrades.iter().any(|p| p == needle)
            {
                refs.insert(format!("{pfx}.prerequisites"));
            }
            if b.required_era.as_deref() == Some(needle) {
                refs.insert(format!("{pfx}.required_era"));
            }
            if b.ambient_vfx_id.as_deref() == Some(needle)
                || b.death_vfx_id.as_deref() == Some(needle)
            {
                refs.insert(format!("{pfx}.vfx"));
            }
            if b.spawns_unit_on_complete.as_deref() == Some(needle) {
                refs.insert(format!("{pfx}.spawns_unit_on_complete"));
            }
            if b.transforms_into_unit.as_deref() == Some(needle) {
                refs.insert(format!("{pfx}.transforms_into_unit"));
            }
            if b.upgrades_to.as_deref() == Some(needle)
                || b.upgrade_options.iter().any(|opt| opt == needle)
            {
                refs.insert(format!("{pfx}.upgrades_to"));
            }
            if b.unlocks_units.iter().any(|u| u == needle)
                || b.stolen_tech_units.iter().any(|u| u == needle)
            {
                refs.insert(format!("{pfx}.unlocks_units"));
            }
            if b.death_effect.survivor_unit.as_deref() == Some(needle) {
                refs.insert(format!("{pfx}.death_effect.survivor_unit"));
            }
            if b.resource_costs.contains_key(needle)
                || b.resource_trickle_per_sec.contains_key(needle)
            {
                refs.insert(format!("{pfx}.resources"));
            }
            for sp in &b.support_powers {
                if sp.id == needle || sp.unit_id.as_deref() == Some(needle) {
                    refs.insert(format!("{pfx}.support_powers[{}]", sp.id));
                }
            }
        }

        // 4. Weapons
        for w in self.weapons.values() {
            if w.id == needle {
                continue;
            }
            if w.muzzle_vfx_id.as_deref() == Some(needle)
                || w.projectile_vfx_id.as_deref() == Some(needle)
                || w.impact_vfx_id.as_deref() == Some(needle)
            {
                refs.insert(format!("weapon:{}.vfx", w.id));
            }
            if w.iter_on_hit_statuses()
                .any(|s| s.on_kill_spawn_unit.as_deref() == Some(needle))
            {
                refs.insert(format!("weapon:{}.on_kill_spawn_unit", w.id));
            }
        }

        // 5. Upgrades & Commander Power Trees
        for upg in self.upgrades.values() {
            if upg.id == needle {
                continue;
            }
            if upg.faction == needle {
                refs.insert(format!("upgrade:{}.faction", upg.id));
            }
            if upg.prerequisites.iter().any(|p| p == needle)
                || upg.prerequisite_upgrades.iter().any(|p| p == needle)
                || upg.target_def_ids.iter().any(|t| t == needle)
                || upg.advances_to_era.as_deref() == Some(needle)
                || upg.required_era.as_deref() == Some(needle)
            {
                refs.insert(format!("upgrade:{}.prerequisites", upg.id));
            }
        }
        for tree in self.commander_trees.values() {
            if tree.id == needle {
                continue;
            }
            if tree.faction == needle {
                refs.insert(format!("commander_tree:{}.faction", tree.id));
            }
            for node in &tree.nodes {
                if node.prerequisites.iter().any(|p| p == needle)
                    || node.grants_support_power.as_deref() == Some(needle)
                    || node.grants_upgrade.as_deref() == Some(needle)
                    || node.unlocks_def_id.as_deref() == Some(needle)
                {
                    refs.insert(format!("commander_tree:{}.nodes[{}]", tree.id, node.id));
                }
            }
        }

        // 6. AI Profiles
        for ai in self.ai_profiles.values() {
            if ai.id == needle {
                continue;
            }
            let pfx = format!("ai_profile:{}", ai.id);
            if ai.faction == needle {
                refs.insert(format!("{pfx}.faction"));
            }
            if ai.build_order.iter().any(|b| b == needle) {
                refs.insert(format!("{pfx}.build_order"));
            }
            if ai.unit_weights.contains_key(needle) {
                refs.insert(format!("{pfx}.unit_weights"));
            }
            if ai.preferred_superweapons.iter().any(|s| s == needle) {
                refs.insert(format!("{pfx}.preferred_superweapons"));
            }
        }

        // 7. Campaigns
        for camp in self.campaigns.values() {
            if camp.id == needle {
                continue;
            }
            if camp.faction == needle {
                refs.insert(format!("campaign:{}.faction", camp.id));
            }
            for m in &camp.missions {
                if m.map_id == needle || m.next_mission_id.as_deref() == Some(needle) {
                    refs.insert(format!("campaign:{}.missions[{}]", camp.id, m.id));
                }
            }
        }

        // 8. Maps & TriggerZones
        fn scan_cond(cond: &TriggerCondition, needle: &str, pfx: &str, out: &mut BTreeSet<String>) {
            match cond {
                TriggerCondition::BuildingCaptured { building_id, .. }
                | TriggerCondition::BuildingDestroyed { building_id, .. }
                    if building_id == needle =>
                {
                    out.insert(format!("{pfx}.condition"));
                }
                TriggerCondition::ObjectiveStatusIs { objective_id, .. }
                    if objective_id == needle =>
                {
                    out.insert(format!("{pfx}.condition"));
                }
                TriggerCondition::All { conditions } | TriggerCondition::Any { conditions } => {
                    for sub in conditions {
                        scan_cond(sub, needle, pfx, out);
                    }
                }
                TriggerCondition::Not { condition } => scan_cond(condition, needle, pfx, out),
                _ => {}
            }
        }

        for map in self.maps.values() {
            if map.id == needle {
                continue;
            }
            let pfx = format!("map:{}", map.id);
            if map
                .resource_patches
                .iter()
                .any(|rp| rp.resource_id == needle)
            {
                refs.insert(format!("{pfx}.resource_patches"));
            }
            if map.item_drops.iter().any(|d| d.item_id == needle) {
                refs.insert(format!("{pfx}.item_drops"));
            }
            for sp in &map.spawns {
                if sp.faction == needle || sp.sub_faction.as_deref() == Some(needle) {
                    refs.insert(format!("{pfx}.spawns[P{}].faction", sp.player_id));
                }
                if sp.initial_buildings.iter().any(|b| b.building_id == needle) {
                    refs.insert(format!("{pfx}.spawns[P{}].initial_buildings", sp.player_id));
                }
                if sp.initial_units.iter().any(|u| u.unit_id == needle) {
                    refs.insert(format!("{pfx}.spawns[P{}].initial_units", sp.player_id));
                }
            }
            if map.preplaced_entities.iter().any(|pe| {
                pe.building_id.as_deref() == Some(needle) || pe.unit_id.as_deref() == Some(needle)
            }) {
                refs.insert(format!("{pfx}.preplaced_entities"));
            }
            for z in &map.trigger_zones {
                let zpfx = format!("{pfx}.trigger_zones[{}]", z.id);
                if let Some(cond) = &z.condition {
                    scan_cond(cond, needle, &zpfx, &mut refs);
                }
                for act in &z.actions {
                    match act {
                        TriggerAction::SpawnReinforcements { unit_id, .. } if unit_id == needle => {
                            refs.insert(format!("{zpfx}.actions"));
                        }
                        TriggerAction::SpawnBuilding { building_id, .. }
                            if building_id == needle =>
                        {
                            refs.insert(format!("{zpfx}.actions"));
                        }
                        TriggerAction::ChangeOwner {
                            building_id: Some(bid),
                            ..
                        } if bid == needle => {
                            refs.insert(format!("{zpfx}.actions"));
                        }
                        TriggerAction::FireSupportPower { power_id, .. } if power_id == needle => {
                            refs.insert(format!("{zpfx}.actions"));
                        }
                        TriggerAction::PlayCutscene { cutscene_id } if cutscene_id == needle => {
                            refs.insert(format!("{zpfx}.actions"));
                        }
                        TriggerAction::SetObjectiveStatus { objective_id, .. }
                        | TriggerAction::AddObjectiveProgress { objective_id, .. }
                            if objective_id == needle =>
                        {
                            refs.insert(format!("{zpfx}.actions"));
                        }
                        _ => {}
                    }
                }
            }
        }

        // 9. Audio catalog
        if self.audio.weapon_sfx.contains_key(needle) {
            refs.insert(format!("audio.weapon_sfx[{needle}]"));
        }
        if self.audio.unit_voices.contains_key(needle) {
            refs.insert(format!("audio.unit_voices[{needle}]"));
        }
        if self.audio.faction_eva_cues.contains_key(needle) {
            refs.insert(format!("audio.faction_eva_cues[{needle}]"));
        }
        for tr in &self.audio.music_tracks {
            if tr.factions.iter().any(|f| f == needle)
                || tr.faction_tags.iter().any(|f| f == needle)
            {
                refs.insert(format!("audio.music_tracks[{}].factions", tr.id));
            }
        }

        // 10. Map prefabs
        if let Ok(prefabs) = self.load_prefab_defs() {
            for pf in prefabs.values() {
                if pf.entities.iter().any(|pe| {
                    pe.building_id.as_deref() == Some(needle)
                        || pe.unit_id.as_deref() == Some(needle)
                        || pe.tag.as_deref() == Some(needle)
                }) {
                    refs.insert(format!("prefab:{}.entities", pf.id));
                }
            }
        }

        refs.into_iter().collect()
    }

    /// Transactionally reloads all TOML catalogs, `[rules]`, and `.rhai`/`.lua` scripts from disk
    /// (or `.rtsmod` archive). If parsing or `validate_mod` fails, `self` remains untouched.
    pub fn reload_from_disk(&mut self) -> Result<HotReloadSummary> {
        let fresh = if self.vfs_layers.len() > 1 {
            let ordered_paths: Vec<&std::path::Path> = self
                .vfs_layers
                .iter()
                .rev()
                .map(|layer| layer.root_path())
                .collect();
            Self::load_mod_stack(&ordered_paths)?
        } else {
            Self::load_from_path(&self.root_dir)?
        };
        let report = crate::validator::validate_mod(&fresh, false);
        if !report.is_valid() {
            let errs: Vec<String> = report
                .issues
                .iter()
                .filter(|i| i.severity == crate::validator::Severity::Error)
                .map(|i| format!("[{}] {}", i.code, i.message))
                .collect();
            bail!(
                "Hot-reload validation failed ({} error(s)): {}",
                errs.len(),
                errs.join("; ")
            );
        }
        let summary = HotReloadSummary {
            units_count: fresh.units.len(),
            buildings_count: fresh.buildings.len(),
            weapons_count: fresh.weapons.len(),
            scripts_count: fresh.raw_scripts.len(),
            content_hash: fresh.content_hash(),
        };
        *self = fresh;
        Ok(summary)
    }
}
