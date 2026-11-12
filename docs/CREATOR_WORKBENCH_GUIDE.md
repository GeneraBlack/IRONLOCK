# IRONLOCK Creator Workbench — Desktop RTS & Modding IDE Guide

## 1. Dual-UI Architecture Overview

`rts-client` separates tactical gameplay from authoring tools using a strict **Dual-UI Architecture**:

1. **Player Mode (`bevy_ui` — Pure Tactical Gameplay):**
   - Clean, distraction-free full-height 3D battlefield paired with a mod-skinned command sidebar (`FactionHudSkinDef`), **Opaque `256×256` Top-Down Terrain Minimap** (`alpha = 255` hillshaded splat terrain, 4-corner heightfield-raycast perspective camera trapezoid, lobby-slot colors `1..8`, event-driven GPU uploads, and `Alt+Click` / HUD `[PING]` allied radar flares), **Clickable Control Groups Bar (`[1..9, 0]`)**, **`[Spacebar]` 8-Event Radar Ring**, and **`IDLE [N]` Economy Filter (`[.]`)**.
   - **Top-Left Collapsible Objectives & `MM:SS` Countdown HUD (`ui/objectives_hud.rs`):** Displays `Primary`, `Secondary`, and `Bonus` `MissionObjectiveDef` tiers with `Pending`/`Active`/`Completed`/`Failed` badges, `current/target` counters, and click-to-jump camera markers.
   - **In-Engine Spline Cutscene Director & Rich EVA Portrait Cards (`ui/cutscene_director.rs`):** Smooth cubic Hermite/smoothstep camera spline playback (`CameraCutsceneDef`) with cinema letterbox bars (`[ESC]` to skip) and stylized Speaker Portrait + Audio Cue + Radar PiP Transmission Cards.
   - **Interactive 8-Step Bootcamp Tutorial Coach (`ui/tutorial_coach.rs`):** Direct Main Menu **`TUTORIAL & BOOTCAMP`** launch into `ra2_bootcamp_01` with live auto-advancing coaching steps (Camera Pan/Zoom -> Box-Select MCV -> Deploy ConYard -> Power Plant -> Refinery -> Barracks & Squad -> Control Group `[Ctrl+1]` -> Attack-Move `[A]`).
   - **Tactical Squads 2.0, Subterranean & Building Upgrade Command Bar:** Live `Light`/`Heavy` directional cover & `Pinned` status badges, `RetreatSquad` (`[R]`), `ReinforceSquadNow` / `Auto-Reinforce` toggle, Squad Weapon Slot upgrades, `ToggleBurrow`, and `UpgradeBuildingTo`.
   - **Competitive Multiplayer UX (`Phases 24`):** Main Menu **`MOD WORKSHOP`** browser & 1-click `.rtsmod` installer (plus automatic missing-mod download on Lobby join), Live **`[👁 Spectate]`** with configurable `0s..60s` anti-cheat delay (`SpectatorClient::connect_smart`), **Deterministic `.rtssav` Multiplayer Save & Synchronized Staging Room Resume**, and **MP Lag-Shield Modal (`ui/lag_shield_modal.rs`)** with `Vote Drop` / `Replace with AI` recovery.
   - Full **UTF-8 TrueType typography** (`UiFontResource`), atomic **`UnifiedClientSettingsFile` (`saves/settings.toml`)** persisting 5-bus audio + mute, 30 hotkeys, QWER grid, WASD pan, UI scale, `EN`/`DE` locale, and live **Display & Graphics** (`WindowMode`, Resolution `1280×720..3840×2160`, VSync, `ShadowQualitySetting`, `SsaoQualitySetting`, FPS cap).
   - **8 Floating Mod-Skinned In-Game Sub-Windows (`PlayerSubWindowStack`):**
     - **Mission Briefing & In-Engine FMV (`Briefing` + Radar PIP):** Frame-0 A/V-synced video streaming with 4:3 pillarboxing, Play/Pause/Skip, progress bar, and live subtitles.
     - **Diplomacy, Tribute & Comms (`[D]`):** Player alliance toggles, `+500` / `+1000` credit tributes, and quick radio taunts.
     - **Faction Technology Tree (`[Y]`):** Tiered visual dependency diagram (`Tier 0` Construction Yard through `Tier 4` Superweapons) with `[BUILT]`, `[READY]`, and `[LOCK]` badges, cameo icons, and click-to-inspect combat & multi-resource stats (`[T]` cycles selected unit combat stances).
     - **Branching Commander Power Tree (`[★ COMMANDER]`):** `1★`–`5★` Generals-style promotion tree (`CommanderPowerTreeDef`) unlocking orbital strikes, support powers, and global veterancy doctrines via `GameCommand::UnlockCommanderNode`.
     - **26-Track Interactive Soundtrack Jukebox (`[J]` / HUD `[🎵]`):** Browse all mod music tracks with `Peace`/`Combat`/`Menu`/`Score` mood badges, faction filters, Play/Pause/Next/Prev, Shuffle, Repeat, and **Dynamic Combat Crossfade** toggle (`1.5s` dual-sink crossfade).
     - **Multi-Slot Save / Load Archive (`[Ctrl+S]`, `[F5]` / `[F8]`):** Versioned Zlib-compressed `SaveGameEnvelope` (`*.rtssav`) browser for both Singleplayer and Lockstep Multiplayer with header metadata and `content_hash` compatibility checks.
     - **Mission Logbook & EVA Archive (`[Tab]`):** Real structured `MissionObjectiveDef` status list plus scrollable timestamped EVA & radio log.
     - **Tactical Client Options:** 4-tab live settings (`Graphics & Display`, `5-Bus Audio`, `Controls & ~30 Rebindable Hotkeys` + **QWER Grid** + **WASD Pan**, `Gameplay` incl. live **English / Deutsch** switcher) persisted atomically to `saves/settings.toml`.
   - Pressing **`[ESC]`** skips active cutscenes or closes open floating sub-windows in LIFO order before opening the Pause Menu, and pointer hover over any open sub-window blocks 3D battlefield clicks/orders underneath.

