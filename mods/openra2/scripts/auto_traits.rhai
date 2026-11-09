// Auto-generated Rhai trait hooks for converted Westwood / OpenRA mods
fn on_self_heal_tick(ctx) {
    if ctx.health > 0.0 && ctx.health < ctx.max_health {
        let next = ctx.health + 0.35;
        if next > ctx.max_health {
            ctx.health = ctx.max_health;
        } else {
            ctx.health = next;
        }
    }
    ctx
}

fn on_custom_deploy(ctx) {
    ctx.deployed = !ctx.deployed;
    ctx
}
