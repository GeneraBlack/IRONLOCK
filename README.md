# IRONLOCK // RUST-RTS — Modular 3D RTS Engine, Creator Workbench 2.0 & Red Alert 2 Total Conversion

> [!WARNING]
> **Experimental Version — Active Work in Progress**
> This engine and its accompanying tools and asset pipelines are currently in an **experimental development stage**. Core systems, data schemas (`mod.toml` / `.toml`), scripting APIs (`.rhai` / `.lua`), 3D rendering pipelines, and network protocols are under active development and **subject to frequent, breaking changes**.

**IRONLOCK // RUST-RTS** is a modern, fully data- and script-driven **multi-genre 3D Real-Time Strategy (RTS) Engine written in Rust (`engine/`)** built on **Bevy 0.15**. It combines a **64-bit fixed-point (`SimScalar = I32F32`) deterministic 20 Hz simulation core** (`1,200+` units in `< 8 ms`/tick), a **14-schema declarative Mod SDK** with multi-mod SemVer dependency graphs and `.rtsmod` (`RTSMOD03`) VFS archives, a dual **Rhai & Lua 2.0 sandboxed scripting runtime**, the in-game **IRONLOCK Creator Workbench 2.0 IDE (`[F12]` / `--studio`)**, **hybrid 1-tick Rollback + Lockstep multiplayer** with UDP hole-punching, Reverse-TCP relay, Ranked Glicko-2 automatch, and an integrated **Online Mod Workshop**, alongside a complete **Command & Conquer: Red Alert 2 & Yuri's Revenge Total Conversion (`rust-ra2/`)** and multi-genre SDK showcase mods.

Central Online Multiplayer Backbone, Mod Workshop & Project Portal: **`https://rust-rts.bls-isp.net/`** (automatic local fallback: `http://127.0.0.1:8080`).

---

## Quick Launch — Start & Stop Control Panel (`Game Mode` / `Engine Mode`)

