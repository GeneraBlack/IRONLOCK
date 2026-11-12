# Standalone Game Cooking & Publishing Guide (`STANDALONE_PUBLISHING.md`)

This guide explains how to package any mod built on the **Rust 3D RTS Engine** into a self-contained, zero-extraction standalone desktop game distribution ready for players on Windows, Linux, and macOS.

---

## 1. Architecture: Zero-Extraction `RTSMOD03` VFS & `ArchiveAssetReader`

Standalone distributions never unpack loose `.toml`, `.glb`, `.png`, `.ogg`, or `.mp4` files into temporary folders at launch:

1. **Zlib-Compressed `RTSMOD03` Archive (`rts_sdk::archive::ModArchive`):**
   - Packs all validated mod manifests, catalogs, maps, campaigns, localization tables, scripts (`.rhai` / `.lua`), 3D glTF models, textures, audio, and videos into a single deterministic binary archive (`mods/<mod_id>.rtsmod`) with per-entry Zlib compression and a verified 64-bit FNV-1a `content_hash`.
   - Automatically excludes authoring scratch directories such as `assets/raw/`.
2. **In-Memory SDK Catalog Loader (`LoadedMod::load_from_path`):**
   - Reads `mod.toml`, catalogs, maps, campaigns, locales, and scripts directly from the `.rtsmod` archive in memory and exposes `LoadedMod::archive_source: Option<Arc<ModArchive>>`.
3. **Custom Bevy 0.15 `ArchiveAssetReader` (`rts-client/src/archive_asset_reader.rs`):**
   - Registered on `AssetSourceId::Default` **before** `DefaultPlugins` initializes whenever `LoadedMod.archive_source` is present.
   - Streams `.glb` scenes, PBR textures, cameo icons, custom fonts, `.wav`/`.ogg`/`.flac` audio, and shaders directly from the in-memory `ModArchive` without touching disk (with transparent disk fallback for developer overrides).

---

## 2. Option A: Export from the `IRONLOCK Creator Workbench` IDE

1. Launch the client in Studio mode:
   ```powershell
   cargo run --manifest-path engine/Cargo.toml -p rts-client -- --mod-path rust-ra2 --studio
   ```
2. Open **`File -> Export Standalone Game...`** from the top Workbench menu bar.
3. Configure the export modal (`studio/export_wizard.rs`):
   - **Game Window Title:** Custom title displayed in the OS window bar (e.g., `"IRONLOCK: Red Alert Remastered"`).
   - **Output Directory:** Target folder for the cooked distribution (e.g., `dist/ironlock-standalone`).
   - **Lock Standalone Player Mode (`lock_player_mode`):** Recommended `true` for public player builds — disables `--studio` and `[F12]` Creator Workbench access so end users experience a pure retail RTS client.
4. Click **`🚀 Cook & Export Standalone Game`**. A background worker validates the mod (`validate_mod`), packs `mods/<mod_id>.rtsmod`, writes `game_bundle.toml` and platform launch scripts, copies compiled `rts-client` / `rts-dedicated-server` binaries if present, and prints the compression ratio and `content_hash` in the live progress log.

---

## 3. Option B: Export via Headless CLI (`rts-asset-pipeline cook-standalone`)

For CI/CD release pipelines or automated builds, build the release binaries first and then run `cook-standalone`:

```powershell
# 1. Build optimized release binaries for client and dedicated server
cargo build --release --manifest-path engine/Cargo.toml -p rts-client -p rts-dedicated-server

# 2. Cook the standalone distribution folder
cargo run --release --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- `
  cook-standalone `
  --mod-path rust-ra2 `
  --output-dir dist/rust-ra2-standalone `
  --title "IRONLOCK: Red Alert Total Conversion" `
  --lock-player-mode true
```

### CLI Options (`cook-standalone`)

| Flag | Default | Description |
| :--- | :--- | :--- |
| `--mod-path <PATH>` | `rust-ra2` | Path to the source mod directory (or existing `.rtsmod` archive). Must pass `validate_mod` with `0` errors. |
| `-o, --output-dir <DIR>` | *(required)* | Target output directory where the standalone distribution will be generated. |
| `--title <STRING>` | `manifest.name` | Custom window title written to `game_bundle.toml` (`window_title`). |
| `--lock-player-mode [BOOL]` | `true` | Writes `lock_player_mode = true` in `game_bundle.toml` and appends `--player-mode` to `Play.bat` / `play.sh`. |

---

## 4. Generated Distribution Layout & `game_bundle.toml`

Running `cook-standalone` produces a clean, self-contained folder:

```text
dist/rust-ra2-standalone/
├── game_bundle.toml              # Auto-detected standalone bundle configuration
├── rts-client.exe                # (Copied automatically when built in release/debug)
├── rts-dedicated-server.exe      # (Copied automatically when built in release/debug)
├── Play.bat                      # Double-click launcher for Windows
├── play.sh                       # Executable POSIX launcher for Linux / macOS
├── README_STANDALONE.md          # Generated quick-start & content_hash manifest
└── mods/
    └── rust-ra2.rtsmod           # Zlib-compressed RTSMOD03 VFS archive
