use super::load_ra2_mod;
use crate::command::GameCommand;
use crate::world::{EntityKind, SimWorld};

#[test]
fn diplomacy_tribute_landmines_weather_and_advanced_ai_work() {
    use crate::ai::{generate_ai_commands_with_profile, AiDifficulty, AiPersonality};
    use crate::world::EnvironmentPreset;

    let loaded = load_ra2_mod();
    let river_map = loaded
        .maps
        .get("river_crossing")
        .expect("river_crossing map should load");
    let mut world = SimWorld::from_mod_map(&loaded, river_map);

    // 1. Dynamic Alliance (`SetAlliance`), Tribute (`TransferCredits`) & Unit Gifting (`TransferUnits`)
    assert!(world.are_hostile(1, 2));
    world.push_command(GameCommand::SetAlliance {
        player_id: 1,
        target_player_id: 2,
        allied: true,
    });
    let p2_credits_before = world.players.get(&2).unwrap().credits;
    world.push_command(GameCommand::TransferCredits {
        from_player_id: 1,
        to_player_id: 2,
        amount: 500,
    });
    let gift_unit_id = world
        .entities
        .values()
        .find(|e| e.player_id == 1 && matches!(e.kind, EntityKind::Unit { .. }))
        .unwrap()
        .id;
    world.push_command(GameCommand::TransferUnits {
        from_player_id: 1,
        to_player_id: 2,
        unit_ids: vec![gift_unit_id],
    });
    world.step(&loaded);
    assert!(
        !world.are_hostile(1, 2),
        "SetAlliance(true) should make Player 1 and Player 2 non-hostile allies"
    );
    assert_eq!(
        world.players.get(&2).unwrap().credits,
        p2_credits_before + 500,
        "TransferCredits should transfer 500 credits to Player 2"
    );
    assert_eq!(
        world.entities.get(&gift_unit_id).unwrap().player_id,
        2,
        "TransferUnits should transfer ownership of unit to Player 2"
    );

    // Break alliance back to hostile
    world.push_command(GameCommand::SetAlliance {
        player_id: 1,
        target_player_id: 2,
        allied: false,
    });
    world.step(&loaded);
    assert!(world.are_hostile(1, 2));

    // 2. Landmine placement (`PlaceMine`), detonation on enemy contact & Engineer disarming
    let eng_id = world
        .spawn_unit(1, "allied_engineer", [16, 16], &loaded)
        .unwrap();
    world.push_command(GameCommand::PlaceMine {
        player_id: 1,
        unit_ids: vec![eng_id],
        target_tile: [18, 16],
    });
    world.step(&loaded);
    assert_eq!(
        world.mines.len(),
        1,
        "Engineer should place a tactical landmine on [18, 16]"
    );

    // Spawn a hostile tank onto [18, 16] -> mine should detonate and damage the tank
    let enemy_tank = world
        .spawn_unit(2, "rhino_tank", [18, 16], &loaded)
        .unwrap();
    let max_hp = world.entities.get(&enemy_tank).unwrap().max_health;
    world.step(&loaded);
    assert!(
        world.mines.is_empty(),
        "Mine should detonate and be removed when a hostile ground unit steps on it"
    );
    assert!(
        world.entities.get(&enemy_tank).unwrap().health < max_hp,
        "Mine detonation should deal splash damage to the triggering enemy tank"
    );

    // Place another mine and let an enemy Engineer disarm it safely
    world.spawn_mine(1, [20, 16], 300.0, 4.0);
    let _enemy_eng = world
        .spawn_unit(2, "soviet_engineer", [19, 16], &loaded)
        .unwrap();
    world.step(&loaded);
    assert!(
        world.mines.is_empty(),
        "Hostile Engineer within 2.5 units should safely disarm the landmine"
    );

    // 3. Weather Gameplay Modifiers (`Night` vision reduction)
    let sample_ent = world.entities.get(&enemy_tank).unwrap().clone();
    world.environment_preset = EnvironmentPreset::Day;
    let vis_day = world.entity_vision_range(&sample_ent, &loaded);
    world.environment_preset = EnvironmentPreset::Night;
    let vis_night = world.entity_vision_range(&sample_ent, &loaded);
    assert!(
        (vis_night - vis_day * 0.75).abs() < 1e-3,
        "Night environment preset should reduce entity vision range by 25%"
    );

    // 4. Advanced AI on `river_crossing` (Naval Shipyard & Engineer neutral capture)
    world.tick = 20;
    let ai_cmds = generate_ai_commands_with_profile(
        &world,
        2,
        &loaded,
        AiDifficulty::Hard,
        AiPersonality::Balanced,
    );
    assert!(
        !ai_cmds.is_empty(),
        "AI on river_crossing should generate commands (Engineer capture / base build)"
    );
}

#[test]
fn vehicle_transports_parasites_service_depot_props_and_declarative_triggers_work() {
    use rts_sdk::schema::PropKind;

    let loaded = load_ra2_mod();
    let river_map = loaded
        .maps
        .get("river_crossing")
        .expect("river_crossing map should load");
    let mut world = SimWorld::from_mod_map(&loaded, river_map);

    // 1. Vehicle Transport (`can_carry_vehicles = true` on `nighthawk_chopper`)
    let chopper_id = world
        .spawn_unit(1, "nighthawk_chopper", [14, 14], &loaded)
        .unwrap();
    let tank_id = world
        .spawn_unit(1, "grizzly_tank", [15, 14], &loaded)
        .unwrap();
    world.push_command(GameCommand::LoadIntoCarrier {
        player_id: 1,
        passenger_ids: vec![tank_id],
        carrier_id: chopper_id,
    });
    for _ in 0..8 {
        world.step(&loaded);
    }
    assert_eq!(
        world.entities.get(&tank_id).unwrap().transported_by,
        Some(chopper_id),
        "Nighthawk Chopper (`can_carry_vehicles = true`) should load a Grizzly Tank"
    );
    world.push_command(GameCommand::UnloadCarrier {
        player_id: 1,
        carrier_ids: vec![chopper_id],
    });
    world.step(&loaded);
    assert!(
        world
            .entities
            .get(&tank_id)
            .unwrap()
            .transported_by
            .is_none(),
        "UnloadCarrier should deploy the transported Grizzly Tank back onto the battlefield"
    );

    // 2. Parasite infection (`terror_drone` -> `parasite_dps = 38.0`) & Service Depot curing (`DockAtServiceDepot`)
    let drone_id = world
        .spawn_unit(2, "terror_drone", [16, 14], &loaded)
        .unwrap();
    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![drone_id],
        target_entity_id: tank_id,
    });
    for _ in 0..10 {
        world.step(&loaded);
    }
    assert!(
        !world.entities.contains_key(&drone_id),
        "Terror Drone should be consumed after latching inside the enemy tank"
    );
    let infected_dps = world.entities.get(&tank_id).unwrap().parasite_dps();
    assert!(
        infected_dps > 0.0,
        "Target Grizzly Tank should have active parasite infection (`parasite_dps() > 0.0`)"
    );

    // Spawn a friendly Service Depot right next to the infected tank and dock at it
    let depot_id = world
        .spawn_building(1, "allied_service_depot", [13, 12], &loaded, false)
        .unwrap();
    world.push_command(GameCommand::DockAtServiceDepot {
        player_id: 1,
        unit_ids: vec![tank_id],
        depot_entity_id: depot_id,
    });
    for _ in 0..15 {
        world.step(&loaded);
    }
    let tank_after = world.entities.get(&tank_id).unwrap();
    assert_eq!(
        tank_after.parasite_dps(),
        0.0,
        "Friendly Service Depot should cure the parasite infection on the docked tank"
    );

    // 3. Chain-reaction ExplosiveBarrel props (`PropKind::ExplosiveBarrel`)
    let props_before = world.props.len();
    assert!(
            props_before >= 4,
            "river_crossing should spawn destructible 3D map props (Trees, Streetlights, ExplosiveBarrels)"
        );
    // Trigger the explosive barrel at [27, 29] (which should chain-detonate [28, 29])
    if let Some(barrel) = world
        .props
        .iter_mut()
        .find(|p| p.kind == PropKind::ExplosiveBarrel)
    {
        barrel.health = 0.0;
    }
    world.step(&loaded);
    assert!(
            world.props.len() <= props_before - 2,
            "Detonating one ExplosiveBarrel at [27, 29] should chain-detonate the adjacent barrel at [28, 29]"
        );

    // 4. Declarative Trigger (`BuildingCaptured` -> `GrantCredits`, `SpawnReinforcements`, `SetObjective`)
    let neutral_plant_id = world
            .entities
            .values()
            .find(|e| {
                e.player_id == 0
                    && matches!(&e.kind, EntityKind::Building { building_id } if building_id == "soviet_industrial_plant")
            })
            .unwrap()
            .id;
    let credits_before = world.players.get(&1).unwrap().credits;
    world.entities.get_mut(&neutral_plant_id).unwrap().player_id = 1;
    world.step(&loaded);
    assert!(
            world.fired_triggers.contains("industrial_plant_captured_bonus"),
            "Capturing neutral_industrial_plant should fire declarative trigger `industrial_plant_captured_bonus`"
        );
    assert!(
        world.players.get(&1).unwrap().credits >= credits_before + 1000,
        "Declarative trigger should execute `GrantCredits` (+1000)"
    );
}

