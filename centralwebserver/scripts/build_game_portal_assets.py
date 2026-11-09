#!/usr/bin/env python3
"""
Builds the authentic game asset bundle and database for the IRONLOCK // RUST-RTS
Production Web Portal (https://rust-rts.bls-isp.net):
  - Optimizes unit & building cameos from rust-ra2/assets/ui/cameos/*.png -> public/assets/cameos/*.webp
  - Optimizes 4K country & campaign screens from rust-ra2/assets/ui/loading_screens/*.png -> public/assets/screens/*.webp
  - Extracts all 3 factions, 10 playable sub-countries, 70 units, 76 buildings, 58 weapons,
    campaigns, and 54 maps (with full tactical radar geometry) into public/game-database.json
"""

import json
import re
import tomllib
from pathlib import Path
from PIL import Image

WORKSPACE_ROOT = Path(__file__).resolve().parent.parent.parent
RA2_ROOT = WORKSPACE_ROOT / "rust-ra2"
PUBLIC_DIR = Path(__file__).resolve().parent.parent / "public"
CAMEOS_OUT_DIR = PUBLIC_DIR / "assets" / "cameos"
SCREENS_OUT_DIR = PUBLIC_DIR / "assets" / "screens"


CAMEO_EXPLICIT_MAP: dict[str, str] = {
    # Allied Units
    "allied_gi": "giicon.png",
    "allied_guardian_gi": "gdgiicon.png",
    "guardian_gi": "gdgiicon.png",
    "allied_engineer": "engnicon.png",
    "allied_attack_dog": "adogicon.png",
    "rocketeer": "jjeticon.png",
    "allied_rocketeer": "jjeticon.png",
    "navy_seal": "sealicon.png",
    "allied_seal": "sealicon.png",
    "tanya": "tanyicon.png",
    "allied_tanya": "tanyicon.png",
    "spy": "spyicon.png",
    "allied_spy": "spyicon.png",
    "chrono_legionnaire": "clegicon.png",
    "sniper": "snipicon.png",
    "british_sniper": "snipicon.png",
    "grizzly_tank": "gtnkicon.png",
    "allied_grizzly_tank": "gtnkicon.png",
    "ifv": "fvicon.png",
    "allied_ifv": "fvicon.png",
    "mirage_tank": "mtnkicon.png",
    "prism_tank": "rtnkicon.png",
    "battle_fortress": "bfrticon.png",
    "tank_destroyer": "tnkdicon.png",
    "chrono_miner": "ahrvicon.png",
    "allied_harvester": "ahrvicon.png",
    "allied_mcv": "mcvicon.png",
    "robot_tank": "roboicon.png",
    "nighthawk": "shadicon.png",
    "nighthawk_chopper": "shadicon.png",
    "nighthawk_transport": "shadicon.png",
    "osprey_asw": "heliicon.png",
    "paradrop_plane": "paraicon.png",
    "crashed_ufo": "diskicon.png",
    "soviet_sub_apc": "sapcicon.png",
    "harrier": "falcicon.png",
    "allied_harrier": "falcicon.png",
    "black_eagle": "beagicon.png",
    "hornet_fighter": "falcicon.png",
    "allied_destroyer": "desticon.png",
    "destroyer": "desticon.png",
    "aegis_cruiser": "agisicon.png",
    "aircraft_carrier": "carricon.png",
    "dolphin": "dlphicon.png",
    "amphibious_transport": "landicon.png",
    "allied_amphibious_transport": "landicon.png",
    "chrono_commando": "ccomicon.png",
    "psi_commando": "psicicon.png",
    "chrono_ivan": "ivncicon.png",
    # Soviet Units
    "soviet_conscript": "e2icon.png",
    "conscript": "e2icon.png",
    "flak_trooper": "flkticon.png",
    "soviet_flak_trooper": "flkticon.png",
    "soviet_engineer": "engnicon.png",
    "soviet_attack_dog": "dogicon.png",
    "tesla_trooper": "shkicon.png",
    "desolator": "desoicon.png",
    "crazy_ivan": "ivanicon.png",
    "terrorist": "trsticon.png",
    "boris": "brisicon.png",
    "cosmonaut": "lunricon.png",
    "soviet_cosmonaut": "lunricon.png",
    "soviet_rhino_tank": "htnkicon.png",
    "rhino_tank": "htnkicon.png",
    "apocalypse_tank": "mtnkuico.png",
    "soviet_apocalypse_tank": "mtnkuico.png",
    "flak_track": "htkicon.png",
    "v3_launcher": "v3icon.png",
    "v3_rocket_launcher": "v3icon.png",
    "terror_drone": "dronicon.png",
    "tesla_tank": "ttnkicon.png",
    "demolition_truck": "trkaicon.png",
    "war_miner": "harvicon.png",
    "soviet_harvester": "harvicon.png",
    "soviet_mcv": "smcvicon.png",
    "kirov_airship": "zepicon.png",
    "kirov": "zepicon.png",
    "siege_chopper": "schpicon.png",
    "mig_fighter": "bplnicon.png",
    "spy_plane": "spypicon.png",
    "typhoon_sub": "subicon.png",
    "sea_scorpion": "hovricon.png",
    "dreadnought": "dredicon.png",
    "giant_squid": "sqdicon.png",
    "soviet_amphibious_transport": "sapcicon.png",
    # Yuri Units
    "yuri_initiate": "initicon.png",
    "initiate": "initicon.png",
    "yuri_brute": "bruticon.png",
    "brute": "bruticon.png",
    "yuri_virus": "vrusicon.png",
    "virus": "vrusicon.png",
    "yuri_clone": "yuriicon.png",
    "yuri_prime": "yurpicon.png",
    "yuri_engineer": "yengicon.png",
    "yuri_slave_miner": "sminicon.png",
    "slave_miner": "sminicon.png",
    "lasher_tank": "ltnkicon.png",
    "yuri_lasher_tank": "ltnkicon.png",
    "gattling_tank": "ytnkicon.png",
    "gatling_tank": "ytnkicon.png",
    "yuri_gatling_tank": "ytnkicon.png",
    "magnetron": "teleicon.png",
    "chaos_drone": "caosicon.png",
    "mastermind": "mindicon.png",
    "floating_disc": "diskicon.png",
    "boomer_sub": "bsubicon.png",
    "yuri_boomer_sub": "bsubicon.png",
    "yuri_mcv": "ypcvicon.png",
    "yuri_amphibious_transport": "yhvricon.png",
    # Allied Buildings
    "allied_conyard": "ayaricon.png",
    "allied_power_plant": "powricon.png",
    "allied_power_plant_overcharged": "powricon.png",
    "allied_barracks": "brrkicon.png",
    "allied_refinery": "reficon.png",
    "allied_war_factory": "gwepicon.png",
    "allied_airforce_command": "asaticon.png",
    "allied_naval_shipyard": "yardicon.png",
    "allied_service_depot": "fixicon.png",
    "allied_battle_lab": "techicon.png",
    "allied_ore_purifier": "gorep.png",
    "allied_robot_control": "rbccicon.png",
    "allied_spy_satellite": "asaticon.png",
    "allied_pillbox": "pillicon.png",
    "allied_patriot_missile": "plticon.png",
    "allied_prism_tower": "prisicon.png",
    "allied_grand_cannon": "gcanicon.png",
    "allied_gap_generator": "gapicon.png",
    "allied_chronosphere": "csphicon.png",
    "allied_weather_controller": "wethicon.png",
    "allied_wall": "wallicon.png",
    # Soviet Buildings
    "soviet_conyard": "nwalicon.png",
    "soviet_tesla_reactor": "npwricon.png",
    "soviet_barracks": "handicon.png",
    "soviet_refinery": "nreficon.png",
    "soviet_war_factory": "nwepicon.png",
    "soviet_radar_tower": "nradicon.png",
    "soviet_naval_shipyard": "yardicon.png",
    "soviet_service_depot": "rfixicon.png",
    "soviet_battle_lab": "ntchicon.png",
    "soviet_nuclear_reactor": "nrcticon.png",
    "soviet_industrial_plant": "indpicon.png",
    "soviet_sentry_gun": "ggunicon.png",
    "soviet_flak_cannon": "flakicon.png",
    "soviet_tesla_coil": "tslaicon.png",
    "soviet_battle_bunker": "bnkricon.png",
    "soviet_iron_curtain": "ironicon.png",
    "soviet_nuclear_silo": "msslicon.png",
    "soviet_wall": "nwalicon.png",
    # Yuri Buildings
    "yuri_conyard": "yconicon.png",
    "yuri_bio_reactor": "ypwricon.png",
    "yuri_barracks": "brckicon.png",
    "yuri_slave_refinery": "sreficon.png",
    "yuri_war_factory": "ywepicon.png",
    "yuri_submarine_pen": "yyrdicon.png",
    "yuri_psychic_sensor": "psisicon.png",
    "yuri_battle_lab": "ytchicon.png",
    "yuri_gatling_cannon": "gtncicon.png",
    "yuri_psychic_tower": "psyticon.png",
    "yuri_tank_bunker": "tbnkicon.png",
    "yuri_grinder": "grndicon.png",
    "yuri_cloning_vats": "clonicon.png",
    "yuri_genetic_mutator": "muteicon.png",
    "yuri_psychic_dominator": "pdomicon.png",
    "yuri_citadel_wall": "citwicon.png",
    # Neutral / Tech Buildings
    "tech_oil_derrick": "indpicon.png",
    "tech_airport": "aparicon.png",
    "tech_hospital": "fixicon.png",
    "tech_outpost": "rfixicon.png",
    "tech_machine_shop": "rfixicon.png",
    "tech_secret_lab": "techicon.png",
    "concrete_wall": "wallicon.png",
    "security_gate": "wallicon.png",
    "neutral_bunker": "bnkricon.png",
    "psychic_beacon": "psyricon.png",
    "psychic_amplifier": "pdomicon.png",
    "time_machine": "csphicon.png",
    "eiffel_tower": "tslaicon.png",
    "statue_of_liberty": "asaticon.png",
    "white_house": "techicon.png",
    "air_force_chapel": "aparicon.png",
    "london_big_ben": "techicon.png",
    "alcatraz_prison": "pillicon.png",
    "kremlin_palace": "ntchicon.png",
    "st_basil_cathedral": "nradicon.png",
    "transylvania_fortress": "ytchicon.png",
    "great_pyramid": "ytchicon.png",
}


