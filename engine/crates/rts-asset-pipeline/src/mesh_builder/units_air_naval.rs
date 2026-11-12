use super::buildings::has_token;
use super::components::*;
use super::primitives::{GeometryBatch, MatSlot};
use rts_sdk::schema::VisualArchetype;

/// Step 2A: Populates geometry for aircraft & helicopter units (Kirov Airship, Night Hawk, Siege Chopper, Harrier, Black Eagle).
pub(super) fn try_populate_aircraft_units(
    name: &str,
    archetype: VisualArchetype,
    scale: [f32; 3],
    batches: &mut [GeometryBatch; 6],
) -> bool {
    let primary = MatSlot::Primary as usize;
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;
    let dark = MatSlot::DarkAccent as usize;

    if has_token(name, "zep") || name.contains("kirov") {
        // Kirov Airship: spline-lathed streamlined cigar envelope + structural ribs + bomb gondola + propellers
        let envelope_profile = [
            [0.03, -0.86],
            [0.24, -0.58],
            [0.38, -0.28],
            [0.42, 0.0],
            [0.40, 0.30],
            [0.32, 0.62],
            [0.05, 0.86],
        ];
        batches[primary].add_spline_lathe_z([0.0, 0.68, 0.0], &envelope_profile, 6, 32, scale);
        for rz in [-0.36, -0.06, 0.24, 0.50] {
            batches[steel].add_cylinder_z([0.0, 0.68, rz], 0.415, 0.025, 32, scale);
        }
        batches[gunmetal].add_tapered_box(
            [0.0, 0.22, 0.04],
            [0.16, 0.42],
            [0.20, 0.48],
            0.12,
            scale,
        );
        batches[emissive].add_box([0.0, 0.24, 0.50], [0.14, 0.05, 0.04], scale);
        batches[primary].add_box([0.0, 0.94, -0.58], [0.035, 0.26, 0.20], scale);
        batches[primary].add_box([0.0, 0.68, -0.58], [0.48, 0.035, 0.18], scale);
        for sx in [-0.44, 0.44] {
            batches[steel].add_cylinder_z([sx, 0.52, -0.10], 0.07, 0.16, 20, scale);
            batches[dark].add_box([sx, 0.52, -0.27], [0.16, 0.02, 0.015], scale);
            batches[dark].add_box([sx, 0.52, -0.27], [0.02, 0.16, 0.015], scale);
        }
        return true;
    }

    if has_token(name, "shad")
        || has_token(name, "hind")
        || name.contains("night hawk")
        || name.contains("chopper")
    {
        // Night Hawk Transport / Siege Chopper: spline-lathed fuselage + twin turboshaft nacelles + skids + 4-blade rotor
        let heli_body = [
            [0.05, -0.78],
            [0.10, -0.46],
            [0.24, -0.14],
            [0.26, 0.16],
            [0.20, 0.42],
            [0.05, 0.58],
        ];
        batches[primary].add_spline_lathe_z([0.0, 0.38, 0.0], &heli_body, 6, 28, scale);
        batches[emissive].add_tapered_box(
            [0.0, 0.44, 0.36],
            [0.17, 0.18],
            [0.11, 0.12],
            0.10,
            scale,
        );
        for nx in [-0.15, 0.15] {
            batches[steel].add_cylinder_z([nx, 0.56, 0.04], 0.075, 0.22, 20, scale);
        }
        batches[steel].add_box([0.0, 0.52, -0.78], [0.03, 0.18, 0.10], scale);
        batches[gunmetal].add_box([-0.24, 0.12, 0.06], [0.03, 0.02, 0.46], scale);
        batches[gunmetal].add_box([0.24, 0.12, 0.06], [0.03, 0.02, 0.46], scale);
        batches[steel].add_cylinder_y([0.0, 0.64, 0.06], 0.065, 0.045, 0.10, 24, scale);
        batches[dark].add_box([0.0, 0.74, 0.06], [0.86, 0.015, 0.045], scale);
        batches[dark].add_box([0.0, 0.74, 0.06], [0.045, 0.015, 0.86], scale);
        return true;
    }

    if (has_token(name, "beag")
        || has_token(name, "orca")
        || archetype == VisualArchetype::Aircraft)
        && !has_token(name, "jumpjet")
        && !name.contains("rocketeer")
    {
        // Harrier / Black Eagle Strike Jet: spline-lathed radome + fuselage + swept wings + underwing missiles + twin tails
        let forward_swept = has_token(name, "beag") || name.contains("black eagle");
        let jet_profile = [
            [0.09, -0.62],
            [0.15, -0.34],
            [0.17, 0.0],
            [0.15, 0.28],
            [0.11, 0.52],
            [0.02, 0.74],
        ];
        batches[primary].add_spline_lathe_z([0.0, 0.36, 0.0], &jet_profile, 6, 28, scale);
        batches[emissive].add_tapered_box(
            [0.0, 0.49, 0.26],
            [0.10, 0.22],
            [0.06, 0.14],
            0.07,
            scale,
        );
        let wing_z = if forward_swept { 0.04 } else { -0.08 };
        batches[primary].add_tapered_box(
            [0.0, 0.36, wing_z],
            [0.76, 0.22],
            [0.70, 0.16],
            0.03,
            scale,
        );
        for mx in [-0.46, -0.28, 0.28, 0.46] {
            batches[steel].add_cylinder_z([mx, 0.28, -0.04], 0.035, 0.26, 16, scale);
        }
        batches[steel].add_box([-0.15, 0.54, -0.48], [0.03, 0.16, 0.14], scale);
        batches[steel].add_box([0.15, 0.54, -0.48], [0.03, 0.16, 0.14], scale);
        batches[gunmetal].add_cylinder_z([-0.10, 0.34, -0.62], 0.065, 0.12, 20, scale);
        batches[gunmetal].add_cylinder_z([0.10, 0.34, -0.62], 0.065, 0.12, 20, scale);
        batches[emissive].add_cylinder_z([-0.10, 0.34, -0.74], 0.045, 0.02, 20, scale);
        batches[emissive].add_cylinder_z([0.10, 0.34, -0.74], 0.045, 0.02, 20, scale);
        return true;
    }

    false
}