#[test]
fn support_powers_airfield_landing_rotated_buildings_and_smart_harvesters_work() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.players.get_mut(&1).unwrap().credits = 20000;

    // 1. Place an Airforce Command + Chronosphere for Player 1
    let af_id = world
        .spawn_building(1, "allied_airforce_command", [15, 10], &loaded, false)
        .unwrap();
    world
        .entities
        .get_mut(&af_id)
        .unwrap()
        .construction_progress = 1.0;

    let chrono_id = world
        .spawn_building(1, "allied_chronosphere", [20, 10], &loaded, false)
        .unwrap();
    world
        .entities
        .get_mut(&chrono_id)
        .unwrap()
        .construction_progress = 1.0;

    // 2. Test rotated building placement (`allied_refinery` is [3, 2] -> rotated [2, 3])
    world.push_command(GameCommand::PlaceBuildingRotated {
        player_id: 1,
        building_id: "allied_refinery".to_string(),
        tile_pos: [10, 15],
        rotated: true,
    });
    world.step(&loaded);
    let ref_ent = world
            .entities
            .values()
            .find(|e| {
                e.tile_origin == [10, 15]
                    && matches!(&e.kind, EntityKind::Building { building_id } if building_id == "allied_refinery")
            })
            .expect("Rotated refinery should spawn");
    assert_eq!(ref_ent.custom_footprint, Some([2, 3]));
    assert!(world.grid.blocked_tiles.contains(&[11, 17]));
    let ref_id = ref_ent.id;
    world
        .entities
        .get_mut(&ref_id)
        .unwrap()
        .construction_progress = 1.0;

    // 3. Test Support Power: `allied_airborne_paradrop` (`Paradrop`) & `allied_spy_plane_scan` (`RadarScan`)
    let units_before = world
        .entities
        .values()
        .filter(|e| !e.is_support_aircraft)
        .count();
    world.push_command(GameCommand::CastSupportPower {
        player_id: 1,
        power_id: "allied_airborne_paradrop".to_string(),
        source_pos: None,
        target_pos: [30.0, 0.0, 30.0],
    });
    world.step(&loaded);
    while world.active_support_aircraft.iter().any(|ac| !ac.dropped) {
        world.step(&loaded);
    }
    assert_eq!(
        world
            .entities
            .values()
            .filter(|e| !e.is_support_aircraft)
            .count(),
        units_before + 5,
        "Allied Airborne Paradrop should spawn 5 GI paratroopers"
    );
    let paratrooper = world
        .entities
        .values()
        .find(|e| e.parachute_timer > 0.0)
        .expect("Spawned paratroopers should have active parachute_timer > 0");
    assert!(paratrooper.position[1] > 4.0);

    world.push_command(GameCommand::CastSupportPower {
        player_id: 1,
        power_id: "allied_spy_plane_scan".to_string(),
        source_pos: None,
        target_pos: [45.0, 0.0, 45.0],
    });
    world.step(&loaded);
    assert_eq!(world.temporary_reveal_zones.len(), 1);
    let vis = world.compute_visibility_map(1, &loaded);
    let scan_tile = world.grid.world_to_tile([45.0, 0.0, 45.0]);
    let idx = (scan_tile[1] * world.grid.width + scan_tile[0]) as usize;
    assert_eq!(
        vis[idx],
        crate::world::TileVisibility::Visible,
        "Spy Plane RadarScan should reveal target area in Fog of War"
    );

    // 4. Test Support Power: `chronosphere_shift` (`ChronoShift`)
    let tank_id = world
        .spawn_unit(1, "grizzly_tank", [12, 12], &loaded)
        .unwrap();
    let src_pos = world.entities.get(&tank_id).unwrap().position;
    let dst_pos = world.grid.tile_to_world([14, 14]);
    world.push_command(GameCommand::CastSupportPower {
        player_id: 1,
        power_id: "chronosphere_shift".to_string(),
        source_pos: Some(src_pos),
        target_pos: dst_pos,
    });
    world.step(&loaded);
    let shifted_tank = world.entities.get(&tank_id).unwrap();
    assert!(
        (shifted_tank.position[0] - dst_pos[0]).abs() <= 4.0
            && (shifted_tank.position[2] - dst_pos[2]).abs() <= 4.0,
        "Chronosphere Shift should teleport vehicle to target coordinates"
    );

    // 5. Test Aircraft ReturnToAirfield & physical landing (`landed_at_airfield`)
    let jet_id = world
        .spawn_unit(1, "nighthawk_chopper", [15, 10], &loaded)
        .unwrap();
    world.push_command(GameCommand::ReturnToAirfield {
        player_id: 1,
        unit_ids: vec![jet_id],
        airfield_entity_id: Some(af_id),
    });
    for _ in 0..30 {
        world.step(&loaded);
    }
    let jet = world.entities.get(&jet_id).unwrap();
    assert!(
        jet.landed_at_airfield,
        "Aircraft at airfield with empty waypoints should physically land on the airfield pad"
    );
    assert_eq!(world.home_airfield_id(jet_id), Some(af_id));
}