def resolve_cameo_filename(entity_id: str, role: str, faction: str, cameo_dir: Path) -> str:
    if entity_id in CAMEO_EXPLICIT_MAP:
        cand = CAMEO_EXPLICIT_MAP[entity_id]
        if (cameo_dir / cand).exists():
            return cand

    # Try heuristic matches
    lower = entity_id.lower()
    if "prism" in lower and "tower" in lower:
        return "prisicon.png"
    if "prism" in lower:
        return "rtnkicon.png"
    if "tesla" in lower and ("coil" in lower or "tower" in lower):
        return "tslaicon.png"
    if "tesla" in lower and "tank" in lower:
        return "ttnkicon.png"
    if "tesla" in lower and "reactor" in lower:
        return "npwricon.png"
    if "tesla" in lower:
        return "shkicon.png"
    if "chrono" in lower and "sphere" in lower:
        return "csphicon.png"
    if "chrono" in lower and "miner" in lower:
        return "ahrvicon.png"
    if "chrono" in lower:
        return "clegicon.png"
    if "weather" in lower:
        return "wethicon.png"
    if "iron_curtain" in lower:
        return "ironicon.png"
    if "nuke" in lower or "nuclear_silo" in lower:
        return "msslicon.png"
    if "nuclear" in lower:
        return "nrcticon.png"
    if "dominator" in lower:
        return "pdomicon.png"
    if "mutator" in lower:
        return "muteicon.png"
    if "clon" in lower:
        return "clonicon.png"
    if "grinder" in lower:
        return "grndicon.png"
    if "bunker" in lower and "tank" in lower:
        return "tbnkicon.png"
    if "bunker" in lower:
        return "bnkricon.png"
    if "gatling" in lower or "gattling" in lower:
        return "ytnkicon.png" if role != "defense" else "gtncicon.png"
    if "psychic_tower" in lower or "psy_tower" in lower:
        return "psyticon.png"
    if "sensor" in lower:
        return "psisicon.png"
    if "radar" in lower or "satellite" in lower or "airforce" in lower:
        return "nradicon.png" if faction == "soviets" else "asaticon.png"
    if "shipyard" in lower or "naval" in lower or "sub_pen" in lower or "submarine_pen" in lower:
        return "yyrdicon.png" if faction == "yuri" else "yardicon.png"
    if "depot" in lower or "repair" in lower:
        return "rfixicon.png" if faction == "soviets" else "fixicon.png"
    if "lab" in lower:
        return "ytchicon.png" if faction == "yuri" else ("ntchicon.png" if faction == "soviets" else "techicon.png")
    if "factory" in lower:
        return "ywepicon.png" if faction == "yuri" else ("nwepicon.png" if faction == "soviets" else "gwepicon.png")
    if "refinery" in lower:
        return "sreficon.png" if faction == "yuri" else ("nreficon.png" if faction == "soviets" else "reficon.png")
    if "barracks" in lower:
        return "brckicon.png" if faction == "yuri" else ("handicon.png" if faction == "soviets" else "brrkicon.png")
    if "power" in lower or "reactor" in lower:
        return "ypwricon.png" if faction == "yuri" else ("npwricon.png" if faction == "soviets" else "powricon.png")
    if "conyard" in lower or "yard" in lower:
        return "yconicon.png" if faction == "yuri" else ("nwalicon.png" if faction == "soviets" else "ayaricon.png")
    if "wall" in lower or "gate" in lower:
        return "citwicon.png" if faction == "yuri" else ("nwalicon.png" if faction == "soviets" else "wallicon.png")
    if "pillbox" in lower or "sentry" in lower:
        return "ggunicon.png" if faction == "soviets" else "pillicon.png"
    if "flak" in lower or "patriot" in lower or "sam" in lower:
        return "flakicon.png" if faction == "soviets" else "plticon.png"
    if "dog" in lower:
        return "dogicon.png" if faction == "soviets" else "adogicon.png"
    if "engineer" in lower:
        return "yengicon.png" if faction == "yuri" else "engnicon.png"
    if "mcv" in lower:
        return "ypcvicon.png" if faction == "yuri" else ("smcvicon.png" if faction == "soviets" else "mcvicon.png")
    if "miner" in lower or "harvester" in lower:
        return "sminicon.png" if faction == "yuri" else ("harvicon.png" if faction == "soviets" else "ahrvicon.png")
    if role == "infantry":
        return "initicon.png" if faction == "yuri" else ("e2icon.png" if faction == "soviets" else "giicon.png")
    return "gtnkicon.png"


