use super::components::*;
use super::primitives::{GeometryBatch, MatSlot};
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
        batches[concrete].add_tapered_box([0.0, 0.09, 0.0], [0.98, 0.98], [0.94, 0.94], 0.09, scale);
        batches[dark].add_box([0.0, 0.19, 0.0], [0.90, 0.02, 0.90], scale);
        // Main multi-level industrial assembly hall with corrugated roof & structural wall pilasters
        batches[primary].add_tapered_box([-0.22, 0.56, -0.16], [0.58, 0.62], [0.50, 0.54], 0.36, scale);
        add_corrugated_roof_and_cladding(batches, [-0.22, 0.56, -0.16], [0.52, 0.36, 0.56], 14, scale);
        batches[steel].add_tapered_box([-0.22, 1.00, -0.16], [0.46, 0.50], [0.36, 0.40], 0.10, scale);
        batches[emissive].add_box([-0.22, 0.64, 0.44], [0.38, 0.09, 0.03], scale);
        batches[emissive].add_box([-0.22, 0.84, 0.41], [0.30, 0.06, 0.03], scale);
        // Secondary fabrication annex & cylindrical storage tanks
        batches[primary].add_box([0.46, 0.42, -0.42], [0.32, 0.24, 0.34], scale);
        add_corrugated_roof_and_cladding(batches, [0.46, 0.42, -0.42], [0.32, 0.24, 0.34], 8, scale);
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
        batches[primary].add_tapered_box([-0.18, 0.42, 0.18], [0.64, 0.56], [0.56, 0.48], 0.24, scale);
        add_corrugated_roof_and_cladding(batches, [-0.18, 0.42, 0.18], [0.58, 0.24, 0.50], 12, scale);
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
        batches[primary].add_tapered_box([0.0, 0.48, -0.18], [0.76, 0.54], [0.64, 0.44], 0.32, scale);
        add_corrugated_roof_and_cladding(batches, [0.0, 0.48, -0.18], [0.68, 0.32, 0.48], 12, scale);
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
        batches[primary].add_tapered_box([-0.16, 0.44, -0.18], [0.64, 0.56], [0.54, 0.48], 0.30, scale);
        add_corrugated_roof_and_cladding(batches, [-0.16, 0.44, -0.18], [0.58, 0.30, 0.50], 12, scale);
        batches[steel].add_cylinder_z([-0.16, 0.74, -0.18], 0.48, 0.46, 28, scale);
        batches[dark].add_box([-0.16, 0.32, 0.38], [0.32, 0.20, 0.05], scale);
        batches[gunmetal].add_tapered_box([0.54, 0.72, -0.36], [0.16, 0.16], [0.12, 0.12], 0.58, scale);
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
        batches[primary].add_tapered_box([-0.18, 0.54, -0.22], [0.68, 0.54], [0.58, 0.46], 0.38, scale);
        add_corrugated_roof_and_cladding(batches, [-0.18, 0.54, -0.22], [0.62, 0.38, 0.48], 14, scale);
        batches[concrete].add_tapered_box([-0.14, 0.18, 0.52], [0.64, 0.36], [0.58, 0.32], 0.09, scale);
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
        batches[primary].add_tapered_box([0.0, 0.58, -0.12], [0.86, 0.72], [0.78, 0.66], 0.42, scale);
        add_corrugated_roof_and_cladding(batches, [0.0, 0.58, -0.12], [0.80, 0.42, 0.68], 16, scale);
        batches[steel].add_cylinder_z([-0.36, 1.02, -0.12], 0.38, 0.64, 28, scale);
        batches[steel].add_cylinder_z([0.36, 1.02, -0.12], 0.38, 0.64, 28, scale);
        batches[concrete].add_tapered_box([0.0, 0.14, 0.76], [0.68, 0.22], [0.60, 0.18], 0.07, scale);
        batches[dark].add_box([0.0, 0.48, 0.61], [0.52, 0.36, 0.04], scale);
        batches[emissive].add_box([0.0, 0.90, 0.62], [0.46, 0.06, 0.04], scale);
        batches[gunmetal].add_cylinder_y([-0.76, 1.18, -0.46], 0.09, 0.07, 0.52, 20, scale);
        batches[gunmetal].add_cylinder_y([-0.76, 1.18, -0.10], 0.09, 0.07, 0.52, 20, scale);
        return true;
    }

    false
}

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
        batches[primary].add_tapered_box([0.0, 0.34, -0.14], [0.82, 0.66], [0.74, 0.58], 0.18, scale);
        add_corrugated_roof_and_cladding(batches, [0.0, 0.34, -0.14], [0.74, 0.18, 0.58], 12, scale);
        for (px, pz) in [(-0.46, 0.42), (0.46, 0.42), (-0.46, -0.12), (0.46, -0.12)] {
            batches[steel].add_cylinder_y([px, 0.54, pz], 0.28, 0.30, 0.05, 24, scale);
            batches[gunmetal].add_cylinder_y([px, 0.57, pz], 0.24, 0.24, 0.025, 24, scale);
            batches[emissive].add_cylinder_y([px, 0.60, pz], 0.16, 0.16, 0.015, 20, scale);
        }
        batches[primary].add_tapered_box([0.0, 0.92, -0.52], [0.28, 0.24], [0.22, 0.18], 0.44, scale);
        batches[emissive].add_box([0.0, 1.18, -0.32], [0.22, 0.09, 0.04], scale);
        batches[steel].add_pitched_cylinder_z([0.0, 1.48, -0.52], 0.34, 0.08, 0.14, 0.55, 28, scale);
        return true;
    }

    if has_token(name, "naradr") || has_token(name, "napsis") || name.contains("radar") || name.contains("psychic sensor") {
        // Soviet Radar Tower / Psychic Sensor: armored command bunker + giant parabolic dish
        batches[concrete].add_box([0.0, 0.10, 0.0], [0.90, 0.10, 0.90], scale);
        batches[primary].add_tapered_box([0.0, 0.46, 0.0], [0.76, 0.76], [0.62, 0.62], 0.28, scale);
        add_corrugated_roof_and_cladding(batches, [0.0, 0.46, 0.0], [0.62, 0.28, 0.62], 12, scale);
        batches[gunmetal].add_cylinder_y([0.0, 0.92, 0.0], 0.28, 0.20, 0.28, 24, scale);
        batches[steel].add_pitched_cylinder_z([0.0, 1.32, -0.10], 0.56, 0.14, 0.22, 0.45, 32, scale);
        batches[emissive].add_pitched_cylinder_z([0.0, 1.36, 0.05], 0.08, 0.03, 0.38, 0.45, 20, scale);
        return true;
    }

    if has_token(name, "gaspysat") || name.contains("spy satellite") {
        // Spy Satellite Uplink: telemetry center + giant skyward deep-space parabolic dish
        batches[concrete].add_box([0.0, 0.10, 0.0], [0.88, 0.10, 0.88], scale);
        batches[primary].add_tapered_box([-0.20, 0.42, 0.18], [0.58, 0.54], [0.50, 0.46], 0.24, scale);
        add_corrugated_roof_and_cladding(batches, [-0.20, 0.42, 0.18], [0.50, 0.24, 0.46], 10, scale);
        batches[steel].add_cylinder_y([0.24, 0.68, -0.20], 0.22, 0.16, 0.34, 24, scale);
        let dish_profile = [[0.14, 0.0], [0.28, 0.11], [0.44, 0.22], [0.56, 0.34]];
        batches[steel].add_spline_lathe_y([0.24, 0.96, -0.20], &dish_profile, 6, 32, scale);
        batches[emissive].add_cylinder_y([0.24, 1.38, -0.20], 0.06, 0.02, 0.24, 20, scale);
        return true;
    }

    if has_token(name, "gagap") || name.contains("gap generator") {
        // Allied Gap Generator: stealth-field obelisk with 4 concentric phase rings
        batches[concrete].add_cylinder_y([0.0, 0.14, 0.0], 0.72, 0.64, 0.14, 28, scale);
        let spire_profile = [[0.28, 0.0], [0.22, 0.36], [0.16, 0.74], [0.12, 1.12], [0.09, 1.48]];
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
        batches[primary].add_tapered_box([-0.24, 1.12, -0.18], [0.42, 0.42], [0.28, 0.28], 0.52, scale);
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
        batches[primary].add_tapered_box([0.0, 0.68, -0.68], [0.48, 0.22], [0.40, 0.18], 0.28, scale);
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
        batches[concrete].add_tapered_box([0.0, 0.08, 0.0], [0.92, 0.92], [0.86, 0.86], 0.08, scale);
        batches[steel].add_cylinder_y([0.0, 0.18, 0.0], 0.52, 0.52, 0.04, 32, scale);
        batches[primary].add_tapered_box([-0.68, 0.52, 0.0], [0.18, 0.58], [0.14, 0.48], 0.36, scale);
        batches[primary].add_tapered_box([0.68, 0.52, 0.0], [0.18, 0.58], [0.14, 0.48], 0.36, scale);
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
        batches[concrete].add_tapered_box([0.0, 0.16, 0.0], [0.94, 0.94], [0.86, 0.86], 0.16, scale);
        batches[dark].add_cylinder_y([0.0, 0.34, 0.0], 0.54, 0.54, 0.06, 32, scale);
        batches[primary].add_box([-0.62, 0.56, 0.0], [0.08, 0.28, 0.48], scale);
        batches[primary].add_box([0.62, 0.56, 0.0], [0.08, 0.28, 0.48], scale);
        let rocket_body = [[0.25, 0.0], [0.24, 0.32], [0.24, 0.64], [0.23, 0.96], [0.22, 1.28]];
        batches[steel].add_spline_lathe_y([0.0, 0.36, 0.0], &rocket_body, 6, 32, scale);
        let warhead_cone = [[0.22, 0.0], [0.19, 0.14], [0.14, 0.28], [0.07, 0.42], [0.015, 0.54]];
        batches[primary].add_spline_lathe_y([0.0, 1.64, 0.0], &warhead_cone, 6, 32, scale);
        batches[emissive].add_cylinder_y([0.0, 1.62, 0.0], 0.235, 0.235, 0.05, 32, scale);
        return true;
    }

    if has_token(name, "gaweat") || name.contains("weather") {
        // Allied Weather Control Device: central lightning spire + 3 angled atmospheric dish emitters
        batches[concrete].add_box([0.0, 0.12, 0.0], [0.94, 0.12, 0.94], scale);
        batches[primary].add_cylinder_y([0.0, 0.46, 0.0], 0.72, 0.58, 0.24, 32, scale);
        let rod_profile = [[0.22, 0.0], [0.16, 0.29], [0.11, 0.58], [0.07, 0.87], [0.05, 1.16]];
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
            batches[primary].add_tapered_box([cx, 0.96, cz], [0.14, 0.14], [0.08, 0.08], 0.42, scale);
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
            batches[primary].add_tapered_box([px, 0.82, pz], [0.16, 0.16], [0.08, 0.08], 0.44, scale);
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
        batches[steel].add_tapered_box([-0.24, 0.92, -0.18], [0.32, 0.32], [0.10, 0.10], 0.78, scale);
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
        batches[primary].add_tapered_box([0.48, 0.86, -0.38], [0.22, 0.22], [0.16, 0.16], 0.68, scale);
        batches[emissive].add_cylinder_y([0.48, 1.62, -0.38], 0.24, 0.28, 0.12, 24, scale);
        return true;
    }

    if has_token(name, "caoutp") || name.contains("outpost") {
        // Tech Outpost: forward repair deck + tall central defense missile turret
        batches[concrete].add_box([0.0, 0.12, 0.0], [0.92, 0.12, 0.92], scale);
        batches[primary].add_tapered_box([0.0, 0.48, -0.24], [0.68, 0.44], [0.58, 0.36], 0.28, scale);
        add_corrugated_roof_and_cladding(batches, [0.0, 0.48, -0.24], [0.58, 0.28, 0.36], 10, scale);
        batches[steel].add_cylinder_y([0.0, 1.02, -0.24], 0.26, 0.20, 0.32, 24, scale);
        batches[gunmetal].add_pitched_cylinder_z([0.0, 1.38, -0.24], 0.18, 0.16, 0.48, 0.35, 20, scale);
        return true;
    }

    if has_token(name, "gawall") || has_token(name, "nawall") || name.contains("wall") {
        // Concrete defense wall with armored buttresses
        batches[concrete].add_tapered_box([0.0, 0.42, 0.0], [0.46, 0.46], [0.36, 0.36], 0.42, scale);
        batches[primary].add_box([0.0, 0.88, 0.0], [0.40, 0.06, 0.40], scale);
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
        batches[primary].add_pitched_cylinder_z([0.0, 0.68, -0.14], 0.28, 0.28, 0.62, 0.48, 4, scale);
        batches[emissive].add_pitched_cylinder_z([0.0, 0.94, 0.36], 0.18, 0.18, 0.06, 0.48, 4, scale);
        return true;
    }

    if has_token(name, "naflak") || name.contains("flak cannon") {
        // Soviet Flak Cannon: heavy AA mount + quad elevated 88mm flak barrels
        batches[concrete].add_cylinder_y([0.0, 0.16, 0.0], 0.58, 0.48, 0.16, 24, scale);
        batches[primary].add_tapered_box([0.0, 0.48, -0.06], [0.34, 0.34], [0.26, 0.26], 0.20, scale);
        for (bx, by) in [(-0.12, 0.56), (0.12, 0.56), (-0.12, 0.68), (0.12, 0.68)] {
            batches[gunmetal].add_pitched_cylinder_z([bx, by, 0.0], 0.045, 0.04, 0.68, 0.58, 16, scale);
        }
        return true;
    }

    if has_token(name, "gtgcan") || name.contains("grand cannon") {
        // French Grand Cannon: massive coastal casemate turret + oversized long-range artillery barrel
        batches[concrete].add_cylinder_y([0.0, 0.22, 0.0], 0.82, 0.70, 0.22, 32, scale);
        batches[primary].add_tapered_box([0.0, 0.64, -0.12], [0.58, 0.62], [0.42, 0.46], 0.26, scale);
        add_rifled_cannon_barrel(batches, [0.0, 0.68, 0.12], 0.095, 1.15, scale);
        batches[steel].add_pitched_cylinder_z([-0.18, 0.74, 0.06], 0.06, 0.06, 0.52, 0.18, 18, scale);
        batches[steel].add_pitched_cylinder_z([0.18, 0.74, 0.06], 0.06, 0.06, 0.52, 0.18, 18, scale);
        return true;
    }

    if has_token(name, "gapill") || has_token(name, "nalasr") || archetype == VisualArchetype::Turret {
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