#[test]
fn spy_infiltration_c4_demolition_directional_armor_reverse_move_and_emp_work() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.players.get_mut(&1).unwrap().credits = 10_000;
    world.players.get_mut(&2).unwrap().credits = 4_000;

    // 1. Test Spy Infiltration: Refinery theft, PowerPlant sabotage, and WarFactory veterancy unlock
    let enemy_ref = world
        .spawn_building(2, "soviet_refinery", [18, 12], &loaded, false)
        .unwrap();
    let spy_1 = world
        .spawn_unit(1, "allied_spy", [18, 14], &loaded)
        .unwrap();
    let credits_before = world.players.get(&1).unwrap().credits;
    world.push_command(GameCommand::InfiltrateBuilding {
        player_id: 1,
        spy_ids: vec![spy_1],
        target_building_id: enemy_ref,
    });
    for _ in 0..10 {
        world.step(&loaded);
    }
    assert!(
        world.players.get(&1).unwrap().credits >= credits_before + 2_000,
        "Spy infiltrating enemy Refinery should steal 50% of enemy credits"
    );
    assert!(
        !world.entities.contains_key(&spy_1),
        "Spy should be consumed upon infiltrating building"
    );

    let enemy_pp = world
        .spawn_building(2, "soviet_tesla_reactor", [22, 12], &loaded, false)
        .unwrap();
    let spy_2 = world
        .spawn_unit(1, "allied_spy", [22, 14], &loaded)
        .unwrap();
    world.push_command(GameCommand::InfiltrateBuilding {
        player_id: 1,
        spy_ids: vec![spy_2],
        target_building_id: enemy_pp,
    });
    for _ in 0..10 {
        world.step(&loaded);
    }
    assert!(
        world.players.get(&2).unwrap().power_sabotage_timer > 20.0
            && world.players.get(&2).unwrap().is_low_power(),
        "Spy infiltrating enemy PowerPlant should trigger power_sabotage_timer and low-power state"
    );

    let enemy_wf = world
        .spawn_building(2, "soviet_war_factory", [26, 12], &loaded, false)
        .unwrap();
    let spy_3 = world
        .spawn_unit(1, "allied_spy", [26, 15], &loaded)
        .unwrap();
    world.push_command(GameCommand::InfiltrateBuilding {
        player_id: 1,
        spy_ids: vec![spy_3],
        target_building_id: enemy_wf,
    });
    for _ in 0..10 {
        world.step(&loaded);
    }
    assert!(
        world.players.get(&1).unwrap().veteran_production_unlocked,
        "Spy infiltrating enemy War Factory should unlock veteran unit production"
    );
    let vet_tank = world
        .spawn_unit(1, "grizzly_tank", [10, 10], &loaded)
        .unwrap();
    assert_eq!(
        world.entities.get(&vet_tank).unwrap().veterancy,
        crate::world::VeterancyRank::Veteran,
        "Newly spawned units after veteran_production_unlocked should start as Veteran"
    );

    // 2. Test Timed C4 Demolition & Engineer C4 Defusal
    let target_b = world
        .spawn_building(2, "soviet_barracks", [14, 18], &loaded, false)
        .unwrap();
    let seal_id = world.spawn_unit(1, "navy_seal", [14, 20], &loaded).unwrap();
    world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![seal_id],
        target_entity_id: target_b,
    });
    world.step(&loaded);
    assert!(
        world.entities.get(&target_b).unwrap().c4_remaining() > 3.0,
        "Navy SEAL with plants_c4 / c4_demolition should attach a 3.5s C4 timer to enemy building"
    );
    // Stop SEAL so C4 timer alone finishes the building, or test Engineer defusal first
    world.push_command(GameCommand::StopUnits {
        player_id: 1,
        entity_ids: vec![seal_id],
    });
    let sov_eng = world
        .spawn_unit(2, "soviet_engineer", [14, 17], &loaded)
        .unwrap();
    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![sov_eng],
        target_entity_id: target_b,
    });
    for _ in 0..5 {
        world.step(&loaded);
    }
    assert_eq!(
        world.entities.get(&target_b).unwrap().c4_remaining(),
        0.0,
        "Friendly Engineer should defuse active C4 charge on building"
    );

    // Re-plant C4 and let it detonate after 3.5s (75 ticks at dt=0.05)
    world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![seal_id],
        target_entity_id: target_b,
    });
    world.step(&loaded);
    world.push_command(GameCommand::StopUnits {
        player_id: 1,
        entity_ids: vec![seal_id],
    });
    for _ in 0..75 {
        world.step(&loaded);
    }
    assert!(
        !world.entities.contains_key(&target_b),
        "Building should be demolished when C4 countdown expires"
    );

    // 3. Test Directional Vehicle Armor (Front = 1.0x, Flank = 1.15x, Rear = 1.35x) & ReverseMove
    let armor_tank_id = world.spawn_unit(2, "rhino_tank", [2, 20], &loaded).unwrap();
    world
        .entities
        .get_mut(&armor_tank_id)
        .unwrap()
        .facing_radians = 0.0; // Facing +Z ([0, 0, +1])
    let armor_tank = world.entities.get(&armor_tank_id).unwrap();
    let t_pos = armor_tank.position;
    let front_mult = crate::combat::directional_armor_damage_multiplier(
        [t_pos[0], t_pos[1], t_pos[2] + 10.0],
        armor_tank,
        &loaded,
    );
    let flank_mult = crate::combat::directional_armor_damage_multiplier(
        [t_pos[0] + 10.0, t_pos[1], t_pos[2]],
        armor_tank,
        &loaded,
    );
    let rear_mult = crate::combat::directional_armor_damage_multiplier(
        [t_pos[0], t_pos[1], t_pos[2] - 10.0],
        armor_tank,
        &loaded,
    );
    assert!((front_mult - 1.0).abs() < 1e-4);
    assert!((flank_mult - 1.15).abs() < 1e-4);
    assert!((rear_mult - 1.35).abs() < 1e-4);

    // ReverseMove moves backward along -Z while keeping facing_radians pointed toward +Z (0.0)
    let rev_target = [t_pos[0], t_pos[1], t_pos[2] - 6.0];
    world.push_command(GameCommand::ReverseMove {
        player_id: 2,
        entity_ids: vec![armor_tank_id],
        target_pos: rev_target,
    });
    for _ in 0..3 {
        world.step(&loaded);
    }
    let moved_tank = world.entities.get(&armor_tank_id).unwrap();
    assert!(
        moved_tank.position[2] < t_pos[2] - 0.5,
        "ReverseMove should move tank backward toward target_pos"
    );
    assert!(
        moved_tank.facing_radians.cos() > 0.8,
        "ReverseMove should keep front armor facing opposite to movement direction (+Z)"
    );

    // 4. Test EMP Shockwave (`SupportPowerKind::EmpPulse` & aircraft crash)
    let lab_id = world
        .spawn_building(2, "soviet_battle_lab", [25, 25], &loaded, false)
        .unwrap();
    world
        .entities
        .get_mut(&lab_id)
        .unwrap()
        .construction_progress = 1.0;
    let emp_center = world.grid.tile_to_world([12, 22]);
    let victim_tank = world
        .spawn_unit(1, "grizzly_tank", [12, 22], &loaded)
        .unwrap();
    let victim_air = world.spawn_unit(1, "harrier", [12, 22], &loaded).unwrap();
    world.push_command(GameCommand::CastSupportPower {
        player_id: 2,
        power_id: "soviet_emp_pulse".to_string(),
        source_pos: None,
        target_pos: emp_center,
    });
    world.step(&loaded);
    assert!(
        world.entities.get(&victim_tank).unwrap().emp_remaining() > 5.0,
        "EMP Pulse should disable ground vehicles with emp_disable_timer > 0"
    );
    assert!(
        !world.entities.contains_key(&victim_air),
        "EMP Pulse should stall and crash airborne aircraft"
    );

    // 5. Test Selectable Formations (`Line`, `Wedge`, `Column`) & Live Army Value / APM
    world.push_command(GameCommand::SetFormation {
        player_id: 1,
        formation: crate::command::FormationKind::Wedge,
    });
    world.step(&loaded);
    assert_eq!(
        world.players.get(&1).unwrap().formation,
        crate::command::FormationKind::Wedge
    );
    assert!(world.player_army_value(1, &loaded) > 0);
    assert!(world.player_apm(1) > 0);
}

#[test]
fn sub_factions_build_limits_gem_harvesting_and_neutral_garrisons_work() {
    let mod_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("rust-ra2");
    let loaded = rts_sdk::loader::LoadedMod::load_from_dir(&mod_dir).expect("load rust-ra2");
    let mut world = SimWorld::from_mod(&loaded);
    world.players.get_mut(&1).unwrap().credits = 50_000;
    world.players.get_mut(&2).unwrap().credits = 50_000;

    // 1. Test Country Sub-Factions (France -> Grand Cannon, Great Britain -> Sniper, Cuba -> Terrorist, USA -> Airborne Paradrop)
    let afc_id = world
        .spawn_building(1, "allied_airforce_command", [4, 24], &loaded, false)
        .unwrap();
    world
        .entities
        .get_mut(&afc_id)
        .unwrap()
        .construction_progress = 1.0;

    // When sub_faction is set to "france", France gets allied_grand_cannon, NOT sniper or allied_airborne_paradrop
    world.set_player_sub_faction(1, Some("france".to_string()));
    let avail_b_fr = world.available_buildings_for_player(1, &loaded);
    let avail_u_fr = world.available_units_for_player(1, &loaded);
    let avail_sp_fr: Vec<_> = world
        .available_support_powers_for_player(1, &loaded)
        .into_iter()
        .map(|(sp, _)| sp.id)
        .collect();
    assert!(avail_b_fr.iter().any(|b| b == "allied_grand_cannon"));
    assert!(!avail_u_fr.iter().any(|u| u == "sniper"));
    assert!(!avail_sp_fr
        .iter()
        .any(|sp| sp == "allied_airborne_paradrop"));

    // When sub_faction is set to "usa", USA gets allied_airborne_paradrop, NOT allied_grand_cannon
    world.set_player_sub_faction(1, Some("usa".to_string()));
    let avail_sp_usa: Vec<_> = world
        .available_support_powers_for_player(1, &loaded)
        .into_iter()
        .map(|(sp, _)| sp.id)
        .collect();
    assert!(avail_sp_usa
        .iter()
        .any(|sp| sp == "allied_airborne_paradrop"));

    // Cuba gets Cuban Terrorist (`terrorist`)
    let sov_radar = world
        .spawn_building(2, "soviet_radar_tower", [36, 18], &loaded, false)
        .unwrap();
    world
        .entities
        .get_mut(&sov_radar)
        .unwrap()
        .construction_progress = 1.0;
    world.set_player_sub_faction(2, Some("cuba".to_string()));
    let avail_u_cuba = world.available_units_for_player(2, &loaded);
    assert!(avail_u_cuba.iter().any(|u| u == "terrorist"));
    assert!(!avail_u_cuba.iter().any(|u| u == "desolator"));

    // Clear sub_faction back to None so all tech is unlocked for subsequent tests
    world.set_player_sub_faction(1, None);
    world.set_player_sub_faction(2, None);

    // 2. Test Hero & Epic Structure Build Limits (`build_limit = 1`)
    let blab_id = world
        .spawn_building(1, "allied_battle_lab", [4, 28], &loaded, false)
        .unwrap();
    world
        .entities
        .get_mut(&blab_id)
        .unwrap()
        .construction_progress = 1.0;
    let _tanya_1 = world.spawn_unit(1, "tanya", [6, 20], &loaded).unwrap();
    assert!(
        world.is_unit_build_limit_reached(1, loaded.units.get("tanya").unwrap()),
        "Tanya has build_limit = 1 and should be blocked once 1 is alive"
    );
    let avail_after_tanya = world.available_units_for_player(1, &loaded);
    assert!(
        !avail_after_tanya.iter().any(|u| u == "tanya"),
        "Tanya should be hidden/blocked from available_units_for_player while alive"
    );

    let _purifier_1 = world
        .spawn_building(1, "allied_ore_purifier", [8, 28], &loaded, false)
        .unwrap();
    assert!(
        world.is_building_build_limit_reached(
            1,
            loaded.buildings.get("allied_ore_purifier").unwrap()
        ),
        "Ore Purifier has build_limit = 1 and should be blocked once 1 is built"
    );

    // 3. Test High-Value Gems (`gem_tiles`) yielding 2x Credits on deposit
    assert!(
        !world.grid.gem_tiles.is_empty(),
        "skirmish_valley.toml should populate high-value gem_tiles at center"
    );
    let miner_id = world
        .entities
        .values()
        .find(|e| {
            e.player_id == 1
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "chrono_miner")
        })
        .map(|e| e.id)
        .unwrap();
    let ref_pos = world
            .entities
            .values()
            .find(|e| {
                e.player_id == 1
                    && matches!(&e.kind, EntityKind::Building { building_id } if building_id == "allied_refinery")
            })
            .map(|e| e.position)
            .unwrap();
    {
        let m = world.entities.get_mut(&miner_id).unwrap();
        m.position = ref_pos;
        m.path_waypoints.clear();
        if let Some(hs) = m.harvester_state.as_mut() {
            hs.phase = crate::world::HarvesterPhase::ReturningToRefinery;
            hs.current_ore = 200;
            hs.current_gems = 200; // 200 gems = +200 extra raw value => 400 base * 1.25 Purifier = 500 credits!
            hs.capacity = 200;
        }
    }
    let credits_before = world.players.get(&1).unwrap().credits;
    world.step(&loaded);
    let credits_after = world.players.get(&1).unwrap().credits;
    assert!(
        credits_after >= credits_before + 400,
        "Depositing 200 Gems should yield at least 2x value (400+ credits), got {}",
        credits_after - credits_before
    );

    // 4. Test Neutral Civilian Building Garrisoning (`player_id == 0`) & Engineer Capture Immunity while Occupied
    let neutral_b_id = world
        .spawn_building(0, "white_house", [18, 18], &loaded, false)
        .unwrap();
    let gi_id = world.spawn_unit(1, "allied_gi", [18, 22], &loaded).unwrap();
    world.push_command(GameCommand::LoadIntoCarrier {
        player_id: 1,
        passenger_ids: vec![gi_id],
        carrier_id: neutral_b_id,
    });
    for _ in 0..25 {
        world.step(&loaded);
    }
    let nb = world.entities.get(&neutral_b_id).unwrap();
    assert_eq!(
        nb.player_id, 1,
        "Neutral civilian building should switch to player 1 when garrisoned by player 1's GI"
    );
    assert!(nb.neutral_garrison);
    assert_eq!(nb.passenger_ids, vec![gi_id]);

    // Enemy Engineer cannot capture a garrisoned building while occupants are inside
    let enemy_eng = world
        .spawn_unit(2, "soviet_engineer", [18, 22], &loaded)
        .unwrap();
    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![enemy_eng],
        target_entity_id: neutral_b_id,
    });
    for _ in 0..10 {
        world.step(&loaded);
    }
    assert_eq!(
        world.entities.get(&neutral_b_id).unwrap().player_id,
        1,
        "Garrisoned building must be immune to enemy Engineer capture until cleared"
    );

    // Unloading all occupants reverts neutral_garrison building back to neutral (player_id == 0)
    world.push_command(GameCommand::UnloadCarrier {
        player_id: 1,
        carrier_ids: vec![neutral_b_id],
    });
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&neutral_b_id).unwrap().player_id,
        0,
        "Emptying a neutral_garrison building should revert ownership to neutral (player_id == 0)"
    );
}