def main() -> None:
    CAMEOS_OUT_DIR.mkdir(parents=True, exist_ok=True)
    SCREENS_OUT_DIR.mkdir(parents=True, exist_ok=True)

    cameo_src_dir = RA2_ROOT / "assets" / "ui" / "cameos"
    screens_src_dir = RA2_ROOT / "assets" / "ui" / "loading_screens"

    factions_raw = tomllib.loads((RA2_ROOT / "factions" / "factions.toml").read_text("utf-8"))["factions"]
    units_raw = tomllib.loads((RA2_ROOT / "units" / "units.toml").read_text("utf-8"))["units"]
    buildings_raw = tomllib.loads((RA2_ROOT / "buildings" / "buildings.toml").read_text("utf-8"))["buildings"]
    weapons_raw = tomllib.loads((RA2_ROOT / "weapons" / "weapons.toml").read_text("utf-8"))["weapons"]
    campaigns_raw = tomllib.loads((RA2_ROOT / "campaigns" / "campaigns.toml").read_text("utf-8"))["campaigns"]

    weapons_by_id: dict[str, dict] = {}
    weapons_out: list[dict] = []
    for w in weapons_raw:
        dmg = float(w.get("damage", 0.0))
        cd = max(0.1, float(w.get("cooldown_secs", 1.0)))
        burst = int(w.get("burst", 1))
        dps = round((dmg * burst) / cd, 1)
        w_entry = {
            "id": w["id"],
            "name": w.get("name", w["id"]),
            "damage": dmg,
            "cooldown_secs": cd,
            "burst": burst,
            "dps": dps,
            "range": float(w.get("range", 10.0)),
            "projectile_speed": float(w.get("projectile_speed", 35.0)),
            "projectile_style": w.get("projectile_style", "shell"),
            "splash_radius": float(w.get("splash_radius", 0.0)),
            "targets_ground": bool(w.get("targets_ground", True)),
            "targets_air": bool(w.get("targets_air", False)),
            "armor_multipliers": w.get(
                "armor_multipliers",
                {"none": 1.0, "light": 1.0, "medium": 1.0, "heavy": 1.0, "structure": 1.0},
            ),
        }
        weapons_by_id[w["id"]] = w_entry
        weapons_out.append(w_entry)

    referenced_cameos: set[str] = set()

    units_out: list[dict] = []
    for u in units_raw:
        uid = u["id"]
        role = u.get("role", "vehicle")
        faction = u.get("faction", "allies")
        cameo_png = resolve_cameo_filename(uid, role, faction, cameo_src_dir)
        cameo_stem = Path(cameo_png).stem
        referenced_cameos.add(cameo_png)

        pw_id = u.get("primary_weapon")
        pw_info = weapons_by_id.get(pw_id) if pw_id else None
        vis = u.get("visual", {})
        abilities = []
        for ab in u.get("abilities", []):
            abilities.append({
                "id": ab.get("id", ""),
                "name": ab.get("name", ab.get("id", "")),
                "cooldown_secs": float(ab.get("cooldown_secs", 0.0)),
                "target_mode": ab.get("target_mode", "self_only"),
            })

        units_out.append({
            "id": uid,
            "name": u.get("name", uid),
            "faction": faction,
            "role": role,
            "movement_domain": u.get("movement_domain", "ground"),
            "cost": int(u.get("cost", 0)),
            "supply_cost": int(u.get("supply_cost", 1)),
            "build_time_secs": float(u.get("build_time_secs", 5.0)),
            "health": float(u.get("health", 100.0)),
            "armor": u.get("armor", "light"),
            "speed": float(u.get("speed", 4.0)),
            "vision_range": float(u.get("vision_range", 14.0)),
            "primary_weapon": pw_id,
            "weapon_details": pw_info,
            "prerequisites": u.get("prerequisites", []),
            "ability_id": u.get("ability_id"),
            "abilities": abilities,
            "crushes_infantry": bool(u.get("crushes_infantry", False)),
            "detects_cloak": bool(u.get("detects_cloak", False)),
            "cloaked": bool(u.get("cloaked", False)),
            "passenger_slots": int(u.get("passenger_slots", 0)),
            "harvest_capacity": int(u.get("harvest_capacity", 0)),
            "cameo": f"/assets/cameos/{cameo_stem}.webp",
            "visual": {
                "archetype": vis.get("archetype", "tank"),
                "scale": vis.get("scale", [1.5, 1.2, 2.0]),
                "primary_color": vis.get("primary_color", [0.22, 0.45, 0.88]),
                "model": vis.get("model", ""),
            },
        })

    buildings_out: list[dict] = []
    for b in buildings_raw:
        bid = b["id"]
        role = b.get("role", "structure")
        faction = b.get("faction", "allies")
        cameo_png = resolve_cameo_filename(bid, role, faction, cameo_src_dir)
        cameo_stem = Path(cameo_png).stem
        referenced_cameos.add(cameo_png)

        pw_id = b.get("primary_weapon")
        pw_info = weapons_by_id.get(pw_id) if pw_id else None
        vis = b.get("visual", {})
        support_powers = []
        for sp in b.get("support_powers", []):
            support_powers.append({
                "id": sp.get("id", ""),
                "name": sp.get("name", sp.get("id", "")),
                "kind": sp.get("kind", ""),
                "cooldown_secs": float(sp.get("cooldown_secs", 60.0)),
                "cost": int(sp.get("cost", 0)),
                "radius": float(sp.get("radius", 6.0)),
                "duration_secs": float(sp.get("duration_secs", 0.0)),
            })

        buildings_out.append({
            "id": bid,
            "name": b.get("name", bid),
            "faction": faction,
            "role": role,
            "cost": int(b.get("cost", 500)),
            "build_time_secs": float(b.get("build_time_secs", 8.0)),
            "health": float(b.get("health", 800.0)),
            "armor": b.get("armor", "structure"),
            "footprint": b.get("footprint", [2, 2]),
            "power_delta": int(b.get("power_delta", 0)),
            "supply_provided": int(b.get("supply_provided", 0)),
            "vision_range": float(b.get("vision_range", 16.0)),
            "garrison_capacity": int(b.get("garrison_capacity", 0)),
            "primary_weapon": pw_id,
            "weapon_details": pw_info,
            "prerequisites": b.get("prerequisites", []),
            "upgrades_to": b.get("upgrades_to"),
            "spawns_unit_on_complete": b.get("spawns_unit_on_complete"),
            "support_powers": support_powers,
            "cameo": f"/assets/cameos/{cameo_stem}.webp",
            "visual": {
                "archetype": vis.get("archetype", "barracks"),
                "scale": vis.get("scale", [2.0, 1.5, 2.0]),
                "primary_color": vis.get("primary_color", [0.22, 0.45, 0.88]),
                "model": vis.get("model", ""),
            },
        })

    # Convert referenced cameos to crisp 120x96 WebP
    converted_cameos = 0
    for png_name in sorted(referenced_cameos):
        src_path = cameo_src_dir / png_name
        if not src_path.exists():
            print(f"WARNING: Missing cameo {src_path}")
            continue
        out_path = CAMEOS_OUT_DIR / f"{Path(png_name).stem}.webp"
        with Image.open(src_path) as im:
            im_rgb = im.convert("RGBA")
            im_resized = im_rgb.resize((120, 96), Image.Resampling.LANCZOS)
            im_resized.save(out_path, "WEBP", quality=90, method=6)
            converted_cameos += 1

    # Convert country & campaign screens to 960x720 WebP
    screens_to_export = [
        "country_usa.png",
        "country_great_britain.png",
        "country_france.png",
        "country_germany.png",
        "country_korea.png",
        "country_russia.png",
        "country_iraq.png",
        "country_libya.png",
        "country_cuba.png",
        "country_yuri.png",
        "ra2_allied_01.png",
        "ra2_allied_03.png",
        "ra2_allied_06.png",
        "ra2_allied_10.png",
        "ra2_allied_12.png",
        "ra2_soviet_01.png",
        "ra2_soviet_03.png",
        "ra2_soviet_05.png",
        "ra2_soviet_11.png",
        "ra2_soviet_12.png",
        "yr_allied_01.png",
        "yr_allied_07.png",
        "yr_soviet_01.png",
        "yr_soviet_06.png",
    ]
    converted_screens = 0
    for scr_name in screens_to_export:
        src_path = screens_src_dir / scr_name
        if not src_path.exists():
            continue
        out_path = SCREENS_OUT_DIR / f"{Path(scr_name).stem}.webp"
        with Image.open(src_path) as im:
            im_rgb = im.convert("RGB")
            im_resized = im_rgb.resize((960, 720), Image.Resampling.LANCZOS)
            im_resized.save(out_path, "WEBP", quality=82, method=6)
            converted_screens += 1

    # Build 10 playable countries list
    countries = [
        {
            "id": "usa",
            "name": "Vereinigte Staaten (USA)",
            "faction": "allies",
            "special_unit": "Airborne Paratroopers & Grizzly Battle Tank",
            "special_cameo": "/assets/cameos/giicon.webp",
            "doctrine": "Luftlande-Fallschirmjäger-Abwurf auf jeder Kartenposition; überlegene mobile Panzerdoktrin.",
            "artwork": "/assets/screens/country_usa.webp",
        },
        {
            "id": "great_britain",
            "name": "Großbritannien",
            "faction": "allies",
            "special_unit": "British Sniper & Mirage Tank",
            "special_cameo": "/assets/cameos/snipicon.webp",
            "doctrine": "Präzisions-Scharfschützen eliminieren feindliche Infanterie und Spezialisten aus sicherer Distanz.",
            "artwork": "/assets/screens/country_great_britain.webp",
        },
        {
            "id": "france",
            "name": "Frankreich",
            "faction": "allies",
            "special_unit": "Grand Cannon Coastal Battery & Prism Tank",
            "special_cameo": "/assets/cameos/gcanicon.webp",
            "doctrine": "Schwerste Langstrecken-Artilleriebefestigung (Grand Cannon) zur uneinnehmbaren Basisverteidigung.",
            "artwork": "/assets/screens/country_france.webp",
        },
        {
            "id": "germany",
            "name": "Deutschland",
            "faction": "allies",
            "special_unit": "Jagdpanzer (Tank Destroyer) & Chronosphäre",
            "special_cameo": "/assets/cameos/tnkdicon.webp",
            "doctrine": "Spezialisierter Panzerjäger mit extremem Durchschlag gegen schwere Sowjet- und Yuri-Panzerverbände.",
            "artwork": "/assets/screens/country_germany.webp",
        },
        {
            "id": "korea",
            "name": "Republik Korea",
            "faction": "allies",
            "special_unit": "Black Eagle Strike Fighter & Aegis Cruiser",
            "special_cameo": "/assets/cameos/beagicon.webp",
            "doctrine": "Schwer gepanzerter Überschall-Jagdbomber für chirurgische Luftschläge gegen Schlüsselziele.",
            "artwork": "/assets/screens/country_korea.webp",
        },
        {
            "id": "russia",
            "name": "Sowjetische Föderation (Russland)",
            "faction": "soviets",
            "special_unit": "Tesla Tank, Apocalypse Tank & Kirov Airship",
            "special_cameo": "/assets/cameos/ttnkicon.webp",
            "doctrine": "Gepanzerter mobiler Tesla-Emitter, der feindliche Verteidigungslinien und Elektronik lahmlegt.",
            "artwork": "/assets/screens/country_russia.webp",
        },
        {
            "id": "iraq",
            "name": "Irak",
            "faction": "soviets",
            "special_unit": "Desolator (Rad-Trooper) & Rhino Heavy Tank",
            "special_cameo": "/assets/cameos/desoicon.webp",
            "doctrine": "Nuklear-biologische Strahlenwaffe; verseucht ganze Gebiete gegen Infanterie und leichte Fahrzeuge.",
            "artwork": "/assets/screens/country_iraq.webp",
        },
        {
            "id": "libya",
            "name": "Libyen",
            "faction": "soviets",
            "special_unit": "Demolition Truck & V3 Rocket Launcher",
            "special_cameo": "/assets/cameos/trkaicon.webp",
            "doctrine": "Taktischer Nuklear-Sprengstofftransporter für verheerende Flächenschäden an feindlichen Basen.",
            "artwork": "/assets/screens/country_libya.webp",
        },
        {
            "id": "cuba",
            "name": "Kuba",
            "faction": "soviets",
            "special_unit": "Cuban Terrorist & Terror Drone",
            "special_cameo": "/assets/cameos/ivanicon.webp",
            "doctrine": "Schnelle asymmetrische Sprengstoff-Kommandos und mechanische Infiltrations-Drohnen.",
            "artwork": "/assets/screens/country_cuba.webp",
        },
        {
            "id": "yuri",
            "name": "Psi-Corps (Yuri's Psychic Army)",
            "faction": "yuri",
            "special_unit": "Yuri Prime, Mastermind, Magnetron & Floating Disc",
            "special_cameo": "/assets/cameos/yurpicon.webp",
            "doctrine": "Gedankenkontrolle feindlicher Armeen, telekinetische Magnetrons, Klon-Stätten und Genetik-Mutatoren.",
            "artwork": "/assets/screens/country_yuri.webp",
        },
    ]

    # Parse all 54 maps with full radar geometry
    maps_out: list[dict] = []
    for map_file in sorted((RA2_ROOT / "maps").glob("*.toml")):
        m_data = tomllib.loads(map_file.read_text("utf-8"))
        mid = m_data.get("id", map_file.stem)
        mname = m_data.get("name", mid)
        width = int(m_data.get("width", 64))
        height = int(m_data.get("height", 64))

        category = "skirmish"
        if mid.startswith("ra2_allied_"):
            category = "campaign_allied"
        elif mid.startswith("ra2_soviet_"):
            category = "campaign_soviet"
        elif mid.startswith("yr_allied_"):
            category = "campaign_yr_allied"
        elif mid.startswith("yr_soviet_"):
            category = "campaign_yr_soviet"
        elif mid.startswith("ra2_bootcamp"):
            category = "bootcamp"

        spawns = []
        for sp in m_data.get("spawns", []):
            spawns.append({
                "player_id": int(sp.get("player_id", 1)),
                "faction": sp.get("faction", "allies"),
                "base_tile": sp.get("base_tile", [10, 10]),
                "initial_buildings_count": len(sp.get("initial_buildings", [])),
                "initial_units_count": len(sp.get("initial_units", [])),
            })

        ore_patches = []
        for op in m_data.get("ore_patches", []):
            ore_patches.append({
                "position": op.get("position", [20, 20]),
                "radius": int(op.get("radius", 3)),
                "amount_per_tile": int(op.get("amount_per_tile", 1200)),
            })

        tech_structures = []
        for pe in m_data.get("preplaced_entities", []):
            if "building_id" in pe:
                tech_structures.append({
                    "building_id": pe["building_id"],
                    "tile_pos": pe.get("tile_pos", [30, 30]),
                })

        maps_out.append({
            "id": mid,
            "name": mname,
            "category": category,
            "width": width,
            "height": height,
            "tile_size": float(m_data.get("tile_size", 2.0)),
            "ore_regen_per_sec": float(m_data.get("ore_regen_per_sec", 3.5)),
            "mission_objective": m_data.get("mission_objective", ""),
            "max_players": max(2, len(spawns)),
            "spawns": spawns,
            "ore_patches": ore_patches,
            "obstacles": m_data.get("obstacles", []),
            "elevation_rects": m_data.get("elevation_rects", []),
            "ramp_rects": m_data.get("ramp_rects", []),
            "water_rects": m_data.get("water_rects", []),
            "bridge_rects": m_data.get("bridge_rects", []),
            "tech_structures": tech_structures,
        })

    database = {
        "game_title": "IRONLOCK // RED ALERT 2 & YURI'S REVENGE 3D",
        "engine_version": "1.0.9",
        "tick_rate_hz": 20,
        "factions": factions_raw,
        "countries": countries,
        "units": units_out,
        "buildings": buildings_out,
        "weapons": weapons_out,
        "campaigns": campaigns_raw,
        "maps": maps_out,
    }

    db_path = PUBLIC_DIR / "game-database.json"
    db_path.write_text(json.dumps(database, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"[OK] Exported {converted_cameos} cameos, {converted_screens} screens, {len(units_out)} units, {len(buildings_out)} buildings, {len(weapons_out)} weapons, {len(maps_out)} maps -> {db_path} ({db_path.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
