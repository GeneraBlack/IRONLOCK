use super::buildings::*;
use super::components::*;
use super::infantry::populate_infantry_units;
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
        batches[gunmetal].add_tapered_box([0.0, 0.22, 0.04], [0.16, 0.42], [0.20, 0.48], 0.12, scale);
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

    if has_token(name, "shad") || has_token(name, "hind") || name.contains("night hawk") || name.contains("chopper") {
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
        batches[emissive].add_tapered_box([0.0, 0.44, 0.36], [0.17, 0.18], [0.11, 0.12], 0.10, scale);
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

    if (has_token(name, "beag") || has_token(name, "orca") || archetype == VisualArchetype::Aircraft)
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
        batches[emissive].add_tapered_box([0.0, 0.49, 0.26], [0.10, 0.22], [0.06, 0.14], 0.07, scale);
        let wing_z = if forward_swept { 0.04 } else { -0.08 };
        batches[primary].add_tapered_box([0.0, 0.36, wing_z], [0.76, 0.22], [0.70, 0.16], 0.03, scale);
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
        batches[gunmetal].add_tapered_box([0.0, 0.20, 0.0], [0.38, 0.84], [0.52, 0.94], 0.18, scale);
        batches[dark].add_box([0.0, 0.40, 0.0], [0.54, 0.03, 0.94], scale);
        batches[emissive].add_box([-0.12, 0.435, 0.0], [0.03, 0.008, 0.86], scale);
        batches[primary].add_tapered_box([0.36, 0.64, -0.18], [0.12, 0.26], [0.09, 0.20], 0.22, scale);
        batches[steel].add_cylinder_y([0.36, 0.98, -0.18], 0.04, 0.02, 0.14, 16, scale);
        for jz in [-0.46, -0.06, 0.34] {
            batches[primary].add_box([-0.14, 0.48, jz], [0.16, 0.04, 0.18], scale);
            batches[emissive].add_box([-0.14, 0.53, jz + 0.06], [0.05, 0.03, 0.06], scale);
        }
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
        batches[primary].add_tapered_box([0.0, 0.54, 0.10], [0.12, 0.26], [0.09, 0.20], 0.20, scale);
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
        batches[primary].add_tapered_box([0.0, 0.46, -0.04], [0.03, 0.15], [0.012, 0.06], 0.15, scale);
        // Pectoral flippers
        batches[primary].add_tapered_box([-0.22, 0.18, 0.16], [0.12, 0.08], [0.04, 0.04], 0.025, scale);
        batches[primary].add_tapered_box([0.22, 0.18, 0.16], [0.12, 0.08], [0.04, 0.04], 0.025, scale);
        // Horizontal tail flukes
        batches[primary].add_tapered_box([0.0, 0.25, -0.60], [0.28, 0.09], [0.14, 0.04], 0.022, scale);
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
        batches[primary].add_tapered_box([0.0, 0.28, -0.48], [0.34, 0.16], [0.12, 0.08], 0.03, scale);
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
            batches[primary].add_pitched_cylinder_z([tx, ty, 0.20], 0.042, 0.018, len, -0.04, 14, scale);
        }
        return true;
    }

    if has_token(name, "lcrf") || has_token(name, "sapc") || name.contains("amphibious") {
        // Amphibious Hovercraft Transport: rubber cushion skirt + bow ramp + twin rear ducted fans
        batches[dark].add_tapered_box([0.0, 0.12, 0.0], [0.48, 0.68], [0.44, 0.64], 0.10, scale);
        batches[primary].add_tapered_box([0.0, 0.34, -0.04], [0.40, 0.56], [0.36, 0.50], 0.14, scale);
        batches[steel].add_box([0.0, 0.34, 0.56], [0.32, 0.14, 0.05], scale);
        batches[steel].add_cylinder_z([-0.22, 0.58, -0.46], 0.14, 0.08, 24, scale);
        batches[steel].add_cylinder_z([0.22, 0.58, -0.46], 0.14, 0.08, 24, scale);
        return true;
    }

    if has_token(name, "aegis") || name.contains("aegis") {
        // Allied Aegis Cruiser: warship hull + SPY phased-array radar superstructure + twin VLS SAM batteries
        batches[primary].add_tapered_box([0.0, 0.20, 0.0], [0.34, 0.78], [0.42, 0.88], 0.18, scale);
        batches[steel].add_tapered_box([0.0, 0.56, -0.04], [0.28, 0.36], [0.20, 0.26], 0.22, scale);
        batches[emissive].add_box([0.0, 0.62, 0.24], [0.18, 0.08, 0.03], scale);
        batches[gunmetal].add_pitched_cylinder_z([0.0, 0.42, 0.52], 0.14, 0.14, 0.26, 0.65, 16, scale);
        batches[gunmetal].add_pitched_cylinder_z([0.0, 0.42, -0.56], 0.14, 0.14, 0.26, 0.65, 16, scale);
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
        batches[gunmetal].add_pitched_cylinder_z([0.0, 0.44, -0.42], 0.05, 0.04, 0.36, 0.25, 16, scale);
        return true;
    }

    false
}