#[test]
fn targeted_superweapons_spy_disguise_detection_building_sell_survivors_and_slave_liberation_work()
{
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    for p in world.players.values_mut() {
        p.power_produced = 3000;
        p.power_consumed = 100;
    }

    // 1. Test Targeted Sidebar Superweapons (`IronCurtain`, `GeneticMutator`, `PsychicDominator`, `NuclearStrike`, `LightningStorm`)
    world.spawn_building(2, "soviet_iron_curtain", [40, 40], &loaded, false);
    world.spawn_building(2, "soviet_nuclear_silo", [44, 40], &loaded, false);
    world.spawn_building(1, "allied_weather_controller", [10, 10], &loaded, false);
    world.spawn_building(2, "yuri_genetic_mutator", [40, 44], &loaded, false);
    world.spawn_building(2, "yuri_psychic_dominator", [44, 44], &loaded, false);

    for econ in world.players.values_mut() {
        econ.support_power_cooldowns.clear();
    }

    // Iron Curtain makes friendly vehicles invulnerable and kills organic infantry in the field
    let p2_tank = world
        .spawn_unit(2, "rhino_tank", [30, 30], &loaded)
        .unwrap();
    let p1_inf = world.spawn_unit(1, "allied_gi", [30, 31], &loaded).unwrap();
    let ic_pos = world.entities.get(&p2_tank).unwrap().position;
    world.push_command(GameCommand::CastSupportPower {
        player_id: 2,
        power_id: "iron_curtain_shield".to_string(),
        source_pos: None,
        target_pos: ic_pos,
    });
    world.step(&loaded);
    assert!(
        world
            .entities
            .get(&p2_tank)
            .unwrap()
            .invulnerable_remaining()
            >= 15.0,
        "Iron Curtain should make friendly vehicle invulnerable"
    );
    assert!(
        !world.entities.contains_key(&p1_inf),
        "Iron Curtain should vaporize organic infantry in its radius"
    );

    // Genetic Mutator converts organic infantry into yuri_brute owned by caster
    let p1_gi_mutate = world.spawn_unit(1, "allied_gi", [25, 25], &loaded).unwrap();
    let gm_pos = world.entities.get(&p1_gi_mutate).unwrap().position;
    world.push_command(GameCommand::CastSupportPower {
        player_id: 2,
        power_id: "genetic_mutator_blast".to_string(),
        source_pos: None,
        target_pos: gm_pos,
    });
    world.step(&loaded);
    assert!(
        !world.entities.contains_key(&p1_gi_mutate),
        "Genetic Mutator should remove original infantry"
    );
    let spawned_brute = world.entities.values().any(|e| {
        e.player_id == 2
            && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "yuri_brute")
    });
    assert!(
        spawned_brute,
        "Genetic Mutator should spawn a yuri_brute owned by the caster"
    );

    // Psychic Dominator permanently mind-controls enemy units in radius
    let p1_grizzly = world
        .spawn_unit(1, "grizzly_tank", [22, 22], &loaded)
        .unwrap();
    let pd_pos = world.entities.get(&p1_grizzly).unwrap().position;
    world.push_command(GameCommand::CastSupportPower {
        player_id: 2,
        power_id: "psychic_dominator_blast".to_string(),
        source_pos: None,
        target_pos: pd_pos,
    });
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&p1_grizzly).unwrap().player_id,
        2,
        "Psychic Dominator should permanently capture enemy Grizzly Tank"
    );

    // 2. Test Spy Disguise vs. Normal Units & Attack Dogs (`detects_disguise = true`)
    let spy_id = world
        .spawn_unit(1, "allied_spy", [35, 20], &loaded)
        .unwrap();
    let enemy_conscript = world
        .spawn_unit(2, "soviet_conscript", [35, 23], &loaded)
        .unwrap();
    world.push_command(GameCommand::DisguiseUnits {
        player_id: 1,
        spy_ids: vec![spy_id],
        target_entity_id: enemy_conscript,
    });
    world.step(&loaded);
    assert!(
        world.disguised_as(spy_id).is_some(),
        "Spy should be disguised as soviet_conscript"
    );
    // Normal enemy conscript at distance 3 tiles (6.0 world units <= 9.5 ppsh_rifle range) should NOT see through the disguise
    let spy_max_hp = world.entities.get(&spy_id).unwrap().max_health;
    for _ in 0..5 {
        world.step(&loaded);
    }
    assert!(
        (world.entities.get(&spy_id).unwrap().health - spy_max_hp).abs() < 1e-3,
        "Normal enemy infantry should ignore disguised spy unless a detector/dog is nearby"
    );
    // Spawn a Soviet Attack Dog (`detects_disguise = true`, `immune_to_mind_control = true`) nearby
    assert!(
        loaded
            .units
            .get("soviet_attack_dog")
            .unwrap()
            .immune_to_mind_control
    );
    let dog_id = world
        .spawn_unit(2, "soviet_attack_dog", [35, 22], &loaded)
        .unwrap();
    for _ in 0..15 {
        world.step(&loaded);
    }
    assert!(
        !world.entities.contains_key(&spy_id)
            || world.entities.get(&spy_id).unwrap().health < spy_max_hp
            || world.entities.get(&dog_id).unwrap().target_entity_id == Some(spy_id),
        "Soviet Attack Dog with detects_disguise should detect and attack the disguised spy"
    );

    // 3. Test Selling a Building spawns infantry survivors & Spy Sabotage on Superweapon resets cooldowns
    let barracks_id = world
        .spawn_building(1, "allied_barracks", [14, 14], &loaded, false)
        .unwrap();
    let p1_gi_before = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == 1
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "allied_gi")
        })
        .count();
    world.push_command(GameCommand::SellBuilding {
        player_id: 1,
        building_id: barracks_id,
    });
    world.step(&loaded);
    let p1_gi_after = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == 1
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "allied_gi")
        })
        .count();
    assert!(
        p1_gi_after > p1_gi_before,
        "Selling a completed Allied Barracks should spawn surviving GI infantry"
    );

    // 4. Test Destroying Yuri Slave Miner liberates enslaved infantry to the attacker's side & FreeUnit Crate
    let slave_miner_id = world
        .spawn_unit(2, "yuri_slave_miner", [16, 16], &loaded)
        .unwrap();
    let p1_conscripts_before = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == 1
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "soviet_conscript")
        })
        .count();
    world.entities.get_mut(&slave_miner_id).unwrap().health = 0.0;
    world.step(&loaded);
    let p1_conscripts_after = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == 1
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "soviet_conscript")
        })
        .count();
    assert_eq!(
        p1_conscripts_after,
        p1_conscripts_before + 2,
        "Destroying Yuri Slave Miner should liberate 2 enslaved workers to join Player 1"
    );

    // Test FreeUnit crate pickup spawns a free combat tank
    let p1_tanks_before = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == 1
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "grizzly_tank")
        })
        .count();
    let collector_gi = world.spawn_unit(1, "allied_gi", [12, 18], &loaded).unwrap();
    world.spawn_crate([12, 18], rts_sdk::schema::CrateKind::FreeUnit);
    world.step(&loaded);
    let _ = collector_gi;
    let p1_tanks_after = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == 1
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "grizzly_tank")
        })
        .count();
    assert_eq!(
        p1_tanks_after,
        p1_tanks_before + 1,
        "Collecting a FreeUnit crate should spawn a free Grizzly Tank for Allied player"
    );
}