2. **Creator Mode (`IRONLOCK Creator Workbench` — `bevy_egui` + `egui_dock` Desktop IDE):**
   - Toggle anytime in singleplayer/skirmish via **`[F12]`**, the Main Menu **`Creator Workbench [F12]`** button, or CLI flag **`--studio`**.
   - Automatically disabled during live multiplayer lockstep matches to guarantee competitive fairness.

---

## 2. Launching the Workbench & CLI Flags

```powershell
# Launch directly into the 8-window IRONLOCK Creator Workbench IDE
cargo run --manifest-path engine/Cargo.toml -p rts-client -- --mod-path rust-ra2 --studio

# Launch with the multi-resource / worker / squad showcase mod in Studio mode
cargo run --manifest-path engine/Cargo.toml -p rts-client -- --mod-path mods/sdk-showcase-workers-squads --studio

# Run automated 12-stage 1440x900 Vulkan visual smoke test (01_main_menu_diorama.png .. 12_creator_workbench_full_schema_and_splines.png)
cargo run --manifest-path engine/Cargo.toml -p rts-client -- --mod-path rust-ra2 --visual-smoke-test
```

---

## 3. Docking Shell, Central 3D Viewport, 3D Gizmos & Workspace Presets

- **Open Interactive Central 3D Viewport:** All `egui_dock` panels dock around the central **`3D Battlefield Viewport`** tab (`clear_background = false`), keeping the live Bevy 3D PBR terrain, units, sockets, and debug heatmaps directly visible and interactive in the middle of the IDE.
- **Exact 3D Heightfield Raycasting (`CollisionGrid::raycast_heightfield`):** Both Player Mode (`camera.rs`) and the Creator Workbench (`studio/mod.rs`) raycast directly against the continuous bilinear vertex heightmap via 2D-Grid-DDA traversal (`500.0` world units), ensuring centimeter-accurate picking and sculpting across high plateaus, sloped ramps, valleys, and elevated bridges.
- **Direct 3D Viewport Picking, 3D Transform Gizmos & Live Brush Previews (`studio/viewport_gizmos.rs`):**
  - When **`Select / Transform`** (`WorldBuilderTool::SelectAndMove`) is active, click, **`Shift + Click`**, or **LMB box-drag** directly in the 3D viewport to select live `SimEntity` units/buildings, `SimCivilianStructure` urban blocks, 3D props, player spawns, and `TriggerZoneDef`s (honoring Scene Hierarchy visibility `👁` and lock `🔒` toggles, bi-directionally synced with Subwindows 2 & 3).
  - Drag the **3D XYZ Translation Arrows**, **Y-Rotation Ring**, or **`TriggerZone` Corner Handles** directly in 3D space; releasing the mouse pushes a reversible `EditorCommand` onto `EditorHistory` (`Ctrl+Z` / `Ctrl+Y`).
  - Terrain sculpt/splat brushes project **height-conforming inner/outer 3D brush rings** plus live **`2-Way` / `4-Way` symmetry preview rings** across the terrain and update only modified `16×16` terrain chunks via `MapTerrainDirtyRects`; placement tools render **green/red per-tile footprint ghosts** rotatable via **`[R]`**.
