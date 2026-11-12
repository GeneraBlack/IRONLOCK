function on_supply_convoy_zone(ctx)
    local p1_nearby = ctx:count_entities_in_radius(31, 31, 6, 1)
    local current_timber = ctx:query_player_resource(1, "timber")
    if p1_nearby > 0 then
        ctx:queue_grant_resource(1, "timber", 250)
        ctx:queue_grant_resource(1, "alloy", 150)
        if current_timber >= 500 then
            ctx:queue_set_objective("Hold Timberline Plateau and reinforce Rifle Squads")
        end
    end
end
