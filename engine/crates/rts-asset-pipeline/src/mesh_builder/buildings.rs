pub(super) use super::buildings_support::{
    build_building_state_rig, try_populate_tech_and_support_buildings,
};
use super::components::*;
use super::primitives::{GeometryBatch, MatSlot};
use super::SocketRigSpec;
use rts_sdk::schema::VisualArchetype;

pub(super) fn has_token(haystack: &str, token: &str) -> bool {
    haystack
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|w| w == token)
}

/// Step 1A: Populates geometry for primary production & economy buildings
/// (Construction Yard, Power Plants, Barracks, Ore Purifier, Ore Refinery, War Factory).
pub(super) fn try_populate_production_buildings(
    name: &str,
    archetype: VisualArchetype,
    scale: [f32; 3],
    batches: &mut [GeometryBatch; 6],
) -> bool {
    let primary = MatSlot::Primary as usize;
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;
    let concrete = MatSlot::Concrete as usize;
    let dark = MatSlot::DarkAccent as usize;

    if has_token(name, "gacnst")
        || has_token(name, "nacnst")
        || has_token(name, "fact")
        || has_token(name, "sfact")
        || archetype == VisualArchetype::ConstructionYard
    {
        // Foundation concrete slab with perimeter curb
        batches[concrete].add_tapered_box(
            [0.0, 0.09, 0.0],
            [0.98, 0.98],
            [0.94, 0.94],
            0.09,
            scale,
        );
        batches[dark].add_box([0.0, 0.19, 0.0], [0.90, 0.02, 0.90], scale);
        // Main multi-level industrial assembly hall with corrugated roof & structural wall pilasters
        batches[primary].add_tapered_box(
            [-0.22, 0.56, -0.16],
            [0.58, 0.62],
            [0.50, 0.54],
            0.36,
            scale,
        );
        add_corrugated_roof_and_cladding(
            batches,
            [-0.22, 0.56, -0.16],
            [0.52, 0.36, 0.56],
            14,
            scale,
        );
        batches[steel].add_tapered_box(
            [-0.22, 1.00, -0.16],
            [0.46, 0.50],
            [0.36, 0.40],
            0.10,
            scale,
        );
        batches[emissive].add_box([-0.22, 0.64, 0.44], [0.38, 0.09, 0.03], scale);
        batches[emissive].add_box([-0.22, 0.84, 0.41], [0.30, 0.06, 0.03], scale);
        // Secondary fabrication annex & cylindrical storage tanks
        batches[primary].add_box([0.46, 0.42, -0.42], [0.32, 0.24, 0.34], scale);
        add_corrugated_roof_and_cladding(
            batches,
            [0.46, 0.42, -0.42],
            [0.32, 0.24, 0.34],
            8,
            scale,
        );
        batches[steel].add_cylinder_y([0.58, 0.52, 0.08], 0.14, 0.14, 0.34, 24, scale);
        batches[steel].add_cylinder_y([0.58, 0.52, 0.38], 0.14, 0.14, 0.34, 24, scale);
        // Command dome on roof
        batches[steel].add_dome_y([-0.22, 1.10, -0.16], 0.28, 0.26, 8, 28, scale);
        // Towering lattice gantry crane with boom, diagonal struts, counter-jib, and hook block
        batches[steel].add_tapered_box([0.52, 0.96, 0.62], [0.12, 0.12], [0.08, 0.08], 0.78, scale);
        for bay in 0..8 {
            let by = 0.28 + (bay as f32) * 0.18;
            batches[gunmetal].add_box([0.52, by, 0.62], [0.13, 0.015, 0.13], scale);
        }
        batches[steel].add_box([0.10, 1.68, 0.62], [0.54, 0.06, 0.08], scale);
        batches[gunmetal].add_cylinder_y([-0.24, 1.38, 0.62], 0.03, 0.03, 0.26, 16, scale);
        batches[emissive].add_cylinder_y([0.52, 1.80, 0.62], 0.07, 0.05, 0.06, 20, scale);
        return true;
    }

    if has_token(name, "nanrct") || has_token(name, "naapwr") || name.contains("nuclear reactor") {
        // Soviet Nuclear Reactor: massive containment dome + 4 hyperbolic cooling towers + glowing green core
        batches[concrete].add_box([0.0, 0.10, 0.0], [0.95, 0.10, 0.95], scale);
        batches[primary].add_tapered_box([0.0, 0.44, 0.0], [0.84, 0.84], [0.72, 0.72], 0.26, scale);
        add_corrugated_roof_and_cladding(batches, [0.0, 0.44, 0.0], [0.76, 0.26, 0.76], 12, scale);
        batches[steel].add_dome_y([0.0, 0.68, 0.0], 0.56, 0.68, 10, 32, scale);
        batches[emissive].add_cylinder_y([0.0, 1.38, 0.0], 0.18, 0.12, 0.08, 24, scale);
        for (cx, cz) in [(-0.62, -0.62), (0.62, -0.62), (-0.62, 0.62), (0.62, 0.62)] {
            add_hyperbolic_cooling_tower(batches, [cx, 0.18, cz], 0.22, 1.18, scale);
        }
        return true;
    }

    if has_token(name, "napowr") || has_token(name, "spowr") || name.contains("tesla reactor") {
        // Soviet Tesla Reactor: copper/steel coil dome + 6 induction rings + twin plasma capacitors
        batches[concrete].add_box([0.0, 0.10, 0.0], [0.90, 0.10, 0.90], scale);
        batches[primary].add_tapered_box([0.0, 0.42, 0.0], [0.78, 0.78], [0.68, 0.68], 0.24, scale);
        add_corrugated_roof_and_cladding(batches, [0.0, 0.42, 0.0], [0.72, 0.24, 0.72], 12, scale);
        batches[gunmetal].add_dome_y([-0.14, 0.64, -0.10], 0.46, 0.54, 10, 32, scale);
        for ring_idx in 0..6 {
            let ry = 0.72 + (ring_idx as f32) * 0.08;
            let rr = 0.48 - (ring_idx as f32) * 0.04;
            batches[steel].add_cylinder_y([-0.14, ry, -0.10], rr, rr, 0.018, 28, scale);
        }
        batches[emissive].add_dome_y([-0.14, 1.18, -0.10], 0.20, 0.20, 8, 24, scale);
        batches[steel].add_cylinder_y([0.46, 0.82, 0.36], 0.16, 0.12, 0.46, 24, scale);
        batches[emissive].add_dome_y([0.46, 1.28, 0.36], 0.14, 0.14, 8, 24, scale);
        return true;
    }

    if has_token(name, "gapowr")
        || has_token(name, "powr")
        || archetype == VisualArchetype::PowerPlant
    {
        // Allied Power Plant: turbine hall + twin sculpted hyperbolic cooling towers + transformer bank
        batches[concrete].add_box([0.0, 0.10, 0.0], [0.92, 0.10, 0.92], scale);
        batches[primary].add_tapered_box(
            [-0.18, 0.42, 0.18],
            [0.64, 0.56],
            [0.56, 0.48],
            0.24,
            scale,
        );
        add_corrugated_roof_and_cladding(
            batches,
            [-0.18, 0.42, 0.18],
            [0.58, 0.24, 0.50],
            12,
            scale,
        );
        batches[emissive].add_box([-0.18, 0.46, 0.73], [0.38, 0.07, 0.03], scale);
        for (tx, tz) in [(-0.34, -0.34), (0.38, -0.28)] {
            add_hyperbolic_cooling_tower(batches, [tx, 0.16, tz], 0.32, 1.24, scale);
        }
        batches[steel].add_box([0.52, 0.44, 0.42], [0.22, 0.24, 0.26], scale);
        return true;
    }

    if has_token(name, "nahand") || has_token(name, "barr") {
        // Soviet Barracks: red-star citadel hall + central sculpted monument statue + parade ramp
        batches[concrete].add_box([0.0, 0.09, 0.0], [0.90, 0.09, 0.90], scale);
        batches[primary].add_tapered_box(
            [0.0, 0.48, -0.18],
            [0.76, 0.54],
            [0.64, 0.44],
            0.32,
            scale,
        );
        add_corrugated_roof_and_cladding(
            batches,
            [0.0, 0.48, -0.18],
            [0.68, 0.32, 0.48],
            12,
            scale,
        );
        batches[dark].add_box([0.0, 0.34, 0.36], [0.36, 0.22, 0.06], scale);
        batches[steel].add_cylinder_y([-0.48, 0.94, -0.22], 0.16, 0.10, 0.42, 24, scale);
        batches[steel].add_cylinder_y([0.48, 0.94, -0.22], 0.16, 0.10, 0.42, 24, scale);
        let statue_profile = [
            [0.22, 0.0],
            [0.18, 0.22],
            [0.10, 0.45],
            [0.16, 0.68],
            [0.12, 0.92],
            [0.20, 1.10],
            [0.04, 1.26],
        ];
        batches[steel].add_spline_lathe_y([0.0, 0.78, -0.16], &statue_profile, 6, 32, scale);
        batches[emissive].add_dome_y([0.0, 1.98, -0.16], 0.15, 0.18, 8, 24, scale);
        return true;
    }

    if has_token(name, "gapile")
        || has_token(name, "tent")
        || archetype == VisualArchetype::Barracks
    {
        // Allied Barracks: arched Quonset drill hall + watchtower + parade ground
        batches[concrete].add_box([0.0, 0.08, 0.0], [0.90, 0.08, 0.90], scale);
        batches[primary].add_tapered_box(
            [-0.16, 0.44, -0.18],
            [0.64, 0.56],
            [0.54, 0.48],
            0.30,
            scale,
        );
        add_corrugated_roof_and_cladding(
            batches,
            [-0.16, 0.44, -0.18],
            [0.58, 0.30, 0.50],
            12,
            scale,
        );
        batches[steel].add_cylinder_z([-0.16, 0.74, -0.18], 0.48, 0.46, 28, scale);
        batches[dark].add_box([-0.16, 0.32, 0.38], [0.32, 0.20, 0.05], scale);
        batches[gunmetal].add_tapered_box(
            [0.54, 0.72, -0.36],
            [0.16, 0.16],
            [0.12, 0.12],
            0.58,
            scale,
        );
        batches[primary].add_box([0.54, 1.36, -0.36], [0.20, 0.12, 0.20], scale);
        batches[emissive].add_cylinder_z([0.54, 1.52, -0.26], 0.08, 0.10, 20, scale);
        return true;
    }

    if has_token(name, "gaorep") || name.contains("purifier") {
        // Allied Ore Purifier: tall distillation tower + 4 chemical tanks + glowing golden smelter core
        batches[concrete].add_box([0.0, 0.10, 0.0], [0.94, 0.10, 0.94], scale);
        batches[primary].add_tapered_box([0.0, 0.48, 0.0], [0.76, 0.76], [0.64, 0.64], 0.30, scale);
        add_corrugated_roof_and_cladding(batches, [0.0, 0.48, 0.0], [0.68, 0.30, 0.68], 12, scale);
        batches[steel].add_cylinder_y([0.0, 1.12, 0.0], 0.28, 0.18, 0.62, 28, scale);
        batches[emissive].add_cylinder_y([0.0, 1.05, 0.0], 0.32, 0.32, 0.12, 28, scale);
        batches[emissive].add_dome_y([0.0, 1.74, 0.0], 0.18, 0.18, 8, 24, scale);
        for (sx, sz) in [(-0.56, -0.56), (0.56, -0.56), (-0.56, 0.56), (0.56, 0.56)] {
            batches[steel].add_cylinder_y([sx, 0.68, sz], 0.18, 0.18, 0.46, 24, scale);
        }
        return true;
    }

    if has_token(name, "garefn")
        || has_token(name, "narefn")
        || has_token(name, "proc")
        || has_token(name, "sproc")
        || archetype == VisualArchetype::Refinery
    {
        // Ore Refinery: smelter hall + harvester unloading ramp + 3 ore silos + smokestacks
        batches[concrete].add_box([0.0, 0.10, 0.0], [0.95, 0.10, 0.95], scale);
        batches[primary].add_tapered_box(
            [-0.18, 0.54, -0.22],
            [0.68, 0.54],
            [0.58, 0.46],
            0.38,
            scale,
        );
        add_corrugated_roof_and_cladding(
            batches,
            [-0.18, 0.54, -0.22],
            [0.62, 0.38, 0.48],
            14,
            scale,
        );
        batches[concrete].add_tapered_box(
            [-0.14, 0.18, 0.52],
            [0.64, 0.36],
            [0.58, 0.32],
            0.09,
            scale,
        );
        batches[dark].add_box([-0.14, 0.34, 0.34], [0.46, 0.18, 0.10], scale);
        batches[emissive].add_box([-0.66, 0.28, 0.52], [0.05, 0.05, 0.30], scale);
        batches[emissive].add_box([0.38, 0.28, 0.52], [0.05, 0.05, 0.30], scale);
        for sz in [-0.52, -0.06, 0.40] {
            batches[steel].add_cylinder_y([0.64, 0.72, sz], 0.20, 0.20, 0.48, 28, scale);
            batches[primary].add_dome_y([0.64, 1.20, sz], 0.20, 0.12, 8, 24, scale);
        }
        batches[gunmetal].add_cylinder_y([-0.58, 1.24, -0.48], 0.10, 0.07, 0.48, 20, scale);
        batches[emissive].add_dome_y([-0.58, 1.72, -0.48], 0.09, 0.10, 6, 20, scale);
        return true;
    }

    if has_token(name, "gaweap")
        || has_token(name, "naweap")
        || has_token(name, "weap")
        || has_token(name, "sweap")
        || archetype == VisualArchetype::WarFactory
    {
        // War Factory: massive multi-arch vehicle assembly bay + rollout ramp + rooftop fans + twin stacks
        batches[concrete].add_box([0.0, 0.10, 0.0], [0.96, 0.10, 0.96], scale);
        batches[primary].add_tapered_box(
            [0.0, 0.58, -0.12],
            [0.86, 0.72],
            [0.78, 0.66],
            0.42,
            scale,
        );
        add_corrugated_roof_and_cladding(
            batches,
            [0.0, 0.58, -0.12],
            [0.80, 0.42, 0.68],
            16,
            scale,
        );
        batches[steel].add_cylinder_z([-0.36, 1.02, -0.12], 0.38, 0.64, 28, scale);
        batches[steel].add_cylinder_z([0.36, 1.02, -0.12], 0.38, 0.64, 28, scale);
        batches[concrete].add_tapered_box(
            [0.0, 0.14, 0.76],
            [0.68, 0.22],
            [0.60, 0.18],
            0.07,
            scale,
        );
        batches[dark].add_box([0.0, 0.48, 0.61], [0.52, 0.36, 0.04], scale);
        batches[emissive].add_box([0.0, 0.90, 0.62], [0.46, 0.06, 0.04], scale);
        batches[gunmetal].add_cylinder_y([-0.76, 1.18, -0.46], 0.09, 0.07, 0.52, 20, scale);
        batches[gunmetal].add_cylinder_y([-0.76, 1.18, -0.10], 0.09, 0.07, 0.52, 20, scale);
        return true;
    }

    false
}