/// Step 2C: Populates geometry for specialized ground vehicles, harvesters, and battle tanks.
pub(super) fn try_populate_vehicle_and_tank_units(
    name: &str,
    archetype: VisualArchetype,
    scale: [f32; 3],
    batches: &mut [GeometryBatch; 6],
) -> bool {
    let primary = MatSlot::Primary as usize;
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;

    if (has_token(name, "dron")
        || has_token(name, "drone")
        || name.contains("terror drone")
        || name.contains("terror_drone"))
        && !name.contains("chaos")
    {
        // Soviet Terror Drone (`dron.shp` — Row 14 of sheet 2: crimson forward head/carapace, silver dorsal dome, 4 high-arched articulated silver blade-legs)
        batches[steel].add_ellipsoid([0.0, 0.36, -0.04], [0.16, 0.11, 0.18], 10, 20, scale);
        batches[primary].add_ellipsoid([0.0, 0.34, 0.10], [0.15, 0.10, 0.16], 10, 20, scale);
        batches[emissive].add_ellipsoid([0.0, 0.34, 0.25], [0.055, 0.032, 0.035], 8, 14, scale);
        // Forward dismantling jaws
        batches[steel].add_oriented_beam([-0.08, 0.29, 0.22], [-0.04, 0.27, 0.38], 0.022, 0.028, 12, scale);
        batches[steel].add_oriented_beam([0.08, 0.29, 0.22], [0.04, 0.27, 0.38], 0.022, 0.028, 12, scale);
        // 4 high-arched articulated silver spider scythe-legs (`hip -> high knee -> blade tip on y = 0.02`)
        for (sx, sz) in [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            let hip = [sx * 0.14, 0.35, sz * 0.08];
            let knee = [sx * 0.28, 0.48, sz * 0.18];
            let ankle = [sx * 0.32, 0.20, sz * 0.24];
            let tip = [sx * 0.28, 0.02, sz * 0.26];
            batches[gunmetal].add_limb_capsule(hip, knee, 0.036, 0.042, 0.10, 14, scale);
            batches[steel].add_limb_capsule(knee, ankle, 0.046, 0.034, 0.14, 14, scale);
            batches[steel].add_limb_capsule(ankle, tip, 0.032, 0.012, 0.05, 14, scale);
        }
        return true;
    }

    if has_token(name, "amcv") || has_token(name, "smcv") || name.contains("mobile construction") {
        // Mobile Construction Vehicle (MCV): heavy 6-wheel mobile base rig + cab + folded crane boom
        add_wheeled_axles(batches, 0.44, &[-0.46, 0.0, 0.46], 0.18, 0.10, scale);
        batches[gunmetal].add_box([0.0, 0.24, 0.0], [0.38, 0.08, 0.66], scale);
        batches[primary].add_tapered_box([0.0, 0.52, 0.42], [0.38, 0.22], [0.32, 0.18], 0.22, scale);
        batches[emissive].add_box([0.0, 0.58, 0.62], [0.28, 0.08, 0.03], scale);
        batches[primary].add_tapered_box([0.0, 0.56, -0.18], [0.40, 0.38], [0.34, 0.34], 0.26, scale);
        batches[steel].add_pitched_cylinder_z([0.0, 0.86, -0.36], 0.08, 0.06, 0.78, 0.18, 20, scale);
        return true;
    }

    if has_token(name, "cmin") || name.contains("chrono miner") {
        // Allied Chrono Miner: wheeled ore truck + front mining scoop + glowing blue Chrono-core sphere
        add_wheeled_axles(batches, 0.44, &[-0.42, 0.36], 0.18, 0.11, scale);
        batches[primary].add_tapered_box([0.0, 0.44, 0.36], [0.36, 0.22], [0.30, 0.18], 0.20, scale);
        batches[emissive].add_dome_y([0.0, 0.38, -0.18], 0.34, 0.42, 10, 28, scale);
        batches[steel].add_cylinder_y([0.0, 0.52, -0.18], 0.38, 0.38, 0.04, 28, scale);
        batches[gunmetal].add_cylinder_x([0.0, 0.18, 0.68], 0.14, 0.42, 24, scale);
        return true;
    }

    if archetype == VisualArchetype::Harvester || has_token(name, "harv") || has_token(name, "sharv") {
        // Soviet War Miner: heavy tracked chassis + sloped ore hopper + front rotary auger + roof 20mm gun
        add_tank_tracks(batches, 0.44, 0.0, 0.58, 0.36, 0.11, 5, scale);
        batches[primary].add_tapered_box([0.0, 0.48, -0.12], [0.36, 0.42], [0.42, 0.46], 0.28, scale);
        batches[emissive].add_tapered_box([0.0, 0.78, -0.12], [0.32, 0.36], [0.20, 0.22], 0.08, scale);
        batches[steel].add_tapered_box([0.0, 0.42, 0.44], [0.34, 0.18], [0.28, 0.14], 0.20, scale);
        batches[gunmetal].add_cylinder_x([0.0, 0.18, 0.68], 0.14, 0.42, 24, scale);
        add_rifled_cannon_barrel(batches, [0.0, 0.72, 0.32], 0.042, 0.42, scale);
        return true;
    }

    if has_token(name, "sref") || name.contains("prism") {
        // Allied Prism Tank: tracked chassis + rear capacitor bay + tall multi-ringed prism emitter tower
        add_tank_tracks(batches, 0.42, 0.0, 0.54, 0.34, 0.11, 5, scale);
        batches[primary].add_tapered_box([0.0, 0.28, 0.0], [0.34, 0.52], [0.30, 0.46], 0.14, scale);
        batches[steel].add_cylinder_y([0.0, 0.58, -0.06], 0.22, 0.14, 0.22, 24, scale);
        for ry in [0.68, 0.86, 1.02] {
            batches[steel].add_cylinder_y([0.0, ry, -0.06], 0.22, 0.22, 0.025, 24, scale);
        }
        batches[emissive].add_cylinder_y([0.0, 0.96, -0.06], 0.12, 0.04, 0.26, 20, scale);
        batches[emissive].add_cylinder_z([0.0, 0.98, 0.14], 0.09, 0.18, 20, scale);
        return true;
    }

    if has_token(name, "apoc") || name.contains("apocalypse") {
        // Soviet Apocalypse Tank: massive 4-track chassis + twin 120mm rifled barrels + Mammoth Tusk SAM pods + cupola
        add_tank_tracks(batches, 0.46, 0.0, 0.62, 0.38, 0.13, 6, scale);
        batches[primary].add_tapered_box([0.0, 0.32, 0.0], [0.38, 0.58], [0.34, 0.50], 0.16, scale);
        batches[primary].add_tapered_box([0.0, 0.58, -0.06], [0.32, 0.34], [0.24, 0.26], 0.16, scale);
        add_rifled_cannon_barrel(batches, [-0.11, 0.58, 0.22], 0.055, 0.72, scale);
        add_rifled_cannon_barrel(batches, [0.11, 0.58, 0.22], 0.055, 0.72, scale);
        add_turret_cupola_and_details(batches, [0.0, 0.58, -0.06], 0.24, 0.74, scale);
        batches[steel].add_pitched_cylinder_z([-0.32, 0.64, -0.16], 0.07, 0.07, 0.32, 0.32, 16, scale);
        batches[steel].add_pitched_cylinder_z([0.32, 0.64, -0.16], 0.07, 0.07, 0.32, 0.32, 16, scale);
        return true;
    }

    if has_token(name, "ttnk") || name.contains("tesla tank") {
        // Soviet Tesla Tank: tracked hull + twin spherical Tesla coil emitters
        add_tank_tracks(batches, 0.42, 0.0, 0.54, 0.34, 0.11, 5, scale);
        batches[primary].add_tapered_box([0.0, 0.30, 0.0], [0.34, 0.52], [0.30, 0.44], 0.15, scale);
        batches[steel].add_tapered_box([0.0, 0.52, -0.08], [0.26, 0.28], [0.20, 0.22], 0.12, scale);
        for tx in [-0.14, 0.14] {
            batches[steel].add_cylinder_z([tx, 0.58, 0.22], 0.07, 0.24, 20, scale);
            batches[emissive].add_dome_y([tx, 0.54, 0.48], 0.10, 0.12, 8, 20, scale);
        }
        return true;
    }

    if has_token(name, "mgtk") || name.contains("mirage") {
        // Allied Mirage Tank: sleek experimental hull + faceted holographic dish turret + thermal barrel
        add_tank_tracks(batches, 0.42, 0.0, 0.52, 0.32, 0.10, 5, scale);
        batches[primary].add_tapered_box([0.0, 0.28, 0.0], [0.36, 0.52], [0.28, 0.42], 0.14, scale);
        batches[steel].add_cylinder_y([0.0, 0.52, -0.04], 0.28, 0.18, 0.12, 16, scale);
        batches[emissive].add_cylinder_z([0.0, 0.54, 0.36], 0.07, 0.28, 16, scale);
        return true;
    }

    if has_token(name, "tnkd") || name.contains("tank destroyer") {
        // German Tank Destroyer: low-profile casemate jagdpanzer + long fixed anti-tank cannon
        add_tank_tracks(batches, 0.43, 0.0, 0.56, 0.32, 0.11, 5, scale);
        batches[primary].add_tapered_box([0.0, 0.36, -0.04], [0.38, 0.54], [0.26, 0.36], 0.20, scale);
        add_rifled_cannon_barrel(batches, [0.0, 0.40, 0.20], 0.058, 0.74, scale);
        add_turret_cupola_and_details(batches, [0.0, 0.36, -0.08], 0.26, 0.56, scale);
        return true;
    }

    if has_token(name, "fv") || name.contains("fighting vehicle") {
        // Allied IFV: 6-wheel fast chassis + rotatable 6-tube missile turret pod
        add_wheeled_axles(batches, 0.42, &[-0.42, 0.0, 0.42], 0.16, 0.09, scale);
        batches[primary].add_tapered_box([0.0, 0.32, 0.0], [0.34, 0.54], [0.28, 0.46], 0.16, scale);
        batches[steel].add_cylinder_y([0.0, 0.54, -0.08], 0.16, 0.14, 0.10, 20, scale);
        batches[primary].add_pitched_cylinder_z([0.0, 0.68, -0.12], 0.18, 0.18, 0.38, 0.25, 16, scale);
        batches[emissive].add_box([0.0, 0.36, 0.48], [0.22, 0.05, 0.03], scale);
        return true;
    }

    if has_token(name, "htk") || name.contains("flak track") {
        // Soviet Flak Track: front steering wheels + rear armored tracks + twin AA flak mount
        add_wheeled_axles(batches, 0.40, &[0.42], 0.16, 0.08, scale);
        add_tank_tracks(batches, 0.40, -0.22, 0.32, 0.30, 0.09, 3, scale);
        batches[primary].add_tapered_box([0.0, 0.32, 0.24], [0.32, 0.28], [0.28, 0.22], 0.16, scale);
        batches[primary].add_box([0.0, 0.26, -0.24], [0.34, 0.10, 0.32], scale);
        batches[gunmetal].add_pitched_cylinder_z([-0.08, 0.46, -0.24], 0.04, 0.035, 0.44, 0.52, 16, scale);
        batches[gunmetal].add_pitched_cylinder_z([0.08, 0.46, -0.24], 0.04, 0.035, 0.44, 0.52, 16, scale);
        return true;
    }

    if has_token(name, "dtruck") || name.contains("demolition") {
        // Libyan Demolition Truck: 6-wheel flatbed truck + glowing nuclear payload drum
        add_wheeled_axles(batches, 0.40, &[-0.40, 0.0, 0.42], 0.16, 0.09, scale);
        batches[primary].add_tapered_box([0.0, 0.38, 0.40], [0.32, 0.18], [0.28, 0.14], 0.20, scale);
        batches[gunmetal].add_box([0.0, 0.24, -0.14], [0.34, 0.06, 0.42], scale);
        batches[emissive].add_cylinder_z([0.0, 0.48, -0.16], 0.24, 0.32, 24, scale);
        batches[steel].add_cylinder_z([0.0, 0.48, -0.16], 0.255, 0.05, 24, scale);
        return true;
    }

    if archetype == VisualArchetype::Tank || has_token(name, "mtnk") || has_token(name, "htnk") {
        // Grizzly / Rhino Battle Tank: road-wheeled tracks + sloped glacis + turret + rifled cannon + cupola
        let is_heavy = has_token(name, "htnk") || has_token(name, "id_3tnk") || name.contains("rhino");
        add_tank_tracks(batches, 0.43, 0.0, 0.56, 0.36, 0.11, 5, scale);
        batches[primary].add_tapered_box([0.0, 0.30, 0.0], [0.36, 0.56], [0.31, 0.46], 0.16, scale);
        batches[emissive].add_box([-0.24, 0.32, 0.52], [0.05, 0.03, 0.03], scale);
        batches[emissive].add_box([0.24, 0.32, 0.52], [0.05, 0.03, 0.03], scale);
        batches[primary].add_tapered_box([0.0, 0.54, -0.05], [0.26, 0.28], [0.20, 0.22], 0.13, scale);
        add_rifled_cannon_barrel(batches, [0.0, 0.54, 0.18], 0.052, 0.68, scale);
        add_turret_cupola_and_details(batches, [0.0, 0.54, -0.05], 0.22, 0.67, scale);
        if is_heavy {
            batches[steel].add_cylinder_x([-0.18, 0.38, -0.56], 0.08, 0.12, 20, scale);
            batches[steel].add_cylinder_x([0.18, 0.38, -0.56], 0.08, 0.12, 20, scale);
        }
        return true;
    }

    false
}

/// Orchestrates 3D model geometry creation by dispatching through each specialized category step.
pub(super) fn populate_archetype_batches(
    raw_name: &str,
    archetype: VisualArchetype,
    scale: [f32; 3],
    batches: &mut [GeometryBatch; 6],
) {
    let name = raw_name.to_ascii_lowercase();
    let is_building_archetype = !matches!(
        archetype,
        VisualArchetype::Infantry
            | VisualArchetype::Tank
            | VisualArchetype::Harvester
            | VisualArchetype::Aircraft
            | VisualArchetype::Ship
    );
    if is_building_archetype {
        if try_populate_production_buildings(&name, archetype, scale, batches) {
            return;
        }
        if try_populate_tech_and_support_buildings(&name, scale, batches) {
            return;
        }
        if try_populate_defensive_turrets(&name, archetype, scale, batches) {
            return;
        }
    }
    if try_populate_aircraft_units(&name, archetype, scale, batches) {
        return;
    }
    if try_populate_naval_units(&name, archetype, scale, batches) {
        return;
    }
    if try_populate_vehicle_and_tank_units(&name, archetype, scale, batches) {
        return;
    }
    populate_infantry_units(&name, scale, batches);
}

