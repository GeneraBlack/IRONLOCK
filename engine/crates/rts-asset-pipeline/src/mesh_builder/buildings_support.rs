use super::buildings::has_token;
use super::components::*;
use super::new_six_batches;
use super::primitives::{GeometryBatch, MatSlot};
use rts_sdk::schema::VisualArchetype;

/// Step 1B: Populates geometry for radar, tech centers, naval yards, superweapons, and civilian structures.
pub(super) fn try_populate_tech_and_support_buildings(
    name: &str,
    scale: [f32; 3],
    batches: &mut [GeometryBatch; 6],
) -> bool {
    let primary = MatSlot::Primary as usize;
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;
    let concrete = MatSlot::Concrete as usize;
    let dark = MatSlot::DarkAccent as usize;

    if has_token(name, "gaairc") || has_token(name, "amradr") || name.contains("airforce") {
        // Allied Airforce Command HQ: 4 VTOL landing pads + control tower + rotating radar dish
        batches[concrete].add_box([0.0, 0.10, 0.0], [0.96, 0.10, 0.96], scale);
        batches[primary].add_tapered_box(
            [0.0, 0.34, -0.14],
            [0.82, 0.66],
            [0.74, 0.58],
            0.18,
            scale,
        );
        add_corrugated_roof_and_cladding(
            batches,
            [0.0, 0.34, -0.14],
            [0.74, 0.18, 0.58],
            12,
            scale,
        );
        for (px, pz) in [(-0.46, 0.42), (0.46, 0.42), (-0.46, -0.12), (0.46, -0.12)] {
            batches[steel].add_cylinder_y([px, 0.54, pz], 0.28, 0.30, 0.05, 24, scale);
            batches[gunmetal].add_cylinder_y([px, 0.57, pz], 0.24, 0.24, 0.025, 24, scale);
            batches[emissive].add_cylinder_y([px, 0.60, pz], 0.16, 0.16, 0.015, 20, scale);
        }
        batches[primary].add_tapered_box(
            [0.0, 0.92, -0.52],
            [0.28, 0.24],
            [0.22, 0.18],
            0.44,
            scale,
        );
        batches[emissive].add_box([0.0, 1.18, -0.32], [0.22, 0.09, 0.04], scale);
        batches[steel].add_pitched_cylinder_z(
            [0.0, 1.48, -0.52],
            0.34,
            0.08,
            0.14,
            0.55,
            28,
            scale,
        );
        return true;
    }

    if has_token(name, "naradr")
        || has_token(name, "napsis")
        || name.contains("radar")
        || name.contains("psychic sensor")
    {
        // Soviet Radar Tower / Psychic Sensor: armored command bunker + giant parabolic dish
        batches[concrete].add_box([0.0, 0.10, 0.0], [0.90, 0.10, 0.90], scale);
        batches[primary].add_tapered_box([0.0, 0.46, 0.0], [0.76, 0.76], [0.62, 0.62], 0.28, scale);
        add_corrugated_roof_and_cladding(batches, [0.0, 0.46, 0.0], [0.62, 0.28, 0.62], 12, scale);
        batches[gunmetal].add_cylinder_y([0.0, 0.92, 0.0], 0.28, 0.20, 0.28, 24, scale);
        batches[steel].add_pitched_cylinder_z(
            [0.0, 1.32, -0.10],
            0.56,
            0.14,
            0.22,
            0.45,
            32,
            scale,
        );
        batches[emissive].add_pitched_cylinder_z(
            [0.0, 1.36, 0.05],
            0.08,
            0.03,
            0.38,
            0.45,
            20,
            scale,
        );
        return true;
    }

    if has_token(name, "gaspysat") || name.contains("spy satellite") {
        // Spy Satellite Uplink: telemetry center + giant skyward deep-space parabolic dish
        batches[concrete].add_box([0.0, 0.10, 0.0], [0.88, 0.10, 0.88], scale);
        batches[primary].add_tapered_box(
            [-0.20, 0.42, 0.18],
            [0.58, 0.54],
            [0.50, 0.46],
            0.24,
            scale,
        );
        add_corrugated_roof_and_cladding(
            batches,
            [-0.20, 0.42, 0.18],
            [0.50, 0.24, 0.46],
            10,
            scale,
        );
        batches[steel].add_cylinder_y([0.24, 0.68, -0.20], 0.22, 0.16, 0.34, 24, scale);
        let dish_profile = [[0.14, 0.0], [0.28, 0.11], [0.44, 0.22], [0.56, 0.34]];
        batches[steel].add_spline_lathe_y([0.24, 0.96, -0.20], &dish_profile, 6, 32, scale);
        batches[emissive].add_cylinder_y([0.24, 1.38, -0.20], 0.06, 0.02, 0.24, 20, scale);
        return true;
    }

    if has_token(name, "gagap") || name.contains("gap generator") || name.contains("gap_generator")
    {
        // Allied Gap Generator: stealth-field obelisk with 4 concentric phase rings
        batches[concrete].add_cylinder_y([0.0, 0.14, 0.0], 0.72, 0.64, 0.14, 28, scale);
        let spire_profile = [
            [0.28, 0.0],
            [0.22, 0.36],
            [0.16, 0.74],
            [0.12, 1.12],
            [0.09, 1.48],
        ];
        batches[primary].add_spline_lathe_y([0.0, 0.24, 0.0], &spire_profile, 6, 28, scale);
        for ry in [0.68, 0.98, 1.28, 1.54] {
            batches[steel].add_cylinder_y([0.0, ry, 0.0], 0.36, 0.36, 0.035, 32, scale);
            batches[emissive].add_cylinder_y([0.0, ry, 0.0], 0.38, 0.38, 0.015, 32, scale);
        }
        batches[emissive].add_dome_y([0.0, 1.72, 0.0], 0.18, 0.22, 10, 28, scale);
        return true;
    }

    if has_token(name, "gatech")
        || has_token(name, "natech")
        || name.contains("tech center")
        || name.contains("battle lab")
    {
        // High-Tech Battle Lab: towering research spire + particle accelerator ring + radar dome + energy core
        batches[concrete].add_box([0.0, 0.10, 0.0], [0.95, 0.10, 0.95], scale);
        batches[primary].add_tapered_box([0.0, 0.46, 0.0], [0.84, 0.84], [0.72, 0.72], 0.28, scale);
        add_corrugated_roof_and_cladding(batches, [0.0, 0.46, 0.0], [0.72, 0.28, 0.72], 12, scale);
        batches[primary].add_tapered_box(
            [-0.24, 1.12, -0.18],
            [0.42, 0.42],
            [0.28, 0.28],
            0.52,
            scale,
        );
        batches[emissive].add_box([-0.24, 1.26, 0.16], [0.22, 0.28, 0.04], scale);
        batches[steel].add_dome_y([-0.24, 1.64, -0.18], 0.26, 0.28, 10, 28, scale);
        batches[steel].add_cylinder_y([-0.24, 2.04, -0.18], 0.04, 0.01, 0.24, 16, scale);
        batches[steel].add_cylinder_y([0.38, 0.86, 0.32], 0.34, 0.28, 0.24, 28, scale);
        batches[emissive].add_dome_y([0.38, 1.10, 0.32], 0.26, 0.24, 10, 28, scale);
        return true;
    }

    if has_token(name, "gayard") || has_token(name, "nayard") || name.contains("naval yard") {
        // Naval Shipyard: U-shaped drydock piers + overhead shipbuilding gantry crane + harbor tower
        batches[concrete].add_box([-0.68, 0.22, 0.0], [0.24, 0.22, 0.92], scale);
        batches[concrete].add_box([0.68, 0.22, 0.0], [0.24, 0.22, 0.92], scale);
        batches[concrete].add_box([0.0, 0.22, -0.68], [0.52, 0.22, 0.24], scale);
        batches[primary].add_tapered_box(
            [0.0, 0.68, -0.68],
            [0.48, 0.22],
            [0.40, 0.18],
            0.28,
            scale,
        );
        add_corrugated_roof_and_cladding(batches, [0.0, 0.68, -0.68], [0.40, 0.28, 0.18], 8, scale);
        batches[steel].add_box([-0.68, 0.86, 0.18], [0.10, 0.46, 0.10], scale);
        batches[steel].add_box([0.68, 0.86, 0.18], [0.10, 0.46, 0.10], scale);
        batches[steel].add_box([0.0, 1.34, 0.18], [0.82, 0.08, 0.12], scale);
        batches[emissive].add_cylinder_y([-0.68, 1.12, 0.72], 0.08, 0.06, 0.12, 20, scale);
        batches[emissive].add_cylinder_y([0.68, 1.12, 0.72], 0.08, 0.06, 0.12, 20, scale);
        return true;
    }

    if has_token(name, "gadept") || has_token(name, "nadept") || name.contains("depot") {
        // Service Depot: drive-on repair platform + hydraulic vehicle lift + twin robotic welding arms
        batches[concrete].add_tapered_box(
            [0.0, 0.08, 0.0],
            [0.92, 0.92],
            [0.86, 0.86],
            0.08,
            scale,
        );
        batches[steel].add_cylinder_y([0.0, 0.18, 0.0], 0.52, 0.52, 0.04, 32, scale);
        batches[primary].add_tapered_box(
            [-0.68, 0.52, 0.0],
            [0.18, 0.58],
            [0.14, 0.48],
            0.36,
            scale,
        );
        batches[primary].add_tapered_box(
            [0.68, 0.52, 0.0],
            [0.18, 0.58],
            [0.14, 0.48],
            0.36,
            scale,
        );
        add_corrugated_roof_and_cladding(batches, [-0.68, 0.52, 0.0], [0.14, 0.36, 0.48], 8, scale);
        add_corrugated_roof_and_cladding(batches, [0.68, 0.52, 0.0], [0.14, 0.36, 0.48], 8, scale);
        batches[gunmetal].add_box([-0.42, 0.94, -0.18], [0.28, 0.07, 0.08], scale);
        batches[gunmetal].add_box([0.42, 0.94, 0.18], [0.28, 0.07, 0.08], scale);
        batches[emissive].add_cylinder_y([-0.20, 0.82, -0.18], 0.06, 0.02, 0.10, 16, scale);
        batches[emissive].add_cylinder_y([0.20, 0.82, 0.18], 0.06, 0.02, 0.10, 16, scale);
        return true;
    }

    if has_token(name, "namisl") || name.contains("nuclear missile") {
        // Soviet Nuclear Missile Silo: armored launch pad + open blast doors + towering ICBM warhead
        batches[concrete].add_tapered_box(
            [0.0, 0.16, 0.0],
            [0.94, 0.94],
            [0.86, 0.86],
            0.16,
            scale,
        );
        batches[dark].add_cylinder_y([0.0, 0.34, 0.0], 0.54, 0.54, 0.06, 32, scale);
        batches[primary].add_box([-0.62, 0.56, 0.0], [0.08, 0.28, 0.48], scale);
        batches[primary].add_box([0.62, 0.56, 0.0], [0.08, 0.28, 0.48], scale);
        let rocket_body = [
            [0.25, 0.0],
            [0.24, 0.32],
            [0.24, 0.64],
            [0.23, 0.96],
            [0.22, 1.28],
        ];
        batches[steel].add_spline_lathe_y([0.0, 0.36, 0.0], &rocket_body, 6, 32, scale);
        let warhead_cone = [
            [0.22, 0.0],
            [0.19, 0.14],
            [0.14, 0.28],
            [0.07, 0.42],
            [0.015, 0.54],
        ];
        batches[primary].add_spline_lathe_y([0.0, 1.64, 0.0], &warhead_cone, 6, 32, scale);
        batches[emissive].add_cylinder_y([0.0, 1.62, 0.0], 0.235, 0.235, 0.05, 32, scale);
        return true;
    }

    if has_token(name, "gaweat") || name.contains("weather") {
        // Allied Weather Control Device: central lightning spire + 3 angled atmospheric dish emitters
        batches[concrete].add_box([0.0, 0.12, 0.0], [0.94, 0.12, 0.94], scale);
        batches[primary].add_cylinder_y([0.0, 0.46, 0.0], 0.72, 0.58, 0.24, 32, scale);
        let rod_profile = [
            [0.22, 0.0],
            [0.16, 0.29],
            [0.11, 0.58],
            [0.07, 0.87],
            [0.05, 1.16],
        ];
        batches[steel].add_spline_lathe_y([0.0, 0.64, 0.0], &rod_profile, 6, 28, scale);
        batches[emissive].add_dome_y([0.0, 1.80, 0.0], 0.18, 0.22, 10, 28, scale);
        for (dx, dz) in [(-0.52, -0.42), (0.52, -0.42), (0.0, 0.56)] {
            batches[steel].add_cylinder_y([dx, 0.82, dz], 0.22, 0.34, 0.16, 24, scale);
            batches[emissive].add_cylinder_y([dx, 1.00, dz], 0.14, 0.06, 0.08, 20, scale);
        }
        return true;
    }

    if has_token(name, "nairon") || name.contains("iron curtain") {
        // Soviet Iron Curtain Device: dark cast-iron base + 4 magnetic claws + glowing crimson energy sphere
        batches[concrete].add_cylinder_y([0.0, 0.14, 0.0], 0.88, 0.80, 0.14, 32, scale);
        batches[gunmetal].add_cylinder_y([0.0, 0.46, 0.0], 0.68, 0.54, 0.20, 32, scale);
        for (cx, cz) in [(-0.48, 0.0), (0.48, 0.0), (0.0, -0.48), (0.0, 0.48)] {
            batches[primary].add_tapered_box(
                [cx, 0.96, cz],
                [0.14, 0.14],
                [0.08, 0.08],
                0.42,
                scale,
            );
        }
        batches[emissive].add_dome_y([0.0, 0.96, 0.0], 0.36, 0.42, 12, 32, scale);
        batches[steel].add_cylinder_y([0.0, 0.96, 0.0], 0.44, 0.44, 0.04, 32, scale);
        return true;
    }

    if has_token(name, "gacsph") || name.contains("chronosphere") {
        // Allied Chronosphere: circular particle ring + 3 stabilizer pylons + glowing cyan sphere
        batches[concrete].add_cylinder_y([0.0, 0.12, 0.0], 0.90, 0.84, 0.12, 32, scale);
        batches[steel].add_cylinder_y([0.0, 0.32, 0.0], 0.72, 0.64, 0.10, 32, scale);
        for (px, pz) in [(-0.54, -0.36), (0.54, -0.36), (0.0, 0.58)] {
            batches[primary].add_tapered_box(
                [px, 0.82, pz],
                [0.16, 0.16],
                [0.08, 0.08],
                0.44,
                scale,
            );
        }
        batches[emissive].add_dome_y([0.0, 0.64, 0.0], 0.46, 0.56, 12, 32, scale);
        batches[steel].add_cylinder_y([0.0, 0.82, 0.0], 0.56, 0.56, 0.04, 32, scale);
        return true;
    }

    if has_token(name, "naclon") || name.contains("cloning") {
        // Cloning Vats: twin vertical glowing bio-tanks + DNA core + connecting catwalk
        batches[concrete].add_box([0.0, 0.10, 0.0], [0.88, 0.10, 0.88], scale);
        batches[primary].add_box([0.0, 0.34, 0.0], [0.76, 0.16, 0.68], scale);
        for cx in [-0.38, 0.38] {
            batches[emissive].add_cylinder_y([cx, 0.92, 0.0], 0.28, 0.28, 0.46, 28, scale);
            batches[steel].add_dome_y([cx, 1.38, 0.0], 0.30, 0.18, 10, 28, scale);
        }
        batches[steel].add_box([0.0, 1.24, 0.0], [0.42, 0.05, 0.14], scale);
        return true;
    }

    if has_token(name, "caoild") || name.contains("oil derrick") {
        // Tech Oil Derrick: tall lattice derrick + walking-beam pumpjack
        batches[concrete].add_box([0.0, 0.08, 0.0], [0.88, 0.08, 0.88], scale);
        batches[steel].add_tapered_box(
            [-0.24, 0.92, -0.18],
            [0.32, 0.32],
            [0.10, 0.10],
            0.78,
            scale,
        );
        batches[primary].add_box([0.28, 0.42, 0.18], [0.22, 0.28, 0.22], scale);
        batches[gunmetal].add_box([0.28, 0.78, 0.18], [0.08, 0.10, 0.48], scale);
        batches[emissive].add_cylinder_y([-0.24, 1.74, -0.18], 0.08, 0.06, 0.06, 20, scale);
        return true;
    }

    if has_token(name, "cahosp") || name.contains("hospital") {
        // Civilian Hospital: medical complex + rooftop helipad + glowing cross emblem
        batches[concrete].add_box([0.0, 0.10, 0.0], [0.92, 0.10, 0.92], scale);
        batches[steel].add_tapered_box([0.0, 0.62, 0.0], [0.78, 0.72], [0.72, 0.66], 0.44, scale);
        add_corrugated_roof_and_cladding(batches, [0.0, 0.62, 0.0], [0.72, 0.44, 0.66], 12, scale);
        batches[emissive].add_box([0.0, 1.09, 0.0], [0.36, 0.03, 0.12], scale);
        batches[emissive].add_box([0.0, 1.09, 0.0], [0.12, 0.03, 0.36], scale);
        batches[emissive].add_box([0.0, 0.66, 0.72], [0.46, 0.10, 0.03], scale);
        return true;
    }

    if has_token(name, "caairp") || name.contains("airport") {
        // Tech Airport: paratrooper hangar + air traffic control tower
        batches[concrete].add_box([0.0, 0.09, 0.0], [0.94, 0.09, 0.94], scale);
        batches[steel].add_cylinder_z([-0.22, 0.48, 0.12], 0.46, 0.58, 28, scale);
        batches[primary].add_tapered_box(
            [0.48, 0.86, -0.38],
            [0.22, 0.22],
            [0.16, 0.16],
            0.68,
            scale,
        );
        batches[emissive].add_cylinder_y([0.48, 1.62, -0.38], 0.24, 0.28, 0.12, 24, scale);
        return true;
    }

    if has_token(name, "caoutp") || name.contains("outpost") {
        // Tech Outpost: forward repair deck + tall central defense missile turret
        batches[concrete].add_box([0.0, 0.12, 0.0], [0.92, 0.12, 0.92], scale);
        batches[primary].add_tapered_box(
            [0.0, 0.48, -0.24],
            [0.68, 0.44],
            [0.58, 0.36],
            0.28,
            scale,
        );
        add_corrugated_roof_and_cladding(
            batches,
            [0.0, 0.48, -0.24],
            [0.58, 0.28, 0.36],
            10,
            scale,
        );
        batches[steel].add_cylinder_y([0.0, 1.02, -0.24], 0.26, 0.20, 0.32, 24, scale);
        batches[gunmetal].add_pitched_cylinder_z(
            [0.0, 1.38, -0.24],
            0.18,
            0.16,
            0.48,
            0.35,
            20,
            scale,
        );
        return true;
    }

    if has_token(name, "gawall") || has_token(name, "nawall") || name.contains("wall") {
        // Concrete defense wall with armored buttresses
        batches[concrete].add_tapered_box(
            [0.0, 0.42, 0.0],
            [0.46, 0.46],
            [0.36, 0.36],
            0.42,
            scale,
        );
        batches[primary].add_box([0.0, 0.88, 0.0], [0.40, 0.06, 0.40], scale);
        return true;
    }

    false
}

