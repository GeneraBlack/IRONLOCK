use super::load_ra2_mod;
use crate::ai::{AiDifficulty, AiPersonality};
use crate::combat::weather_storm::WEATHER_STORM_TOTAL_BOLTS;
use crate::command::GameCommand;
use crate::world::airpad_docking::airfield_pad_world_coords;
use crate::world::country_bonuses::resolve_deterministic_sub_faction;
use crate::world::{
    EntityKind, GameMode, LobbyLaunchSimConfig, LobbySlotKind, LobbySlotSimSpec, RadiationZone,
    SimWorld,
};

#[test]
fn all_ten_ra2_countries_apply_passive_bonuses_and_preserve_none_baseline() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();

    // 0. Baseline (`sub_faction: None`) keeps exact 1.0 multipliers + VIPs/slaves stay out of build queues
    world.set_player_sub_faction(1, None);
    world.set_player_sub_faction(2, None);
    let _b1 = world
        .spawn_building(1, "allied_barracks", [4, 4], &loaded, false)
        .unwrap();
    let _b2 = world
        .spawn_building(2, "yuri_barracks", [20, 20], &loaded, false)
        .unwrap();
    let p1_avail = world.available_units_for_player(1, &loaded);
    let p2_avail = world.available_units_for_player(2, &loaded);
    assert!(!p1_avail.contains(&"president_dugan".to_string()));
    assert!(!p1_avail.contains(&"prof_einstein".to_string()));
    assert!(!p2_avail.contains(&"yuri_slave".to_string()));

    // Verify `yr_soviet_06_to_the_moon` includes starting `soviet_cosmonaut` lunar squad
    let moon_map = loaded
        .maps
        .get("yr_soviet_06_to_the_moon")
        .expect("yr_soviet_06_to_the_moon should exist");
    let has_cosmonauts = moon_map
        .spawns
        .iter()
        .flat_map(|s| s.initial_units.iter())
        .filter(|u| u.unit_id == "soviet_cosmonaut")
        .count();
    assert!(
        has_cosmonauts >= 3,
        "Moon mission should spawn a starting soviet_cosmonaut strike squad"
    );

    let base_gi = world.spawn_unit(1, "allied_gi", [6, 4], &loaded).unwrap();
    let base_gi_hp = world.entities[&base_gi].max_health;
    let gi_def = loaded.units.get("allied_gi").unwrap();
    assert_eq!(
        world.effective_country_unit_cost(1, gi_def, &loaded),
        gi_def.cost
    );

    // 1. USA: +15% infantry HP + free "airborne_paradrop" support power (without Airforce Command!)
    world.set_player_sub_faction(1, Some("usa".to_string()));
    let usa_gi = world.spawn_unit(1, "allied_gi", [7, 4], &loaded).unwrap();
    assert!((world.entities[&usa_gi].max_health - base_gi_hp * 1.15).abs() < 0.05);
    let usa_powers = world.available_support_powers_for_player(1, &loaded);
    assert!(
        usa_powers
            .iter()
            .any(|(sp, _)| sp.id.contains("airborne_paradrop")),
        "USA should unlock free airborne paradrop without Airforce Command"
    );
    world.push_command(GameCommand::CastSupportPower {
        player_id: 1,
        power_id: "airborne_paradrop".to_string(),
        source_pos: None,
        target_pos: [16.0, 0.0, 16.0],
    });
    world.step(&loaded);
    assert!(
        !world.active_support_aircraft.is_empty(),
        "Casting USA free 'airborne_paradrop' should launch delivery aircraft"
    );

    // 2. Korea: +18% aircraft damage + +10% aircraft HP (`black_eagle`)
    let be_base_hp = loaded.units["black_eagle"].health;
    world.set_player_sub_faction(1, Some("korea".to_string()));
    let be_id = world.spawn_unit(1, "black_eagle", [8, 4], &loaded).unwrap();
    assert!((world.entities[&be_id].max_health - be_base_hp * 1.10).abs() < 0.1);
    assert!((world.entities[&be_id].script_ctx.damage_multiplier - 1.18).abs() < 1e-4);

    // 3. France: +18% defensive structure rate-of-fire (`allied_grand_cannon`)
    world.set_player_sub_faction(1, Some("france".to_string()));
    let gc_id = world
        .spawn_building(1, "allied_grand_cannon", [10, 4], &loaded, false)
        .unwrap();
    assert!((world.entities[&gc_id].buff_fire_rate_multiplier() - 1.18).abs() < 1e-4);

    // 4. Great Britain: +15% infantry attack range & damage (`sniper`)
    world.set_player_sub_faction(1, Some("great_britain".to_string()));
    let sniper_id = world.spawn_unit(1, "sniper", [12, 4], &loaded).unwrap();
    assert!((world.entities[&sniper_id].script_ctx.damage_multiplier - 1.15).abs() < 1e-4);
    assert!(world.entities[&sniper_id].script_ctx.range_bonus > 1.5);

    // 5. Germany: +18% anti-armor vehicle firepower (`tank_destroyer` vs `rhino_tank`)
    world.set_player_sub_faction(1, Some("germany".to_string()));
    let td_id = world
        .spawn_unit(1, "tank_destroyer", [13, 4], &loaded)
        .unwrap();
    let enemy_rhino = world.spawn_unit(2, "rhino_tank", [15, 4], &loaded).unwrap();
    let aa_mult = world.country_anti_armor_damage_multiplier(td_id, enemy_rhino, &loaded);
    assert!((aa_mult - 1.18).abs() < 1e-4);

    // 6. Russia: -12% vehicle build cost (verified on `ProduceUnit` + `CancelProduction`!)
    world.players.get_mut(&2).unwrap().faction = "soviets".to_string();
    world.set_player_sub_faction(2, Some("russia".to_string()));
    let wf_id = world
        .spawn_building(2, "soviet_war_factory", [24, 20], &loaded, false)
        .unwrap();
    let _rad_id = world
        .spawn_building(2, "soviet_radar_tower", [28, 20], &loaded, false)
        .unwrap();
    let tesla_def = loaded.units.get("tesla_tank").unwrap();
    let expected_russia_cost = ((tesla_def.cost as f32) * 0.88).round() as u32;
    assert_eq!(
        world.effective_country_unit_cost(2, tesla_def, &loaded),
        expected_russia_cost
    );
    world.players.get_mut(&2).unwrap().credits = 5_000;
    world.push_command(GameCommand::ProduceUnit {
        player_id: 2,
        producer_entity_id: Some(wf_id),
        unit_id: "tesla_tank".to_string(),
    });
    world.step(&loaded);
    assert_eq!(
        world.players[&2].credits,
        5_000 - expected_russia_cost,
        "Russia should pay -12% discounted credits when queuing a vehicle"
    );
    world.push_command(GameCommand::CancelProduction {
        player_id: 2,
        unit_id: "tesla_tank".to_string(),
    });
    world.step(&loaded);
    assert_eq!(
        world.players[&2].credits, 5_000,
        "Canceling discounted Russia vehicle must refund exact discounted cost"
    );

    // 7. Iraq: 65% radiation hazard resistance (`desolator`)
    for ent in world.entities.values_mut() {
        ent.stance = crate::command::UnitStance::HoldFire;
    }
    world.set_player_sub_faction(1, None);
    world.set_player_sub_faction(2, Some("iraq".to_string()));
    let p1_victim = world.spawn_unit(1, "allied_gi", [30, 30], &loaded).unwrap();
    let p2_iraqi = world
        .spawn_unit(2, "soviet_conscript", [30, 31], &loaded)
        .unwrap();
    world.entities.get_mut(&p1_victim).unwrap().stance = crate::command::UnitStance::HoldFire;
    world.entities.get_mut(&p2_iraqi).unwrap().stance = crate::command::UnitStance::HoldFire;
    let rad_pos = world.entities[&p1_victim].position;
    world.radiation_zones.push(RadiationZone {
        position: rad_pos,
        radius: 5.0,
        dps: 40.0,
        remaining_secs: 5.0,
        is_toxic_cloud: false,
        owner_player_id: 0,
    });
    let p1_hp0 = world.entities[&p1_victim].health;
    let p2_hp0 = world.entities[&p2_iraqi].health;
    world.step(&loaded);
    let p1_loss = p1_hp0 - world.entities[&p1_victim].health;
    let p2_loss = p2_hp0 - world.entities[&p2_iraqi].health;
    assert!(p1_loss > 1.9);
    assert!(
        (p2_loss - p1_loss * 0.35).abs() < 0.05,
        "Iraq units should take only 35% radiation damage (65% resist): p1={p1_loss}, p2={p2_loss}"
    );
    world.radiation_zones.clear();

    // 8. Libya: +14% vehicle movement speed (`demolition_truck`)
    world.set_player_sub_faction(2, Some("libya".to_string()));
    let demo_id = world
        .spawn_unit(2, "demolition_truck", [16, 15], &loaded)
        .unwrap();
    assert!((world.entities[&demo_id].script_ctx.speed_multiplier - 1.14).abs() < 1e-4);

    // 9. Cuba: -15% infantry build cost (`terrorist`)
    world.set_player_sub_faction(2, Some("cuba".to_string()));
    let terr_def = loaded.units.get("terrorist").unwrap();
    assert_eq!(
        world.effective_country_unit_cost(2, terr_def, &loaded),
        ((terr_def.cost as f32) * 0.85).round() as u32
    );

    // 10. Yuri (Psi-Corps): -12% psychic/cloning cost & faster mind-control cooldown
    world.players.get_mut(&2).unwrap().faction = "yuri".to_string();
    world.set_player_sub_faction(2, Some("yuri".to_string()));
    let init_def = loaded.units.get("yuri_initiate").unwrap();
    assert_eq!(
        world.effective_country_unit_cost(2, init_def, &loaded),
        ((init_def.cost as f32) * 0.88).round() as u32
    );
    let yp_id = world
        .spawn_unit(2, "yuri_prime", [18, 15], &loaded)
        .unwrap();
    assert!(world.entities[&yp_id].buff_fire_rate_multiplier() > 1.20);

    // Deterministic AI / "random" country roll on LobbyLaunchSimConfig
    let det_a = resolve_deterministic_sub_faction(&loaded, "allies", None, true, 0x1234_5678, 2);
    let det_b = resolve_deterministic_sub_faction(&loaded, "allies", None, true, 0x1234_5678, 2);
    assert_eq!(det_a, det_b);
    assert!(det_a.is_some());
    let launch_cfg = LobbyLaunchSimConfig {
        slots: vec![
            LobbySlotSimSpec {
                player_id: 1,
                kind: LobbySlotKind::Human,
                faction: "allies".to_string(),
                sub_faction: Some("random".to_string()),
                team_id: 1,
                color_index: 1,
                spawn_index: Some(0),
                handicap_pct: 100,
            },
            LobbySlotSimSpec {
                player_id: 2,
                kind: LobbySlotKind::AiBot {
                    difficulty: AiDifficulty::Hard,
                    personality: AiPersonality::Balanced,
                },
                faction: "soviets".to_string(),
                sub_faction: None,
                team_id: 2,
                color_index: 2,
                spawn_index: Some(1),
                handicap_pct: 100,
            },
        ],
        starting_credits: 10_000,
        superweapons_enabled: true,
        crates_enabled: true,
        game_mode: GameMode::Standard,
        random_seed: 0xCAFE_F00D,
    };
    let launched = SimWorld::from_lobby_launch(&loaded, &loaded.entry_map, &launch_cfg);
    assert!(launched.players[&1].sub_faction.is_some());
    assert!(launched.players[&2].sub_faction.is_some());
}

