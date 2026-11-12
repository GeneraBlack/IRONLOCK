//! Integration & unit tests for Phases 27–28 (`Tasks 233, 234, 239-Sim, 242-Sim, 243-Sim, 248-Sim, 251`):
//! - Live `FlowFieldHandle` steering (zero `Vec<[f32;3]>` cloning), `16x16` dirty-sector `HpaGraphCache`,
//!   `SimScalar` Velocity-Space ORCA + `YieldStepAside` friendly nudging, and flat `CollisionGrid` storage.
//! - Pre-fire 3D parabolic LOF (`has_ballistic_line_of_fire`), swept 3D projectile wall anti-tunneling,
//!   finite missile `turn_rate_deg_per_sec`, and penetrating `DeliveryKind::Beam` & `Cone`.
//! - `MoveAlongCustomCurve`, `AreaCommand` (`AreaRepair`, `AreaHarvest`, `AreaAttack`, `AreaGuard`),
//!   and `ReclaimHumanSlot`.
//! - `TriggerGraphDef` topological evaluation (`last_fired_wires` + `Delay` + `LogicGate` + cycle guard),
//!   `BehaviorTreeDef` AI (`blackboard` + `active_node_path`), `mutator-turbo-economy` overlay stack,
//!   and cross-profile (`debug == --release`) Showcase Mod #2 `state_hash()` verification.

use super::{load_ra2_mod, load_showcase2_mod, load_showcase2_with_turbo_mutator};
use crate::ai::{step_ai_player_in_world, AiCommanderState, AiDifficulty};
use crate::combat::{has_ballistic_line_of_fire, SimProjectile};
use crate::command::{AreaCommandKind, GameCommand};
use crate::det_math::det_distance_xz_f32;
use crate::world::SimWorld;
use rts_sdk::schema::{
    DeliveryKind, LogicGateOp, MapDef, MovementDomain, PlayerSpawnDef, TriggerAction,
    TriggerCondition, TriggerEventKind, TriggerGraphDef, TriggerGraphNode, TriggerGraphNodeKind,
    TriggerGraphWire,
};

