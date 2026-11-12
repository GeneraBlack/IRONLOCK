# Rust 3D RTS Engine — Auto-Derived Mod Schema Reference (`MOD_SCHEMA_REFERENCE.md`)

> **100% Auto-Derived from `rts-sdk` Rust types via `schemars::schema_for!(...)`.**
> Regenerate schemas, `taplo.toml`, and this document with:
> `cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- gen-docs --mod-path rust-ra2`

## 1. Catalog & JSON-Schema Index

| Catalog | SDK Root Type | JSON Schema File | Taplo Glob Patterns | Summary |
| :--- | :--- | :--- | :--- | :--- |
| **`mod`** | `ModManifest` | `schemas/mod.schema.json` | `mod.toml`, `**/mod.toml` | Root mod manifest (`mod.toml`): SemVer `depends_on` DAG, HUD layout preset, `[catalogs]`, and `[rules]` (`economy_model`, `eras`, `damage_matrix`, `gamemode`). |
| **`factions`** | `FactionCatalog` | `schemas/factions.schema.json` | `factions/*.toml`, `factions.toml`, `**/factions/*.toml`, `**/factions.toml` | Playable factions, sub-factions, starting credits/units, and HUD skin themes (`factions/*.toml`). |
| **`units`** | `UnitCatalog` | `schemas/units.schema.json` | `units/*.toml`, `units.toml`, `**/units/*.toml`, `**/units.toml` | Mobile & air/naval unit definitions, composable trait specs, `hero_trait`, `energy_pool`, abilities, and PBR/KTX2 `VisualDef` (`units/*.toml`). |
| **`buildings`** | `BuildingCatalog` | `schemas/buildings.schema.json` | `buildings/*.toml`, `buildings.toml`, `**/buildings/*.toml`, `**/buildings.toml` | Structures, base defenses, superweapons, streaming build power, era gates, and support powers (`buildings/*.toml`). |
| **`weapons`** | `WeaponCatalog` | `schemas/weapons.schema.json` | `weapons/*.toml`, `weapons.toml`, `**/weapons/*.toml`, `**/weapons.toml` | Weapons, `DeliveryKind` (`instant`, `ballistic`, `missile`, `beam`, `cone`), warhead tags, status effects, and VFX bindings (`weapons/*.toml`). |
| **`upgrades`** | `UpgradeCatalog` | `schemas/upgrades.schema.json` | `upgrades/*.toml`, `upgrades.toml`, `**/upgrades/*.toml`, `**/upgrades.toml` | Tech-tree research upgrades, era transitions (`advances_to_era`), tag/def stat modifiers, and commander doctrines (`upgrades/*.toml`). |
| **`resources`** | `ResourceCatalog` | `schemas/resources.schema.json` | `resources/*.toml`, `resources.toml`, `**/resources/*.toml`, `**/resources.toml` | Custom economy resource types, gather rates, storage caps, and HUD badge colors (`resources/*.toml`). |
| **`ai_profiles`** | `AiProfileCatalog` | `schemas/ai_profiles.schema.json` | `ai/*.toml`, `ai_profiles.toml`, `**/ai/*.toml`, `**/ai_profiles.toml` | Skirmish & campaign AI personalities, build orders, unit weights, and modular `BehaviorTreeDef` (`ai/*.toml`). |
| **`campaigns`** | `CampaignCatalog` | `schemas/campaigns.schema.json` | `campaigns/*.toml`, `campaigns.toml`, `**/campaigns/*.toml`, `**/campaigns.toml` | Singleplayer & co-op campaign missions, briefings, difficulty modifiers, and `trigger_graphs` (`campaigns/*.toml`). |
| **`audio`** | `AudioCatalog` | `schemas/audio.schema.json` | `audio/*.toml`, `audio.toml`, `**/audio/*.toml`, `**/audio.toml` | Weapon SFX, unit voice banks, EVA announcer cues, and adaptive music tracks (`audio/*.toml`). |
| **`locales`** | `LocaleCatalog` | `schemas/locales.schema.json` | `locales/*.toml`, `**/locales/*.toml` | Localized UI, unit, objective, and EVA string tables (`locales/*.toml`). |
| **`maps`** | `MapDef` | `schemas/maps.schema.json` | `maps/*.toml`, `**/maps/*.toml` | Skirmish & mission maps: terrain/water/elevation rects, spawns, `capture_zones`, `item_drops`, `trigger_zones`, and `trigger_graphs` (`maps/*.toml`). |
| **`vfx`** | `VfxCatalog` | `schemas/vfx.schema.json` | `vfx/*.toml`, `vfx.toml`, `**/vfx/*.toml`, `**/vfx.toml` | Declarative particle emitters, flipbook/curve parameters, and ground decal links (`vfx/*.toml`). |
| **`items`** | `ItemCatalog` | `schemas/items.schema.json` | `items/*.toml`, `items.toml`, `**/items/*.toml`, `**/items.toml` | Equippable & consumable hero inventory items, passive stat bonuses, and active item abilities (`items/*.toml`). |

## 2. Root Catalog Schemas

### 2.1 `mod` (`schemas/mod.schema.json` — `ModManifest`)

Root mod manifest (`mod.toml`): SemVer `depends_on` DAG, HUD layout preset, `[catalogs]`, and `[rules]` (`economy_model`, `eras`, `damage_matrix`, `gamemode`).

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `authors` | `Vec<string>` | no | `[]` |  |
| `catalogs` | `ModCatalogPaths` | no | `{...}` |  |
| `default_hud_layout` | `HudLayoutPreset` | no | `"classic_right_sidebar"` |  |
| `depends_on` | `Vec<ModDependency>` | no | `[]` |  |
| `description` | `string` | no | `""` |  |
| `engine_version` | `string` | yes | — |  |
| `entry_map` | `string` | no | `""` |  |
| `extends` | `string` \| `null` | no | `None` |  |
| `id` | `string` | yes | — |  |
| `name` | `string` | yes | — |  |
| `rules` | `ModRules` | no | `{...}` |  |
| `scripts` | `Vec<string>` | no | `[]` |  |
| `version` | `string` | yes | — |  |

### 2.2 `factions` (`schemas/factions.schema.json` — `FactionCatalog`)

Playable factions, sub-factions, starting credits/units, and HUD skin themes (`factions/*.toml`).

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `factions` | `Vec<FactionDef>` | yes | — |  |

### 2.3 `units` (`schemas/units.schema.json` — `UnitCatalog`)

Mobile & air/naval unit definitions, composable trait specs, `hero_trait`, `energy_pool`, abilities, and PBR/KTX2 `VisualDef` (`units/*.toml`).

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `units` | `Vec<UnitDef>` | yes | — |  |

### 2.4 `buildings` (`schemas/buildings.schema.json` — `BuildingCatalog`)

Structures, base defenses, superweapons, streaming build power, era gates, and support powers (`buildings/*.toml`).

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `buildings` | `Vec<BuildingDef>` | yes | — |  |

### 2.5 `weapons` (`schemas/weapons.schema.json` — `WeaponCatalog`)

Weapons, `DeliveryKind` (`instant`, `ballistic`, `missile`, `beam`, `cone`), warhead tags, status effects, and VFX bindings (`weapons/*.toml`).

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `weapons` | `Vec<WeaponDef>` | yes | — |  |

### 2.6 `upgrades` (`schemas/upgrades.schema.json` — `UpgradeCatalog`)

Tech-tree research upgrades, era transitions (`advances_to_era`), tag/def stat modifiers, and commander doctrines (`upgrades/*.toml`).

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `commander_trees` | `Vec<CommanderPowerTreeDef>` | no | `[]` |  |
| `items` | `Vec<ItemDef>` | no | `[]` |  |
| `upgrades` | `Vec<UpgradeDef>` | no | `[]` |  |

### 2.7 `resources` (`schemas/resources.schema.json` — `ResourceCatalog`)

Custom economy resource types, gather rates, storage caps, and HUD badge colors (`resources/*.toml`).

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `resources` | `Vec<ResourceTypeDef>` | no | `[]` |  |

### 2.8 `ai_profiles` (`schemas/ai_profiles.schema.json` — `AiProfileCatalog`)

Skirmish & campaign AI personalities, build orders, unit weights, and modular `BehaviorTreeDef` (`ai/*.toml`).

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `profiles` | `Vec<AiProfileDef>` | no | `[]` |  |

