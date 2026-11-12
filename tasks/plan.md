# Implementation Plan & Architecture Roadmap: IRONLOCK // RUST-RTS

## Overview
Dieser Plan dokumentiert die Gesamtarchitektur und alle abgeschlossenen Entwicklungsphasen (Meilensteine 1 bis 12) der daten- und skriptgetriebenen 3D-RTS-Engine (`engine/`), der Total Conversion [`rust-ra2/`](file:///d:/projekt/rust-rts/rust-ra2/mod.toml) sowie des zentralen Multiplayer-Backbones ([`centralwebserver/`](file:///d:/projekt/rust-rts/centralwebserver/README.md) & [`ns8-rust-rts/`](file:///d:/projekt/rust-rts/ns8-rust-rts/README.md)) für **`rust-rts.bls-isp.net`**.

## Dependency Graph

```text
rts-sdk (TOML Schemas, Loader, Validator, Rhai + Lua ScriptHost)
    │
    ├──► rts-asset-pipeline (CLI & Converter: .glb Mesh-Builder, Westwood .mix/.vxl/.shp/.ini/.wav, OpenRA MiniYAML)
    │        │
    │        └──► rust-ra2 & mods/* (3 Fraktionen, 144 .glb-Modelle, 404 Audio-Clips, 138 4K/HD-Videos, 827 KI-Texturen)
    │
    └──► rts-simulation (Deterministischer 20-Hz-Simulationskern: Commands, A*, Combat, Superwaffen, Skirmish-KI)
             │
             └──► rts-network (Lockstep-Transport, Replay, UPnP/NAT-Traversal, Reverse-TCP-Relay & Lobby-Client)
                      │
                      ├──► rts-client (Bevy 0.15 3D-Client: PBR-Terrain, .glb-Sync, VFX, Audio, RA2-HUD & Engine Studio)
                      │
                      └──► centralwebserver & ns8-rust-rts (Master-Lobby-Register & Relay-Hub auf rust-rts.bls-isp.net)
```

## Completed Phases & Checkpoints

### Phase 1: Foundation (`rts-sdk` & Dual Scripting)
- [x] Workspace-Setup & `rts-sdk` Daten-Schemata (`manifest.rs`, `schema/`, `loader.rs`)
- [x] Duale Skript-Engine (`.rhai` & `.lua`) & Cross-Reference-Validator (`scripting/`, `validator.rs`)

### Phase 2: Asset-Pipeline & Modding Toolchain (`rts-asset-pipeline`)
- [x] Prozeduraler Multi-Node-`.glb`-Generator (`mesh_builder/`), Validator, Inspector & Scaffolder
- [x] Westwood-Konverter (`.mix`, `.vxl` Voxel-Isosurface, `.shp` 3D-Relief/Turntable, `.ini`, IMA-ADPCM `.wav`) & OpenRA-Konverter (`openra/`)

### Phase 3: Deterministic 20-Hz RTS Simulation (`rts-simulation`)
- [x] `SimWorld`, serialisierbare `GameCommand`-Pipeline, A*-Wegfindung (Land, Wasser, Amphibisch, Luft) & Bewegung
- [x] Kombinierter Kampf, Basisbau, Erz-Wirtschaft, Strombilanz, Veteranen-System, Garnisonen, Brücken, Spionage, Superwaffen & Skirmish-KI (`ai/`)

### Phase 4: 3D Bevy Client & In-Game Engine Studio (`rts-client`)
- [x] Bevy 0.15 3D-Client, kontinuierliches Vertex-Terrain, `.glb`-Entity-Sync mit Turm-/Rotor-Animation & 3D-VFX-Partikelsystem (`vfx/`)
- [x] Authentisches RA2-Sidebar-HUD mit Cameo-Icons, Radar-Minimap, Audio-Engine & In-Game Engine Studio (`[ESC]`)

### Phase 5: Lockstep Multiplayer, 3-Tier NAT/UPnP & `rust-rts.bls-isp.net` (`rts-network` & `centralwebserver`)
- [x] Deterministischer TCP-Lockstep-Transport (`LockstepServer` / `LockstepClient`), `state_hash()`-Desync-Prüfung & `.replay.json`-Verifikation
- [x] 3-Stufen-NAT/UPnP-Traversal (`nat_upnp.rs`) + Reverse-TCP-Relay-Tunnel (`RelayTunnelRoom`, Ports `27900–27999`)
- [x] Zentraler Node.js/TypeScript Webserver (`centralwebserver/`) & Rootless NethServer 8 Modul (`ns8-rust-rts/`) für `https://rust-rts.bls-isp.net/`

### Phase 6: Complete `rust-ra2` Total Conversion & GPU/AI Asset Remaster (AMD Radeon RX 7900 XTX)
- [x] **144 `.glb` 3D-Modelle** aus echten `.vxl`-Voxeln und `.shp`-Sprites rekonstruiert
- [x] **404 `.wav` Audiodateien** (SFX, EVA, Länder-Taunts, 26 Musiktracks) entpackt und eingebunden
- [x] **138 Videos** (`55` 4K `2880×2160` FMVs + `83` `1120×880` Radar-PIPs) via `bwdif` @ 30 FPS + Vulkan (`vk:0`) + `h264_amf` remastert
- [x] **827 Texturen** (207 Cameos `8×`, 51 Ladebildschirme `4×` 4K+, 36 HUD-Panels `4×`, 517 Cursor `4×`, 16 VFX-Flipbooks) per Multi-Model Vulkan KI (`Waifu2x CUNet` + `Real-ESRGAN`) hochskaliert

---

### Phase 7: Engine/Mod-Entkopplung & Konsolidierung der Kernsysteme ([`tasks/refactoring_plan.md`](file:///d:/projekt/rust-rts/tasks/refactoring_plan.md))
Detailplan (v2 nach Review) siehe [`tasks/refactoring_plan.md`](file:///d:/projekt/rust-rts/tasks/refactoring_plan.md); ausführbare Task-Checkliste siehe [`tasks/todo.md`](file:///d:/projekt/rust-rts/tasks/todo.md) (**Tasks 114–121**).

- [x] **Phase 7.1 (Tasks 114–115):** Zentrale `CapabilityQueries` (`world/queries.rs`) & zentrales `EntityLifecycle`-System (`world/lifecycle.rs`)
- [x] **Phase 7.2 (Tasks 116–117):** Universelles `StatusSystem` (`StatusEffectDef` / `ActiveBuff`) mit Client-VFX/HUD-Metadaten & Clean-Cut-Abbau der 16 Einzel-Timer auf `SimEntity`
- [x] **Phase 7.3 (Tasks 118–119):** Globale `ModRules` (`mod.toml`), kompositorisches `EffectPayload`-System & Auslagerung aller hartcodierten RA2-Regeln (IFV-Waffenadaption, Superwaffen, Infiltration, Crates) nach `rust-ra2/`
- [x] **Phase 7.4 (Tasks 120–121):** Clean-Cut-Konsolidierung von `WeaponDef` sowie gemeinsame Trait-Substructs (`ContainerSpec`, `StealthSensorSpec`, `DeathPayloadSpec`, `PointDefenseSpec`, `SupportAuraSpec`) für `UnitDef` & `BuildingDef`

---

### Phase 8: Komplettierung der RTS-Engine & des Dev/Mod-Kits ([`tasks/engine_dev_mod_kit_plan.md`](file:///d:/projekt/rust-rts/tasks/engine_dev_mod_kit_plan.md))
Detailplan siehe [`tasks/engine_dev_mod_kit_plan.md`](file:///d:/projekt/rust-rts/tasks/engine_dev_mod_kit_plan.md); ausführbare Task-Checkliste siehe [`tasks/todo.md`](file:///d:/projekt/rust-rts/tasks/todo.md) (**Tasks 122–131**).

- [x] **Phase 8.1 (Tasks 122–123):** Client-Entkopplung (`SubFactionDef`, `VisualDef.icon`, `MissionDef`-Medien) & datengetriebenes Audio-/Voice-/Lokalisierungs-System (`AudioCatalog`, `LocaleCatalog`)
- [x] **Phase 8.2 (Tasks 124–125):** Mod-Vererbung (`extends`), deterministischer `content_hash()` im Multiplayer-Handshake, `.rtsmod`-Packer (`pack`/`unpack`) & Live Hot-Reload (`[F12]`)
- [x] **Phase 8.3 (Tasks 126–127):** 3D-Karteneditor v2 (Pinselgrößen, Symmetrie-Modi, Spawn-/Trigger-Platzierung, Map-Playability-Validator) & Headless Balance-Simulator (`benchmark-balance`)
- [x] **Phase 8.4 (Tasks 128–129):** Datengetriebene KI-Profile (`ai_profiles.toml`, Build-Orders, `on_ai_tick`) & hierarchisches Group-Path-Caching mit Formations-Kohäsion und Boid-Steering
- [x] **Phase 8.5 (Tasks 130–131):** Kampagnen-Direktor (Stateful Trigger-Variablen, `All`/`Any`/`Not`, Kamera-/EVA-Direktor, Multi-Action-Skript-API) & automatischer SDK-Dokumentations-Generator (`generate-docs` -> `docs/MOD_DEV_KIT.md`)

---

### Phase 9: Studio-Grade RTS-Engine & Mod-SDK Komplettierung ([`tasks/phase9_rts_engine_mod_kit_plan.md`](file:///d:/projekt/rust-rts/tasks/phase9_rts_engine_mod_kit_plan.md))
Detailplan siehe [`tasks/phase9_rts_engine_mod_kit_plan.md`](file:///d:/projekt/rust-rts/tasks/phase9_rts_engine_mod_kit_plan.md); ausführbare Task-Checkliste siehe [`tasks/todo.md`](file:///d:/projekt/rust-rts/tasks/todo.md) (**Tasks 132–141**).

- [x] **Phase 9.1 (Tasks 132–133):** Komplettes Mod-Projekt-Scaffolding (`rts-mod-tools new-mod`), JSON-Schema/Taplo-Generator (`generate-schema`) für IDE-Autocomplete & semantischer Mod-/Tech-Tree-Linter (`UNREACHABLE_PREREQUISITE`, `ZERO_EFFECTIVE_DAMAGE`, `ORPHANED_*`)
- [x] **Phase 9.2 (Tasks 134–135):** Granulare `SubsystemHashes` & deterministischer Desync-Differ (`SimWorld::diff_desync`), Keyframe-Snapshotted Fast-Seek Replays mit APM/Economy-Telemetrie & Spectator/Caster-HUD
- [x] **Phase 9.3 (Tasks 136–137):** Subsystem-Tick-Profiler (`SimTickProfile`), interaktive In-Game Dev-Konsole (`[F9]`) mit Single-Tick-Debugger (`step [N]`) & schaltbare 3D-Debug-Gizmo-Layer
- [x] **Phase 9.4 (Tasks 138–139):** Deterministischer Seed-basierter Skirmish-Map-Generator (`generate_procedural_map` / `generate-map`) & persistentes Kampagnen-Profil (`CampaignSaveProfile`) mit Credits- und Veteranen-Carry-Over
- [x] **Phase 9.5 (Tasks 140–141):** Datengetriebene 3D-Turret-Rotation, Barrel-Recoil & Rotor-Sub-Meshes sowie deklarativer Mod-Szenario-Test-Runner (`rts-mod-tools test-mod`)

---

### Phase 10: `rust-ra2` Content-Completion & Client-Polish ([`tasks/phase10_ra2_completion_plan.md`](file:///d:/projekt/rust-rts/tasks/phase10_ra2_completion_plan.md))
Detailplan siehe [`tasks/phase10_ra2_completion_plan.md`](file:///d:/projekt/rust-rts/tasks/phase10_ra2_completion_plan.md); ausführbare Task-Checkliste siehe [`tasks/todo.md`](file:///d:/projekt/rust-rts/tasks/todo.md) (**Tasks 142–149**).

- [x] **Phase 10.1 (Task 142):** Bereinigung der 2 Linter-Warnungen (`gatling_tank`, `boomer_torpedo_cruise`) & vollständige `[catalogs]`-Deklaration in `rust-ra2/mod.toml` (`0` Linter-Warnungen)
- [x] **Phase 10.2 (Tasks 143–144):** Vollständiger 40-Missionen-Kampagnenkatalog (`campaigns.toml` mit 5 Kampagnen, 4K-Ladebildschirmen, FMV-/Radar-Videos, Carry-Over) & mehrstufige Kampagnen-Trigger
- [x] **Phase 10.3 (Tasks 145–146):** Client-Menü-Polish — 5-Kampagnen-Wähler mit Missions-Pagination & Fortschrittsanzeige sowie Skirmish-Map-Filter (14 MP-Karten) + 1-Klick Prozeduraler Seed-Map-Generator
- [x] **Phase 10.4 (Tasks 147–149):** In-Game FMV-Video- & Soundtrack-Jukebox-Player, Ausbau von `rust-ra2/tests/mod_scenarios.toml` (Allies, Soviets, Yuri, Kampagnen-Trigger) & End-to-End-Verifikation
- [x] **Phase 11 (Tasks 150–156):** Authentisches Red Alert 2 Fraktions-HUD (`credits.png`, `top.png`, `radar.png`, `side1..3.png`, `power.png`, `bkgdlg.png`), Full-Bleed Cameo-Grid mit Original-Beschriftung, 100% vertikales 3D-Schlachtfeld im Player Game Mode, `.shp`-Isometrie-Ausrichtung (`+38°` Yaw-Korrektur), glitzernde Erz-Kristallteppiche & 3D-VFX-Flipbook-Billboards (`vfx_catalog.json`)

---

### Phases 12–16: Dual-UI-Architektur (`bevy_ui` + `IRONLOCK Creator Workbench`) & Next-Gen Engine-Kernsysteme (**Tasks 157–180**)

- [x] **Phase 12 (Tasks 157–161):** GUI-Neuordnung, UTF-8-Schriften & Hardware-RTS-Cursors, modulare `bevy_ui`-Widgets, schwebende In-Game-Spieler-Subfenster (Diplomatie, Tech-Tree, Logbuch, Optionen) sowie die `IRONLOCK Creator Workbench` (`bevy_egui` + `egui_dock` Multi-Window Docking-Shell, `EditorHistory` Undo/Redo, Asset-Browser, Szenen-Hierarchie & Roundtrip-TOML-Inspektor)
- [x] **Phase 13 (Tasks 162–166):** Exakte Sub-Tick-Transform-Interpolation (`Prev -> Curr`), GPU-Instanced World-UI & 3D-Prop-Meshes (Gizmo-Ablösung), kontinuierliche Heightmaps & 4-Layer WGSL Terrain-Splatmapping (`MapDef::synthesize_height_and_splat`), animierter Wasser-Shader & Smooth Fog-of-War sowie Creator-Subfenster 4 (Interaktiver 3D-World-Builder)
- [x] **Phase 14 (Tasks 167–170):** Zustandsgesteuertes Animationssystem (`EntityAnimState` / `AnimClipMap`), benannte Bone-/Muzzle-Sockets (`SocketOffset`) & Gebäude-Schadensmodelle, 3D-Spatial-Audio mit 5-Kanal-Bus-Mixer (`AudioBusKind`) & fraktionsspezifischer EVA-Queue (`faction_eva_cues`), nativer In-Engine FMV-Video-Decoder & Creator-Subfenster 7 (3D-Asset-Turntable & Socket-Editor)
- [x] **Phase 15 (Tasks 171–175):** Brushfire-Clearance, HPA\* & Shared Flow-Fields, deterministisches ORCA Local Steering & datengetriebene Fahrzeug-/Flugzeug-Kinematik (`LocomotorSpec`), physikalische 3D-Ballistik (`BallisticSpec`), Sichtlinien-Okklusion & zerstörbare Stadtumgebung (`CivilianStructureDef`), hierarchische strategische KI (`AiCommanderState`, Influence-Maps, Base-Planner) & Creator-Subfenster 8 (Diagnostics, Profiler & AI-Heatmaps)
- [x] **Phase 16 (Tasks 176–180):** Komponierbares Trait- & Multi-Ressourcen-SDK (`resources.toml`, `BuilderTraitSpec`, `GathererTraitSpec`, `SquadTraitSpec`, Referenz-Mod `mods/sdk-showcase-workers-squads`), Creator-Subfenster 5 (Spreadsheet-Balance-Editor & Tech-Graph), Creator-Subfenster 6 (Visueller Trigger- & Skript-Editor), deterministischer Fixed-Point-Kern (`SimScalar = I32F32`) & UDP-Lockstep mit Jitter-Buffer, Hot-Reconnect, Delayed Spectator & `rts-dedicated-server`
- [x] **Phase 17 (Tasks 181–185):** 3D-Editor-Direktinteraktion, kontinuierliches Heightfield-Raycasting & Mehr-Ebenen-Brückentrennung (`CollisionGrid`), gechunktes `16×16`-Terrain-Streaming, 3D-Viewport-Gizmos & Live-Pinselringe, isolierter Offscreen-3D-Turntable (`RenderTarget::Image`), interaktiver Node-Graph & QWER/WASD-Hotkey-System
- [x] **Phase 18 (Tasks 186–191):** Synchronisierter Pre-Game Multiplayer Staging Room & In-Game-Chat, aktiver UDP-Fast-Path, automatischer Custom-Map-Transfer, Auto-Reconnect, Desync-Forensik, Token-Multiplex-Relay, signierte Glicko-2/ELO-Matches & Automatch, versioniertes Zlib-`SaveGameEnvelope` + `MultiCampaignSaveStore` + Multi-Slot Save/Load-UI, `MatchTelemetry` + 4-Tab Post-Game-Statistik-Screen & dediziertes 3D-Replay-/Spectator-HUD
- [x] **Phase 19 (Tasks 192–196):** Next-Gen RTS-Grafik mit HDR-Kamera, Bloom, SSAO, prozeduraler Skybox/IBL & Cascaded Shadows, `4×4`-Splat-Texel/Tile PBR-Terrain mit Height-Blending & Triplanar-Klippen, höhenkonforme Krater-/Kettenspur-Decals, 3D-Fahrzeugwracks & echtes GPU-Partikel-Instancing, native Bevy `AnimationPlayer`/`AnimationGraph` glTF-Skelettanimation sowie A/V-synchrone Video-Cutscenes & `.ogg`/`.flac` 3D-Spatial-Audio
- [x] **Phase 20 (Tasks 197–204):** Universelle `Shift`-Befehlswarteschlange (`VecDeque<QueuedOrder>`) & Rechtsklick-Drag-Formationen, vereinheitlichte Zivil-Garnisonen mit Ruinen, Auto-Connect-Mauern, animierte Tore & physische Support-Flugzeuge, durchgängiger `SimScalar`-Determinismus + deterministische Rhai/Lua-Mathe-Sandbox, `1.200+`-Einheiten-Skalierung (`WorldSpatialIndex` & `PlayerVisibilityGrid` `< 8 ms`), durchgängige EN/DE-Lokalisierung (`LocaleCatalog`), vollständige Westwood-Konverter-Suite (`.csf`, `.aud`, `IsoMapPack5`, Multi-Frame-`.hva`, Blowfish-`.mix`), direktes `RTSMOD03` `.rtsmod`-VFS-Mounting + `cook-standalone` Game-Exporter & 8-Stufen-End-to-End-Abnahmegate


---

### Phases 21–24: Commercial Engine, `rust-ra2` & Creator Workbench Completion (**Tasks 205–228**)

- [x] **Phase 21 (Tasks 205–210):** `1.200`-Unit Client-GPU/LOD-Skalierung (`glb_optimizer.rs` `LOD0`/`LOD1`, `VisibilityRange`, Frustum-/FoW-Skip, `FixedEntitySockets`), Shader-Fraktionsfarben (`TeamDamageMaterialPalette`), 3D-Baugerüste, Schadens-Verkohlung & Wetter-Partikel/Wolkenschatten, opake `256×256` Top-Down-Terrain-Minimap mit Perspektiv-Trapez & Team-Pings, Cameo-Layout-Bereinigung + klickbare `[1..9, 0]`-Kontrollgruppenleiste + `[Leertaste]`-Event-Ring + `IDLE [N]`, interaktive 26-Track-Musik-Jukebox (`MusicMood` Peace/Combat-Crossfade) & A/V-synchroner Video-Start sowie vereinheitlichte `saves/settings.toml`-Persistenz (`UnifiedClientSettingsFile`)
- [x] **Phase 22 (Tasks 211–216):** Strukturierte `MissionObjectiveDef`s (`Primary`/`Secondary`/`Bonus`), `MM:SS`-Countdown-Timer, VIP-Tags (`PreplacedEntityDef.tag`) & dynamische Kartengrenzen (`ExpandPlayableBounds`), In-Engine Cutscene-Kamera-Splines (`CameraCutsceneDef`) mit Letterbox & Porträt-Transmissionen, interaktives 8-Schritte Bootcamp-Tutorial & Kampagnen-Missions-Upgrade, uferseitige `BridgeRepairHut`-Reparaturhütten + `tech_communications_center` + Generals `CommanderPowerTreeDef` (`1★–5★`), Taktische Squads 2.0 (`Light`/`Heavy`-Cover, `Pinned`, `RetreatSquad`, Waffen-Slot-Upgrades) & Modbare Strategie-KI 2.0 (freie Persönlichkeiten, Marine-Landungen, Chokepoint-Mauern, Service-Depot-Retreat, Zweit-MCV-Expansion & volle Rhai/Lua-`on_ai_tick`-Steuerung)
- [x] **Phase 23 (Tasks 217–221):** Disk-gestützte Studio-Katalog-Persistenz (`LoadedMod::upsert_*` / `delete_*` / `find_id_references`) & Vertex-Stride-Splatmaps (`(width + 1) * (height + 1)`), vollständiger 10-Schema Property Inspector & zweisprachiger Side-by-Side `LocaleCatalog`-Editor, Map Builder Pro (Catmull-Rom Straßen-/Klippen-/Fluss-Splines, Auto-Ramp-Glättung, `MapPrefabDef` `.prefab.toml`-Stempel, Cutscene-Gizmos & Live-Wetter), Script-Studio IDE (Syntax-Highlighting, Fehler-Sprung, API-Browser, Live-Watch) + 5 Save/Load-Fixes sowie moderner `.glb`/`.png`/`.ogg`-Asset-Importer, Deep-Binary-Header-Validator (`validate_binary_asset_header`) & Offscreen-Cameo-Generator (`render-cameos`)
- [x] **Phase 24 (Tasks 222–228):** MP Lag-Timeout-Detektor (`PeerStallStatus`, Vote-Drop / KI-Übernahme) & bidirektionales UDP-Hole-Punching (`UdpPunchCoordinator`), deterministisches Multiplayer-Save (`.rtssav`) & synchronisierter Staging-Room-Resume, zweistufiger `120s`-Spectator-Snapshot-Ring mit konfigurierbarem Delay & Live-`Spectate`, Online Mod- & Map-Workshop (`centralwebserver` `/api/v1/workshop/*`, In-Game-Browser & Lobby-Auto-Download), erweiterte `test-mod`-Szenario-Assertions für `rust-ra2` & `sdk-showcase-workers-squads`, Cross-OS CI/CD-Matrix (`ubuntu-latest`, `windows-latest`, `macos-latest`) + Standalone-Release-Cooker (`.github/workflows/{ci,release}.yml`) & 10-Stufen End-to-End Vulkan-Abnahmegate (`01..12`)


---

### Phases 25–28: Modular Multi-Genre RTS Engine, Strategic Zoom, Creator Workbench 2.0 & Rollback/Dedicated Fleet (**Tasks 229–252**)

- [x] **Wave 1 SDK Foundation (`rts-sdk` — Tasks 229, 230–232-Schema, 234-Split, 240/242–245-Schema):** Multi-Mod Dependency DAG (`depends_on` + SemVer, Zykluserkennung, `stack_content_hash`), geschichtetes Multi-Archiv-`ModVfs` (`vfs_layers`), `$append`/`$remove`/`$delete`-Patches, `schemars::JsonSchema` auf allen 12+2 Katalogen, Schemata für `ArmorTag`×`WarheadTag`, `BuffModifierSpec`/`StackPolicy`, `EconomyModel::Streaming`, `EraDef`, `HeroTraitSpec` + `ItemCatalog`, `ScriptContext` 2.0 + `FlatSpatialScriptGrid`, `HudLayoutPreset`, `TriggerGraphDef`, `BehaviorTreeDef`, `VfxCatalog`, PBR-`VisualDef` sowie `< 800`-Zeilen-Modularisierung von `loader/` & `validator/`
- [x] **Phase 25 (Tasks 229–234):** Multi-Mod-DAG & `schemars` Auto-JSON-Schemas, Sparse-Trait-Komposition (`EntityTraitStores`, `DefId`/`TagId`), freie `ArmorTag`×`WarheadTag`-Schadensmatrix & `BuffModifierStack`, BAR/SupCom Streaming-Ökonomie (`EconomyModel::Streaming`), Epochen (`EraDef`) & lokale Gebäude-Forschungsqueues, WC3-Helden (`HeroTraitSpec`, Mana, unabhängige Cooldowns, 6-Slot-Inventar) & Lazy-Skript-API 2.0, `O(1)`-Flow-Field-Steering, Dirty-Sektor-HPA\*, Velocity-Space ORCA, 3D-Segment-Ballistik sowie flache Binär-Grids & `< 0,5 ms` `RTSB01`-Snapshots
- [x] **Phase 26 (Tasks 235–240):** `rts_client`-Library mit modularer Bevy `PluginGroup`, `States<AppPhase>`, typisierten `Event`s & Resource-Split, `unit_pbr_team.wgsl` (`ExtendedMaterial`) & statisches Prop-GPU-Instancing, nahtloser Orbital Strategic Zoom (`12m..550m`) mit GPU-Taktik-Icons & globaler `[Shift]`-Pfadübersicht, GPU-Terrain-Shader-FoW, projizierte Normal-Decals & GPU-Shader-Partikel, echtes 3D-Listener-Audio + Freihand-Kurvenformationen, Kreis-Area-Befehle & Smart-/Auto-Cast sowie modbare HUD-Layout-Presets (`ClassicRightSidebar`, `ModernBottomCommandCard`, `CompactBarOverlay`), Global Production Strip, Helden-HUD & Farbenblind-Modi
- [x] **Phase 27 (Tasks 241–246):** Modularisierung aller `studio/**`-Großdateien (`< 750` Zeilen/Datei) & `WorkbenchPanel`-Registry, visueller Trigger- & Missions-Node-Graph-Editor (`TriggerGraphDef`) mit Live-Playtest-Tracing, datengetriebene KI-Behavior-Trees (`BehaviorTreeDef`) & visueller KI-Graph-Editor, datengetriebenes `VfxCatalog`-Schema (`vfx/vfx.toml`) & 3D-VFX-/Projektil-Studio, interaktives PBR-Material-/Team-Mask-Studio + KTX2/Mipmap/Cameo-Atlas-Cooker sowie Mod-Mutator- & Delta-Overlay-Wizard mit Override-Diff-Viewer
- [x] **Phase 28 (Tasks 247–252):** Hybrides Rollback-/Zero-Latency-Netcode (`RollbackSchedule`, `1`-Tick-LAN & TCP/`#mux:`-Relay-RTT), persistenter Crash-Restart-Rejoin (`saves/reconnect_session.json`) & reversibler KI-Slot-Reclaim (`GameCommand::ReclaimHumanSlot`), autoritative `rts-dedicated-server`-Flotten-Orchestrierung in `centralwebserver` & Docker/NS8, vollautomatische Desync-Forensik (`.diff.json` + `.replay.json` + Auto-Resync) & `lobby_registry`/`transport`-Split, Referenz-Showcase-Mod #2 (`mods/sdk-showcase-heroes-ages-mutators` + `mods/mutator-turbo-economy`) sowie `ENGINE_MODULAR_ARCHITECTURE.md` & 12-Stufen End-to-End Vulkan-Zertifizierung (`01..14`)