fn make_empty_world_with_size(
    loaded: &rts_sdk::loader::LoadedMod,
    width: u32,
    height: u32,
) -> SimWorld {
    let faction_id = loaded
        .factions
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| "solaris".to_string());
    let map = MapDef {
        id: "phase27_28_arena".to_string(),
        name: "Phase 27-28 Arena".to_string(),
        width,
        height,
        tile_size: 2.0,
        spawns: vec![
            PlayerSpawnDef {
                player_id: 1,
                faction: faction_id.clone(),
                sub_faction: None,
                base_tile: [6, 6],
                team_id: 1,
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
            PlayerSpawnDef {
                player_id: 2,
                faction: faction_id,
                sub_faction: None,
                base_tile: [(width as i32) - 8, (height as i32) - 8],
                team_id: 2,
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
        ],
        ..Default::default()
    };
    let mut world = SimWorld::from_mod_map(loaded, &map);
    world.capture_zones.clear();
    world.victory_tickets.clear();
    world
}

fn make_showcase2_world(loaded: &rts_sdk::loader::LoadedMod) -> SimWorld {
    let mut world = make_empty_world_with_size(loaded, 64, 64);
    for pid in [1_u8, 2_u8] {
        if let Some(econ) = world.players.get_mut(&pid) {
            econ.credits = 5_000;
            econ.current_era = loaded.manifest.rules.eras.first().map(|e| e.id.clone());
            econ.resources.insert("credits".to_string(), 5_000);
            econ.resources.insert("aether".to_string(), 3_000);
            econ.resources.insert("relic_flux".to_string(), 800);
            econ.power_produced = 400;
            econ.power_consumed = 20;
            econ.supply_cap = 120;
        }
    }
    world
}

#[test]
fn task233_live_flow_field_handle_dirty_hpa_and_orca_yield_step_aside() {
    let loaded = load_showcase2_mod();
    let mut world = make_showcase2_world(&loaded);

    // 1. Group >= 8 units uses shared live `FlowFieldHandle` without cloning 300x waypoint vectors
    let mut group_ids = Vec::new();
    for i in 0..10 {
        let uid = world
            .spawn_unit(1, "legion_sentinel", [8 + (i % 5), 8 + (i / 5)], &loaded)
            .unwrap();
        group_ids.push(uid);
    }
    let goal_world = world.grid.tile_to_world([48, 48]);
    world.apply_command(
        GameCommand::MoveUnits {
            player_id: 1,
            entity_ids: group_ids.clone(),
            target_pos: goal_world,
        },
        &loaded,
    );
    let shared_handle = world.entities[&group_ids[0]].active_flow_handle;
    assert!(
        shared_handle.is_some(),
        "Group >= 4 units must be assigned a shared live FlowFieldHandle"
    );
    let flow_assigned = group_ids
        .iter()
        .filter(|&&id| {
            let e = &world.entities[&id];
            e.active_flow_handle == shared_handle && e.path_waypoints.len() <= 1
        })
        .count();
    assert_eq!(
        flow_assigned, 10,
        "All 10 marching units must steer off the same FlowFieldHandle without multi-waypoint paths"
    );

    let start_centroid_x: f32 = group_ids
        .iter()
        .map(|&id| world.entities[&id].position[0])
        .sum::<f32>()
        / 10.0;
    for _ in 0..15 {
        world.step(&loaded);
    }
    let moved_centroid_x: f32 = group_ids
        .iter()
        .map(|&id| world.entities[&id].position[0])
        .sum::<f32>()
        / 10.0;
    assert!(
        moved_centroid_x > start_centroid_x + 2.0,
        "Live FlowFieldHandle steering must advance group smoothly"
    );

    // 2. Incremental 16x16 dirty-sector HPA* patching on building spawn/destroy
    let mut hpa_world = make_empty_world_with_size(&loaded, 96, 96);
    let path_before = hpa_world.hpa_cache.find_path(
        &hpa_world.grid,
        [6, 20],
        [84, 20],
        MovementDomain::Ground,
        0.0,
        &[],
        1,
    );
    assert!(!path_before.is_empty());
    let epoch_before = hpa_world.grid.topology_epoch;
    let _b_id = hpa_world
        .spawn_building(1, "vanguard_barracks", [44, 19], &loaded, false)
        .unwrap();
    assert!(hpa_world.grid.topology_epoch > epoch_before);
    let path_around = hpa_world.hpa_cache.find_path(
        &hpa_world.grid,
        [6, 20],
        [84, 20],
        MovementDomain::Ground,
        0.0,
        &[],
        1,
    );
    assert!(!path_around.is_empty());
    for wp in &path_around {
        let t = hpa_world.grid.world_to_tile(*wp);
        assert!(
            !hpa_world.grid.blocked_tiles.contains(&t),
            "Incremental HPA* path must route around newly placed building footprint"
        );
    }

    // 3. YieldStepAside: idle friendly unit in a 2-tile canyon corridor steps aside for moving ally
    let mut narrow = make_showcase2_world(&loaded);
    for x in 12..=24 {
        narrow.grid.blocked_tiles.insert([x, 19]);
        narrow.grid.blocked_tiles.insert([x, 22]);
    }
    narrow.grid.topology_epoch += 1;
    narrow.grid.rebuild_clearance_map();

    let mover = narrow
        .spawn_unit(1, "legion_sentinel", [13, 20], &loaded)
        .unwrap();
    let idle_ally = narrow
        .spawn_unit(1, "legion_sentinel", [18, 20], &loaded)
        .unwrap();
    let idle_start = narrow.entities[&idle_ally].position;
    let dest = narrow.grid.tile_to_world([23, 20]);
    narrow.apply_command(
        GameCommand::MoveUnits {
            player_id: 1,
            entity_ids: vec![mover],
            target_pos: dest,
        },
        &loaded,
    );
    for _ in 0..90 {
        narrow.step(&loaded);
    }
    let idle_after = narrow.entities[&idle_ally].position;
    assert!(
        det_distance_xz_f32(idle_start, idle_after) > 0.15,
        "Idle friendly unit should YieldStepAside when approached by a friendly mover"
    );
    assert!(
        det_distance_xz_f32(narrow.entities[&mover].position, dest) < 2.5,
        "Mover should pass the yielding friendly unit and reach destination"
    );
}

#[test]
fn task233_3d_ballistic_lof_swept_wall_anti_tunneling_missile_turn_rate_and_beams_cones() {
    let ra2 = load_ra2_mod();
    let showcase2 = load_showcase2_mod();

    // 1. Pre-fire 3D parabolic LOF: wall blocks flat tank cannon, while high-arc artillery clears it!
    let mut lof_world = make_empty_world_with_size(&ra2, 40, 40);
    for z in 10..=22 {
        lof_world
            .spawn_building(2, "concrete_wall", [16, z], &ra2, false)
            .expect("spawn concrete_wall");
    }
    let shooter_pos = lof_world.grid.tile_to_world([12, 16]);
    let target_pos = lof_world.grid.tile_to_world([20, 16]);

    let mut flat_cannon = showcase2.weapons["sunlance_rifle"].clone();
    flat_cannon.delivery = DeliveryKind::Ballistic;
    flat_cannon.arc_height = 0.0;
    flat_cannon.ballistics.gravity = 0.0;
    flat_cannon.ballistics.collides_with_structures = true;

    let high_arc_siege = &showcase2.weapons["meteor_siege_cannon"];

    assert!(
        !has_ballistic_line_of_fire(
            &lof_world,
            &ra2,
            0,
            1,
            shooter_pos,
            None,
            target_pos,
            &flat_cannon,
            None,
        ),
        "Flat direct-fire projectile must be occluded by intervening concrete_wall"
    );
    assert!(
        has_ballistic_line_of_fire(
            &lof_world,
            &showcase2,
            0,
            1,
            shooter_pos,
            None,
            target_pos,
            high_arc_siege,
            None,
        ),
        "High-arc ballistic siege cannon (arc_height = 6.5) must clear the wall!"
    );

    // 2. Swept 3D segment anti-tunneling: ultra-fast (160 u/s -> 8.0 units/tick > 2.0-unit wall!) flat shell hits wall
    let wall_ent_id = lof_world
        .entities
        .values()
        .find(|e| e.tile_origin == [16, 16])
        .map(|e| e.id)
        .unwrap();
    let wall_hp_before = lof_world.entities[&wall_ent_id].health;
    let protected_unit = lof_world
        .spawn_unit(2, "soviet_conscript", [20, 16], &ra2)
        .expect("spawn soviet_conscript");
    let protected_hp_before = lof_world.entities[&protected_unit].health;

    lof_world.projectiles.push(SimProjectile {
        id: 9001,
        attacker_id: 0,
        attacker_player: 1,
        target_id: protected_unit,
        weapon_id: "grizzly_cannon".to_string(),
        damage: 85.0,
        start_pos: [shooter_pos[0], 0.8, shooter_pos[2]],
        prev_pos: [shooter_pos[0], 0.8, shooter_pos[2]],
        current_pos: [shooter_pos[0], 0.8, shooter_pos[2]],
        target_pos: [target_pos[0], 0.8, target_pos[2]],
        speed: 160.0,
        delivery: DeliveryKind::Ballistic,
        arc_height: 0.0,
        total_dist: 16.0,
        velocity: [160.0, 0.0, 0.0],
        gravity: 0.0,
        collides_with_terrain: true,
        collides_with_structures: true,
        ..Default::default()
    });
    lof_world.step(&ra2);
    assert!(
        lof_world.entities[&wall_ent_id].health < wall_hp_before,
        "Swept 3D segment raycast must intercept 160 u/s shell on the 1-tile wall without tunneling!"
    );
    assert_eq!(
        lof_world.entities[&protected_unit].health, protected_hp_before,
        "Unit behind wall must take zero damage from intercepted flat shell"
    );

    // 3. Penetrating Beam (`sunbeam_spire`) and Flame Cone (`pyre_warden`) multi-target hits
    let mut beam_world = make_showcase2_world(&showcase2);
    let _beam_hq = beam_world
        .spawn_building(1, "citadel_hq", [8, 20], &showcase2, false)
        .unwrap();
    let spire = beam_world
        .spawn_building(1, "sunbeam_spire", [14, 20], &showcase2, false)
        .unwrap();
    let col_enemy_near = beam_world
        .spawn_unit(2, "legion_sentinel", [17, 20], &showcase2)
        .unwrap();
    let col_enemy_far = beam_world
        .spawn_unit(2, "legion_sentinel", [20, 20], &showcase2)
        .unwrap();
    let hp_near = beam_world.entities[&col_enemy_near].health;
    let hp_far = beam_world.entities[&col_enemy_far].health;
    beam_world.apply_command(
        GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![spire],
            target_entity_id: col_enemy_far,
        },
        &showcase2,
    );
    for _ in 0..5 {
        beam_world.step(&showcase2);
    }
    assert!(
        beam_world.entities[&col_enemy_near].health < hp_near
            && beam_world.entities[&col_enemy_far].health < hp_far,
        "Penetrating DeliveryKind::Beam (sunbeam_spire) must damage all collinear enemies along the ray"
    );

    let mut cone_world = make_showcase2_world(&showcase2);
    let pyre = cone_world
        .spawn_unit(1, "pyre_warden", [16, 20], &showcase2)
        .unwrap();
    let cone_a = cone_world
        .spawn_unit(2, "legion_sentinel", [19, 20], &showcase2)
        .unwrap();
    let cone_b = cone_world
        .spawn_unit(2, "legion_sentinel", [19, 21], &showcase2)
        .unwrap();
    let cone_hp_a = cone_world.entities[&cone_a].health;
    let cone_hp_b = cone_world.entities[&cone_b].health;
    cone_world.apply_command(
        GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![pyre],
            target_entity_id: cone_a,
        },
        &showcase2,
    );
    for _ in 0..5 {
        cone_world.step(&showcase2);
    }
    assert!(
        cone_world.entities[&cone_a].health < cone_hp_a
            && cone_world.entities[&cone_b].health < cone_hp_b,
        "DeliveryKind::Cone (pyre_warden) must damage multiple enemies inside its cone angle"
    );

    // 4. Finite missile turn_rate_deg_per_sec: perpendicular missile arcs smoothly rather than snapping 90 deg
    let mut turn_world = make_showcase2_world(&showcase2);
    let missile_target = turn_world
        .spawn_unit(2, "legion_sentinel", [28, 20], &showcase2)
        .unwrap();
    let m_target_pos = turn_world.entities[&missile_target].position;
    turn_world.projectiles.push(SimProjectile {
        id: 9002,
        attacker_id: 0,
        attacker_player: 1,
        target_id: missile_target,
        weapon_id: "aether_seeker_pod".to_string(),
        damage: 40.0,
        start_pos: [32.0, 1.5, 40.0],
        prev_pos: [32.0, 1.5, 40.0],
        current_pos: [32.0, 1.5, 40.0],
        target_pos: [m_target_pos[0], 1.5, m_target_pos[2]],
        speed: 20.0,
        delivery: DeliveryKind::Missile,
        total_dist: 24.0,
        velocity: [0.0, 0.0, 20.0],
        turn_rate_deg_per_sec: 90.0,
        ..Default::default()
    });
    turn_world.step(&showcase2);
    let m_vel = turn_world.projectiles[0].velocity;
    assert!(
        m_vel[0] > 0.5 && m_vel[2] > 18.0,
        "Finite turn_rate_deg_per_sec = 90 deg/s (4.5 deg/tick) must steer gradually, got vel={m_vel:?}"
    );
}

