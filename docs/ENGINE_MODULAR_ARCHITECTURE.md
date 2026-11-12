# Modular 3D RTS Engine Architecture (`ENGINE_MODULAR_ARCHITECTURE.md`)

This document describes the modular crate architecture, Bevy `PluginGroup` embedding API, ECS schedule & resource topology, multi-layer `.rtsmod` VFS stack, rendering & Strategic Zoom pipeline, data-driven HUD presets, and hybrid rollback/dedicated-server netcode of the **Rust 3D RTS Engine**.

---

## 1. Crate Dependency Graph & Zero-Hardcoded-Content Boundary

```text
                ┌──────────────────────────────────────┐
                │              rts-sdk                 │
                │  14 Declarative Schemas (`schemars`) │
                │  Multi-Mod DAG (`depends_on`/SemVer) │
                │  Layered `ModVfs` & Sandboxed Rhai/  │
                │              Lua 2.0                 │
                └──────────────────┬───────────────────┘
                                   │
           ┌───────────────────────┼───────────────────────┐
           ▼                       ▼                       ▼
┌─────────────────────┐ ┌─────────────────────┐ ┌─────────────────────┐
│   rts-simulation    │ │ rts-asset-pipeline  │ │  centralwebserver   │
│  Fixed-Point 20 Hz  │ │  CLI Validator,     │ │  Lobby, Relay,      │
│  Sparse Trait Stores│ │  JSON-Schema Gen,   │ │  Workshop, Glicko-2 │
│  Flow-Field + ORCA  │ │  KTX2/Cameo Cooker  │ │  & Dedicated Fleet  │
│  `RTSB01` Snapshots │ │  `cook-standalone`  │ │  Orchestrator       │
└──────────┬──────────┘ └─────────────────────┘ └──────────▲──────────┘
           │                                               │
           ▼                                               │
┌─────────────────────┐                                    │
│     rts-network     │────────────────────────────────────┘
│  Lockstep + 1-Tick  │
│  `RollbackSchedule` │
│  Crash-Reconnect &  │
│  Desync Forensics   │
└──────────┬──────────┘
           │
     ┌─────┴──────────────────────────┐
     ▼                                ▼
┌─────────────────────────┐  ┌─────────────────────────┐
│  `rts_client` (lib/bin) │  │ `rts-dedicated-server`  │
│ `RtsEngineClientPlugins`│  │ Authoritative Headless  │
│ Bevy 0.15 3D Client &   │  │ Staging + Lockstep Host │
│ Creator Workbench 2.0   │  │ (Docker / NS8 Ready)    │
└─────────────────────────┘  └─────────────────────────┘
```

- **`rts-sdk`:** Zero simulation or rendering dependencies. Owns `ModManifest` (`depends_on`, `default_hud_layout`, `economy_model`), all 14 declarative catalogs (`Units`, `Buildings`, `Weapons`, `Upgrades`, `CommanderTrees`, `Factions`, `Resources`, `AiProfiles` + `BehaviorTreeDef`, `Campaigns`, `Maps` + `TriggerGraphDef`, `Prefabs`, `Locales`, `Items`, `VfxCatalog`), `ModArchive` (`RTSMOD03`), layered `ModVfs`, `$append`/`$remove`/`$delete` TOML patch operators, and deterministic `.rhai` / `.lua` 2.0 sandboxes.
- **`rts-simulation`:** Pure deterministic `SimScalar` fixed-point simulation (`20 Hz` tick rate). Zero GPU/OS-clock dependencies. Uses `DefId(u16)` / `TagId(u16)` hot-path interning, sparse `EntityTraitStores`, free-form `ArmorTag × WarheadTag` damage matrices, `BuffModifierStack`, `EconomyModel::{LumpSum, Streaming}`, `EraDef` progression, `HeroInstanceState` + 6-slot inventory, `O(1)` `FlowFieldHandle` steering + incremental `16×16` HPA\* + velocity-space ORCA, and `< 0.5 ms` binary `RTSB01` snapshots.
- **`rts-network`:** Deterministic lockstep + 1-tick (`50 ms` LAN) `RollbackSchedule` ring buffer, UDP + TCP/`#mux:` relay transport with monotonic RTT measurement, `120s` anti-cheat delayed spectator ring, persistent crash-restart rejoin (`ReconnectSessionInfo` + `ReclaimHumanSlot`), and automatic `DesyncForensicsBundle` generation.
- **`rts-client` (`lib` `rts_client` + thin `bin` `rts-client`):** Embeddable Bevy 0.15 client library exposing `RtsEngineClientPlugins`.

---

## 2. Embedding `rts_client` as a Library (`RtsEngineClientPlugins`)

`engine/crates/rts-client/Cargo.toml` exposes both `[lib] name = "rts_client"` and a minimal `< 50`-line `[[bin]] name = "rts-client"` entrypoint. External standalone RTS titles or custom launchers can embed the entire engine (or selectively disable individual plugins such as `CreatorWorkbenchPlugin` or `VisualSmokeHarnessPlugin`) using standard Bevy `PluginGroup` composition:

```rust
use bevy::prelude::*;
use rts_client::{
    register_mod_vfs_asset_source, resolve_workspace_root_for_mod, AssetRootResource,
    ModResource, RtsEngineClientPlugins, SimWorldResource,
};
use rts_client::studio::CreatorWorkbenchPlugin;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::SimWorld;

fn main() -> anyhow::Result<()> {
    let loaded = LoadedMod::load_mod_with_overlays(
        std::path::Path::new("mods/sdk-showcase-heroes-ages-mutators"),
        &[std::path::PathBuf::from("mods/mutator-turbo-economy")],
    )?;
    let sim_world = SimWorld::from_mod(&loaded);
    let ws_root = resolve_workspace_root_for_mod(&loaded.root_dir);

    let mut app = App::new();
    // Mount multi-layer `.rtsmod` + directory VFS BEFORE `DefaultPlugins`
    register_mod_vfs_asset_source(&mut app, &loaded, ws_root.clone());

    app.add_plugins(DefaultPlugins)
        .insert_resource(AssetRootResource(ws_root))
        .insert_resource(ModResource(loaded))
        .insert_resource(SimWorldResource(sim_world))
        .add_plugins(
            RtsEngineClientPlugins
                .build()
                .disable::<CreatorWorkbenchPlugin>(), // Pure retail player build
        )
        .run();
    Ok(())
}
```

### Bundled Domain Plugins in `RtsEngineClientPlugins`

| Plugin | Responsibility |
| :--- | :--- |
| `RtsStatePlugin` | Initializes `States<AppPhase>` & `States<MenuScreen>`, typed `Event`s (`MatchLifecycleEvent`, `ModSwitchEvent`, `ReplayControlEvent`, `HudPresetChangedEvent`, `DesyncRecoveryEvent`), split domain sub-resources (`SelectionDomainState`, `CommandCursorDomainState`, `MultiplayerSessionDomainState`, `DisplayAudioDomainState`), and `RtsSystemSet` ordering. |
| `RtsCameraPlugin` | Damped orbital RTS camera rig (`12m..550m` zoom-to-cursor, `Alt+MMB` orbit, edge scroll, trauma shake, and Strategic Zoom crossfade thresholds). |
| `ClientRenderingAvPlugin` | Custom WGSL PBR + Team-Color-Mask `ExtendedMaterial` (`unit_pbr_team.wgsl`), 5-layer vertex splat terrain + GPU world-XZ FoW (`terrain_splat.wgsl`, `water.wgsl`), GPU-instanced static props & Strategic Zoom icons, normal-mapped decals, GPU instanced particles (`particles_instanced.wgsl`), and A/V-synced FMV streaming. |
| `RtsInputPlugin` | Tactical LMB/RMB picking against heightfield DDA, Smart-Cast, RMB freehand curved formations (`MoveAlongCustomCurve`), LMB circle-drag area commands (`AreaRepair`, `AreaHarvest`, `AreaAttack`, `AreaGuard`), and `UiInputBlocker` `ComputedNode` hit-testing. |
| `RtsHudPlugin` | Live-switchable `HudLayoutPreset`s (`ClassicRightSidebar`, `ModernBottomCommandCard`, `CompactBarOverlay`), Global Production/Era/Streaming Economy Strip, Hero Level/Mana/6-Slot Inventory Bar, accessible `256×256` minimap, and 8 floating sub-windows. |
| `RtsAudioPlugin` | 5-bus spatial 3D camera-listener audio (`SpatialListener` synced to `Camera3d`), distance attenuation, 26-track peace/combat crossfade jukebox, and procedural SFX synthesizer fallback. |
| `RtsNetLockstepPlugin` | `20 Hz` `FixedUpdate` lockstep & 1-tick `RollbackSchedule` stepper, `saves/reconnect_session.json` crash-recovery persistence, automatic `DesyncForensicsBundle` dump + `apply_authoritative_resync`, and async lobby/workshop task polling. |
| `CreatorWorkbenchPlugin` | Modular 12-panel `bevy_egui` + `egui_dock` desktop IDE (`< 750` lines/file): Visual Trigger Node-Graph Editor, AI Behavior Tree Canvas, 3D VFX & Material Studio, Mod Overlay/Mutator Wizard, Map Builder Pro, and 10-schema CRUD. |
| `VisualSmokeHarnessPlugin` | Automated 14-stage (`01..14`) `1440×900` Vulkan screenshot certification harness in `PostUpdate`. |

---

## 3. Deterministic ECS Frame Schedule (`RtsSystemSet`)

All `Update` systems are organized into six strictly chained `RtsSystemSet` phases (verified in CI by `rts_engine_client_plugins_headless_schedule_builds_with_zero_b0001_b0002_conflicts`):