- **Strict Input Gating (`UiInputBlocker`):** `input/mod.rs` and `camera.rs` verify `egui_ctx.wants_pointer_input()`, `egui_ctx.wants_keyboard_input()`, floating `egui::LayerId` order, and `studio_viewport_rect` bounds every frame. Clicking sliders, dragging dock splitters, opening dropdowns, or typing in script/TOML editors never leaks clicks, box-selections, zoom wheel events, or RTS hotkeys into the 3D scene.
- **6 Curated Layout Presets (`View / Windows -> Workspace Layout Presets`):**
  1. **Default 8-Window IDE** — Balanced full-featured layout with Asset Browser & Hierarchy on the left, Property Inspector & World Builder on the right, and Diagnostics, Spreadsheet, Tech Graph, Triggers & Previewer docked below.
  2. **Level Design & Terrain** — Maximizes the central 3D viewport alongside World Builder, Scene Hierarchy, and Triggers.
  3. **Mod Data & Balance** — Side-by-side Asset Browser, Property Inspector, Balance Spreadsheet, and Tech Graph / Combat Duel Simulator.
  4. **Campaign & Triggers** — Focuses Scene Hierarchy, Trigger & Scripting Studio, and Diagnostics.
  5. **Asset & Socket Rigging** — Pairs Asset Browser with the 3D Asset & Socket Studio and Property Inspector.
  6. **AI & Sim Profiler** — Highlights Diagnostics & AI Heatmaps, Tech Graph, and Balance Spreadsheet.
- Layouts persist automatically to `saves/studio_layout.json` and can be reset anytime via **`View / Windows -> Reset Default Layout`**.

---

## 4. Reversible Command History (`EditorHistory`) & Live Playtest Workflow

- **`Undo [Ctrl+Z]` / `Redo [Ctrl+Y]` (`studio/history.rs`):** Maintains a reversible command stack (`>= 64` steps) covering:
  - Continuous terrain heightmap sculpting and 5-layer splat painting strokes,
  - Map entity placement, translation, and deletion (including mirrored symmetry placements),
  - SDK catalog edits (`UnitDef`, `BuildingDef`, `WeaponDef`) made in the Property Inspector or Balance Spreadsheet.
- **1-Click Playtest & Snapshot Restore:**
  - Click **`▶ Playtest Map`** in the Top Action Toolbar (or `Simulation -> Playtest Map`) to capture a deterministic `(SimWorld, MapDef)` snapshot and immediately test AI commanders, triggers, and combat inside the central viewport.
  - Click **`⏹ Stop & Restore Snapshot`** to halt simulation and restore exact pre-playtest entity positions, economies, cooldowns, and terrain state.
- **Transactional Hot-Reload (`⚡ Hot-Reload Mod` / `[Ctrl+F12]`):** Reloads `.toml`, `.rhai`, and `.lua` files from disk via `LoadedMod::reload_from_disk()` and applies updated stats to live `SimWorld` entities without restarting.

---

## 5. Reference Guide to All 9 Dockable Sub-Windows