/// Step 1C: Populates geometry for base defense turrets (Prism Tower, Tesla Coil, Patriot SAM, Flak, Grand Cannon, Pillbox).
pub(super) fn try_populate_defensive_turrets(
    name: &str,
    archetype: VisualArchetype,
    scale: [f32; 3],
    batches: &mut [GeometryBatch; 6],
) -> bool {
    let primary = MatSlot::Primary as usize;
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;
    let concrete = MatSlot::Concrete as usize;

    if has_token(name, "atesla") || name.contains("prism tower") {
        // Allied Prism Tower: spline-lathed stepped chrome column + 6 focusing rings + faceted prism crystal
        batches[concrete].add_cylinder_y([0.0, 0.16, 0.0], 0.58, 0.48, 0.16, 32, scale);
        let prism_col = [
            [0.36, 0.0],
            [0.30, 0.26],
            [0.24, 0.52],
            [0.20, 0.78],
            [0.16, 1.04],
        ];
        batches[primary].add_spline_lathe_y([0.0, 0.28, 0.0], &prism_col, 6, 32, scale);
        for (ry, rr) in [(0.52, 0.36), (0.72, 0.33), (0.92, 0.30), (1.12, 0.27)] {
            batches[steel].add_cylinder_y([0.0, ry, 0.0], rr, rr, 0.028, 32, scale);
        }
        let crystal_profile = [[0.06, 0.0], [0.24, 0.16], [0.18, 0.34], [0.02, 0.48]];
        batches[emissive].add_spline_lathe_y([0.0, 1.24, 0.0], &crystal_profile, 5, 24, scale);
        return true;
    }

    if (has_token(name, "tesla") || has_token(name, "tsla"))
        && !name.contains("trooper")
        && !name.contains("tank")
    {
        // Soviet Tesla Coil: insulator base + 3 chrome transformer rings + silver discharge sphere
        batches[concrete].add_cylinder_y([0.0, 0.18, 0.0], 0.56, 0.46, 0.18, 28, scale);
        let coil_profile = [[0.26, 0.0], [0.20, 0.32], [0.15, 0.64], [0.11, 0.96]];
        batches[primary].add_spline_lathe_y([0.0, 0.30, 0.0], &coil_profile, 6, 28, scale);
        for (ry, rr) in [(0.64, 0.38), (0.90, 0.32), (1.16, 0.26)] {
            batches[steel].add_cylinder_y([0.0, ry, 0.0], rr, rr, 0.035, 32, scale);
        }
        batches[emissive].add_dome_y([0.0, 1.32, 0.0], 0.24, 0.26, 12, 32, scale);
        return true;
    }

    if has_token(name, "nasam") || name.contains("patriot") {
        // Allied Patriot Missile System: rotatable pedestal + angled 4-cell SAM launcher + radar dish
        batches[concrete].add_cylinder_y([0.0, 0.14, 0.0], 0.58, 0.50, 0.14, 24, scale);
        batches[steel].add_cylinder_y([0.0, 0.42, 0.0], 0.26, 0.22, 0.18, 24, scale);
        batches[primary].add_pitched_cylinder_z(
            [0.0, 0.68, -0.14],
            0.28,
            0.28,
            0.62,
            0.48,
            4,
            scale,
        );
        batches[emissive].add_pitched_cylinder_z(
            [0.0, 0.94, 0.36],
            0.18,
            0.18,
            0.06,
            0.48,
            4,
            scale,
        );
        return true;
    }

    if has_token(name, "naflak") || name.contains("flak cannon") {
        // Soviet Flak Cannon: heavy AA mount + quad elevated 88mm flak barrels
        batches[concrete].add_cylinder_y([0.0, 0.16, 0.0], 0.58, 0.48, 0.16, 24, scale);
        batches[primary].add_tapered_box(
            [0.0, 0.48, -0.06],
            [0.34, 0.34],
            [0.26, 0.26],
            0.20,
            scale,
        );
        for (bx, by) in [(-0.12, 0.56), (0.12, 0.56), (-0.12, 0.68), (0.12, 0.68)] {
            batches[gunmetal].add_pitched_cylinder_z(
                [bx, by, 0.0],
                0.045,
                0.04,
                0.68,
                0.58,
                16,
                scale,
            );
        }
        return true;
    }

    if has_token(name, "gtgcan") || name.contains("grand cannon") {
        // French Grand Cannon: massive coastal casemate turret + oversized long-range artillery barrel
        batches[concrete].add_cylinder_y([0.0, 0.22, 0.0], 0.82, 0.70, 0.22, 32, scale);
        batches[primary].add_tapered_box(
            [0.0, 0.64, -0.12],
            [0.58, 0.62],
            [0.42, 0.46],
            0.26,
            scale,
        );
        add_rifled_cannon_barrel(batches, [0.0, 0.68, 0.12], 0.095, 1.15, scale);
        batches[steel].add_pitched_cylinder_z(
            [-0.18, 0.74, 0.06],
            0.06,
            0.06,
            0.52,
            0.18,
            18,
            scale,
        );
        batches[steel].add_pitched_cylinder_z(
            [0.18, 0.74, 0.06],
            0.06,
            0.06,
            0.52,
            0.18,
            18,
            scale,
        );
        return true;
    }

    if has_token(name, "gapill")
        || has_token(name, "nalasr")
        || archetype == VisualArchetype::Turret
    {
        // Pillbox / Sentry Gun: octagonal bunker + twin Vulcan autocannon barrels
        batches[concrete].add_cylinder_y([0.0, 0.22, 0.0], 0.56, 0.46, 0.22, 24, scale);
        batches[primary].add_dome_y([0.0, 0.44, 0.0], 0.42, 0.28, 10, 24, scale);
        add_rifled_cannon_barrel(batches, [-0.11, 0.54, 0.14], 0.045, 0.52, scale);
        add_rifled_cannon_barrel(batches, [0.11, 0.54, 0.14], 0.045, 0.52, scale);
        batches[emissive].add_box([0.0, 0.58, 0.32], [0.18, 0.04, 0.04], scale);
        return true;
    }

    false
}

