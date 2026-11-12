fn on_squad_take_cover(ctx) {
    if ctx.deployed {
        ctx.deployed = false;
        ctx.armor_multiplier = 1.0;
        ctx.speed_multiplier = 1.0;
        ctx.range_bonus = 0.0;
        ctx.flag_dug_in = false;
    } else {
        ctx.deployed = true;
        ctx.armor_multiplier = 1.45;
        ctx.speed_multiplier = 0.35;
        ctx.range_bonus = 1.5;
        ctx.flag_dug_in = true;
    }
    ctx
}

fn on_tick(ctx) {
    if ctx.tick > 0 && ctx.tick % 200 == 0 {
        let p1_timber = ctx.query_player_resource(1, "timber");
        if p1_timber >= 1000 && ctx.flag_alloy_dividend_paid != true {
            ctx.queue_grant_resource(1, "alloy", 50);
            ctx.flag_alloy_dividend_paid = true;
        }
    }
    ctx
}