### Subwindow 1: Asset Browser, 10-Schema CRUD & Import Wizards (`studio/asset_browser.rs`)
- Browse all loaded mod catalogs across **all 10 declarative schemas**: **`Units`**, **`Buildings`**, **`Weapons`**, **`Upgrades`**, **`CommanderTrees`**, **`Factions`**, **`Resources`**, **`AiProfiles`**, **`Campaigns`**, and **`Maps`**, plus **`Prefabs`**, **`Scripts`**, **`Props`**, **`Audio`**, and **`Videos`**.
- **Disk-Backed `+ New` & Reference-Checked `Delete` Wizards:** Every catalog category supports 1-click creation and safe deletion via `LoadedMod::upsert_*` / `delete_*` with automatic `LoadedMod::find_id_references` dependency protection so broken cross-references can never be saved.
- Built-in creation & import modals:
  - **New Mod Scaffold Wizard** (standalone or `extends = "rust-ra2"` sub-mod),
  - **Universal Schema Creation Wizards** (`Unit`, `Building`, `Weapon`, `Upgrade`, `CommanderTree`, `Faction`, `Resource`, `AiProfile`, `Campaign`, `Skirmish Map`),
  - **Modern Asset Import Wizard** (`.glb`, `.png`, `.ogg`, `.flac`, `.wav`, `.mp4` via `rts_asset_pipeline::discovery::import_modern_asset` with automatic `.glb` bounding-box, animation clip & socket discovery),
  - **Westwood / OpenRA Legacy Import Wizard** (`.mix`, `.csf`, `.aud`, `.vxl`/`.hva`, `IsoMapPack5`).

### Subwindow 2: Scene Hierarchy & Outliner (`studio/hierarchy.rs`)
- Live scene outliner grouped into **`Neutral (Player 0)`**, **`Player 1..8`** forces, **`TriggerZones`**, **`CameraCutscenes`**, **`ResourceNodes`**, and **`CivilianStructures`**.
- Per-entity **Visibility (`👁`)** and **Transform Lock (`🔒`)** toggles.
- Click to select and inspect in Subwindow 3; click **`🎯 Snap`** (or double-click) to smoothly center the 3D RTS camera on the target.

### Subwindow 3: Complete 10-Schema Property Inspector (`studio/property_inspector.rs`, `studio/inspector_extended_schemas.rs`)
- Dual-mode collapsible inspector for **live map `SimEntity` instances** (including `PreplacedEntityDef.tag` VIP tags) and **all 10 declarative SDK schemas** with full `EditorHistory` Undo/Redo:
  - **`UnitDef`, `BuildingDef`, `WeaponDef`:** Complete field coverage including `LocomotorKind`, `BallisticSpec`, `BuilderTraitSpec`, `GathererTraitSpec`, `SquadTraitSpec` 2.0 (`light_cover_damage_mult`, `heavy_cover_damage_mult`, `pin_suppression_threshold`, `weapon_upgrade_slots`, `available_weapon_upgrades`), `SupportAuraSpec` (`grants_radar`, periodic reveal), `BuildingDef.{upgrade_options, addon_sockets}`, `subterranean`, `disguise`, `mind_control`, and reference dropdowns.
  - **Extended Schemas (`inspector_extended_schemas.rs`):** Dedicated form editors for **`FactionDef` / `SubFactionDef`**, **`UpgradeDef`**, **`CommanderPowerTreeDef` (`1★–5★` nodes)**, **`ResourceTypeDef`**, **`AiProfileDef` 2.0** (`personality_id`, `unit_role_quotas`, `target_priority_weights`, `expand_at_credits`, `superweapon_doctrine`, `amphibious_assault_enabled`), **`CampaignDef` / `MissionDef`**, and **`AudioCatalog` (`26+` `MusicMood` tracks)**.
- **1-Click Disk Persistence:** Writes changes directly to the mod's `.toml` catalogs via `LoadedMod::upsert_*` / `save_audio_catalog` and triggers live hot-reload.