### 2.9 `campaigns` (`schemas/campaigns.schema.json` — `CampaignCatalog`)

Singleplayer & co-op campaign missions, briefings, difficulty modifiers, and `trigger_graphs` (`campaigns/*.toml`).

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `campaigns` | `Vec<CampaignDef>` | no | `[]` |  |

### 2.10 `audio` (`schemas/audio.schema.json` — `AudioCatalog`)

Weapon SFX, unit voice banks, EVA announcer cues, and adaptive music tracks (`audio/*.toml`).

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `eva_cues` | `Map<String, string>` | no | `{}` |  |
| `faction_eva_cues` | `Map<String, Map<String, string>>` | no | `{}` |  |
| `music_tracks` | `Vec<MusicTrackDef>` | no | `[]` |  |
| `unit_voices` | `Map<String, UnitVoiceSpec>` | no | `{}` |  |
| `weapon_sfx` | `Map<String, string>` | no | `{}` |  |

### 2.11 `locales` (`schemas/locales.schema.json` — `LocaleCatalog`)

Localized UI, unit, objective, and EVA string tables (`locales/*.toml`).

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `language` | `string` | no | `""` |  |
| `strings` | `Map<String, string>` | no | `{}` |  |

### 2.12 `maps` (`schemas/maps.schema.json` — `MapDef`)

Skirmish & mission maps: terrain/water/elevation rects, spawns, `capture_zones`, `item_drops`, `trigger_zones`, and `trigger_graphs` (`maps/*.toml`).

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `bridge_rects` | `Vec<ObstacleRectDef>` | no | `[]` |  |
| `capture_zones` | `Vec<CaptureZoneDef>` | no | `[]` |  |
| `civilian_structures` | `Vec<CivilianStructureDef>` | no | `[]` |  |
| `cutscenes` | `Vec<CameraCutsceneDef>` | no | `[]` |  |
| `elevation_rects` | `Vec<ObstacleRectDef>` | no | `[]` |  |
| `gem_patches` | `Vec<OrePatchDef>` | no | `[]` |  |
| `height` | `integer` | yes | — |  |
| `heightmap` | `HeightmapDef` \| null | no | `None` |  |
| `id` | `string` | yes | — |  |
| `initial_weather` | `string` \| `null` | no | `None` |  |
| `item_drops` | `Vec<PreplacedItemDropDef>` | no | `[]` |  |
| `mission_objective` | `string` \| `null` | no | `None` |  |
| `name` | `string` | yes | — |  |
| `objectives` | `Vec<MissionObjectiveDef>` | no | `[]` |  |
| `obstacles` | `Vec<ObstacleRectDef>` | no | `[]` |  |
| `ore_patches` | `Vec<OrePatchDef>` | no | `[]` |  |
| `ore_regen_per_sec` | `number` | no | `0.0` |  |
| `playable_bounds` | `array` \| `null` | no | `None` |  |
| `preplaced_entities` | `Vec<PreplacedEntityDef>` | no | `[]` |  |
| `props` | `Vec<MapPropDef>` | no | `[]` |  |
| `ramp_rects` | `Vec<ObstacleRectDef>` | no | `[]` |  |
| `resource_patches` | `Vec<ResourcePatchDef>` | no | `[]` |  |
| `spawns` | `Vec<PlayerSpawnDef>` | no | `[]` |  |
| `splatmap` | `TerrainSplatDef` \| null | no | `None` |  |
| `tile_size` | `number` | no | `2.0` |  |
| `trigger_graphs` | `Vec<TriggerGraphDef>` | no | `[]` |  |
| `trigger_zones` | `Vec<TriggerZoneDef>` | no | `[]` |  |
| `water_rects` | `Vec<ObstacleRectDef>` | no | `[]` |  |
| `width` | `integer` | yes | — |  |

### 2.13 `vfx` (`schemas/vfx.schema.json` — `VfxCatalog`)

Declarative particle emitters, flipbook/curve parameters, and ground decal links (`vfx/*.toml`).

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `effects` | `Vec<VfxEffectDef>` | no | `[]` |  |

### 2.14 `items` (`schemas/items.schema.json` — `ItemCatalog`)

Equippable & consumable hero inventory items, passive stat bonuses, and active item abilities (`items/*.toml`).

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `items` | `Vec<ItemDef>` | no | `[]` |  |

## 3. Shared SDK Struct & Enum Definitions

### `AbilityDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `allow_auto_cast` | `boolean` | no | `false` |  |
| `applies_status` | `StatusEffectDef` \| null | no | `None` |  |
| `cast_vfx_id` | `string` \| `null` | no | `None` |  |
| `cooldown_reduction_per_rank` | `number` | no | `0.0` |  |
| `cooldown_secs` | `number` | no | `0.0` |  |
| `cost` | `integer` | no | `0` |  |
| `default_auto_cast` | `boolean` | no | `false` |  |
| `energy_cost` | `number` | no | `0.0` |  |
| `id` | `string` | yes | — |  |
| `impact_vfx_id` | `string` \| `null` | no | `None` |  |
| `max_rank` | `integer` | no | `1` |  |
| `name` | `string` | yes | — |  |
| `payloads` | `Vec<EffectPayload>` | no | `[]` |  |
| `radius` | `number` | no | `0.0` |  |
| `range` | `number` | no | `0.0` |  |
| `script_hook` | `string` \| `null` | no | `None` |  |
| `target_mode` | `AbilityTargetMode` | no | `"self_only"` |  |
| `value_scale_per_rank` | `number` | no | `0.0` |  |

### `AbilityTargetMode`

- **Enum Values:** `"self_only"`, `"single_entity"`, `"ground_point"`, `"area_radius"`

### `AiBtActionKind`

- **Variants:** `action = "execute_build_order"`, `action = "train_unit"`, `action = "construct_building"`, `action = "research_upgrade"`, `action = "advance_era"`, `action = "expand_outpost"`, `action = "launch_attack_wave"`, `action = "launch_amphibious_wave"`, `action = "cast_superweapon"`, `action = "retreat_wounded"`, `action = "defend_base"`, `action = "capture_control_point"`, `action = "use_hero_skills"`

### `AiProfileDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `amphibious_assault_enabled` | `boolean` \| `null` | no | `None` |  |
| `behavior_tree` | `BehaviorTreeDef` \| null | no | `None` |  |
| `build_order` | `Vec<string>` | no | `[]` |  |
| `expand_at_credits` | `integer` \| `null` | no | `None` |  |
| `faction` | `string` | yes | — |  |
| `id` | `string` | yes | — |  |
| `min_attack_wave_size` | `integer` \| `null` | no | `None` |  |
| `on_ai_tick` | `string` \| `null` | no | `None` |  |
| `personality` | `string` | no | `"balanced"` |  |
| `personality_id` | `string` \| `null` | no | `None` |  |
| `preferred_superweapons` | `Vec<string>` | no | `[]` |  |
| `superweapon_doctrine` | `string` \| `null` | no | `None` |  |
| `target_priority_weights` | `Map<String, number>` | no | `{}` |  |
| `unit_role_quotas` | `Map<String, number>` | no | `{}` |  |
| `unit_weights` | `Map<String, integer>` | no | `{}` |  |

### `AnimClipMap`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `attack` | `string` \| `null` | no | `None` |  |
| `constructing` | `string` \| `null` | no | `None` |  |
| `damaged` | `string` \| `null` | no | `None` |  |
| `deploy` | `string` \| `null` | no | `None` |  |
| `die` | `string` \| `null` | no | `None` |  |
| `idle` | `string` \| `null` | no | `None` |  |
| `producing` | `string` \| `null` | no | `None` |  |
| `prone` | `string` \| `null` | no | `None` |  |
| `walk` | `string` \| `null` | no | `None` |  |

### `ArmorType`

- **Enum Values:** `"none"`, `"light"`, `"medium"`, `"heavy"`, `"structure"`

### `BallisticSpec`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `collides_with_structures` | `boolean` | no | `false` |  |
| `collides_with_terrain` | `boolean` | no | `true` |  |
| `cone_half_angle_deg` | `number` | no | `0.0` |  |
| `gravity` | `number` | no | `0.0` |  |
| `lead_factor` | `number` | no | `1.0` |  |
| `moving_scatter_multiplier` | `number` | no | `1.0` |  |
| `penetrating` | `boolean` | no | `false` |  |
| `scatter_radius` | `number` | no | `0.0` |  |
| `suppression_amount` | `number` | no | `0.0` |  |
| `turn_rate_deg_per_sec` | `number` | no | `0.0` |  |

