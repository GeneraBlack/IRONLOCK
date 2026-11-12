#[cfg(test)]
mod tests {
    use crate::ai::{
        generate_ai_commands_with_difficulty, generate_ai_commands_with_profile, AiDifficulty,
        AiPersonality,
    };
    use crate::combat::SimProjectile;
    use crate::command::GameCommand;
    use crate::map_gen::{generate_procedural_map, MapGenConfig};
    use crate::tests::load_ra2_mod;
    use crate::world::{
        DestroyedBridgeSpan, EntityKind, EnvironmentPreset, MapSymmetryMode, SimWorld,
        VeterancyRank,
    };
    use rts_sdk::schema::{
        AbilityDef, AbilityTargetMode, AiProfileDef, CrateKind, DeliveryKind, EffectPayload,
        MovementDomain, ProjectileStyle,
    };

    #[test]
    fn security_and_payload_validation_rejects_nan_sandbox_and_illegal_placements() {
        let loaded = load_ra2_mod();
        let mut world = SimWorld::from_mod(&loaded);
        world.players.get_mut(&1).unwrap().credits = 20_000;

        let p1_tank = world
            .spawn_unit(1, "grizzly_tank", [14, 16], &loaded)
            .unwrap();
        let orig_pos = world.entities.get(&p1_tank).unwrap().position;

        // 1. Reject NaN / Inf coordinates across all positional GameCommands
        let bad_coords = [
            [f32::NAN, 0.0, 10.0],
            [10.0, f32::INFINITY, 10.0],
            [10.0, 0.0, f32::NEG_INFINITY],
            [2_000_000.0, 0.0, 10.0],
        ];
        for bad in bad_coords {
            for cmd in [
                GameCommand::MoveUnits {
                    player_id: 1,
                    entity_ids: vec![p1_tank],
                    target_pos: bad,
                },
                GameCommand::QueueMoveUnits {
                    player_id: 1,
                    entity_ids: vec![p1_tank],
                    target_pos: bad,
                },
                GameCommand::AttackMove {
                    player_id: 1,
                    entity_ids: vec![p1_tank],
                    target_pos: bad,
                },
                GameCommand::PatrolUnits {
                    player_id: 1,
                    entity_ids: vec![p1_tank],
                    target_pos: bad,
                },
                GameCommand::ReverseMove {
                    player_id: 1,
                    entity_ids: vec![p1_tank],
                    target_pos: bad,
                },
                GameCommand::AttackGround {
                    player_id: 1,
                    attacker_ids: vec![p1_tank],
                    target_pos: bad,
                },
                GameCommand::SetRallyPoint {
                    player_id: 1,
                    building_ids: vec![1],
                    rally_pos: bad,
                },
                GameCommand::QueueRallyPoint {
                    player_id: 1,
                    building_ids: vec![1],
                    rally_pos: bad,
                },
                GameCommand::PingLocation {
                    player_id: 1,
                    position: bad,
                },
                GameCommand::CastSupportPower {
                    player_id: 1,
                    power_id: "paratrooper_drop".to_string(),
                    source_pos: Some(bad),
                    target_pos: [20.0, 0.0, 20.0],
                },
                GameCommand::CastTargetedAbility {
                    player_id: 1,
                    caster_ids: vec![p1_tank],
                    target_pos: bad,
                    target_entity_id: None,
                },
            ] {
                assert!(
                    !cmd.is_payload_valid(),
                    "Non-finite or extreme coordinate command must be rejected: {cmd:?}"
                );
                world.push_command(cmd);
            }
        }
        for bad_tile_cmd in [
            GameCommand::PlaceBuilding {
                player_id: 1,
                building_id: "allied_power_plant".to_string(),
                tile_pos: [i32::MAX, 10],
            },
            GameCommand::PlaceWallLine {
                player_id: 1,
                building_id: "allied_fortress_wall".to_string(),
                start_tile: [10, 10],
                end_tile: [i32::MIN, 10],
            },
            GameCommand::PlaceMine {
                player_id: 1,
                unit_ids: vec![p1_tank],
                target_tile: [999_999, 10],
            },
        ] {
            assert!(!bad_tile_cmd.is_payload_valid());
            world.push_command(bad_tile_cmd);
        }
        world.step(&loaded);
        let after_tank = world.entities.get(&p1_tank).unwrap();
        assert_eq!(after_tank.position, orig_pos);
        assert!(after_tank.path_waypoints.is_empty());
        assert!(after_tank.attack_ground_pos.is_none());
        assert!(world.active_pings.is_empty());
        let saved_json =
            serde_json::to_string(&world).expect("SimWorld must remain valid JSON without NaN/Inf");
        assert!(!saved_json.contains("NaN"));

        // 2. Multiplayer gate on SpawnSandboxUnit and SetEnvironmentPreset + owner_player_id
        world.multiplayer_mode = true;
        let count_before = world.entities.len();
        let spawn_cmd = GameCommand::SpawnSandboxUnit {
            player_id: 1,
            unit_id: "apocalypse_tank".to_string(),
            tile_pos: [12, 16],
        };
        assert_eq!(spawn_cmd.owner_player_id(), Some(1));
        world.push_command(spawn_cmd.clone());
        world.push_command(GameCommand::SetEnvironmentPreset {
            player_id: 2,
            preset: EnvironmentPreset::Night,
        });
        world.step(&loaded);
        assert_eq!(
            world.entities.len(),
            count_before,
            "SpawnSandboxUnit must be rejected in multiplayer_mode"
        );
        assert_eq!(
            world.environment_preset,
            EnvironmentPreset::Day,
            "Non-host player_id=2 must not change EnvironmentPreset in multiplayer_mode"
        );

        // Host (player_id == 1) CAN change EnvironmentPreset; singleplayer allows SpawnSandboxUnit
        world.push_command(GameCommand::SetEnvironmentPreset {
            player_id: 1,
            preset: EnvironmentPreset::Night,
        });
        world.step(&loaded);
        assert_eq!(world.environment_preset, EnvironmentPreset::Night);

        world.multiplayer_mode = false;
        world.push_command(spawn_cmd);
        world.step(&loaded);
        assert_eq!(world.entities.len(), count_before + 1);

        // 3. Water-bound building dry-land rejection & PlaceWallLine non-wall rejection + per-segment cost
        let _af = world.spawn_building(1, "allied_war_factory", [6, 17], &loaded, false);
        let dry_count_before = world.entities.len();
        world.push_command(GameCommand::PlaceBuilding {
            player_id: 1,
            building_id: "allied_naval_shipyard".to_string(),
            tile_pos: [2, 8],
        });
        world.push_command(GameCommand::PlaceWallLine {
            player_id: 1,
            building_id: "allied_power_plant".to_string(),
            start_tile: [2, 6],
            end_tile: [4, 6],
        });
        world.step(&loaded);
        assert_eq!(
            world.entities.len(),
            dry_count_before,
            "Water-bound shipyard on dry land and PlaceWallLine with non-wall building must be rejected"
        );

        let wall_id = loaded
            .buildings
            .values()
            .find(|b| b.role == rts_sdk::schema::BuildingRole::Wall)
            .map(|b| b.id.clone())
            .unwrap();
        let wall_cost = loaded.buildings.get(&wall_id).unwrap().cost;
        let credits_before_wall = world.players.get(&1).unwrap().credits;
        world.push_command(GameCommand::PlaceWallLine {
            player_id: 1,
            building_id: wall_id,
            start_tile: [4, 6],
            end_tile: [7, 6],
        });
        world.step(&loaded);
        let credits_after_wall = world.players.get(&1).unwrap().credits;
        assert_eq!(
            credits_before_wall - credits_after_wall,
            wall_cost * 4,
            "PlaceWallLine across 4 unblocked tiles within build radius must charge 4 * wall_cost"
        );

        // 4. Unified superweapon cooldown between CastTargetedAbility and CastSupportPower
        let cs_id = world
            .spawn_building(1, "allied_chronosphere", [2, 18], &loaded, false)
            .unwrap();
        for i in 0..4 {
            let _ = world.spawn_building(1, "allied_power_plant", [2 + i * 3, 22], &loaded, false);
        }
        world.entities.get_mut(&cs_id).unwrap().ability_cooldown = 0.0;
        let mover = world
            .spawn_unit(1, "grizzly_tank", [10, 20], &loaded)
            .unwrap();
        world.push_command(GameCommand::CastTargetedAbility {
            player_id: 1,
            caster_ids: vec![cs_id],
            target_pos: world.grid.tile_to_world([12, 20]),
            target_entity_id: Some(mover),
        });
        world.step(&loaded);
        assert!(world.entities.get(&cs_id).unwrap().ability_cooldown > 0.0);
        let synced_sw_cd = world
            .players
            .get(&1)
            .unwrap()
            .support_power_cooldowns
            .values()
            .copied()
            .fold(0.0_f32, f32::max);
        assert!(
            synced_sw_cd > 0.0,
            "Casting superweapon via CastTargetedAbility must also set support_power_cooldowns"
        );
    }