#[test]
fn slave_miner_deploy_migration_superweapon_power_pause_tech_airport_powers_and_stolen_tech_matrix_work(
) {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();
    world.grid.ore_tiles.clear();
    world.grid.gem_tiles.clear();

    // 1. Test Yuri Slave Miner (`mobile_refinery`) Stationary Deployment (`deployed = true`, +40% armor, self-heal) & Automatic Ore-Patch Migration
    world.grid.ore_tiles.insert([10, 10], 40);
    world.grid.ore_tiles.insert([24, 10], 500);
    let sm_id = world
        .spawn_unit(2, "yuri_slave_miner", [10, 10], &loaded)
        .unwrap();
    world.entities.get_mut(&sm_id).unwrap().health = 600.0;
    for _ in 0..22 {
        world.step(&loaded);
    }
    // After mining the 40 ore at [10, 10], the local 6-tile radius is exhausted on the next 20-tick cycle -> packs up and migrates toward [24, 10]
    for _ in 0..22 {
        world.step(&loaded);
    }
    let sm_ent = world.entities.get(&sm_id).unwrap();
    assert!(
        sm_ent.health > 600.0,
        "Stationary deployed Yuri Slave Miner should passively self-repair while harvesting"
    );
    assert!(
            !sm_ent.script_ctx.deployed && sm_ent.position[0] > 20.5,
            "Yuri Slave Miner should automatically pack up (`deployed = false`) and migrate toward the next ore field at [24, 10] once local ore is exhausted"
        );

    // Also test manual ActivateAbility deploy toggle on yuri_slave_miner
    world.push_command(GameCommand::ActivateAbility {
        player_id: 2,
        entity_ids: vec![sm_id],
    });
    world.step(&loaded);
    let sm_deployed = world.entities.get(&sm_id).unwrap();
    assert!(
            sm_deployed.script_ctx.deployed
                && (sm_deployed.script_ctx.armor_multiplier - 1.40).abs() < 1e-4,
            "ActivateAbility on Yuri Slave Miner should engage stationary armored refinery mode (+40% armor)"
        );

    // 2. Test Superweapon Cooldown Pause under Low Power / Power Sabotage & Global Fog-of-War Reveal
    let _nuke_silo = world
        .spawn_building(2, "soviet_nuclear_silo", [30, 30], &loaded, false)
        .unwrap();
    world
        .players
        .get_mut(&2)
        .unwrap()
        .support_power_cooldowns
        .insert("soviet_nuclear_strike".to_string(), 60.0);
    // Player 2 has no power plants (-200 power delta -> is_low_power() == true)
    assert!(world.players.get(&2).unwrap().is_low_power());
    for _ in 0..10 {
        world.step(&loaded);
    }
    let cd_during_low_power = world
        .players
        .get(&2)
        .unwrap()
        .support_power_cooldowns
        .get("soviet_nuclear_strike")
        .copied()
        .unwrap();
    assert!(
        (cd_during_low_power - 60.0).abs() < 1e-4,
        "Superweapon cooldown must pause completely (0.0 rate) while owner is in low power"
    );
    // Restore power to Player 2 -> cooldown resumes ticking down
    world.spawn_building(2, "soviet_nuclear_reactor", [34, 30], &loaded, false);
    assert!(!world.players.get(&2).unwrap().is_low_power());
    for _ in 0..10 {
        world.step(&loaded);
    }
    let cd_with_power = world
        .players
        .get(&2)
        .unwrap()
        .support_power_cooldowns
        .get("soviet_nuclear_strike")
        .copied()
        .unwrap();
    assert!(
        cd_with_power < 60.0,
        "Superweapon cooldown should resume ticking once power is restored"
    );
    // Verify active_superweapon_warnings reports live support_power_cooldowns countdown
    let warnings = world.active_superweapon_warnings(&loaded);
    assert!(
        warnings
            .iter()
            .any(|(pid, _, cd)| *pid == 2 && (*cd - cd_with_power).abs() < 1e-3),
        "active_superweapon_warnings should report live support_power_cooldowns timer"
    );

    // 3. Test Targetable Sidebar Support Powers on Captured Tech Airport (`tech_airport_paradrop`) & Time Machine (`time_machine_chrono_shift`)
    let _airport = world
        .spawn_building(2, "tech_airport", [12, 30], &loaded, false)
        .unwrap();
    let _time_machine = world
        .spawn_building(1, "time_machine", [16, 30], &loaded, false)
        .unwrap();
    let sov_conscripts_before = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == 2
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "soviet_conscript")
        })
        .count();
    world.push_command(GameCommand::CastSupportPower {
        player_id: 2,
        power_id: "tech_airport_paradrop".to_string(),
        source_pos: None,
        target_pos: world.grid.tile_to_world([20, 20]),
    });
    world.step(&loaded);
    while world.active_support_aircraft.iter().any(|ac| !ac.dropped) {
        world.step(&loaded);
    }
    let sov_conscripts_after = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == 2
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "soviet_conscript")
        })
        .count();
    assert_eq!(
            sov_conscripts_after,
            sov_conscripts_before + 6,
            "Soviet player casting tech_airport_paradrop should paradrop 6 faction-specific soviet_conscript paratroopers"
        );

    // 4. Test Cross-Faction Stolen-Tech Commando Matrix (`resolve_stolen_tech_commando_with_rules`) & Cloning Vats Twin Commando Spawn
    assert_eq!(
        SimWorld::resolve_stolen_tech_commando_with_rules(
            "allies",
            "soviets",
            &loaded.manifest.rules
        ),
        "chrono_ivan"
    );
    assert_eq!(
        SimWorld::resolve_stolen_tech_commando_with_rules(
            "soviets",
            "soviets",
            &loaded.manifest.rules
        ),
        "chrono_ivan"
    );
    assert_eq!(
        SimWorld::resolve_stolen_tech_commando_with_rules(
            "soviets",
            "allies",
            &loaded.manifest.rules
        ),
        "chrono_commando"
    );
    assert_eq!(
        SimWorld::resolve_stolen_tech_commando_with_rules("allies", "yuri", &loaded.manifest.rules),
        "psi_commando"
    );

    // Train a stolen-tech commando (`chrono_commando`) at Barracks while owning `yuri_cloning_vats` -> spawns twin clone!
    world
        .players
        .get_mut(&1)
        .unwrap()
        .stolen_tech_units
        .insert("chrono_commando".to_string());
    world.players.get_mut(&1).unwrap().credits = 10_000;
    world.spawn_building(1, "allied_power_plant", [2, 2], &loaded, false);
    let p1_barracks = world
        .spawn_building(1, "allied_barracks", [4, 2], &loaded, false)
        .unwrap();
    let _p1_vats = world
        .spawn_building(1, "yuri_cloning_vats", [8, 2], &loaded, false)
        .unwrap();
    world.push_command(GameCommand::ProduceUnit {
        player_id: 1,
        producer_entity_id: Some(p1_barracks),
        unit_id: "chrono_commando".to_string(),
    });
    world.step(&loaded);
    world
        .entities
        .get_mut(&p1_barracks)
        .unwrap()
        .production_queue[0]
        .remaining_secs = 0.02;
    world.step(&loaded);
    let chrono_commandos = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == 1
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "chrono_commando")
        })
        .count();
    assert_eq!(
            chrono_commandos, 2,
            "Training a stolen-tech Chrono Commando while owning Cloning Vats should spawn a free twin clone"
        );
}