### `BehaviorTreeDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `id` | `string` | yes | — |  |
| `name` | `string` | no | `""` |  |
| `nodes` | `Vec<BtNode>` | no | `[]` |  |
| `root_node` | `string` | yes | — |  |

### `BlackboardConditionKind`

- **Variants:** `check = "credits_above"`, `check = "credits_below"`, `check = "resource_above"`, `check = "under_attack"`, `check = "army_count_at_least"`, `check = "enemy_air_ratio_above"`, `check = "era_at_least"`, `check = "has_era"`, `check = "has_upgrade"`, `check = "has_building"`, `check = "supply_ratio_above"`, `check = "hero_alive"`, `check = "blackboard_key_at_least"`

### `BtNode`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `base_utility` | `number` | no | `1.0` |  |
| `canvas_pos` | `Vec<number>` | no | `[0.0,0.0]` |  |
| `comment` | `string` | no | `""` |  |
| `id` | `string` | yes | — |  |
| `label` | `string` | no | `""` |  |
| `node_kind` | `BtNodeKind` | yes | — |  |

### `BtNodeKind`

- **Variants:** `kind = "selector"`, `kind = "sequence"`, `kind = "utility_selector"`, `kind = "inverter"`, `kind = "cooldown"`, `kind = "blackboard_condition"`, `kind = "ai_action"`

### `BuffModifierSpec`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `armor_mult` | `number` | no | `1.0` |  |
| `build_speed_mult` | `number` | no | `1.0` |  |
| `damage_mult` | `number` | no | `1.0` |  |
| `dispel_category` | `string` \| `null` | no | `None` |  |
| `fire_rate_mult` | `number` | no | `1.0` |  |
| `hp_regen_per_sec` | `number` | no | `0.0` |  |
| `max_stacks` | `integer` | no | `1` |  |
| `range_bonus` | `number` | no | `0.0` |  |
| `speed_mult` | `number` | no | `1.0` |  |
| `stack_policy` | `StackPolicy` | no | `"refresh"` |  |
| `vision_mult` | `number` | no | `1.0` |  |

### `BuilderTraitSpec`

Composable trait enabling a worker/dozer unit to construct buildings on-site and repair structures.

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `build_range` | `number` | no | `3.0` |  |
| `build_speed_multiplier` | `number` | no | `1.0` |  |
| `buildable_buildings` | `Vec<string>` | no | `[]` | Building IDs this worker can construct (`[]` allows all buildings available to its faction). |
| `can_repair` | `boolean` | no | `true` |  |
| `consumed_on_complete` | `boolean` | no | `false` |  |
| `repair_hp_per_sec` | `number` | no | `35.0` |  |

### `BuildingDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `abilities` | `Vec<AbilityDef>` | no | `[]` |  |
| `addon_sockets` | `Vec<string>` | no | `[]` |  |
| `airfield_pads` | `integer` | no | `0` |  |
| `ambient_vfx_id` | `string` \| `null` | no | `None` |  |
| `armor` | `ArmorType` | no | `"light"` |  |
| `armor_tag` | `string` \| `null` | no | `None` |  |
| `build_limit` | `integer` | no | `0` |  |
| `build_time_secs` | `number` | yes | — |  |
| `builder_trait` | `BuilderTraitSpec` \| null | no | `None` |  |
| `capture_cash_bonus` | `integer` | no | `0` |  |
| `cloning_vats` | `boolean` | no | `false` |  |
| `container` | `ContainerSpec` | no | `{...}` |  |
| `cost` | `integer` | yes | — |  |
| `crushable` | `boolean` | no | `false` |  |
| `death_effect` | `DeathPayloadSpec` | no | `{...}` |  |
| `death_vfx_id` | `string` \| `null` | no | `None` |  |
| `energy_pool` | `EnergyPoolSpec` \| null | no | `None` |  |
| `faction` | `string` | yes | — |  |
| `footprint` | `Vec<integer>` | no | `[2,2]` |  |
| `gatherer_trait` | `GathererTraitSpec` \| null | no | `None` |  |
| `grinder` | `boolean` | no | `false` |  |
| `health` | `number` | yes | — |  |
| `id` | `string` | yes | — |  |
| `leaves_ruin` | `boolean` | no | `false` |  |
| `name` | `string` | yes | — |  |
| `point_defense` | `PointDefenseSpec` | no | `{...}` |  |
| `power_delta` | `integer` | no | `0` |  |
| `prerequisite_upgrades` | `Vec<string>` | no | `[]` |  |
| `prerequisites` | `Vec<string>` | no | `[]` |  |
| `primary_weapon` | `string` \| `null` | no | `None` |  |
| `required_era` | `string` \| `null` | no | `None` |  |
| `resource_costs` | `Map<String, integer>` | no | `{}` |  |
| `resource_trickle_per_sec` | `Map<String, integer>` | no | `{}` |  |
| `role` | `BuildingRole` | yes | — |  |
| `ruin_cover_bonus` | `number` | no | `0.0` |  |
| `script_hooks` | `Map<String, string>` | no | `{}` |  |
| `spawns_unit_on_complete` | `string` \| `null` | no | `None` |  |
| `squad_trait` | `SquadTraitSpec` \| null | no | `None` |  |
| `stealth` | `StealthSensorSpec` | no | `{...}` |  |
| `stolen_tech_units` | `Vec<string>` | no | `[]` |  |
| `streaming_build_power` | `number` \| `null` | no | `None` |  |
| `sub_factions` | `Vec<string>` | no | `[]` |  |
| `superweapon` | `SuperweaponType` \| null | no | `None` |  |
| `supply_provided` | `integer` | no | `0` |  |
| `support_aura` | `SupportAuraSpec` | no | `{...}` |  |
| `support_powers` | `Vec<SupportPowerDef>` | no | `[]` |  |
| `tags` | `Vec<string>` | no | `[]` |  |
| `transforms_into_unit` | `string` \| `null` | no | `None` |  |
| `unlocks_units` | `Vec<string>` | no | `[]` |  |
| `upgrade_cost` | `integer` | no | `0` |  |
| `upgrade_options` | `Vec<string>` | no | `[]` |  |
| `upgrade_time_secs` | `number` | no | `0.0` |  |
| `upgrades_to` | `string` \| `null` | no | `None` |  |
| `vision_range` | `number` | no | `16.0` |  |
| `visual` | `VisualDef` | yes | — |  |
| `water_bound` | `boolean` | no | `false` |  |

### `BuildingRole`

- **Enum Values:** `"construction_yard"`, `"power_plant"`, `"barracks"`, `"refinery"`, `"war_factory"`, `"defense"`, `"tech_center"`, `"wall"`, `"gate"`, `"bridge"`, `"bridge_repair_hut"`, `"airfield"`, `"service_depot"`, `"support"`

### `CameraCutsceneDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `id` | `string` | yes | — |  |
| `keyframes` | `Vec<CameraKeyframe>` | no | `[]` |  |
| `letterbox` | `boolean` | no | `true` |  |
| `skippable` | `boolean` | no | `true` |  |

### `CameraKeyframe`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `distance` | `number` | no | `38.0` |  |
| `focus` | `Vec<number>` | no | `[0.0,0.0,0.0]` |  |
| `pitch_deg` | `number` | no | `55.0` |  |
| `time_secs` | `number` | no | `0.0` |  |
| `yaw_deg` | `number` | no | `0.0` |  |

### `CampaignDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `description` | `string` | no | `""` |  |
| `display_name` | `string` | yes | — |  |
| `faction` | `string` | yes | — |  |
| `id` | `string` | yes | — |  |
| `missions` | `Vec<MissionDef>` | no | `[]` |  |

### `CaptureZoneDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `capture_time_secs` | `number` | no | `8.0` |  |
| `id` | `string` | yes | — |  |
| `initial_owner` | `integer` \| `null` | no | `None` |  |
| `radius` | `number` | no | `5.0` |  |
| `tile_pos` | `Vec<integer>` | yes | — |  |

