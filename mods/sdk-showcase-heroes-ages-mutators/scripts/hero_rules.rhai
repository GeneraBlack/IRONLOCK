// ==============================================================================
// Deterministic Rhai Hero & Relic Rules (`scripts/hero_rules.rhai`)
// Uses Lazy-Evaluated ScriptContext 2.0 (`apply_buff`, `deal_area_damage`)
// ==============================================================================

fn on_archon_hit(ctx) {
    if ctx.entity_id > 0 {
        ctx.apply_buff(ctx.entity_id, "archon_battle_fervor", 3.0, 1);
    }
    ctx
}

fn on_solar_nova_cast(ctx) {
    let tx = if ctx.has_target_pos { ctx.target_x } else { ctx.pos_x };
    let ty = if ctx.has_target_pos { ctx.target_y } else { ctx.pos_y };
    let tz = if ctx.has_target_pos { ctx.target_z } else { ctx.pos_z };

    ctx.deal_area_damage(tx, ty, tz, 4.5, 45.0, false);
    if ctx.entity_id > 0 {
        ctx.apply_buff(ctx.entity_id, "solar_radiance_empower", 5.0, 1);
    }
    ctx
}

fn on_aegis_bulwark_cast(ctx) {
    if ctx.entity_id > 0 {
        ctx.armor_multiplier = 1.35;
        ctx.apply_buff(ctx.entity_id, "aegis_ward_aura", 6.0, 1);
    }
    ctx
}

fn on_astral_blink_cast(ctx) {
    let tx = if ctx.has_target_pos { ctx.target_x } else { ctx.pos_x };
    let ty = if ctx.has_target_pos { ctx.target_y } else { ctx.pos_y };
    let tz = if ctx.has_target_pos { ctx.target_z } else { ctx.pos_z };

    ctx.deal_area_damage(tx, ty, tz, 3.2, 35.0, false);
    if ctx.entity_id > 0 {
        ctx.speed_multiplier = 1.20;
        ctx.apply_buff(ctx.entity_id, "astral_phase_haste", 4.0, 1);
    }
    ctx
}
