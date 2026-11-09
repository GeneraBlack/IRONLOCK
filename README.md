# IRONLOCK // RUST-RTS — Data- & Script-Driven 3D RTS Engine & Red Alert 2 Total Conversion

> [!WARNING]
> **Experimental Version — Active Work in Progress**
> This engine and its accompanying tools and asset pipelines are currently in an **experimental development stage**. Core systems, data schemas (`mod.toml` / `.toml`), scripting APIs (`.rhai` / `.lua`), 3D rendering pipelines, and network protocols are under active development and **subject to frequent, breaking changes**.

**IRONLOCK // RUST-RTS** is a modern, fully data- and script-driven **3D Real-Time Strategy (RTS) Engine written in Rust (`engine/`)** built on **Bevy 0.15**. It features a deterministic **20 Hz lockstep simulation core**, a dual **Rhai & Lua sandboxed scripting runtime**, an integrated **in-game 3D Engine Studio & Map Editor**, **3-tier NAT/UPnP traversal with Reverse-TCP Relay multiplayer**, and a complete **Command & Conquer: Red Alert 2 & Yuri's Revenge Total Conversion (`rust-ra2/`)** with AI-reconstructed 3D `.glb` models, 4K 30 FPS remastered cutscenes, and neural-upscaled UI/VFX assets.

Central Online Multiplayer Backbone & Project Portal: **`https://rust-rts.bls-isp.net/`** (automatic local fallback: `http://127.0.0.1:8080`).

---

## Quick Launch — Start & Stop Control Panel (`Game Mode` / `Engine Mode`)