    #[test]
    fn state_hash_completeness_seeded_rng_and_save_load_continuity() {
        let loaded = load_ra2_mod();
        let mut world = SimWorld::from_mod(&loaded).with_seed(0xCAFE_BABE_1234_5678);
        let unit_id = world
            .spawn_unit(1, "grizzly_tank", [16, 16], &loaded)
            .unwrap();

        let mut prev_hash = world.state_hash();
        let mut assert_hash_changes = |w: &SimWorld, label: &str| {
            let next = w.state_hash();
            assert_ne!(
                next, prev_hash,
                "state_hash() must change when {label} mutates"
            );
            prev_hash = next;
        };

        world.entities.get_mut(&unit_id).unwrap().weapon_cooldown = 1.25;
        assert_hash_changes(&world, "SimEntity.weapon_cooldown");

        world.entities.get_mut(&unit_id).unwrap().ability_cooldown = 3.5;
        assert_hash_changes(&world, "SimEntity.ability_cooldown");

        world.entities.get_mut(&unit_id).unwrap().reload_timer = 0.75;
        assert_hash_changes(&world, "SimEntity.reload_timer");

        world.entities.get_mut(&unit_id).unwrap().experience = 120.0;
        assert_hash_changes(&world, "SimEntity.experience");

        world.entities.get_mut(&unit_id).unwrap().facing_radians = 1.1;
        assert_hash_changes(&world, "SimEntity.facing_radians");

        world
            .entities
            .get_mut(&unit_id)
            .unwrap()
            .path_waypoints
            .push([30.0, 0.0, 30.0]);
        assert_hash_changes(&world, "SimEntity.path_waypoints");

        world
            .entities
            .get_mut(&unit_id)
            .unwrap()
            .script_ctx
            .custom_flags
            .insert("charge_stacks".to_string(), true);
        assert_hash_changes(&world, "SimEntity.script_ctx.custom_flags");

        world.grid.blocked_tiles.insert([41, 41]);
        assert_hash_changes(&world, "PathGrid.blocked_tiles");

        world
            .fired_triggers
            .insert("custom_trigger_alpha".to_string());
        assert_hash_changes(&world, "SimWorld.fired_triggers");

        world.power_outage_timers.insert(1, 15.0);
        assert_hash_changes(&world, "SimWorld.power_outage_timers");

        world.koth_timers.insert(1, 22.5);
        assert_hash_changes(&world, "SimWorld.koth_timers");

        world
            .players
            .get_mut(&1)
            .unwrap()
            .researched_upgrades
            .insert("composite_armor".to_string());
        assert_hash_changes(&world, "PlayerEconomy.researched_upgrades IDs");

        // Replace upgrade ID while keeping set length == 1 to prove actual IDs (not just .len()) are hashed
        {
            let upgs = &mut world.players.get_mut(&1).unwrap().researched_upgrades;
            upgs.clear();
            upgs.insert("jet_afterburners".to_string());
        }
        assert_hash_changes(&world, "PlayerEconomy.researched_upgrades distinct ID");

        world.players.get_mut(&1).unwrap().building_queue =
            Some(("allied_pillbox".to_string(), 2.0, 5.0));
        assert_hash_changes(&world, "PlayerEconomy.building_queue");

        world.rng_state ^= 0xDEAD_BEEF;
        assert_hash_changes(&world, "SimWorld.rng_state");

        world.projectiles.push(SimProjectile {
            id: 9001,
            attacker_id: unit_id,
            attacker_player: 1,
            target_id: 0,
            weapon_id: "cannon_105mm".to_string(),
            damage: 65.0,
            start_pos: [10.0, 1.0, 10.0],
            current_pos: [12.0, 1.5, 10.0],
            target_pos: [20.0, 0.0, 10.0],
            progress: 0.25,
            speed: 30.0,
            total_dist: 10.0,
            arc_height: 1.0,
            delivery: DeliveryKind::Ballistic,
            friendly_fire: false,
            projectile_style: ProjectileStyle::Bullet,
            splash_radius: 1.5,
            mind_control: false,
            mind_control_limit: 1,
            warp_erase: false,
            c4_demolition: false,
            clears_garrison: false,
            health: 0.0,
            max_health: 0.0,
            ..Default::default()
        });
        assert_hash_changes(&world, "SimProjectile");
        world.projectiles[0].progress = 0.55;
        assert_hash_changes(&world, "SimProjectile.progress");

        // Save/Load 20-tick continuation must produce bit-identical state_hash
        let json = serde_json::to_string(&world).unwrap();
        let mut loaded_copy: SimWorld = serde_json::from_str(&json).unwrap();
        assert_eq!(world.state_hash(), loaded_copy.state_hash());
        assert_eq!(world.subsystem_hashes(), loaded_copy.subsystem_hashes());
        for _ in 0..20 {
            world.step(&loaded);
            loaded_copy.step(&loaded);
        }
        assert_eq!(
            world.state_hash(),
            loaded_copy.state_hash(),
            "Saved and reloaded SimWorld must stay lockstep-identical across subsequent ticks"
        );

        // Seeded PRNG: same seed -> identical crate spawns; different seed -> divergent crate spawns
        let mut w_seed_a1 = SimWorld::from_mod(&loaded).with_seed(111_222_333);
        let mut w_seed_a2 = SimWorld::from_mod(&loaded).with_seed(111_222_333);
        let mut w_seed_b = SimWorld::from_mod(&loaded).with_seed(999_888_777);
        let interval = loaded.manifest.rules.crate_spawn_interval_ticks.max(1);
        w_seed_a1.tick = interval;
        w_seed_a2.tick = interval;
        w_seed_b.tick = interval;
        w_seed_a1.step_crates(&loaded);
        w_seed_a2.step_crates(&loaded);
        w_seed_b.step_crates(&loaded);
        assert_eq!(w_seed_a1.crates, w_seed_a2.crates);
        assert_ne!(w_seed_a1.crates, w_seed_b.crates);
    }