### `CivilianStructureDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `cover_bonus` | `number` | no | `0.4000000059604645` |  |
| `footprint` | `Vec<integer>` | no | `[2,2]` |  |
| `id` | `string` | yes | — |  |
| `leaves_ruin` | `boolean` | no | `true` |  |
| `max_health` | `number` | no | `600.0` |  |
| `tile_pos` | `Vec<integer>` | yes | — |  |

### `CommanderPowerNodeDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `cost_points` | `integer` | no | `0` |  |
| `description` | `string` | no | `""` |  |
| `display_name` | `string` | yes | — |  |
| `grants_support_power` | `string` \| `null` | no | `None` |  |
| `grants_upgrade` | `string` \| `null` | no | `None` |  |
| `icon` | `string` \| `null` | no | `None` |  |
| `id` | `string` | yes | — |  |
| `prerequisites` | `Vec<string>` | no | `[]` |  |
| `required_rank` | `integer` | no | `0` |  |
| `unlocks_def_id` | `string` \| `null` | no | `None` |  |

### `CommanderPowerTreeDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `display_name` | `string` | yes | — |  |
| `faction` | `string` | no | `"any"` |  |
| `id` | `string` | yes | — |  |
| `nodes` | `Vec<CommanderPowerNodeDef>` | no | `[]` |  |

### `ContainerSpec`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `accepts_vehicles` | `boolean` | no | `false` |  |
| `adapts_weapon_to_passenger` | `boolean` | no | `false` |  |
| `capacity` | `integer` | no | `0` |  |
| `engineer_repair_aura_per_sec` | `number` | no | `0.0` |  |
| `passengers_can_fire` | `boolean` | no | `false` |  |
| `power_per_occupant` | `integer` | no | `0` |  |
| `tunnel_network` | `boolean` | no | `false` |  |

### `DamageMatrixDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `warhead_vs_armor` | `Map<String, Map<String, number>>` | no | `{}` |  |

### `DeathPayloadSpec`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `drops_salvage` | `boolean` | no | `false` |  |
| `explosion_damage` | `number` | no | `0.0` |  |
| `explosion_radius` | `number` | no | `0.0` |  |
| `leaves_radiation` | `boolean` | no | `false` |  |
| `survivor_chance` | `number` | no | `0.0` |  |
| `survivor_unit` | `string` \| `null` | no | `None` |  |

### `DeliveryKind`

- **Enum Values:** `"instant"`, `"ballistic"`, `"missile"`, `"beam"`, `"cone"`

### `EconomyModel`

- **Enum Values:** `"lump_sum"`, `"streaming"`

### `EffectPayload`

- **Variants:** `kind = "spawn_units"`, `kind = "reveal_area"`, `kind = "area_damage"`, `kind = "spawn_hazard_zone"`, `kind = "apply_area_status"`, `kind = "teleport_units"`, `kind = "mutate_organic_infantry"`, `kind = "mind_control_wave"`, `kind = "unload_passengers"`

### `EnergyPoolSpec`

Regenerating mana/energy pool consumed by abilities with `energy_cost > 0`.

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `initial_energy` | `number` | no | `100.0` |  |
| `max_energy` | `number` | no | `100.0` |  |
| `regen_per_sec` | `number` | no | `2.5` |  |

### `EraDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `cost` | `integer` | no | `0` |  |
| `display_name_key` | `string` | no | `""` |  |
| `id` | `string` | yes | — |  |
| `order` | `integer` | no | `0` |  |
| `prerequisite_buildings` | `Vec<string>` | no | `[]` |  |
| `prerequisite_upgrades` | `Vec<string>` | no | `[]` |  |
| `research_time_secs` | `number` | no | `0.0` |  |
| `resource_costs` | `Map<String, integer>` | no | `{}` |  |
| `unlocks_tech_tier` | `integer` | no | `0` |  |

### `FactionDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `campaign_loading_screens` | `Vec<string>` | no | `[]` |  |
| `color` | `Vec<number>` | yes | — |  |
| `description` | `string` | no | `""` |  |
| `hud_skin` | `FactionHudSkinDef` | no | `{...}` |  |
| `id` | `string` | yes | — |  |
| `loading_screen` | `string` \| `null` | no | `None` |  |
| `name` | `string` | yes | — |  |
| `starting_credits` | `integer` | yes | — |  |
| `sub_factions` | `Vec<SubFactionDef>` | no | `[]` |  |

### `FactionHudSkinDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `addon_panel` | `string` \| `null` | no | `None` |  |
| `cameo_footer_frame` | `string` \| `null` | no | `None` |  |
| `cameo_row_frame` | `string` \| `null` | no | `None` |  |
| `credits_panel` | `string` \| `null` | no | `None` |  |
| `dialog_background` | `string` \| `null` | no | `None` |  |
| `diplomacy_button` | `string` \| `null` | no | `None` |  |
| `options_button` | `string` \| `null` | no | `None` |  |
| `power_bar_icon` | `string` \| `null` | no | `None` |  |
| `radar_frame` | `string` \| `null` | no | `None` |  |
| `repair_button` | `string` \| `null` | no | `None` |  |
| `sell_button` | `string` \| `null` | no | `None` |  |
| `side_header` | `string` \| `null` | no | `None` |  |
| `tab_icons` | `Vec<string>` | no | `[]` |  |
| `top_panel` | `string` \| `null` | no | `None` |  |

### `GamemodeRulesDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `annihilation_enabled` | `boolean` | no | `true` |  |
| `capture_zones_to_bleed` | `integer` | no | `1` |  |
| `initial_tickets` | `integer` | no | `500` |  |
| `multi_capture_points` | `boolean` | no | `false` |  |
| `survival_waves` | `Vec<SurvivalWaveDef>` | no | `[]` |  |
| `ticket_bleed_per_sec` | `number` | no | `2.0` |  |
| `victory_ticket_bleed` | `boolean` | no | `false` |  |

### `GathererTraitSpec`

Composable trait enabling a worker/harvester unit to gather one or more named resource types.

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `allowed_resources` | `Vec<string>` | no | `["credits"]` |  |
| `carry_capacity` | `integer` | no | `500` |  |
| `dropoff_buildings` | `Vec<string>` | no | `[]` | Optional explicit drop-off building IDs (`[]` falls back to `Refinery`/`ConstructionYard` buildings). |
| `gather_rate_per_sec` | `number` | no | `50.0` |  |

### `HazardZoneSpec`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `dps` | `number` | no | `18.0` |  |
| `duration_secs` | `number` | no | `6.0` |  |
| `is_toxic_cloud` | `boolean` | no | `false` |  |
| `radius` | `number` | no | `3.0` |  |

### `HeightmapDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `height` | `integer` | yes | — |  |
| `heights` | `Vec<number>` | no | `[]` |  |
| `width` | `integer` | yes | — |  |

### `HeroTraitSpec`

Composable WC3 / Dawn-of-War style Hero trait with level progression, skill points, altar revival, and multi-slot item inventory.

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `damage_mult_per_level` | `number` | no | `0.07999999821186066` |  |
| `energy_per_level` | `number` | no | `20.0` |  |
| `hp_per_level` | `number` | no | `65.0` |  |
| `inventory_slots` | `integer` | no | `6` |  |
| `learnable_abilities` | `Vec<string>` | no | `[]` |  |
| `max_level` | `integer` | no | `10` |  |
| `revive_at_building_tags` | `Vec<string>` | no | `[]` |  |
| `revive_cost_mult` | `number` | no | `0.5` |  |
| `revive_time_per_level_secs` | `number` | no | `4.0` |  |
| `xp_curve` | `Vec<number>` | no | `{...}` |  |
| `xp_share_radius` | `number` | no | `24.0` |  |

### `HudLayoutPreset`

- **Enum Values:** `"classic_right_sidebar"`, `"modern_bottom_command_card"`, `"compact_bar_overlay"`

### `InitialBuildingSpawn`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `building_id` | `string` | yes | — |  |
| `tile_pos` | `Vec<integer>` | yes | — |  |

### `InitialUnitSpawn`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `tile_pos` | `Vec<integer>` | yes | — |  |
| `unit_id` | `string` | yes | — |  |

### `ItemDef`