/// Named building state child meshes (`ScaffoldFrame`, `FactoryDoor`, `DamageOverlay`)
/// emitted alongside `Hull`, `Turret`, and `Rotor` so the client can drive construction,
/// unit rollout doors, and battle-damage states.
pub(super) struct BuildingStateRig {
    pub(super) scaffold_frame: [GeometryBatch; 6],
    pub(super) factory_door: [GeometryBatch; 6],
    pub(super) factory_door_translation: [f32; 3],
    pub(super) damage_overlay: [GeometryBatch; 6],
}

pub(super) fn build_building_state_rig(
    raw_name: &str,
    archetype: VisualArchetype,
    scale: [f32; 3],
) -> BuildingStateRig {
    let name_buf = raw_name.to_ascii_lowercase();
    let name = name_buf.as_str();
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;
    let pal = MatSlot::Concrete as usize;
    let dark = MatSlot::DarkAccent as usize;

    let is_compact = archetype == VisualArchetype::Turret
        || has_token(name, "atesla")
        || has_token(name, "tesla")
        || has_token(name, "tsla")
        || has_token(name, "nasam")
        || name.contains("patriot")
        || has_token(name, "naflak")
        || name.contains("flak cannon")
        || has_token(name, "gtgcan")
        || name.contains("grand cannon")
        || has_token(name, "gapill")
        || has_token(name, "nalasr")
        || has_token(name, "gawall")
        || has_token(name, "nawall")
        || name.contains("wall")
        || has_token(name, "gagap");

    let (half_xz, top_y, door_center, door_half) = if is_compact {
        (
            0.62f32,
            1.22f32,
            [0.0f32, 0.28, 0.48],
            [0.18f32, 0.16, 0.028],
        )
    } else if has_token(name, "gaweap")
        || has_token(name, "naweap")
        || has_token(name, "weap")
        || has_token(name, "sweap")
        || archetype == VisualArchetype::WarFactory
    {
        (0.95, 1.38, [0.0, 0.48, 0.63], [0.50, 0.35, 0.032])
    } else if has_token(name, "garefn")
        || has_token(name, "narefn")
        || has_token(name, "proc")
        || has_token(name, "sproc")
        || archetype == VisualArchetype::Refinery
    {
        (0.95, 1.42, [-0.14, 0.34, 0.38], [0.44, 0.18, 0.032])
    } else if has_token(name, "nahand")
        || has_token(name, "barr")
        || has_token(name, "gapile")
        || has_token(name, "tent")
        || archetype == VisualArchetype::Barracks
    {
        let dx = if has_token(name, "gapile") || has_token(name, "tent") {
            -0.16
        } else {
            0.0
        };
        (0.90, 1.34, [dx, 0.32, 0.41], [0.32, 0.20, 0.030])
    } else if has_token(name, "namisl") || name.contains("nuclear missile") {
        (0.94, 1.78, [0.0, 0.38, 0.0], [0.50, 0.035, 0.46])
    } else {
        (0.94, 1.44, [0.0, 0.34, 0.56], [0.30, 0.22, 0.030])
    };

    // 1. ScaffoldFrame: perimeter steel construction lattice, catwalk ledgers & crane diagonal braces
    let mut scaffold = new_six_batches();
    for sx in [-half_xz, half_xz] {
        for sz in [-half_xz, half_xz] {
            scaffold[steel].add_box([sx, top_y * 0.5, sz], [0.032, top_y * 0.5, 0.032], scale);
        }
    }
    let tier_heights = [top_y * 0.32, top_y * 0.64, top_y * 0.96];
    for &ty in &tier_heights {
        for side in [-1.0f32, 1.0] {
            scaffold[gunmetal].add_box([0.0, ty, side * half_xz], [half_xz, 0.020, 0.022], scale);
            scaffold[gunmetal].add_box([side * half_xz, ty, 0.0], [0.022, 0.020, half_xz], scale);
        }
    }
    // Safety-yellow scaffold catwalk toe-boards on mid tier + X-bracing struts
    scaffold[pal].set_color([0.95, 0.78, 0.14]);
    scaffold[pal].add_box(
        [0.0, tier_heights[1] + 0.022, half_xz],
        [half_xz * 0.88, 0.014, 0.045],
        scale,
    );
    scaffold[pal].add_box(
        [0.0, tier_heights[1] + 0.022, -half_xz],
        [half_xz * 0.88, 0.014, 0.045],
        scale,
    );
    for side in [-1.0f32, 1.0] {
        scaffold[steel].add_oriented_beam(
            [-half_xz, 0.04, side * half_xz],
            [half_xz, tier_heights[1], side * half_xz],
            0.016,
            0.016,
            10,
            scale,
        );
        scaffold[steel].add_oriented_beam(
            [side * half_xz, 0.04, -half_xz],
            [side * half_xz, tier_heights[1], half_xz],
            0.016,
            0.016,
            10,
            scale,
        );
    }
    for b in &mut scaffold {
        b.smooth_vertex_normals(0.45);
    }

    // 2. FactoryDoor: ribbed armored bay shutter / blast hatch centered at its hinge/slide pivot
    let mut door = new_six_batches();
    door[gunmetal].add_box(door_center, door_half, scale);
    let slat_count = 4usize;
    for s in 0..slat_count {
        let t = (s as f32 / (slat_count - 1) as f32) * 2.0 - 1.0;
        if door_half[1] >= door_half[2] {
            let sy_pos = door_center[1] + t * (door_half[1] * 0.72);
            door[steel].add_box(
                [door_center[0], sy_pos, door_center[2] + door_half[2] * 0.65],
                [door_half[0] * 0.92, door_half[1] * 0.14, 0.016],
                scale,
            );
        } else {
            let sz_pos = door_center[2] + t * (door_half[2] * 0.72);
            door[steel].add_box(
                [door_center[0], door_center[1] + door_half[1] * 0.65, sz_pos],
                [door_half[0] * 0.92, 0.016, door_half[2] * 0.14],
                scale,
            );
        }
    }
    door[pal].set_color([0.96, 0.76, 0.12]);
    door[pal].add_box(
        [
            door_center[0],
            door_center[1] - door_half[1] * 0.80,
            door_center[2] + door_half[2] * 0.55,
        ],
        [door_half[0] * 0.88, 0.022, 0.018],
        scale,
    );
    door[emissive].add_box(
        [
            door_center[0],
            door_center[1] + door_half[1] * 0.86,
            door_center[2] + door_half[2] * 0.65,
        ],
        [door_half[0] * 0.45, 0.018, 0.016],
        scale,
    );
    let door_world_pivot = [
        door_center[0] * scale[0],
        door_center[1] * scale[1],
        door_center[2] * scale[2],
    ];
    for b in &mut door {
        b.smooth_vertex_normals(0.45);
        b.translate_all([
            -door_world_pivot[0],
            -door_world_pivot[1],
            -door_world_pivot[2],
        ]);
    }

    // 3. DamageOverlay: scorched blast plates, buckled steel girders & glowing ember fissures
    let mut dmg = new_six_batches();
    let wall_r = half_xz * 0.74;
    dmg[dark].add_box(
        [-wall_r * 0.45, top_y * 0.42, wall_r],
        [0.20, 0.14, 0.025],
        scale,
    );
    dmg[dark].add_box(
        [wall_r, top_y * 0.48, -wall_r * 0.30],
        [0.025, 0.16, 0.22],
        scale,
    );
    dmg[gunmetal].add_oriented_beam(
        [-wall_r * 0.62, top_y * 0.30, wall_r + 0.02],
        [-wall_r * 0.22, top_y * 0.58, wall_r + 0.04],
        0.022,
        0.022,
        10,
        scale,
    );
    dmg[gunmetal].add_oriented_beam(
        [wall_r + 0.02, top_y * 0.34, -wall_r * 0.50],
        [wall_r + 0.04, top_y * 0.66, -wall_r * 0.12],
        0.022,
        0.022,
        10,
        scale,
    );
    dmg[emissive].add_oriented_beam(
        [-wall_r * 0.55, top_y * 0.36, wall_r + 0.03],
        [-wall_r * 0.30, top_y * 0.50, wall_r + 0.03],
        0.015,
        0.015,
        8,
        scale,
    );
    dmg[emissive].add_ellipsoid(
        [wall_r * 0.30, top_y * 0.68, -wall_r * 0.25],
        [0.09, 0.035, 0.08],
        8,
        14,
        scale,
    );
    dmg[pal].set_color([0.24, 0.23, 0.22]);
    dmg[pal].add_box(
        [-wall_r * 0.72, 0.08, wall_r * 0.85],
        [0.12, 0.07, 0.10],
        scale,
    );
    dmg[pal].add_box(
        [wall_r * 0.82, 0.07, -wall_r * 0.68],
        [0.11, 0.06, 0.12],
        scale,
    );
    for b in &mut dmg {
        b.smooth_vertex_normals(0.45);
    }

    BuildingStateRig {
        scaffold_frame: scaffold,
        factory_door: door,
        factory_door_translation: door_world_pivot,
        damage_overlay: dmg,
    }
}