    #[test]
    fn slave_miner_single_income_and_single_self_heal() {
        let loaded = load_ra2_mod();
        let mut world = SimWorld::from_mod(&loaded);
        world.entities.clear();
        world.grid.blocked_tiles.clear();
        world.grid.ore_tiles.clear();
        world.grid.gem_tiles.clear();
        world.ore_regen_per_sec = 0.0;

        world.grid.ore_tiles.insert([15, 15], 500);
        world.players.get_mut(&2).unwrap().credits = 1_000;
        world.players.get_mut(&2).unwrap().credits_harvested = 0;

        let miner_id = world
            .spawn_unit(2, "yuri_slave_miner", [14, 15], &loaded)
            .unwrap();
        {
            let m = world.entities.get_mut(&miner_id).unwrap();
            m.health = m.max_health - 50.0;
        }
        let hp_before = world.entities.get(&miner_id).unwrap().health;

        // Run 20 ticks (1.0s at dt=0.05):
        // Tick 1 transitions Idle -> Harvesting(timer=0.0); Tick 2 scoops 100 (timer->0.5);
        // Tick 12 scoops 100 (timer->0.5). Tick 20 MUST NOT double-mine or add +4.0 extra heal!
        for _ in 0..20 {
            world.step(&loaded);
        }

        let harvested = world.players.get(&2).unwrap().credits_harvested;
        let rem_ore = world.grid.ore_tiles.get(&[15, 15]).copied().unwrap_or(0);
        let hp_after = world.entities.get(&miner_id).unwrap().health;
        assert_eq!(
            harvested, 200,
            "Slave Miner must refine strictly single-source income (200 across 20 ticks, got {harvested})"
        );
        assert_eq!(rem_ore, 300);
        let healed = hp_after - hp_before;
        assert!(
            healed > 0.0 && healed <= 12.5,
            "Deployed Slave Miner must heal only via the single 12 HP/s loop (got +{healed} HP)"
        );
    }