A standalone start/stop launcher is included in the repository root ([`start-stop-engine.bat`](file:///d:/projekt/rust-rts/start-stop-engine.bat) and [`launcher.ps1`](file:///d:/projekt/rust-rts/launcher.ps1)):

### 1. Graphical 2-Button Launcher (Windows Desktop)
Double-click [`start-stop-engine.bat`](file:///d:/projekt/rust-rts/start-stop-engine.bat) to open the **IRONLOCK // RUST-RTS Control Panel**:
- **Mod Selector:** Choose between `rust-ra2` (*Red Alert 2 & Yuri's Revenge*), `mods/openra2`, `mods/westwood-classic`, and `mods/openra-classic`.
- **`GAME MODE` Button:** Cleanly stops any running instance and launches `rts-client.exe --mod-path <mod>` directly into playable 3D RTS combat.
- **`ENGINE MODE` Button:** Cleanly stops any running instance and launches `rts-client.exe --mod-path <mod> --studio` with the graphical **Engine Studio, Mod Manager, 3D Map Editor, and Westwood/OpenRA Converter** overlay active.
- **`STOP ENGINE` Button:** Immediately terminates any running `rts-client` process and displays live process/PID status.

### 2. Command-Line Usage
```powershell
# Open the graphical Start & Stop Control Panel
.\start-stop-engine.bat

# Start directly in Game Mode (default mod: rust-ra2)
.\start-stop-engine.bat game

# Start directly in Engine Mode (--studio overlay enabled)
.\start-stop-engine.bat engine

# Stop any running engine instance
.\start-stop-engine.bat stop
```

---

## Architecture & Workspace Modules

| Module / Directory | Package / Role | Description |
| :--- | :--- | :--- |
| [`engine/crates/rts-sdk`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/lib.rs) | `rts-sdk` | Mod manifest (`mod.toml`), TOML data schemas (factions, units, buildings, weapons, superweapons, campaigns, maps), cross-reference validator, and dual sandboxed scripting engine (`.rhai` + `.lua`). |
| [`engine/crates/rts-asset-pipeline`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/main.rs) | `rts-asset-pipeline` | Modding CLI & asset conversion pipeline: multi-node `.glb` 3D model generator (`generate-models`), Westwood archive/asset converter (`.mix`, `.vxl` 3D voxel super-resolution, `.shp` 3D relief & turntable reconstruction, `.ini`, `.csf`, `.wav`, `.bik`), OpenRA converter (`MiniYAML`), validator (`validate`), and tech-tree inspector (`inspect`). |
| [`engine/crates/rts-simulation`](file:///d:/projekt/rust-rts/engine/crates/rts-simulation/src/lib.rs) | `rts-simulation` | Deterministic 20 Hz simulation core (`SimWorld`, 64-bit FNV-1a `state_hash()`): multi-domain A* pathfinding (land, naval, amphibious, air), combined-arms combat, veterancy, building garrisons, destructible bridges, espionage, superweapons, Chrono-Shift, engineer capture/repair, and adaptive skirmish AI. |
| [`engine/crates/rts-network`](file:///d:/projekt/rust-rts/engine/crates/rts-network/src/lib.rs) | `rts-network` | Deterministic TCP lockstep multiplayer (`LockstepServer` / `LockstepClient`), replay recording & headless verification (`.replay.json`), 3-tier NAT/UPnP port mapping (`nat_upnp.rs`), Reverse-TCP relay tunneling, and embedded lobby registry server. |
| [`engine/crates/rts-client`](file:///d:/projekt/rust-rts/engine/crates/rts-client/src/main.rs) | `rts-client` | Bevy 0.15 3D client & in-game Engine Studio (`[ESC]` / `--studio`): continuous 3D terrain with cliff/water shaders, `.glb` entity synchronization, 3D particle VFX, procedural & mod audio, authentic RA2 sidebar HUD with 8× neural-upscaled cameos, interactive radar minimap, 3D map editor, and online lobby browser. |
| [`rust-ra2/`](file:///d:/projekt/rust-rts/rust-ra2/mod.toml) | `rust-ra2` | Complete *Command & Conquer: Red Alert 2 & Yuri's Revenge* Total Conversion: **3 factions** (*Allies*, *Soviet*, *Yuri*), **48 units**, **42 buildings** (+ tech buildings & landmarks), **46 weapons**, **54 playable `.toml` maps** (+ 298 original `.map` files), **144 `.glb` 3D models**, **404 audio tracks/cues**, **138 4K/HD 30 FPS videos**, and **827 AI-remastered UI/VFX textures**. |
| [`mods/`](file:///d:/projekt/rust-rts/mods) | `openra2`, `westwood-classic`, `openra-classic` | Additional playable reference mods generated by the automated OpenRA and Westwood conversion pipelines. |
| [`centralwebserver/`](file:///d:/projekt/rust-rts/centralwebserver/README.md) | `rust-rts-centralwebserver` | Node.js/TypeScript web platform for **`rust-rts.bls-isp.net`** (also packaged for rootless **NethServer 8 / Podman** in [`Containerfile.ns8`](file:///d:/projekt/rust-rts/centralwebserver/Containerfile.ns8)): project portal, live lobby browser, match/replay registry (`/api/matches`), and Reverse-TCP NAT Relay Hub (ports `27900–27999`). |

---

## 3D Asset Reconstruction & 4K AI Remastering Pipeline (`rust-ra2`)

All proprietary Westwood assets from *Red Alert 2* (`ra2.mix`, `language.mix`, `MOVIES01/02.MIX`) and *Yuri's Revenge* (`ra2md.mix`, `langmd.mix`, `MOVMD03.MIX`) are converted into open, modern 3D and multimedia formats:

1. **3D Buildings, Superweapons & Landmarks (`71` `.glb` Models — AI Image-to-3D + Isometric Unprojection):**
   - **Tencent Hunyuan3D-2 (`hunyuan3d-dit-v2-0`, `octree_resolution=256`) + Blender 5.2 Ultra-Sharp Pipeline:** Reconstructs true watertight 360° 3D architectural meshes (`~175,000–210,000` vertices / `~350,000–420,000` triangles per model, `w/u = 88%–107%` depth-to-width ratio) with freestanding 3D sub-structures (construction cranes, hyperboloid cooling towers and curved pipes, geodesic/hemispherical domes, Tesla coil rings, flagpoles, and antennas).
   - **3D Mean-Curvature (`curv_norm`) Ridge/Cavity Enhancement & `1024×1024` Pure-Albedo Color Baking:** Combines sub-degree silhouette IoU camera alignment, `320×320` Z-buffer visibility projection, 4-pass harmonic 3D mesh-edge color wrapping for occluded surfaces, high-pass luminance normal micro-displacement, and 1-ring vertex-color unsharp masking (`COLOR_0` `VEC4` float).
2. **3D Vehicles, Aircraft, Ships & Missiles (`47` `.vxl` $\rightarrow$ `.glb` Models):**
   - **Multi-Octave (`4×` / `4×4×4 = 64×` Volumetric Sub-Voxel Density) Edge-Directed Voxel Super-Resolution ([`voxel_meshing.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/westwood/voxel_meshing.rs)):** Upscales 3D voxel grids in 24-bit RGB, computes volumetric `3×3×3` cavity ambient occlusion (`AO`), and applies 12-pass volume-preserving Taubin ($\lambda/\mu$) surface smoothing (`~66,000–704,000` vertices per vehicle).
3. **3D Infantry & `.shp` Special Units (`26` `.shp` $\rightarrow$ `.glb` Models):**
   - **4-Octave (`16×` Ultra-HD) xBRZ/Vector Super-Resolution + Multi-Frame Volumetric 3D Sculpting ([`sprite_relief.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/westwood/sprite_relief.rs)):** Generates upright 360° 3D infantry models (`~85,000–242,000` vertices) using orthogonal `90°` side-profile depth envelopes (`Frame 1` & `Frame 5`), 4-layer $C^1$ anatomical sculpting, and true `180°` back-surface projection from `Frame 7`.
4. **4K UHD (`2880×2160` @ `30 FPS`) FMV Cutscenes & HD Radar-PIP Videos (`138` `.mp4` Videos):**
   - Deinterlaces original `480i`/`600i` `@ 15 FPS` Bink (`.bik`) videos, synthesizes motion-compensated `30 FPS` progressive frames via bidirectional optical flow (`aobmc`), and upscales to `2880×2160` (`55` full-screen FMVs) and `1120×880` (`83` radar PIPs) via Vulkan `libplacebo` (`ewa_lanczossharp`) + custom 2-pass GLSL WarpSharp shaders and hardware `h264_amf` encoding.
5. **Neural-AI 2D UI, Cameos, Loading Screens & VFX (`827` PNGs) & Audio (`404` WAV/OGG Tracks):**
   - **51 Loading Screens** upscaled to `3200×2400` (`4×`), **207 Unit/Building Cameos** upscaled to `480×384` (`8×` 3-model ensemble), **36 Faction HUD Panels**, **517 Mouse Cursors**, and **16 Superweapon/Explosion VFX Flipbooks** remastered on GPU via **Waifu2x CUNet + Real-ESRGAN**.
   - **404 Audio Files** decoded from Westwood IMA ADPCM (`audio.bag` / `THEME.MIX`) into 16-bit PCM `.wav` (`192` EVA voiceovers, `100` unit/weapon SFX, `80` country taunts, `6` core cues) and stereo Ogg Vorbis `.ogg` (`26` Frank Klepacki music tracks).

---

## Developer CLI & Build Instructions

### 1. Build, Test & Run the 3D Engine
```powershell
# Compile the entire Rust workspace in release mode
cargo build --manifest-path engine/Cargo.toml --release

# Run all unit and integration tests across all 5 engine crates
cargo test --manifest-path engine/Cargo.toml --all-targets

# Launch the 3D Client in Game Mode (Red Alert 2 & Yuri's Revenge)
.\engine\target\release\rts-client.exe --mod-path rust-ra2

# Launch the 3D Client in Engine Studio Mode (Mod Manager, Map Editor & Converter)
.\engine\target\release\rts-client.exe --mod-path rust-ra2 --studio
```

### 2. Lockstep Multiplayer & Central Webserver (`rust-rts.bls-isp.net`)
```powershell
# Host a multiplayer match (registers on http://rust-rts.bls-isp.net or local fallback http://127.0.0.1:8080,
# configures UPnP port forwarding, and opens a Reverse-TCP Relay tunnel for CGNAT/DS-Lite)
.\engine\target\release\rts-client.exe --mod-path rust-ra2 --host 127.0.0.1:7878 --lobby-name "Operation Red Dawn"

# Connect a second player directly (or join via [ESC] -> Online Lobbys in-game)
.\engine\target\release\rts-client.exe --mod-path rust-ra2 --connect 127.0.0.1:7878

# Run the Central Node.js/TypeScript Webserver & Reverse-TCP Relay Hub
cd centralwebserver
npm install
npm test
PUBLIC_HOST=rust-rts.bls-isp.net RELAY_PORT_RANGE=27900-27999 PORT=8080 npm start
```

### 3. Asset Pipeline & Modding CLI (`rts-asset-pipeline`)
```powershell
# Validate a mod (manifest, TOML schemas, tech-tree cross-references, .glb models, .rhai & .lua scripts)
.\engine\target\release\rts-asset-pipeline.exe validate --mod-path rust-ra2

# Inspect faction tech-trees, unit cost/DPS balance, and building prerequisites
.\engine\target\release\rts-asset-pipeline.exe inspect --mod-path rust-ra2

# Generate or refresh procedural 3D .glb models for a mod
.\engine\target\release\rts-asset-pipeline.exe generate-models --mod-path rust-ra2
```

---

## Documentation & Technical Specifications

- [`CAPABILITY_MAP.md`](file:///d:/projekt/rust-rts/CAPABILITY_MAP.md) — Module capability map and dependency graph across all engine and server components
- [`SPEC.md`](file:///d:/projekt/rust-rts/SPEC.md) — Master technical specification of the engine, asset pipeline, and network architecture
- [`SPEC-rts-mod-sdk.md`](file:///d:/projekt/rust-rts/SPEC-rts-mod-sdk.md) — `rts-sdk` specification (TOML schemas, cross-reference validator, Rhai & Lua scripting)
- [`SPEC-rts-sim-core.md`](file:///d:/projekt/rust-rts/SPEC-rts-sim-core.md) — `rts-simulation` specification (20 Hz deterministic simulation, A* pathfinding, combat, superweapons, AI)
- [`SPEC-rts-mod-tools.md`](file:///d:/projekt/rust-rts/SPEC-rts-mod-tools.md) — `rts-asset-pipeline` specification (`.glb` generator, Westwood `.vxl`/`.shp`/`.mix` & OpenRA converter)
- [`SPEC-rts-net.md`](file:///d:/projekt/rust-rts/SPEC-rts-net.md) — `rts-network` specification (lockstep protocol, replay verification, 3-tier NAT/UPnP & Reverse-TCP Relay)
- [`SPEC-rts-client-3d.md`](file:///d:/projekt/rust-rts/SPEC-rts-client-3d.md) — `rts-client` specification (Bevy 3D renderer, audio, VFX, RA2 HUD & Engine Studio)
- [`SPEC-mod-rust-ra2.md`](file:///d:/projekt/rust-rts/SPEC-mod-rust-ra2.md) — `rust-ra2` Total Conversion specification (3 factions, 144 3D `.glb` models, 4K videos, neural UI/VFX remaster)
- [`docs/MOD_DEV_KIT.md`](file:///d:/projekt/rust-rts/docs/MOD_DEV_KIT.md) — Modder SDK & asset pipeline developer guide
- [`centralwebserver/README.md`](file:///d:/projekt/rust-rts/centralwebserver/README.md) — Central multiplayer webserver & NethServer 8 (`Containerfile.ns8`) deployment guide

---

## Non-Commercial Community & Asset Reconstruction Notice

> [!IMPORTANT]
> **Strictly Non-Commercial Project — Never for Sale or Monetization**
> **IRONLOCK // RUST-RTS** is an independent, strictly **non-commercial** open-source community, research, and preservation project. It is **never intended for commercial use, sale, paywalling, or monetization of any kind**.

- **100% Clean-Room Engine Implementation:** The entire Rust engine (`engine/`), lockstep networking stack, scripting runtime, and central multiplayer server (`centralwebserver/`) are written from scratch without using any proprietary code.
- **Transformative Re-Engineering & Preservation of Lost Classics:** Because the original Westwood Studios C++ source code and 3D Studio Max (`.max`) production assets for *Red Alert 2* and *Yuri's Revenge* were historically lost following the studio's closure in 2003, all visual and multimedia assets in `rust-ra2/` have undergone extensive, transformative technical reconstruction:
  - **3D Models (`.glb`):** Synthesized into full 360° high-poly 3D meshes (`~66,000–704,000` vertices) from low-resolution legacy 2D pixels and voxel slices via neural Image-to-3D generation (**Tencent Hunyuan3D-2**), Blender 3D mean-curvature sculpting, and `64×` volumetric super-resolution.
  - **Graphics & UI (`.png`):** Neural-reconstructed up to `4×–16×` resolution via multi-model ensembles (**Waifu2x CUNet**, **Real-ESRGAN**, and **xBRZ** vectorization).
  - **Videos (`.mp4`) & Audio (`.wav` / `.ogg`):** Restored from legacy interlaced `15 FPS` streams into motion-interpolated `30 FPS` 4K UHD (`2880×2160`) video via optical-flow synthesis and GLSL shader pipelines, alongside modern PCM/Vorbis audio decoding.
- **Trademarks:** *Command & Conquer*, *Red Alert 2*, and *Yuri's Revenge* are trademarks of Electronic Arts Inc. This project is a community-driven preservation and technical showcase effort and is not affiliated with or endorsed by Electronic Arts.