/// Computes exact weapon muzzle, exhaust stack, and damage-smoke socket locations for buildings & defenses.
pub(super) fn compute_building_sockets(
    raw_name: &str,
    archetype: VisualArchetype,
    scale: [f32; 3],
) -> SocketRigSpec {
    let name_buf = raw_name.to_ascii_lowercase();
    let name = name_buf.as_str();
    let [sx, sy, sz] = scale;

    if has_token(name, "atesla") || name.contains("prism tower") {
        return SocketRigSpec {
            muzzle_01: Some([0.0, 1.72 * sy, 0.0]),
            muzzle_02: None,
            exhaust_01: None,
            damage_smoke_01: [0.0, 1.15 * sy, 0.0],
        };
    }
    if (has_token(name, "tesla") || has_token(name, "tsla"))
        && !name.contains("trooper")
        && !name.contains("tank")
        && !name.contains("reactor")
    {
        return SocketRigSpec {
            muzzle_01: Some([0.0, 1.56 * sy, 0.0]),
            muzzle_02: None,
            exhaust_01: None,
            damage_smoke_01: [0.0, 1.10 * sy, 0.0],
        };
    }
    if has_token(name, "nasam") || name.contains("patriot") {
        return SocketRigSpec {
            muzzle_01: Some([0.0, 0.98 * sy, 0.42 * sz]),
            muzzle_02: None,
            exhaust_01: None,
            damage_smoke_01: [0.0, 0.72 * sy, 0.0],
        };
    }
    if has_token(name, "naflak") || name.contains("flak cannon") {
        return SocketRigSpec {
            muzzle_01: Some([-0.12 * sx, 0.99 * sy, 0.57 * sz]),
            muzzle_02: Some([0.12 * sx, 0.99 * sy, 0.57 * sz]),
            exhaust_01: None,
            damage_smoke_01: [0.0, 0.68 * sy, -0.06 * sz],
        };
    }
    if has_token(name, "gtgcan") || name.contains("grand cannon") {
        return SocketRigSpec {
            muzzle_01: Some([0.0, 0.68 * sy, 1.27 * sz]),
            muzzle_02: None,
            exhaust_01: Some([0.0, 0.84 * sy, -0.58 * sz]),
            damage_smoke_01: [0.0, 0.90 * sy, -0.12 * sz],
        };
    }
    if has_token(name, "gapill")
        || has_token(name, "nalasr")
        || archetype == VisualArchetype::Turret
    {
        return SocketRigSpec {
            muzzle_01: Some([-0.11 * sx, 0.54 * sy, 0.66 * sz]),
            muzzle_02: Some([0.11 * sx, 0.54 * sy, 0.66 * sz]),
            exhaust_01: None,
            damage_smoke_01: [0.0, 0.68 * sy, 0.0],
        };
    }
    if has_token(name, "caoutp") || name.contains("outpost") {
        return SocketRigSpec {
            muzzle_01: Some([0.0, 1.54 * sy, 0.21 * sz]),
            muzzle_02: None,
            exhaust_01: Some([0.0, 0.76 * sy, -0.58 * sz]),
            damage_smoke_01: [0.0, 0.82 * sy, -0.24 * sz],
        };
    }
    if has_token(name, "nanrct") || has_token(name, "naapwr") || name.contains("nuclear reactor") {
        return SocketRigSpec {
            muzzle_01: None,
            muzzle_02: None,
            exhaust_01: Some([-0.62 * sx, 1.36 * sy, -0.62 * sz]),
            damage_smoke_01: [0.0, 1.36 * sy, 0.0],
        };
    }
    if has_token(name, "napowr") || has_token(name, "spowr") || name.contains("tesla reactor") {
        return SocketRigSpec {
            muzzle_01: None,
            muzzle_02: None,
            exhaust_01: Some([0.46 * sx, 1.38 * sy, 0.36 * sz]),
            damage_smoke_01: [-0.14 * sx, 1.22 * sy, -0.10 * sz],
        };
    }
    if has_token(name, "gapowr")
        || has_token(name, "powr")
        || archetype == VisualArchetype::PowerPlant
    {
        return SocketRigSpec {
            muzzle_01: None,
            muzzle_02: None,
            exhaust_01: Some([-0.34 * sx, 1.40 * sy, -0.34 * sz]),
            damage_smoke_01: [-0.18 * sx, 0.72 * sy, 0.18 * sz],
        };
    }
    if has_token(name, "garefn")
        || has_token(name, "narefn")
        || has_token(name, "proc")
        || has_token(name, "sproc")
        || archetype == VisualArchetype::Refinery
    {
        return SocketRigSpec {
            muzzle_01: None,
            muzzle_02: None,
            exhaust_01: Some([-0.58 * sx, 1.76 * sy, -0.48 * sz]),
            damage_smoke_01: [-0.18 * sx, 0.95 * sy, -0.22 * sz],
        };
    }
    if has_token(name, "gaorep") || name.contains("purifier") {
        return SocketRigSpec {
            muzzle_01: None,
            muzzle_02: None,
            exhaust_01: Some([0.0, 1.88 * sy, 0.0]),
            damage_smoke_01: [0.0, 1.10 * sy, 0.0],
        };
    }
    if has_token(name, "gaweap")
        || has_token(name, "naweap")
        || has_token(name, "weap")
        || has_token(name, "sweap")
        || archetype == VisualArchetype::WarFactory
    {
        return SocketRigSpec {
            muzzle_01: None,
            muzzle_02: None,
            exhaust_01: Some([-0.76 * sx, 1.70 * sy, -0.46 * sz]),
            damage_smoke_01: [0.0, 1.05 * sy, -0.12 * sz],
        };
    }
    if has_token(name, "caoild") || name.contains("oil derrick") {
        return SocketRigSpec {
            muzzle_01: None,
            muzzle_02: None,
            exhaust_01: Some([-0.24 * sx, 1.80 * sy, -0.18 * sz]),
            damage_smoke_01: [0.28 * sx, 0.78 * sy, 0.18 * sz],
        };
    }

    SocketRigSpec {
        muzzle_01: None,
        muzzle_02: None,
        exhaust_01: Some([0.48 * sx, 0.96 * sy, -0.48 * sz]),
        damage_smoke_01: [0.0, 1.02 * sy, 0.0],
    }
}