    #[test]
    fn ai_build_radius_airfield_jet_survival_attack_move_and_fow_fairness() {
        let mut loaded = load_ra2_mod();
        let mut world = SimWorld::from_mod(&loaded);
        world.entities.clear();
        world.grid.blocked_tiles.clear();
        world.grid.water_tiles.clear();

        // 1. AI build placement must strictly satisfy is_within_build_radius
        let _cy = world
            .spawn_building(2, "allied_conyard", [24, 24], &loaded, false)
            .unwrap();
        let _pp = world
            .spawn_building(2, "allied_power_plant", [28, 24], &loaded, false)
            .unwrap();
        world.players.get_mut(&2).unwrap().credits = 10_000;
        let cmds = generate_ai_commands_with_difficulty(&world, 2, &loaded, AiDifficulty::Hard);
        let placed = cmds.iter().find_map(|c| match c {
            GameCommand::PlaceBuilding {
                building_id,
                tile_pos,
                ..
            } => Some((building_id.clone(), *tile_pos)),
            _ => None,
        });
        let (b_id, place_tile) = placed.expect("AI should emit PlaceBuilding");
        let fp = loaded.buildings.get(&b_id).unwrap().footprint;
        assert!(
            world.is_within_build_radius(2, place_tile, fp),
            "AI chosen build tile {place_tile:?} must be within build radius"
        );

        // 2. Docked Airfield jets must NOT be rallied off pads or overproduced to crash
        let _ref = world
            .spawn_building(2, "allied_refinery", [20, 24], &loaded, false)
            .unwrap();
        let _bar = world
            .spawn_building(2, "allied_barracks", [24, 28], &loaded, false)
            .unwrap();
        let af_id = world
            .spawn_building(2, "allied_airforce_command", [20, 28], &loaded, false)
            .unwrap();
        let pad_cap = SimWorld::building_airfield_pad_capacity(
            loaded.buildings.get("allied_airforce_command").unwrap(),
        );
        let mut jet_ids = Vec::new();
        for i in 0..pad_cap {
            let jid = world
                .spawn_unit(2, "harrier", [20 + (i as i32 % 2), 28], &loaded)
                .unwrap();
            let af_pos = world.entities.get(&af_id).unwrap().position;
            {
                let j = world.entities.get_mut(&jid).unwrap();
                j.position = [af_pos[0], 0.45, af_pos[2]];
                j.landed_at_airfield = true;
            }
            world.set_home_airfield_id(jid, Some(af_id));
            jet_ids.push(jid);
        }
        for _ in 0..30 {
            let ai_cmds =
                generate_ai_commands_with_difficulty(&world, 2, &loaded, AiDifficulty::Hard);
            for cmd in ai_cmds {
                if let GameCommand::ProduceUnit { ref unit_id, .. } = cmd {
                    assert_ne!(
                        unit_id, "harrier",
                        "AI must not queue pad-dependent jets when Airfield pads are already full"
                    );
                }
                world.push_command(cmd);
            }
            world.step(&loaded);
        }
        for &jid in &jet_ids {
            let jet = world
                .entities
                .get(&jid)
                .expect("Docked AI Harrier Jet must survive without crashing");
            assert!(jet.health > 0.0);
            assert!(
                jet.landed_at_airfield,
                "Idle AI Harrier Jet must remain docked on its Airfield pad"
            );
        }

        // 3. Medium AI respects shroud (`explored_tiles`) + cloak, and dispatches AttackMove wave
        let hidden_enemy = world.spawn_unit(1, "mirage_tank", [4, 4], &loaded).unwrap();
        world
            .entities
            .get_mut(&hidden_enemy)
            .unwrap()
            .permanent_cloak = true;
        world.update_explored_tiles(&loaded);
        for i in 0..9 {
            let _ = world.spawn_unit(2, "grizzly_tank", [22 + i, 22], &loaded);
        }
        world.tick = 1500; // >= grace period (900), multiple of decision_interval (20) & wave_interval (500)
        let wave_cmds =
            generate_ai_commands_with_difficulty(&world, 2, &loaded, AiDifficulty::Medium);
        assert!(
            !wave_cmds.iter().any(|c| matches!(
                c,
                GameCommand::AttackTarget { target_entity_id, .. } if *target_entity_id == hidden_enemy
            )),
            "Medium AI must never maphack an undetected cloaked unit in shroud"
        );
        assert!(
            wave_cmds
                .iter()
                .any(|c| matches!(c, GameCommand::AttackMove { .. })),
            "Medium AI wave must dispatch GameCommand::AttackMove to scout/engage"
        );

        // 4. Preferred superweapon selection via AiProfileDef::preferred_superweapons
        let _wf = world
            .spawn_building(2, "allied_war_factory", [28, 28], &loaded, false)
            .unwrap();
        let _ref2 = world
            .spawn_building(2, "allied_refinery", [15, 24], &loaded, false)
            .unwrap();
        let _def = world
            .spawn_building(2, "allied_pillbox", [19, 22], &loaded, false)
            .unwrap();
        let _lab = world
            .spawn_building(2, "allied_battle_lab", [15, 28], &loaded, false)
            .unwrap();
        for i in 0..4 {
            let _ = world
                .spawn_building(2, "allied_power_plant", [33, 16 + i * 3], &loaded, false)
                .unwrap();
        }
        {
            let p2 = world.players.get_mut(&2).unwrap();
            p2.faction = "allies".to_string();
            p2.credits = 12_000;
            p2.power_produced = 2_000;
            p2.building_queue = None;
        }
        loaded.ai_profiles.clear();
        loaded.ai_profiles.insert(
            "allies_sw_pref".to_string(),
            AiProfileDef {
                id: "allies_sw_pref".to_string(),
                faction: "allies".to_string(),
                personality: "balanced".to_string(),
                build_order: Vec::new(),
                unit_weights: Default::default(),
                min_attack_wave_size: None,
                preferred_superweapons: vec!["weather_lightning_storm".to_string()],
                on_ai_tick: None,
                ..Default::default()
            },
        );
        world.tick = 10;
        let sw_cmds = generate_ai_commands_with_profile(
            &world,
            2,
            &loaded,
            AiDifficulty::Hard,
            AiPersonality::Balanced,
        );
        assert!(
            sw_cmds.iter().any(|c| matches!(
                c,
                GameCommand::PlaceBuilding { building_id, .. } if building_id == "allied_weather_controller"
            )),
            "AI should honor AiProfileDef::preferred_superweapons when choosing a superweapon: {sw_cmds:?}"
        );
    }

