use rts_sdk::schema::{VisualArchetype, VisualDef};
use serde_json::json;

use super::buildings::has_token;
use super::gltf_writer::{append_batches_to_gltf_buffers, build_infantry_gltf_animations};
use super::infantry::build_articulated_infantry_rig;
use super::new_six_batches;
use super::primitives::GeometryBatch;

pub(super) type ArticulatedBatches = (
    [GeometryBatch; 6],
    [GeometryBatch; 6],
    [GeometryBatch; 6],
    [GeometryBatch; 6],
);

pub(super) fn split_hull_turret_barrel_and_rotor_batches(
    name: &str,
    archetype: VisualArchetype,
    scale: [f32; 3],
    batches: &[GeometryBatch; 6],
) -> ArticulatedBatches {
    let mut hull = new_six_batches();
    let mut turret = new_six_batches();
    let mut barrel = new_six_batches();
    let mut rotor = new_six_batches();

    let lower = name.to_ascii_lowercase();

    // Only rotary-wing aircraft (helicopters/tiltrotors) and spinning radar/sensor domes get a Rotor node;
    // fixed-wing jets, bombers, transport planes, and Kirov airships must NOT slice their upper hull/fins!
    let is_fixed_wing_or_blimp = lower.contains("harrier")
        || lower.contains("black_eagle")
        || lower.contains("black eagle")
        || lower.contains("beag")
        || lower.contains("orca")
        || lower.contains("hornet")
        || lower.contains("paradrop")
        || lower.contains("spy_plane")
        || lower.contains("spy plane")
        || lower.contains("kirov")
        || lower.contains("zep");
    let has_rotor = !is_fixed_wing_or_blimp
        && (lower.contains("chopper")
            || lower.contains("heli")
            || lower.contains("osprey")
            || lower.contains("nighthawk")
            || lower.contains("night hawk")
            || lower.contains("shad")
            || lower.contains("hind")
            || lower.contains("radar")
            || lower.contains("sensor")
            || lower.contains("spysat")
            || lower.contains("spy satellite")
            || lower.contains("airforce"));

    let is_casemate_or_nonturreted = lower.contains("tank_destroyer")
        || lower.contains("tank destroyer")
        || lower.contains("tnkd")
        || lower.contains("demolition")
        || lower.contains("dtruck")
        || lower.contains("mcv")
        || lower.contains("amcv")
        || lower.contains("smcv")
        || lower.contains("chrono_miner")
        || lower.contains("chrono miner")
        || lower.contains("cmin")
        || lower.contains("hover")
        || lower.contains("amphibious")
        || lower.contains("landing craft")
        || lower.contains("lcrf")
        || lower.contains("sapc")
        || lower.contains("lcv")
        || lower.contains("zubr")
        || lower.contains("squid")
        || lower.contains("sqd")
        || lower.contains("dolphin")
        || lower.contains("dlph")
        || lower.contains("sub")
        || lower.contains("typhoon")
        || lower.contains("carrier")
        || lower.contains("dron")
        || lower.contains("drone")
        || lower.contains("v3");

    let turret_split_y = if is_casemate_or_nonturreted || has_rotor {
        None
    } else {
        match archetype {
            VisualArchetype::Tank => Some(0.43 * scale[1]),
            VisualArchetype::Turret => {
                if lower.contains("prism")
                    || lower.contains("atesla")
                    || lower.contains("tesla")
                    || lower.contains("tsla")
                {
                    None
                } else {
                    Some(0.32 * scale[1])
                }
            }
            VisualArchetype::Ship => Some(0.48 * scale[1]),
            VisualArchetype::Harvester => {
                if lower.contains("war_miner")
                    || lower.contains("war miner")
                    || lower.contains("harv")
                    || lower.contains("slave")
                {
                    Some(0.66 * scale[1])
                } else {
                    None
                }
            }
            _ => None,
        }
    };

    let has_recoil_barrel = turret_split_y.is_some()
        && !lower.contains("ifv")
        && !lower.contains("fighting vehicle")
        && !lower.contains("fv")
        && !lower.contains("aegis")
        && !lower.contains("patriot")
        && !lower.contains("nasam")
        && !lower.contains("tesla")
        && !lower.contains("ttnk")
        && !lower.contains("prism")
        && !lower.contains("sref")
        && !lower.contains("magnetron");

    let rotor_split_y = if has_rotor {
        if archetype == VisualArchetype::Aircraft
            || lower.contains("chopper")
            || lower.contains("heli")
            || lower.contains("nighthawk")
            || lower.contains("night hawk")
            || lower.contains("osprey")
            || lower.contains("shad")
            || lower.contains("hind")
        {
            Some(0.62 * scale[1])
        } else {
            Some(1.18 * scale[1])
        }
    } else {
        None
    };

    for (mat_idx, src) in batches.iter().enumerate() {
        if turret_split_y.is_none() && rotor_split_y.is_none() {
            hull[mat_idx].positions = src.positions.clone();
            hull[mat_idx].normals = src.normals.clone();
            hull[mat_idx].colors = src.colors.clone();
            hull[mat_idx].indices = src.indices.clone();
            continue;
        }
        for tri in src.indices.chunks_exact(3) {
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;
            let cx = (src.positions[i0][0] + src.positions[i1][0] + src.positions[i2][0]) / 3.0;
            let cy = (src.positions[i0][1] + src.positions[i1][1] + src.positions[i2][1]) / 3.0;
            let cz = (src.positions[i0][2] + src.positions[i1][2] + src.positions[i2][2]) / 3.0;
            let dst = if let Some(ry) = rotor_split_y {
                if cy >= ry {
                    &mut rotor[mat_idx]
                } else {
                    &mut hull[mat_idx]
                }
            } else if let Some(ty) = turret_split_y {
                if cy >= ty {
                    if has_recoil_barrel && cz >= 0.24 * scale[2] && cx.abs() <= 0.24 * scale[0] {
                        &mut barrel[mat_idx]
                    } else {
                        &mut turret[mat_idx]
                    }
                } else {
                    &mut hull[mat_idx]
                }
            } else {
                &mut hull[mat_idx]
            };
            let base = dst.positions.len() as u32;
            for &vi in &[i0, i1, i2] {
                dst.positions.push(src.positions[vi]);
                dst.normals.push(src.normals[vi]);
                dst.colors
                    .push(src.colors.get(vi).copied().unwrap_or([1.0, 1.0, 1.0]));
            }
            dst.indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
    }

    // Ensure hull is never empty if all geometry happened to sit above split_y
    if hull.iter().all(|b| b.positions.is_empty()) {
        for i in 0..6 {
            if !turret[i].positions.is_empty() {
                std::mem::swap(&mut hull[i], &mut turret[i]);
            } else if !rotor[i].positions.is_empty() {
                std::mem::swap(&mut hull[i], &mut rotor[i]);
            }
        }
    }

    (hull, turret, barrel, rotor)
}

pub(super) fn is_articulated_infantry_model(raw_name: &str, archetype: VisualArchetype) -> bool {
    if archetype != VisualArchetype::Infantry {
        return false;
    }
    let lower = raw_name.to_ascii_lowercase();
    !(has_token(&lower, "dron")
        || has_token(&lower, "drone")
        || lower.contains("terror drone")
        || lower.contains("terror_drone"))
}

pub(super) fn is_building_archetype(archetype: VisualArchetype) -> bool {
    !matches!(
        archetype,
        VisualArchetype::Infantry
            | VisualArchetype::Tank
            | VisualArchetype::Harvester
            | VisualArchetype::Aircraft
            | VisualArchetype::Ship
    )
}

pub(super) fn build_infantry_gltf_scene(
    name: &str,
    visual: &VisualDef,
    bin_chunk: &mut Vec<u8>,
    buffer_views: &mut Vec<serde_json::Value>,
    accessors: &mut Vec<serde_json::Value>,
) -> (
    serde_json::Value,
    serde_json::Value,
    Option<Vec<serde_json::Value>>,
) {
    // Articulated Infantry Rig: Hull -> [Torso -> [Head, WeaponArm -> Muzzle_*], LeftLeg, RightLeg, DamageSmoke_01]
    let rig = build_articulated_infantry_rig(name, visual.scale);
    let mut meshes_vec = Vec::with_capacity(6);

    let mut push_part_mesh =
        |part_label: &str, part_batches: &[GeometryBatch; 6]| -> Option<usize> {
            let prims =
                append_batches_to_gltf_buffers(part_batches, bin_chunk, buffer_views, accessors);
            if prims.is_empty() {
                None
            } else {
                let idx = meshes_vec.len();
                meshes_vec.push(json!({
                    "name": format!("{name}_{part_label}_mesh"),
                    "primitives": prims
                }));
                Some(idx)
            }
        };

    let hull_mesh_opt = push_part_mesh("hull", &rig.hull);
    let torso_mesh_opt = push_part_mesh("torso", &rig.torso);
    let head_mesh_opt = push_part_mesh("head", &rig.head);
    let left_leg_mesh_opt = push_part_mesh("left_leg", &rig.left_leg);
    let right_leg_mesh_opt = push_part_mesh("right_leg", &rig.right_leg);
    let weapon_arm_mesh_opt = push_part_mesh("weapon_arm", &rig.weapon_arm);

    // Node indices: 0=Hull, 1=Torso, 2=Head, 3=LeftLeg, 4=RightLeg, 5=WeaponArm, 6+=Sockets
    let mut nodes_vec = vec![
        serde_json::Value::Null,
        serde_json::Value::Null,
        serde_json::Value::Null,
        serde_json::Value::Null,
        serde_json::Value::Null,
        serde_json::Value::Null,
    ];

    let arm_origin = [
        rig.torso_translation[0] + rig.weapon_arm_translation[0],
        rig.torso_translation[1] + rig.weapon_arm_translation[1],
        rig.torso_translation[2] + rig.weapon_arm_translation[2],
    ];
    let m1_local = visual
        .find_socket("Muzzle_01")
        .map(|w| {
            [
                w[0] - arm_origin[0],
                w[1] - arm_origin[1],
                w[2] - arm_origin[2],
            ]
        })
        .unwrap_or(rig.muzzle_01_local);
    let m2_local_opt = visual
        .find_socket("Muzzle_02")
        .map(|w| {
            [
                w[0] - arm_origin[0],
                w[1] - arm_origin[1],
                w[2] - arm_origin[2],
            ]
        })
        .or(rig.muzzle_02_local);
    let exhaust_local_opt = visual
        .find_socket("Exhaust_01")
        .map(|w| {
            [
                w[0] - rig.torso_translation[0],
                w[1] - rig.torso_translation[1],
                w[2] - rig.torso_translation[2],
            ]
        })
        .or(rig.exhaust_01_local);
    let smoke_pos = visual
        .find_socket("DamageSmoke_01")
        .unwrap_or(rig.damage_smoke_01);

    let muzzle_01_idx = nodes_vec.len();
    nodes_vec.push(json!({
        "name": "Muzzle_01",
        "translation": m1_local
    }));
    let mut arm_children = vec![muzzle_01_idx];
    if let Some(m2) = m2_local_opt {
        let m2_idx = nodes_vec.len();
        nodes_vec.push(json!({
            "name": "Muzzle_02",
            "translation": m2
        }));
        arm_children.push(m2_idx);
    }

    let mut torso_children = vec![2usize, 5usize];
    if let Some(ex) = exhaust_local_opt {
        let ex_idx = nodes_vec.len();
        nodes_vec.push(json!({
            "name": "Exhaust_01",
            "translation": ex
        }));
        torso_children.push(ex_idx);
    }

    let smoke_idx = nodes_vec.len();
    nodes_vec.push(json!({
        "name": "DamageSmoke_01",
        "translation": smoke_pos
    }));
    let hull_children = vec![1usize, 3usize, 4usize, smoke_idx];

    let make_limb_node = |node_name: &str,
                          mesh_opt: Option<usize>,
                          tr_opt: Option<[f32; 3]>,
                          children: &[usize]|
     -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        obj.insert("name".to_string(), json!(node_name));
        if let Some(m_idx) = mesh_opt {
            obj.insert("mesh".to_string(), json!(m_idx));
        }
        if let Some(tr) = tr_opt {
            obj.insert("translation".to_string(), json!(tr));
        }
        if !children.is_empty() {
            obj.insert("children".to_string(), json!(children));
        }
        serde_json::Value::Object(obj)
    };

    nodes_vec[0] = make_limb_node("Hull", hull_mesh_opt, None, &hull_children);
    nodes_vec[1] = make_limb_node(
        "Torso",
        torso_mesh_opt,
        Some(rig.torso_translation),
        &torso_children,
    );
    nodes_vec[2] = make_limb_node("Head", head_mesh_opt, Some(rig.head_translation), &[]);
    nodes_vec[3] = make_limb_node(
        "LeftLeg",
        left_leg_mesh_opt,
        Some(rig.left_leg_translation),
        &[],
    );
    nodes_vec[4] = make_limb_node(
        "RightLeg",
        right_leg_mesh_opt,
        Some(rig.right_leg_translation),
        &[],
    );
    nodes_vec[5] = make_limb_node(
        "WeaponArm",
        weapon_arm_mesh_opt,
        Some(rig.weapon_arm_translation),
        &arm_children,
    );

    let animations =
        build_infantry_gltf_animations(&rig, 1, 2, 3, 4, 5, bin_chunk, buffer_views, accessors);

    (
        serde_json::Value::Array(nodes_vec),
        serde_json::Value::Array(meshes_vec),
        Some(animations),
    )
}
