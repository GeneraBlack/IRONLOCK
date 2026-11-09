# Spec: `rts-client` (`engine/crates/rts-client`)

## Objective
Bevy 0.15 basierter 3D-RTS-Client und integriertes Grafisches Engine Studio ([`engine/crates/rts-client`](file:///d:/projekt/rust-rts/engine/crates/rts-client/src/main.rs)). Lädt eine beliebige Mod (standardmäßig `rust-ra2`), führt `SimWorld` im 20-Hz-Fixed-Timestep (Singleplayer mit Skirmish-KI, Lockstep-Multiplayer oder 3D-Replay-Wiedergabe) aus, rendert das kontinuierliche 3D-Terrain sowie alle `.glb`-Einheiten und -Gebäude mit animierten Türmen/Rotoren, 3D-VFX-Partikeln und Audio und stellt das authentische RA2-Sidebar-HUD samt KI-upscaled Cameo-Icons und Online-Lobby-Browser bereit.

## Tech Stack
- `bevy` `0.15` (DX12/Vulkan PBR, `bevy_gltf`, `bevy_ui`, `bevy_gizmos`), `rts-sdk`, `rts-asset-pipeline`, `rts-simulation`, `rts-network`, `clap`

## Commands
```powershell
# 3D-Client starten (Standard-Mod: rust-ra2)
cargo run --manifest-path engine/Cargo.toml -p rts-client -- --mod-path rust-ra2

# Direkt mit geöffnetem Engine Studio & Online-Lobby-Browser starten
cargo run --manifest-path engine/Cargo.toml -p rts-client -- --mod-path rust-ra2 --studio

# Als Online-Multiplayer-Host starten (registriert Lobby auf rust-rts.bls-isp.net bzw. lokalem Fallback)
cargo run --manifest-path engine/Cargo.toml -p rts-client -- --mod-path rust-ra2 --host 127.0.0.1:7878
```

## Project Structure
- [`engine/crates/rts-client/src/main.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-client/src/main.rs) — App-Setup, CLI-Parameter, FixedUpdate-Lockstep-/KI-Loop & Lobby-Auto-Resolver (`resolve_and_ensure_lobby_webserver`)
- [`engine/crates/rts-client/src/camera.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-client/src/camera.rs) — `RtsCameraRig` (WASD/Edge-Pan, Zoom, Rotation, Kamera-Bookmarks `Ctrl+F1..F4`)
- [`engine/crates/rts-client/src/audio.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-client/src/audio.rs) — Prozeduraler WAV-Synthesizer + Loader für Mod-Audio-Overrides (`rust-ra2/assets/audio/`)
- [`engine/crates/rts-client/src/input/`](file:///d:/projekt/rust-rts/engine/crates/rts-client/src/input/mod.rs) — Modulare RTS-Eingabesteuerung (`keyboard.rs`, `hotkeys.rs`, `mouse_left.rs`, `mouse_right.rs`), `ClientSettings` (`saves/settings.toml`) & `ClientInteractionState`
- [`engine/crates/rts-client/src/rendering/`](file:///d:/projekt/rust-rts/engine/crates/rts-client/src/rendering/mod.rs) — Kontinuierliches 3D-Vertex-Terrain (`mod.rs`), `.glb`-Entity-Synchronisation & Turm-/Rotor-Animation (`entity_sync.rs`), Auswahlringe, Gesundheitsbalken & Bau-Vorschau (`gizmos.rs`, `gizmos_world.rs`)
- [`engine/crates/rts-client/src/ui/`](file:///d:/projekt/rust-rts/engine/crates/rts-client/src/ui/mod.rs) — RA2-Sidebar mit KI-remasterten Cameo-Icons (`sidebar.rs`), Top-Ressourcen-/Stromleiste (`hud_update.rs`), interaktive Radar-Minimap (`minimap.rs`), Einheiten-Inspektor (`inspector.rs`) & In-Game Engine Studio / Online-Lobby-Browser (`studio_layout.rs`, `actions_studio.rs`)
- [`engine/crates/rts-client/src/vfx/`](file:///d:/projekt/rust-rts/engine/crates/rts-client/src/vfx/mod.rs) — 3D-Projektile, gezackte Tesla-/Prisma-Strahlen, Druckwellen, Partikel-Explosionen, Nuklearpilz & Wettersturm-VFX (`systems.rs`)