    #[test]
    fn spatial_combat_scaling_cliff_los_and_repath_cooldown() {
        let loaded = load_ra2_mod();
        let mut world = SimWorld::from_mod(&loaded);
        world.entities.clear();
        world.grid.blocked_tiles.clear();
        world.grid.elevation_tiles.clear();
        world.grid.ramp_tiles.clear();

        // 1. Intermediate cliff plateau blocks low-ground LoS & direct combat targeting
        for x in 14..=16 {
            for z in 10..=20 {
                world.grid.elevation_tiles.insert([x, z]);
            }
        }
        assert!(
            !world.grid.has_line_of_sight([11, 15], 0.0, [19, 15]),
            "Intermediate cliff wall must occlude low-ground-to-low-ground LoS"
        );
        let low_west = world
            .spawn_unit(1, "grizzly_tank", [11, 15], &loaded)
            .unwrap();
        let low_east = world
            .spawn_unit(2, "rhino_tank", [18, 15], &loaded)
            .unwrap();
        assert!(
            !crate::combat::is_target_visible_to(
                &world,
                &loaded,
                1,
                world.entities.get(&low_east).unwrap()
            ),
            "Hostile unit behind intermediate cliff plateau must not be visible without spotter"
        );
        let east_hp_before = world.entities.get(&low_east).unwrap().health;
        world.step(&loaded);
        assert_eq!(
            world.entities.get(&low_east).unwrap().health,
            east_hp_before,
            "Direct-fire tank must not shoot through an intermediate cliff wall"
        );
        let _ = low_west;

        // 2. Diagonal wall corner-cutting prevention & unreachable target repath_cooldown_ticks
        world.entities.clear();
        world.grid.elevation_tiles.clear();
        world.grid.blocked_tiles.insert([11, 10]);
        world.grid.blocked_tiles.insert([10, 11]);
        let diag_path = world.grid.find_path_with_domain_and_gates(
            [10, 10],
            [11, 11],
            MovementDomain::Ground,
            &[],
        );
        assert_ne!(
            diag_path.first().copied(),
            Some(world.grid.tile_to_world([11, 11])),
            "A* must not cut diagonally between two touching orthogonal wall tiles"
        );

        // Seal target inside an unreachable box and verify repath_cooldown_ticks throttles A*
        world.grid.blocked_tiles.clear();
        for x in 29..=31 {
            for z in 29..=31 {
                if [x, z] != [30, 30] {
                    world.grid.blocked_tiles.insert([x, z]);
                }
            }
        }
        let chaser = world
            .spawn_unit(1, "grizzly_tank", [20, 30], &loaded)
            .unwrap();
        let boxed_enemy = world
            .spawn_unit(2, "soviet_conscript", [30, 30], &loaded)
            .unwrap();
        world.push_command(GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![chaser],
            target_entity_id: boxed_enemy,
        });
        world.step(&loaded);
        assert!(
            world.entities.get(&chaser).unwrap().repath_cooldown_ticks > 0,
            "Failed A* chase to sealed target must set repath_cooldown_ticks > 0"
        );