#[test]
fn yuri_slave_miner_spawns_five_slaves_harvests_ore_respawns_and_frees_slaves_on_death() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();
    world.grid.ore_tiles.clear();
    world.grid.gem_tiles.clear();

    // Place rich ore and gem tiles right next to the Slave Miner
    for dx in -2..=2 {
        for dz in -2..=2 {
            let t = [14 + dx, 14 + dz];
            world.grid.ore_tiles.insert(t, 2_000);
            if dx == 2 && dz == 0 {
                world.grid.gem_tiles.insert(t);
            }
        }
    }

    let miner_id = world
        .spawn_unit(2, "yuri_slave_miner", [14, 14], &loaded)
        .unwrap();
    world.step(&loaded);

    let count_slaves_for = |w: &SimWorld, pid: u8| -> Vec<u64> {
        w.entities
            .values()
            .filter(|e| {
                e.health > 0.0
                    && e.player_id == pid
                    && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "yuri_slave")
            })
            .map(|e| e.id)
            .collect()
    };

    let initial_slaves = count_slaves_for(&world, 2);
    assert_eq!(
        initial_slaves.len(),
        5,
        "Active yuri_slave_miner must spawn 5 tethered yuri_slave workers"
    );

    // Clear miner's internal harvester_state so credit gain strictly proves slave worker deposits
    let start_credits = 1_000_u32;
    world.players.get_mut(&2).unwrap().credits = start_credits;
    world
        .players
        .get_mut(&2)
        .unwrap()
        .resources
        .insert("credits".to_string(), start_credits);
    world
        .players
        .get_mut(&2)
        .unwrap()
        .fractional_resources
        .insert("credits".to_string(), (start_credits as i64) << 32);

    // Step 4.5s (90 ticks): slaves walk to nearby ore/gems, shovel for 1.8s, and deposit +40/+80 credits
    for _ in 0..90 {
        if let Some(m) = world.entities.get_mut(&miner_id) {
            m.harvester_state = None;
        }
        world.step(&loaded);
    }
    assert!(
        world.players[&2].credits >= start_credits + 200,
        "5 slaves should harvest and deposit ore/gem credits directly into owner ({} -> {})",
        start_credits,
        world.players[&2].credits
    );

    // Kill 1 slave and verify 1 lost slave respawns after 8.0s (160 ticks)
    let victim_slave = count_slaves_for(&world, 2)[0];
    world.entities.get_mut(&victim_slave).unwrap().health = 0.0;
    world.step(&loaded);
    assert_eq!(count_slaves_for(&world, 2).len(), 4);

    for _ in 0..165 {
        world.step(&loaded);
    }
    assert_eq!(
        count_slaves_for(&world, 2).len(),
        5,
        "Slave Miner should respawn 1 lost slave after 8.0s"
    );

    // Kill the Slave Miner on behalf of Player 1 -> all 5 living slaves switch to Player 1 ("Freed Slaves")!
    {
        let m_mut = world.entities.get_mut(&miner_id).unwrap();
        m_mut.last_damaged_by_player = 1;
        m_mut.health = 0.0;
    }
    world.step(&loaded);
    world.step(&loaded);

    assert_eq!(count_slaves_for(&world, 2).len(), 0);
    let freed_slaves = count_slaves_for(&world, 1);
    assert_eq!(
        freed_slaves.len(),
        5,
        "Killing yuri_slave_miner must liberate all 5 living yuri_slaves to the killer's player"
    );
    for sid in freed_slaves {
        assert_eq!(
            world.entities[&sid].primary_weapon.as_deref(),
            Some("slave_shovel")
        );
    }
}