Equippable or consumable inventory item definition (`items/items.toml`).

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `active_ability` | `AbilityDef` \| null | no | `None` |  |
| `bonus_max_energy` | `number` | no | `0.0` |  |
| `bonus_max_health` | `number` | no | `0.0` |  |
| `consumable` | `boolean` | no | `false` |  |
| `cost` | `integer` | no | `0` |  |
| `description` | `string` | no | `""` |  |
| `icon` | `string` \| `null` | no | `None` |  |
| `id` | `string` | yes | — |  |
| `max_charges` | `integer` | no | `1` |  |
| `name` | `string` | yes | — |  |
| `passive_modifiers` | `BuffModifierSpec` | no | `{...}` |  |
| `visual_model` | `string` \| `null` | no | `None` |  |

### `LocomotorKind`

- **Enum Values:** `"infantry"`, `"wheeled"`, `"tracked"`, `"hover"`, `"naval"`, `"fixed_wing_aircraft"`, `"vtol"`

### `LocomotorSpec`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `accel` | `number` | no | `0.0` |  |
| `attack_run_radius` | `number` | no | `0.0` |  |
| `decel` | `number` | no | `0.0` |  |
| `kind` | `LocomotorKind` \| null | no | `None` |  |
| `turn_rate_deg_per_sec` | `number` | no | `0.0` |  |
| `turret_slew_deg_per_sec` | `number` | no | `0.0` |  |

### `LogicGateOp`

- **Variants:** `op = "and"`, `op = "or"`, `op = "not"`, `op = "counter_threshold"`

### `MapPropDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `kind` | `PropKind` | yes | — |  |
| `tile_pos` | `Vec<integer>` | yes | — |  |

### `MissionDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `briefing` | `string` | no | `""` |  |
| `carry_over_credits_ratio` | `number` | no | `0.0` |  |
| `carry_over_max_veteran_units` | `integer` | no | `0` |  |
| `display_name` | `string` | yes | — |  |
| `id` | `string` | yes | — |  |
| `intro_video` | `string` \| `null` | no | `None` |  |
| `loading_screen` | `string` \| `null` | no | `None` |  |
| `map_id` | `string` | yes | — |  |
| `next_mission_id` | `string` \| `null` | no | `None` |  |
| `radar_video` | `string` \| `null` | no | `None` |  |
| `trigger_graphs` | `Vec<TriggerGraphDef>` | no | `[]` |  |
| `victory_video` | `string` \| `null` | no | `None` |  |

### `MissionObjectiveDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `description` | `string` \| `null` | no | `None` |  |
| `id` | `string` | yes | — |  |
| `marker_tile` | `array` \| `null` | no | `None` |  |
| `target_count` | `integer` | no | `0` |  |
| `tier` | `ObjectiveTier` | no | `"primary"` |  |
| `title` | `string` | yes | — |  |

### `ModCatalogPaths`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `buildings` | `string` | no | `""` |  |
| `factions` | `string` | no | `""` |  |
| `items` | `string` | no | `""` |  |
| `units` | `string` | no | `""` |  |
| `vfx` | `string` | no | `""` |  |
| `weapons` | `string` | no | `""` |  |

### `ModDependency`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `id` | `string` | yes | — |  |
| `optional` | `boolean` | no | `false` |  |
| `version_req` | `string` | no | `"*"` |  |

### `ModRules`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `building_repair_credits_per_cycle` | `integer` | no | `2` |  |
| `building_repair_hp_per_sec` | `number` | no | `28.0` |  |
| `commander_rank_xp_thresholds` | `Vec<number>` | no | `[150.0,500.0,1100.0,2000.0]` |  |
| `crate_max_active` | `integer` | no | `8` |  |
| `crate_spawn_interval_ticks` | `integer` | no | `600` |  |
| `damage_matrix` | `DamageMatrixDef` | no | `{"warhead_vs_armor":{}}` |  |
| `default_infantry_by_faction` | `Map<String, string>` | no | `{}` |  |
| `default_liberated_worker_unit` | `string` | no | `""` |  |
| `default_mutant_unit` | `string` | no | `""` |  |
| `economy_model` | `EconomyModel` | no | `"lump_sum"` |  |
| `elite_xp_threshold` | `number` | no | `550.0` |  |
| `eras` | `Vec<EraDef>` | no | `[]` |  |
| `free_unit_by_faction` | `Map<String, string>` | no | `{}` |  |
| `gamemode` | `GamemodeRulesDef` | no | `{...}` |  |
| `low_power_speed_factor` | `number` | no | `0.5` |  |
| `sell_refund_ratio` | `number` | no | `0.5` |  |
| `stolen_tech_commando_by_pair` | `Map<String, string>` | no | `{}` |  |
| `veteran_xp_threshold` | `number` | no | `220.0` |  |

### `MovementDomain`

- **Enum Values:** `"ground"`, `"air"`, `"naval"`, `"amphibious"`, `"jumpjet"`, `"chrono"`

### `MusicMood`

- **Enum Values:** `"peace"`, `"combat"`, `"menu"`, `"score"`

### `MusicTrackDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `display_name` | `string` | yes | — |  |
| `duration_hint_secs` | `number` \| `null` | no | `None` |  |
| `faction_tags` | `Vec<string>` | no | `[]` |  |
| `factions` | `Vec<string>` | no | `[]` |  |
| `id` | `string` | yes | — |  |
| `mood` | `MusicMood` | no | `"peace"` |  |
| `path` | `string` | yes | — |  |

### `ObjectiveStatus`

- **Enum Values:** `"pending"`, `"active"`, `"completed"`, `"failed"`

### `ObjectiveTier`

- **Enum Values:** `"primary"`, `"secondary"`, `"bonus"`

### `ObstacleRectDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `max` | `Vec<integer>` | yes | — |  |
| `min` | `Vec<integer>` | yes | — |  |

### `OrePatchDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `amount_per_tile` | `integer` | yes | — |  |
| `position` | `Vec<integer>` | yes | — |  |
| `radius` | `integer` | yes | — |  |

### `PlayerSpawnDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `base_tile` | `Vec<integer>` | yes | — |  |
| `faction` | `string` | yes | — |  |
| `initial_buildings` | `Vec<InitialBuildingSpawn>` | no | `[]` |  |
| `initial_units` | `Vec<InitialUnitSpawn>` | no | `[]` |  |
| `player_id` | `integer` | yes | — |  |
| `sub_faction` | `string` \| `null` | no | `None` |  |
| `team_id` | `integer` | no | `0` |  |

### `PointDefenseSpec`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `cooldown_secs` | `number` | no | `0.0` |  |
| `intercept_radius` | `number` | no | `0.0` |  |

### `PreplacedEntityDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `building_id` | `string` \| `null` | no | `None` |  |
| `player_id` | `integer` | yes | — |  |
| `tag` | `string` \| `null` | no | `None` |  |
| `tile_pos` | `Vec<integer>` | yes | — |  |
| `unit_id` | `string` \| `null` | no | `None` |  |

### `PreplacedItemDropDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `item_id` | `string` | yes | — |  |
| `tile_pos` | `Vec<integer>` | yes | — |  |

### `ProjectileStyle`

- **Enum Values:** `"bullet"`, `"laser"`, `"tesla"`, `"radiation"`, `"sonic"`, `"mind_control"`, `"missile"`

### `PropKind`

- **Enum Values:** `"tree"`, `"streetlight"`, `"explosive_barrel"`

### `ResourcePatchDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `amount_per_tile` | `integer` | no | `600` |  |
| `position` | `Vec<integer>` | yes | — |  |
| `radius` | `integer` | no | `3` |  |
| `resource_id` | `string` | yes | — |  |

### `ResourceTypeDef`

Configurable economy resource definition loaded optionally from `resources.toml`. When omitted by a mod, `ResourceTypeDef::default_catalog()` supplies `credits` and `power`.

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `color_rgb` | `Vec<number>` | no | `{...}` |  |
| `display_name` | `string` | yes | — |  |
| `icon` | `string` \| `null` | no | `None` |  |
| `id` | `string` | yes | — |  |
| `is_capacity_pool` | `boolean` | no | `false` |  |
| `starting_amount` | `integer` | no | `0` |  |

### `SocketOffset`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `name` | `string` | yes | — |  |
| `offset` | `Vec<number>` | no | `[0.0,0.0,0.0]` |  |

### `SquadTraitSpec`