        // 3. Spatial combat scaling: 400 combat units step cleanly in milliseconds (no O(N^3) stall)
        world.entities.clear();
        world.grid.blocked_tiles.clear();
        for i in 0..200 {
            let x = 2 + (i % 20);
            let z = 2 + (i / 20);
            let _ = world.spawn_unit(1, "grizzly_tank", [x, z], &loaded);
            let _ = world.spawn_unit(2, "rhino_tank", [x + 22, z + 22], &loaded);
        }
        let t0 = std::time::Instant::now();
        world.step(&loaded);
        let elapsed = t0.elapsed();
        assert!(
            elapsed.as_millis() < 1_500,
            "400-unit combat step must run without O(N^3) blowup (took {elapsed:?})"
        );
    }

    #[test]
    fn bridge_demolition_naval_passage_abilities_console_and_non_square_mapgen() {
        let mut loaded = load_ra2_mod();
        let mut world = SimWorld::from_mod(&loaded);
        world.entities.clear();
        world.grid.blocked_tiles.clear();
        world.grid.water_tiles.clear();
        world.grid.bridge_tiles.clear();
        world.grid.bridge_seed_tiles.clear();

        // 1. River across x=20..=21, z=10..=20 with a bridge span at [20, 15], footprint [2, 2]
        for x in 20..=21 {
            for z in 10..=20 {
                world.grid.water_tiles.insert([x, z]);
            }
        }
        for x in 20..=21 {
            for z in 15..=16 {
                world.grid.bridge_seed_tiles.insert([x, z]);
            }
        }
        world.grid.set_footprint_bridge([20, 15], [2, 2], true);

        let ground_before = world.grid.find_path_with_domain_and_gates(
            [18, 15],
            [23, 15],
            MovementDomain::Ground,
            &[],
        );
        assert!(
            !ground_before.is_empty(),
            "Ground units must cross intact river bridge"
        );

        // Demolish the map bridge via AttackGround with an artillery/siege unit
        let siege = world
            .spawn_unit(1, "prism_tank", [15, 15], &loaded)
            .unwrap();
        world.push_command(GameCommand::AttackGround {
            player_id: 1,
            attacker_ids: vec![siege],
            target_pos: world.grid.tile_to_world([20, 15]),
        });
        for _ in 0..8 {
            world.step(&loaded);
        }
        assert!(
            !world.grid.bridge_tiles.contains(&[20, 15]),
            "AttackGround must demolish map bridge_tiles"
        );
        assert!(
            !world.destroyed_bridges.is_empty(),
            "Demolishing map bridge_tiles must record a DestroyedBridgeSpan for Engineer repair"
        );
        assert!(
            !world.grid.blocked_tiles.contains(&[20, 15]),
            "Demolished bridge over water_tiles must NOT leave blocked_tiles blocking ships"
        );

        let naval_through_gap = world.grid.find_path_with_domain_and_gates(
            [20, 11],
            [20, 19],
            MovementDomain::Naval,
            &[],
        );
        assert!(
            !naval_through_gap.is_empty(),
            "Naval units must sail freely through demolished bridge water gap"
        );
        let amphib_through_gap = world.grid.find_path_with_domain_and_gates(
            [18, 15],
            [23, 15],
            MovementDomain::Amphibious,
            &[],
        );
        assert!(
            !amphib_through_gap.is_empty(),
            "Amphibious units must swim across demolished bridge water gap"
        );

        // Engineer walks to the destroyed bridge shore and rebuilds the bridge span
        world.entities.remove(&siege);
        world.destroyed_bridges = vec![DestroyedBridgeSpan {
            building_id: String::new(),
            tile_origin: [20, 15],
            footprint: [2, 2],
            rotated: false,
            destroyed_by_player_id: 0,
        }];
        let eng = world
            .spawn_unit(1, "allied_engineer", [19, 15], &loaded)
            .unwrap();
        world.step(&loaded);
        assert!(
            world.grid.bridge_tiles.contains(&[20, 15]) && !world.entities.contains_key(&eng),
            "Engineer adjacent to DestroyedBridgeSpan must rebuild bridge_tiles and consume itself"
        );

        // 2. AbilityDef { cost, range, target_mode } enforcement
        if let Some(u_def) = loaded.units.get_mut("grizzly_tank") {
            u_def.abilities = vec![AbilityDef {
                id: "paid_reinforce_drop".to_string(),
                name: "Paid Reinforce Drop".to_string(),
                cooldown_secs: 10.0,
                cost: 500,
                range: 6.0,
                radius: 3.0,
                target_mode: AbilityTargetMode::GroundPoint,
                applies_status: None,
                payloads: vec![EffectPayload::SpawnUnits {
                    unit_id: Some("allied_gi".to_string()),
                    count: 1,
                    parachute: false,
                }],
                script_hook: None,
                ..Default::default()
            }];
        }
        let caster = world
            .spawn_unit(1, "grizzly_tank", [10, 10], &loaded)
            .unwrap();
        let ents_before_ability = world.entities.len();
        world.players.get_mut(&1).unwrap().credits = 300; // < 500 cost -> rejected
        world.push_command(GameCommand::CastTargetedAbilityIndex {
            player_id: 1,
            caster_ids: vec![caster],
            ability_index: 0,
            target_pos: world.grid.tile_to_world([10, 10]),
            target_entity_id: None,
        });
        world.step(&loaded);
        assert_eq!(world.players.get(&1).unwrap().credits, 300);
        assert_eq!(world.entities.len(), ents_before_ability);

        world.players.get_mut(&1).unwrap().credits = 1_000;
        // Out of range (distance > 6.0) -> rejected
        world.push_command(GameCommand::CastTargetedAbilityIndex {
            player_id: 1,
            caster_ids: vec![caster],
            ability_index: 0,
            target_pos: world.grid.tile_to_world([30, 30]),
            target_entity_id: None,
        });
        world.step(&loaded);
        assert_eq!(world.players.get(&1).unwrap().credits, 1_000);
        assert_eq!(world.entities.len(), ents_before_ability);

        // In range & affordable -> deducts 500 credits and spawns reinforcement
        world.push_command(GameCommand::CastTargetedAbilityIndex {
            player_id: 1,
            caster_ids: vec![caster],
            ability_index: 0,
            target_pos: world.grid.tile_to_world([11, 10]),
            target_entity_id: None,
        });
        world.step(&loaded);
        assert_eq!(world.players.get(&1).unwrap().credits, 500);
        assert_eq!(world.entities.len(), ents_before_ability + 1);

        // 3. CrateKind::VeterancyPromo syncs experience, Mind-Control recalculates power,
        //    dev-console `trigger <id>` arg order, and non-square Rotational4Way procedural map bounds
        let rookie = world.spawn_unit(1, "allied_gi", [12, 12], &loaded).unwrap();
        world.spawn_crate([12, 12], CrateKind::VeterancyPromo);
        world.step(&loaded);
        let promoted = world.entities.get(&rookie).unwrap();
        assert_eq!(promoted.veterancy, VeterancyRank::Veteran);
        assert!(
            promoted.experience >= loaded.manifest.rules.veteran_xp_threshold
                && promoted.experience > 0.0,
            "VeterancyPromo crate must sync experience with VeterancyRank::Veteran"
        );

        world.trigger_zones.push(rts_sdk::schema::TriggerZoneDef {
            id: "custom_mission_trig".to_string(),
            min: [0, 0],
            max: [1, 1],
            once: true,
            condition: None,
            actions: vec![rts_sdk::schema::TriggerAction::GrantCredits {
                player_id: 1,
                amount: 777,
            }],
            script_hook: String::new(),
        });
        let creds_pre_trig = world.players.get(&1).unwrap().credits;
        let out = world
            .execute_dev_console_command("trigger custom_mission_trig", 1, &loaded, false)
            .expect("dev console trigger command should succeed");
        assert!(out.contains("custom_mission_trig"));
        assert_eq!(world.players.get(&1).unwrap().credits, creds_pre_trig + 777);

        let rect_map = generate_procedural_map(
            &MapGenConfig {
                id: "rect_rot4".to_string(),
                name: "Rectangular 4-Way".to_string(),
                seed: 7,
                width: 80,
                height: 44,
                player_count: 4,
                symmetry: MapSymmetryMode::Rotational4Way,
                ore_patches_per_player: 2,
            },
            &loaded,
        );
        for sp in &rect_map.spawns {
            assert!(
                sp.base_tile[0] >= 0
                    && sp.base_tile[0] < 80
                    && sp.base_tile[1] >= 0
                    && sp.base_tile[1] < 44,
                "Non-square Rotational4Way spawn {:?} must stay strictly in 80x44 bounds",
                sp.base_tile
            );
        }
        for ore in &rect_map.ore_patches {
            assert!(
                ore.position[0] >= 0
                    && ore.position[0] < 80
                    && ore.position[1] >= 0
                    && ore.position[1] < 44,
                "Non-square Rotational4Way ore {:?} must stay strictly in 80x44 bounds",
                ore.position
            );
        }

        // Verify mind-control capture of a power building immediately recalculates both players' power
        let p2_pp = world
            .spawn_building(2, "soviet_tesla_reactor", [14, 18], &loaded, false)
            .unwrap();
        let yuri_prime = world
            .spawn_unit(1, "yuri_prime", [14, 21], &loaded)
            .unwrap();
        let p1_pwr_before = world.players.get(&1).unwrap().power_produced;
        let p2_pwr_before = world.players.get(&2).unwrap().power_produced;
        world.push_command(GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![yuri_prime],
            target_entity_id: p2_pp,
        });
        for _ in 0..4 {
            world.step(&loaded);
        }
        assert_eq!(world.entities.get(&p2_pp).unwrap().player_id, 1);
        assert!(
            world.players.get(&1).unwrap().power_produced > p1_pwr_before
                && world.players.get(&2).unwrap().power_produced < p2_pwr_before,
            "Mind-controlling an enemy power plant must immediately recalculate power for both players"
        );
        let _ = EntityKind::Unit {
            unit_id: String::new(),
        };
    }
}