```

### `game_bundle.toml` Schema (`StandaloneBundleConfig`)

```toml
window_title = "IRONLOCK: Red Alert Total Conversion"
default_mod_path = "mods/rust-ra2.rtsmod"
lock_player_mode = true
version = "0.2.0"
```

When `rts-client` starts, `resolve_bundle_launch_config` inspects the working directory and the executable's own directory for `game_bundle.toml`:
- If `--mod-path` was not explicitly passed on the CLI, `default_mod_path` is mounted automatically via `ArchiveAssetReader`.
- `window_title` sets the primary OS window title.
- When `lock_player_mode = true` (or `--player-mode` is passed on the CLI), `--studio`, `[F12]`, and the Main Menu **Creator Workbench** button are disabled so the game runs strictly in Player Mode.

---

## 5. Pre-Release Verification Checklist

Before distributing your cooked build to players:

1. **Zero Validation Errors & Warnings:**
   ```powershell
   cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- validate --mod-path rust-ra2
   ```
2. **Declarative Mod Scenario Tests Pass:**
   ```powershell
   cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- test-mod --mod-path rust-ra2
   ```
3. **Direct Zero-Extraction `.rtsmod` Boot Verification:**
   ```powershell
   cargo run --manifest-path engine/Cargo.toml -p rts-client -- --mod-path dist/rust-ra2-standalone/mods/rust-ra2.rtsmod --player-mode
   ```
4. **Dedicated Multiplayer Server Compatibility:**
   - Host or run `rts-dedicated-server --mod-path mods/<mod_id>.rtsmod` — the `content_hash` inside `.rtsmod` is identical to the unpacked source mod, guaranteeing seamless multiplayer handshake verification, live `SpectatorClient` delayed broadcasts (`0s..60s`), and deterministic `.rtssav` Multiplayer Save/Resume.

---

## 6. Online Mod & Map Workshop Publishing (`Phases 24`)

In addition to standalone desktop distributions, creators can publish `.rtsmod` archives directly to the **Online Mod & Map Workshop** hosted by `centralwebserver`:

1. **1-Click Studio Publishing (`studio/export_wizard.rs`):**
   - Open **`File -> Export Standalone Game...`** inside the Creator Workbench and click **`☁ Publish .rtsmod to Online Workshop`**.
   - A background worker runs `validate_mod` (including `validate_binary_asset_header` deep asset checks), packs `ModArchive::pack_from_dir`, computes the SHA-256 checksum, and uploads the signed payload to `POST /api/v1/workshop/publish`.
2. **In-Game Workshop Browser & Automatic Lobby Sync (`ui/screens/workshop_screen.rs`):**
   - Players browse, search, and install community mods & map packs in 1 click via Main Menu -> **`MOD WORKSHOP`** (`GET /api/v1/workshop/items` + `GET /api/v1/workshop/download/:id` -> `mods/<id>.rtsmod` verified against SHA-256).
   - Joining a Multiplayer Lobby room whose `mod_id` / `mod_content_hash` is not installed locally automatically prompts to download and verify the matching `.rtsmod` from the Workshop before entering the Staging Room.

---

## 7. Automated Cross-OS GitHub Actions Release Cooker (`.github/workflows/`)

- **`.github/workflows/ci.yml`:** Runs on every push and PR across `ubuntu-latest`, `windows-latest`, and `macos-latest`, enforcing `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, workspace & `--release` determinism tests, `rts-asset-pipeline validate` + `test-mod` on `rust-ra2` and `mods/sdk-showcase-workers-squads`, and `centralwebserver` TypeScript tests.
- **`.github/workflows/release.yml`:** Triggered automatically on `v*` tags to optimize `.glb` `LOD0`/`LOD1` meshes (`optimize-models`), build `--release` `rts-client` and `rts-dedicated-server` binaries on Linux, Windows, and macOS, execute `cook-standalone`, and attach ready-to-play `.zip` / `.tar.gz` bundles directly to the GitHub Release.

---

## 8. Embedding `rts_client` as a Modular Bevy Library & Multi-Layer VFS (`Phases 25–28`)

Standalone games can either ship the data-driven `rts-client` binary with `game_bundle.toml` OR embed `rts_client::RtsEngineClientPlugins` directly inside a custom Rust binary crate (see [`docs/ENGINE_MODULAR_ARCHITECTURE.md`](ENGINE_MODULAR_ARCHITECTURE.md)):

1. **Mount Multi-Layer `.rtsmod` / Overlay VFS Before `DefaultPlugins`:**
   Call `rts_client::register_mod_vfs_asset_source(&mut app, &loaded_mod, workspace_root)` so Bevy resolves `.glb`, `.ktx2`, `.png`, `.ogg`, and `.wgsl` assets top-down across `LoadedMod.vfs_layers` (`base.rtsmod` + mutator/expansion `.rtsmod` overlays) with zero disk extraction.
2. **Cook Mipmapped `.ktx2` Textures & Cameo Sprite Atlases (`Task 245`):**
   ```powershell
   cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- cook-textures --mod-path mods/sdk-showcase-heroes-ages-mutators
   ```
   Generates GPU-ready RGBA8 mipmapped `.ktx2` textures plus `assets/icons/cameo_atlas.{png,toml}` automatically integrated during `cook-standalone`.
3. **Authoritative Dedicated Server Fleet, Hybrid Rollback & Crash-Reconnect (`Tasks 247–250`):**
   - `rts-dedicated-server` supports `--mod-stack`, `--lobby-registry-url` fleet registration/heartbeats (`/api/v1/servers/{register,heartbeat,allocate}`), signed Ed25519 Glicko-2 match reporting, `RollbackSchedule` 1-tick/0-latency hybrid rollback, `saves/reconnect_session.json` crash-rejoin (`ReclaimHumanSlot`), and automated `desync_tick_<N>.{diff,replay}.json` forensics bundles.
   - Multi-stage container deployment is ready out of the box via `centralwebserver/Dockerfile` and rootless Podman/NethServer 8 `centralwebserver/Containerfile.ns8`.
