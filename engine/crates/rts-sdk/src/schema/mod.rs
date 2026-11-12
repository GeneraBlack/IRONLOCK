pub mod ai_behavior_tree;
pub mod combat_defs;
pub mod common;
pub mod entities;
pub mod traits;
pub mod trigger_graph;
pub mod vfx_defs;
pub mod world_defs;

pub use ai_behavior_tree::*;
pub use combat_defs::*;
pub use common::*;
pub use entities::*;
pub use traits::*;
pub use trigger_graph::*;
pub use vfx_defs::*;
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

    #[test]
    fn generates_schemars_json_schemas_for_all_root_catalogs() {
        let _ = schemars::schema_for!(crate::manifest::ModManifest);
        let _ = schemars::schema_for!(FactionCatalog);
        let _ = schemars::schema_for!(UnitCatalog);
        let _ = schemars::schema_for!(BuildingCatalog);
        let _ = schemars::schema_for!(WeaponCatalog);
        let _ = schemars::schema_for!(UpgradeCatalog);
        let _ = schemars::schema_for!(ResourceCatalog);
        let _ = schemars::schema_for!(AiProfileCatalog);
        let _ = schemars::schema_for!(CampaignCatalog);
        let _ = schemars::schema_for!(AudioCatalog);
        let _ = schemars::schema_for!(LocaleCatalog);
        let _ = schemars::schema_for!(MapDef);
        let _ = schemars::schema_for!(VfxCatalog);
        let _ = schemars::schema_for!(ItemCatalog);
    }
}