A standalone start/stop launcher is included in the repository root ([`start-stop-engine.bat`](file:///d:/projekt/rust-rts/start-stop-engine.bat) and [`launcher.ps1`](file:///d:/projekt/rust-rts/launcher.ps1)):

### 1. Graphical 2-Button Launcher (Windows Desktop)
Double-click [`start-stop-engine.bat`](file:///d:/projekt/rust-rts/start-stop-engine.bat) to open the **IRONLOCK // RUST-RTS Control Panel**:
- **Mod Selector:** Choose between `rust-ra2` (*Red Alert 2 & Yuri's Revenge*), `mods/sdk-showcase-workers-squads`, `mods/sdk-showcase-heroes-ages-mutators`, `mods/openra2`, `mods/westwood-classic`, and `mods/openra-classic`.
- **`GAME MODE` Button:** Launches `rts-client.exe --mod-path <mod>` directly into playable 3D RTS combat (compiling `--release` first with live `COMPILING...` status if needed, and allowing a second local instance for multiplayer testing without force-killing an active host).
- **`ENGINE MODE` Button:** Launches `rts-client.exe --mod-path <mod> --studio` directly into the **IRONLOCK Creator Workbench 2.0** desktop modding IDE.
- **`STOP ENGINE` Button:** Gracefully closes (with force fallback) running `rts-client` instances and displays live process/PID status.

### 2. Command-Line Usage
```powershell
# Open the graphical Start & Stop Control Panel
.\start-stop-engine.bat

# Start directly in Game Mode (default mod: rust-ra2)
.\start-stop-engine.bat game

# Start directly in Engine Mode (--studio Creator Workbench IDE enabled)
.\start-stop-engine.bat engine

# Stop any running engine instance
.\start-stop-engine.bat stop
```

---

## Architecture & Workspace Modules

| Module / Directory | Package / Role | Description |
| :--- | :--- | :--- |
| [`engine/crates/rts-sdk`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/lib.rs) | `rts-sdk` | **14 Declarative Schemas** (`Units`, `Buildings`, `Weapons`, `Upgrades`, `CommanderTrees`, `Factions`, `Resources`, `AiProfiles` + `BehaviorTreeDef`, `Campaigns`, `Maps` + `TriggerGraphDef`, `Prefabs`, `Locales`, `Items`, `VfxCatalog`), `schemars` JSON-Schema generator, Multi-Mod Dependency DAG (`depends_on` + SemVer), layered `ModVfs` (`.rtsmod` `RTSMOD03` + folders), `$append`/`$remove`/`$delete` TOML patch directives, versioned `SaveGameEnvelope` (`.rtssav`), and deterministic `.rhai` & `.lua` 2.0 sandboxes. |
| [`engine/crates/rts-asset-pipeline`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/main.rs) | `rts-asset-pipeline` | Modding CLI (`rts-mod-tools` / `rts-asset-pipeline`): deep validator (`validate`), headless scenario test runner (`test-mod`), procedural `.glb` generator (`generate-models`), QEM `LOD0`/`LOD1` mesh optimizer (`optimize-models`), KTX2/mipmap/cameo-atlas texture cooker (`cook-textures`), offscreen 3D cameo studio (`render-cameos`), standalone game distribution cooker (`cook-standalone`), Westwood converter (`.mix` + Blowfish, `.vxl`/`.hva`, `.shp`, `.ini`, `.csf`, `.aud`/`.wav`, `IsoMapPack5`), and OpenRA (`MiniYAML`) converter. |
| [`engine/crates/rts-simulation`](file:///d:/projekt/rust-rts/engine/crates/rts-simulation/src/lib.rs) | `rts-simulation` | **Deterministic 20 Hz `SimScalar` (`I32F32`) Simulation Core (`SimWorld`):** `< 0.5 ms` binary `RTSB01` snapshots, 7-domain `state_hash()`, incremental `16×16` HPA\* + shared `O(1)` flow fields + velocity-space RVO2/ORCA (`1,200+` units `< 8 ms`), continuous heightfield DDA raycasting & multi-level bridge separation, sparse `EntityTraitStores` (workers, tactical squads with directional cover/suppression, heroes with mana & 6-slot inventory), `LumpSum` & `Streaming` economy models, `Age I–IV` eras, free-form `ArmorTag × WarheadTag` damage matrices, 3D ballistics, `TriggerGraphDef` runtime, and data-driven AI Behavior Trees + Influence Maps. |
| [`engine/crates/rts-network`](file:///d:/projekt/rust-rts/engine/crates/rts-network/src/lib.rs) | `rts-network` | **Hybrid 1-Tick Rollback (`RollbackSchedule`) + Lockstep Netcode:** TCP + active UDP fast-path with `AdaptiveJitterBuffer`, bidirectional UDP hole-punching (`UdpPunchCoordinator`), 3-tier NAT/UPnP port mapping, token-multiplexed Reverse-TCP Relay, 8-slot synchronized Multiplayer Staging Room, deterministic `.rtssav` MP save/resume, persistent crash-restart rejoin (`ReclaimHumanSlot`), MP Lag-Shield (`Vote Drop` / `AI Takeover`), `120s` delayed spectator ring, automatic `DesyncForensicsBundle` dumps, and headless **`rts-dedicated-server`**. |
| [`engine/crates/rts-client`](file:///d:/projekt/rust-rts/engine/crates/rts-client/src/lib.rs) | `rts-client` (`lib` + `bin`) | **Bevy 0.15 3D RTS Client (`RtsEngineClientPlugins`) & Creator Workbench 2.0 (`[F12]` / `--studio`):** HDR + Bloom + SSAO, custom WGSL PBR + Team-Color-Mask shader (`unit_pbr_team.wgsl`), 5-layer height-blended splat terrain + GPU world-XZ Fog of War (`terrain_splat.wgsl`, `water.wgsl`), seamless **Orbital Strategic Zoom (`12m..550m`)** with GPU tactical icons, normal-mapped crater/track decals, GPU-instanced particles & `vfx.toml` emitters, glTF skeletal `AnimationGraph`, 5-bus 3D spatial audio + 26-track Jukebox, in-engine A/V-synced 4K FMV decoder, switchable HUD presets (`ClassicRightSidebar`, `ModernBottomCommandCard`, `CompactBarOverlay`), and live `EN`/`DE` localization. |
| [`rust-ra2/`](file:///d:/projekt/rust-rts/rust-ra2/mod.toml) | `rust-ra2` | Complete *Command & Conquer: Red Alert 2 & Yuri's Revenge* Total Conversion: **3 factions** (*Allies*, *Soviet*, *Yuri* + sub-factions), **77 units**, **76 buildings**, **63 weapons**, **5 campaigns (40 wired missions)** + interactive **8-step Bootcamp Tutorial**, **54 playable `.toml` maps** (+ 298 original `.map` files), **150+ `.glb` 3D models (`LOD0`/`LOD1`)**, **404 audio tracks/cues**, **138 4K/HD 30 FPS videos**, **860+ AI-remastered UI/VFX textures**, and complete `en.toml`/`de.toml` localization. |
| [`mods/`](file:///d:/projekt/rust-rts/mods) | Reference & Showcase Mods | Includes **`sdk-showcase-workers-squads`** (3 resources, Dozer workers, 6-man tactical cover squads), **`sdk-showcase-heroes-ages-mutators`** (BAR-style streaming economy, Ages I–III, Hero with mana/inventory, custom damage matrix, AI behavior tree, `ModernBottomCommandCard`), **`mutator-turbo-economy`** (stacked delta overlay mod), plus `openra2`, `westwood-classic`, and `openra-classic`. |
| [`centralwebserver/`](file:///d:/projekt/rust-rts/centralwebserver/README.md) | `rust-rts-centralwebserver` | Node.js/TypeScript web platform for **`rust-rts.bls-isp.net`** (packaged for rootless **NethServer 8 / Podman** in [`Containerfile.ns8`](file:///d:/projekt/rust-rts/centralwebserver/Containerfile.ns8)): project portal, live lobby browser, **Online Mod & Map Workshop** (`/api/v1/workshop`), **Ranked Glicko-2/ELO Automatch** (`/api/v1/matchmaking`), **Dedicated Server Fleet Orchestrator** (`/api/v1/servers`), and Reverse-TCP/UDP Relay Hub (**Ports `30100–30139`**). |

---

## Key Engine & Creator Workbench 2.0 Capabilities

### 1. Dual-UI Architecture: Player Mode vs. `IRONLOCK Creator Workbench 2.0` (`[F12]`)
- **Pure Tactical Player Mode (`bevy_ui`):**
  - Switchable HUD Layouts (`ClassicRightSidebar` with authentic RA2 faction skins and `104×74` full-bleed cameos, `ModernBottomCommandCard`, and `CompactBarOverlay`), opaque `256×256` hillshaded minimap with 4-corner heightfield camera trapezoid and `Alt+Click` team pings, clickable `[1..9, 0]` control groups, `[Spacebar]` radar event ring, and right-click freehand curve/line formations.
  - **8 Floating In-Game Sub-Windows:** Mission Briefing & A/V-Synced FMV Player, Diplomacy & Tribute (`[D]`), Tiered Visual Tech Tree (`[Y]`), Generals `1★–5★` Commander Power Tree, 26-Track Interactive Soundtrack Jukebox (`[J]`) with `1.5s` Peace/Combat crossfade, Multi-Slot `.rtssav` Save/Load Archive (`[F5]`/`[F8]`), Mission Logbook (`[Tab]`), and 4-Tab Options (`saves/settings.toml`).
- **IRONLOCK Creator Workbench 2.0 (`bevy_egui` + `egui_dock` Desktop IDE):**
  - **Interactive Central 3D Viewport:** Centimeter-accurate 3D heightfield DDA raycasting, 3D XYZ translation & rotation gizmos, reversible `EditorHistory` (`Ctrl+Z` / `Ctrl+Y`), **1-Click Playtest & Snapshot Restore**, and transactional Hot-Reload (`[Ctrl+F12]`).
  - **12+ Specialized Dockable Studio Panels:**
    1. **Asset Browser & 10-Schema CRUD Wizards** (with automatic `find_id_references` dependency protection and modern `.glb`/`.png`/`.ogg` + Westwood `.mix` import wizards)
    2. **Scene Hierarchy & Layer Outliner** (visibility `👁`, lock `🔒`, and camera snap)
    3. **Complete 10-Schema Property Inspector & Side-by-Side EN/DE Locale Editor**
    4. **Map Builder Pro** (16×16 chunked terrain sculpting, 5-layer splat painting, `2-Way`/`4-Way` symmetry, Catmull-Rom road/cliff/river splines, auto-ramp smoothing, `.prefab.toml` stamps, and cutscene spline gizmos)
    5. **Spreadsheet Balance Editor, Visual Tech-Tree Graph & 1-Click Headless Combat Duel Simulator**
    6. **Visual Trigger & Mission Node-Graph Editor** (with live playtest execution wire highlighting) + **Rhai/Lua Script IDE** (syntax highlighting, API browser, live variable watch)
    7. **Isolated Offscreen 3D Asset Turntable (`512×384`), Bone-Socket Editor & 1-Click Cameo Renderer**
    8. **Live Diagnostics, Linter Jump-to-Field, Tick Profiler & 3D AI/Navigation Heatmaps**
    9. **Visual AI Behavior-Tree Graph Editor** with live Blackboard inspector
    10. **Interactive 3D VFX, Particle & Projectile Studio (`vfx/vfx.toml`)**
    11. **PBR Material, Team-Color-Mask & KTX2 Texture Studio**
    12. **Mod Overlay / Mutator Wizard & 1-Click Standalone Game Exporter (`RTSMOD03`)**

### 2. 3D Asset Reconstruction & 4K AI Remastering Pipeline (`rust-ra2`)
1. **3D Buildings, Superweapons & Landmarks (`71+` `.glb` Models — AI Image-to-3D + Isometric Unprojection + QEM `LOD0`/`LOD1`):**
   - Reconstructed via **Tencent Hunyuan3D-2 (`hunyuan3d-dit-v2-0`) + Blender 5.2 Ultra-Sharp Pipeline** into watertight 360° 3D architectural meshes with 3D mean-curvature ridge/cavity enhancement, pure-albedo color baking, and pure-Rust QEM `LOD0`/`LOD1` multi-node hierarchies (`glb_optimizer.rs`) for `1,200+` unit mass battles.
2. **3D Vehicles, Aircraft, Ships & Missiles (`50+` `.vxl` $\rightarrow$ `.glb` Models):**
   - Multi-octave (`64×` volumetric sub-voxel density) edge-directed voxel super-resolution ([`voxel_meshing.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/westwood/voxel_meshing.rs)) with `3×3×3` cavity AO, Taubin ($\lambda/\mu$) smoothing, and articulated `Hull` / `Turret` / `Barrel` / `Rotor` sub-nodes.
3. **3D Infantry & `.shp` Special Units (`26+` `.shp` $\rightarrow$ `.glb` Models):**
   - `16×` xBRZ/vector super-resolution + multi-frame orthogonal 3D anatomical sculpting ([`sprite_relief.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/westwood/sprite_relief.rs)) with skeletal animation clips.
4. **4K UHD (`2880×2160` @ `30 FPS`) FMV Cutscenes & HD Radar-PIP Videos (`138` `.mp4` Videos):**
   - Optical-flow motion-interpolated `30 FPS` progressive frames upscaled via Vulkan `libplacebo` (`ewa_lanczossharp`) + 2-pass GLSL WarpSharp shaders (`55` full-screen FMVs and `83` radar PIPs), streamed natively in-engine with frame-accurate audio sync.
5. **Neural-AI 2D UI, Cameos, Loading Screens & VFX (`860+` PNGs) & Audio (`404` WAV/OGG Tracks):**
   - **51 Loading Screens** (`3200×2400`), **240+ Unit/Building Cameos** (`480×384`), **36 Faction HUD Panels**, **517 Mouse Cursors**, **16 VFX Flipbooks**, and **404 Audio Tracks** (`192` EVA cues, `100` combat SFX, `80` country taunts, and `26` stereo `.ogg` music tracks).

---

## Developer CLI & Build Instructions

### 1. Build, Test & Run the 3D Engine
```powershell
# Compile the entire Rust workspace in release mode
cargo build --manifest-path engine/Cargo.toml --release

# Run all unit, integration, and determinism tests across all 5 engine crates
cargo test --manifest-path engine/Cargo.toml --workspace --all-targets

# Launch the 3D Client in Player Game Mode (Red Alert 2 & Yuri's Revenge)
.\engine\target\release\rts-client.exe --mod-path rust-ra2

# Launch the 3D Client in Creator Workbench 2.0 IDE Mode (--studio / [F12])
.\engine\target\release\rts-client.exe --mod-path rust-ra2 --studio

# Launch with a stacked Mod + Mutator overlay
.\engine\target\release\rts-client.exe --mod-path mods/sdk-showcase-heroes-ages-mutators --overlay-mod mods/mutator-turbo-economy
```

### 2. Asset Pipeline, Mod Validation, Testing & Standalone Cooking (`rts-asset-pipeline`)
```powershell
# Deep-validate a mod (schemas, cross-references, binary .glb/.png/.ogg/.ktx2 headers, .rhai & .lua scripts)
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- validate --mod-path rust-ra2

# Run automated headless gameplay & lockstep-determinism scenario tests (tests/*.toml)
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- test-mod --mod-path rust-ra2

# Cook a zero-extraction standalone desktop game distribution (.rtsmod RTSMOD03 VFS bundle)
cargo run --release --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- cook-standalone --mod-path rust-ra2 --output-dir dist/ironlock-ra2 --title "IRONLOCK: Red Alert Total Conversion"

# Regenerate JSON Schemas (schemars), taplo.toml, and SDK reference documentation
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- gen-docs --output docs/MOD_DEV_KIT.md --schemas-dir schemas
```

### 3. Hybrid Rollback/Lockstep Multiplayer, Dedicated Server & Central Webserver
```powershell
# Host a multiplayer match (registers on https://rust-rts.bls-isp.net or local http://127.0.0.1:8080,
# configures TCP/UDP UPnP port forwarding, UDP hole-punching, and token-multiplexed Relay fallback)
.\engine\target\release\rts-client.exe --mod-path rust-ra2 --host 0.0.0.0:7878 --lobby-name "Operation Red Dawn"

# Run the authoritative headless Dedicated Server (supports staging rooms, mod stacks & fleet orchestration)
cargo run --release --manifest-path engine/Cargo.toml -p rts-network --bin rts-dedicated-server -- --bind 0.0.0.0:7878 --mod-path rust-ra2

# Run the Central Node.js/TypeScript Webserver, Workshop, Matchmaking & Relay Hub
cd centralwebserver
npm install
npm test
PUBLIC_HOST=rust-rts.bls-isp.net RELAY_PORT_START=30100 RELAY_PORT_END=30139 PORT=8080 npm start
```

---

## Documentation & Technical Specifications

- [`docs/ENGINE_MODULAR_ARCHITECTURE.md`](file:///d:/projekt/rust-rts/docs/ENGINE_MODULAR_ARCHITECTURE.md) — Modular crate architecture, `RtsEngineClientPlugins` embedding API, layered `ModVfs`, Strategic Zoom, and Rollback/Dedicated Fleet netcode
- [`docs/CREATOR_WORKBENCH_GUIDE.md`](file:///d:/projekt/rust-rts/docs/CREATOR_WORKBENCH_GUIDE.md) — Complete guide to the 12-panel `IRONLOCK Creator Workbench 2.0` desktop IDE (`[F12]` / `--studio`)
- [`docs/MOD_SCHEMA_REFERENCE.md`](file:///d:/projekt/rust-rts/docs/MOD_SCHEMA_REFERENCE.md) — Declarative TOML schema reference across all 14 SDK catalogs, traits, and patch operators
- [`docs/STANDALONE_PUBLISHING.md`](file:///d:/projekt/rust-rts/docs/STANDALONE_PUBLISHING.md) — Zero-extraction `RTSMOD03` archive cooking (`cook-standalone`) and standalone distribution guide
- [`docs/MOD_DEV_KIT.md`](file:///d:/projekt/rust-rts/docs/MOD_DEV_KIT.md) — Modder SDK, CLI toolchain, and `.rhai` / `.lua` 2.0 scripting handbook
- [`CAPABILITY_MAP.md`](file:///d:/projekt/rust-rts/CAPABILITY_MAP.md) — Module capability map and dependency graph across all engine and server components
- [`SPEC.md`](file:///d:/projekt/rust-rts/SPEC.md) — Master technical specification (`SPEC-rts-mod-sdk.md`, `SPEC-rts-sim-core.md`, `SPEC-rts-mod-tools.md`, `SPEC-rts-net.md`, `SPEC-rts-client-3d.md`, `SPEC-mod-rust-ra2.md`)
- [`centralwebserver/README.md`](file:///d:/projekt/rust-rts/centralwebserver/README.md) — Central multiplayer webserver, Workshop, Matchmaking & NethServer 8 (`Containerfile.ns8`) deployment guide

---

## Non-Commercial Community & Asset Reconstruction Notice

> [!IMPORTANT]
> **Strictly Non-Commercial Project — Never for Sale or Monetization**
> **IRONLOCK // RUST-RTS** is an independent, strictly **non-commercial** open-source community, research, and preservation project. It is **never intended for commercial use, sale, paywalling, or monetization of any kind**.

- **100% Clean-Room Engine Implementation:** The entire Rust engine (`engine/`), hybrid rollback/lockstep networking stack, scripting runtime, Creator Workbench IDE, and central multiplayer server (`centralwebserver/`) are written from scratch without using any proprietary code.
- **Transformative Re-Engineering & Preservation of Lost Classics:** Because the original Westwood Studios C++ source code and 3D Studio Max (`.max`) production assets for *Red Alert 2* and *Yuri's Revenge* were historically lost following the studio's closure in 2003, all visual and multimedia assets in `rust-ra2/` have undergone extensive, transformative technical reconstruction:
  - **3D Models (`.glb`):** Synthesized into full 360° 3D meshes (`LOD0`/`LOD1`) from low-resolution legacy 2D pixels and voxel slices via neural Image-to-3D generation (**Tencent Hunyuan3D-2**), Blender 3D mean-curvature sculpting, and `64×` volumetric super-resolution.
  - **Graphics & UI (`.png`):** Neural-reconstructed up to `4×–16×` resolution via multi-model ensembles (**Waifu2x CUNet**, **Real-ESRGAN**, and **xBRZ** vectorization).
  - **Videos (`.mp4`) & Audio (`.wav` / `.ogg`):** Restored from legacy interlaced `15 FPS` streams into motion-interpolated `30 FPS` 4K UHD (`2880×2160`) video via optical-flow synthesis and GLSL shader pipelines, alongside modern PCM/Vorbis audio decoding.
- **Trademarks:** *Command & Conquer*, *Red Alert 2*, and *Yuri's Revenge* are trademarks of Electronic Arts Inc. This project is a community-driven preservation and technical showcase effort and is not affiliated with or endorsed by Electronic Arts.