### Subwindow 4: Map Builder Pro — Splines, Auto-Ramps, Prefabs & Live Weather (`studio/world_builder.rs`, `studio/spline_and_prefab_tools.rs`)
- **Heightmap Sculpt & True Auto-Ramp Grading:** `Raise`, `Lower`, `Smooth`, `Flatten / Plateau`, and **`Auto-Ramp`** (click-drag start->end height-interpolating ramp grading with smooth bank falloff).
- **Vertex-Exact 5-Layer WGSL Splat Paint (`(width + 1) * (height + 1) * 4`):** `Grass`, `Dirt`, `Sand`, `Asphalt`, and `CliffRock` (`[0, 0, 0, 0]` normalized residual) updating dirty `16×16` terrain chunks live.
- **Catmull-Rom 3D Spline Tool (`spline_and_prefab_tools.rs`):** Place, drag, insert, and bake smooth Catmull-Rom control-point splines into **Asphalt Roads**, **Carved Water Rivers**, or **Elevated Cliff Ridges** with reversible `EditorHistory` snapshots.
- **Reusable `.prefab.toml` Stamp Brush (`MapPrefabDef`):** Capture any multi-entity / terrain / splat selection into `prefabs/<id>.prefab.toml` and stamp rotated (`0°/90°/180°/270°`) base layouts, bridge crossings, or urban blocks anywhere on the map with Undo/Redo.
- **3D Cutscene Camera Frustum Gizmos & Live Weather Preview:** Visualize and drag `CameraCutsceneDef` keyframe frustums directly in 3D space and preview `MapDef.initial_weather` (`Clear`, `Rain`, `Snow`, `Sandstorm`, `NightOps`, `IonStorm`) live in the viewport.

### Subwindow 5a & 5b: Multi-Schema Balance Spreadsheet, Visual Tech Graph & Combat Duel Simulator (`studio/data_table_window.rs`, `studio/tech_graph_window.rs`)
- **5a — Multi-Entity Balance Spreadsheet:** Sortable, filterable table across `Units`, `Buildings`, `Weapons`, `Upgrades`, and `AiProfiles` with inline numeric cell editing (`Cost`, `HP`, `Speed`, `Build Time`, `Power`, `Damage`, `Range`, `Cooldown`, `DPS`) recorded directly into `EditorHistory`.
- **5b — Interactive Drag-and-Drop Tech Graph:** Panable, zoomable (`0.50x..1.75x`) Bezier dependency canvas per faction with drag-wire prerequisite editing, automatic DAG cycle detection, and **1-Click Headless Combat Duel Simulator** (`Win-%`, `Draw-%`, `Mean TTK`, `Surviving HP-%`).

### Subwindow 6: Campaign, Cutscene, Trigger & Script IDE (`studio/trigger_studio_window.rs`, `studio/script_ide_support.rs`)
- **Structured `MissionObjectiveDef`, Countdown & Cutscene Editor:** Author `Primary`/`Secondary`/`Bonus` objectives, `MM:SS` countdown timers, `ExpandPlayableBounds`, and multi-keyframe `CameraCutsceneDef` splines alongside visual `TriggerZone` and nested `TriggerCondition` -> `TriggerAction` trees.
- **Full `.rhai` & `.lua` Script IDE (`script_ide_support.rs`):** Features `LayoutJob` keyword/string/comment/number syntax highlighting, synchronized line-number gutter, **clickable compiler-error line jump**, searchable **Script API Reference & 1-click snippet inserter** (including `on_ai_tick` AI 2.0 hooks), and a **Live Playtest Watch Table** inspecting `trigger_vars`, `world_custom_flags`, `mission_objectives`, and `countdown_timers` in real time.

### Subwindow 7: Isolated Offscreen 3D Turntable, Socket Rigging & 1-Click Cameo Studio (`studio/asset_previewer.rs`)
- **Isolated Offscreen 3D Turntable (`RenderLayers::layer(3)` -> `RenderTarget::Image`):** Orbit, zoom, team-color tint, and preview `LOD0`/`LOD1` `.glb` models, `VisualDef.sockets` (`[🔥 Test Fire]`), animation clips, `.ogg`/`.flac`/`.wav` audio, and A/V-synced `.mp4` FMVs.
- **`[📸 Capture Cameo PNG]` Studio Button:** Triggers `PreviewStageState.capture_cameo_request` to render a crisp `96×72` sidebar cameo PNG directly to `assets/icons/<id>.png` and bind `icon` on the active `UnitDef` / `BuildingDef`.

### Subwindow 8: Live Mod Diagnostics, Deep Binary Validator, Sim Profiler & AI 2.0 Inspector (`studio/diagnostics_window.rs`)
- **Clickable `validate_mod` Linter Console:** Runs full semantic, script, vertex-stride splatmap, and **Deep Binary Header Validation** (`validate_binary_asset_header` verifying `.glb` magic, `.png` IHDR, `.ogg` `OggS`, `.flac` `fLaC`, `.wav` `RIFF`, `.mp4` `ftyp`) with click-to-jump navigation into Subwindows 3, 5a, or 6.
- **Live `SimTickProfile` Subsystem Graph, Single-Tick Stepper & 3D AI 2.0 Heatmaps:** Inspect `Pathfinding`, `Movement`, `Combat`, `Projectiles`, `AI`, and `Triggers/FoW` durations plus `AiCommanderState` personality, naval landing waves, chokepoint defenses, and economy expansion state.

