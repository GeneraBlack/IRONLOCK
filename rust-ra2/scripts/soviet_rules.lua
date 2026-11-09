-- Soviet faction script hooks written in Lua (.lua)

function on_soviet_overcharge(ctx)
    if ctx.deployed then
        ctx.deployed = false
        ctx.damage_multiplier = 1.0
        ctx.armor_multiplier = 1.0
        ctx.speed_multiplier = 1.0
        ctx.flag_overcharged = false
    else
        ctx.deployed = true
        ctx.damage_multiplier = 1.4
        ctx.armor_multiplier = 1.2
        ctx.speed_multiplier = 0.75
        ctx.flag_overcharged = true
    end
    return ctx
end

function on_apocalypse_blitz(ctx)
    if ctx.deployed then
        ctx.deployed = false
        ctx.damage_multiplier = 1.0
        ctx.armor_multiplier = 1.0
        ctx.speed_multiplier = 1.0
        ctx.range_bonus = 0.0
    else
        ctx.deployed = true
        ctx.damage_multiplier = 1.6
        ctx.armor_multiplier = 1.45
        ctx.speed_multiplier = 0.6
        ctx.range_bonus = 2.0
    end
    return ctx
end
