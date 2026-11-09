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