#[test]
fn desolator_rad_field_deploy_air_crash_damage_transport_bailout_and_ai_mobile_bunkers_work() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();
    world.grid.water_tiles.clear();
    world.radiation_zones.clear();

    // 1. Test Desolator Rad-Field Deployment (`rad_irradiation_field` in `ActivateAbility` & `step_combat`)
    let desolator = world
        .spawn_unit(2, "desolator", [10, 10], &loaded)
        .expect("should spawn desolator");
    let nearby_enemy_gi = world
        .spawn_unit(1, "allied_gi", [11, 10], &loaded)
        .expect("should spawn allied_gi");
    // Put both units on HoldFire so we test pure deployed RadiationZone damage without direct weapon fire
    world.entities.get_mut(&desolator).unwrap().stance = crate::command::UnitStance::HoldFire;
    world.entities.get_mut(&nearby_enemy_gi).unwrap().stance = crate::command::UnitStance::HoldFire;
    let gi_hp_before = world.entities.get(&nearby_enemy_gi).unwrap().health;
    let des_hp_before = world.entities.get(&desolator).unwrap().health;

    world.push_command(GameCommand::ActivateAbility {
        player_id: 2,
        entity_ids: vec![desolator],
    });
    for _ in 0..8 {
        world.step(&loaded);
    }
    let des_ent = world.entities.get(&desolator).unwrap();
    assert!(
        des_ent.script_ctx.deployed && des_ent.script_ctx.speed_multiplier == 0.0,
        "Deployed Desolator should be immobile (`speed_multiplier == 0.0`)"
    );
    assert!(
        !world.radiation_zones.is_empty(),
        "Deployed Desolator should create and sustain an active RadiationZone"
    );
    assert_eq!(
            des_ent.health, des_hp_before,
            "Desolator (`immune_to_radiation = true`) must not take damage from his own radiation field"
        );
    assert!(
        world.entities.get(&nearby_enemy_gi).unwrap().health < gi_hp_before,
        "Enemy infantry inside deployed Desolator radiation field should take radiation damage"
    );

    // Undeploy Desolator -> restores mobility
    world.push_command(GameCommand::ActivateAbility {
        player_id: 2,
        entity_ids: vec![desolator],
    });
    world.step(&loaded);
    assert!(
        !world.entities.get(&desolator).unwrap().script_ctx.deployed
            && world
                .entities
                .get(&desolator)
                .unwrap()
                .script_ctx
                .speed_multiplier
                == 1.0,
        "Undeploying Desolator should restore normal movement speed"
    );

    // 2. Test Airborne Aircraft Crash-Impact AoE Damage on Destruction (`kirov_airship` / `harrier`)
    let doomed_kirov = world
        .spawn_unit(2, "kirov_airship", [20, 20], &loaded)
        .expect("should spawn kirov_airship");
    let ground_victim_tank = world
        .spawn_unit(1, "grizzly_tank", [20, 20], &loaded)
        .expect("should spawn grizzly_tank under kirov");
    let high_flying_bystander = world
        .spawn_unit(1, "harrier", [20, 20], &loaded)
        .expect("should spawn harrier in air");
    let tank_hp_before = world.entities.get(&ground_victim_tank).unwrap().health;
    let harrier_hp_before = world.entities.get(&high_flying_bystander).unwrap().health;

    world.entities.get_mut(&doomed_kirov).unwrap().health = 0.0;
    world.step(&loaded);
    assert!(
            !world.entities.contains_key(&ground_victim_tank)
                || world.entities.get(&ground_victim_tank).unwrap().health < tank_hp_before - 250.0,
            "Downed Kirov Airship crashing onto ground should deal heavy crash-impact AoE damage to units underneath"
        );
    assert_eq!(
        world.entities.get(&high_flying_bystander).unwrap().health,
        harrier_hp_before,
        "Ground crash-impact AoE should not damage high-altitude aircraft"
    );

    // 3. Test Transport Passenger Survival / Bailout (Land) vs. Total Loss (Air / Deep Water)
    // 3a. Airborne NightHawk Chopper destroyed -> passengers inside perish
    let doomed_chopper = world
        .spawn_unit(1, "nighthawk_chopper", [26, 10], &loaded)
        .unwrap();
    let air_passenger = world.spawn_unit(1, "allied_gi", [26, 10], &loaded).unwrap();
    world
        .entities
        .get_mut(&air_passenger)
        .unwrap()
        .transported_by = Some(doomed_chopper);
    world
        .entities
        .get_mut(&doomed_chopper)
        .unwrap()
        .passenger_ids
        .push(air_passenger);
    world.entities.get_mut(&doomed_chopper).unwrap().health = 0.0;
    world.step(&loaded);
    assert!(
        !world.entities.contains_key(&air_passenger),
        "Passengers inside an airborne transport shot down in mid-air must perish with the crash"
    );

    // 3b. Naval Hover Transport destroyed over deep water -> passengers inside drown
    world.grid.water_tiles.insert([30, 10]);
    let doomed_hover = world
        .spawn_unit(2, "soviet_zubr_transport", [30, 10], &loaded)
        .unwrap();
    let water_passenger = world
        .spawn_unit(2, "soviet_conscript", [30, 10], &loaded)
        .unwrap();
    world
        .entities
        .get_mut(&water_passenger)
        .unwrap()
        .transported_by = Some(doomed_hover);
    world
        .entities
        .get_mut(&doomed_hover)
        .unwrap()
        .passenger_ids
        .push(water_passenger);
    world.entities.get_mut(&doomed_hover).unwrap().health = 0.0;
    world.step(&loaded);
    assert!(
        !world.entities.contains_key(&water_passenger),
        "Passengers inside a transport destroyed over deep water must sink and perish"
    );

    // 3c. Ground APC (`flak_track`) destroyed on dry land -> passenger bails out with 50% blast damage
    let doomed_apc = world
        .spawn_unit(2, "flak_track", [26, 16], &loaded)
        .unwrap();
    let land_passenger = world
        .spawn_unit(2, "tesla_trooper", [26, 16], &loaded)
        .unwrap();
    let p_max_hp = world.entities.get(&land_passenger).unwrap().max_health;
    world
        .entities
        .get_mut(&land_passenger)
        .unwrap()
        .transported_by = Some(doomed_apc);
    world
        .entities
        .get_mut(&doomed_apc)
        .unwrap()
        .passenger_ids
        .push(land_passenger);
    world.entities.get_mut(&doomed_apc).unwrap().health = 0.0;
    world.step(&loaded);
    let survivor_p = world
        .entities
        .get(&land_passenger)
        .expect("Passenger inside land APC should survive via emergency bailout");
    assert!(
        survivor_p.transported_by.is_none(),
        "Bailed-out passenger should be ejected onto the battlefield (`transported_by == None`)"
    );
    assert!(
        (survivor_p.health - p_max_hp * 0.5).abs() < 1e-3,
        "Bailed-out passenger should suffer 50% blast damage (expected {}, got {})",
        p_max_hp * 0.5,
        survivor_p.health
    );

    // 4. Test Tactical AI Mobile Bunker Loading (`battle_fortress`) & Frontline APC Unload (`flak_track`)
    world.entities.clear();
    world.tick = 20;
    let ai_fortress = world
        .spawn_unit(2, "battle_fortress", [12, 12], &loaded)
        .unwrap();
    let ai_inf = world
        .spawn_unit(2, "guardian_gi", [13, 12], &loaded)
        .unwrap();
    let ai_frontline_apc = world
        .spawn_unit(2, "flak_track", [24, 24], &loaded)
        .unwrap();
    let apc_troop = world
        .spawn_unit(2, "soviet_conscript", [24, 24], &loaded)
        .unwrap();
    world.entities.get_mut(&apc_troop).unwrap().transported_by = Some(ai_frontline_apc);
    world
        .entities
        .get_mut(&ai_frontline_apc)
        .unwrap()
        .passenger_ids
        .push(apc_troop);
    let _enemy_at_front = world
        .spawn_unit(1, "grizzly_tank", [26, 24], &loaded)
        .unwrap();

    let ai_cmds = crate::ai::generate_ai_commands_with_difficulty(
        &world,
        2,
        &loaded,
        crate::ai::AiDifficulty::Hard,
    );
    assert!(
        ai_cmds.iter().any(|c| matches!(
            c,
            GameCommand::LoadIntoCarrier {
                passenger_ids,
                carrier_id,
                ..
            } if *carrier_id == ai_fortress && passenger_ids.contains(&ai_inf)
        )),
        "Hard AI should load idle combat infantry into a friendly Battle Fortress mobile bunker"
    );
    assert!(
            ai_cmds.iter().any(|c| matches!(
                c,
                GameCommand::UnloadCarrier { carrier_ids, .. }
                    if carrier_ids.contains(&ai_frontline_apc)
            )),
            "Hard AI should automatically unload a closed Flak Track APC when it reaches enemy engagement range"
        );
}