#[test]
fn task239_248_custom_curve_movement_area_commands_and_reclaim_human_slot() {
    let loaded = load_showcase2_mod();
    let mut world = make_showcase2_world(&loaded);

    // 1. MoveAlongCustomCurve distributes 4 units evenly by arc-length with perpendicular arrival facing
    let mut squad = Vec::new();
    for i in 0..4 {
        squad.push(
            world
                .spawn_unit(1, "legion_sentinel", [10 + i, 10], &loaded)
                .unwrap(),
        );
    }
    world.apply_command(
        GameCommand::MoveAlongCustomCurve {
            player_id: 1,
            entity_ids: squad.clone(),
            curve_points: vec![[24.0, 0.0, 20.0], [36.0, 0.0, 20.0], [36.0, 0.0, 32.0]],
            queued: false,
        },
        &loaded,
    );
    for &uid in &squad {
        assert!(
            world.entities[&uid].desired_arrival_facing.is_some(),
            "MoveAlongCustomCurve must assign perpendicular arrival facing"
        );
    }
    for _ in 0..120 {
        world.step(&loaded);
    }
    for i in 0..squad.len() {
        for j in (i + 1)..squad.len() {
            let d = det_distance_xz_f32(
                world.entities[&squad[i]].position,
                world.entities[&squad[j]].position,
            );
            assert!(
                d > 1.2,
                "Units on custom curve should settle at spaced arc-length slots (dist={d})"
            );
        }
    }

    // 2. AreaCommand (AreaAttack & AreaGuard) + ReclaimHumanSlot
    let enemy = world
        .spawn_unit(2, "legion_sentinel", [38, 16], &loaded)
        .unwrap();
    let enemy_pos = world.entities[&enemy].position;
    world.apply_command(
        GameCommand::AreaCommand {
            player_id: 1,
            entity_ids: vec![squad[0], squad[1]],
            kind: AreaCommandKind::AreaAttack,
            center: enemy_pos,
            radius: 12.0,
            queued: false,
        },
        &loaded,
    );
    assert!(
        world.entities[&squad[0]].target_entity_id == Some(enemy)
            || world.entities[&squad[0]].attack_move_dest.is_some(),
        "AreaAttack should engage hostile inside radius"
    );

    let hq = world
        .spawn_building(1, "citadel_hq", [14, 28], &loaded, false)
        .unwrap();
    let hq_pos = world.entities[&hq].position;
    world.apply_command(
        GameCommand::AreaCommand {
            player_id: 1,
            entity_ids: vec![squad[2]],
            kind: AreaCommandKind::AreaGuard,
            center: hq_pos,
            radius: 10.0,
            queued: false,
        },
        &loaded,
    );
    assert_eq!(world.entities[&squad[2]].guard_origin, Some(hq_pos));
    assert!(
        !world.entities[&squad[2]].patrol_waypoints.is_empty(),
        "AreaGuard must assign a perimeter patrol ring anchored at guard_origin"
    );

    // Enable AI takeover on slot 1, then ReclaimHumanSlot (`Task 248-Sim`)
    world.apply_command(
        GameCommand::EnableAiTakeover {
            player_id: 1,
            difficulty: AiDifficulty::Hard,
        },
        &loaded,
    );
    assert!(world.ai_takeover_players.contains_key(&1));
    world.apply_command(GameCommand::ReclaimHumanSlot { player_id: 1 }, &loaded);
    assert!(
        !world.ai_takeover_players.contains_key(&1),
        "ReclaimHumanSlot must return AI-controlled slot back to human control"
    );
}