Composable trait turning an infantry unit into a multi-member squad with casualties, reinforcement, directional cover reduction, and suppression morale penalties.

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `auto_reinforce_default` | `boolean` \| `null` | no | `None` |  |
| `available_weapon_upgrades` | `Vec<string>` | no | `[]` |  |
| `directional_cover_multiplier` | `number` | no | `0.550000011920929` |  |
| `heavy_cover_damage_mult` | `number` \| `null` | no | `None` |  |
| `light_cover_damage_mult` | `number` \| `null` | no | `None` |  |
| `max_suppression` | `number` | no | `100.0` |  |
| `member_count` | `integer` | no | `5` |  |
| `member_spacing` | `number` | no | `0.6499999761581421` |  |
| `pin_suppression_threshold` | `number` \| `null` | no | `None` |  |
| `reinforce_cost_ratio` | `number` | no | `0.5` |  |
| `weapon_upgrade_slots` | `integer` | no | `0` |  |

### `StackPolicy`

- **Enum Values:** `"refresh"`, `"stack_intensity"`, `"independent"`

### `StatusEffectDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `armor_multiplier` | `number` | no | `1.0` |  |
| `build_speed_mult` | `number` | no | `1.0` |  |
| `confused` | `boolean` | no | `false` |  |
| `cure_at_service_depot` | `boolean` | no | `false` |  |
| `damage_multiplier` | `number` | no | `1.0` |  |
| `delayed_burst_damage` | `number` | no | `0.0` |  |
| `delayed_burst_splash` | `number` | no | `0.0` |  |
| `disarmed` | `boolean` | no | `false` |  |
| `dispel_category` | `string` \| `null` | no | `None` |  |
| `display_label` | `string` \| `null` | no | `None` |  |
| `dot_dps` | `number` | no | `0.0` |  |
| `duration_secs` | `number` | yes | — |  |
| `fire_rate_mult` | `number` | no | `1.0` |  |
| `hp_regen_per_sec` | `number` | no | `0.0` |  |
| `id` | `string` | yes | — |  |
| `immobilized` | `boolean` | no | `false` |  |
| `invulnerable` | `boolean` | no | `false` |  |
| `max_stacks` | `integer` | no | `1` |  |
| `modifier` | `BuffModifierSpec` \| null | no | `None` |  |
| `on_kill_spawn_unit` | `string` \| `null` | no | `None` |  |
| `phased_out` | `boolean` | no | `false` |  |
| `power_charge_boost` | `boolean` | no | `false` |  |
| `range_bonus` | `number` | no | `0.0` |  |
| `speed_multiplier` | `number` | no | `1.0` |  |
| `stack_policy` | `StackPolicy` | no | `"refresh"` |  |
| `stunned` | `boolean` | no | `false` |  |
| `vfx_ring_color` | `array` \| `null` | no | `None` |  |
| `vfx_style` | `StatusVfxStyle` | no | `"none"` |  |
| `vision_mult` | `number` | no | `1.0` |  |

### `StatusVfxStyle`

- **Enum Values:** `"none"`, `"ring"`, `"electric_arcs"`, `"parasite_sparks"`, `"countdown_beacon"`, `"tentacle_grapple"`, `"temporal_vortex"`

### `StealthSensorSpec`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `can_disguise_as_enemy` | `boolean` | no | `false` |  |
| `cloak_field_radius` | `number` | no | `0.0` |  |
| `cloaked` | `boolean` | no | `false` |  |
| `detects_cloak` | `boolean` | no | `false` |  |
| `detects_disguise` | `boolean` | no | `false` |  |
| `disguises_as_terrain` | `boolean` | no | `false` |  |
| `psychic_sensor_radius` | `number` | no | `0.0` |  |
| `reveal_on_fire_secs` | `number` | no | `0.0` |  |

### `SubFactionDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `id` | `string` | yes | — |  |
| `loading_screen` | `string` \| `null` | no | `None` |  |
| `name` | `string` | yes | — |  |
| `specialty` | `string` | no | `""` |  |

### `SuperweaponType`

- **Enum Values:** `"nuclear_missile"`, `"weather_storm"`, `"iron_curtain"`, `"chronosphere"`, `"paratroopers"`, `"genetic_mutator"`, `"psychic_dominator"`

### `SupportAuraSpec`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `cash_trickle_per_sec` | `integer` | no | `0` |  |
| `grants_radar` | `boolean` | no | `false` |  |
| `heals_infantry_per_sec` | `number` | no | `0.0` |  |
| `mobile_heal_per_sec` | `number` | no | `0.0` |  |
| `mobile_heal_radius` | `number` | no | `0.0` |  |
| `ore_refine_multiplier` | `number` | no | `0.0` |  |
| `periodic_reveal_interval_secs` | `integer` \| `null` | no | `None` |  |
| `periodic_reveal_radius` | `number` | no | `0.0` |  |
| `repairs_ships_per_sec` | `number` | no | `0.0` |  |
| `repairs_vehicles_per_sec` | `number` | no | `0.0` |  |
| `vehicle_cost_discount` | `number` | no | `0.0` |  |

### `SupportPowerDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `cooldown_secs` | `number` | no | `45.0` |  |
| `count` | `integer` | no | `5` |  |
| `damage` | `number` | no | `0.0` |  |
| `delivery_aircraft_hp` | `number` \| `null` | no | `None` |  |
| `delivery_speed` | `number` \| `null` | no | `None` |  |
| `duration_secs` | `number` | no | `0.0` |  |
| `id` | `string` | yes | — |  |
| `kind` | `SupportPowerKind` | no | `"paradrop"` |  |
| `missile_flight_secs` | `number` \| `null` | no | `None` |  |
| `name` | `string` | yes | — |  |
| `payloads` | `Vec<EffectPayload>` | no | `[]` |  |
| `radius` | `number` | no | `5.0` |  |
| `sub_factions` | `Vec<string>` | no | `[]` |  |
| `unit_id` | `string` \| `null` | no | `None` |  |

### `SupportPowerKind`

- **Enum Values:** `"paradrop"`, `"radar_scan"`, `"airstrike"`, `"chrono_shift"`, `"emp_pulse"`, `"force_shield"`, `"nuclear_strike"`, `"lightning_storm"`, `"iron_curtain"`, `"genetic_mutator"`, `"psychic_dominator"`

### `SurvivalWaveDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `count` | `integer` | no | `1` |  |
| `delay_secs` | `number` | no | `0.0` |  |
| `spawn_tile` | `array` \| `null` | no | `None` |  |
| `unit_id` | `string` | yes | — |  |

### `TeamMaskMode`

- **Enum Values:** `"auto_neutral_gray"`, `"alpha_mask"`, `"vertex_color_mask"`, `"full_tint"`, `"disabled"`

### `TerrainSplatDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `layers` | `Vec<string>` | no | `{...}` |  |
| `weights_rgba` | `Vec<Vec<integer>>` | no | `[]` |  |

### `TriggerAction`

- **Variants:** `type = "spawn_reinforcements"`, `type = "spawn_building"`, `type = "change_owner"`, `type = "fire_support_power"`, `type = "grant_credits"`, `type = "reveal_area"`, `type = "reveal_area_for_player"`, `type = "set_objective"`, `type = "upsert_objective"`, `type = "set_objective_status"`, `type = "add_objective_progress"`, `type = "start_countdown"`, `type = "stop_countdown"`, `type = "play_radar_video"`, `type = "play_cutscene"`, `type = "expand_playable_bounds"`, `type = "set_weather"`, `type = "declare_winner"`, `type = "set_variable"`, `type = "add_variable"`, `type = "focus_camera"`, `type = "eva_transmission"`, `type = "issue_order"`, `type = "apply_status_effect"`, `type = "teleport_entity"`, `type = "spawn_capture_zone"`

### `TriggerCondition`

- **Variants:** `type = "entered_zone"`, `type = "timer_elapsed"`, `type = "periodic_timer"`, `type = "countdown_expired"`, `type = "tagged_entity_destroyed"`, `type = "tagged_entity_entered_zone"`, `type = "objective_status_is"`, `type = "building_captured"`, `type = "player_units_below"`, `type = "building_destroyed"`, `type = "all_buildings_destroyed"`, `type = "variable_at_least"`, `type = "variable_equals"`, `type = "all"`, `type = "any"`, `type = "not"`

### `TriggerEventKind`