### Subwindow 9: Side-by-Side Bilingual `LocaleCatalog` Editor (`studio/locale_editor_window.rs`)
- **Side-by-Side `en.toml` ↔ `de.toml` Table Editor:** Displays every localization key across `locales/en.toml` and `locales/de.toml` in a unified searchable table with **`Missing Translation` filter**, **1-Click `Scan Mod for Untranslated Keys`** (harvesting keys from units, buildings, upgrades, commander trees, campaigns, and map objectives), inline editing with `EditorHistory` Undo/Redo, and **`💾 Save Both Locales`** via `LoadedMod::save_locale_catalog`.

### Creator Workbench 2.0 Panels (`Phases 25–28`, `WorkbenchPanel` Registry — `< 750` lines/module)
- **Subwindow 10 — Visual Trigger & Mission Node-Graph Canvas (`studio/trigger_node_graph/`):** Pan/zoom 2D Bezier node-graph editor authoring `TriggerGraphDef`s (`EventStart`, `LogicGate`, `Delay`, `IfElse`, `Action`) without writing Rhai/Lua, complete with **Live Playtest Wire Tracing** (`last_fired_graph_nodes` glows active wires gold in real time!).
- **Subwindow 11 — AI Behavior-Tree & Live Blackboard Canvas (`studio/ai_graph_window/`):** Visual hierarchy & node editor for `AiProfileDef.behavior_tree` (`Selector`, `Sequence`, `UtilitySelector`, `Inverter`, `Cooldown`, `BlackboardCondition`, `AiAction`) with live per-tick AI blackboard inspection (`active_bt_node_by_player`).
- **Subwindow 12 — 3D VFX, Particle & Projectile Studio (`studio/vfx_studio_window/`):** Author `vfx/vfx.toml` (`VfxCatalog`) multi-emitter 3-stop size/HDR-color/alpha curves, cone/sphere bursts, beams/ribbons, camera shake, and ground decals with `[▶ Loop Effect]` live preview on `RenderLayers::layer(3)`.
- **Subwindow 13 — Interactive PBR Material & Team-Mask Studio (`studio/material_studio/`):** Tune `VisualDef.{metallic, roughness, emissive_boost, team_mask_mode, team_mask_threshold, normal_map_path, orm_map_path}` live on the 3D turntable (`unit_pbr_team.wgsl`), switch lighting presets (`NoonSun`, `Dusk`, `NightOps`, `IonStorm`), and run 1-click KTX2/Mipmap/Cameo-Atlas texture cooking (`rts-asset-pipeline cook-textures`).
- **Subwindow 14 — Mod Mutator & Delta-Overlay Wizard (`studio/mod_overlay_wizard/`):** Scaffold lightweight balance patches, unit packs, or mutator overlays (`depends_on` + minimal TOML delta serialization) and inspect base-vs-overlay field overrides in the **Inheritance & Override Diff Table**.

---

## 6. Next-Gen Tactical UX, HUD Presets & 14-Stage Visual Smoke Harness (`Phases 26–28`)

- **Orbital Strategic Zoom (`12m..550m`):** Smooth cursor-anchored zoom transitioning seamlessly at `>= 190m` into crisp GPU-instanced NATO/SupCom tactical icons + global `[Shift]` queued-order path lines.
- **Smart Command UX (`src/input/`):**
  - **Freehand Curved Formation Drag:** Hold **RMB** and drag across terrain to preview and issue `GameCommand::MoveAlongCustomCurve` custom frontlines.
  - **Circle-Drag Area Commands:** **LMB** drag a world circle while in `Repair`, `Harvest`, `Attack`, or `Guard` cursor mode to dispatch `GameCommand::AreaCommand` across all valid targets inside the radius.
  - **Smart-Cast & Right-Click Auto-Cast:** Enable **Smart-Cast** in `Options -> Interface` for instant cursor-targeted ability hotkeys, and **Right-Click** ability cameos to toggle golden `ToggleAbilityAutoCast` frames.