#[test]
fn task242_trigger_graph_dag_delays_logic_gates_wire_trace_and_cycle_guard() {
    let loaded = load_showcase2_mod();
    let mut world = make_showcase2_world(&loaded);

    let dag = TriggerGraphDef {
        id: "test_dag".to_string(),
        name: "MatchStart Grant Credits After Delay".to_string(),
        enabled: true,
        nodes: vec![
            TriggerGraphNode {
                id: "ev_start".to_string(),
                title: "On Match Start".to_string(),
                comment: String::new(),
                group_id: None,
                canvas_pos: [0.0, 0.0],
                node_kind: TriggerGraphNodeKind::EventStart {
                    event: TriggerEventKind::MatchStart,
                    once: true,
                },
            },
            TriggerGraphNode {
                id: "gate_or".to_string(),
                title: "OR Gate".to_string(),
                comment: String::new(),
                group_id: None,
                canvas_pos: [180.0, 0.0],
                node_kind: TriggerGraphNodeKind::LogicGate {
                    gate: LogicGateOp::Or,
                },
            },
            TriggerGraphNode {
                id: "delay_node".to_string(),
                title: "0.10s Delay".to_string(),
                comment: String::new(),
                group_id: None,
                canvas_pos: [360.0, 0.0],
                node_kind: TriggerGraphNodeKind::Delay {
                    delay_secs: 0.10,
                    repeat_count: 0,
                    repeat_interval_secs: 0.0,
                },
            },
            TriggerGraphNode {
                id: "if_node".to_string(),
                title: "Check Elapsed".to_string(),
                comment: String::new(),
                group_id: None,
                canvas_pos: [540.0, 0.0],
                node_kind: TriggerGraphNodeKind::IfElse {
                    condition: TriggerCondition::TimerElapsed { secs: 0.05 },
                },
            },
            TriggerGraphNode {
                id: "grant_credits".to_string(),
                title: "Grant +777 Credits".to_string(),
                comment: String::new(),
                group_id: None,
                canvas_pos: [720.0, 0.0],
                node_kind: TriggerGraphNodeKind::Action {
                    action: TriggerAction::GrantCredits {
                        player_id: 1,
                        amount: 777,
                    },
                },
            },
        ],
        wires: vec![
            TriggerGraphWire {
                id: "w1".to_string(),
                from_node: "ev_start".to_string(),
                from_port: "out".to_string(),
                to_node: "gate_or".to_string(),
                to_port: "in".to_string(),
            },
            TriggerGraphWire {
                id: "w2".to_string(),
                from_node: "gate_or".to_string(),
                from_port: "out".to_string(),
                to_node: "delay_node".to_string(),
                to_port: "in".to_string(),
            },
            TriggerGraphWire {
                id: "w3".to_string(),
                from_node: "delay_node".to_string(),
                from_port: "out".to_string(),
                to_node: "if_node".to_string(),
                to_port: "in".to_string(),
            },
            TriggerGraphWire {
                id: "w4".to_string(),
                from_node: "if_node".to_string(),
                from_port: "true".to_string(),
                to_node: "grant_credits".to_string(),
                to_port: "in".to_string(),
            },
        ],
        groups: Vec::new(),
    };

    let cyclic = TriggerGraphDef {
        id: "bad_cycle".to_string(),
        name: "Cyclic Graph".to_string(),
        enabled: true,
        nodes: vec![
            TriggerGraphNode {
                id: "a".to_string(),
                title: String::new(),
                comment: String::new(),
                group_id: None,
                canvas_pos: [0.0, 0.0],
                node_kind: TriggerGraphNodeKind::LogicGate {
                    gate: LogicGateOp::Or,
                },
            },
            TriggerGraphNode {
                id: "b".to_string(),
                title: String::new(),
                comment: String::new(),
                group_id: None,
                canvas_pos: [100.0, 0.0],
                node_kind: TriggerGraphNodeKind::LogicGate {
                    gate: LogicGateOp::Or,
                },
            },
        ],
        wires: vec![
            TriggerGraphWire {
                id: "ca".to_string(),
                from_node: "a".to_string(),
                from_port: "out".to_string(),
                to_node: "b".to_string(),
                to_port: "in".to_string(),
            },
            TriggerGraphWire {
                id: "cb".to_string(),
                from_node: "b".to_string(),
                from_port: "out".to_string(),
                to_node: "a".to_string(),
                to_port: "in".to_string(),
            },
        ],
        groups: Vec::new(),
    };
    assert!(cyclic.topological_node_order().is_err());

    let start_credits = world.players[&1].credits;
    world.trigger_graphs = vec![dag, cyclic];

    // Tick 0 fires MatchStart -> w1 -> w2 -> queues 2-tick Delay
    world.step(&loaded);
    assert!(
        world
            .last_fired_wires
            .iter()
            .any(|(gid, wid, _)| gid == "test_dag" && wid == "w1"),
        "last_fired_wires must record w1 pulse on tick 0"
    );
    assert_eq!(
        world.players[&1].credits, start_credits,
        "GrantCredits must wait for the 0.10s Delay node before firing"
    );

    for _ in 0..4 {
        world.step(&loaded);
    }
    assert_eq!(
        world.players[&1].credits,
        start_credits + 777,
        "After 0.10s Delay matures, IfElse('true') -> GrantCredits(+777) must fire"
    );
    assert!(world
        .last_fired_wires
        .iter()
        .any(|(gid, wid, _)| gid == "test_dag" && wid == "w4"));
}

