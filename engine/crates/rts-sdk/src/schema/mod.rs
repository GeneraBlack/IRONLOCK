pub mod combat_defs;
pub mod common;
pub mod entities;
pub mod world_defs;

pub use combat_defs::*;
pub use common::*;
pub use entities::*;
pub use world_defs::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_unit_and_building_catalogs_and_computes_weapon_damage() {
        let weapon_toml = r#"
[[weapons]]
id = "cannon_105mm"
name = "105mm Tank Cannon"
damage = 65.0
range = 14.0
cooldown_secs = 2.0
projectile_speed = 35.0
armor_multipliers = { heavy = 1.2, light = 0.6, structure = 0.8 }
"#;
        let weapons: WeaponCatalog = toml::from_str(weapon_toml).expect("weapons should parse");
        assert_eq!(weapons.weapons.len(), 1);
        let cannon = &weapons.weapons[0];
        assert!((cannon.effective_damage(ArmorType::Heavy) - 78.0).abs() < 1e-4);
        assert!((cannon.effective_damage(ArmorType::Medium) - 65.0).abs() < 1e-4);

        let unit_toml = r#"
[[units]]
id = "grizzly_tank"
name = "Grizzly Battle Tank"
faction = "allies"
role = "vehicle"
cost = 700
build_time_secs = 7.0
health = 400.0
armor = "heavy"
speed = 5.5
vision_range = 18.0
primary_weapon = "cannon_105mm"
prerequisites = ["allied_war_factory"]
ability_id = "hull_down"
script_hooks = { on_ability = "on_grizzly_ability" }

[units.visual]
model = "assets/models/grizzly_tank.glb"
archetype = "tank"
scale = [1.8, 1.1, 2.4]
primary_color = [0.22, 0.45, 0.82]
"#;
        let units: UnitCatalog = toml::from_str(unit_toml).expect("units should parse");
        assert_eq!(units.units.len(), 1);
        assert_eq!(units.units[0].id, "grizzly_tank");
        assert_eq!(units.units[0].visual.archetype, VisualArchetype::Tank);
    }
}