- **3 Live-Switchable HUD Layout Presets (`Options -> Interface` / `ModManifest.default_hud_layout`):**
  1. `ClassicRightSidebar` (C&C / RA2 classic right sidebar)
  2. `ModernBottomCommandCard` (WC3 / SC2 / Stormgate bottom bar with Hero Level/Mana/6-Slot Inventory & `4×3` command card)
  3. `CompactBarOverlay` (BAR / SupCom minimalist compact overlay with Global Production, Era & Streaming Economy Strip)
- **Accessibility Colorblind Modes:** `Off`, `Deuteranopia`, `Protanopia`, `Tritanopia`, and `AllyEnemyHighContrast` updating 3D shaders, health bars, and radar minimap dots simultaneously.
- **14-Stage Automated Vulkan Smoke Harness (`--visual-smoke-test <dir>`):** Deterministically captures screenshots `01..14` at `1440×900` (`13_orbital_strategic_zoom_and_bottom_hud.png` & `14_creator_workbench_node_graphs_and_vfx_studio.png`).

---

## 7. Keyboard Shortcut Reference

| Shortcut | Mode | Action |
| :--- | :--- | :--- |
| **`[F12]`** | Player / Studio | Toggle `IRONLOCK Creator Workbench` IDE (Singleplayer/Skirmish) |
| **`[Ctrl + F12]`** | Player / Studio | Transactional Live Hot-Reload of `.toml`, `.rhai`, and `.lua` files |
| **`[Ctrl + Z]` / `[Ctrl + Y]`** | Studio | Undo / Redo terrain, spline, prefab, entity, locale, or SDK edit (`EditorHistory`) |
| **`[Spacebar]`** | Player In-Game | Cycle camera through last 8 Radar Events (under attack, unit ready, ping, superweapon, objective) |
| **`[.]` (Period)** | Player In-Game | Cycle & center on idle economy units (harvesters, builder workers, engineers, undeployed MCVs) |
| **`[Alt + Click]`** | Player In-Game | Send allied `PingLocation` flare on terrain or minimap |
| **`[1..9, 0]` / `[Ctrl+1..0]`** | Player In-Game | Select / double-tap focus / bind numbered Control Group (also clickable on HUD bar) |
| **`[R]`** | Player / Studio | Retreat selected tactical squad (`RetreatSquad`) or rotate building/prefab footprint (`90°`) |
| **`[J]`** | Player In-Game | Toggle **26-Track Interactive Soundtrack Jukebox** window |
| **`[D]` / `[Y]` / `[Tab]`** | Player In-Game | Toggle **Diplomacy**, **Faction Tech Tree**, or **Mission Logbook & Objectives** window |
| **`[Ctrl + S]` / `[F5]` / `[F8]`** | Player In-Game | Save/Load Archive modal / QuickSave (`.rtssav` SP & MP) / QuickLoad |
| **`[Enter]` / `[Shift+Enter]`** | Player In-Game | Open All-Chat / Team-Chat input bar in Multiplayer |
| **`[ESC]`** | Player In-Game | Skip active cutscene -> cancel placement -> close topmost window -> toggle Pause Menu |
| **`[F9]` / `[Shift + F9]`** | Player / Studio | Toggle Telemetry & Profiler overlay / Toggle Master Audio Mute |

---

## 8. Standalone Game Export & Online Mod Workshop Publishing (`studio/export_wizard.rs`)

Use **`File -> Export Standalone Game...`** (`studio/export_wizard.rs`) to:
1. Cook the active mod into a self-contained desktop game folder (`mods/<id>.rtsmod` Zlib VFS archive + `game_bundle.toml` + `Play.bat` / `play.sh` + engine binaries), or
2. Click **`☁ Publish .rtsmod to Online Workshop`** to pack, SHA-256 hash, and upload your mod or map pack directly to `centralwebserver` (`/api/v1/workshop/publish`) so players can browse and install it in 1 click from the Main Menu **`MOD WORKSHOP`** screen! See [`docs/STANDALONE_PUBLISHING.md`](STANDALONE_PUBLISHING.md) for full details.