/// Step 2B: Populates geometry for naval units (Aircraft Carrier, Typhoon Sub, Dolphin, Giant Squid, Hovercraft, Aegis, Destroyer).
pub(super) fn try_populate_naval_units(
    name: &str,
    archetype: VisualArchetype,
    scale: [f32; 3],
    batches: &mut [GeometryBatch; 6],
) -> bool {
    let primary = MatSlot::Primary as usize;
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;
    let dark = MatSlot::DarkAccent as usize;

    if has_token(name, "carrier") || name.contains("aircraft carrier") {
        // Allied Aircraft Carrier: wide flight deck + starboard island tower + 3 Hornet jets on deck
        batches[gunmetal].add_tapered_box(
            [0.0, 0.20, 0.0],
            [0.38, 0.84],
            [0.52, 0.94],
            0.18,
            scale,
        );
        batches[dark].add_box([0.0, 0.40, 0.0], [0.54, 0.03, 0.94], scale);
        batches[emissive].add_box([-0.12, 0.435, 0.0], [0.03, 0.008, 0.86], scale);
        batches[primary].add_tapered_box(
            [0.36, 0.64, -0.18],
            [0.12, 0.26],
            [0.09, 0.20],
            0.22,
            scale,
        );
        batches[steel].add_cylinder_y([0.36, 0.98, -0.18], 0.04, 0.02, 0.14, 16, scale);
        for jz in [-0.46, -0.06, 0.34] {
            batches[primary].add_box([-0.14, 0.48, jz], [0.16, 0.04, 0.18], scale);
            batches[emissive].add_box([-0.14, 0.53, jz + 0.06], [0.05, 0.03, 0.06], scale);
        }
        return true;
    }

    if name.contains("sub_apc") || name.contains("sub apc") || has_token(name, "sapc") {
        // Soviet Sub-APC: armored amphibious/submersible troop carrier with caterpillar hydro-tracks + bow ramp + conning hatch
        batches[gunmetal].add_tapered_box(
            [-0.38, 0.14, 0.0],
            [0.10, 0.66],
            [0.08, 0.60],
            0.12,
            scale,
        );
        batches[gunmetal].add_tapered_box(
            [0.38, 0.14, 0.0],
            [0.10, 0.66],
            [0.08, 0.60],
            0.12,
            scale,
        );
        batches[primary].add_tapered_box([0.0, 0.28, 0.0], [0.34, 0.68], [0.28, 0.58], 0.18, scale);
        batches[steel].add_box([0.0, 0.28, 0.64], [0.24, 0.14, 0.05], scale);
        batches[primary].add_dome_y([0.0, 0.46, -0.10], 0.18, 0.16, 8, 20, scale);
        batches[emissive].add_box([0.0, 0.40, 0.48], [0.16, 0.045, 0.03], scale);
        batches[steel].add_cylinder_z([-0.18, 0.20, -0.68], 0.08, 0.06, 18, scale);
        batches[steel].add_cylinder_z([0.18, 0.20, -0.68], 0.08, 0.06, 18, scale);
        return true;
    }

    if has_token(name, "sub") || name.contains("typhoon") || name.contains("submarine") {
        // Soviet Typhoon Attack Sub: spline-lathed teardrop hull + conning tower sail + dive planes + bow tubes
        let sub_hull = [
            [0.03, -0.84],
            [0.18, -0.56],
            [0.28, -0.24],
            [0.30, 0.06],
            [0.29, 0.36],
            [0.22, 0.64],
            [0.05, 0.84],
        ];
        batches[dark].add_spline_lathe_z([0.0, 0.24, 0.0], &sub_hull, 6, 28, scale);
        batches[primary].add_tapered_box(
            [0.0, 0.54, 0.10],
            [0.12, 0.26],
            [0.09, 0.20],
            0.20,
            scale,
        );
        batches[steel].add_box([0.0, 0.56, 0.14], [0.24, 0.02, 0.08], scale);
        batches[steel].add_cylinder_y([0.0, 0.82, 0.12], 0.025, 0.02, 0.10, 16, scale);
        batches[emissive].add_cylinder_z([-0.08, 0.24, 0.79], 0.04, 0.03, 16, scale);
        batches[emissive].add_cylinder_z([0.08, 0.24, 0.79], 0.04, 0.03, 16, scale);
        return true;
    }

    if has_token(name, "dlph") || name.contains("dolphin") {
        // Allied Dolphin: spline-lathed marine mammal body + rostrum + pectoral flippers + dorsal fin + tail flukes + sonic harness
        let dlph_body = [
            [0.02, -0.62],
            [0.09, -0.38],
            [0.19, -0.10],
            [0.22, 0.16],
            [0.16, 0.40],
            [0.07, 0.56],
            [0.03, 0.68],
        ];
        batches[primary].add_spline_lathe_z([0.0, 0.26, 0.0], &dlph_body, 6, 28, scale);
        // Dorsal fin
        batches[primary].add_tapered_box(
            [0.0, 0.46, -0.04],
            [0.03, 0.15],
            [0.012, 0.06],
            0.15,
            scale,
        );
        // Pectoral flippers
        batches[primary].add_tapered_box(
            [-0.22, 0.18, 0.16],
            [0.12, 0.08],
            [0.04, 0.04],
            0.025,
            scale,
        );
        batches[primary].add_tapered_box(
            [0.22, 0.18, 0.16],
            [0.12, 0.08],
            [0.04, 0.04],
            0.025,
            scale,
        );
        // Horizontal tail flukes
        batches[primary].add_tapered_box(
            [0.0, 0.25, -0.60],
            [0.28, 0.09],
            [0.14, 0.04],
            0.022,
            scale,
        );
        // Allied Sonic Amplifier Harness & glowing sonar emitter dome
        batches[steel].add_cylinder_z([0.0, 0.27, 0.20], 0.225, 0.065, 24, scale);
        batches[gunmetal].add_box([0.0, 0.42, 0.20], [0.08, 0.06, 0.08], scale);
        batches[emissive].add_cylinder_z([0.0, 0.36, 0.44], 0.065, 0.10, 20, scale);
        batches[emissive].add_dome_y([0.0, 0.36, 0.54], 0.055, 0.055, 6, 16, scale);
        return true;
    }

    if has_token(name, "sqd") || name.contains("squid") {
        // Soviet Giant Squid: spline-lathed cephalopod mantle + lateral tail fins + glowing eyes + 8 articulated forward tentacles
        let squid_mantle = [
            [0.02, -0.68],
            [0.16, -0.48],
            [0.24, -0.22],
            [0.22, 0.04],
            [0.18, 0.22],
        ];
        batches[primary].add_spline_lathe_z([0.0, 0.28, 0.0], &squid_mantle, 6, 28, scale);
        // Arrowhead tail fins
        batches[primary].add_tapered_box(
            [0.0, 0.28, -0.48],
            [0.34, 0.16],
            [0.12, 0.08],
            0.03,
            scale,
        );
        // Mind-control / bio-collar ring & glowing eyes
        batches[steel].add_cylinder_z([0.0, 0.28, 0.08], 0.205, 0.04, 24, scale);
        batches[emissive].add_dome_y([-0.16, 0.30, 0.14], 0.048, 0.048, 6, 14, scale);
        batches[emissive].add_dome_y([0.16, 0.30, 0.14], 0.048, 0.048, 6, 14, scale);
        // 6 grasping arms + 2 long strike tentacles
        for (tx, ty, len) in [
            (-0.20, 0.18, 0.44),
            (-0.12, 0.26, 0.36),
            (-0.04, 0.16, 0.38),
            (0.04, 0.16, 0.38),
            (0.12, 0.26, 0.36),
            (0.20, 0.18, 0.44),
        ] {
            batches[primary].add_pitched_cylinder_z(
                [tx, ty, 0.20],
                0.042,
                0.018,
                len,
                -0.04,
                14,
                scale,
            );
        }
        return true;
    }

    if name.contains("zubr") {
        // Soviet Zubr Heavy Assault Hovercraft: massive air-cushion skirt + raised armored bridge + 3 rear ducted propulsion pylons
        batches[dark].add_tapered_box([0.0, 0.12, 0.0], [0.54, 0.78], [0.50, 0.74], 0.11, scale);
        batches[primary].add_tapered_box(
            [0.0, 0.34, 0.02],
            [0.46, 0.66],
            [0.42, 0.60],
            0.14,
            scale,
        );
        batches[steel].add_box([0.0, 0.34, 0.68], [0.36, 0.15, 0.05], scale);
        batches[primary].add_tapered_box(
            [0.0, 0.56, 0.10],
            [0.22, 0.22],
            [0.16, 0.16],
            0.12,
            scale,
        );
        batches[emissive].add_box([0.0, 0.58, 0.27], [0.15, 0.05, 0.03], scale);
        for px in [-0.28, 0.0, 0.28] {
            batches[steel].add_cylinder_z([px, 0.60, -0.54], 0.12, 0.09, 22, scale);
            batches[dark].add_cylinder_z([px, 0.60, -0.54], 0.09, 0.10, 18, scale);
        }
        return true;
    }

    if name.contains("yuri_hover") || (name.contains("hover") && name.contains("yuri")) {
        // Yuri Psionic Hover Transport: curved hovering disc skirt + streamlined purple troop deck + glowing psionic beacon dome
        batches[dark].add_cylinder_y([0.0, 0.12, 0.0], 0.56, 0.50, 0.10, 28, scale);
        batches[primary].add_tapered_box([0.0, 0.30, 0.0], [0.42, 0.60], [0.34, 0.50], 0.13, scale);
        batches[emissive].add_dome_y([0.0, 0.43, 0.08], 0.20, 0.18, 8, 24, scale);
        batches[steel].add_cylinder_y([0.0, 0.44, 0.08], 0.24, 0.24, 0.025, 24, scale);
        batches[steel].add_cylinder_z([-0.24, 0.46, -0.46], 0.11, 0.10, 20, scale);
        batches[steel].add_cylinder_z([0.24, 0.46, -0.46], 0.11, 0.10, 20, scale);
        return true;
    }

    if has_token(name, "lcrf")
        || has_token(name, "lcv")
        || name.contains("landing craft")
        || name.contains("hover")
        || name.contains("amphibious")
    {
        // Allied Landing Craft Hovercraft: rubber cushion skirt + bow ramp + twin rear ducted fans
        batches[dark].add_tapered_box([0.0, 0.12, 0.0], [0.48, 0.68], [0.44, 0.64], 0.10, scale);
        batches[primary].add_tapered_box(
            [0.0, 0.34, -0.04],
            [0.40, 0.56],
            [0.36, 0.50],
            0.14,
            scale,
        );
        batches[steel].add_box([0.0, 0.34, 0.56], [0.32, 0.14, 0.05], scale);
        batches[emissive].add_box([0.0, 0.48, -0.18], [0.18, 0.06, 0.04], scale);
        batches[steel].add_cylinder_z([-0.22, 0.58, -0.46], 0.14, 0.08, 24, scale);
        batches[steel].add_cylinder_z([0.22, 0.58, -0.46], 0.14, 0.08, 24, scale);
        return true;
    }

    if has_token(name, "aegis") || name.contains("aegis") {
        // Allied Aegis Cruiser: warship hull + SPY phased-array radar superstructure + twin VLS SAM batteries
        batches[primary].add_tapered_box([0.0, 0.20, 0.0], [0.34, 0.78], [0.42, 0.88], 0.18, scale);
        batches[steel].add_tapered_box([0.0, 0.56, -0.04], [0.28, 0.36], [0.20, 0.26], 0.22, scale);
        batches[emissive].add_box([0.0, 0.62, 0.24], [0.18, 0.08, 0.03], scale);
        batches[gunmetal].add_pitched_cylinder_z(
            [0.0, 0.42, 0.52],
            0.14,
            0.14,
            0.26,
            0.65,
            16,
            scale,
        );
        batches[gunmetal].add_pitched_cylinder_z(
            [0.0, 0.42, -0.56],
            0.14,
            0.14,
            0.26,
            0.65,
            16,
            scale,
        );
        return true;
    }

    if archetype == VisualArchetype::Ship
        || has_token(name, "dest")
        || has_token(name, "cdest")
        || has_token(name, "hyd")
    {
        // Destroyer / Sea Scorpion: sleek warship hull + bridge + radar mast + fore/aft naval guns
        batches[primary].add_tapered_box([0.0, 0.20, 0.0], [0.34, 0.74], [0.42, 0.84], 0.18, scale);
        batches[steel].add_tapered_box([0.0, 0.52, -0.08], [0.24, 0.32], [0.18, 0.22], 0.18, scale);
        batches[emissive].add_box([0.0, 0.58, 0.16], [0.16, 0.05, 0.04], scale);
        batches[gunmetal].add_cylinder_y([0.0, 0.84, -0.12], 0.05, 0.02, 0.18, 16, scale);
        add_rifled_cannon_barrel(batches, [0.0, 0.44, 0.36], 0.055, 0.42, scale);
        batches[gunmetal].add_pitched_cylinder_z(
            [0.0, 0.44, -0.42],
            0.05,
            0.04,
            0.36,
            0.25,
            16,
            scale,
        );
        return true;
    }

    false
}
