// Allied faction script hooks written in Rhai (.rhai)

fn on_gi_deploy(ctx) {
    if ctx.deployed {
        ctx.deployed = false;
        ctx.speed_multiplier = 1.0;
        ctx.armor_multiplier = 1.0;
        ctx.damage_multiplier = 1.0;
        ctx.range_bonus = 0.0;
        ctx.flag_sandbagged = false;
    } else {
        ctx.deployed = true;
        ctx.speed_multiplier = 0.0;
        ctx.armor_multiplier = 1.5;
        ctx.damage_multiplier = 1.35;
        ctx.range_bonus = 3.5;
        ctx.flag_sandbagged = true;
    }
    ctx
}

fn on_grizzly_maneuver(ctx) {
    if ctx.deployed {
        ctx.deployed = false;
        ctx.speed_multiplier = 1.0;
        ctx.armor_multiplier = 1.0;
    } else {
        ctx.deployed = true;
        ctx.speed_multiplier = 1.35;
        ctx.armor_multiplier = 0.85;
    }
    ctx
}

fn on_prism_focus(ctx) {
    if ctx.deployed {
        ctx.deployed = false;
        ctx.speed_multiplier = 1.0;
        ctx.damage_multiplier = 1.0;
        ctx.range_bonus = 0.0;
    } else {
        ctx.deployed = true;
        ctx.speed_multiplier = 0.4;
        ctx.damage_multiplier = 1.55;
        ctx.range_bonus = 5.0;
    }
    ctx
}