#[test]
fn task243_251_behavior_tree_ai_mutator_overlay_and_cross_profile_showcase2_hash() {
    // 1. BehaviorTreeDef AI evaluation populates `blackboard` and `active_node_path`
    let showcase2 = load_showcase2_mod();
    let mut ai_world = make_showcase2_world(&showcase2);
    let _p1_hq = ai_world
        .spawn_building(1, "citadel_hq", [10, 10], &showcase2, false)
        .unwrap();
    let _p2_hq = ai_world
        .spawn_building(2, "citadel_hq", [48, 48], &showcase2, false)
        .unwrap();
    let _p2_w = ai_world
        .spawn_unit(2, "acolyte_worker", [46, 48], &showcase2)
        .unwrap();
    ai_world
        .ai_commanders
        .insert(2, AiCommanderState::new(2, 64, 64));
    ai_world.tick = 10;
    step_ai_player_in_world(
        &mut ai_world,
        2,
        &showcase2,
        AiDifficulty::Hard,
        Some("archon_ascendancy"),
    );
    let cmdr = &ai_world.ai_commanders[&2];
    assert!(
        !cmdr.blackboard.is_empty(),
        "BehaviorTree AI step must populate commander blackboard"
    );
    assert!(
        !cmdr.active_node_path.is_empty(),
        "BehaviorTree AI step must record active_node_path for Workbench live inspector"
    );

    // 2. Stacked overlay `mutator-turbo-economy` on top of `sdk-showcase-heroes-ages-mutators` (`Task 251`)
    let stacked = load_showcase2_with_turbo_mutator();
    assert!(
        stacked.dependency_chain.len() >= 2,
        "Stacked load must include base showcase2 + mutator-turbo-economy"
    );
    assert!(
        (stacked.manifest.rules.gamemode.ticket_bleed_per_sec - 8.0).abs() < 1e-4,
        "mutator-turbo-economy must override rules.gamemode.ticket_bleed_per_sec to 8.0"
    );
    let worker_def = stacked.units.get("acolyte_worker").unwrap();
    assert_eq!(
        worker_def.streaming_build_power,
        Some(90.0),
        "mutator-turbo-economy must boost acolyte_worker streaming_build_power to 90.0"
    );
    assert!(
        worker_def.tags.iter().any(|t| t == "turbo_boosted"),
        "mutator-turbo-economy $append directive must append 'turbo_boosted' tag"
    );

    // 3. Deterministic 200-tick Showcase Mod #2 match verifying identical debug == --release state_hash()
    let mut det_world = make_showcase2_world(&showcase2);
    let _hq1 = det_world
        .spawn_building(1, "citadel_hq", [10, 14], &showcase2, false)
        .unwrap();
    let b1 = det_world
        .spawn_building(1, "vanguard_barracks", [16, 14], &showcase2, false)
        .unwrap();
    let hero1 = det_world
        .spawn_unit(1, "archon_commander", [20, 20], &showcase2)
        .unwrap();
    let _s1 = det_world
        .spawn_unit(1, "legion_sentinel", [22, 20], &showcase2)
        .unwrap();

    let _hq2 = det_world
        .spawn_building(2, "citadel_hq", [50, 50], &showcase2, false)
        .unwrap();
    let _s2 = det_world
        .spawn_unit(2, "legion_sentinel", [28, 20], &showcase2)
        .unwrap();

    det_world.apply_command(
        GameCommand::LearnHeroSkill {
            player_id: 1,
            hero_id: hero1,
            ability_id: "solar_nova".to_string(),
        },
        &showcase2,
    );
    det_world.apply_command(
        GameCommand::ProduceUnit {
            player_id: 1,
            producer_entity_id: Some(b1),
            unit_id: "legion_sentinel".to_string(),
        },
        &showcase2,
    );
    det_world.apply_command(
        GameCommand::AttackMove {
            player_id: 1,
            entity_ids: vec![hero1],
            target_pos: det_world.grid.tile_to_world([28, 20]),
        },
        &showcase2,
    );

    let mut det_world_b = det_world.clone();
    for _ in 0..200 {
        det_world.step(&showcase2);
        det_world_b.step(&showcase2);
    }
    let h_a = det_world.state_hash();
    let h_b = det_world_b.state_hash();
    assert_eq!(
        h_a, h_b,
        "Two Showcase Mod #2 runs must produce identical state_hash"
    );
    assert_eq!(
        h_a, 0xaa1f_600b_0385_e6f7_u64,
        "Cross-profile (debug == --release) Showcase Mod #2 200-tick state_hash drifted: 0x{h_a:016x}"
    );

    // Roundtrip RTSB01 snapshot on live Showcase #2 world (including HeroInstanceState, EnergyPoolState, Streaming fractional ledger)
    let encoded = det_world.encode_fast_binary();
    let decoded = SimWorld::decode_fast_binary(&encoded).expect("RTSB01 decode Showcase #2");
    assert_eq!(
        decoded.state_hash(),
        h_a,
        "RTSB01 binary snapshot must preserve exact state_hash on Showcase Mod #2"
    );
    println!("[Showcase2 Determinism] 200-tick hash = 0x{h_a:016x}");
}