#[test]
fn mind_control_target_rotation_chrono_stasis_critical_garrison_bailout_and_psychic_beacon_work() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    let initial_ids: Vec<u64> = world.entities.keys().copied().collect();
    for id in initial_ids {
        world.entities.remove(&id);
    }

    // 1. Mind-Control Target Switching at Full Capacity (`yuri_clone`) & Multi-Target Auto-Rotation (`mastermind`)
    let yuri_clone = world
        .spawn_unit(2, "yuri_clone", [10, 10], &loaded)
        .unwrap();
    let tank_a = world
        .spawn_unit(1, "grizzly_tank", [12, 10], &loaded)
        .unwrap();
    let tank_b = world
        .spawn_unit(1, "grizzly_tank", [10, 12], &loaded)
        .unwrap();
    world.entities.get_mut(&tank_a).unwrap().stance = crate::command::UnitStance::HoldFire;
    world.entities.get_mut(&tank_b).unwrap().stance = crate::command::UnitStance::HoldFire;

    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![yuri_clone],
        target_entity_id: tank_a,
    });
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&tank_a).unwrap().player_id,
        2,
        "Yuri Clone should mind-control Tank A"
    );
    assert_eq!(
        world.entities.get(&yuri_clone).unwrap().target_entity_id,
        None,
        "Yuri Clone should clear target_entity_id immediately after capturing a target"
    );

    // Explicitly command Yuri Clone (at 1/1 limit) to mind-control Tank B -> Tank A should be released back to Player 1
    world.entities.get_mut(&yuri_clone).unwrap().weapon_cooldown = 0.0;
    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![yuri_clone],
        target_entity_id: tank_b,
    });
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&tank_b).unwrap().player_id,
        2,
        "Yuri Clone should capture the newly ordered Tank B"
    );
    assert_eq!(
            world.entities.get(&tank_a).unwrap().player_id,
            1,
            "Oldest controlled unit (Tank A) should be released back to Player 1 when Yuri Clone switches targets at capacity"
        );

    // Mastermind (`mind_control_limit = 3`) auto-rotates across 3 hostile targets without manual re-targeting
    world.entities.remove(&yuri_clone);
    world.entities.remove(&tank_a);
    world.entities.remove(&tank_b);
    let mastermind = world
        .spawn_unit(2, "mastermind", [20, 20], &loaded)
        .unwrap();
    let m_v1 = world.spawn_unit(1, "allied_gi", [22, 20], &loaded).unwrap();
    let m_v2 = world.spawn_unit(1, "allied_gi", [20, 22], &loaded).unwrap();
    let m_v3 = world.spawn_unit(1, "allied_gi", [18, 20], &loaded).unwrap();
    for vid in [m_v1, m_v2, m_v3] {
        world.entities.get_mut(&vid).unwrap().stance = crate::command::UnitStance::HoldFire;
    }
    for _ in 0..3 {
        world.entities.get_mut(&mastermind).unwrap().weapon_cooldown = 0.0;
        world.step(&loaded);
    }
    assert_eq!(
        world.entities.get(&m_v1).unwrap().player_id,
        2,
        "Mastermind should auto-capture target 1"
    );
    assert_eq!(
        world.entities.get(&m_v2).unwrap().player_id,
        2,
        "Mastermind should auto-rotate and capture target 2"
    );
    assert_eq!(
        world.entities.get(&m_v3).unwrap().player_id,
        2,
        "Mastermind should auto-rotate and capture target 3 up to mind_control_limit = 3"
    );

    // 2. Chrono Legionnaire Distance-Scaled Phase-In Stasis (`chrono_disable_timer`) & Temporal-Lock Freeze
    let cleg = world
        .spawn_unit(1, "chrono_legionnaire", [6, 6], &loaded)
        .unwrap();
    let long_dest = world.grid.tile_to_world([56, 56]);
    world.push_command(GameCommand::MoveUnits {
        player_id: 1,
        entity_ids: vec![cleg],
        target_pos: long_dest,
    });
    world.step(&loaded);
    let stasis_after_long_jump = world
        .entities
        .get(&cleg)
        .unwrap()
        .chrono_disable_remaining();
    assert!(
            stasis_after_long_jump > 4.0,
            "Cross-map Chrono teleport should incur a long distance-scaled phase-in stasis (> 4.0s), got {stasis_after_long_jump}"
        );

    let enemy_wf = world
        .spawn_building(2, "soviet_war_factory", [30, 30], &loaded, false)
        .unwrap();
    world
        .entities
        .get_mut(&enemy_wf)
        .unwrap()
        .production_queue
        .push(crate::world::ProductionItem {
            unit_id: "rhino_tank".to_string(),
            remaining_secs: 10.0,
            total_secs: 10.0,
            on_hold: false,
        });
    world
        .entities
        .get_mut(&enemy_wf)
        .unwrap()
        .set_temporal_lock(2.0);
    let locked_gi = world.spawn_unit(2, "allied_gi", [34, 30], &loaded).unwrap();
    world
        .entities
        .get_mut(&locked_gi)
        .unwrap()
        .set_temporal_lock(2.0);
    world.push_command(GameCommand::ActivateAbility {
        player_id: 2,
        entity_ids: vec![locked_gi],
    });
    world.step(&loaded);
    assert!(
            (world.entities.get(&enemy_wf).unwrap().production_queue[0].remaining_secs - 10.0).abs()
                < 1e-4,
            "War Factory locked in temporal stasis (`temporal_lock_timer > 0.0`) must freeze its production queue"
        );
    assert!(
            !world.entities.get(&locked_gi).unwrap().script_ctx.deployed,
            "Unit locked in temporal stasis (`temporal_lock_timer > 0.0`) must not be able to activate abilities"
        );

    // 3. Automatic Emergency Evacuation of Garrisoned Building at Critical Red Health (<= 15% HP) & Engineer Repair
    let civ_bldg = world
        .spawn_building(0, "st_basil_cathedral", [15, 35], &loaded, false)
        .unwrap();
    let gar_gi = world.spawn_unit(1, "allied_gi", [15, 35], &loaded).unwrap();
    world.entities.get_mut(&gar_gi).unwrap().transported_by = Some(civ_bldg);
    {
        let b_mut = world.entities.get_mut(&civ_bldg).unwrap();
        b_mut.player_id = 1;
        b_mut.neutral_garrison = true;
        b_mut.passenger_ids.push(gar_gi);
        b_mut.health = b_mut.max_health * 0.14;
        b_mut.script_ctx.health = b_mut.health;
    }
    world.step(&loaded);
    let b_after = world.entities.get(&civ_bldg).unwrap();
    assert!(
            b_after.passenger_ids.is_empty() && b_after.player_id == 0,
            "Garrisoned civilian building dropping to <= 15% HP should auto-evacuate infantry and revert to neutral"
        );
    assert!(
        world
            .entities
            .get(&gar_gi)
            .unwrap()
            .transported_by
            .is_none(),
        "Evacuated infantry should be placed outside the critically damaged building"
    );

    // Engineer repairs the critically damaged neutral civilian building back to 100% HP
    let eng = world
        .spawn_unit(1, "allied_engineer", [16, 39], &loaded)
        .unwrap();
    world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![eng],
        target_entity_id: civ_bldg,
    });
    for _ in 0..25 {
        world.step(&loaded);
    }
    let b_repaired = world.entities.get(&civ_bldg).unwrap();
    assert!(
            (b_repaired.health - b_repaired.max_health).abs() < 1.0 && b_repaired.player_id == 0,
            "Engineer entering a damaged neutral garrison should repair it to full health while keeping it neutral"
        );

    // 4. Psychic Beacon (`psychic_beacon`) Support Power & Liberation on Beacon Destruction
    for _ in 0..3 {
        world.spawn_building(2, "yuri_bio_reactor", [42, 42], &loaded, false);
    }
    let beacon = world
        .spawn_building(2, "psychic_beacon", [46, 42], &loaded, false)
        .unwrap();
    for econ in world.players.values_mut() {
        econ.support_power_cooldowns.clear();
    }
    let beacon_victim = world
        .spawn_unit(1, "grizzly_tank", [46, 30], &loaded)
        .unwrap();
    let victim_pos = world.entities.get(&beacon_victim).unwrap().position;
    world.push_command(GameCommand::CastSupportPower {
        player_id: 2,
        power_id: "psychic_beacon_control".to_string(),
        source_pos: None,
        target_pos: victim_pos,
    });
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&beacon_victim).unwrap().player_id,
        2,
        "Psychic Beacon support power should mind-control enemy units in target radius"
    );
    assert_eq!(
        world.mind_controlled_by(beacon_victim),
        Some((beacon, 1)),
        "Units mind-controlled by Psychic Beacon should be linked to the Beacon entity ID"
    );

    // Destroy the Psychic Beacon -> controlled units must be liberated back to Player 1
    world.entities.get_mut(&beacon).unwrap().health = 0.0;
    world.step(&loaded);
    assert_eq!(
            world.entities.get(&beacon_victim).unwrap().player_id,
            1,
            "Destroying the Psychic Beacon must immediately liberate all units it mind-controlled back to their original owner"
        );
}

