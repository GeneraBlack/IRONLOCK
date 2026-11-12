mod converter;
mod map_reader;
mod miniyaml;

pub use converter::*;
pub use map_reader::*;
pub use miniyaml::{parse_miniyaml, parse_openra_range, MiniYamlNode};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::westwood::write_converted_bundle_to_dir;
    use rts_sdk::loader::LoadedMod;
    use rts_sdk::validator::validate_mod;
    use std::fs;

    #[test]
    fn converts_openra_miniyaml_rules_weapons_and_map_into_valid_mod() {
        let rules_yaml = r#"
^Soldier:
	RevealsShroud:
		Range: 4c512
	Armor:
		Type: None
	Mobile:
		Speed: 56

^Vehicle:
	RevealsShroud:
		Range: 6c0
	Armor:
		Type: Heavy
	Mobile:
		Speed: 85

POWR:
	Inherits: ^Building
	Buildable:
		Prerequisites: ~structures.allies, ~structures.soviet
	Valued:
		Cost: 300
	Tooltip:
		Name: Power Plant
	Building:
		Dimensions: 2,2
	Health:
		HP: 40000
	Armor:
		Type: Wood
	Power:
		Amount: 100

TENT:
	Inherits: ^Building
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

WEAP:
	Inherits: ^Building
	Buildable:
		Prerequisites: powr, ~structures.soviet
	Valued:
		Cost: 2000
	Tooltip:
		Name: War Factory
	Building:
		Dimensions: 3,2
	Health:
		HP: 100000
	Power:
		Amount: -30
	Production:
		Produces: Vehicle

E1:
	Inherits: ^Soldier
	Buildable:
		Prerequisites: tent, ~infantry.allies
	Valued:
		Cost: 100
	Tooltip:
		Name: Rifle Infantry
	Health:
		HP: 5000
	Armament:
		Weapon: M1Carbine

3TNK:
	Inherits: ^Vehicle
	Buildable:
		Prerequisites: weap, ~vehicles.soviet
	Valued:
		Cost: 950
	Tooltip:
		Name: Heavy Tank
	Health:
		HP: 40000
	Armament:
		Weapon: 105mm
"#;

        let weapons_yaml = r#"
M1Carbine:
	ReloadDelay: 20
	Range: 5c0
	Warhead@1Dam: SpreadDamage
		Damage: 1500
		Versus:
			None: 100
			Light: 60
			Heavy: 25

105mm:
	ReloadDelay: 50
	Range: 5c512
	Projectile: Bullet
		Speed: 682
	Warhead@1Dam: SpreadDamage
		Damage: 6500
		Versus:
			None: 30
			Light: 75
			Heavy: 115
"#;

        let map_yaml = r#"
Title: OpenRA Coastal Clash
MapSize: 48,48
Actors:
	Actor0: mpspawn
		Location: 8,8
		Owner: Neutral
	Actor1: mpspawn
		Location: 38,38
		Owner: Neutral
	Actor2: mine
		Location: 14,12
		Owner: Neutral
"#;

        let bundle = convert_openra_yaml_to_bundle(
            rules_yaml,
            Some(weapons_yaml),
            Some(map_yaml),
            "openra-conv",
            "OpenRA Converted Mod",
        )
        .expect("should convert OpenRA MiniYAML");

        assert_eq!(bundle.buildings.buildings.len(), 3);
        assert_eq!(bundle.units.units.len(), 2);
        assert_eq!(bundle.weapons.weapons.len(), 2);
        assert_eq!(bundle.map.name, "OpenRA Coastal Clash");
        assert_eq!(bundle.map.width, 48);

        let tank = bundle
            .units
            .units
            .iter()
            .find(|u| u.id == "id_3tnk")
            .expect("3TNK should convert to id_3tnk");
        assert_eq!(tank.name, "Heavy Tank");
        assert_eq!(tank.faction, "soviets");
        assert!((tank.health - 400.0).abs() < 1e-3);
        assert_eq!(tank.prerequisites, vec!["weap".to_string()]);

        let tmp_dir = std::env::temp_dir().join("rts_test_openra_conv");
        let _ = fs::remove_dir_all(&tmp_dir);
        write_converted_bundle_to_dir(&bundle, &tmp_dir, None).expect("should write OpenRA mod");
        let loaded = LoadedMod::load_from_dir(&tmp_dir).expect("should load OpenRA mod");
        let report = validate_mod(&loaded, true);
        assert!(
            report.is_valid(),
            "Converted OpenRA mod should validate with 0 errors: {:?}",
            report.issues
        );
        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn converts_openra_map_bin_multi_factions_and_changes_health_rhai_hooks() {
        let rules_yaml = r#"
ACNST:
	Buildable:
		Factions: atreides
	Valued:
		Cost: 2000
	Tooltip:
		Name: Atreides Construction Yard
	Building:
		Dimensions: 3,3
	BaseProvider:
	Health:
		HP: 120000

HCNST:
	Buildable:
		Factions: harkonnen
	Valued:
		Cost: 2000
	Tooltip:
		Name: Harkonnen Construction Yard
	Building:
		Dimensions: 3,3
	BaseProvider:
	Health:
		HP: 120000

OCNST:
	Buildable:
		Factions: ordos
	Valued:
		Cost: 2000
	Tooltip:
		Name: Ordos Construction Yard
	Building:
		Dimensions: 3,3
	BaseProvider:
	Health:
		HP: 120000

SONIC:
	Buildable:
		Factions: atreides
	Valued:
		Cost: 1200
	Tooltip:
		Name: Sonic Tank
	Health:
		HP: 35000
	Mobile:
		Speed: 70
	ChangesHealth:
		Step: 50
"#;
        let mut bundle =
            convert_openra_yaml_to_bundle(rules_yaml, None, None, "dune2k-test", "Dune 2000 Test")
                .expect("should convert Dune 2000 OpenRA YAML");

        let faction_ids: Vec<&str> = bundle
            .factions
            .factions
            .iter()
            .map(|f| f.id.as_str())
            .collect();
        assert_eq!(faction_ids, vec!["atreides", "harkonnen", "ordos"]);
        assert_eq!(
            bundle.manifest.scripts,
            vec!["scripts/auto_traits.rhai".to_string()]
        );

        let map_bin = build_sample_openra_map_bin(44, 44);
        parse_openra_map_bin(&map_bin, &mut bundle.map).expect("should parse map.bin");
        assert_eq!(bundle.map.width, 44);
        assert!(!bundle.map.water_rects.is_empty());
        assert_eq!(bundle.map.ore_patches.len(), 2);
    }
}