- **Variants:** `event = "match_start"`, `event = "tick_interval"`, `event = "timer_elapsed"`, `event = "zone_entered"`, `event = "entity_destroyed"`, `event = "building_captured"`, `event = "condition_met"`

### `TriggerGraphDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `enabled` | `boolean` | no | `true` |  |
| `groups` | `Vec<TriggerGraphGroupDef>` | no | `[]` |  |
| `id` | `string` | yes | — |  |
| `name` | `string` | no | `""` |  |
| `nodes` | `Vec<TriggerGraphNode>` | no | `[]` |  |
| `wires` | `Vec<TriggerGraphWire>` | no | `[]` |  |

### `TriggerGraphGroupDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `color_rgba` | `Vec<number>` | no | `{...}` |  |
| `comment` | `string` | no | `""` |  |
| `id` | `string` | yes | — |  |
| `rect_max` | `Vec<number>` | no | `[0.0,0.0]` |  |
| `rect_min` | `Vec<number>` | no | `[0.0,0.0]` |  |
| `title` | `string` | yes | — |  |

### `TriggerGraphNode`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `canvas_pos` | `Vec<number>` | no | `[0.0,0.0]` |  |
| `comment` | `string` | no | `""` |  |
| `group_id` | `string` \| `null` | no | `None` |  |
| `id` | `string` | yes | — |  |
| `node_kind` | `TriggerGraphNodeKind` | yes | — |  |
| `title` | `string` | no | `""` |  |

### `TriggerGraphNodeKind`

- **Variants:** `kind = "event_start"`, `kind = "logic_gate"`, `kind = "delay"`, `kind = "if_else"`, `kind = "action"`

### `TriggerGraphWire`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `from_node` | `string` | yes | — |  |
| `from_port` | `string` | no | `"out"` |  |
| `id` | `string` | yes | — |  |
| `to_node` | `string` | yes | — |  |
| `to_port` | `string` | no | `"in"` |  |

### `TriggerOrderKind`

- **Enum Values:** `"move"`, `"attack_move"`, `"patrol"`, `"guard"`, `"stop"`, `"hold_position"`

### `TriggerZoneDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `actions` | `Vec<TriggerAction>` | no | `[]` |  |
| `condition` | `TriggerCondition` \| null | no | `None` |  |
| `id` | `string` | yes | — |  |
| `max` | `Vec<integer>` | no | `[0,0]` |  |
| `min` | `Vec<integer>` | no | `[0,0]` |  |
| `once` | `boolean` | no | `true` |  |
| `script_hook` | `string` | no | `""` |  |

### `UnitDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `abilities` | `Vec<AbilityDef>` | no | `[]` |  |
| `ability_id` | `string` \| `null` | no | `None` |  |
| `ambient_vfx_id` | `string` \| `null` | no | `None` |  |
| `armor` | `ArmorType` | no | `"light"` |  |
| `armor_tag` | `string` \| `null` | no | `None` |  |
| `build_limit` | `integer` | no | `0` |  |
| `build_time_secs` | `number` | yes | — |  |
| `builder_trait` | `BuilderTraitSpec` \| null | no | `None` |  |
| `cash_bounty_ratio` | `number` | no | `0.0` |  |
| `chrono_harvester` | `boolean` | no | `false` |  |
| `chrono_locomotion` | `boolean` | no | `false` |  |
| `container` | `ContainerSpec` | no | `{...}` |  |
| `cost` | `integer` | yes | — |  |
| `cruise_altitude` | `number` | no | `0.0` |  |
| `crushes_infantry` | `boolean` | no | `false` |  |
| `crushes_vehicles` | `boolean` | no | `false` |  |
| `death_effect` | `DeathPayloadSpec` | no | `{...}` |  |
| `death_vfx_id` | `string` \| `null` | no | `None` |  |
| `deploy_weapon` | `string` \| `null` | no | `None` |  |
| `elite_weapon` | `string` \| `null` | no | `None` |  |
| `energy_pool` | `EnergyPoolSpec` \| null | no | `None` |  |
| `faction` | `string` | yes | — |  |
| `garrison_damage_multiplier` | `number` | no | `0.0` |  |
| `gatherer_trait` | `GathererTraitSpec` \| null | no | `None` |  |
| `harvest_capacity` | `integer` | no | `0` |  |
| `health` | `number` | yes | — |  |
| `hero_trait` | `HeroTraitSpec` \| null | no | `None` |  |
| `id` | `string` | yes | — |  |
| `immobile_when_deployed` | `boolean` | no | `false` |  |
| `immune_to_mind_control` | `boolean` | no | `false` |  |
| `immune_to_radiation` | `boolean` | no | `false` |  |
| `kamikaze` | `boolean` | no | `false` |  |
| `lands_when_deployed` | `boolean` | no | `false` |  |
| `lays_mines` | `boolean` | no | `false` |  |
| `locomotor` | `LocomotorSpec` | no | `{...}` |  |
| `max_ammo` | `integer` | no | `0` |  |
| `max_drones` | `integer` | no | `0` |  |
| `mind_control_limit` | `integer` | no | `0` |  |
| `mind_control_overload_dps` | `number` | no | `0.0` |  |
| `mine_damage` | `number` | no | `0.0` |  |
| `mine_radius` | `number` | no | `0.0` |  |
| `mobile_refinery` | `boolean` | no | `false` |  |
| `movement_domain` | `MovementDomain` | no | `"ground"` |  |
| `name` | `string` | yes | — |  |
| `passenger_mode_labels` | `Map<String, string>` | no | `{}` |  |
| `passenger_weapon_by_role` | `Map<String, string>` | no | `{}` |  |
| `passenger_weapon_by_unit` | `Map<String, string>` | no | `{}` |  |
| `plants_c4` | `boolean` | no | `false` |  |
| `point_defense` | `PointDefenseSpec` | no | `{...}` |  |
| `prerequisite_upgrades` | `Vec<string>` | no | `[]` |  |
| `prerequisites` | `Vec<string>` | no | `[]` |  |
| `primary_weapon` | `string` \| `null` | no | `None` |  |
| `psi_wave_damage` | `number` | no | `0.0` |  |
| `psi_wave_radius` | `number` | no | `0.0` |  |
| `required_era` | `string` \| `null` | no | `None` |  |
| `requires_powered_building` | `string` \| `null` | no | `None` |  |
| `resource_costs` | `Map<String, integer>` | no | `{}` |  |
| `resource_trickle_per_sec` | `Map<String, integer>` | no | `{}` |  |
| `role` | `UnitRole` | yes | — |  |
| `script_hooks` | `Map<String, string>` | no | `{}` |  |
| `secondary_weapon` | `string` \| `null` | no | `None` |  |
| `self_heal_per_sec` | `number` | no | `0.0` |  |
| `spawns_drone_unit` | `string` \| `null` | no | `None` |  |
| `speed` | `number` | yes | — |  |
| `spy_infiltrator` | `boolean` | no | `false` |  |
| `squad_trait` | `SquadTraitSpec` \| null | no | `None` |  |
| `stealth` | `StealthSensorSpec` | no | `{...}` |  |
| `streaming_build_power` | `number` \| `null` | no | `None` |  |
| `sub_factions` | `Vec<string>` | no | `[]` |  |
| `subterranean` | `boolean` | no | `false` |  |
| `supply_cost` | `integer` | no | `1` |  |
| `support_aura` | `SupportAuraSpec` | no | `{...}` |  |
| `tags` | `Vec<string>` | no | `[]` |  |
| `transforms_into_building` | `string` \| `null` | no | `None` |  |
| `uncrushable` | `boolean` | no | `false` |  |
| `vision_range` | `number` | no | `16.0` |  |
| `visual` | `VisualDef` | yes | — |  |

### `UnitRole`

- **Enum Values:** `"infantry"`, `"vehicle"`, `"harvester"`, `"engineer"`

### `UnitVoiceSpec`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `attack_order` | `Vec<string>` | no | `[]` |  |
| `death` | `Vec<string>` | no | `[]` |  |
| `move_order` | `Vec<string>` | no | `[]` |  |
| `select` | `Vec<string>` | no | `[]` |  |

