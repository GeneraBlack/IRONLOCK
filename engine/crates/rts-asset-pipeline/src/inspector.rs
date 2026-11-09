use rts_sdk::loader::LoadedMod;

pub fn format_mod_inspection(loaded: &LoadedMod) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "=== Mod Inspection: {} ({}) v{} ===\n",
        loaded.manifest.name, loaded.manifest.id, loaded.manifest.version
    ));
    out.push_str(&format!(
        "Entry Map: {} ({}x{}, tile_size={})\n",
        loaded.entry_map.name,
        loaded.entry_map.width,
        loaded.entry_map.height,
        loaded.entry_map.tile_size
    ));
    out.push_str(&format!(
        "Factions ({}):\n",
        loaded.factions.len()
    ));
    for f in loaded.factions.values() {
        out.push_str(&format!(
            "  - [{}] {} (Start Credits: {})\n",
            f.id, f.name, f.starting_credits
        ));
    }

    out.push_str(&format!("\nBuildings ({}):\n", loaded.buildings.len()));
    for b in loaded.buildings.values() {
        let prereqs = if b.prerequisites.is_empty() {
            "None".to_string()
        } else {
            b.prerequisites.join(", ")
        };
        out.push_str(&format!(
            "  - [{}] {} (Faction: {}, Cost: {}, HP: {:.0}, Power: {:+}, Prereqs: {})\n",
            b.id, b.name, b.faction, b.cost, b.health, b.power_delta, prereqs
        ));
    }

    out.push_str(&format!("\nUnits ({}):\n", loaded.units.len()));
    for u in loaded.units.values() {
        let dps = u
            .primary_weapon
            .as_ref()
            .and_then(|w_id| loaded.weapons.get(w_id))
            .map(|w| w.damage / w.cooldown_secs.max(0.001))
            .unwrap_or(0.0);
        let prereqs = if u.prerequisites.is_empty() {
            "None".to_string()
        } else {
            u.prerequisites.join(", ")
        };
        out.push_str(&format!(
            "  - [{}] {} (Faction: {}, Cost: {}, HP: {:.0}, Speed: {:.1}, DPS: {:.1}, Prereqs: {})\n",
            u.id, u.name, u.faction, u.cost, u.health, u.speed, dps, prereqs
        ));
    }

    let hooks = loaded.script_host.registered_hooks();
    out.push_str(&format!("\nRegistered Script Hooks ({}):\n", hooks.len()));
    for (hook_name, lang) in hooks {
        out.push_str(&format!("  - {hook_name} ({lang:?})\n"));
    }

    out
}
