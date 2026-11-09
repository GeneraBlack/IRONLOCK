# Spec: `rts-simulation` (`engine/crates/rts-simulation`)

## Objective
Deterministischer 20-Hz Fixed-Timestep-RTS-Simulationskern ([`engine/crates/rts-simulation`](file:///d:/projekt/rust-rts/engine/crates/rts-simulation/src/lib.rs)). Verarbeitet serialisierbare `GameCommand`-Ereignisse, berechnet Bewegung/Wegfindung über verschiedene Bewegungsdomänen (Land, Wasser, Amphibisch, Luft, Hover), kombinierten Kampf, Strom-/Ressourcenwirtschaft, Basisbau, Einheitenproduktion, Superwaffen, Garnisonen, Spionage sowie die adaptive Skirmish-KI und triggert Mod-Skript-Hooks über `rts-sdk`.

## Tech Stack
- `rts-sdk`, `serde`, `serde_json`, `glam`, `thiserror`, `anyhow`

## Commands
```powershell
cargo test --manifest-path engine/Cargo.toml -p rts-simulation
```

## Project Structure
- [`engine/crates/rts-simulation/src/lib.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-simulation/src/lib.rs)
- [`engine/crates/rts-simulation/src/command.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-simulation/src/command.rs) — Serialisierbare `GameCommand`-Typen (Move, AttackMove, Patrol, Guard, ReverseMove, PlaceBuilding, ProduceUnit, ActivateAbility, FireSupportPower, ChronoShift, EnterTransport, Garrison, etc.)
- [`engine/crates/rts-simulation/src/pathfinding.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-simulation/src/pathfinding.rs) — A*-Wegfindung unter Berücksichtigung von Höhenstufen, Rampen, Brücken, Wasser und `LocomotionType`
- [`engine/crates/rts-simulation/src/world/`](file:///d:/projekt/rust-rts/engine/crates/rts-simulation/src/world/mod.rs) — `SimWorld` Zustandsverwaltung, 64-Bit FNV-1a `state_hash()`, Command-Handler (`commands_abilities.rs`, `commands_construction.rs`, `commands_tactical.rs`, `commands_unit_orders.rs`), Bewegung (`movement.rs`), Trigger & Fog of War (`triggers_and_fow.rs`)
- [`engine/crates/rts-simulation/src/combat/`](file:///d:/projekt/rust-rts/engine/crates/rts-simulation/src/combat/mod.rs) — Zielerfassung (`targeting.rs`), Schadensberechnung, Rüstungstypen, Veteranen-Beförderung (`Rookie`, `Veteran`, `Elite`) und Mind-Control (`resolution.rs`)
- [`engine/crates/rts-simulation/src/production/`](file:///d:/projekt/rust-rts/engine/crates/rts-simulation/src/production/mod.rs) — Bauschleifen, Erz-Raffinerie-Zyklus, Tech-Gebäude (Ölbohrturm, Flughafen, Reparaturdepot) & Superwaffen-Timer (`support_structures.rs`)
- [`engine/crates/rts-simulation/src/ai/`](file:///d:/projekt/rust-rts/engine/crates/rts-simulation/src/ai/mod.rs) — Deterministische Skirmish-KI mit Schwierigkeitsgraden (`Easy`, `Medium`, `Hard`, `Brutal`) und Persönlichkeiten (`Balanced`, `Rush`, `Turtle`, `Tech`)

## Testing Strategy
- Umfassende Testsuite in [`engine/crates/rts-simulation/src/tests/`](file:///d:/projekt/rust-rts/engine/crates/rts-simulation/src/tests/mod.rs) (`core_mechanics.rs`, `combined_arms.rs`, `generals_and_stealth.rs`, `tactical_and_world.rs`).
- 200-Tick-Determinismus-Beweis zweier unabhängiger `SimWorld`-Instanzen mit bit-identischem `state_hash()`.

## Boundaries
- **Always:** Deterministische Datenstrukturen (`BTreeMap`/`BTreeSet`) und sortierte Iterationen für 100 % Lockstep-Multiplayer-Synchronität.
- **Never:** Nicht-deterministische `HashMap`-Iterationen oder Systemzeit-Abfragen in `SimWorld::step()`.