### `UpgradeDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `advances_to_era` | `string` \| `null` | no | `None` |  |
| `armor_bonus_mult` | `number` | no | `1.0` |  |
| `cash_bounty_ratio` | `number` | no | `0.0` |  |
| `cost` | `integer` | yes | — |  |
| `damage_bonus_mult` | `number` | no | `1.0` |  |
| `faction` | `string` | no | `"any"` |  |
| `id` | `string` | yes | — |  |
| `name` | `string` | yes | — |  |
| `prerequisite_upgrades` | `Vec<string>` | no | `[]` |  |
| `prerequisites` | `Vec<string>` | no | `[]` |  |
| `range_bonus` | `number` | no | `0.0` |  |
| `required_era` | `string` \| `null` | no | `None` |  |
| `requires_promotion_point` | `boolean` | no | `false` |  |
| `research_time_secs` | `number` | yes | — |  |
| `resource_costs` | `Map<String, integer>` | no | `{}` |  |
| `speed_bonus_mult` | `number` | no | `1.0` |  |
| `target_def_ids` | `Vec<string>` | no | `[]` |  |
| `target_role` | `UnitRole` \| null | no | `None` |  |
| `target_tags` | `Vec<string>` | no | `[]` |  |

### `VfxBlendMode`

- **Enum Values:** `"additive"`, `"alpha_blend"`

### `VfxDecalLinkSpec`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `decal_kind` | `string` | yes | — |  |
| `fade_time_secs` | `number` | no | `18.0` |  |
| `radius` | `number` | no | `2.200000047683716` |  |

### `VfxEffectDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `camera_shake_intensity` | `number` | no | `0.0` |  |
| `camera_shake_radius` | `number` | no | `0.0` |  |
| `duration_secs` | `number` | no | `1.2000000476837158` |  |
| `emitters` | `Vec<VfxEmitterDef>` | no | `[]` |  |
| `fallback_style` | `ProjectileStyle` | no | `"bullet"` |  |
| `ground_decal` | `VfxDecalLinkSpec` \| null | no | `None` |  |
| `id` | `string` | yes | — |  |
| `looping` | `boolean` | no | `false` |  |
| `name` | `string` | no | `""` |  |

### `VfxEmitterDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `beam_width` | `number` | no | `0.0` |  |
| `blend_mode` | `VfxBlendMode` | no | `"additive"` |  |
| `burst_count` | `integer` | no | `0` |  |
| `color_ramp_rgba` | `Vec<Vec<number>>` | no | `{...}` |  |
| `cone_angle_deg` | `number` | no | `30.0` |  |
| `continuous_rate_per_sec` | `number` | no | `0.0` |  |
| `drag` | `number` | no | `0.0` |  |
| `duration_secs` | `number` | no | `0.800000011920929` |  |
| `gravity` | `number` | no | `0.0` |  |
| `id` | `string` | yes | — |  |
| `lifetime_range_secs` | `Vec<number>` | no | `{...}` |  |
| `local_offset` | `Vec<number>` | no | `[0.0,0.0,0.0]` |  |
| `looping` | `boolean` | no | `false` |  |
| `ribbon_width` | `number` | no | `0.0` |  |
| `shape` | `VfxEmitterShape` | no | `"sphere"` |  |
| `size_ramp` | `Vec<number>` | no | `{...}` |  |
| `speed_range` | `Vec<number>` | no | `[2.0,6.5]` |  |
| `sphere_radius` | `number` | no | `0.25` |  |
| `texture_path` | `string` \| `null` | no | `None` |  |

### `VfxEmitterShape`

- **Enum Values:** `"point"`, `"sphere"`, `"cone"`, `"ring"`, `"beam"`, `"ribbon"`

### `VisualArchetype`

- **Enum Values:** `"infantry"`, `"tank"`, `"harvester"`, `"aircraft"`, `"ship"`, `"construction_yard"`, `"power_plant"`, `"barracks"`, `"refinery"`, `"war_factory"`, `"turret"`, `"generic_box"`

### `VisualDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `animations` | `AnimClipMap` | no | `{...}` |  |
| `archetype` | `VisualArchetype` | no | `"generic_box"` |  |
| `barrel_recoil_distance` | `number` | no | `0.0` |  |
| `emissive_boost` | `number` \| `null` | no | `None` |  |
| `has_damage_states` | `boolean` | no | `false` |  |
| `has_turret` | `boolean` | no | `false` |  |
| `icon` | `string` \| `null` | no | `None` |  |
| `metallic` | `number` \| `null` | no | `None` |  |
| `model` | `string` | yes | — |  |
| `normal_map_path` | `string` \| `null` | no | `None` |  |
| `orm_map_path` | `string` \| `null` | no | `None` |  |
| `primary_color` | `Vec<number>` | no | `{...}` |  |
| `roughness` | `number` \| `null` | no | `None` |  |
| `scale` | `Vec<number>` | no | `[1.0,1.0,1.0]` |  |
| `sockets` | `Vec<SocketOffset>` | no | `[]` |  |
| `spin_speed_deg_per_sec` | `number` | no | `0.0` |  |
| `team_color_tint` | `boolean` | no | `true` |  |
| `team_mask_mode` | `TeamMaskMode` | no | `"auto_neutral_gray"` |  |
| `team_mask_threshold` | `number` \| `null` | no | `None` |  |

### `WeaponDef`

| Field | Type | Required | Default | Description |
| :--- | :--- | :---: | :--- | :--- |
| `airstrike_delay_secs` | `number` | no | `0.0` |  |
| `arc_height` | `number` | no | `0.0` |  |
| `armor_multipliers` | `Map<String, number>` | no | `{}` |  |
| `ballistics` | `BallisticSpec` | no | `{...}` |  |
| `burst` | `integer` | no | `1` |  |
| `c4_demolition` | `boolean` | no | `false` |  |
| `calls_airstrike_damage` | `number` | no | `0.0` |  |
| `can_mind_control_buildings` | `boolean` | no | `false` |  |
| `clears_garrison` | `boolean` | no | `false` |  |
| `clears_grapple` | `boolean` | no | `false` |  |
| `cone_angle_deg` | `number` | no | `0.0` |  |
| `cooldown_secs` | `number` | yes | — |  |
| `damage` | `number` | yes | — |  |
| `delivery` | `DeliveryKind` | no | `"instant"` |  |
| `disables_building_duration_secs` | `number` | no | `0.0` |  |
| `friendly_fire` | `boolean` | no | `false` |  |
| `id` | `string` | yes | — |  |
| `impact_vfx_id` | `string` \| `null` | no | `None` |  |
| `interceptable_by_aa_health` | `number` | no | `0.0` |  |
| `kamikaze` | `boolean` | no | `false` |  |
| `max_spin_up_multiplier` | `number` | no | `0.0` |  |
| `min_range` | `number` | no | `0.0` |  |
| `mind_control` | `boolean` | no | `false` |  |
| `muzzle_vfx_id` | `string` \| `null` | no | `None` |  |
| `name` | `string` | yes | — |  |
| `naval_only` | `boolean` | no | `false` |  |
| `on_hit_status` | `StatusEffectDef` \| null | no | `None` |  |
| `on_hit_statuses` | `Vec<StatusEffectDef>` | no | `[]` |  |
| `on_hit_zone` | `HazardZoneSpec` \| null | no | `None` |  |
| `prism_support` | `boolean` | no | `false` |  |
| `projectile_speed` | `number` | no | `30.0` |  |
| `projectile_style` | `ProjectileStyle` | no | `"bullet"` |  |
| `projectile_vfx_id` | `string` \| `null` | no | `None` |  |
| `range` | `number` | yes | — |  |
| `refraction_damage_ratio` | `number` | no | `0.0` |  |
| `refraction_range` | `number` | no | `0.0` |  |
| `refraction_targets` | `integer` | no | `0` |  |
| `spin_up_per_shot` | `number` | no | `0.0` |  |
| `splash_radius` | `number` | no | `0.0` |  |
| `steals_credits_per_hit` | `integer` | no | `0` |  |
| `targets_air` | `boolean` | no | `false` |  |
| `targets_ground` | `boolean` | no | `true` |  |
| `temporal_erase_dps` | `number` | no | `0.0` |  |
| `tractor_pull_distance` | `number` | no | `0.0` |  |
| `turn_rate_deg_per_sec` | `number` \| `null` | no | `None` |  |
| `warhead_tag` | `string` \| `null` | no | `None` |  |
| `warp_erase` | `boolean` | no | `false` |  |