#[test]
fn crazy_ivan_and_chrono_ivan_timed_bomb_detonates_aoe_and_engineer_defuses() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();

    // 1. Crazy Ivan plants TimedBomb on Player 1 tank -> Friendly Engineer targets & defuses it!
    let p1_tank = world
        .spawn_unit(1, "grizzly_tank", [10, 10], &loaded)
        .unwrap();
    let p1_eng = world
        .spawn_unit(1, "allied_engineer", [12, 10], &loaded)
        .unwrap();
    let ivan = world
        .spawn_unit(2, "crazy_ivan", [10, 11], &loaded)
        .unwrap();

    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![ivan],
        target_entity_id: p1_tank,
    });
    for _ in 0..6 {
        world.step(&loaded);
    }
    let bomb = *world
        .timed_bombs
        .get(&p1_tank)
        .expect("Crazy Ivan hit must register TimedBomb in SimWorld.timed_bombs");
    assert!((bomb.damage - 420.0).abs() < 1e-3);
    assert!((bomb.blast_radius - 4.2).abs() < 1e-3);
    assert_eq!(bomb.planter_player_id, 2);

    // Order friendly Engineer to target bombed tank -> walks over and defuses
    world.entities.remove(&ivan);
    world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![p1_eng],
        target_entity_id: p1_tank,
    });
    for _ in 0..25 {
        world.step(&loaded);
    }
    assert!(
        !world.timed_bombs.contains_key(&p1_tank) && world.entities[&p1_tank].c4_remaining() == 0.0,
        "Friendly Engineer targeting bombed ally must defuse TimedBomb"
    );

    // 2. Chrono Ivan plants TimedBomb -> expires at 4.5s and detonates heavy 420 AoE blast!
    let c_ivan = world
        .spawn_unit(2, "chrono_ivan", [20, 11], &loaded)
        .unwrap();
    let doomed_bldg = world
        .spawn_building(1, "allied_power_plant", [20, 9], &loaded, false)
        .unwrap();
    let bystander = world.spawn_unit(1, "allied_gi", [21, 11], &loaded).unwrap();
    let bldg_hp0 = world.entities[&doomed_bldg].health;

    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![c_ivan],
        target_entity_id: doomed_bldg,
    });
    for _ in 0..6 {
        world.step(&loaded);
    }
    assert!(world.timed_bombs.contains_key(&doomed_bldg));
    world.entities.remove(&c_ivan);

    for _ in 0..95 {
        world.step(&loaded);
    }
    assert!(!world.timed_bombs.contains_key(&doomed_bldg));
    assert!(
        world.entities[&doomed_bldg].health <= bldg_hp0 - 419.0,
        "TimedBomb detonation should deal 420 direct damage to bombed structure"
    );
    assert!(
        !world.entities.contains_key(&bystander),
        "4.2m AoE explosion should kill adjacent bystander infantry"
    );
}

