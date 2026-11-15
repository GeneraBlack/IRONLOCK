"""
Wave-2 Articulated 3D Infantry, Specialists, Commandos & Attack Dogs (`w2_infantry_3d`).

Owns all 41 `bucket == "infantry_3d"` `.glb` models in `wave2_manifest.json` plus the 4
side-by-side comparison sheets `docs/model_sheets/infantry_3d_{1..4}.png`.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Callable, Dict, List, Tuple

from ..glb_postprocess import GlbVerificationSummary
from .allied_infantry import ALLIED_INFANTRY_BUILDERS
from .soviet_infantry import SOVIET_INFANTRY_BUILDERS
from .yuri_neutral_dogs import YURI_NEUTRAL_DOG_BUILDERS


@dataclass(frozen=True)
class InfantryAssetMeta:
    stem: str
    sheet_faction: str  # "allied" | "soviet" | "yuri"
    sprite_id: str      # Canonical key in `sprites_catalog.json["infantry"]`
    cameo_rel: str      # Relative path under `rust-ra2/`
    display_title: str
    subtitle: str
    hero_yaw_deg: float = 34.0
    hero_pitch_deg: float = 19.0
    is_dog: bool = False


INFANTRY_3D_BUILDERS: Dict[str, Callable[[str], GlbVerificationSummary]] = {
    **ALLIED_INFANTRY_BUILDERS,
    **SOVIET_INFANTRY_BUILDERS,
    **YURI_NEUTRAL_DOG_BUILDERS,
}

# Ordered 41-unit showcase split across 4 sheets (`11 + 10 + 10 + 10 = 41`)
INFANTRY_3D_CATALOG: Tuple[InfantryAssetMeta, ...] = (
    # --- Sheet 1 (11 Allied Infantry & Specialists) ---
    InfantryAssetMeta("allied_gi", "allied", "allied_gi", "assets/ui/cameos/giicon.png", "ALLIED G.I. RIFLEMAN", "PASGT Helmet, Blue Ballistic Armor, Rucksack & M4 Carbine"),
    InfantryAssetMeta("gi", "allied", "allied_gi", "assets/ui/cameos/giicon.png", "G.I. (TACTICAL OPTIC VARIANT)", "Visor PASGT, Thigh Pouches, Team Armor & Carbine"),
    InfantryAssetMeta("allied_infantry", "allied", "allied_gi", "assets/ui/cameos/giicon.png", "ALLIED INFANTRY (RADIO SQUAD)", "PASGT Helmet, Whip-Antenna Comms Pack & Carbine"),
    InfantryAssetMeta("guardian_gi", "allied", "guardian_gi", "assets/ui/cameos/gdgiicon.png", "ALLIED GUARDIAN G.I.", "Blast Visor, Deployable Shield & Shoulder Javelin AT Tube", hero_yaw_deg=42.0),
    InfantryAssetMeta("rocketeer", "allied", "rocketeer", "assets/ui/cameos/jjeticon.png", "ALLIED ROCKETEER", "Twin Chrome Jump-Jet Turbines, Aero Winglets & Dual SMGs", hero_yaw_deg=138.0),
    InfantryAssetMeta("allied_engineer", "allied", "allied_engineer", "assets/ui/cameos/engnicon.png", "ALLIED COMBAT ENGINEER", "Yellow Hard-Hat, Utility Vest, Gold/Team Briefcase & Pistol", hero_yaw_deg=-34.0),
    InfantryAssetMeta("sniper", "allied", "sniper", "assets/ui/cameos/snipicon.png", "BRITISH MARKSMAN SNIPER", "Tilted Team Beret, Camo Mantle & .338 Scoped Sniper Rifle", hero_yaw_deg=44.0),
    InfantryAssetMeta("allied_spy", "allied", "allied_spy", "assets/ui/cameos/spyicon.png", "ALLIED COVERT SPY", "Tailored Tuxedo, Team Tie/Badge, Attache Case & Silenced PPK", hero_yaw_deg=-32.0),
    InfantryAssetMeta("tanya", "allied", "tanya", "assets/ui/cameos/tanyicon.png", "SPECIAL AGENT TANYA", "Beret, Tactical Vest, C4 Pack & Dual Chrome Desert Eagles (2 Muzzles)", hero_yaw_deg=28.0),
    InfantryAssetMeta("chrono_legionnaire", "allied", "chrono_legionnaire", "assets/ui/cameos/clegicon.png", "CHRONO LEGIONNAIRE", "White/Chrome Suit, Glowing Chrono-Torus Halo & Phase Rifle", hero_yaw_deg=48.0),
    InfantryAssetMeta("navy_seal", "allied", "navy_seal", "assets/ui/cameos/sealicon.png", "ALLIED NAVY SEAL", "Black Wetsuit, Glowing Green NVGs, C4 Satchel & MP5-SD", hero_yaw_deg=36.0),
    # --- Sheet 2 (10 Allied Chrono-Commando + Soviet Infantry & Heavies) ---
    InfantryAssetMeta("chrono_commando", "allied", "chrono_commando", "assets/ui/cameos/chrono_commando.png", "CHRONO COMMANDO", "Spec-Ops SEAL Armor, Chrono-Torus Halo & Chrono MP5", hero_yaw_deg=46.0),
    InfantryAssetMeta("soviet_conscript", "soviet", "soviet_conscript", "assets/ui/cameos/e2icon.png", "SOVIET CONSCRIPT", "Flared Khaki Greatcoat, SSH-40 Gas-Mask Helmet & PPSh Drum Rifle"),
    InfantryAssetMeta("conscript", "soviet", "soviet_conscript", "assets/ui/cameos/e2icon.png", "CONSCRIPT (RIFLEMAN VARIANT)", "SSH-40 Steel Helmet, Red Pauldrons, Greatcoat & Drum Rifle"),
    InfantryAssetMeta("soviet_infantry", "soviet", "soviet_conscript", "assets/ui/cameos/e2icon.png", "SOVIET INFANTRY (OLIVE COAT)", "Olive Greatcoat, Respirator SSH-40, Bedroll & PPSh Drum Rifle"),
    InfantryAssetMeta("flak_trooper", "soviet", "flak_trooper", "assets/ui/cameos/flkticon.png", "SOVIET FLAK TROOPER", "Dark Iron Plate, Rotary Ammo Drum & Shoulder Flak Cannon", hero_yaw_deg=44.0),
    InfantryAssetMeta("tesla_trooper", "soviet", "tesla_trooper", "assets/ui/cameos/shkicon.png", "SOVIET TESLA TROOPER", "Bulky Power-Suit, Blue Visor Slit, Twin Copper Coils & Arc Gauntlet", hero_yaw_deg=42.0),
    InfantryAssetMeta("soviet_engineer", "soviet", "soviet_engineer", "assets/ui/cameos/engnicon.png", "SOVIET COMBAT ENGINEER", "Yellow/Red Hard-Hat, Crimson Vest & Soviet Repair Case", hero_yaw_deg=-34.0),
    InfantryAssetMeta("crazy_ivan", "soviet", "crazy_ivan", "assets/ui/cameos/ivanicon.png", "CRAZY IVAN", "Wild Beard, Eyepatch, Open Red Vest & Sparking TNT Bundle", hero_yaw_deg=32.0),
    InfantryAssetMeta("chrono_ivan", "soviet", "chrono_ivan", "assets/ui/cameos/chrono_ivan.png", "CHRONO IVAN", "Crazy Ivan Bomber Rig + Chrono-Torus Teleportation Pack", hero_yaw_deg=46.0),
    InfantryAssetMeta("desolator", "soviet", "desolator", "assets/ui/cameos/desoicon.png", "IRAQI DESOLATOR", "NBC Hazmat Carapace, Twin Glowing Isotope Tanks & Rad-Cannon", hero_yaw_deg=44.0),
    # --- Sheet 3 (10 Soviet Commandos + Yuri Psychics & Mutants) ---
    InfantryAssetMeta("boris", "soviet", "boris", "assets/ui/cameos/brisicon.png", "BORIS (SOVIET HERO)", "Fur Ushanka, Crimson-Trimmed Coat, Laser Designator & AKM", hero_yaw_deg=36.0),
    InfantryAssetMeta("terrorist", "soviet", "terrorist", "assets/ui/cameos/trsticon.png", "CUBAN TERRORIST", "Strapped C4 Chest Bricks, Copper Coils, Armed Timer & SMG", hero_yaw_deg=32.0),
    InfantryAssetMeta("yuri_initiate", "yuri", "yuri_initiate", "assets/ui/cameos/initicon.png", "YURI PSI-CORPS INITIATE", "Purple Mantle, Bronze Skull Mask, Glowing Goggles & Pyro Gauntlets"),
    InfantryAssetMeta("initiate", "yuri", "yuri_initiate", "assets/ui/cameos/initicon.png", "INITIATE (BRONZE GREAVES)", "Psi-Corps Respirator, Bronze Greaves & Pyrokinetic Gauntlets"),
    InfantryAssetMeta("yuri_infantry", "yuri", "yuri_initiate", "assets/ui/cameos/initicon.png", "YURI INFANTRY (ROBED ADEPT)", "Psi-Corps Skirted Tunic, Psychic Goggles & Pyro Gauntlets"),
    InfantryAssetMeta("yuri_clone", "yuri", "yuri_clone", "assets/ui/cameos/yuriicon.png", "YURI CLONE", "Bald Cranium, Gold Neural Headband, High-Collar Robe & Psi-Orb", hero_yaw_deg=30.0),
    InfantryAssetMeta("yuri", "yuri", "yuri_clone", "assets/ui/cameos/yuriicon.png", "YURI ADEPT", "Ornate Gold Psi-Collar, Flowing Purple Robe & Mind-Wave Orb", hero_yaw_deg=30.0),
    InfantryAssetMeta("yuri_prime", "yuri", "yuri_prime", "assets/ui/cameos/yurpicon.png", "YURI PRIME (HOVER THRONE)", "Anti-Gravity Cybernetic Hover-Pod, Neural Spires & Macro-Cranium", hero_yaw_deg=34.0),
    InfantryAssetMeta("yuri_brute", "yuri", "yuri_brute", "assets/ui/cameos/bruticon.png", "YURI GENETIC BRUTE", "1.48m Hulking Mutant, Mohawk, Purple Pants & Spiked Bracers", hero_yaw_deg=32.0),
    InfantryAssetMeta("brute", "yuri", "yuri_brute", "assets/ui/cameos/bruticon.png", "BRUTE (BRONZE SMASHER)", "1.48m Muscular Mutant Giant, Bronze Knee/Arm Spikes", hero_yaw_deg=32.0),
    # --- Sheet 4 (10 Yuri Specialists, Shared Units & Quadruped Attack Dogs) ---
    InfantryAssetMeta("yuri_virus", "yuri", "yuri_virus", "assets/ui/cameos/vrusicon.png", "YURI VIRUS BIO-SNIPER", "Sleek Purple Bio-Suit, Glowing Green Vials & Toxin Dart Rifle", hero_yaw_deg=42.0),
    InfantryAssetMeta("psi_commando", "yuri", "psi_commando", "assets/ui/cameos/psi_commando.png", "PSI COMMANDO", "Purple/Olive Spec-Ops Rig, Psychic Visor, C4 Pack & Psi-MP5", hero_yaw_deg=36.0),
    InfantryAssetMeta("yuri_psi_commando", "yuri", "psi_commando", "assets/ui/cameos/psi_commando.png", "YURI PSI COMMANDO", "Elite Psi-Saboteur Helmet, Psychic Amplifier & Suppressed SMG", hero_yaw_deg=36.0),
    InfantryAssetMeta("yuri_engineer", "yuri", "yuri_engineer", "assets/ui/cameos/yengicon.png", "YURI COMBAT ENGINEER", "Yellow Hard-Hat, Purple Vest & Bronze/Purple Psi-Repair Case", hero_yaw_deg=-34.0),
    InfantryAssetMeta("engineer", "allied", "allied_engineer", "assets/ui/cameos/engnicon.png", "FIELD ENGINEER (SHARED)", "Headlamp Hard-Hat, Hazard Belt, Team Vest & Steel Case", hero_yaw_deg=-34.0),
    InfantryAssetMeta("scout", "allied", "allied_gi", "assets/ui/cameos/giicon.png", "RECON SCOUT (SHARED)", "Boonie Hat, Tactical Goggles, Whip-Antenna Pack & Carbine", hero_yaw_deg=36.0),
    InfantryAssetMeta("allied_attack_dog", "allied", "allied_attack_dog", "assets/ui/cameos/adogicon.png", "ALLIED ATTACK DOG", "German Shepherd Coat, Dark Saddle, Pointed Ears & Team Harness", hero_yaw_deg=56.0, is_dog=True),
    InfantryAssetMeta("attack_dog", "allied", "allied_attack_dog", "assets/ui/cameos/adogicon.png", "ATTACK DOG (K-9 VARIANT)", "German Shepherd K-9 Quadruped Rig, Snapping Jaw & Collar", hero_yaw_deg=56.0, is_dog=True),
    InfantryAssetMeta("soviet_attack_dog", "soviet", "soviet_attack_dog", "assets/ui/cameos/dogicon.png", "SOVIET ATTACK DOG", "Siberian Husky Silver/White Coat, Ice Eyes & Red War-Harness", hero_yaw_deg=56.0, is_dog=True),
    InfantryAssetMeta("soviet_dog", "soviet", "soviet_attack_dog", "assets/ui/cameos/dogicon.png", "SOVIET DOG (WAR-HOUND)", "Siberian Husky Quadruped Rig, Spiked Collar & Bushy Tail", hero_yaw_deg=56.0, is_dog=True),
)

SHEET_SPLITS: Tuple[Tuple[str, str, List[InfantryAssetMeta]], ...] = (
    ("infantry_3d_1.png", "WAVE-2 INFANTRY 3D (1/4) - ALLIED INFANTRY & SPECIALISTS", list(INFANTRY_3D_CATALOG[0:11])),
    ("infantry_3d_2.png", "WAVE-2 INFANTRY 3D (2/4) - CHRONO COMMANDO & SOVIET TROOPERS", list(INFANTRY_3D_CATALOG[11:21])),
    ("infantry_3d_3.png", "WAVE-2 INFANTRY 3D (3/4) - SOVIET HEROES, YURI PSYCHICS & BRUTES", list(INFANTRY_3D_CATALOG[21:31])),
    ("infantry_3d_4.png", "WAVE-2 INFANTRY 3D (4/4) - YURI SPECIALISTS, SHARED & K-9 DOGS", list(INFANTRY_3D_CATALOG[31:41])),
)
