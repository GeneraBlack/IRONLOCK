use super::{smart_import, SmartImportReport};
use crate::openra::build_sample_openra_map_bin;
use crate::westwood::{build_sample_shp_bytes, build_sample_vxl_bytes, westwood_mix_crc};
use anyhow::Context;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

/// Generates ready-to-use Westwood (.ini + .vxl + .mix) and OpenRA (MiniYAML rules + weapons + map)
/// sample legacy folders under `legacy-imports/` and converts both into playable 3D mods under `mods/`.
pub fn generate_and_convert_demo_packs(
    workspace_root: &Path,
) -> Result<(SmartImportReport, SmartImportReport)> {
    let ww_src = workspace_root.join("legacy-imports/westwood-classic");
    let ora_src = workspace_root.join("legacy-imports/openra-classic");
    fs::create_dir_all(&ww_src)?;
    fs::create_dir_all(ora_src.join("rules"))?;
    fs::create_dir_all(ora_src.join("weapons"))?;
    fs::create_dir_all(ora_src.join("maps"))?;

    let sample_ini = r#"; Classic Westwood Red Alert 2 / Tiberian Sun Legacy Ruleset
[BuildingTypes]
0=GACNST
1=GAPOWR
2=GAPILE
3=GAREFN
4=GAWEAP
5=NACNST
6=NAPOWR
7=NAHAND
8=NAREFN
9=NAWEAP
10=TESLA

[InfantryTypes]
0=E1
1=E2
2=ENGINEER

[VehicleTypes]
0=MTNK
1=HTNK
2=CMIN
3=HARV

[GACNST]
Name=Allied Construction Yard
Cost=2500
Strength=1500
Armor=concrete
ConstructionYard=yes
Owner=Allies
Foundation=3x3

[GAPOWR]
Name=Allied Power Plant
Cost=800
Strength=750
Armor=wood
Power=200
Prerequisite=GACNST
Owner=Allies
Foundation=2x2

[GAPILE]
Name=Allied Barracks
Cost=500
Strength=800
Armor=steel
Power=-20
Prerequisite=GAPOWR
Factory=InfantryType
Owner=Allies
Foundation=2x2

[GAREFN]
Name=Allied Ore Refinery
Cost=2000
Strength=1000
Armor=wood
Power=-50
Refinery=yes
Prerequisite=GAPOWR
Owner=Allies
Foundation=3x3

[GAWEAP]
Name=Allied War Factory
Cost=2000
Strength=1100
Armor=steel
Power=-50
Factory=UnitType
Prerequisite=GAREFN
Owner=Allies
Foundation=3x3

[NACNST]
Name=Soviet Construction Yard
Cost=2500
Strength=1500
Armor=concrete
ConstructionYard=yes
Owner=Soviet
Foundation=3x3

[NAPOWR]
Name=Soviet Tesla Reactor
Cost=600
Strength=750
Armor=wood
Power=150
Prerequisite=NACNST
Owner=Soviet
Foundation=2x2

[NAHAND]
Name=Soviet Barracks
Cost=500
Strength=800
Armor=steel
Power=-20
Prerequisite=NAPOWR
Factory=InfantryType
Owner=Soviet
Foundation=2x2

[NAREFN]
Name=Soviet Ore Refinery
Cost=2000
Strength=1000
Armor=wood
Power=-50
Refinery=yes
Prerequisite=NAPOWR
Owner=Soviet
Foundation=3x3

[NAWEAP]
Name=Soviet War Factory
Cost=2000
Strength=1100
Armor=steel
Power=-50
Factory=UnitType
Prerequisite=NAREFN
Owner=Soviet
Foundation=3x3

[TESLA]
Name=Tesla Coil
Cost=1500
Strength=650
Armor=steel
Power=-75
Primary=TeslaZap
Prerequisite=NAPOWR
Owner=Soviet
Foundation=1x1

[E1]
Name=GI Squad
Cost=200
Strength=125
Armor=light
Speed=4
Sight=6
Primary=M60
Prerequisite=GAPILE
Owner=Allies

[E2]
Name=Conscript
Cost=100
Strength=110
Armor=light
Speed=4
Sight=5
Primary=AK47
Prerequisite=NAHAND
Owner=Soviet

[ENGINEER]
Name=Combat Engineer
Cost=500
Strength=80
Armor=none
Speed=4
Engineer=yes
Prerequisite=GAPILE
Owner=Allies

[MTNK]
Name=Grizzly Tank (VXL)
Cost=700
Strength=400
Armor=heavy
Speed=7
Sight=8
Primary=105mm
Prerequisite=GAWEAP
Owner=Allies

[HTNK]
Name=Rhino Heavy Tank
Cost=900
Strength=550
Armor=heavy
Speed=6
Sight=8
Primary=120mm
Prerequisite=NAWEAP
Owner=Soviet

[CMIN]
Name=Chrono Miner
Cost=1400
Strength=950
Armor=medium
Speed=5
Harvester=yes
Prerequisite=GAWEAP
Owner=Allies

[HARV]
Name=War Miner
Cost=1400
Strength=1050
Armor=heavy
Speed=5
Harvester=yes
Prerequisite=NAWEAP
Owner=Soviet

[M60]
Damage=22
ROF=20
Range=5.5
Warhead=SA

[AK47]
Damage=18
ROF=20
Range=5.0
Warhead=SA

[105mm]
Damage=65
ROF=45
Range=6.5
Warhead=AP

[120mm]
Damage=90
ROF=52
Range=6.5
Warhead=AP

[TeslaZap]
Damage=135
ROF=60
Range=8.0
Warhead=AP

[SA]
Verses=100%,85%,60%,35%,25%

[AP]
Verses=40%,75%,100%,125%,85%
"#;

    // Write both a .mix archive containing rules.ini and a sample .vxl voxel tank
    let crc = westwood_mix_crc("rules.ini");
    let payload = sample_ini.as_bytes();
    let mut mix = Vec::new();
    mix.extend_from_slice(&1u16.to_le_bytes());
    mix.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    mix.extend_from_slice(&crc.to_le_bytes());
    mix.extend_from_slice(&0u32.to_le_bytes());
    mix.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    mix.extend_from_slice(payload);
    fs::write(ww_src.join("westwood_rules.mix"), &mix)?;
    fs::write(ww_src.join("rulesmd.ini"), sample_ini)?;
    if !ww_src.join("mtnk.vxl").exists() {
        fs::write(ww_src.join("mtnk.vxl"), build_sample_vxl_bytes(6, 8, 4))?;
        fs::write(ww_src.join("mtnktur.vxl"), build_sample_vxl_bytes(4, 4, 2))?;
        fs::write(ww_src.join("mtnkbarl.vxl"), build_sample_vxl_bytes(2, 5, 1))?;
    }
    if !ww_src.join("e1.shp").exists() {
        fs::write(ww_src.join("e1.shp"), build_sample_shp_bytes(10, 14))?;
    }

    let openra_rules = r#"
^Soldier:
	RevealsShroud:
		Range: 5c0
	Armor:
		Type: Light
	Mobile:
		Speed: 56

^Tank:
	RevealsShroud:
		Range: 7c0
	Armor:
		Type: Heavy
	Mobile:
		Speed: 84

FACT:
	Buildable:
		Prerequisites: ~structures.allies
	Valued:
		Cost: 2500
	Tooltip:
		Name: Allied Construction Yard
	Building:
		Dimensions: 3,3
	BaseProvider:
	Health:
		HP: 150000
	Armor:
		Type: Concrete

SFACT:
	Buildable:
		Prerequisites: ~structures.soviet
	Valued:
		Cost: 2500
	Tooltip:
		Name: Soviet Construction Yard
	Building:
		Dimensions: 3,3
	BaseProvider:
	Health:
		HP: 150000
	Armor:
		Type: Concrete

POWR:
	Buildable:
		Prerequisites: fact, ~structures.allies
	Valued:
		Cost: 300
	Tooltip:
		Name: Allied Power Plant
	Building:
		Dimensions: 2,2
	Health:
		HP: 45000
	Power:
		Amount: 150

SPOWR:
	Buildable:
		Prerequisites: sfact, ~structures.soviet
	Valued:
		Cost: 300
	Tooltip:
		Name: Soviet Power Plant
	Building:
		Dimensions: 2,2
	Health:
		HP: 45000
	Power:
		Amount: 150

TENT:
	Buildable:
		Prerequisites: powr, ~structures.allies
	Valued:
		Cost: 500
	Tooltip:
		Name: Allied Barracks
	Building:
		Dimensions: 2,2
	Health:
		HP: 80000
	Power:
		Amount: -20
	Production:
		Produces: Infantry

BARR:
	Buildable:
		Prerequisites: spowr, ~structures.soviet
	Valued:
		Cost: 500
	Tooltip:
		Name: Soviet Barracks
	Building:
		Dimensions: 2,2
	Health:
		HP: 80000
	Power:
		Amount: -20
	Production:
		Produces: Infantry

PROC:
	Buildable:
		Prerequisites: powr, ~structures.allies
	Valued:
		Cost: 1400
	Tooltip:
		Name: Allied Ore Refinery
	Building:
		Dimensions: 3,3
	Refinery:
	Health:
		HP: 90000
	Power:
		Amount: -40

SPROC:
	Buildable:
		Prerequisites: spowr, ~structures.soviet
	Valued:
		Cost: 1400
	Tooltip:
		Name: Soviet Ore Refinery
	Building:
		Dimensions: 3,3
	Refinery:
	Health:
		HP: 90000
	Power:
		Amount: -40

WEAP:
	Buildable:
		Prerequisites: proc, ~structures.allies
	Valued:
		Cost: 2000
	Tooltip:
		Name: Allied War Factory
	Building:
		Dimensions: 3,2
	Health:
		HP: 100000
	Power:
		Amount: -40
	Production:
		Produces: Vehicle

SWEAP:
	Buildable:
		Prerequisites: sproc, ~structures.soviet
	Valued:
		Cost: 2000
	Tooltip:
		Name: Soviet War Factory
	Building:
		Dimensions: 3,2
	Health:
		HP: 100000
	Power:
		Amount: -40
	Production:
		Produces: Vehicle

TSLA:
	Buildable:
		Prerequisites: spowr, ~structures.soviet
	Valued:
		Cost: 1200
	Tooltip:
		Name: Tesla Coil
	Building:
		Dimensions: 1,1
	Health:
		HP: 60000
	Power:
		Amount: -60
	Armament:
		Weapon: PortaTesla

E1:
	Inherits: ^Soldier
	Buildable:
		Prerequisites: tent, ~infantry.allies
	Valued:
		Cost: 100
	Tooltip:
		Name: Rifle Infantry
	Health:
		HP: 12000
	Armament:
		Weapon: M1Carbine

SOV_E1:
	Inherits: ^Soldier
	Buildable:
		Prerequisites: barr, ~infantry.soviet
	Valued:
		Cost: 100
	Tooltip:
		Name: Soviet Shock Trooper
	Health:
		HP: 13000
	Armament:
		Weapon: M1Carbine

2TNK:
	Inherits: ^Tank
	Buildable:
		Prerequisites: weap, ~vehicles.allies
	Valued:
		Cost: 800
	Tooltip:
		Name: Medium Tank
	Health:
		HP: 42000
	Armament:
		Weapon: 90mm

3TNK:
	Inherits: ^Tank
	Buildable:
		Prerequisites: sweap, ~vehicles.soviet
	Valued:
		Cost: 950
	Tooltip:
		Name: Heavy Tank
	Health:
		HP: 56000
	Armament:
		Weapon: 105mm

HARV:
	Inherits: ^Tank
	Buildable:
		Prerequisites: weap, ~vehicles.allies
	Valued:
		Cost: 1100
	Tooltip:
		Name: Allied Ore Truck
	Harvester:
	Health:
		HP: 90000

SHARV:
	Inherits: ^Tank
	Buildable:
		Prerequisites: sweap, ~vehicles.soviet
	Valued:
		Cost: 1100
	Tooltip:
		Name: Soviet War Miner
	Harvester:
	Health:
		HP: 95000
"#;

    let openra_weapons = r#"
M1Carbine:
	ReloadDelay: 20
	Range: 5c0
	Warhead@1Dam: SpreadDamage
		Damage: 2000
		Versus:
			None: 100
			Light: 75
			Heavy: 25

90mm:
	ReloadDelay: 40
	Range: 6c0
	Warhead@1Dam: SpreadDamage
		Damage: 5500
		Versus:
			None: 35
			Light: 75
			Heavy: 115

105mm:
	ReloadDelay: 50
	Range: 6c512
	Warhead@1Dam: SpreadDamage
		Damage: 7500
		Versus:
			None: 35
			Light: 80
			Heavy: 120

PortaTesla:
	ReloadDelay: 60
	Range: 7c512
	Warhead@1Dam: SpreadDamage
		Damage: 12000
		Versus:
			None: 100
			Light: 100
			Heavy: 110
"#;

    let openra_map = r#"
Title: OpenRA Classic Clash
MapSize: 40,40
Actors:
	Actor0: mpspawn
		Location: 6,6
		Owner: Neutral
	Actor1: mpspawn
		Location: 32,32
		Owner: Neutral
	Actor2: mine
		Location: 12,10
		Owner: Neutral
	Actor3: mine
		Location: 26,28
		Owner: Neutral
"#;

    fs::write(ora_src.join("rules/actors.yaml"), openra_rules)?;
    fs::write(ora_src.join("weapons/weapons.yaml"), openra_weapons)?;
    fs::write(ora_src.join("maps/map.yaml"), openra_map)?;
    fs::write(
        ora_src.join("maps/map.bin"),
        build_sample_openra_map_bin(40, 40),
    )?;

    let ww_out = workspace_root.join("mods/westwood-classic");
    let ora_out = workspace_root.join("mods/openra-classic");

    let r1 = smart_import(&ww_src, Some(&ww_out))
        .context("Failed to smart-import Westwood demo pack")?;
    let r2 = smart_import(&ora_src, Some(&ora_out))
        .context("Failed to smart-import OpenRA demo pack")?;

    Ok((r1, r2))
}

/// Discovers all valid mods across workspace root and `mods/` directory.
pub fn discover_available_mods(workspace_root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let default_ra2 = workspace_root.join("rust-ra2");
    if default_ra2.join("mod.toml").exists() {
        found.push(default_ra2);
    }
    let mods_dir = workspace_root.join("mods");
    if mods_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&mods_dir) {
            let mut subdirs: Vec<PathBuf> = entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.join("mod.toml").exists())
                .collect();
            subdirs.sort();
            found.extend(subdirs);
        }
    }
    found
}