#[test]
fn weather_storm_cell_strikes_ten_staggered_bolts_jams_radar_and_jets_glide_onto_pads() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();

    for p in world.players.values_mut() {
        p.power_produced = 2_000;
        p.power_consumed = 100;
        p.support_power_cooldowns.clear();
    }

    let _wc = world
        .spawn_building(1, "allied_weather_controller", [4, 4], &loaded, false)
        .unwrap();
    let enemy_cy = world
        .spawn_building(2, "soviet_conyard", [20, 20], &loaded, false)
        .unwrap();
    let enemy_tank = world
        .spawn_unit(2, "rhino_tank", [19, 20], &loaded)
        .unwrap();
    let storm_center = world.entities[&enemy_cy].position;
    let cy_hp0 = world.entities[&enemy_cy].health;

    world.push_command(GameCommand::CastSupportPower {
        player_id: 1,
        power_id: "weather_lightning_storm".to_string(),
        source_pos: None,
        target_pos: storm_center,
    });

    let mut total_lightning_events = 0usize;
    world.step(&loaded);
    total_lightning_events += world
        .combat_events
        .iter()
        .filter(|ev| ev.weapon_id == "weather_lightning_storm")
        .count();

    assert_eq!(world.weather_storms.len(), 1);
    assert!((world.weather_storms[0].duration_secs - 6.0).abs() < 1e-4);
    assert!((world.weather_storms[0].radius - 7.5).abs() < 1e-4);
    assert_eq!(
        world.weather_storms[0].total_bolts,
        WEATHER_STORM_TOTAL_BOLTS
    );
    assert!(world.is_player_radar_jammed(2));
    assert!(!world.is_player_radar_jammed(1));
    assert!(
        (1..10).contains(&total_lightning_events),
        "Weather Storm bolts must be staggered over 6.0s rather than firing all 10 on tick 1"
    );

    // Step across 6.0s (122 ticks): verify all 10 staggered bolts strike within 7.5m radius
    for _ in 0..122 {
        world.step(&loaded);
        for ev in &world.combat_events {
            if ev.weapon_id == "weather_lightning_storm" {
                total_lightning_events += 1;
                let dx = ev.target_pos[0] - storm_center[0];
                let dz = ev.target_pos[2] - storm_center[2];
                let dist = (dx * dx + dz * dz).sqrt();
                assert!(dist <= 7.6, "Bolt landed outside 7.5m storm radius: {dist}");
            }
        }
    }
    assert_eq!(
        total_lightning_events, 10,
        "WeatherStormCell must strike exact 10 deterministic staggered lightning bolts over 6.0s"
    );
    assert!(world.weather_storms.is_empty());
    assert!(
        world.is_player_radar_jammed(2),
        "10s hostile radar jam should still be active right after 6.0s storm ends"
    );
    let cy_hp_after = world
        .entities
        .get(&enemy_cy)
        .map(|e| e.health)
        .unwrap_or(0.0);
    let tank_hp_after = world
        .entities
        .get(&enemy_tank)
        .map(|e| e.health)
        .unwrap_or(0.0);
    assert!(cy_hp_after < cy_hp0 || tank_hp_after < 400.0);

    // Step remaining 4.2s -> 10.0s radar jam expires cleanly
    for _ in 0..85 {
        world.step(&loaded);
    }
    assert!(!world.is_player_radar_jammed(2));

    // Physical Airpad Reload: returning `black_eagle` & `harrier` smoothly descend onto distinct pad coords
    world.set_player_sub_faction(1, Some("korea".to_string()));
    let af_id = world
        .spawn_building(1, "allied_airforce_command", [8, 8], &loaded, false)
        .unwrap();
    let pad0 = airfield_pad_world_coords(&world, af_id, 0).unwrap();
    let pad1 = airfield_pad_world_coords(&world, af_id, 1).unwrap();
    let jet0 = world
        .spawn_unit(1, "black_eagle", [10, 10], &loaded)
        .unwrap();
    let jet1 = world.spawn_unit(1, "harrier", [11, 10], &loaded).unwrap();
    world.entities.get_mut(&jet0).unwrap().current_ammo = 0;
    world.entities.get_mut(&jet1).unwrap().current_ammo = 0;

    let mut observed_mid_glide = false;
    for _ in 0..80 {
        world.step(&loaded);
        let y0 = world.entities[&jet0].position[1];
        if y0 > pad0[1] + 0.3 && y0 < 4.5 {
            observed_mid_glide = true;
        }
    }
    assert!(
        observed_mid_glide,
        "Returning strike aircraft should smoothly descend through intermediate glide altitudes"
    );
    let pos0 = world.entities[&jet0].position;
    let pos1 = world.entities[&jet1].position;
    assert!(((pos0[0] - pad0[0]).hypot(pos0[2] - pad0[2])) < 0.25);
    assert!(((pos1[0] - pad1[0]).hypot(pos1[2] - pad1[2])) < 0.25);
    assert!((pos0[1] - pad0[1]).abs() < 0.15);
    assert_eq!(
        world.entities[&jet0].current_ammo,
        world.entities[&jet0].max_ammo
    );
}
