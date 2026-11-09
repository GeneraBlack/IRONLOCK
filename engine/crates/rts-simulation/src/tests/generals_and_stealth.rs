use super::load_ra2_mod;
use crate::command::GameCommand;
use crate::world::{EntityKind, SimWorld};

    #[test]
    fn building_upgrade_survivor_bailout_salvage_and_auto_c4_defusal_work() {
        let loaded = load_ra2_mod();
        let mut world = SimWorld::from_mod(&loaded);
        world.players.get_mut(&1).unwrap().credits = 5000;
        world.players.get_mut(&2).unwrap().credits = 5000;

        // 1. Test In-Place Building Upgrade (`GameCommand::UpgradeBuilding`)
        let pp_id = world
            .spawn_building(1, "allied_power_plant", [8, 8], &loaded, false)
            .unwrap();
        let power_before = world.players.get(&1).unwrap().power_produced;
        world.push_command(GameCommand::UpgradeBuilding {
            player_id: 1,
            building_id: pp_id,
        });
        world.step(&loaded);
        assert!(
            world.entities.get(&pp_id).unwrap().upgrading_to.is_some(),
            "UpgradeBuilding should start in-place upgrade timer on allied_power_plant"
        );
        for _ in 0..80 {
            world.step(&loaded);
        }
        let pp_ent = world.entities.get(&pp_id).unwrap();
        assert_eq!(
            pp_ent.kind,
            crate::world::EntityKind::Building {
                building_id: "allied_power_plant_overcharged".to_string()
            },
            "allied_power_plant should transform in-place into allied_power_plant_overcharged"
        );
        assert!(
            world.players.get(&1).unwrap().power_produced >= power_before + 100,
            "Overcharged power plant should boost player power_produced"
        );
        assert!(
            world
                .owned_building_ids(1)
                .contains("allied_power_plant"),
            "Upgraded power plant should still satisfy allied_power_plant tech prerequisites"
        );

        // 2. Test Surviving Crew on Building Sell & Vehicle Destruction + Salvage Scrap Drop & Pickup
        let units_before_sell = world
            .entities
            .values()
            .filter(|e| {
                e.player_id == 1
                    && matches!(
                        &e.kind,
                        crate::world::EntityKind::Unit { unit_id } if unit_id == "allied_engineer"
                    )
            })
            .count();
        world.push_command(GameCommand::SellBuilding {
            player_id: 1,
            building_id: pp_id,
        });
        world.step(&loaded);
        let units_after_sell = world
            .entities
            .values()
            .filter(|e| {
                e.player_id == 1
                    && matches!(
                        &e.kind,
                        crate::world::EntityKind::Unit { unit_id } if unit_id == "allied_engineer"
                    )
            })
            .count();
        assert_eq!(
            units_after_sell,
            units_before_sell + 1,
            "Selling a completed building with survivor_unit should spawn a surviving crewman"
        );

        // Force 100% survivor_chance on a copy of loaded mod to test deterministic combat bailout + salvage drop
        let mut loaded_100 = load_ra2_mod();
        loaded_100
            .units
            .get_mut("rhino_tank")
            .unwrap()
            .death_effect
            .survivor_chance = 1.0;
        let doomed_rhino = world
            .spawn_unit(2, "rhino_tank", [16, 16], &loaded_100)
            .unwrap();
        let killer_tank = world
            .spawn_unit(1, "grizzly_tank", [16, 17], &loaded_100)
            .unwrap();
        world
            .entities
            .get_mut(&doomed_rhino)
            .unwrap()
            .health = 10.0;
        world.push_command(GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![killer_tank],
            target_entity_id: doomed_rhino,
        });
        for _ in 0..5 {
            world.step(&loaded_100);
        }
        assert!(
            !world.entities.contains_key(&doomed_rhino),
            "Doomed Rhino Tank should be destroyed"
        );
        let has_conscript_survivor = world.entities.values().any(|e| {
            e.player_id == 2
                && matches!(
                    &e.kind,
                    crate::world::EntityKind::Unit { unit_id } if unit_id == "soviet_conscript"
                )
        });
        assert!(
            has_conscript_survivor,
            "Destroyed Rhino Tank with survivor_chance=1.0 should spawn a surviving soviet_conscript crewman"
        );

        // Killer tank is within 1 tile of [16, 16], so it either already picked up the SalvageScrap crate or we move onto it
        let scrap_pos = world.grid.tile_to_world([16, 16]);
        world.push_command(GameCommand::MoveUnits {
            player_id: 1,
            entity_ids: vec![killer_tank],
            target_pos: scrap_pos,
        });
        for _ in 0..8 {
            world.step(&loaded_100);
        }
        assert!(
            world.entities.get(&killer_tank).unwrap().salvage_upgraded,
            "Vehicle collecting SalvageScrap crate from destroyed tank should gain salvage_upgraded"
        );
        assert!(
            world.last_event_world_pos.is_some(),
            "World should record last_event_world_pos on combat destruction"
        );

        // 3. Test Automated Engineer C4 Defusal (without explicit player AttackTarget order!)
        let c4_b = world
            .spawn_building(1, "allied_barracks", [6, 18], &loaded, false)
            .unwrap();
        world.entities.get_mut(&c4_b).unwrap().set_c4_bomb(3.2, 1500.0, 0.0, 2);
        let _idle_eng = world
            .spawn_unit(1, "allied_engineer", [6, 21], &loaded)
            .unwrap();
        for _ in 0..20 {
            world.step(&loaded);
        }
        assert_eq!(
            world.entities.get(&c4_b).unwrap().c4_remaining(),
            0.0,
            "Idle friendly Engineer within range should automatically move to and defuse C4 on friendly building"
        );
    }

    #[test]
    fn commander_promotions_doctrines_tunnel_network_gap_generator_ore_purifier_and_subterranean_work() {
        let loaded = load_ra2_mod();
        let mut world = SimWorld::from_mod(&loaded);
        world.players.get_mut(&1).unwrap().credits = 10_000;
        world.players.get_mut(&2).unwrap().credits = 10_000;

        // 1. Commander XP, Promotion Points, Doctrine Upgrade & Kill Bounty + Battlefield Scars
        // Without a promotion point, researching `allied_doctrine_bounty` must be rejected
        assert_eq!(world.players.get(&1).unwrap().promotion_points, 0);
        world.push_command(GameCommand::ResearchUpgrade {
            player_id: 1,
            upgrade_id: "allied_doctrine_bounty".to_string(),
        });
        world.step(&loaded);
        assert!(
            world.players.get(&1).unwrap().active_upgrade.is_none(),
            "Doctrine upgrade requiring promotion point must not start at 0 PP"
        );

        // Award 160 XP -> Rank 2 (+1 PP)
        world.players.get_mut(&1).unwrap().add_commander_xp(160.0);
        assert_eq!(world.players.get(&1).unwrap().commander_rank, 2);
        assert_eq!(world.players.get(&1).unwrap().promotion_points, 1);

        world.push_command(GameCommand::ResearchUpgrade {
            player_id: 1,
            upgrade_id: "allied_doctrine_bounty".to_string(),
        });
        world.step(&loaded);
        assert_eq!(
            world.players.get(&1).unwrap().promotion_points,
            0,
            "Researching doctrine upgrade should consume 1 promotion point"
        );
        // Complete research
        for _ in 0..210 {
            world.step(&loaded);
        }
        assert!(
            world
                .players
                .get(&1)
                .unwrap()
                .researched_upgrades
                .contains("allied_doctrine_bounty")
        );

        // Kill an enemy Rhino Tank (cost 900) and verify 25% kill bounty (+225 credits) and Battlefield Scar
        let enemy_rhino = world.spawn_unit(2, "rhino_tank", [14, 14], &loaded).unwrap();
        world.entities.get_mut(&enemy_rhino).unwrap().health = 10.0;
        let allied_tank = world
            .spawn_unit(1, "grizzly_tank", [14, 15], &loaded)
            .unwrap();
        let credits_before_kill = world.players.get(&1).unwrap().credits;
        let scars_before = world.battlefield_scars.len();
        world.push_command(GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![allied_tank],
            target_entity_id: enemy_rhino,
        });
        for _ in 0..5 {
            world.step(&loaded);
        }
        assert!(
            world.players.get(&1).unwrap().credits >= credits_before_kill + 225,
            "Killing a 900-credit Rhino Tank with 25% bounty doctrine should award +225 credits"
        );
        assert!(
            world.battlefield_scars.len() > scars_before,
            "Destroyed vehicle should leave a persistent SimBattlefieldScar wreck"
        );

        // 2. Shared Tunnel Network: enter Bunker A at [4, 18] and unload from Bunker B at [24, 18]
        let bunker_a = world
            .spawn_building(2, "soviet_battle_bunker", [4, 18], &loaded, false)
            .unwrap();
        let bunker_b = world
            .spawn_building(2, "soviet_battle_bunker", [24, 18], &loaded, false)
            .unwrap();
        let conscript = world
            .spawn_unit(2, "soviet_conscript", [4, 20], &loaded)
            .unwrap();
        world.push_command(GameCommand::LoadIntoCarrier {
            player_id: 2,
            passenger_ids: vec![conscript],
            carrier_id: bunker_a,
        });
        for _ in 0..15 {
            world.step(&loaded);
        }
        assert_eq!(
            world.entities.get(&conscript).unwrap().transported_by,
            Some(bunker_a),
            "Conscript should board Tunnel Bunker A"
        );
        assert!(
            world
                .players
                .get(&2)
                .unwrap()
                .shared_tunnel_passengers
                .contains(&conscript),
            "Boarding a tunnel_network building should register unit in shared_tunnel_passengers"
        );

        // Unload at Bunker B on the other side of the map!
        world.push_command(GameCommand::UnloadCarrier {
            player_id: 2,
            carrier_ids: vec![bunker_b],
        });
        world.step(&loaded);
        let c_ent = world.entities.get(&conscript).unwrap();
        assert!(c_ent.transported_by.is_none());
        assert!(
            (c_ent.tile_origin[0] - 25).abs() <= 3 && (c_ent.tile_origin[1] - 20).abs() <= 3,
            "Conscript unloaded from Tunnel Bunker B should exit near [24, 18], got {:?}",
            c_ent.tile_origin
        );

        // 3. Gap Generator cloak field & CloakStealth crate
        let _pp = world
            .spawn_building(1, "allied_power_plant", [8, 8], &loaded, false)
            .unwrap();
        let _gap = world
            .spawn_building(1, "allied_gap_generator", [10, 10], &loaded, false)
            .unwrap();
        let nearby_gi = world.spawn_unit(1, "allied_gi", [11, 11], &loaded).unwrap();
        assert!(
            world.is_entity_cloaked(world.entities.get(&nearby_gi).unwrap(), &loaded),
            "Friendly unit inside powered Allied Gap Generator radius should be cloaked"
        );

        // 4. Ore Purifier (+25% ore refine multiplier)
        assert!(
            (world.player_ore_refine_multiplier(1, &loaded) - 1.0).abs() < 1e-4,
            "Base ore refine multiplier without purifier should be 1.0"
        );
        let _purifier = world
            .spawn_building(1, "allied_ore_purifier", [12, 8], &loaded, false)
            .unwrap();
        assert!(
            (world.player_ore_refine_multiplier(1, &loaded) - 1.25).abs() < 1e-4,
            "Allied Ore Purifier should boost ore refine multiplier to 1.25"
        );

        // 5. Subterranean APC burrowing under obstacles
        let sub_apc = world
            .spawn_unit(2, "soviet_sub_apc", [6, 24], &loaded)
            .unwrap();
        let dest_world = world.grid.tile_to_world([18, 24]);
        world.push_command(GameCommand::MoveUnits {
            player_id: 2,
            entity_ids: vec![sub_apc],
            target_pos: dest_world,
        });
        world.step(&loaded);
        let sub_ent = world.entities.get(&sub_apc).unwrap();
        assert!(
            sub_ent.burrowed && sub_ent.position[1] < 0.0,
            "Moving subterranean unit should burrow below ground elevation"
        );
    }

    #[test]
    fn cloning_vats_capture_bonus_garrison_clear_force_shield_and_crusher_work() {
        let loaded = load_ra2_mod();
        let mut world = SimWorld::from_mod(&loaded);

        // 1. Cloning Vats: training an infantry unit in a Barracks while owning Cloning Vats spawns a free twin,
        //    and sending an infantry unit into Cloning Vats recycles it for 75% of its cost.
        world.spawn_building(2, "soviet_tesla_reactor", [1, 4], &loaded, false);
        let barracks = world
            .spawn_building(2, "soviet_barracks", [4, 4], &loaded, false)
            .unwrap();
        let vats = world
            .spawn_building(2, "yuri_cloning_vats", [8, 4], &loaded, false)
            .unwrap();
        let conscripts_before = world
            .entities
            .values()
            .filter(|e| {
                e.player_id == 2
                    && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "soviet_conscript")
            })
            .count();
        world
            .entities
            .get_mut(&barracks)
            .unwrap()
            .production_queue
            .push(crate::world::ProductionItem {
                unit_id: "soviet_conscript".to_string(),
                remaining_secs: 0.02,
                total_secs: 2.0,
            });
        world.step(&loaded);
        let conscripts_after: Vec<u64> = world
            .entities
            .values()
            .filter(|e| {
                e.player_id == 2
                    && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "soviet_conscript")
            })
            .map(|e| e.id)
            .collect();
        assert_eq!(
            conscripts_after.len(),
            conscripts_before + 2,
            "Owning Cloning Vats should spawn a free twin infantry when a Barracks completes training"
        );

        let recycle_unit = *conscripts_after.last().unwrap();
        let credits_before_recycle = world.players.get(&2).unwrap().credits;
        world.push_command(GameCommand::LoadIntoCarrier {
            player_id: 2,
            passenger_ids: vec![recycle_unit],
            carrier_id: vats,
        });
        for _ in 0..10 {
            world.step(&loaded);
        }
        assert!(
            !world.entities.contains_key(&recycle_unit),
            "Infantry sent into Cloning Vats should be recycled"
        );
        assert_eq!(
            world.players.get(&2).unwrap().credits,
            credits_before_recycle + 75,
            "Recycling a 100-credit Conscript in Cloning Vats should refund 75 credits (75%)"
        );

        // 2. Tech Oil Derrick one-time Engineer Capture Cash Bonus (+1000 credits)
        let derrick = world
            .spawn_building(0, "tech_oil_derrick", [14, 4], &loaded, false)
            .unwrap();
        let eng = world
            .spawn_unit(1, "allied_engineer", [14, 6], &loaded)
            .unwrap();
        let p1_credits_before = world.players.get(&1).unwrap().credits;
        world.push_command(GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![eng],
            target_entity_id: derrick,
        });
        for _ in 0..8 {
            world.step(&loaded);
        }
        assert_eq!(world.entities.get(&derrick).unwrap().player_id, 1);
        assert!(
            world.players.get(&1).unwrap().credits >= p1_credits_before + 1000,
            "Capturing Tech Oil Derrick for the first time should award +1000 capture cash bonus"
        );

        // 3. Garrison-Clearing Weapon (`rad_beam` with `clears_garrison = true`) kills garrisoned passengers inside a bunker
        let bunker = world
            .spawn_building(1, "allied_pillbox", [14, 14], &loaded, false)
            .unwrap();
        let gi = world
            .spawn_unit(1, "allied_gi", [14, 15], &loaded)
            .unwrap();
        world.entities.get_mut(&gi).unwrap().transported_by = Some(bunker);
        world
            .entities
            .get_mut(&bunker)
            .unwrap()
            .passenger_ids
            .push(gi);
        let desolator = world
            .spawn_unit(2, "desolator", [14, 18], &loaded)
            .unwrap();
        world.push_command(GameCommand::AttackTarget {
            player_id: 2,
            attacker_ids: vec![desolator],
            target_entity_id: bunker,
        });
        for _ in 0..10 {
            world.step(&loaded);
        }
        assert!(
            world
                .entities
                .get(&bunker)
                .unwrap()
                .passenger_ids
                .is_empty(),
            "Desolator's rad_beam (clears_garrison = true) should kill garrisoned infantry inside the pillbox"
        );
        assert!(
            world.entities.get(&bunker).unwrap().health > 0.0,
            "Garrison-clearing weapon should leave the pillbox structure standing"
        );

        // 4. Force Shield support power grants invulnerability to friendly buildings in radius & triggers temporary power outage
        let cy_pos = world
            .entities
            .values()
            .find(|e| e.player_id == 1 && matches!(e.kind, EntityKind::Building { .. }))
            .unwrap()
            .position;
        world.push_command(GameCommand::CastSupportPower {
            player_id: 1,
            power_id: "allied_force_shield".to_string(),
            source_pos: None,
            target_pos: cy_pos,
        });
        world.step(&loaded);
        let shielded = world
            .entities
            .values()
            .any(|e| e.player_id == 1 && e.invulnerable_remaining() > 10.0);
        assert!(
            shielded,
            "Force Shield support power should make friendly buildings in radius invulnerable"
        );
        assert!(
            world.power_outage_timers.get(&1).copied().unwrap_or(0.0) > 0.0,
            "Force Shield should cause a temporary power outage tradeoff"
        );

        // 5. Heavy Tank Crushing: Apocalypse Tank crushes Concrete Wall (`crushable = true`) and regular GI,
        //    but cannot crush heavy-armored Tesla Trooper (`uncrushable = true`)
        let wall = world
            .spawn_building(2, "concrete_wall", [20, 20], &loaded, false)
            .unwrap();
        let wall_pos = world.entities.get(&wall).unwrap().position;
        let apoc = world
            .spawn_unit(1, "apocalypse_tank", [20, 21], &loaded)
            .unwrap();
        world.entities.get_mut(&apoc).unwrap().position = wall_pos;
        let tesla = world
            .spawn_unit(2, "tesla_trooper", [20, 21], &loaded)
            .unwrap();
        world.entities.get_mut(&tesla).unwrap().position = wall_pos;
        world.step(&loaded);
        assert!(
            !world.entities.contains_key(&wall),
            "Heavy crusher vehicle (Apocalypse Tank) should crush crushable concrete_wall"
        );
        assert!(
            world.entities.contains_key(&tesla),
            "Uncrushable heavy infantry (Tesla Trooper) should survive tank crushing"
        );
        assert!(
            world.entities.get(&apoc).unwrap().experience >= 50.0,
            "Crushing enemy entities should award veterancy XP to the crusher vehicle"
        );

        // 6. Superweapon Global Reveal & Active Warnings
        let _silo = world
            .spawn_building(2, "yuri_psychic_dominator", [26, 26], &loaded, false)
            .unwrap();
        let warnings = world.active_superweapon_warnings(&loaded);
        assert!(
            warnings.iter().any(|(pid, _, _)| *pid == 2),
            "Completed Soviet Missile Silo should appear in active_superweapon_warnings"
        );
        let vis_p1 = world.compute_visibility_map(1, &loaded);
        assert_eq!(
            world.tile_visibility_in_map(&vis_p1, [26, 26]),
            crate::world::TileVisibility::Visible,
            "Enemy superweapon tile should be globally revealed to Player 1"
        );
    }

    #[test]
    fn ifv_weapon_adaptation_prism_chaining_kamikaze_yuri_superweapons_and_power_toggle_work() {
        let loaded = load_ra2_mod();
        let mut world = SimWorld::from_mod(&loaded);
        for pid in [1u8, 2u8] {
            for _ in 0..3 {
                world.spawn_building(pid, "allied_power_plant", [2, 2], &loaded, false);
            }
        }

        // 1. Multigunner IFV Passenger Weapon Adaptation & Engineer Repair Mode
        let ifv = world.spawn_unit(1, "ifv", [10, 10], &loaded).unwrap();
        assert_eq!(
            world
                .ifv_passenger_mode_label(world.entities.get(&ifv).unwrap(), &loaded)
                .as_deref(),
            Some("Default Missile Turret")
        );
        let eng = world
            .spawn_unit(1, "allied_engineer", [10, 11], &loaded)
            .unwrap();
        world.entities.get_mut(&eng).unwrap().transported_by = Some(ifv);
        world.entities.get_mut(&ifv).unwrap().passenger_ids.push(eng);
        assert!(
            world
                .ifv_passenger_mode_label(world.entities.get(&ifv).unwrap(), &loaded)
                .unwrap()
                .contains("Mobile Repair Arm"),
            "Engineer inside IFV should switch IFV to Mobile Repair Arm mode"
        );
        let dmg_tank = world
            .spawn_unit(1, "grizzly_tank", [10, 12], &loaded)
            .unwrap();
        world.entities.get_mut(&dmg_tank).unwrap().health = 200.0;
        for _ in 0..10 {
            world.step(&loaded);
        }
        assert!(
            world.entities.get(&dmg_tank).unwrap().health > 200.0,
            "IFV with Engineer passenger should automatically repair nearby damaged vehicles"
        );

        // Swap passenger to Desolator -> IFV fires rad_beam and leaves radiation zone
        world.entities.get_mut(&ifv).unwrap().passenger_ids.clear();
        let deso = world.spawn_unit(1, "desolator", [10, 11], &loaded).unwrap();
        world.entities.get_mut(&deso).unwrap().transported_by = Some(ifv);
        world
            .entities
            .get_mut(&ifv)
            .unwrap()
            .passenger_ids
            .push(deso);
        let target_tank = world
            .spawn_unit(2, "rhino_tank", [10, 15], &loaded)
            .unwrap();
        world.push_command(GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![ifv],
            target_entity_id: target_tank,
        });
        let rad_before = world.radiation_zones.len();
        world.step(&loaded);
        assert!(
            world.radiation_zones.len() > rad_before,
            "IFV carrying a Desolator should fire rad_beam and create a radiation zone"
        );

        // 2. Cooperative Prism Tower Beam Chaining (`prism_support = true`)
        let tower_main = world
            .spawn_building(1, "allied_prism_tower", [16, 16], &loaded, false)
            .unwrap();
        let tower_sup = world
            .spawn_building(1, "allied_prism_tower", [17, 16], &loaded, false)
            .unwrap();
        let prism_victim = world
            .spawn_unit(2, "apocalypse_tank", [16, 20], &loaded)
            .unwrap();
        world.push_command(GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![tower_main],
            target_entity_id: prism_victim,
        });
        world.step(&loaded);
        assert!(
            world.entities.get(&tower_sup).unwrap().weapon_cooldown > 0.0,
            "Supporting Prism Tower should link its beam into the firing tower and enter support cooldown"
        );
        assert!(
            world.entities.get(&tower_main).unwrap().power_charge_remaining() > 0.0,
            "Firing Prism Tower should receive prism chain charge boost"
        );

        // 3. Kamikaze Demolition Truck (`kamikaze = true`) self-destructs on attack impact
        let demo_truck = world
            .spawn_unit(2, "demolition_truck", [22, 10], &loaded)
            .unwrap();
        let enemy_bunker = world
            .spawn_building(1, "allied_pillbox", [22, 12], &loaded, false)
            .unwrap();
        let bunker_hp_before = world.entities.get(&enemy_bunker).unwrap().health;
        world.push_command(GameCommand::AttackTarget {
            player_id: 2,
            attacker_ids: vec![demo_truck],
            target_entity_id: enemy_bunker,
        });
        world.step(&loaded);
        assert!(
            !world.entities.contains_key(&demo_truck),
            "Kamikaze Demolition Truck should detonate and destroy itself on attack"
        );
        assert!(
            world.entities.get(&enemy_bunker).unwrap().health < bunker_hp_before - 200.0,
            "Kamikaze Demolition Truck should deal massive demolition damage"
        );

        // 4. Yuri Superweapons: Genetic Mutator & Psychic Dominator
        let mutator = world
            .spawn_building(2, "yuri_genetic_mutator", [6, 22], &loaded, false)
            .unwrap();
        let gi_victim = world
            .spawn_unit(1, "allied_gi", [14, 22], &loaded)
            .unwrap();
        let gi_pos = world.entities.get(&gi_victim).unwrap().position;
        world.push_command(GameCommand::CastTargetedAbility {
            player_id: 2,
            caster_ids: vec![mutator],
            target_pos: gi_pos,
            target_entity_id: None,
        });
        world.step(&loaded);
        assert!(
            !world.entities.contains_key(&gi_victim),
            "Genetic Mutator should replace infantry in radius"
        );
        let has_brute = world.entities.values().any(|e| {
            e.player_id == 2
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "yuri_brute")
        });
        assert!(
            has_brute,
            "Genetic Mutator should spawn friendly yuri_brute units in place of mutated infantry"
        );

        let dominator = world
            .spawn_building(2, "yuri_psychic_dominator", [6, 26], &loaded, false)
            .unwrap();
        let tank_victim = world
            .spawn_unit(1, "grizzly_tank", [18, 26], &loaded)
            .unwrap();
        let tank_pos = world.entities.get(&tank_victim).unwrap().position;
        world.push_command(GameCommand::CastTargetedAbility {
            player_id: 2,
            caster_ids: vec![dominator],
            target_pos: tank_pos,
            target_entity_id: None,
        });
        world.step(&loaded);
        assert_eq!(
            world.entities.get(&tank_victim).unwrap().player_id,
            2,
            "Psychic Dominator should permanently mind-control enemy units in target radius"
        );

        // 5. Individual Building Power Toggle (`GameCommand::ToggleBuildingPower`)
        let consumed_before = world.players.get(&2).unwrap().power_consumed;
        world.push_command(GameCommand::ToggleBuildingPower {
            player_id: 2,
            building_ids: vec![dominator],
        });
        world.step(&loaded);
        assert!(
            world.entities.get(&dominator).unwrap().power_disabled,
            "ToggleBuildingPower should set power_disabled = true on the building"
        );
        assert_eq!(
            world.players.get(&2).unwrap().power_consumed,
            consumed_before - 200,
            "Disabling power on Psychic Dominator (-200 power) should reduce player power_consumed by 200"
        );
    }

    #[test]
    fn battle_fortress_fireports_crusher_industrial_plant_grinder_chaos_magnetron_and_carrier_drones_work()
    {
        let loaded = load_ra2_mod();
        let mut world = SimWorld::from_mod(&loaded);
        let start_ids: Vec<u64> = world.entities.keys().copied().collect();
        for id in start_ids {
            if let Some(e) = world.entities.get(&id) {
                if matches!(e.kind, EntityKind::Unit { .. }) {
                    world.entities.remove(&id);
                }
            }
        }

        // 1. Battle Fortress Mobile Bunker Fireports (`passengers_can_fire = true`) & Vehicle Crushing (`crushes_vehicles = true`)
        let fortress = world
            .spawn_unit(1, "battle_fortress", [8, 8], &loaded)
            .unwrap();
        let sniper = world.spawn_unit(1, "sniper", [8, 9], &loaded).unwrap();
        world.entities.get_mut(&sniper).unwrap().transported_by = Some(fortress);
        world
            .entities
            .get_mut(&fortress)
            .unwrap()
            .passenger_ids
            .push(sniper);
        let enemy_conscript = world
            .spawn_unit(2, "soviet_conscript", [12, 8], &loaded)
            .unwrap();
        let conscript_hp_before = world.entities.get(&enemy_conscript).unwrap().health;
        world.step(&loaded);
        assert!(
            world.entities.get(&sniper).unwrap().weapon_cooldown > 0.0,
            "Sniper passenger inside Battle Fortress should fire through mobile fireports"
        );
        let conscript_hp_after = world
            .entities
            .get(&enemy_conscript)
            .map(|e| e.health)
            .unwrap_or(0.0);
        assert!(
            conscript_hp_after < conscript_hp_before - 80.0,
            "Enemy infantry should take sniper damage from passenger inside Battle Fortress"
        );

        // Battle Fortress crushes an enemy tank (`crushes_vehicles = true`)
        let enemy_tank = world.spawn_unit(2, "rhino_tank", [9, 8], &loaded).unwrap();
        let tank_pos = world.entities.get(&enemy_tank).unwrap().position;
        world.push_command(GameCommand::MoveUnits {
            player_id: 1,
            entity_ids: vec![fortress],
            target_pos: [tank_pos[0] + 2.0, tank_pos[1], tank_pos[2]],
        });
        for _ in 0..8 {
            world.step(&loaded);
        }
        assert!(
            !world.entities.contains_key(&enemy_tank),
            "Battle Fortress (crushes_vehicles = true) should crush enemy Rhino Tank"
        );

        // 2. Soviet Industrial Plant (-25% vehicle cost/time) & Yuri Grinder (80% vehicle/unit recycle)
        let _soviet_reactor = world
            .spawn_building(2, "soviet_nuclear_reactor", [2, 20], &loaded, false)
            .unwrap();
        let _soviet_wf = world
            .spawn_building(2, "soviet_war_factory", [6, 20], &loaded, false)
            .unwrap();
        let _ind_plant = world
            .spawn_building(2, "soviet_industrial_plant", [10, 20], &loaded, false)
            .unwrap();
        world.players.get_mut(&2).unwrap().credits = 2000;
        world.push_command(GameCommand::ProduceUnit {
            player_id: 2,
            producer_entity_id: None,
            unit_id: "rhino_tank".to_string(),
        });
        world.step(&loaded);
        assert_eq!(
            world.players.get(&2).unwrap().credits,
            2000 - 675,
            "Soviet Industrial Plant should reduce 900-credit Rhino Tank cost by 25% to 675 credits"
        );

        let grinder = world
            .spawn_building(2, "yuri_grinder", [14, 20], &loaded, false)
            .unwrap();
        let scrap_tank = world
            .spawn_unit(2, "rhino_tank", [14, 22], &loaded)
            .unwrap();
        let credits_before_grind = world.players.get(&2).unwrap().credits;
        world.push_command(GameCommand::LoadIntoCarrier {
            player_id: 2,
            passenger_ids: vec![scrap_tank],
            carrier_id: grinder,
        });
        world.step(&loaded);
        assert!(
            !world.entities.contains_key(&scrap_tank),
            "Tank ordered into Yuri Grinder should be recycled"
        );
        assert_eq!(
            world.players.get(&2).unwrap().credits,
            credits_before_grind + 720,
            "Yuri Grinder should refund 80% (720 credits) of a 900-credit Rhino Tank"
        );

        // 3. Chaos Drone Frenzy (`confuse_duration_secs`) & Magnetron Tractor Beam (`tractor_pull_distance`)
        let chaos = world
            .spawn_unit(2, "chaos_drone", [20, 12], &loaded)
            .unwrap();
        let victim_tank = world
            .spawn_unit(1, "grizzly_tank", [22, 12], &loaded)
            .unwrap();
        world.push_command(GameCommand::AttackTarget {
            player_id: 2,
            attacker_ids: vec![chaos],
            target_entity_id: victim_tank,
        });
        world.step(&loaded);
        assert!(
            world.entities.get(&victim_tank).unwrap().confused_remaining() > 4.0,
            "Chaos Drone gas should apply confused_timer to hit enemy units"
        );

        let magnetron = world.spawn_unit(2, "magnetron", [20, 18], &loaded).unwrap();
        let pull_target = world
            .spawn_unit(1, "grizzly_tank", [26, 18], &loaded)
            .unwrap();
        let dist_before = {
            let m_pos = world.entities.get(&magnetron).unwrap().position;
            let t_pos = world.entities.get(&pull_target).unwrap().position;
            ((m_pos[0] - t_pos[0]).powi(2) + (m_pos[2] - t_pos[2]).powi(2)).sqrt()
        };
        world.push_command(GameCommand::AttackTarget {
            player_id: 2,
            attacker_ids: vec![magnetron],
            target_entity_id: pull_target,
        });
        world.step(&loaded);
        let dist_after = {
            let m_pos = world.entities.get(&magnetron).unwrap().position;
            let t_pos = world.entities.get(&pull_target).unwrap().position;
            ((m_pos[0] - t_pos[0]).powi(2) + (m_pos[2] - t_pos[2]).powi(2)).sqrt()
        };
        assert!(
            dist_after < dist_before - 2.0,
            "Magnetron tractor beam should pull enemy vehicle closer ({dist_before} -> {dist_after})"
        );

        // 4. Allied Aircraft Carrier Hornet Squadron (`spawns_drone_unit = "hornet_fighter"`)
        let carrier = world
            .spawn_unit(1, "aircraft_carrier", [10, 26], &loaded)
            .unwrap();
        let naval_target = world
            .spawn_building(2, "soviet_sentry_gun", [18, 26], &loaded, false)
            .unwrap();
        world.push_command(GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![carrier],
            target_entity_id: naval_target,
        });
        world.step(&loaded);
        let spawned_hornet = world
            .entities
            .values()
            .any(|e| e.parent_carrier_id == Some(carrier) && e.target_entity_id == Some(naval_target));
        assert!(
            spawned_hornet,
            "Aircraft Carrier should launch Hornet Fighter drones targeting the carrier's target"
        );
    }

    #[test]
    fn mirage_spy_disguise_point_defense_chrono_erase_floating_disc_and_rally_chains_work() {
        let loaded = load_ra2_mod();
        let mut world = SimWorld::from_mod(&loaded);

        // 1. Mirage Tank Terrain Disguise (`disguises_as_terrain`) & Spy Disguise (`DisguiseUnits`)
        let mirage_id = world.spawn_unit(1, "mirage_tank", [10, 10], &loaded).unwrap();
        assert!(
            world.is_entity_cloaked(world.entities.get(&mirage_id).unwrap(), &loaded),
            "Stationary Mirage Tank should be disguised as terrain (cloaked)"
        );
        let move_dest = world.grid.tile_to_world([15, 10]);
        world.push_command(GameCommand::MoveUnits {
            player_id: 1,
            entity_ids: vec![mirage_id],
            target_pos: move_dest,
        });
        world.step(&loaded);
        assert!(
            !world.is_entity_cloaked(world.entities.get(&mirage_id).unwrap(), &loaded),
            "Moving Mirage Tank should drop its terrain disguise while driving"
        );
        world.push_command(GameCommand::StopUnits {
            player_id: 1,
            entity_ids: vec![mirage_id],
        });
        world.step(&loaded);
        assert!(
            world.is_entity_cloaked(world.entities.get(&mirage_id).unwrap(), &loaded),
            "Stopped Mirage Tank should immediately re-engage its terrain disguise"
        );

        let spy_id = world.spawn_unit(1, "allied_spy", [12, 12], &loaded).unwrap();
        let enemy_conscript = world
            .spawn_unit(2, "soviet_conscript", [14, 12], &loaded)
            .unwrap();
        world.push_command(GameCommand::DisguiseUnits {
            player_id: 1,
            spy_ids: vec![spy_id],
            target_entity_id: enemy_conscript,
        });
        world.step(&loaded);
        assert_eq!(
            world.entities.get(&spy_id).unwrap().disguised_as,
            Some(("soviet_conscript".to_string(), 2)),
            "Spy should adopt the target infantry unit's ID and player faction as disguise"
        );

        // 2. Point-Defense Laser Missile Interception (`intercepts_missiles_radius`)
        let ifv_id = world.spawn_unit(1, "ifv", [20, 20], &loaded).unwrap();
        let ifv_pos = world.entities.get(&ifv_id).unwrap().position;
        let hp_before = world.entities.get(&ifv_id).unwrap().health;
        world.projectiles.push(crate::combat::SimProjectile {
            id: 9001,
            attacker_id: enemy_conscript,
            attacker_player: 2,
            target_id: ifv_id,
            weapon_id: "v3_ballistic_rocket".to_string(),
            damage: 180.0,
            start_pos: [ifv_pos[0] + 6.0, 2.0, ifv_pos[2]],
            current_pos: [ifv_pos[0] + 4.0, 2.0, ifv_pos[2]],
            target_pos: ifv_pos,
            speed: 20.0,
            delivery: rts_sdk::schema::DeliveryKind::Missile,
            arc_height: 2.0,
            friendly_fire: false,
            progress: 0.3,
            total_dist: 6.0,
            projectile_style: rts_sdk::schema::ProjectileStyle::Missile,
            splash_radius: 2.0,
            mind_control: false,
            mind_control_limit: 0,
            warp_erase: false,
            c4_demolition: false,
            clears_garrison: false,
            health: 50.0,
            max_health: 50.0,
        });
        world.step(&loaded);
        assert!(
            world.projectiles.iter().all(|p| p.id != 9001),
            "Point-Defense IFV should intercept incoming hostile missile in mid-air"
        );
        assert_eq!(
            world.entities.get(&ifv_id).unwrap().health,
            hp_before,
            "Intercepted missile should deal 0 damage"
        );
        assert!(
            world.entities.get(&ifv_id).unwrap().point_defense_timer > 0.0,
            "Point-Defense system should enter cooldown after intercepting a missile"
        );

        // 3. Chrono Legionnaire (`chrono_locomotion`) & Temporal Erase (`temporal_erase_dps`)
        let cleg_id = world
            .spawn_unit(1, "chrono_legionnaire", [18, 14], &loaded)
            .unwrap();
        let blink_dest = world.grid.tile_to_world([24, 14]);
        world.push_command(GameCommand::MoveUnits {
            player_id: 1,
            entity_ids: vec![cleg_id],
            target_pos: blink_dest,
        });
        world.step(&loaded);
        let cleg_ent = world.entities.get(&cleg_id).unwrap();
        assert!(
            (cleg_ent.position[0] - blink_dest[0]).abs() < 0.2
                && (cleg_ent.position[2] - blink_dest[2]).abs() < 0.2,
            "Chrono Legionnaire with chrono_locomotion should blink directly to destination"
        );
        assert!(cleg_ent.chrono_disable_remaining() > 0.0);
        world.entities.get_mut(&cleg_id).unwrap().set_chrono_disable(0.0);

        let erase_victim = world
            .spawn_unit(2, "rhino_tank", [25, 14], &loaded)
            .unwrap();
        let crates_before = world.crates.len();
        world.push_command(GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![cleg_id],
            target_entity_id: erase_victim,
        });
        world.step(&loaded);
        assert!(
            world.entities.get(&erase_victim).unwrap().temporal_lock_remaining() > 1.0
                && world.entities.get(&erase_victim).unwrap().temporal_progress > 0.0,
            "Neutron Eraser should freeze target in time and build temporal instability"
        );
        for _ in 0..85 {
            world.step(&loaded);
        }
        assert!(
            !world.entities.contains_key(&erase_victim),
            "Rhino Tank should be erased from history once temporal_progress reaches max_health"
        );
        assert_eq!(
            world.crates.len(),
            crates_before,
            "Temporally erased unit must not drop salvage crates"
        );

        // 4. Floating Disc Building Depower & Credit Siphon (`disables_building_duration_secs`, `steals_credits_per_hit`)
        let disc_id = world
            .spawn_unit(2, "floating_disc", [16, 24], &loaded)
            .unwrap();
        let enemy_refinery = world
            .spawn_building(1, "allied_refinery", [18, 24], &loaded, false)
            .unwrap();
        world.players.get_mut(&1).unwrap().credits = 1000;
        let p2_credits_before = world.players.get(&2).unwrap().credits;
        world.push_command(GameCommand::AttackTarget {
            player_id: 2,
            attacker_ids: vec![disc_id],
            target_entity_id: enemy_refinery,
        });
        world.step(&loaded);
        assert!(
            world.entities.get(&enemy_refinery).unwrap().emp_remaining() >= 5.5,
            "Floating Disc saucer beam should disable enemy structure"
        );
        assert_eq!(
            world.players.get(&1).unwrap().credits,
            950,
            "Floating Disc should siphon 50 credits from victim player"
        );
        assert_eq!(
            world.players.get(&2).unwrap().credits,
            p2_credits_before + 50,
            "Floating Disc should add siphoned 50 credits to attacker player"
        );

        // 5. Production Queue Pause (`TogglePauseProduction`) & Multi-Waypoint Rally Chains (`QueueRallyPoint`)
        let barracks_id = world
            .spawn_building(1, "allied_barracks", [8, 22], &loaded, false)
            .unwrap();
        let wp1 = world.grid.tile_to_world([12, 22]);
        let wp2 = world.grid.tile_to_world([12, 26]);
        world.push_command(GameCommand::SetRallyPoint {
            player_id: 1,
            building_ids: vec![barracks_id],
            rally_pos: wp1,
        });
        world.push_command(GameCommand::QueueRallyPoint {
            player_id: 1,
            building_ids: vec![barracks_id],
            rally_pos: wp2,
        });
        world.push_command(GameCommand::ProduceUnit {
            player_id: 1,
            producer_entity_id: Some(barracks_id),
            unit_id: "allied_gi".to_string(),
        });
        world.step(&loaded);
        let rem_before_pause = world
            .entities
            .get(&barracks_id)
            .unwrap()
            .production_queue[0]
            .remaining_secs;
        world.push_command(GameCommand::TogglePauseProduction {
            player_id: 1,
            building_id: Some(barracks_id),
        });
        for _ in 0..5 {
            world.step(&loaded);
        }
        let rem_during_pause = world
            .entities
            .get(&barracks_id)
            .unwrap()
            .production_queue[0]
            .remaining_secs;
        assert!(
            (rem_during_pause - rem_before_pause).abs() < 1e-4,
            "Paused production queue should not tick down remaining_secs"
        );
        // Resume and complete production
        world.push_command(GameCommand::TogglePauseProduction {
            player_id: 1,
            building_id: Some(barracks_id),
        });
        world.entities.get_mut(&barracks_id).unwrap().production_queue[0].remaining_secs = 0.02;
        world.step(&loaded);
        let trained_gi = world
            .entities
            .values()
            .filter(|e| {
                e.player_id == 1
                    && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "allied_gi")
            })
            .max_by_key(|e| e.id)
            .unwrap();
        assert_eq!(
            trained_gi.path_waypoints.last().copied(),
            Some(wp2),
            "Unit spawned from Barracks with chained rally waypoints should follow full route to final waypoint"
        );
    }

    #[test]
    fn gatling_spinup_boris_airstrike_robot_link_mastermind_overload_squid_grapple_and_mobile_refinery_work()
    {
        let loaded = load_ra2_mod();
        let mut world = SimWorld::from_mod(&loaded);

        // 1. Gatling Spin-Up (`spin_up_per_shot`, `max_spin_up_multiplier`)
        let gat_id = world.spawn_unit(2, "gattling_tank", [10, 10], &loaded).unwrap();
        let target_bldg = world
            .spawn_building(1, "allied_conyard", [14, 10], &loaded, false)
            .unwrap();
        world.push_command(GameCommand::AttackTarget {
            player_id: 2,
            attacker_ids: vec![gat_id],
            target_entity_id: target_bldg,
        });
        world.step(&loaded);
        let cd_shot_1 = world.entities.get(&gat_id).unwrap().weapon_cooldown;
        let spin_1 = world.entities.get(&gat_id).unwrap().spin_up_level;
        assert!(spin_1 > 0.0, "Gatling Tank should increase spin_up_level after firing");
        for _ in 0..16 {
            world.step(&loaded);
        }
        let spin_later = world.entities.get(&gat_id).unwrap().spin_up_level;
        assert!(
            spin_later > spin_1,
            "Continuous Gatling fire should further increase spin_up_level ({spin_1} -> {spin_later})"
        );
        // Trigger next shot and verify accelerated cooldown
        world.entities.get_mut(&gat_id).unwrap().weapon_cooldown = 0.0;
        world.step(&loaded);
        let cd_shot_later = world.entities.get(&gat_id).unwrap().weapon_cooldown;
        assert!(
            cd_shot_later < cd_shot_1,
            "Spun-up Gatling Tank should have a shorter weapon cooldown ({cd_shot_later} < {cd_shot_1})"
        );

        // 2. Hero Boris MiG Laser Target Designator (`boris_laser_flare` -> `calls_airstrike_damage`)
        let boris_id = world.spawn_unit(2, "boris", [10, 16], &loaded).unwrap();
        let enemy_bldg = world
            .spawn_building(1, "allied_power_plant", [15, 16], &loaded, false)
            .unwrap();
        let bldg_hp_before = world.entities.get(&enemy_bldg).unwrap().health;
        world.push_command(GameCommand::AttackTarget {
            player_id: 2,
            attacker_ids: vec![boris_id],
            target_entity_id: enemy_bldg,
        });
        world.step(&loaded);
        assert!(
            world.entities.get(&enemy_bldg).unwrap().pending_airstrike_timer > 0.0,
            "Boris targeting an enemy building should lock on with his MiG laser designator"
        );
        for _ in 0..45 {
            world.step(&loaded);
        }
        let bldg_hp_after = world
            .entities
            .get(&enemy_bldg)
            .map(|e| e.health)
            .unwrap_or(0.0);
        assert!(
            bldg_hp_after < bldg_hp_before - 500.0,
            "MiG airstrike called by Boris should deal massive damage to the designated building"
        );

        // 3. Robot Control Link (`requires_powered_building = "allied_robot_control"`) & Immunities
        let robot_id = world.spawn_unit(1, "robot_tank", [10, 22], &loaded).unwrap();
        assert!(
            world.is_entity_control_offline(world.entities.get(&robot_id).unwrap(), &loaded),
            "Robot Tank without an active Allied Robot Control Center should be offline"
        );
        let rc_id = world
            .spawn_building(1, "allied_robot_control", [6, 22], &loaded, false)
            .unwrap();
        assert!(
            !world.is_entity_control_offline(world.entities.get(&robot_id).unwrap(), &loaded),
            "Robot Tank should come online once Allied Robot Control Center is powered and operational"
        );
        // Toggle power off on Robot Control Center -> Robot Tank goes offline again
        world.push_command(GameCommand::ToggleBuildingPower {
            player_id: 1,
            building_ids: vec![rc_id],
        });
        world.step(&loaded);
        assert!(
            world.is_entity_control_offline(world.entities.get(&robot_id).unwrap(), &loaded),
            "Robot Tank should go offline when Robot Control Center power is disabled"
        );

        // 4. Mastermind Psychic Overload (`mind_control_overload_dps`) & Robot Mind-Control Immunity
        let mastermind_id = world.spawn_unit(2, "mastermind", [20, 22], &loaded).unwrap();
        // Try mind-controlling the immune Robot Tank -> should not change player_id
        world.push_command(GameCommand::AttackTarget {
            player_id: 2,
            attacker_ids: vec![mastermind_id],
            target_entity_id: robot_id,
        });
        world.step(&loaded);
        assert_eq!(
            world.entities.get(&robot_id).unwrap().player_id,
            1,
            "Unmanned Robot Tank should be immune to mind control"
        );

        // Now mind-control 4 human infantry (exceeding `mind_control_limit = 3`)
        for i in 0..4 {
            let v_id = world
                .spawn_unit(1, "allied_gi", [22 + i, 22], &loaded)
                .unwrap();
            world.entities.get_mut(&mastermind_id).unwrap().weapon_cooldown = 0.0;
            world.push_command(GameCommand::AttackTarget {
                player_id: 2,
                attacker_ids: vec![mastermind_id],
                target_entity_id: v_id,
            });
            world.step(&loaded);
            assert_eq!(
                world.entities.get(&v_id).unwrap().player_id,
                2,
                "Mastermind should be able to mind-control unit #{} (even beyond limit 3)",
                i + 1
            );
        }
        let mm_hp_before = world.entities.get(&mastermind_id).unwrap().health;
        for _ in 0..10 {
            world.step(&loaded);
        }
        let mm_hp_after = world.entities.get(&mastermind_id).unwrap().health;
        assert!(
            mm_hp_after < mm_hp_before,
            "Mastermind controlling 4 units (> limit 3) should suffer psychic overload self-damage ({mm_hp_before} -> {mm_hp_after})"
        );

        // 5. Giant Squid Grapple (`grapple_duration_secs`) & Dolphin Sonic Un-Grapple (`clears_grapple`)
        let squid_id = world.spawn_unit(2, "giant_squid", [26, 10], &loaded).unwrap();
        let destroyer_id = world
            .spawn_unit(1, "allied_destroyer", [28, 10], &loaded)
            .unwrap();
        world.push_command(GameCommand::AttackTarget {
            player_id: 2,
            attacker_ids: vec![squid_id],
            target_entity_id: destroyer_id,
        });
        world.step(&loaded);
        assert!(
            world.entities.get(&destroyer_id).unwrap().grappled_remaining() > 0.0,
            "Giant Squid attack should grapple the enemy Destroyer"
        );
        let dolphin_id = world.spawn_unit(1, "dolphin", [25, 10], &loaded).unwrap();
        world.push_command(GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![dolphin_id],
            target_entity_id: squid_id,
        });
        world.step(&loaded);
        assert_eq!(
            world.entities.get(&destroyer_id).unwrap().grappled_remaining(),
            0.0,
            "Allied Dolphin sonic pulse should break the Giant Squid's grapple on the Destroyer"
        );

        // 6. Mobile Refinery (`yuri_slave_miner` with `mobile_refinery = true`)
        let p2_credits_before = world.players.get(&2).unwrap().credits;
        world.grid.ore_tiles.insert([30, 18], 1000);
        let slave_miner_id = world
            .spawn_unit(2, "yuri_slave_miner", [30, 18], &loaded)
            .unwrap();
        if let Some(sm) = world.entities.get_mut(&slave_miner_id) {
            if let Some(h) = &mut sm.harvester_state {
                h.current_ore = h.capacity - 50;
                h.phase = crate::world::HarvesterPhase::Harvesting([30, 18]);
                h.harvest_timer = 0.02;
            }
        }
        world.step(&loaded);
        let p2_credits_after = world.players.get(&2).unwrap().credits;
        assert!(
            p2_credits_after > p2_credits_before,
            "Mobile Refinery (Yuri Slave Miner) should refine ore directly on the spot into credits without returning to a refinery ({p2_credits_before} -> {p2_credits_after})"
        );
    }

    #[test]
    fn bioreactor_power_tank_bunker_siege_chopper_ivan_bomb_nuclear_meltdown_and_shipyard_repair_work()
    {
        let loaded = load_ra2_mod();
        let mut world = SimWorld::from_mod(&loaded);

        // 1. Yuri Bio-Reactor (`power_per_occupant = 100`)
        let bio_id = world
            .spawn_building(2, "yuri_bio_reactor", [6, 6], &loaded, false)
            .unwrap();
        let base_power = world.players.get(&2).unwrap().power_produced;
        let initiate_id = world
            .spawn_unit(2, "yuri_initiate", [6, 8], &loaded)
            .unwrap();
        world.push_command(GameCommand::LoadIntoCarrier {
            player_id: 2,
            passenger_ids: vec![initiate_id],
            carrier_id: bio_id,
        });
        for _ in 0..8 {
            world.step(&loaded);
        }
        assert_eq!(
            world.entities.get(&initiate_id).unwrap().transported_by,
            Some(bio_id),
            "Yuri Initiate should garrison inside the Bio-Reactor"
        );
        let boosted_power = world.players.get(&2).unwrap().power_produced;
        assert_eq!(
            boosted_power,
            base_power + 100,
            "Each infantry occupant inside Yuri Bio-Reactor should add +100 power"
        );
        world.push_command(GameCommand::UnloadCarrier {
            player_id: 2,
            carrier_ids: vec![bio_id],
        });
        world.step(&loaded);
        assert_eq!(
            world.players.get(&2).unwrap().power_produced,
            base_power,
            "Unloading Bio-Reactor should remove occupant power bonus"
        );

        // 2. Yuri Tank Bunker (`garrison_accepts_vehicles = true`, `passengers_can_fire = true`)
        let tank_bunker_id = world
            .spawn_building(2, "yuri_tank_bunker", [22, 16], &loaded, false)
            .unwrap();
        let lasher_id = world
            .spawn_unit(2, "lasher_tank", [22, 18], &loaded)
            .unwrap();
        world.push_command(GameCommand::LoadIntoCarrier {
            player_id: 2,
            passenger_ids: vec![lasher_id],
            carrier_id: tank_bunker_id,
        });
        for _ in 0..8 {
            world.step(&loaded);
        }
        assert_eq!(
            world.entities.get(&lasher_id).unwrap().transported_by,
            Some(tank_bunker_id),
            "Ground vehicle (Lasher Tank) should be able to enter Yuri Tank Bunker"
        );
        if let Some(l) = world.entities.get_mut(&lasher_id) {
            l.weapon_cooldown = 0.0;
        }
        let enemy_gi = world.spawn_unit(1, "allied_gi", [22, 20], &loaded).unwrap();
        let gi_hp_before = world.entities.get(&enemy_gi).unwrap().health;
        for _ in 0..8 {
            world.step(&loaded);
        }
        let gi_hp_after = world
            .entities
            .get(&enemy_gi)
            .map(|e| e.health)
            .unwrap_or(0.0);
        assert!(
            gi_hp_after < gi_hp_before,
            "Vehicle garrisoned inside Yuri Tank Bunker should fire its weapon from the bunker fireport ({gi_hp_before} -> {gi_hp_after})"
        );
        if let Some(e) = world.entities.get_mut(&enemy_gi) {
            e.health = 0.0;
        }
        world.step(&loaded);

        // 3. Soviet Siege Chopper (`lands_when_deployed = true`, `deploy_weapon = "chopper_130mm"`)
        let chopper_id = world
            .spawn_unit(2, "siege_chopper", [18, 8], &loaded)
            .unwrap();
        let air_y = world.entities.get(&chopper_id).unwrap().position[1];
        assert_eq!(
            crate::combat::entity_domain(world.entities.get(&chopper_id).unwrap(), &loaded),
            rts_sdk::schema::MovementDomain::Air
        );
        world.push_command(GameCommand::ActivateAbility {
            player_id: 2,
            entity_ids: vec![chopper_id],
        });
        world.step(&loaded);
        let chopper_ent = world.entities.get(&chopper_id).unwrap();
        assert!(chopper_ent.script_ctx.deployed);
        assert!(
            chopper_ent.position[1] < air_y - 1.5,
            "Deployed Siege Chopper should land on the ground ({air_y} -> {})",
            chopper_ent.position[1]
        );
        assert_eq!(
            crate::combat::entity_domain(chopper_ent, &loaded),
            rts_sdk::schema::MovementDomain::Ground
        );
        let (active_wid, _) = crate::combat::select_weapon_for_target(
            &world,
            chopper_ent,
            rts_sdk::schema::MovementDomain::Ground,
            false,
            &loaded,
        )
        .unwrap();
        assert_eq!(
            active_wid, "chopper_130mm",
            "Landed Siege Chopper should switch to 130mm siege artillery cannon"
        );

        // 4. Crazy Ivan Timed Bomb (`attaches_bomb_timer_secs = 4.0`, `attaches_bomb_damage = 550.0`) & Engineer Defusal
        let ivan_id = world.spawn_unit(2, "crazy_ivan", [22, 8], &loaded).unwrap();
        let bomb_carrier = world
            .spawn_unit(2, "soviet_conscript", [23, 8], &loaded)
            .unwrap();
        world.push_command(GameCommand::AttackTarget {
            player_id: 2,
            attacker_ids: vec![ivan_id],
            target_entity_id: bomb_carrier,
        });
        for _ in 0..4 {
            world.step(&loaded);
        }
        let bc_ent = world.entities.get(&bomb_carrier).unwrap();
        let bc_bomb_dmg = bc_ent.c4_bomb_info().map(|(_, dmg, _, _)| dmg).unwrap_or(0.0);
        assert!(
            bc_ent.c4_remaining() > 0.0 && (bc_bomb_dmg - 380.0).abs() < 1.0,
            "Crazy Ivan should plant a timed dynamite bomb on the target unit (timer={}, dmg={})",
            bc_ent.c4_remaining(),
            bc_bomb_dmg
        );
        let eng_id = world
            .spawn_unit(2, "soviet_engineer", [24, 8], &loaded)
            .unwrap();
        world.push_command(GameCommand::AttackTarget {
            player_id: 2,
            attacker_ids: vec![eng_id],
            target_entity_id: bomb_carrier,
        });
        for _ in 0..4 {
            world.step(&loaded);
        }
        assert_eq!(
            world.entities.get(&bomb_carrier).unwrap().c4_remaining(),
            0.0,
            "Friendly Engineer should be able to defuse a Crazy Ivan bomb on a unit"
        );

        // 5. Soviet Nuclear Reactor Meltdown (`meltdown_damage = 900.0`, `meltdown_leaves_radiation = true`)
        let reactor_id = world
            .spawn_building(2, "soviet_nuclear_reactor", [24, 20], &loaded, false)
            .unwrap();
        let bystander_id = world
            .spawn_unit(2, "rhino_tank", [25, 24], &loaded)
            .unwrap();
        let bystander_hp_before = world.entities.get(&bystander_id).unwrap().health;
        let rad_zones_before = world.radiation_zones.len();
        if let Some(r) = world.entities.get_mut(&reactor_id) {
            r.health = 0.0;
        }
        world.step(&loaded);
        let bystander_hp_after = world
            .entities
            .get(&bystander_id)
            .map(|e| e.health)
            .unwrap_or(0.0);
        assert!(
            bystander_hp_after < bystander_hp_before,
            "Destroyed Soviet Nuclear Reactor should trigger a meltdown AoE explosion ({bystander_hp_before} -> {bystander_hp_after})"
        );
        assert!(
            world.radiation_zones.len() > rad_zones_before,
            "Destroyed Soviet Nuclear Reactor should leave a radiation fallout zone"
        );

        // 6. Iron Curtain Organic Infantry Lethality vs. Vehicle Invulnerability
        let ic_bldg = world
            .spawn_building(2, "soviet_iron_curtain", [6, 20], &loaded, false)
            .unwrap();
        if let Some(b) = world.entities.get_mut(&ic_bldg) {
            b.ability_cooldown = 0.0;
        }
        let ic_tank = world
            .spawn_unit(2, "rhino_tank", [10, 26], &loaded)
            .unwrap();
        let ic_infantry = world
            .spawn_unit(2, "soviet_conscript", [11, 26], &loaded)
            .unwrap();
        let ic_pos = world.entities.get(&ic_tank).unwrap().position;
        world.push_command(GameCommand::CastTargetedAbility {
            player_id: 2,
            caster_ids: vec![ic_bldg],
            target_pos: ic_pos,
            target_entity_id: None,
        });
        world.step(&loaded);
        assert!(
            world.entities.get(&ic_tank).unwrap().invulnerable_remaining() > 0.0,
            "Iron Curtain should make friendly vehicle invulnerable"
        );
        assert!(
            !world.entities.contains_key(&ic_infantry),
            "Iron Curtain should fatally crush organic infantry in its AoE"
        );

        // 7. Naval Shipyard Auto-Repair (`repairs_ships_per_sec = 45.0`)
        let shipyard_id = world
            .spawn_building(1, "allied_naval_shipyard", [18, 26], &loaded, false)
            .unwrap();
        let destroyer_id = world
            .spawn_unit(1, "allied_destroyer", [19, 28], &loaded)
            .unwrap();
        let ship_pos = world.entities.get(&shipyard_id).unwrap().position;
        if let Some(d) = world.entities.get_mut(&destroyer_id) {
            d.position = [ship_pos[0] + 2.0, d.position[1], ship_pos[2] + 2.0];
            d.health = d.max_health * 0.5;
        }
        let ship_hp_before = world.entities.get(&destroyer_id).unwrap().health;
        for _ in 0..5 {
            world.step(&loaded);
        }
        let ship_hp_after = world.entities.get(&destroyer_id).unwrap().health;
        assert!(
            ship_hp_after > ship_hp_before,
            "Damaged naval unit docked near Allied Naval Shipyard should be repaired automatically ({ship_hp_before} -> {ship_hp_after})"
        );
    }

    #[test]
    fn aa_missile_interception_secret_lab_unlock_psi_wave_sub_reveal_and_destroyer_osprey_work() {
        let loaded = load_ra2_mod();
        let mut world = SimWorld::from_mod(&loaded);
        world.entities.clear();
        for pid in [1u8, 2u8] {
            for i in 0..3 {
                world.spawn_building(pid, "allied_power_plant", [2 + i * 3, 2], &loaded, false);
            }
            if let Some(econ) = world.players.get_mut(&pid) {
                econ.credits = 20_000;
            }
        }

        // 1. Interceptable Cruise Missiles (`v3_ballistic_rocket` shot down mid-air by Patriot AA)
        let target_bldg = world
            .spawn_building(1, "allied_power_plant", [10, 10], &loaded, false)
            .unwrap();
        let patriot = world
            .spawn_building(1, "soviet_flak_cannon", [12, 10], &loaded, false)
            .unwrap();
        let v3 = world
            .spawn_unit(2, "v3_rocket_launcher", [22, 10], &loaded)
            .unwrap();
        let bldg_hp_before = world.entities.get(&target_bldg).unwrap().health;
        world.push_command(GameCommand::AttackTarget {
            player_id: 2,
            attacker_ids: vec![v3],
            target_entity_id: target_bldg,
        });
        world.step(&loaded);
        assert!(
            !world.projectiles.is_empty(),
            "V3 Launcher should spawn an in-flight interceptable ballistic missile projectile"
        );
        assert!(
            world.projectiles[0].max_health >= 90.0,
            "V3 ballistic rocket should have interceptable AA health"
        );
        for _ in 0..30 {
            world.step(&loaded);
        }
        let bldg_hp_after = world.entities.get(&target_bldg).unwrap().health;
        assert_eq!(
            bldg_hp_after, bldg_hp_before,
            "Patriot AA defense should shoot down the V3 rocket mid-air before it hits the building"
        );
        assert!(
            world.entities.get(&patriot).unwrap().weapon_cooldown > 0.0,
            "Patriot AA battery should have fired to intercept the incoming V3 missile"
        );

        // 2. Tech Secret Lab Blueprint Unlock (`unlocks_units` cross-faction production)
        let barracks_p1 = world
            .spawn_building(1, "allied_barracks", [6, 16], &loaded, false)
            .unwrap();
        let _wf_p1 = world
            .spawn_building(1, "allied_war_factory", [10, 16], &loaded, false)
            .unwrap();
        assert!(
            !world
                .available_units_for_player(1, &loaded)
                .contains(&"desolator".to_string()),
            "Allied Player 1 should not have Soviet Desolator unlocked before capturing Tech Secret Lab"
        );
        let _secret_lab = world
            .spawn_building(1, "tech_secret_lab", [14, 16], &loaded, false)
            .unwrap();
        let unlocked = world.available_units_for_player(1, &loaded);
        assert!(
            unlocked.contains(&"desolator".to_string())
                && unlocked.contains(&"apocalypse_tank".to_string()),
            "Tech Secret Lab should unlock secret cross-faction blueprints (Desolator & Apocalypse Tank)"
        );
        world.push_command(GameCommand::ProduceUnit {
            player_id: 1,
            producer_entity_id: Some(barracks_p1),
            unit_id: "desolator".to_string(),
        });
        world.step(&loaded);
        assert_eq!(
            world
                .entities
                .get(&barracks_p1)
                .unwrap()
                .production_queue
                .len(),
            1,
            "Allied Barracks should be able to queue a Secret Lab-unlocked Desolator"
        );

        // 3. Yuri Clone Psi-Wave AoE Infantry Blast (`psi_wave_radius = 5.5`, `psi_wave_damage = 280.0`)
        let yuri_clone = world
            .spawn_unit(2, "yuri_clone", [20, 20], &loaded)
            .unwrap();
        let enemy_gi = world.spawn_unit(1, "allied_gi", [21, 20], &loaded).unwrap();
        let enemy_tank = world
            .spawn_unit(1, "grizzly_tank", [20, 21], &loaded)
            .unwrap();
        let tank_hp_before = world.entities.get(&enemy_tank).unwrap().health;
        world.push_command(GameCommand::ActivateAbility {
            player_id: 2,
            entity_ids: vec![yuri_clone],
        });
        world.step(&loaded);
        assert!(
            world
                .entities
                .get(&enemy_gi)
                .map(|e| e.health <= 0.0)
                .unwrap_or(true),
            "Yuri Clone Psi-Wave blast should kill nearby hostile infantry"
        );
        assert_eq!(
            world.entities.get(&enemy_tank).unwrap().health,
            tank_hp_before,
            "Yuri Clone Psi-Wave blast should not damage armored vehicles"
        );

        // 4. Submarine Uncloak on Fire (`reveal_on_fire_secs`) & Allied Destroyer ASW Osprey Carrier
        let sub = world
            .spawn_unit(2, "typhoon_sub", [26, 22], &loaded)
            .unwrap();
        let destroyer = world
            .spawn_unit(1, "allied_destroyer", [26, 26], &loaded)
            .unwrap();
        assert!(
            world.is_entity_cloaked(world.entities.get(&sub).unwrap(), &loaded),
            "Typhoon Submarine should be cloaked before firing"
        );
        world.push_command(GameCommand::AttackTarget {
            player_id: 2,
            attacker_ids: vec![sub],
            target_entity_id: destroyer,
        });
        world.push_command(GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![destroyer],
            target_entity_id: sub,
        });
        world.step(&loaded);
        assert!(
            world.entities.get(&sub).unwrap().reveal_timer > 0.0
                && !world.is_entity_cloaked(world.entities.get(&sub).unwrap(), &loaded),
            "Typhoon Submarine should uncloak (`reveal_timer > 0.0`) immediately after firing torpedoes"
        );
        let osprey_spawned = world.entities.values().any(|e| {
            e.parent_carrier_id == Some(destroyer)
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "osprey_asw")
        });
        assert!(
            osprey_spawned,
            "Allied Destroyer should launch an ASW Osprey drone when engaging a target"
        );
    }

    #[test]
    fn chrono_miner_teleport_naval_c4_elite_self_heal_psychic_sensor_slave_miner_and_ai_chopper_work() {
        let loaded = load_ra2_mod();
        let mut world = SimWorld::from_mod(&loaded);
        world.entities.clear();
        for pid in [1u8, 2u8] {
            for i in 0..3 {
                world.spawn_building(pid, "allied_power_plant", [2 + i * 3, 2], &loaded, false);
            }
            if let Some(econ) = world.players.get_mut(&pid) {
                econ.credits = 10_000;
            }
        }

        // 1. Chrono Miner Emergency & Return Chrono-Teleport (`chrono_harvester = true`)
        let _refinery = world
            .spawn_building(1, "allied_refinery", [6, 8], &loaded, false)
            .unwrap();
        let miner = world
            .spawn_unit(1, "chrono_miner", [24, 24], &loaded)
            .unwrap();
        if let Some(m) = world.entities.get_mut(&miner) {
            m.health = m.max_health * 0.30; // Critically damaged (<45%)
            if let Some(h) = &mut m.harvester_state {
                h.phase = crate::world::HarvesterPhase::Harvesting([24, 24]);
                h.current_ore = 300;
            }
        }
        world.step(&loaded);
        let miner_tile_after = world.entities.get(&miner).unwrap().tile_origin;
        assert!(
            (miner_tile_after[0] - 7).abs() <= 3 && (miner_tile_after[1] - 11).abs() <= 3,
            "Critically damaged Chrono Miner should immediately Chrono-teleport back to the refinery ({miner_tile_after:?})"
        );
        assert!(
            world
                .combat_events
                .iter()
                .any(|ev| ev.weapon_id == "chrono_return"),
            "Chrono Miner teleport should emit a Chrono return VFX event"
        );

        // 2. Amphibious Commando (`navy_seal` / `tanya`) Naval C4 Sabotage against Warships
        let seal = world.spawn_unit(1, "navy_seal", [14, 14], &loaded).unwrap();
        let enemy_ship = world
            .spawn_unit(2, "dreadnought", [15, 14], &loaded)
            .unwrap();
        world.push_command(GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![seal],
            target_entity_id: enemy_ship,
        });
        world.step(&loaded);
        assert!(
            world.entities.get(&enemy_ship).unwrap().c4_remaining() > 0.0,
            "Navy SEAL C4 demolition should arm a C4 charge on an enemy naval warship"
        );

        // 3. Elite Veterancy Self-Repair (`VeterancyRank::Elite`) & Kirov Airship Passive Self-Heal (`self_heal_per_sec`)
        let elite_tank = world
            .spawn_unit(1, "grizzly_tank", [10, 18], &loaded)
            .unwrap();
        let kirov = world
            .spawn_unit(2, "kirov_airship", [18, 18], &loaded)
            .unwrap();
        if let Some(t) = world.entities.get_mut(&elite_tank) {
            t.veterancy = crate::world::VeterancyRank::Elite;
            t.health = 200.0;
        }
        if let Some(k) = world.entities.get_mut(&kirov) {
            k.health = 1000.0;
        }
        for _ in 0..10 {
            world.step(&loaded);
        }
        assert!(
            world.entities.get(&elite_tank).unwrap().health > 200.0,
            "Elite Veterancy unit should passively self-repair over time"
        );
        assert!(
            world.entities.get(&kirov).unwrap().health > 1000.0,
            "Kirov Airship (`self_heal_per_sec = 12.0`) should passively regenerate hull HP"
        );

        // 4. Yuri Psychic Radar Sensor (`psychic_sensor_radius = 28.0`) Threat Detection
        let _sensor = world
            .spawn_building(2, "yuri_psychic_sensor", [20, 10], &loaded, false)
            .unwrap();
        let yuri_target = world
            .spawn_unit(2, "lasher_tank", [22, 10], &loaded)
            .unwrap();
        let enemy_attacker = world
            .spawn_unit(1, "prism_tank", [28, 10], &loaded)
            .unwrap();
        world.entities.get_mut(&enemy_attacker).unwrap().target_entity_id = Some(yuri_target);
        let threats = world.psychic_sensor_threat_lines(2, &loaded);
        assert!(
            !threats.is_empty(),
            "Yuri Psychic Sensor should detect hostile attack orders targeting friendly units within sensor radius"
        );

        // 5. Yuri Slave Miner (`mobile_refinery = true`) Autonomous Proximity Ore Harvesting
        world.grid.ore_tiles.insert([12, 24], 500);
        let _slave_miner = world
            .spawn_unit(2, "yuri_slave_miner", [10, 24], &loaded)
            .unwrap();
        let p2_credits_before = world.players.get(&2).unwrap().credits;
        for _ in 0..25 {
            world.step(&loaded);
        }
        let p2_credits_after = world.players.get(&2).unwrap().credits;
        assert!(
            p2_credits_after > p2_credits_before,
            "Yuri Slave Miner should autonomously harvest nearby ore tiles and credit the player ({p2_credits_before} -> {p2_credits_after})"
        );

        // 6. Tactical AI: Siege Chopper Artillery Deployment & Yuri Clone Psi-Wave
        let ai_chopper = world
            .spawn_unit(2, "siege_chopper", [20, 26], &loaded)
            .unwrap();
        let _hostile_target = world
            .spawn_unit(1, "grizzly_tank", [25, 26], &loaded)
            .unwrap();
        while !world.tick.is_multiple_of(10) {
            world.step(&loaded);
        }
        let ai_cmds = crate::ai::generate_ai_commands_with_difficulty(
            &world,
            2,
            &loaded,
            crate::ai::AiDifficulty::Hard,
        );
        assert!(
            ai_cmds.iter().any(|c| matches!(
                c,
                GameCommand::ActivateAbility { entity_ids, .. } if entity_ids.contains(&ai_chopper)
            )),
            "Hard AI should automatically deploy Siege Chopper into 130mm artillery mode when enemies are within range"
        );
    }

    #[test]
    fn elite_weapon_terror_drone_reemerge_virus_toxic_cloud_spy_radar_reset_chrono_shift_and_ai_bunker_work(
    ) {
        let loaded = load_ra2_mod();
        let mut world = SimWorld::from_mod(&loaded);
        world.entities.clear();
        world.grid.blocked_tiles.clear();

        // 1. Elite Weapon Upgrade (`elite_weapon = "elite_tesla_bolt"` on `tesla_trooper`) & Range/Cooldown Bonuses
        let elite_tesla = world
            .spawn_unit(2, "tesla_trooper", [10, 10], &loaded)
            .unwrap();
        world.entities.get_mut(&elite_tesla).unwrap().veterancy = crate::world::VeterancyRank::Elite;
        let target_gi = world.spawn_unit(1, "allied_gi", [17, 10], &loaded).unwrap();
        world.push_command(GameCommand::AttackTarget {
            player_id: 2,
            attacker_ids: vec![elite_tesla],
            target_entity_id: target_gi,
        });
        world.step(&loaded);
        assert!(
            world
                .combat_events
                .iter()
                .any(|ev| ev.attacker_id == elite_tesla && ev.weapon_id == "elite_tesla_bolt"),
            "Elite Tesla Trooper should fire upgraded `elite_tesla_bolt` weapon"
        );

        // 2. Terror Drone Re-Emergence from Destroyed Host Vehicle (`infected_parasite_unit_id`)
        let drone = world
            .spawn_unit(2, "terror_drone", [10, 14], &loaded)
            .unwrap();
        let doomed_tank = world
            .spawn_unit(1, "grizzly_tank", [11, 14], &loaded)
            .unwrap();
        world.entities.get_mut(&doomed_tank).unwrap().health = 80.0;
        world.push_command(GameCommand::AttackTarget {
            player_id: 2,
            attacker_ids: vec![drone],
            target_entity_id: doomed_tank,
        });
        for _ in 0..35 {
            world.step(&loaded);
        }
        assert!(
            !world.entities.contains_key(&doomed_tank),
            "Host tank should be destroyed by internal Terror Drone parasite damage"
        );
        let reemerged_drone = world.entities.values().any(|e| {
            e.player_id == 2
                && e.health > 0.0
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "terror_drone")
        });
        assert!(
            reemerged_drone,
            "Terror Drone should re-emerge alive from the destroyed host vehicle wreck"
        );

        // 3. Virus Biotoxin Cloud (`is_toxic_cloud = true` damages organic infantry, spares vehicles)
        let virus = world
            .spawn_unit(2, "yuri_virus", [10, 20], &loaded)
            .unwrap();
        let victim_inf = world.spawn_unit(1, "allied_gi", [14, 20], &loaded).unwrap();
        world.entities.get_mut(&victim_inf).unwrap().health = 50.0;
        let bystander_inf = world.spawn_unit(1, "allied_gi", [14, 21], &loaded).unwrap();
        let bystander_tank = world
            .spawn_unit(1, "grizzly_tank", [14, 19], &loaded)
            .unwrap();
        let inf_hp_before = world.entities.get(&bystander_inf).unwrap().health;
        let tank_hp_before = world.entities.get(&bystander_tank).unwrap().health;
        world.push_command(GameCommand::AttackTarget {
            player_id: 2,
            attacker_ids: vec![virus],
            target_entity_id: victim_inf,
        });
        world.step(&loaded);
        assert!(
            world.radiation_zones.iter().any(|rz| rz.is_toxic_cloud),
            "Virus sniper kill should spawn a biotoxin cloud (`is_toxic_cloud = true`)"
        );
        world.step(&loaded);
        assert!(
            world.entities.get(&bystander_inf).unwrap().health < inf_hp_before,
            "Biotoxin cloud should damage nearby organic infantry"
        );
        assert_eq!(
            world.entities.get(&bystander_tank).unwrap().health,
            tank_hp_before,
            "Enclosed armored vehicles should be immune to organic biotoxin clouds"
        );

        // 4. Allied Covert Spy Infiltration of Radar Building -> Resets Victim Explored Shroud
        let radar = world
            .spawn_building(2, "soviet_radar_tower", [22, 14], &loaded, false)
            .unwrap();
        let total_tiles = (world.grid.width * world.grid.height) as usize;
        world.explored_tiles.insert(2, vec![true; total_tiles]);
        let spy = world
            .spawn_unit(1, "allied_spy", [22, 16], &loaded)
            .unwrap();
        world.push_command(GameCommand::InfiltrateBuilding {
            player_id: 1,
            spy_ids: vec![spy],
            target_building_id: radar,
        });
        for _ in 0..8 {
            world.step(&loaded);
        }
        let remote_idx = 0; // Tile [0, 0] far outside Player 2's active vision
        assert!(
            !world.explored_tiles.get(&2).unwrap()[remote_idx],
            "Spy infiltrating enemy Radar Tower should reset the victim's explored Fog-of-War shroud"
        );

        // 5. Chronosphere Shift (`SupportPowerKind::ChronoShift`) teleports Chrono Legionnaire & vehicles, kills organic infantry
        let _chrono_bldg = world
            .spawn_building(1, "allied_chronosphere", [6, 6], &loaded, false)
            .unwrap();
        let shift_tank = world
            .spawn_unit(1, "grizzly_tank", [20, 26], &loaded)
            .unwrap();
        let shift_legionnaire = world
            .spawn_unit(1, "chrono_legionnaire", [21, 26], &loaded)
            .unwrap();
        let shift_gi = world.spawn_unit(1, "allied_gi", [20, 27], &loaded).unwrap();
        let src_pos = world.entities.get(&shift_tank).unwrap().position;
        let dst_pos = world.grid.tile_to_world([28, 26]);
        world.push_command(GameCommand::CastSupportPower {
            player_id: 1,
            power_id: "chronosphere_shift".to_string(),
            source_pos: Some(src_pos),
            target_pos: dst_pos,
        });
        world.step(&loaded);
        assert!(
            !world.entities.contains_key(&shift_gi),
            "Regular organic infantry without a Chrono suit should be killed by Chronosphere Shift"
        );
        let leg_after = world
            .entities
            .get(&shift_legionnaire)
            .expect("Chrono Legionnaire should survive Chronosphere Shift");
        assert!(
            (leg_after.position[0] - dst_pos[0]).abs() <= 4.0,
            "Chrono Legionnaire (`chrono_locomotion = true`) should teleport safely with vehicles"
        );

        // 6. Tactical AI Auto-Garrisoning of Bio Reactors / Battle Bunkers (`LoadIntoCarrier`)
        while !world.tick.is_multiple_of(10) {
            world.step(&loaded);
        }
        let bio_reactor = world
            .spawn_building(2, "yuri_bio_reactor", [2, 28], &loaded, false)
            .unwrap();
        let ai_initiate = world
            .spawn_unit(2, "yuri_initiate", [4, 28], &loaded)
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
                } if *carrier_id == bio_reactor && passenger_ids.contains(&ai_initiate)
            )),
            "Hard AI should automatically garrison nearby idle infantry into friendly Bio Reactors / Battle Bunkers"
        );
    }