#[test]
fn carrier_drone_docking_airfield_pad_cap_homeless_jet_crash_mc_sell_capture_and_spy_airfield_vet_work(
) {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();

    for x in 4..=16 {
        for z in 4..=12 {
            world.grid.water_tiles.insert([x, z]);
        }
    }

    // 1. Carrier Drone Lifecycle: Hornet launch, return & hangar docking after target destruction, and orphan crash when carrier dies
    let carrier = world
        .spawn_unit(1, "aircraft_carrier", [8, 8], &loaded)
        .unwrap();
    let coastal_target = world
        .spawn_unit(2, "soviet_conscript", [14, 14], &loaded)
        .unwrap();
    world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![carrier],
        target_entity_id: coastal_target,
    });
    world.step(&loaded);
    let hornet_id = world
        .entities
        .values()
        .find(|e| world.trait_stores.parent_carrier_id(e) == Some(carrier))
        .expect("Aircraft Carrier should launch a Hornet Fighter drone")
        .id;

    // Move Hornet out towards target, then eliminate target -> Hornet should fly back and dock into Carrier hangar
    world.entities.get_mut(&hornet_id).unwrap().position = [24.0, 5.0, 24.0];
    world.entities.remove(&coastal_target);
    for _ in 0..45 {
        world.step(&loaded);
    }
    assert!(
            !world.entities.contains_key(&hornet_id),
            "Returning Hornet drone should dock back into the Aircraft Carrier hangar once its target is eliminated"
        );

    // Launch another Hornet against a second target, then destroy the Carrier -> airborne Hornet must crash immediately
    let coastal_target_2 = world
        .spawn_building(2, "soviet_barracks", [16, 14], &loaded, false)
        .unwrap();
    world.entities.get_mut(&carrier).unwrap().weapon_cooldown = 0.0;
    world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![carrier],
        target_entity_id: coastal_target_2,
    });
    world.step(&loaded);
    let hornet_2 = world
        .entities
        .values()
        .find(|e| world.trait_stores.parent_carrier_id(e) == Some(carrier))
        .expect("Carrier should re-launch Hornet drone")
        .id;
    world.entities.get_mut(&carrier).unwrap().health = 0.0;
    world.step(&loaded);
    assert!(
            !world.entities.contains_key(&hornet_2),
            "Airborne Hornet drone must crash and be destroyed when its parent Aircraft Carrier is destroyed"
        );
    world.entities.remove(&coastal_target_2);

    // 2. Airfield 4-Pad Capacity Enforcement (`allied_airforce_command` = max 4 pads) & Homeless Jet Crash
    let af_1 = world
        .spawn_building(1, "allied_airforce_command", [20, 20], &loaded, false)
        .unwrap();
    let af_2 = world
        .spawn_building(1, "allied_airforce_command", [32, 20], &loaded, false)
        .unwrap();
    let mut jets = Vec::new();
    for _ in 0..5 {
        let j = world.spawn_unit(1, "harrier", [20, 20], &loaded).unwrap();
        jets.push(j);
    }
    world.step(&loaded);
    let docked_at_af1 = jets
        .iter()
        .filter(|&&jid| {
            let e = world.entities.get(&jid).unwrap();
            e.landed_at_airfield && world.home_airfield_id(jid) == Some(af_1)
        })
        .count();
    assert_eq!(
        docked_at_af1, 4,
        "A single Allied Airforce Command must allow at most 4 jets to dock on its 4 pads"
    );
    let fifth_jet = jets[4];
    assert_eq!(
        world.home_airfield_id(fifth_jet),
        Some(af_2),
        "5th Harrier exceeding Airfield 1's 4-pad capacity should automatically divert to Airfield 2"
    );

    // Destroy both airfields: jets landed on Airfield 1 are destroyed with it, and airborne 5th jet with 0 ammo crashes
    world.entities.get_mut(&fifth_jet).unwrap().current_ammo = 0;
    world
        .entities
        .get_mut(&fifth_jet)
        .unwrap()
        .landed_at_airfield = false;
    world.entities.get_mut(&af_1).unwrap().health = 0.0;
    world.entities.get_mut(&af_2).unwrap().health = 0.0;
    world.step(&loaded);
    world.step(&loaded);
    for &jid in &jets {
        assert!(
                !world.entities.contains_key(&jid),
                "Landed jets on a destroyed Airfield and homeless airborne jets out of ammo with no Airfield must be destroyed/crash"
            );
    }

    // 3. Selling or Engineer-Capturing a Psychic Tower / Beacon Liberates Mind-Controlled Units & Defuses C4
    for _ in 0..2 {
        world.spawn_building(2, "yuri_bio_reactor", [4, 30], &loaded, false);
    }
    let psy_tower_1 = world
        .spawn_building(2, "yuri_psychic_tower", [8, 30], &loaded, false)
        .unwrap();
    let victim_1 = world
        .spawn_unit(1, "grizzly_tank", [10, 30], &loaded)
        .unwrap();
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&victim_1).unwrap().player_id,
        2,
        "Psychic Tower 1 should mind-control Victim 1"
    );
    world.push_command(GameCommand::SellBuilding {
        player_id: 2,
        building_id: psy_tower_1,
    });
    world.step(&loaded);
    assert_eq!(
            world.entities.get(&victim_1).unwrap().player_id,
            1,
            "Selling a Psychic Tower must immediately liberate its mind-controlled unit back to its original owner"
        );

    let psy_tower_2 = world
        .spawn_building(2, "yuri_psychic_tower", [14, 30], &loaded, false)
        .unwrap();
    world.entities.get_mut(&psy_tower_2).unwrap().power_disabled = true;
    let victim_2 = world
        .spawn_unit(1, "grizzly_tank", [16, 30], &loaded)
        .unwrap();
    {
        let v2 = world.entities.get_mut(&victim_2).unwrap();
        v2.player_id = 2;
    }
    world.set_mind_controlled_by(victim_2, Some((psy_tower_2, 1)));
    world
        .entities
        .get_mut(&psy_tower_2)
        .unwrap()
        .set_c4_bomb(5.0, 1500.0, 0.0, 1);
    let cap_eng = world
        .spawn_unit(1, "allied_engineer", [14, 32], &loaded)
        .unwrap();
    world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![cap_eng],
        target_entity_id: psy_tower_2,
    });
    for _ in 0..10 {
        world.step(&loaded);
    }
    assert_eq!(
        world.entities.get(&psy_tower_2).unwrap().player_id,
        1,
        "Allied Engineer should capture Psychic Tower 2"
    );
    assert_eq!(
        world.entities.get(&psy_tower_2).unwrap().c4_remaining(),
        0.0,
        "Engineer capturing a building with active C4 should also defuse the C4 charge"
    );
    assert_eq!(
            world.entities.get(&victim_2).unwrap().player_id,
            1,
            "Engineer capturing an enemy Psychic Tower must liberate the units it was previously mind-controlling"
        );
    assert!(
        world.mind_controlled_by(victim_2).is_none(),
        "Liberated unit should have mind_controlled_by cleared"
    );

    // 4. Spy Infiltration of Enemy Airfield (`BuildingRole::Airfield`) Unlocks Veteran Aircraft (`veteran_aircraft_unlocked`)
    let enemy_af = world
        .spawn_building(2, "allied_airforce_command", [26, 32], &loaded, false)
        .unwrap();
    let spy = world
        .spawn_unit(1, "allied_spy", [26, 35], &loaded)
        .unwrap();
    world.push_command(GameCommand::InfiltrateBuilding {
        player_id: 1,
        spy_ids: vec![spy],
        target_building_id: enemy_af,
    });
    for _ in 0..15 {
        world.step(&loaded);
    }
    assert!(
        world.players.get(&1).unwrap().veteran_aircraft_unlocked,
        "Infiltrating an enemy Airfield with a Spy should unlock veteran_aircraft_unlocked"
    );
    let vet_harrier = world.spawn_unit(1, "harrier", [20, 32], &loaded).unwrap();
    let vet_rocketeer = world.spawn_unit(1, "rocketeer", [21, 32], &loaded).unwrap();
    let rookie_tank = world
        .spawn_unit(1, "grizzly_tank", [22, 32], &loaded)
        .unwrap();
    assert_eq!(
        world.entities.get(&vet_harrier).unwrap().veterancy,
        crate::world::VeterancyRank::Veteran,
        "Newly spawned Harrier jet should start as Veteran after Airfield infiltration"
    );
    assert_eq!(
        world.entities.get(&vet_rocketeer).unwrap().veterancy,
        crate::world::VeterancyRank::Veteran,
        "Newly spawned Rocketeer jumpjet should start as Veteran after Airfield infiltration"
    );
    assert_eq!(
        world.entities.get(&rookie_tank).unwrap().veterancy,
        crate::world::VeterancyRank::Rookie,
        "Ground tank should remain Rookie when only Airfield was infiltrated"
    );

    // 5. Grinder Transport + Passenger Refund & Genetic Mutator Mind-Control Release
    let grinder = world
        .spawn_building(2, "yuri_grinder", [34, 32], &loaded, false)
        .unwrap();
    let transport = world
        .spawn_unit(2, "flak_track", [34, 34], &loaded)
        .unwrap();
    let passenger = world
        .spawn_unit(2, "soviet_conscript", [34, 34], &loaded)
        .unwrap();
    world.entities.get_mut(&passenger).unwrap().transported_by = Some(transport);
    world
        .entities
        .get_mut(&transport)
        .unwrap()
        .passenger_ids
        .push(passenger);
    let p2_credits_before = world.players.get(&2).unwrap().credits;
    world.push_command(GameCommand::LoadIntoCarrier {
        player_id: 2,
        passenger_ids: vec![transport],
        carrier_id: grinder,
    });
    world.step(&loaded);
    assert!(
        !world.entities.contains_key(&transport) && !world.entities.contains_key(&passenger),
        "Loaded transport recycled in Grinder should recycle both the transport and its passenger"
    );
    assert_eq!(
            world.players.get(&2).unwrap().credits,
            p2_credits_before + 480,
            "Recycling Flak Track (500 -> 400) + Conscript (100 -> 80) in Grinder should refund 480 credits"
        );
}