```text
FixedUpdate (20 Hz):
  └─ step_fixed_simulation  ──►FixedPostUpdate: capture_sim_snapshots_fixed

Update (Variable Refresh 60..240 Hz):
  1. RtsSystemSet::EnvRebuild          (map/mod hot-swap & chunk dirty rebuilds)
  2. RtsSystemSet::FogRefresh          (player/spectator FoW cache + `States<AppPhase>` sync)
  3. RtsSystemSet::SubwindowsAndWidgets(window/display apply, UTF-8 fonts, Cutscene Director,
                                        `HudLayoutPreset` & `ComputedNode` blocker rects, Workbench IDE)
  4. RtsSystemSet::CameraAndInput      (`update_rts_camera` -> `handle_rts_input` ->
                                        `handle_sidebar_buttons` -> `update_and_interact_radar_minimap`)
  5. RtsSystemSet::SimVisualSync       (interpolated 3D entity transforms, PBR team/damage materials,
                                        GPU Strategic Icons, decals, GPU particles, 3D spatial audio)
  6. RtsSystemSet::HudUpdate           (Global Production/Era/Streaming Strip, Hero/Inventory Bar,
                                        Sidebar Cameos, Inspector, Tactical Bar, Lag/Desync Shield)

PostUpdate:
  └─ sync_hdr_camera_ssao_and_ibl, sync_directional_shadow_quality, drive_visual_smoke_test
```

---

## 4. Multi-Mod Dependency DAG & Zero-Extraction Layered VFS

1. **Topological Mod Stack (`LoadedMod::load_mod_with_overlays`):**
   - Resolves `ModManifest.depends_on` (`id`, SemVer `version_req`, `optional`) plus lobby-selected mutator overlays (`LobbyMatchRules.active_mutators`) via Kahn/DFS topological sorting with cycle detection (`MOD_DEPENDENCY_CYCLE`).
   - Deep-merges TOML catalogs by `id` across the ordered layer stack, supporting `$append`, `$remove`, and `$delete = true` patch directives, and computes a unified 64-bit FNV-1a `stack_content_hash`.
2. **Zero-Extraction `LayeredModVfsAssetReader` (`plugins/state_events.rs`):**
   - Mounts any combination of loose mod folders and Zlib-compressed `RTSMOD03` `.rtsmod` archives in reverse layer priority (`topmost mutator -> child mod -> base mod -> workspace fallback`) on Bevy's default `AssetSource` with zero temporary disk extraction.

---

## 5. Data-Driven HUD Layouts, Accessibility & Smart Command UX

- **Three Live-Switchable `HudLayoutPreset`s (`ui/hud_layouts.rs`):**
  1. `ClassicRightSidebar`: Traditional C&C / Red Alert `268px` right command sidebar with integrated top-right radar and `2×N` cameo grid.
  2. `ModernBottomCommandCard`: SC2 / WC3 / Stormgate bottom deck — bottom-left `220×220` radar, bottom-center selected Unit/Hero Portrait + Level/XP + HP/Mana + **6-Slot Inventory**, and bottom-right **`4×3` Command/Build Card**.
  3. `CompactBarOverlay`: Minimalist BAR / Supreme Commander edge-pinned micro-panels maximizing open battlefield screen area for Orbital Strategic Zoom (`165m..550m`).
- **Exact `ComputedNode` UI Hit-Testing:** Every active HUD panel carries `HudPanelBlocker`, populating `UiInputBlocker.computed_hud_rects` from Bevy's layout engine so 3D terrain clicks never leak through any preset.
- **Colorblind Modes (`ColorblindPaletteMode`):** `Off`, `Deuteranopia`, `Protanopia`, `Tritanopia`, and `AllyEnemyHighContrast` remap 3D PBR team tints, Strategic Zoom icons, health bars, and `256×256` minimap blips simultaneously.
- **Smart Command UX:** RMB freehand curved formation drag (`MoveAlongCustomCurve`), LMB circle-drag area commands (`AreaRepair`, `AreaHarvest`, `AreaAttack`, `AreaGuard`), 1-key **Smart-Cast** at cursor, and RMB **Auto-Cast** ability toggles (`ToggleAbilityAutoCast`).

---

## 6. Hybrid 1-Tick Rollback, Crash-Reconnect & Desync Forensics

- **1-Tick Hybrid Rollback (`RollbackSchedule`):** Measures peer RTT over both UDP and TCP/`#mux:` relay frames; runs at `1`-tick (`50 ms`) latency on LAN/low-ping links and rewinds/re-simulates late peer commands seamlessly using `< 0.5 ms` `RTSB01` ring-buffer snapshots.
- **Crash-Restart Rejoin (`saves/reconnect_session.json`):** Written automatically when a multiplayer match launches (`ReconnectSessionInfo` with `reconnect_token` & `stack_content_hash`) and cleared on normal match completion. If the client restarts mid-match, Main Menu surfaces **`[⚡ REJOIN ACTIVE MATCH]`** to stream the live host snapshot and issue `GameCommand::ReclaimHumanSlot` over a temporary AI takeover slot.
- **Automated Desync Forensics:** On any `DesyncDetected` event, `LockstepServer` sends a compressed reference snapshot and `rts-client` writes `saves/desync_tick_<N>.diff.json` (entity-level field diff + 7-domain `SubsystemHashes`) and `saves/desync_tick_<N>.replay.json`, offering 1-click or automatic `apply_authoritative_resync`.
