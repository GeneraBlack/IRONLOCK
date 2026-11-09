# Task Breakdown: Rust 3D RTS Engine & `rust-ra2`

## Task 1: Workspace-Setup & `rts-mod-sdk` Daten-Schemata

**Description:** Anlegen des Cargo-Workspaces in `engine/Cargo.toml` sowie Implementierung des Mod-Manifests (`mod.toml`), der Katalog-Schemata (Fraktionen, Waffen, GebÃ¤ude, Einheiten, 3D-Visual-Archetypen, Karten) und des Verzeichnis-Loaders in `rts-mod-sdk`.

**Acceptance criteria:**
- [x] `engine/Cargo.toml` definiert den Workspace fÃ¼r die Engine-Crates.
- [x] `rts-mod-sdk` deserialisiert `mod.toml`, `factions`, `weapons`, `buildings`, `units` und `maps` vollstÃ¤ndig.
- [x] Unit-Tests prÃ¼fen das Laden und ZusammenfÃ¼hren von Mod-Katalogen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-mod-sdk`
- [x] Build succeeds: `cargo build --manifest-path engine/Cargo.toml -p rts-mod-sdk`

**Dependencies:** None

**Files likely touched:**
- `engine/Cargo.toml`
- `engine/crates/rts-mod-sdk/Cargo.toml`
- `engine/crates/rts-mod-sdk/src/lib.rs`
- `engine/crates/rts-mod-sdk/src/manifest.rs`
- `engine/crates/rts-mod-sdk/src/schema.rs`

**Estimated scope:** Medium (5 files)

---

## Task 2: Duale Skript-Engine (`.rhai` & `.lua`) & Mod-Validator in `rts-mod-sdk`

**Description:** Erweiterung von `rts-mod-sdk` um den Mod-Verzeichnis-Loader, den Cross-Reference-Validator (Waffen, Tech-Tree-Voraussetzungen, ZyklenprÃ¼fung, Skript-Hooks) sowie den dualen `ScriptHost`, der sowohl `.rhai`-Skripte als auch `.lua`-Skripte lÃ¤dt und Hooks (`on_spawn`, `on_ability`, `on_tick`) auf einem `ScriptContext` ausfÃ¼hrt.

**Acceptance criteria:**
- [x] Sowohl `.rhai`- als auch `.lua`-Skripte kÃ¶nnen geladen werden und verÃ¤ndern deterministisch Werte im `ScriptContext` (z. B. `speed_multiplier`, `armor_multiplier`, `damage_multiplier`, `custom_flags`).
- [x] Der Validator erkennt fehlende Waffen-IDs, unbekannte GebÃ¤ude-Voraussetzungen, Zyklen im Tech-Tree und fehlende Skript-Dateien.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-mod-sdk`
- [x] Build succeeds: `cargo clippy --manifest-path engine/Cargo.toml -p rts-mod-sdk -- -D warnings`

**Dependencies:** Task 1

**Files likely touched:**
- `engine/crates/rts-mod-sdk/Cargo.toml`
- `engine/crates/rts-mod-sdk/src/loader.rs`
- `engine/crates/rts-mod-sdk/src/validator.rs`
- `engine/crates/rts-mod-sdk/src/scripting.rs`
- `engine/crates/rts-mod-sdk/src/lib.rs`

**Estimated scope:** Medium (5 files)

---

## Checkpoint: After Tasks 1-2 (Mod-SDK Foundation)
- [x] Alle Tests in `rts-mod-sdk` bestehen (`cargo test --manifest-path engine/Cargo.toml -p rts-mod-sdk`)
- [x] Duales Scripting (`.rhai` + `.lua`) und Schema-Validierung funktionieren

---

## Task 3: CLI-Tool `rts-mod-tools` inkl. 3D-`.glb`-Generator

**Description:** Implementierung des Mod-Entwickler-Hilfsprogramms `rts-mod-tools` mit den Unterbefehlen `generate-models` (erzeugt valide Binary-glTF-2.0-`.glb`-Modelle fÃ¼r Panzer, Infanterie, Sammler, Kraftwerke, Fabriken, Teslaspulen usw.), `validate` (prÃ¼ft eine Mod inkl. `.glb`-Existenz und Skript-Syntax), `inspect` (gibt Fraktions-, Tech-Tree- und DPS-Tabellen aus) und `scaffold-unit` (erzeugt Vorlagen fÃ¼r neue Einheiten).

**Acceptance criteria:**
- [x] `generate-models` schreibt standardkonforme `.glb`-Dateien (Magic `0x46546C67`, Version 2, JSON-Chunk + BIN-Vertex/Index-Buffer).
- [x] `validate` und `inspect` liefern prÃ¤zise Berichte fÃ¼r einen angegebenen `--mod-path`.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-mod-tools`
- [x] Build succeeds: `cargo build --manifest-path engine/Cargo.toml -p rts-mod-tools`

**Dependencies:** Task 2

**Files likely touched:**
- `engine/crates/rts-mod-tools/Cargo.toml`
- `engine/crates/rts-mod-tools/src/main.rs`
- `engine/crates/rts-mod-tools/src/glb_gen.rs`
- `engine/crates/rts-mod-tools/src/inspector.rs`
- `engine/crates/rts-mod-tools/src/scaffold.rs`

**Estimated scope:** Medium (5 files)

---

## Task 4: Aufbau der `rust-ra2` Total Conversion Mod & 3D-Asset-Generierung

**Description:** Erstellung der kompletten *Command & Conquer: Red Alert 2* Beispiel-Mod im Ordner `rust-ra2/` mit Alliierten und Sowjets, Waffen, GebÃ¤uden (Bauhof, Kraftwerk/Tesla-Reaktor, Kaserne, Erzraffinerie, Waffenfabrik, Teslaspule), Einheiten (GI, Rekrut, Chronominer, Kriegssammler, Grizzly-Panzer, Rhino-Panzer), einer 3D-Karte (`skirmish_valley.toml`), einem Rhai-Skript (`allied_rules.rhai` fÃ¼r GI Deploy & Grizzly Maneuver) und einem Lua-Skript (`soviet_rules.lua` fÃ¼r Rhino Overcharge), gefolgt vom Generieren der `.glb`-Modelle per `rts-mod-tools`.

**Acceptance criteria:**
- [x] `rts-mod-tools generate-models --mod-path rust-ra2` erzeugt alle `.glb`-3D-Modelle unter `rust-ra2/assets/models/`.
- [x] `rts-mod-tools validate --mod-path rust-ra2` meldet `0 errors`.
- [x] `rts-mod-tools inspect --mod-path rust-ra2` zeigt alle alliierten und sowjetischen Einheiten, GebÃ¤ude und Tech-AbhÃ¤ngigkeiten an.

**Verification:**
- [x] Manual/CLI check: `cargo run --manifest-path engine/Cargo.toml -p rts-mod-tools -- validate --mod-path rust-ra2`
- [x] Manual/CLI check: `cargo run --manifest-path engine/Cargo.toml -p rts-mod-tools -- inspect --mod-path rust-ra2`

**Dependencies:** Task 3

**Files likely touched:**
- `rust-ra2/mod.toml`
- `rust-ra2/factions/factions.toml`
- `rust-ra2/weapons/weapons.toml`
- `rust-ra2/buildings/buildings.toml`
- `rust-ra2/units/units.toml`
- `rust-ra2/scripts/allied_rules.rhai`
- `rust-ra2/scripts/soviet_rules.lua`
- `rust-ra2/maps/skirmish_valley.toml`

**Estimated scope:** Medium (Mod-Konfigurationsdateien)

---

## Task 5: `rts-sim-core` GrundgerÃ¼st, `GameCommand`-Pipeline & A*-Wegfindung

**Description:** Implementierung des deterministischen RTS-Simulationskerns (`rts-sim-core`) mit `SimWorld`, Karten-/Kollisionsgitter, A*-Wegfindung, Initialisierung einer Karte aus einer geladenen Mod (`LoadedMod`) und AusfÃ¼hrung von Bewegungsbefehlen (`GameCommand::MoveUnits`).

**Acceptance criteria:**
- [x] `SimWorld::from_mod()` spawnt StartgebÃ¤ude, Starteinheiten und Erzfelder gemÃ¤ÃŸ der geladenen Karte.
- [x] `GameCommand::MoveUnits` berechnet einen kollisionsfreien A*-Pfad und bewegt Einheiten deterministisch pro Tick zum Ziel.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sim-core`
- [x] Build succeeds: `cargo build --manifest-path engine/Cargo.toml -p rts-sim-core`

**Dependencies:** Task 2, Task 4

**Files likely touched:**
- `engine/crates/rts-sim-core/Cargo.toml`
- `engine/crates/rts-sim-core/src/lib.rs`
- `engine/crates/rts-sim-core/src/command.rs`
- `engine/crates/rts-sim-core/src/world.rs`
- `engine/crates/rts-sim-core/src/pathfinding.rs`

**Estimated scope:** Medium (5 files)

---

## Task 6: Kampf, Basisbau, Erz-Wirtschaft, Strombilanz & Skript-FÃ¤higkeiten in `rts-sim-core`

**Description:** VervollstÃ¤ndigung von `rts-sim-core` um Zielerfassung und Waffenschaden (`combat.rs`), GebÃ¤udeplatzierung, Tech-Tree-PrÃ¼fung, Einheiten-Bauschleifen, Strombilanz (Low-Power-Verlangsamung wie in RA2), automatischen Sammler-Erntezyklus (`production.rs`) sowie das AusfÃ¼hren von Rhai-/Lua-SpezialfÃ¤higkeiten (`GameCommand::ActivateAbility`) inkl. deterministischem `state_hash()`.

**Acceptance criteria:**
- [x] Einheiten bekÃ¤mpfen Ziele in Reichweite bzw. auf `AttackTarget`-Befehl unter BerÃ¼cksichtigung von Waffen-Cooldown und RÃ¼stungstyp.
- [x] GebÃ¤ude kÃ¶nnen gebaut werden (`PlaceBuilding`), Ã¤ndern die Strombilanz und schalten neue Einheiten fÃ¼r `ProduceUnit` frei; Sammler bauen Erz ab und liefern Credits in der Raffinerie ab.
- [x] `ActivateAbility` ruft den zugehÃ¶rigen `.rhai`- oder `.lua`-Hook auf (z. B. GI Deploy oder Sowjet Overcharge).
- [x] Ein Determinismus-Test Ã¼ber 200 Ticks mit zwei `SimWorld`-Instanzen auf der `rust-ra2`-Mod beweist identische `state_hash()`-Werte.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sim-core`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml -p rts-sim-core -- -D warnings`

**Dependencies:** Task 5

**Files likely touched:**
- `engine/crates/rts-sim-core/src/combat.rs`
- `engine/crates/rts-sim-core/src/production.rs`
- `engine/crates/rts-sim-core/src/world.rs`
- `engine/crates/rts-sim-core/src/lib.rs`

**Estimated scope:** Medium (4 files)

---

## Task 7: Bevy 3D-Client GrundgerÃ¼st, 3D-RTS-Kamera & `.glb`-Szenen-Synchronisation

**Description:** Aufbau des `rts-client-3d`-Crates mit Bevy 0.15, Laden einer beliebigen Mod per `--mod-path` (Default: `rust-ra2`), 3D-Terrain- & Erzfeld-Darstellung, RTS-Kamerasteuerung (WASD/Pfeiltasten, Mausrad-Zoom, Q/E-Rotation) sowie Synchronisation der `SimWorld`-EntitÃ¤ten mit Bevy-3D-Szenen (Laden der `.glb`-Modelle aus der Mod inkl. Auswahl- und Gesundheitsbalken-Gizmos).

**Acceptance criteria:**
- [x] `rts-client-3d` kompiliert und lÃ¤dt beim Start `rust-ra2` sowie die generierten `.glb`-Modelle.
- [x] Die 3D-RTS-Kamera lÃ¤sst sich flÃ¼ssig Ã¼ber das Schlachtfeld bewegen, zoomen und drehen.
- [x] Alle EntitÃ¤ten aus `SimWorld` (Bauhof, Panzer, Infanterie, Sammler, Erzfelder) werden in 3D dargestellt und ihre Positionen/Rotationen pro Tick synchronisiert.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-client-3d`
- [x] Build succeeds: `cargo build --manifest-path engine/Cargo.toml -p rts-client-3d`

**Dependencies:** Task 6

**Files likely touched:**
- `engine/crates/rts-client-3d/Cargo.toml`
- `engine/crates/rts-client-3d/src/main.rs`
- `engine/crates/rts-client-3d/src/camera.rs`
- `engine/crates/rts-client-3d/src/render_sync.rs`

**Estimated scope:** Medium (4 files)

---

## Task 8: RTS-Input (Auswahlrahmen, Kommandos, Bau-Modus) & RA2-Sidebar-HUD

**Description:** Implementierung der kompletten Spieler-Interaktion in `rts-client-3d`: 2D-Drag-Selection-Box und Linksklick-Auswahl, Rechtsklick-Raycasting auf den 3D-Boden/Gegner (sendet `MoveUnits` bzw. `AttackTarget`), Hotkey `G` / `Space` fÃ¼r SpezialfÃ¤higkeiten (`ActivateAbility` -> triggert Rhai/Lua-Skript), sowie eine dynamisch aus dem Mod-Katalog generierte RA2-Sidebar (Credits, Stromanzeige, GebÃ¤ude-Bau-Buttons mit 3D-Platzierungs-Vorschau und Einheiten-Produktions-Buttons).

**Acceptance criteria:**
- [x] Einheiten kÃ¶nnen per Klick oder Auswahlrahmen selektiert und per Rechtsklick bewegt oder in den Kampf geschickt werden.
- [x] Die Sidebar zeigt Credits, Strombilanz sowie alle freigeschalteten GebÃ¤ude und Einheiten der Spieler-Fraktion aus `rust-ra2` dynamisch an.
- [x] GebÃ¤ude kÃ¶nnen aus der Sidebar im 3D-Raum platziert, Einheiten produziert und SpezialfÃ¤higkeiten (Rhai/Lua) ausgelÃ¶st werden.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 72: `rts-sdk` & `rust-ra2` â€” Schemata fÃ¼r Klon-StÃ¤tte (`cloning_vats`), Tech-EinnahmeprÃ¤mie (`capture_cash_bonus`), Ãœberroll-Schutz/Mauern (`uncrushable`, `crushable`), Garnisons-RÃ¤umung (`clears_garrison`) & Force-Shield (`SupportPowerKind::ForceShield`) (`combat_defs.rs`, `entities.rs`, `rust-ra2/`)

**Description:** Erweiterung des Mod-SDKs und des Referenz-Mods `rust-ra2` fÃ¼r Meilenstein 18:
1) **`WeaponDef` & `SupportPowerKind` (`combat_defs.rs`)**:
   - `clears_garrison: bool` (`#[serde(default)]`) auf `WeaponDef` fÃ¼r Flammen-/Strahlungs-/Toxinwaffen, die verschanzte Infanterie direkt in besetzten GebÃ¤uden ausschalten.
   - `SupportPowerKind::ForceShield` fÃ¼r temporÃ¤re GebÃ¤ude-Unverwundbarkeit.
2) **`BuildingDef` & `UnitDef` (`entities.rs`)**:
   - `cloning_vats: bool`, `capture_cash_bonus: u32`, `crushable: bool` (`#[serde(default)]`) auf `BuildingDef`.
   - `uncrushable: bool` (`#[serde(default)]`) auf `UnitDef` (fÃ¼r schwere Infanterie wie Tesla-Trooper).
3) **Referenz-Mod `rust-ra2` (`buildings.toml`, `units.toml`, `weapons.toml`)**:
   - `soviet_cloning_vats` (`cloning_vats = true`), `tech_oil_derrick` (`capture_cash_bonus = 1000`), `allied_force_shield` (`SupportPowerKind::ForceShield`), `rad_beam` (`clears_garrison = true`), `tesla_trooper` (`uncrushable = true`).

**Acceptance criteria:**
- [ ] `rts-sdk` kompiliert fehlerfrei, validiert `rust-ra2` ohne Fehler und besteht alle Unit-Tests.

**Verification:**
- [ ] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 73: `rts-simulation` â€” Klon-StÃ¤tte & Infanterie-Recycling, Tech-Einnahme-Sofortbonus, Garnisons-RÃ¤umung, Force-Shield, Globale Superwaffen-Warnung & `production/`-Modularisierung (`production/`, `combat/`, `world/`)

**Description:** Deterministische Simulationslogik fÃ¼r alle Meilenstein-18-Mechaniken inkl. Aufteilung von `production.rs` in `src/production/` (`mod.rs`, `support_structures.rs`):
1) **Klon-StÃ¤tte (`cloning_vats`) & Infanterie-Recycling (`production/mod.rs`, `commands_construction.rs`)**:
   - Jede in einer Kaserne fertiggestellte Infanterie-Einheit spawnt automatisch einen kostenlosen Zwillings-Klon an einer aktiven eigenen Klon-StÃ¤tte.
   - Eigene Infanterie, die per `LoadIntoCarrier` in eine eigene Klon-StÃ¤tte geschickt wird, wird fÃ¼r `75%` ihrer Baukosten recycelt.
2) **Tech-GebÃ¤ude Einnahme-SofortprÃ¤mie (`capture_cash_bonus` in `combat/resolution.rs`)**:
   - Beim erstmaligen Einnehmen eines neutralen/feindlichen GebÃ¤udes mit `capture_cash_bonus > 0` (`!capture_bonus_claimed`) erhÃ¤lt der Eroberer sofort den Credit-Bonus.
3) **Garnisons-RÃ¤umungswaffen (`clears_garrison` in `combat/mod.rs`, `combat/targeting.rs`)**:
   - Waffen mit `clears_garrison = true` fÃ¼gen bei Treffern auf besetzte GebÃ¤ude (`!passenger_ids.is_empty()`) den verschanzten Insassen direkt Schaden zu und entfernen getÃ¶tete Insassen.
4) **Erweitertes Ãœberrollen (`crushes_infantry` vs. `uncrushable` & `b_def.crushable` in `world/movement.rs`)**:
   - Schwere Fahrzeuge Ã¼berrollen verwundbare feindliche Infanterie (`!uncrushable`) und leichte Mauern/ZÃ¤une (`b_def.crushable`), erhalten dafÃ¼r Veterancy- & Commander-XP.
5) **Force-Shield (`SupportPowerKind::ForceShield`) & Globale Superwaffen-Aufdeckung/Warnung (`commands_tactical.rs`, `triggers_and_fow.rs`, `queries.rs`)**:
   - `ForceShield` macht eigene GebÃ¤ude im Zielradius temporÃ¤r unverwundbar (`invulnerable_timer`).
   - Aktive Superwaffen-GebÃ¤ude decken ihren Umkreis fÃ¼r alle Spieler im Fog of War auf und liefern globale Countdown-Warnungen (`active_superweapon_warnings`).

**Acceptance criteria:**
- [ ] Alle neuen Mechaniken arbeiten deterministisch; `production/` ist sauber in Submodule unter 400 Zeilen aufgeteilt; ein neuer Integrationstest in `tests/generals_and_stealth.rs` prÃ¼ft alle Features.

**Verification:**
- [ ] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 74: `rts-client` â€” 360Â°-Kameradrehung (`Q`/`E` Yaw-Orbit & `Home`-Reset), 3D-Wegpunkt-Ketten, Force-Shield-Kuppeln & Superwaffen-Countdown im HUD (`camera.rs`, `rendering/`, `ui/`)

**Description:** Visuelle und steuerungstechnische Integration im 3D-Client:
1) **360Â°-Kameradrehung (`yaw_radians` mit `Q` / `E` in `camera.rs`)**:
   - Sanfte Drehung der 3D-Kamera um den Fokuspunkt (`Q` / `E`), Kamera-Reset mit `Home` und winkelkorrigiertes WASD-/Edge-Panning.
2) **3D-Wegpunkt-Linienketten & Force-Shield-Kuppeln (`rendering/gizmos_world.rs`)**:
   - Zeichnet durchgehende 3D-Pfadlinien Ã¼ber alle `path_waypoints` ausgewÃ¤hlter Einheiten sowie leuchtende Energieschild-Kuppeln um GebÃ¤ude mit aktivem `invulnerable_timer > 0.0`.
3) **Globale Superwaffen-Warnanzeige im Top-HUD (`ui/hud_update.rs`)**:
   - Zeigt aktive Superwaffen-Countdowns (`âš  SW: ...`) prominent in der oberen Kommandozeile an.

**Acceptance criteria:**
- [ ] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [ ] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [ ] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`

---

## Task 63: `rts-mod-sdk` & `rust-ra2` â€” Generals-Doktrinen (`requires_promotion_point`, `cash_bounty_ratio`), Tunnel-Netzwerke (`tunnel_network`), Tarnfeld-Generatoren (`cloak_field_radius`), Erz-Aufbereiter (`ore_refine_multiplier`), Subterrane Einheiten (`subterranean`) & Tarnkisten (`CrateKind::CloakStealth`) (`schema.rs`)

**Description:** Erweiterung des Mod-SDK-Schemas fÃ¼r Meilenstein 17:
1) **Generals-Doktrinen & Kopfgeld-Boni (`UpgradeDef`, `UnitDef` in `schema.rs`)**:
   - `requires_promotion_point: bool` und `cash_bounty_ratio: f32` auf `UpgradeDef` sowie `cash_bounty_ratio: f32` auf `UnitDef` (mit `#[serde(default)]`).
2) **Tunnel-Netzwerke, Tarnfeld-Generatoren, Erz-Aufbereiter & Subterrane Einheiten (`BuildingDef`, `UnitDef` in `schema.rs`)**:
   - `tunnel_network: bool`, `cloak_field_radius: f32` und `ore_refine_multiplier: f32` auf `BuildingDef` (mit `#[serde(default)]`).
   - `subterranean: bool` und `cloak_field_radius: f32` auf `UnitDef` sowie `CrateKind::CloakStealth`.
3) **Referenz-Mod (`rust-ra2`)**:
   - ErgÃ¤nzung von Doktrin-Upgrades (`allied_doctrine_bounty`, `soviet_doctrine_war_spoils`), `allied_gap_generator`, `allied_ore_purifier`, Tunnel-Netzwerk-GebÃ¤uden und `soviet_sub_apc` in `rust-ra2`.

**Acceptance criteria:**
- [x] `UpgradeDef`, `BuildingDef`, `UnitDef` und `CrateKind` unterstÃ¼tzen alle neuen Felder abwÃ¤rtskompatibel.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-mod-sdk`

---

## Task 64: `rts-sim-core` â€” Kommandanten-RÃ¤nge & Promotion Points (`commander_xp`, `commander_rank`, `promotion_points`), Vernetztes Tunnel-Transportsystem (`shared_tunnel_passengers`), Tarnfeld-Generatoren, Erz-Aufbereiter, Subterrane Bewegung, Kopfgeld-System & Persistente Schlachtfeld-Spuren (`SimBattlefieldScar`) (`world.rs`, `combat.rs`, `ai.rs`)

**Description:** Ausbau des deterministischen Simulationskerns (`rts-sim-core`):
1) **Kommandanten-RÃ¤nge & BefÃ¶rderungspunkte (`PlayerEconomy::commander_xp`, `commander_rank`, `promotion_points`)**:
   - GewÃ¤hrt Spielern fÃ¼r Kampfschaden und zerstÃ¶rte Feindeinheiten/-gebÃ¤ude Commander-XP, befÃ¶rdert sie Ã¼ber 5 RÃ¤nge (`1â˜…` bis `5â˜…`) und vergibt je Rang `+1` Promotion Point fÃ¼r Doktrin-Upgrades (`requires_promotion_point`).
2) **Globales Tunnel-Netzwerk (`PlayerEconomy::shared_tunnel_passengers`), Tarnfeld-Aura & Erz-Aufbereiter (`world.rs`, `combat.rs`)**:
   - GebÃ¤ude mit `tunnel_network = true` teilen sich `shared_tunnel_passengers`, sodass Truppen in Tunnel A einsteigen und sofort an Tunnel B wieder aussteigen kÃ¶nnen.
   - Aktive Tarnfeld-Generatoren (`cloak_field_radius`) tarnen verbÃ¼ndete Einheiten im Radius; Erz-Aufbereiter (`ore_refine_multiplier`) steigern den Raffinerie-Ertrag und reichern nahes Erz an.
   - Subterrane Einheiten (`subterranean = true`) graben sich wÃ¤hrend der Bewegung unter die Erde ein (`burrowed: bool`), sind dabei getarnt und Ã¼berwinden Klippen/Hindernisse unterirdisch.
3) **Kopfgeld-Credits (`cash_bounty_ratio`), `CrateKind::CloakStealth` (`permanent_cloak`) & Persistente Schlachtfeld-Krater/Wracks (`SimBattlefieldScar`)**:
   - Gutschrift von Kopfgeld-Credits bei AbschÃ¼ssen, permanente Tarnung durch `CloakStealth`-Kisten und Erzeugung zeitlich verbleibender `SimBattlefieldScar`-EintrÃ¤ge (Krater & Fahrzeugwracks) bei Explosionen.

**Acceptance criteria:**
- [x] Commander-RÃ¤nge, Doktrinen, Tunnel-Netzwerke, Tarnfelder, Erz-Aufbereiter, Subterrane Bewegung, Kopfgelder und Schlachtfeld-Spuren arbeiten 100 % deterministisch und bestehen alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sim-core`

---

## Task 65: `rts-client-3d` â€” 3D-Darstellung von Explosionskratern & Fahrzeugwracks (`SimBattlefieldScar`), 3D-Tarnfeld-Energiekuppeln (`cloak_field_radius`), 3D-Untergrund-Bohrspuren (`burrowed`) & 3D-Formations-Zielvorschau (`render_sync.rs`, `vfx.rs`)

**Description:** Ausbau der 3D-Visualisierung in `render_sync.rs` und `vfx.rs`:
1) **3D-Schlachtfeld-Krater & Rauchende Wracks (`SimBattlefieldScar`)**:
   - Zeichnet dunkle Explosionskrater und verkohlte Fahrzeugwrack-Geometrie auf dem 3D-Terrain inkl. langsamem Ausblenden.
2) **3D-Tarnfeld-Kuppeln, Untergrund-Bohrspuren & 3D-Formations-Zielvorschau (`draw_rts_gizmos`, `vfx.rs`)**:
   - Visualisiert aktive Tarnfeld-Generatoren (`cloak_field_radius`), eingegrabene Einheiten (`burrowed`) mit aufgeworfenen Erdpartikeln sowie die berechneten 3D-Formations-Zielpunkte am Mauszeiger fÃ¼r ausgewÃ¤hlte TruppenverbÃ¤nde.

**Acceptance criteria:**
- [x] 3D-Krater/Wracks, Tarnfeld-Kuppeln, Bohrspuren und Formations-Vorschau-Ringe kompilieren fehlerfrei und laufen ohne Bevy-ECS-Konflikte.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-client-3d`

---

## Task 66: `rts-client-3d` â€” `Shift + Linksklick` Serien-GebÃ¤udebau, Commander-Rang- & Promotion-Point-Anzeige im HUD sowie Doktrin-Buttons in der Sidebar (`input.rs`, `hud.rs`)

**Description:** VollstÃ¤ndige Integration aller Meilenstein-17-Systeme in Steuerung und HUD:
1) **`Shift + Linksklick` Serien-GebÃ¤udeplatzierung (`input.rs`)**:
   - HÃ¤lt der Spieler beim Platzieren eines GebÃ¤udes `Shift` gedrÃ¼ckt, bleibt `placing_building_id` aktiv, sodass mehrere GebÃ¤ude in Serie platziert werden kÃ¶nnen.
2) **Commander-Rang (`â˜…`), Promotion Points (`PP`) & Doktrin-Freischaltung im HUD (`hud.rs`)**:
   - Zeigt aktuellen Commander-Rang, XP-Fortschritt und verfÃ¼gbare Promotion Points in der Top-Bar an und hebt Doktrin-Upgrades (`[â˜… DOCTRINE: 1 PP]`) in der Sidebar hervor.

**Acceptance criteria:**
- [x] `Shift + LMB` Serien-GebÃ¤udebau, Commander-Rang-HUD und Doktrin-Buttons funktionieren fehlerfrei.
- [x] Alle Tests und Clippy-Checks Ã¼ber den gesamten Workspace bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 43: `rts-mod-sdk` & `rts-net` â€” Kampagnen-Schema (`CampaignDef`, `MissionDef`), Minenleger-Schema (`lays_mines`) & Multiplayer-Chat (`NetMessage::ChatMessage`) (`schema.rs`, `loader.rs`, `validator.rs`, `protocol.rs`, `transport.rs`)

**Description:** Ausbau von `rts-mod-sdk` und `rts-net` fÃ¼r Kampagnen-Progression, Landminen und Multiplayer-Chat:
1) **Kampagnen- & Missions-Schema (`CampaignDef`, `MissionDef` in `schema.rs`, `loader.rs`, `validator.rs`)**:
   - `CampaignDef` (`id`, `display_name`, `faction`, `missions: Vec<MissionDef>`) und `MissionDef` (`id`, `display_name`, `map_id`, `briefing`, `next_mission_id`, `carry_over_credits_ratio`).
   - Automatisches Laden von `campaigns/*.toml` in `LoadedMod.campaigns` sowie Referenz-PrÃ¼fung im Mod-Validator.
2) **Minenleger-Eigenschaften auf `UnitDef` (`schema.rs`)**:
   - `lays_mines: bool`, `mine_damage: f32`, `mine_radius: f32` (mit `#[serde(default)]`).
3) **Multiplayer-Chat (`protocol.rs`, `transport.rs`)**:
   - `NetMessage::ChatMessage { sender_player_id: u8, team_only: bool, message: String }` sowie Sende-/Empfangsmethoden in `LockstepServer` und `LockstepClient`.

**Acceptance criteria:**
- [x] `CampaignDef` und `MissionDef` laden und validieren fehlerfrei; `rust-ra2` enthÃ¤lt eine Beispiel-Kampagne (`campaigns/campaigns.toml`).
- [x] `LockstepServer` und `LockstepClient` Ã¼bertragen `NetMessage::ChatMessage` zuverlÃ¤ssig zwischen Peers.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-mod-sdk -p rts-net`

---

## Task 44: `rts-sim-core` â€” Dynamische Diplomatie & Tribut-Handel (`SetAlliance`, `TransferCredits`, `TransferUnits`), Landminen (`SimMine`, `PlaceMine`), Wetter-Gameplay-Effekte & Fortgeschrittene KI (`command.rs`, `world.rs`, `combat.rs`, `ai.rs`)

**Description:** Erweiterung des deterministischen Simulationskerns um die verbleibenden RTS-Gameplay- und KI-Systeme:
1) **Dynamische Diplomatie & Tribut-System (`SetAlliance`, `TransferCredits`, `TransferUnits`)**:
   - Laufzeit-Allianzen (`allied_players` inkl. geteilter Fog-of-War-Sicht), Credits-Ãœberweisung an andere Spieler und Ãœbergabe ausgewÃ¤hlter Einheiten an VerbÃ¼ndete.
2) **Taktische Landminen (`SimMine`, `GameCommand::PlaceMine`)**:
   - Platzieren getarnter Minen auf Bodenkacheln durch Einheiten mit `lays_mines` (oder `is_engineer`); Detonation mit FlÃ¤chenschaden bei feindlichem Kontakt, Aufdeckung durch `detects_cloak`-Detektoren und EntschÃ¤rfung durch Ingenieure.
3) **Gameplay-Auswirkungen von Wetter/Tageszeit (`EnvironmentPreset`)**:
   - `Night` reduziert Sichtweiten um 25 %; `IonStorm` reduziert Flugzeug-Geschwindigkeit um 30 % und stÃ¶rt einfaches Radar.
4) **Fortgeschrittene KI (`ai.rs`)**:
   - Ingenieur-Ausbildung zur Einnahme neutraler Tech-GebÃ¤ude (`player_id == 0`) und BrÃ¼ckenreparatur, Werft-/Marine-Bau auf Wasserkarten sowie automatisches Ausweichen aus Strahlungszonen und Artillerie-Kiting.

**Acceptance criteria:**
- [x] Dynamische Allianzen, Tribut-Zahlungen, Einheiten-Ãœbergabe, Landminen, Wetter-Modifikatoren und die erweiterte KI arbeiten 100 % deterministisch und bestehen alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sim-core`

---

## Task 45: `rts-mod-tools` & `rts-client-3d` â€” 3D-Sub-Node-Animationen (`Rotor`-Node, Prozedurales Walk-/Tilt-Rigging) & 3D-Landminen-Darstellung (`glb_gen.rs`, `render_sync.rs`)

**Description:** Ausbau der 3D-Modell- und Animations-Pipeline:
1) **`Rotor`-Sub-Node in `.glb`-Modellen (`glb_gen.rs`)**:
   - Erzeugt fÃ¼r Lufteinheiten und Radar-/Sensor-GebÃ¤ude einen dedizierten `"Rotor"`-Child-Node in der glTF-Hierarchie.
2) **Echtzeit-Sub-Node- & Bewegungs-Animation (`render_sync.rs`)**:
   - Kontinuierliche Rotation von `"Rotor"`-Nodes, prozeduraler Infanterie-Laufzyklus (Stride/Bobbing), Fahrzeug-Neigung beim Beschleunigen und 3D-Darstellung von `SimMine`-Objekten.

**Acceptance criteria:**
- [x] `glb_gen` erzeugt valide `.glb`-Modelle mit `"Rotor"`-Nodes; `render_sync.rs` animiert Rotoren, Infanterie und Fahrzeuge flÃ¼ssig.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-mod-tools -p rts-client-3d`

---

## Task 46: `rts-client-3d` â€” Kampagnen-Progression (â€žNext Missionâ€œ), Diplomatie-/Tribut-UI, In-Game-Chat (`[Enter]`), Freie IP-Eingabe & Replay-Scrubber mit POV-Switch (`input.rs`, `hud.rs`, `main.rs`)

**Description:** VollstÃ¤ndige Integration aller Meilenstein-12-Systeme in HUD, Steuerung und Engine-Studio:
1) **Kampagnen-Auswahl, Missions-Briefing & â€žNEXT MISSIONâ€œ-Ãœbergang**:
   - Starten von Kampagnen im Studio, Anzeige des Briefings und direkter Ãœbergang zur Folgemission (`next_mission_id`) nach einem Sieg inkl. Credits-Carry-Over.
2) **Diplomatie- & Tribut-Steuerung im `[Tab]`-Scoreboard / Studio + Landminen-Hotkey (`[M]`)**:
   - Buttons fÃ¼r Allianz-Toggle (`ALLY`/`ENEMY`), `+500$` Tribut-Zahlung und `GIFT UNITS` sowie `[M]` zum Platzieren von Landminen.
3) **In-Game-Chat (`[Enter]`) & Editierbares Server-IP:Port-Textfeld im Studio**:
   - Live-Chat-Eingabe per Tastatur und Anzeige der letzten Chat-Nachrichten im HUD sowie freie Tastatureingabe fÃ¼r `IP:Port` im Multiplayer-MenÃ¼.
4) **Replay-Timeline-Scrubber (`<<` / `>>`) & Fog-of-War POV-Umschalter (`ALL VISION` / `P1` / `P2`)**:
   - Vor-/ZurÃ¼ckspulen in laufenden Replays und Umschalten der Spielersicht im Observer-/Replay-Modus.

**Acceptance criteria:**
- [x] Kampagnen-Flow, Diplomatie-Buttons, `[M]`-Minenlegen, `[Enter]`-Chat, IP-Textfeld und Replay-Scrubber/POV funktionieren fehlerfrei.
- [x] Alle Tests und Clippy-Checks Ã¼ber den gesamten Workspace bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 47: `rts-mod-sdk` â€” Fahrzeug-Transporter (`can_carry_vehicles`), Parasiten-Waffen (`parasite_dps`), Deklaratives Map-Trigger-System (`TriggerCondition`, `TriggerAction`) & 3D-Map-Props (`PropKind`, `MapPropDef`) (`schema.rs`, `validator.rs`)

**Description:** Erweiterung des Mod-SDK-Schemas fÃ¼r Meilenstein 13:
1) **Fahrzeug-Transporter & Parasiten-Befall (`schema.rs`)**:
   - `can_carry_vehicles: bool` auf `UnitDef` sowie `parasite_dps: f32` auf `WeaponDef` (jeweils mit `#[serde(default)]`).
2) **Deklaratives Map-Trigger-System (`TriggerCondition`, `TriggerAction` in `schema.rs`, `validator.rs`)**:
   - Erweiterung von `TriggerZoneDef` um optionale deklarative Bedingungen (`EnteredZone`, `TimerElapsed`, `BuildingCaptured`, `PlayerUnitsBelow`) und Aktionen (`SpawnReinforcements`, `GrantCredits`, `RevealArea`, `SetObjective`, `SetWeather`, `DeclareWinner`).
3) **ZerstÃ¶rbare & Explosive 3D-Map-Props (`PropKind`, `MapPropDef` in `schema.rs`)**:
   - `PropKind` (`Tree`, `Streetlight`, `ExplosiveBarrel`) und `props: Vec<MapPropDef>` auf `MapDef`.

**Acceptance criteria:**
- [x] `UnitDef`, `WeaponDef`, `TriggerZoneDef` und `MapDef` unterstÃ¼tzen alle neuen Felder abwÃ¤rtskompatibel; `validate_mod` prÃ¼ft `SpawnReinforcements`-Einheitenreferenzen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-mod-sdk`

---

## Task 48: `rts-sim-core` â€” Fahrzeug-Transport, Parasiten-Infektion & Service-Depot-Docking (`DockAtServiceDepot`), ZerstÃ¶rbare/Explosive `SimProp`-Objekte & Deklarative Map-Trigger-AusfÃ¼hrung (`command.rs`, `world.rs`, `combat.rs`)

**Description:** Ausbau des deterministischen Simulationskerns (`rts-sim-core`):
1) **Fahrzeug-Transporter (`can_carry_vehicles`)**:
   - Erlaubt `LoadIntoCarrier` auch fÃ¼r `UnitRole::Vehicle` und `UnitRole::Harvester`, wenn der Transporter `can_carry_vehicles = true` besitzt.
2) **Parasiten-Befall (`infected_parasite_dps`) & Service-Depot-Reparatur/Reinigung (`GameCommand::DockAtServiceDepot`)**:
   - Waffen mit `parasite_dps > 0.0` (z. B. Terror-Drohne, Riesenkalmar) infizieren getroffene Fahrzeuge/Schiffe mit kontinuierlichem Innenschaden und 30 % Verlangsamung, wobei der Angreifer im Ziel aufgeht.
   - Ein Service-Depot (`repairs_vehicles_per_sec > 0.0`) oder mobiler Mechaniker entfernt den Parasiten sofort und repariert das Fahrzeug; `GameCommand::DockAtServiceDepot` schickt ausgewÃ¤hlte Fahrzeuge gezielt zum Service-Depot.
3) **ZerstÃ¶rbare & Explosive Karten-Props (`SimProp`)**:
   - Schwere Fahrzeuge (`crushes_infantry`) walzen BÃ¤ume/Laternen (`Tree`, `Streetlight`) nieder; FlÃ¤chenschaden zerstÃ¶rt Props und bringt `ExplosiveBarrel`-FÃ¤sser mit `160.0` Splash-Schaden (inkl. Kettenreaktionen) zur Explosion.
4) **Deklarative Map-Trigger-AusfÃ¼hrung (`step_map_triggers`)**:
   - Wertet `TriggerCondition` deterministisch aus und fÃ¼hrt `TriggerAction`s (`SpawnReinforcements`, `GrantCredits`, `RevealArea`, `SetObjective`, `SetWeather`, `DeclareWinner`) aus.

**Acceptance criteria:**
- [x] Fahrzeug-Transport, Parasiten-Infektion + Heilung am Service-Depot, explosive FÃ¤sser und deklarative Map-Trigger arbeiten deterministisch und bestehen alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sim-core`

---

## Task 49: `rts-client-3d` â€” 3D-Darstellung von Map-Props (`Tree`, `Streetlight`, `ExplosiveBarrel`), Persistente 3D-Einschlagkrater (`ScorchMark`) & Dynamische Sonnen-/Himmel-/Ionensturm-AtmosphÃ¤re (`render_sync.rs`, `vfx.rs`)

**Description:** Erweiterung der 3D-Umgebungs- und Effekt-Darstellung:
1) **3D-Map-Props (`render_sync.rs`)**:
   - Zeichnet 3D-BÃ¤ume, StraÃŸenlaternen und rote Gefahrgut-/SprengstofffÃ¤sser (`SimProp`) auf dem Terrain.
2) **Persistente 3D-Einschlagkrater & Brandspuren (`vfx.rs`)**:
   - Schwere Explosionen hinterlassen nachglÃ¼hende/geschwÃ¤rzte 3D-Kraterringe auf dem Boden, die Ã¼ber Zeit verblassen.
3) **Dynamische Himmels-, Nebel- & Sonnenwinkel-Transition + Ionensturm-Blitze (`render_sync.rs`)**:
   - Passt `ClearColor`, `DistanceFog`, Sonnenrichtung (Schattenwurf) und periodische Blitz-Aufhellungen im `IonStorm` live an `EnvironmentPreset` an.

**Acceptance criteria:**
- [x] 3D-Props, Krater und Wetter-/Sonnen-AtmosphÃ¤re kompilieren und laufen ohne ECS-Konflikte.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-client-3d`

---

## Task 50: `rts-client-3d` â€” Multi-Selection Sub-Group-Tabbing & Filter-Chips, `Shift+Klick` (5x) Batch-Training mit Queue-Badges (`[xN]`), Rechtsklick-Service-Depot-Docking & Persistente `saves/settings.toml` (`input.rs`, `hud.rs`, `audio.rs`)

**Description:** Komfort-, UI- und Konfigurations-Ausbau im 3D-Client:
1) **Multi-Selection Sub-Group-Tabbing (`[Tab]` bei Auswahl) & Klickbare Sub-Group-Filter-Chips**:
   - Zeigt bei gemischten Auswahlen alle enthaltenen Einheitentypen mit Anzahl (`[GI x6] [Grizzly x4]`) im unteren Inspektor und als Filter-Buttons in der Sidebar an; `[Tab]` schaltet die aktive Untergruppe fÃ¼r gezielte FÃ¤higkeiten (`[G]`) weiter.
2) **`Shift + Linksklick` (5x Batch-Produktion) & Live-Queue-ZÃ¤hler (`[xN]`)**:
   - `Shift + Linksklick` auf einen Einheiten-Button reiht 5 Einheiten auf einmal ein; jeder Button zeigt die aktuelle Anzahl in den Produktionswarteschlangen (`[xN]`).
3) **Rechtsklick auf eigenes Service-Depot**:
   - Schickt ausgewÃ¤hlte Fahrzeuge per `GameCommand::DockAtServiceDepot` direkt zur Reparatur/Entseuchung.
4) **Persistente Engine-Einstellungen (`saves/settings.toml`)**:
   - Speichert und lÃ¤dt Audio-LautstÃ¤rke (`master_volume`), Kamera-Geschwindigkeit (`camera_pan_speed`) und Standard-Fog-of-War automatisch beim Start und erlaubt Anpassungen direkt im Engine-Studio.

**Acceptance criteria:**
- [x] Sub-Group-Cycling, Batch-Training (`5x`), Queue-Badges, Service-Depot-Docking und `saves/settings.toml`-Persistenz funktionieren fehlerfrei.
- [x] Alle Tests und Clippy-Checks Ã¼ber den gesamten Workspace bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 51: `rts-mod-sdk` â€” Datengetriebene Support-Powers (`SupportPowerDef`, `SupportPowerKind`) & Flugplatz-LandeplÃ¤tze (`airfield_pads`) (`schema.rs`, `validator.rs`)

**Description:** Erweiterung des Mod-SDK-Schemas fÃ¼r Meilenstein 14:
1) **Taktische Kommandanten-/Support-Powers (`SupportPowerDef`, `SupportPowerKind` in `schema.rs`)**:
   - UnterstÃ¼tzt `SupportPowerKind::Paradrop`, `RadarScan`, `Airstrike` und `ChronoShift` mit Cooldown (`cooldown_secs`), Zielradius (`radius`), Abwurf-Einheit (`unit_id`, `count`), Aufdeck-Dauer (`duration_secs`) und FlÃ¤chenschaden (`damage`).
   - `support_powers: Vec<SupportPowerDef>` und `airfield_pads: u8` auf `BuildingDef` (mit `#[serde(default)]`).
2) **Statische Referenz-Validierung (`validator.rs`)**:
   - PrÃ¼ft in `validate_mod`, dass jede `SupportPowerDef` mit `unit_id: Some(uid)` auf eine gÃ¼ltige Einheit in `loaded.units` verweist.

**Acceptance criteria:**
- [x] `BuildingDef` unterstÃ¼tzt `support_powers` und `airfield_pads` abwÃ¤rtskompatibel; `validate_mod` prÃ¼ft `SupportPowerDef`-Einheitenreferenzen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-mod-sdk`

---

## Task 52: `rts-sim-core` â€” Support-Powers-AusfÃ¼hrung (`CastSupportPower`, `parachute_timer`), Flugplatz-Docking & Luftraum-Kreisen (`ReturnToAirfield`, `landed_at_airfield`, `loiter_angle`), Gedrehte GebÃ¤ude-Footprints (`[R]`) & Intelligente Sammler-Koordination (`command.rs`, `world.rs`)

**Description:** Erweiterung des deterministischen Simulationskerns (`rts-sim-core`):
1) **Support-Powers & Fallschirm-AbwÃ¼rfe (`GameCommand::CastSupportPower`, `parachute_timer`)**:
   - Verwaltung pro Spieler (`support_power_cooldowns` auf `PlayerEconomy`), Cooldown-Tick und deterministische AusfÃ¼hrung von `Paradrop` (Einheiten schweben an Fallschirmen von `y = 6.5` herab), `RadarScan` (`temporary_reveal_zones`), `Airstrike` (FlÃ¤chenschaden + Krater) und `ChronoShift` (Gruppen-Teleport von Fahrzeugen).
2) **Flugplatz-Landeplatz-Bindung, 3D-Landung & Luftraum-Kreisen (`GameCommand::ReturnToAirfield`, `landed_at_airfield`, `loiter_angle`)**:
   - Flugzeuge erhalten ein `home_airfield_id`, kehren bei `current_ammo == 0` oder per `ReturnToAirfield` automatisch zum Flugplatz zurÃ¼ck, setzen auf dem Pad auf (`landed_at_airfield = true`, `y = elev + 0.45`), laden Munition/HP nach und kreisen im Flugbetrieb Ã¼ber ihrem Zielgebiet (`loiter_angle`).
3) **Gedrehte GebÃ¤ude-Platzierung (`PlaceBuilding` mit `rotated: bool` / `custom_footprint`)**:
   - Erlaubt das Drehen asymmetrischer GebÃ¤ude-Footprints (`[w, h]` -> `[h, w]`) beim Platzieren.
4) **Intelligente Sammler-Koordination & Gefahren-RÃ¼ckzug (`step_harvesters`)**:
   - Sammler wÃ¤hlen automatisch die am wenigsten ausgelastete Raffinerie, verteilen sich auf freie Erzkacheln und ziehen sich bei schwerem Beschuss selbststÃ¤ndig zur nÃ¤chsten Raffinerie zurÃ¼ck.

**Acceptance criteria:**
- [x] Support-Powers, Fallschirm-Landung, Flugplatz-Pad-Docking, GebÃ¤ude-Rotation und Smart-Harvester-Logik arbeiten deterministisch und bestehen alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sim-core`

---

## Task 53: `rts-client-3d` â€” 3D-Fallschirme, 3D-Flugplatz-Landung & Loiter-Banking, 3D-Bauradius-Ring & Einzelkachel-Footprint-Raster sowie 3D-Wegpunkt-Ketten & 3D-Sammelpunkt-Fahnen (`render_sync.rs`)

**Description:** Ausbau der 3D-Visualisierung in `render_sync.rs`:
1) **3D-Fallschirm-Kuppeln & Seile**:
   - Zeichnet Ã¼ber Einheiten mit `parachute_timer > 0.0` eine 3D-Fallschirmkuppel wÃ¤hrend des Sinkflugs.
2) **3D-Flugplatz-Landung & Kurvenlage (Banking)**:
   - Gelandete Flugzeuge (`landed_at_airfield`) stehen sichtbar auf den Pads des Flugplatzes; fliegende Jets kreisen mit sanfter Roll-Neigung (`loiter_angle`).
3) **3D-Bauradius-Ring & Kachel-Footprint-Raster (`PlacementTileGrid`)**:
   - Zeigt im Baumodus den erlaubten Bauradius um eigene GebÃ¤ude sowie fÃ¼r jede einzelne Footprint-Kachel unter dem Ghost-Modell GrÃ¼n (bebaubar) oder Rot (blockiert).
4) **VollstÃ¤ndige 3D-Wegpunkt-Ketten (`waypoint_queue`) & 3D-Rally-Point-Fahnen**:
   - Verbindet alle geplanten `Shift+RMB`-Wegpunkte ausgewÃ¤hlter Einheiten mit 3D-Linien und zeichnet fÃ¼r ausgewÃ¤hlte Fabriken eine 3D-Sammelpunkt-Fahne samt Verbindungslinie.

**Acceptance criteria:**
- [x] Alle neuen 3D-Visualisierungen kompilieren fehlerfrei und laufen ohne Bevy-ECS-Konflikte.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-client-3d`

---

## Task 54: `rts-client-3d` â€” Support-Power-HUD & Zielauswahl, `[R]` GebÃ¤ude-Rotation, `[F9]` In-Game Tech-Tree- & Einheiten-EnzyklopÃ¤die sowie Konfigurierbare Hotkeys in `saves/settings.toml` (`input.rs`, `hud.rs`)

**Description:** Integration aller Meilenstein-14-Systeme in Steuerung, HUD und Persistenz:
1) **Support-Power-Sidebar & Zielmodus (`SidebarAction::TriggerSupportPower`)**:
   - Zeigt freigeschaltete Support-Powers mit Live-Cooldown in der rechten Sidebar; Klick aktiviert den Zielmodus (inkl. 2-Stufen-Quell-/Zielklick fÃ¼r `ChronoShift`).
2) **`[R]` GebÃ¤ude-Rotation im Baumodus & Rechtsklick-Flugplatz-RÃ¼ckkehr**:
   - `[R]` dreht die Footprint-Ausrichtung wÃ¤hrend der GebÃ¤udeplatzierung; Rechtsklick mit ausgewÃ¤hlten Flugzeugen auf ein eigenes `Airfield` sendet `GameCommand::ReturnToAirfield`.
3) **`[F9]` In-Game Tech-Tree- & Einheiten-EnzyklopÃ¤die-Overlay (`TechTreeModalRoot`)**:
   - Zeigt dynamisch aus `LoadedMod` alle Fraktionen, GebÃ¤ude- und Einheiten-Voraussetzungen (`prerequisites`), Kosten, Waffen-DPS, Konter-Matrix (`damage_vs_armor`) und Support-Powers.
4) **Konfigurierbare Hotkeys in `saves/settings.toml` (`ClientSettings`)**:
   - Speichert und lÃ¤dt frei anpassbare TastaturkÃ¼rzel (`hotkey_attack_move`, `hotkey_stop`, `hotkey_scatter`, `hotkey_activate_ability`, `hotkey_rotate_building`).

**Acceptance criteria:**
- [x] Support-Power-Buttons, `[R]`-Rotation, `[F9]`-EnzyklopÃ¤die und Hotkey-Persistenz in `saves/settings.toml` funktionieren fehlerfrei.
- [x] Alle Tests und Clippy-Checks Ã¼ber den gesamten Workspace bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 39: `rts-mod-sdk` & `rts-net` â€” Einheiten-Detektoren (`detects_cloak`), Mobile Heiler (`heal_per_sec`, `heal_radius`) & Lockstep-`WorldStateSync`-Resync (`schema.rs`, `protocol.rs`, `transport.rs`)

**Description:** Erweiterung von `rts-mod-sdk` und `rts-net`:
1) **Einheiten-Tarnungsdetektoren & Mobile Heiler/Mechaniker (`schema.rs`)**:
   - `detects_cloak: bool`, `heal_per_sec: f32` und `heal_radius: f32` auf `UnitDef` (mit `#[serde(default)]`).
2) **Lockstep-Snapshot-Resync (`protocol.rs`, `transport.rs`)**:
   - `NetMessage::WorldStateSync { tick: u64, world_json: String }` zum Senden und Empfangen vollstÃ¤ndiger `SimWorld`-Snapshots bei Desync oder Reconnect sowie konfigurierbare Spieler-Slots (`2..=8`).

**Acceptance criteria:**
- [x] `UnitDef` unterstÃ¼tzt `detects_cloak`, `heal_per_sec` und `heal_radius`.
- [x] `LockstepServer` und `LockstepClient` leiten `WorldStateSync`-Snapshots deterministisch weiter.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-mod-sdk -p rts-net`

---

## Task 40: `rts-sim-core` â€” Sidebar-Vorproduktion (`QueueBuilding`), Mauer-Linienbau (`PlaceWallLine`), Boden-Zwangsbeschuss (`AttackGround` + BrÃ¼cken-Sprengung), Live-Re-Pathing & Mobile Heiler (`command.rs`, `world.rs`, `combat.rs`, `pathfinding.rs`)

**Description:** Implementierung der letzten 4 Simulations-Bausteine in `rts-sim-core`:
1) **Sidebar-GebÃ¤ude-Vorproduktion (`building_queue`, `ready_building_id`) & Mauer-Linienbau (`GameCommand::PlaceWallLine`)**:
   - `GameCommand::QueueBuilding` und `GameCommand::CancelBuildingQueue` fÃ¼r Westwood-typisches Vorbauen in der Sidebar (â€žConstruction Complete -> Placeâ€œ) inkl. Multi-ConYard-Beschleunigung.
   - `GameCommand::PlaceWallLine` zieht in einem Befehl eine verbundene Linie aus Mauern/Toren/BrÃ¼cken von `start_tile` bis `end_tile`.
2) **Boden-Zwangsbeschuss (`GameCommand::AttackGround`) & ZerstÃ¶rbare/Reparierbare BrÃ¼cken im Gefecht**:
   - Einheiten mit `attack_ground_pos` feuern auf beliebige Boden-/Wasserkacheln (auch in den Fog of War), verursachen Splash-Schaden und kÃ¶nnen BrÃ¼cken (`bridge_tiles`) durch Beschuss sprengen (woraufhin Einheiten auf der BrÃ¼cke versinken); Ingenieure kÃ¶nnen zerstÃ¶rte `bridge_seed_tiles` wieder instand setzen.
3) **Dynamisches Live-Re-Pathing & Stuck-Recovery (`step_movement`)**:
   - PrÃ¼ft vor jedem Schritt, ob der nÃ¤chste Wegpunkt durch ein neues GebÃ¤ude, geschlossenes Tor oder eine gesprengte BrÃ¼cke unpassierbar wurde, und berechnet bei Blockade oder `stuck_timer` automatisch eine Ausweichroute.
4) **Mobile SanitÃ¤ter/Mechaniker (`heal_per_sec`), Einheiten-Detektoren (`detects_cloak`) & GebÃ¤ude-Schadensstufen**:
   - Einheiten mit `heal_per_sec > 0.0` heilen beschÃ¤digte eigene/alliierte Einheiten im Umkreis automatisch; Einheiten mit `detects_cloak` decken getarnte Feinde auf; GebÃ¤ude unter 25 % HP (`Condition Red`) arbeiten um 25 % langsamer.

**Acceptance criteria:**
- [x] `QueueBuilding`, `PlaceWallLine`, `AttackGround` (inkl. BrÃ¼cken-ZerstÃ¶rung), Live-Re-Pathing und mobile Heilung/Detektion arbeiten deterministisch und bestehen alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sim-core`

---

## Task 41: `rts-client-3d` â€” ZusammenhÃ¤ngendes 3D-Heightmap-Terrain-Mesh, Kamera-HÃ¶henanpassung & Kontinuierliche Schadens-Rauch-/Feuer-VFX (`render_sync.rs`, `vfx.rs`, `camera.rs`)

**Description:** Ausbau der 3D-Grafik- und Effekt-Engine:
1) **ZusammenhÃ¤ngendes 3D-Heightmap-Terrain-Mesh (`build_continuous_terrain_mesh` in `render_sync.rs`)**:
   - Erzeugt ein echtes, zusammenhÃ¤ngendes Vertex-Color-3D-GelÃ¤nde-Mesh aus `elevation_map`, `ramp_tiles` und `water_tiles` mit sanften Rampen-SchrÃ¤gen, Felsflanken und Flussbetten.
2) **Kamera-Fokus-HÃ¶henanpassung (`camera.rs`, `input.rs`)**:
   - Passt `rig.focus.y` sanft an die aktuelle GelÃ¤ndehÃ¶he unter dem Kamerafokus an.
3) **GebÃ¤ude-/Fahrzeug-Schadens-VFX (`vfx.rs`)**:
   - Spawnt kontinuierlich aufsteigenden Rauch bei `< 50 %` HP (`Condition Yellow`) und flackernde Flammen bei `< 25 %` HP (`Condition Red`).

**Acceptance criteria:**
- [x] Das 3D-Heightmap-Mesh, die Kamera-HÃ¶henanpassung und die Schadens-Partikel kompilieren und laufen fehlerfrei.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-client-3d`

---

## Task 42: `rts-client-3d` â€” Steuerung & UI fÃ¼r `Ctrl + RMB` Ground-Fire, Mauer-Drag-Build, Sidebar-Vorproduktion & Savegame-/Replay-Browser (`input.rs`, `hud.rs`)

**Description:** VollstÃ¤ndige Bedienbarkeit aller neuen Engine-Systeme im Client:
1) **`Ctrl + Rechtsklick` (`AttackGround`)**: Erzwingt Beschuss auf beliebige Boden-/BrÃ¼ckenkacheln im 3D-Viewport und auf der Minimap.
2) **Mauer-Linien-Drag-Build (`wall_drag_start_tile`)**: Ziehen mit gedrÃ¼ckter linker Maustaste beim Platzieren von Mauern zeichnet eine 3D-Vorschaulinie und sendet `PlaceWallLine`.
3) **Sidebar-GebÃ¤ude-Vorproduktion**: Anzeige des Baufortschritts (`READY` / `%`), Rechtsklick zum Vorbauen/Abbrechen (`QueueBuilding` / `CancelBuildingQueue`).
4) **Dynamischer Savegame- & Replay-Browser + Multi-Slot-Host & Resync im Engine-Studio (`hud.rs`)**:
   - Listet alle `.json`-Savegames und `.replay.json`-Dateien im Ordner `saves/` als direkt anklickbare Buttons auf und erlaubt Multiplayer-Hosting fÃ¼r `2 / 4 / 8` Spieler sowie manuellen/automatischen `WorldStateSync`.

**Acceptance criteria:**
- [x] Alle neuen Steuerungen (`Ctrl + RMB`, Mauer-Drag, Sidebar-Queue, Savegame-/Replay-Browser) funktionieren fehlerfrei.
- [x] Alle Tests und Clippy-Checks Ã¼ber den gesamten Workspace bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 27: `rts-mod-sdk` â€” Generisches Buff-/Ability-/Upgrade-Schema, Multi-Map-Support, Rampen & Skript-Event-Bus (`schema.rs`, `loader.rs`, `scripting.rs`)

**Description:** Ausbau von `rts-mod-sdk` zu einem vollstÃ¤ndig mod-unabhÃ¤ngigen RTS-Daten- und Skript-Framework:
1) **Generisches `StatusEffectDef`, `AbilityDef` & `UpgradeDef` Schema (`schema.rs`)**:
   - `AbilityTargetMode` (`SelfOnly`, `SingleEntity`, `GroundPoint`, `AreaRadius`), `StatusEffectDef` (Dauer, Speed-/Damage-/Armor-Multiplikatoren, `invulnerable`, `stunned`, `dot_dps`), `AbilityDef` (Cooldown, Reichweite, Radius, Zielmodus, Status-Effekt, Skript-Hook) und `UpgradeDef` (Erforschbare globale Fraktions-/Spieler-Upgrades).
   - `supply_cost` und `supply_provided` auf Einheiten/GebÃ¤uden fÃ¼r generisches BevÃ¶lkerungs-/Supply-Management.
   - `ramp_rects` in `MapDef` sowie `MapDef::save_to_file` fÃ¼r den `.toml`-Kartenexport.
2) **Multi-Map- & Upgrade-Loader (`loader.rs`)**:
   - LÃ¤dt alle Karten im `maps/`-Verzeichnis in `LoadedMod.maps` sowie optionale `upgrades/upgrades.toml` in `LoadedMod.upgrades`.
3) **Globaler Skript-Event-Bus (`scripting.rs`)**:
   - `WorldEventContext` & `ScriptHost::invoke_world_event` (`on_tick`, `on_unit_spawned`, `on_entity_destroyed`, `on_building_captured`) fÃ¼r Rhai und Lua.

**Acceptance criteria:**
- [x] `StatusEffectDef`, `AbilityDef`, `UpgradeDef`, `ramp_rects` und `LoadedMod.maps` laden und serialisieren fehlerfrei.
- [x] `ScriptHost::invoke_world_event` fÃ¼hrt globale Rhai- und Lua-Welt-Events deterministisch aus.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-mod-sdk`

---

## Task 28: `rts-sim-core` â€” Unit Stances, Formationen, Patrol & Guard, Klippen-/Rampen-Pathfinding mit Line-of-Sight, Teams/Neutral & Upgrades (`pathfinding.rs`, `command.rs`, `world.rs`, `combat.rs`)

**Description:** Erweiterung des deterministischen Simulationskerns um universelle RTS-Mechaniken:
1) **Unit Stances (`Aggressive`, `Defensive`, `HoldGround`, `HoldFire`), `PatrolUnits` & `GuardEntity`**:
   - Stance-abhÃ¤ngiges Ziel- und Verfolgungsverhalten, Endlos-Patrouillen zwischen Wegpunkten und Eskortieren von Zieleinheiten.
2) **Taktische Gruppen-Formationen (`compute_squad_formation_offsets`)**:
   - Automatische Staffelung von Fahrzeuggruppen, Infanterie und FernkÃ¤mpfern beim Bewegungsbefehl entlang der Marschrichtung.
3) **Klippen- vs. Rampen-Pathfinding & Line-of-Sight (LoS) HÃ¶hen-Abschattung**:
   - `ramp_tiles` in `CollisionGrid`, HÃ¶hen-TraversierungsprÃ¼fung und Sichtlinien-Blockade von tieferem GelÃ¤nde auf hohe Plateaus.
4) **Generische `ActiveBuff`-Effekte, Supply/Pop-Cap, Forschungs-Upgrades (`ResearchUpgrade`), Ziel-Abilities (`CastTargetedAbility`) & Teams (`team_id`, `player_id = 0` Neutral)**.

**Acceptance criteria:**
- [x] Gruppen bewegen sich in geordneter Formation; `PatrolUnits`, `GuardEntity` und `SetUnitStance` funktionieren deterministisch.
- [x] Upgrades, Supply-Berechnung, generische Buffs, Line-of-Sight-Abschattung und `export_current_map_def` bestehen alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sim-core`

---

## Task 29: `rts-mod-tools` & `rts-client-3d` â€” Mehrteilige `.glb`-Szenenhierarchie (`Hull` + `Turret` Child-Node), Sub-Node-3D-Rigging & Echte Bevy-Audio-Wiedergabe (`glb_gen.rs`, `render_sync.rs`, `audio.rs`)

**Description:** Upgrade der 3D-Asset- und Audio-Pipeline:
1) **Mehrteilige `.glb`-Hierarchie (`glb_gen.rs`)**:
   - Erzeugt echte 2-Node-glTF-Szenen (`Node 0: "Hull"`, Child `Node 1: "Turret"`) fÃ¼r Fahrzeuge, GeschÃ¼tztÃ¼rme, Tech-Zentren und Lufteinheiten.
2) **Echtes `.glb` Sub-Node-Rigging (`render_sync.rs`)**:
   - Rotiert den `"Turret"`-Child-Node geladener `.glb`-Modelle direkt anhand von `turret_facing_radians - facing_radians` inkl. Rohr-RÃ¼ckstoÃŸ und rendert 3D-Rampen (`ramp_tiles`).
3) **Echte Bevy-Audio-Pipeline (`audio.rs`)**:
   - Registriert die synthetisierten `.wav`-Sounds (sowie optionale Mod-Sounds) als `Assets<AudioSource>` und spawnt echte Bevy-`AudioPlayer`-Entities mit DistanzdÃ¤mpfung.

**Acceptance criteria:**
- [x] `glb_gen` erzeugt valide Multi-Node-`.glb`-Dateien mit `"Hull"` und `"Turret"`.
- [x] `audio.rs` und `render_sync.rs` kompilieren und bestehen alle Bevy-Schedule- und Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-mod-tools -p rts-client-3d`

---

## Task 30: `rts-client-3d` â€” HUD & Steuerung fÃ¼r Stances (`[T]`), Patrol (`[Z]`), Guard (`[F]`), Ziel-Abilities, Upgrades, Map-Selector & `.toml`-Kartenexport (`input.rs`, `hud.rs`)

**Description:** VollstÃ¤ndige Integration der neuen Engine-Systeme in Steuerung, HUD und Engine-Studio:
1) **Neue Taktik-Befehle im HUD & per Hotkey**: `[T]` Stance wechseln, `[Z]` Patrouille, `[F]` Eskortieren/Guard, `[G]` Ziel-FÃ¤higkeit/Superwaffe mit Cooldown-Anzeige.
2) **Forschungs-Upgrades in der rechten Sidebar**: Anzeige und Start verfÃ¼gbarer `UpgradeDef`-Forschungen.
3) **Multi-Map-Selector im Studio & `.toml`-Export (`[F11]`) im 3D-Map-Editor**: Speichern bearbeiteter Karten direkt als `.toml`-Datei und Auswahl beliebiger Karten aus `maps/`.

**Acceptance criteria:**
- [x] Alle neuen Hotkeys (`[T]`, `[Z]`, `[F]`, `[F11]`), Sidebar-Upgrades und der Map-Selector funktionieren fehlerfrei.
- [x] Alle Tests und Clippy-PrÃ¼fungen Ã¼ber den gesamten Workspace laufen ohne Warnungen durch.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 31: `rts-mod-sdk` â€” Ballistik-/Projektil-Schema (`DeliveryKind`, `min_range`), Szenario-EntitÃ¤ten (`PreplacedEntityDef`, `TriggerZoneDef`) & Kampagnen-Skript-API (`schema.rs`, `scripting.rs`)

**Description:** Ausbau von `rts-mod-sdk` fÃ¼r echte Projektil-Physik, Szenario-Karten und Kampagnen-Skripting:
1) **Waffen-Ballistik & Mindestreichweite (`schema.rs`)**:
   - `DeliveryKind` (`Instant`, `Ballistic`, `Missile`), `projectile_speed`, `arc_height`, `min_range` und `friendly_fire` in `WeaponDef`.
2) **Szenario-EntitÃ¤ten & Trigger-Zonen in `MapDef` (`schema.rs`)**:
   - `PreplacedEntityDef` (vorplatzierte neutrale/eigene GebÃ¤ude & Einheiten) und `TriggerZoneDef` (Rechteck-Zonen, die beim Betreten `on_trigger_zone` in Rhai/Lua auslÃ¶sen).
3) **Erweiterte Kampagnen- & Wellen-Skript-API (`scripting.rs`)**:
   - `WorldEventContext` unterstÃ¼tzt `spawn_unit`, `spawn_building`, `order_attack_move`, `set_objective` und `declare_winner`.

**Acceptance criteria:**
- [x] `DeliveryKind`, `PreplacedEntityDef`, `TriggerZoneDef` und die erweiterten `WorldEventContext`-Methoden funktionieren in Rhai und Lua.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-mod-sdk`

---

## Task 32: `rts-sim-core` â€” Echte Projektil-Simulation (`SimProjectile`), `SpatialHashGrid`, Dynamische Einheiten-Separation, `PlayerStats` & Trigger-Zonen (`combat.rs`, `world.rs`, `pathfinding.rs`)

**Description:** Ausbau des deterministischen Simulationskerns:
1) **Echte Projektil-Simulation (`SimProjectile`, `step_projectiles`)**:
   - Ballistische Geschosse (`Ballistic`) fliegen mit Parabelbogen zum Zielpunkt (schnelle Einheiten kÃ¶nnen ausweichen); Lenkraketen (`Missile`) verfolgen bewegliche Ziele Ã¼ber Ticks hinweg; `min_range` verhindert Nahbereichs-SchÃ¼sse von Artillerie.
2) **`SpatialHashGrid` & Dynamische Soft-Collision Separation (`step_unit_separation`)**:
   - RÃ¤umlicher Hash-Index fÃ¼r schnelle Nachbarschaftsabfragen und sanftes Auseinanderschieben Ã¼berlappender Einheiten (`collision_radius`), damit Einheiten niemals auf derselben Koordinate stapeln.
3) **Match-Statistiken (`PlayerStats`), Vorplatzierte EntitÃ¤ten & Trigger-Zonen**:
   - Tracking von `units_built`, `units_killed`, `units_lost`, `buildings_destroyed`, `credits_harvested`; Spawnen von `preplaced_entities` und Auswertung von `trigger_zones` inkl. Skript-Spawns.

**Acceptance criteria:**
- [x] Ballistische Projektile, Homing-Missiles, `SpatialHashGrid`, Einheiten-Separation, `PlayerStats` und Trigger-Zonen arbeiten 100% deterministisch.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sim-core`

---

## Task 33: `rts-client-3d` & `rts-net` â€” In-Game 3D-Replay-Player, Observer-Modus, Kamera-Bookmarks & Szenario-Editor (`main.rs`, `input.rs`, `render_sync.rs`, `vfx.rs`)

**Description:** Erweiterung des 3D-Clients um Replay-Wiedergabe, Kamera-Bookmarks und Szenario-Editor:
1) **Live In-Game 3D-Replay-Player & Observer-Modus**:
   - Aufzeichnung nach `saves/last_match.replay.json` und interaktives Abspielen im 3D-Client Ã¼ber das Engine-Studio inkl. Observer-Sicht.
2) **Kamera-Bookmarks (`Alt + 1..4` speichern / abrufen)** zum sofortigen Anspringen wichtiger Kartenpositionen.
3) **Szenario-Editor-Pinsel (`NEUTRAL BUILDING` / `UNIT SPAWN`)** im `[F10]`-Editor inkl. `.toml`-Export (`[F11]`).

**Acceptance criteria:**
- [x] Replays kÃ¶nnen im 3D-Client geladen und abgespielt werden; Kamera-Bookmarks und Szenario-Export funktionieren.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-client-3d`

---

## Task 34: `rts-client-3d` â€” Live-Scoreboard (`[Tab]`), Missionsziel-Anzeige & Post-Game Statistik-Overlay (`hud.rs`)

**Description:** VollstÃ¤ndige HUD-Integration fÃ¼r Match-Statistiken, Replay-Steuerung und Missionsziele:
1) **`[Tab]` Match-Scoreboard & End-of-Match Statistik-Banner**: Zeigt fÃ¼r alle Spieler gebaute/zerstÃ¶rte/verlorene Einheiten & GebÃ¤ude sowie gesammelte Credits.
2) **Missionsziel-Anzeige (`mission_objective`)** und Replay-Steuerung im HUD & Studio.

**Acceptance criteria:**
- [x] Alle Tests und Clippy-Checks Ã¼ber den gesamten Workspace bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 9: `rts-net` Protokoll, Framing, Replay-System & Lockstep-Scheduler

**Description:** Anlegen des Crates `engine/crates/rts-net` mit dem Netzwerkprotokoll (`NetMessage`), lÃ¤ngenprÃ¤fixiertem Stream-Framing (`FrameReader`), dem deterministischen Replay-Recorder/Player (`ReplayFile`) sowie der lokalen `LockstepSchedule` (Tick-Buffering, `input_delay_ticks` und `state_hash`-Desync-Erkennung).

**Acceptance criteria:**
- [x] `NetMessage` unterstÃ¼tzt Handshake (`ClientHello`, `ServerWelcome`, `ServerReject`), Tick-Austausch (`ClientTickCommands`, `ServerTickBundle`) und `DesyncDetected`.
- [x] `ReplayFile` zeichnet ein Match Ã¼ber `N` Ticks auf und verifiziert beim Abspielen exakt denselben finalen `state_hash`.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-net`

**Dependencies:** Task 6

**Files likely touched:**
- `engine/Cargo.toml`
- `engine/crates/rts-net/Cargo.toml`
- `engine/crates/rts-net/src/lib.rs`
- `engine/crates/rts-net/src/protocol.rs`
- `engine/crates/rts-net/src/replay.rs`
- `engine/crates/rts-net/src/lockstep.rs`

**Estimated scope:** Medium (5 files)

---

## Task 10: Non-Blocking TCP `LockstepServer` & `LockstepClient` inkl. 2-Peer-Integrationstest

**Description:** Implementierung des nicht-blockierenden TCP-Servers (`LockstepServer`) und Clients (`LockstepClient`) in `rts-net/src/transport.rs`, inklusive Mod-KompatibilitÃ¤tsprÃ¼fung (`mod_id` + `mod_version`), automatischer `player_id`-Vergabe, ZusammenfÃ¼hrung der Spieler-Commands pro Tick und `state_hash`-PrÃ¼fung zwischen allen verbundenen Spielern.

**Acceptance criteria:**
- [x] Ein Client mit falscher `mod_id` oder `mod_version` erhÃ¤lt `ServerReject`.
- [x] Zwei verbundene `LockstepClient`-Instanzen (Alliierte & Sowjets) fÃ¼hren Ã¼ber einen `LockstepServer` 60 Ticks mit echten Spielbefehlen aus und erreichen exakt denselben `state_hash()` ohne Desync.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-net`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml -p rts-net -- -D warnings`

**Dependencies:** Task 9

**Files likely touched:**
- `engine/crates/rts-net/src/transport.rs`
- `engine/crates/rts-net/src/lib.rs`

**Estimated scope:** Small (2 files)

---

## Task 11: Multiplayer- (`--host` / `--connect`) & Replay-Integration in `rts-client-3d`

**Description:** Einbindung von `rts-net` in `rts-client-3d`: CLI-Parameter `--host <ADDR>`, `--connect <ADDR>`, `--save-replay <FILE>` und `--verify-replay <FILE>`, Routing aller Spieler-Inputs Ã¼ber die aktive `NetRuntimeResource` sowie Live-Anzeige des Netzwerk-/Sync-Status (Modus, verbundene Peers, Input-Delay, Desync-Status) in der Sidebar.

**Acceptance criteria:**
- [x] `rts-client-3d` kann per `--host 127.0.0.1:7878` ein Spiel hosten und ein zweiter Prozess per `--connect 127.0.0.1:7878` als Spieler 2 beitreten.
- [x] Alle lokalen Befehle laufen im Multiplayer-Modus deterministisch Ã¼ber den Lockstep-Tick-Puffer.
- [x] Replays kÃ¶nnen per `--save-replay` gespeichert und per `--verify-replay` verifiziert werden.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 12: Ingenieur-GebÃ¤udeeroberung/-Reparatur, Sieg-Erkennung & deterministische Skirmish-KI in `rts-sim-core`

**Description:** Erweiterung von `rts-sim-core` um:
1) Ingenieur-Logik (`UnitRole::Engineer`): Bewegt sich auf ein ZielgebÃ¤ude zu und erobert feindliche GebÃ¤ude (`player_id`-Wechsel) bzw. repariert eigene GebÃ¤ude auf volle HP, wobei der Ingenieur verbraucht wird.
2) Sieg-/Niederlage-PrÃ¼fung (`winner_player_id` in `SimWorld`).
3) Deterministische Skirmish-KI (`ai.rs`), die Ã¼ber regulÃ¤re `GameCommand`-Befehle Kraftwerke/GebÃ¤ude baut, Einheiten produziert, SpezialfÃ¤higkeiten zÃ¼ndet und in Wellen angreift.

**Acceptance criteria:**
- [x] Ein Ingenieur, der per `AttackTarget` auf ein feindliches GebÃ¤ude geschickt wird, Ã¼bernimmt dieses GebÃ¤ude bei Ankunft fÃ¼r seine Fraktion und verschwindet.
- [x] Die Skirmish-KI erzeugt valide `GameCommand`s fÃ¼r Basisbau, Einheitenproduktion und Angriffe.
- [x] Wenn alle GebÃ¤ude und Einheiten eines Spielers zerstÃ¶rt oder erobert wurden, wird `winner_player_id` gesetzt.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sim-core`

**Dependencies:** Task 6

**Files likely touched:**
- `engine/crates/rts-sim-core/src/ai.rs`
- `engine/crates/rts-sim-core/src/combat.rs`
- `engine/crates/rts-sim-core/src/world.rs`
- `engine/crates/rts-sim-core/src/lib.rs`

**Estimated scope:** Medium (4 files)

---

## Task 13: Erweiterung von `rust-ra2` um Battle Labs, Ingenieure, Prism Tank & Apocalypse Tank

**Description:** Ausbau der `rust-ra2` Total Conversion Mod um das High-Tech-Tier aus Red Alert 2:
- GebÃ¤ude: `allied_battle_lab`, `soviet_battle_lab`
- Waffen: `prism_beam` (Laserstrahl), `apocalypse_twin_120mm` (Doppelkanone)
- Einheiten: `allied_engineer`, `soviet_engineer`, `prism_tank` (mit Rhai-Hook `on_prism_focus`), `apocalypse_tank` (mit Lua-Hook `on_apocalypse_blitz`)
- Generierung der neuen `.glb`-3D-Modelle per `rts-mod-tools generate-models`.

**Acceptance criteria:**
- [x] `rts-mod-tools generate-models --mod-path rust-ra2` erzeugt die `.glb`-Modelle fÃ¼r Battle Labs, Ingenieure, Prism Tank und Apocalypse Tank.
- [x] `rts-mod-tools validate --mod-path rust-ra2` meldet `0 errors`.

**Verification:**
- [x] CLI check: `cargo run --manifest-path engine/Cargo.toml -p rts-mod-tools -- validate --mod-path rust-ra2`

**Dependencies:** Task 12

**Files likely touched:**
- `rust-ra2/weapons/weapons.toml`
- `rust-ra2/buildings/buildings.toml`
- `rust-ra2/units/units.toml`
- `rust-ra2/scripts/allied_rules.rhai`
- `rust-ra2/scripts/soviet_rules.lua`

**Estimated scope:** Medium (5 files)

---

## Task 14: Integration von Skirmish-KI, Ingenieur-Befehlen, Prisma-Strahl & Sieg-Anzeige in `rts-client-3d`

**Description:** Einbindung der Skirmish-KI im 3D-Client (standardmÃ¤ÃŸig fÃ¼r Spieler 2 im Offline-Modus aktiv, umschaltbar per Button/CLI), Rechtsklick mit Ingenieur auf eigene/feindliche GebÃ¤ude, leuchtender Prisma-Laserstrahl in `render_sync.rs` sowie Sieg-/Niederlage-Status in der Sidebar.

**Acceptance criteria:**
- [x] Im Offline-Skirmish agiert Spieler 2 standardmÃ¤ÃŸig als aktiver KI-Gegner (kann per Sidebar-Button ein-/ausgeschaltet werden).
- [x] Rechtsklick mit einem Ingenieur auf ein feindliches oder beschÃ¤digtes eigenes GebÃ¤ude schickt den Ingenieur zum Erobern/Reparieren.
- [x] Alle Tests und Clippy-PrÃ¼fungen Ã¼ber den gesamten Workspace sind grÃ¼n.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 15: Hochdetaillierter `.glb`-3D-Modellgenerator mit Zylindern, SchrÃ¤gpanzerung & Emissions-Materialien

**Description:** Ausbau von `rts-mod-tools/src/glb_gen.rs` von einfachen Quadern auf mehrteilige detaillierte 3D-Meshes mit 4 PBR-Materialien (`PrimaryFaction`, `DarkGunmetal`, `SteelAccent`, `GlowingEmissive`), zylindrischen Kanonenrohren/KÃ¼hltÃ¼rmen/Silos, abgeschrÃ¤gter Frontpanzerung und spezifischen Silhouetten (Doppelrohr fÃ¼r Apocalypse, leuchtender Prisma-Emitter fÃ¼r Prism Tank, Reaktorkuppeln, Radar-SchÃ¼sseln fÃ¼r Battle Labs).

**Acceptance criteria:**
- [x] `glb_gen.rs` unterstÃ¼tzt Zylinder-/Kegelstumpf-/SchrÃ¤g-Geometrien und bis zu 4 glTF-PBR-Materialien inkl. `emissiveFactor`.
- [x] `rts-mod-tools generate-models --mod-path rust-ra2 --force` aktualisiert alle 24 `.glb`-Modelle in `rust-ra2/assets/models/`.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-mod-tools`

**Dependencies:** Task 13

**Files likely touched:**
- `engine/crates/rts-mod-tools/src/glb_gen.rs`
- `rust-ra2/assets/models/*.glb`

**Estimated scope:** Medium

---

## Task 16: Modernes RTS-Simulations-Upgrade in `rts-sim-core` (Unit Separation, Attack-Move, Veteranen-System & DeathEvents)

**Description:** Erweiterung von `rts-sim-core` fÃ¼r ein flÃ¼ssiges, modernes RTS-SpielgefÃ¼hl:
1) Weiche Einheiten-AbstoÃŸung (`apply_unit_separation`), damit Einheiten nicht ineinander stapeln.
2) `GameCommand::AttackMove` und `GameCommand::StopUnits`.
3) Veteranen-RÃ¤nge (`VeterancyRank`: `Rookie`, `Veteran`, `Elite`) durch Kampferfahrung (XP) mit Schadens-/RÃ¼stungs-Bonus und Elite-Selbstheilung.
4) `DestructionEvent` in `SimWorld` (Position, Radius, GebÃ¤udeflag), damit der 3D-Client groÃŸe Explosionen, Druckwellen und Lichtblitze abspielen kann.

**Acceptance criteria:**
- [x] Einheiten halten automatisch Abstand zueinander und Ã¼berlappen nicht auf demselben Punkt.
- [x] `AttackMove` lÃ¤sst Einheiten zum Zielort marschieren und unterwegs automatisch Feinde in Reichweite bekÃ¤mpfen.
- [x] Einheiten steigen durch verursachten Schaden / Kills zu `Veteran` und `Elite` auf.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sim-core`

**Dependencies:** Task 12

**Files likely touched:**
- `engine/crates/rts-sim-core/src/command.rs`
- `engine/crates/rts-sim-core/src/world.rs`
- `engine/crates/rts-sim-core/src/combat.rs`
- `engine/crates/rts-sim-core/src/lib.rs`

**Estimated scope:** Medium (4 files)

---

## Task 17: 3D-Grafik, VFX-Partikelsystem (`vfx.rs`), 144-Hz-Interpolation, Steuergruppen & Live-Radar-Minimap in `rts-client-3d`

**Description:** Massives Grafik-, Effekt- und UX-Upgrade fÃ¼r `rts-client-3d`:
1) Neues Modul `vfx.rs`: Echte fliegende 3D-Projektile, MÃ¼ndungsfeuer, Einschlags-Funken, mehrstufige 3D-Explosionen (Feuerball, expandierender Schockwellen-Ring, TrÃ¼mmer-Partikel mit Schwerkraft, dynamisches `PointLight`), aufsteigender Rauch/Feuer bei beschÃ¤digten GebÃ¤uden sowie nachglÃ¼hende, mehrteilige Tesla-Blitze & Prisma-Laserstrahlen.
2) Verbesserte 3D-Kartenoptik & Beleuchtung (`render_sync.rs`): Schachbrett-/Taktik-Terrainkacheln, geschichtete Klippen, mehrteilige Erzkristall-Cluster, atmosphÃ¤risches Dual-Lighting, sowie flÃ¼ssige Frame-Interpolation (Position & Rotation) zwischen den 20-Hz-Ticks.
3) Moderne RTS-UX (`input.rs` & `hud.rs`): Attack-Move (`[X]` + Klick), Stop (`[H]`), Steuergruppen (`Strg+1..5` / `1..5`), Doppelklick-Typauswahl, Veteranen-Chevrons und Produktions-Fortschrittsanzeigen.

**Acceptance criteria:**
- [x] Kampf erzeugt sichtbare 3D-Projektile, Lichtblitze, Funken, nachglÃ¼hende Laser/Blitze und groÃŸe Explosionen bei ZerstÃ¶rung.
- [x] Einheiten bewegen und drehen sich mit flÃ¼ssiger Frame-Interpolation und zeigen ihren Veteranen-Rang an.
- [x] Die Sidebar enthÃ¤lt Produktions-Fortschrittsanzeigen und moderne RTS-Befehle.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 18: Erweiterte RTS-Befehle in `rts-sim-core` (GebÃ¤ude-Sammelpunkte, Shift-Wegpunkt-Kette & Taktisches Scatter)

**Description:** Ausbau von `rts-sim-core` fÃ¼r prÃ¤zise Einheiten- und Produktionssteuerung:
1) `GameCommand::SetRallyPoint { player_id, building_ids, rally_pos }`: ProduktionsgebÃ¤ude (Kasernen, Waffenfabriken) speichern einen `rally_point: Option<[f32; 3]>`, zu dem neu produzierte Einheiten nach dem Spawnen automatisch marschieren.
2) `GameCommand::ScatterUnits { player_id, entity_ids }`: Klassischer C&C/RA2-Scatter-Befehl (`[V]`), der ausgewÃ¤hlte Einheiten sofort sternfÃ¶rmig vom Gruppenzentrum wegrÃ¼cken lÃ¤sst, um FlÃ¤chen-/Artilleriefeuer auszuweichen.
3) `Shift`-Wegpunkt-Verkettung (`queue_waypoints: bool` oder `GameCommand::QueueMoveUnits`), damit Spieler mit `Shift + Rechtsklick` mehrteilige Marschrouten abstecken kÃ¶nnen.

**Acceptance criteria:**
- [x] ProduktionsgebÃ¤ude mit gesetztem `rally_point` schicken neu fertiggestellte Einheiten automatisch zum Sammelpunkt.
- [x] `ScatterUnits` verteilt ausgewÃ¤hlte Einheiten deterministisch auf freie Nachbarkacheln.
- [x] `QueueMoveUnits` hÃ¤ngt einen neuen A*-Pfad an das Ende bestehender `path_waypoints` an, statt sie zu Ã¼berschreiben.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sim-core`

**Dependencies:** Task 16

**Files likely touched:**
- `engine/crates/rts-sim-core/src/command.rs`
- `engine/crates/rts-sim-core/src/world.rs`
- `engine/crates/rts-sim-core/src/production.rs`
- `engine/crates/rts-sim-core/src/lib.rs`

**Estimated scope:** Medium (4 files)

---

## Task 19: Moderne RTS-Kamerasteuerung (`camera.rs`) & Profi-Eingabesteuerung (`input.rs`)

**Description:** Ausbau von `camera.rs` und `input.rs` fÃ¼r ein erstklassiges RTS-Control-GefÃ¼hl:
1) **GedÃ¤mpftes Kamera-Rig (`RtsCameraRig`)**: Weiche Interpolation von `target_focus`, `target_distance`, `target_yaw_radians` und `target_pitch_radians`.
2) **Maus-Kanten-Scrolling & Mittlere Maustaste (Drag-Pan / `Alt+MMB`-Orbit)**: Kamera fÃ¤hrt am Bildschirmrand (abschaltbar/deaktiviert im Studio-Modal) oder per gedrÃ¼cktem Mausrad flÃ¼ssig Ã¼ber die Karte; `[Home]`/`[Pos1]` zentriert die eigene Basis, `[C]` oder Doppel-Tippen auf `[1..5]` zentriert die ausgewÃ¤hlte Truppe.
3) **Shift-Auswahl, Strg+Klick-Typauswahl, Shift-Wegpunkte & Fabrik-Sammelpunkte**:
   - `Shift + Linksklick / Auswahlrahmen` fÃ¼gt Einheiten zur bestehenden Auswahl hinzu.
   - `Strg + Linksklick` (oder Doppelklick) auf eine Einheit wÃ¤hlt alle eigenen Einheiten desselben Typs aus.
   - `Rechtsklick` bei ausgewÃ¤hltem ProduktionsgebÃ¤ude setzt dessen `SetRallyPoint`.
   - `Shift + Rechtsklick` reiht Wegpunkte in die Marschroute ein (`QueueMoveUnits`).
   - `[V]` lÃ¶st `ScatterUnits` aus.

**Acceptance criteria:**
- [x] Kamera unterstÃ¼tzt gedÃ¤mpftes Zoomen/Schwenken, MMB-Drag-Panning, Mauskanten-Scrollen und Basis-/Truppen-Zentrierung (`[Home]`, `[C]`).
- [x] `Shift`-Auswahl, `Shift`-Wegpunkte, `[V]` Scatter und Fabrik-Sammelpunkte funktionieren nahtlos.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-client-3d`

**Dependencies:** Task 18

**Files likely touched:**
- `engine/crates/rts-client-3d/src/camera.rs`
- `engine/crates/rts-client-3d/src/input.rs`

**Estimated scope:** Medium (2 files)

---

## Task 20: Interaktive 2D-Radar-Minimap, Taktische Befehlsleiste (`hud.rs`) & Erweitertes 3D-Rendering (`render_sync.rs`, `vfx.rs`)

**Description:** Ausbau von UI und 3D-Rendering in `rts-client-3d`:
1) **Interaktive Echtzeit-Radar-Minimap (`hud.rs`)**: Dynamisch gerenderte 2D-Minimap-Textur (`180x180` Pixel) oben in der rechten Sidebar mit Terrain, Klippen, leuchtenden Erzadern, Alliierten/Sowjet-Einheiten & GebÃ¤uden sowie dem aktuellen weiÃŸen Kamera-Sichtfeld-Rahmen. Linksklick auf das Radar bewegt die 3D-Kamera dorthin; Rechtsklick schickt ausgewÃ¤hlte Einheiten dorthin.
2) **Visuelle Produktions-Fortschrittsleiste, Kosten-VerfÃ¼gbarkeit & Taktische Aktions-Buttons (`hud.rs`)**: Animierter Fortschrittsbalken fÃ¼r aktive Bauschleifen, optische Kennzeichnung nicht bezahlbarer Bauoptionen sowie klickbare Taktik-Buttons (`Attack-Move [X]`, `Stop [H]`, `Scatter [V]`, `Ability [G]`, `Center [C]`) und Steuergruppen-Anzeige (`[1]..[5]`).
3) **Erweitertes 3D-Rendering (`render_sync.rs`, `vfx.rs`, `main.rs`)**:
   - AtmosphÃ¤rischer `DistanceFog` und abgestimmte Horizon-ClearColor in `main.rs`.
   - 3D-Waffenreichweiten-Kreise fÃ¼r ausgewÃ¤hlte Kampfeinheiten/VerteidigungstÃ¼rme, lokales 3D-Baugitter (`6x6` Kacheln grÃ¼n/rot) beim Platzieren von GebÃ¤uden sowie Sammelpunkt-Linie & 3D-Diamant-Marker fÃ¼r ProduktionsgebÃ¤ude in `render_sync.rs`.
   - 3D-Befehls-BestÃ¤tigungsringe (`CommandMarkerVfx`) bei Bewegungs-/Angriffsbefehlen und bleibende Boden-Explosionskrater (`ScorchDecalVfx`) in `vfx.rs`.

**Acceptance criteria:**
- [x] Die interaktive Radar-Minimap zeigt Karte, Erz, Einheiten und Kamerarahmen live an und reagiert auf Links-/Rechtsklicks.
- [x] 3D-Waffenreichweiten-Kreise, 3D-Baugitter, Sammelpunkt-Gizmos, 3D-Klickringe und Explosionskrater werden im 3D-Viewport gerendert.
- [x] Alle Tests und Clippy-Checks im gesamten Workspace laufen ohne Warnungen durch.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 21: KI-Schwierigkeitsgrade (`Easy`, `Medium`, `Hard`, `Brutal`), Optionaler Fog of War (`TileVisibility`) & GebÃ¤ude-Reparatur/Verkauf in `rts-sim-core`

**Description:** Ausbau von `rts-sim-core` um:
1) **4 KI-Schwierigkeitsgrade (`AiDifficulty::Easy`, `Medium`, `Hard`, `Brutal`) in `ai.rs`**:
   - `Easy`: Langsameres Bautempo (alle 40 Ticks), 70s Aufbauphase, 800 Credits Reserve, kleine Angriffswellen ohne SpezialfÃ¤higkeiten.
   - `Medium`: Ausgewogenes Standardverhalten (alle 20 Ticks, 45s Aufbauphase, 400 Credits Reserve, Combined-Arms-Wellen alle 25s).
   - `Hard`: Schnelle Reaktion (alle 10 Ticks, 30s Aufbauphase, 200 Credits Reserve), gezielter Tech-Up auf Battle Lab (Prism/Apocalypse), zweiter Sammler, koordinierte `AttackMove`-Wellen alle 18s mit Fokus auf Wirtschaft/ProduktionsgebÃ¤ude.
   - `Brutal`: HÃ¶chste AggressivitÃ¤t (alle 10 Ticks, 20s Aufbauphase, Wirtschafts-Effizienzbonus, Doppel-Produktion bei hohen Credits, Fokus-Feuer auf verwundete Ziele/Sammler, Wellen alle 13s).
2) **Berechnung des Fog of War (`TileVisibility::Unexplored`, `Explored`, `Visible`) in `world.rs`**:
   - Speichert pro Spieler die bereits erkundeten Kacheln (`explored_tiles: BTreeMap<u8, Vec<bool>>`) und berechnet live die aktuell sichtbaren Kacheln (`compute_player_visibility`) anhand der `vision_range` aller eigenen Einheiten und GebÃ¤ude.
3) **Klassische C&C/RA2 Basis-Interaktionen (`GameCommand::SellBuilding` & `GameCommand::ToggleRepairBuilding`)**:
   - `SellBuilding`: Verkauft eigene GebÃ¤ude fÃ¼r 50 % KostenrÃ¼ckerstattung und lÃ¶st ein `DestructionEvent` (Deconstruction) aus.
   - `ToggleRepairBuilding`: Schaltet die laufende Reparatur (`is_repairing: bool` auf `SimEntity`) an beschÃ¤digten GebÃ¤uden gegen geringe Credit-Kosten pro Tick ein/aus.

**Acceptance criteria:**
- [x] Alle 4 `AiDifficulty`-Stufen arbeiten deterministisch und skalieren Bautempo, Tech-Fokus und Wellen-Frequenz.
- [x] `SimWorld::tile_visibility(player_id, tile)` unterscheidet sauber zwischen `Unexplored`, `Explored` und `Visible`.
- [x] `SellBuilding` erstattet 50 % der Baukosten; `ToggleRepairBuilding` repariert beschÃ¤digte GebÃ¤ude schrittweise.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sim-core`

**Dependencies:** Task 18

**Files likely touched:**
- `engine/crates/rts-sim-core/src/ai.rs`
- `engine/crates/rts-sim-core/src/command.rs`
- `engine/crates/rts-sim-core/src/world.rs`
- `engine/crates/rts-sim-core/src/lib.rs`

**Estimated scope:** Medium (4 files)

---

## Task 22: Optionaler 3D- & Radar-Fog-of-War, Bau-Aufstiegsanimation, Staub-/Erz-Partikel & Kamera-Shake in `rts-client-3d`

**Description:** Visuelles Effekt- und Fog-of-War-Upgrade in `render_sync.rs`, `vfx.rs` und `camera.rs`:
1) **Optionaler Fog of War (`[F5]` oder Top-Bar-Button)**:
   - Dynamische 3D-Fog-of-War-Overlay-Textur (`FogOfWarOverlayImage` auf einer transparenten 3D-Ebene Ã¼ber dem Terrain): Unentdeckte Gebiete sind tiefschwarz (`Unexplored`), erkundete Gebiete auÃŸerhalb der Sichtweite abgedunkelt (`Explored`), aktiver Sichtbereich glasklar (`Visible`).
   - Feindliche mobile Einheiten auÃŸerhalb des aktiven Sichtbereichs werden bei aktiviertem Fog of War im 3D-Viewport und auf der Radar-Minimap ausgeblendet.
2) **Erweiterte 3D-Effekte (`vfx.rs` & `render_sync.rs`)**:
   - **3D-GebÃ¤ude-Bauanimation (`spawn_rise_progress`)**: Neu platzierte GebÃ¤ude fahren flÃ¼ssig aus dem Boden hoch (`scale.y: 0.05 -> 1.0`), begleitet von einem goldenen 3D-BaugerÃ¼st-Rahmen und SchweiÃŸfunken.
   - **Fahrzeug-Staubspuren & Sammler-Erzfunken**: Fahrende Fahrzeuge erzeugen Staubwolken an den Ketten; erntende Sammler schlagen goldene Funken aus den Erzkristallen; Einheiten-BefÃ¶rderungen (`Veteran`/`Elite`) und SpezialfÃ¤higkeiten (`[G]`) zÃ¼nden leuchtende 3D-Energieringe.
3) **Kamera-Shake (`trauma` in `RtsCameraRig`)**:
   - Explosionen und schwere GeschÃ¼tze erzeugen ein kurzes, entfernungsabhÃ¤ngiges Kamera-Beben.

**Acceptance criteria:**
- [x] Fog of War lÃ¤sst sich jederzeit per `[F5]` oder UI-Button live ein- und ausschalten (wirkt auf 3D-Welt, Einheiten-Sichtbarkeit und Radar-Minimap).
- [x] Neue GebÃ¤ude wachsen animiert aus dem Boden; Fahrzeuge, Sammler und BefÃ¶rderungen erzeugen sichtbare 3D-Partikeleffekte; Explosionen lÃ¶sen Kamera-Shake aus.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-client-3d`

**Dependencies:** Task 21

**Files likely touched:**
- `engine/crates/rts-client-3d/src/camera.rs`
- `engine/crates/rts-client-3d/src/render_sync.rs`
- `engine/crates/rts-client-3d/src/vfx.rs`

**Estimated scope:** Large (3 files)

---

## Task 23: SpielgefÃ¼hl-Upgrade im HUD (`hud.rs`, `input.rs`, `main.rs`): KI-Stufenwahl, Spielgeschwindigkeit/Pause, EVA-Gefechtsticker & Reparatur/Verkauf

**Description:** Abrundung des modernen RTS-SpielgefÃ¼hls in Steuerung und UI:
1) **KI-Schwierigkeitswahl (`[F4]` / Top-Bar & Studio-Modal)**: Zyklisches Umschalten zwischen `AI: EASY`, `AI: MEDIUM`, `AI: HARD`, `AI: BRUTAL` und `AI: OFF`.
2) **Spielgeschwindigkeit & Taktische Pause (`[P]` Pause, `[F6]` Speed `0.5x / 1.0x / 1.5x / 2.0x`)**: Volle Kontrolle Ã¼ber das Offline-Simulationstempo, inklusive Befehlsvergabe wÃ¤hrend der taktischen Pause.
3) **GebÃ¤ude-Reparatur (`[R]`) & GebÃ¤ude-Verkauf (`[Delete]` / UI-Buttons)**: Direkt per Hotkey oder Ã¼ber die Taktik-Toolbar auslÃ¶sbar.
4) **Live EVA-Gefechts-Ticker (Battle Feed)**: Kompaktes Status-Feed-Banner fÃ¼r wichtige Ereignisse (Basis wird angegriffen, Einheit auf Veteran/Elite befÃ¶rdert, Low-Power-Warnung, GebÃ¤ude erobert/verkauft).

**Acceptance criteria:**
- [x] KI-Schwierigkeitsgrad, Fog of War (`[F5]`), Spielgeschwindigkeit (`[F6]`) und Taktische Pause (`[P]`) sind per Hotkey und UI-Button steuerbar.
- [x] GebÃ¤ude kÃ¶nnen per `[R]` repariert und per `[Del]` verkauft werden.
- [x] Alle Tests und Clippy-PrÃ¼fungen im gesamten Workspace laufen ohne Warnungen durch.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 24: `rts-mod-sdk` & `rts-sim-core` â€” Transporter & Garnisonen (`cargo_capacity`, `garrison_capacity`), Bauradius, Queue-Abbruch, HÃ¶henterrain (`elevation`) & GeschÃ¼tzturm-Zielausrichtung + Savegames

**Description:** Ausbau von `rts-mod-sdk`, `rts-sim-core` und `rust-ra2` um alle Kernmechaniken einer vollwertigen RTS-Engine:
1) **Transporter & GebÃ¤ude-Garnisonen (`cargo_capacity`, `garrison_capacity`, `LoadIntoCarrier`, `UnloadCarrier`)**:
   - Infanterie kann in Transporter oder Bunker/GarnisonsgebÃ¤ude einsteigen (`passenger_ids`, `transported_by`), ist darin geschÃ¼tzt, verstÃ¤rkt die Feuerkraft besetzter GebÃ¤ude (+25 % Schaden pro Infanterist) und kann per `[U]` (`UnloadCarrier`) wieder abgesetzt werden.
2) **UnabhÃ¤ngige GeschÃ¼tzturm-Rotation (`turret_facing_radians`) & RÃ¼ckstoÃŸ (`recoil_timer`)**:
   - Fahrzeuge und GeschÃ¼tztÃ¼rme richten ihren Turm unabhÃ¤ngig von der Fahrtrichtung (`facing_radians`) auf das aktuelle Ziel aus und lÃ¶sen beim Schuss einen RÃ¼ckstoÃŸ-Timer aus.
3) **3D-HÃ¶henterrain (`elevation_rects` / `elevation_tiles`) & High-Ground-Kampfbonus**:
   - Karten unterstÃ¼tzen erhÃ¶hte Plateaus (`y = 1.35`); Einheiten und GebÃ¤ude auf AnhÃ¶hen erhalten +20 % Waffenreichweite, +15 % Schaden und +25 % Sichtweite gegen tiefer gelegene Ziele.
4) **C&C-Bauradius (`is_within_build_radius`) & Einheiten-Queue-Abbruch (`CancelProduction`)**:
   - Neue GebÃ¤ude mÃ¼ssen innerhalb des Bauradius eigener GebÃ¤ude platziert werden; Einheiten in der Bauschleife kÃ¶nnen per `CancelProduction` gegen 100 % Credit-RÃ¼ckerstattung abgebrochen werden.
5) **Savegame-Serialisierung (`SimWorld::save_to_file` / `SimWorld::load_from_file`)**:
   - Komplette JSON-Serialisierung und Wiederherstellung des laufenden Matches.

**Acceptance criteria:**
- [x] Infanterie kann in Transporter/Bunker eingeladen (`LoadIntoCarrier`), mitbewegt und per `UnloadCarrier` wieder ausgeladen werden; besetzte Bunker erhalten Feuerkraft-Bonus.
- [x] `turret_facing_radians` zielt unabhÃ¤ngig von `facing_radians`; Einheiten auf `elevation_tiles` stehen auf `y = 1.35` und erhalten High-Ground-Bonus.
- [x] `CancelProduction` erstattet 100 % der Einheitenkosten; `SimWorld` lÃ¤sst sich verlustfrei speichern und laden.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sim-core`

---

## Task 25: `rts-client-3d` â€” Animierte 3D-GeschÃ¼tztÃ¼rme, Radar-SchÃ¼sseln, Flugzeug-Banking, 3D-HÃ¶henplateaus & Bauradius-Vorschau (`render_sync.rs`)

**Description:** Visuelle 3D-Darstellung der neuen Engine-Systeme in `render_sync.rs`:
1) **Artikulierte 3D-GeschÃ¼tztÃ¼rme & Kanonenrohr-RÃ¼ckstoÃŸ**: Zeichnet und animiert die unabhÃ¤ngige Turm-Ausrichtung (`turret_facing_radians`) inkl. RÃ¼ckstoÃŸ-Versatz (`recoil_timer`) sowie rotierende Radar-Antennen auf Tech-Zentren und Kurvenneigung (Roll/Pitch) fÃ¼r Flugzeuge.
2) **3D-HÃ¶henplateaus & Rampen**: Rendert erhÃ¶hte Terrain-Plateaus (`elevation_tiles`) mit abgeschrÃ¤gten Auffahrtsrampen im 3D-Viewport.
3) **Garnisons- & Passagier-Pips (`[â– â– â–¡â–¡]`) + Bauradius-Kreis**: Zeigt Ã¼ber Transportern und besetzten Bunkern grÃ¼ne Passagier-Indikatoren sowie beim GebÃ¤udebau den zulÃ¤ssigen Bauradius an.

**Acceptance criteria:**
- [x] GeschÃ¼tztÃ¼rme zielen sichtbar auf Feinde; HÃ¶henplateaus und Passagier-Pips werden in 3D dargestellt.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-client-3d`

---

## Task 26: `rts-client-3d` â€” In-Game 3D-Map-Editor (`[F10]`), Quick-Save/Load (`[F7]`/`[F8]`), Rechtsklick-Queue-Abbruch & Prozedurales Combat-Audio (`audio.rs`, `input.rs`, `hud.rs`)

**Description:** VollstÃ¤ndige Bedienbarkeit aller neuen Engine-Features im Client:
1) **Transporter/Bunker beladen per Rechtsklick & Entladen per `[U]` (`UNLOAD [U]`)**.
2) **Rechtsklick auf Einheiten-Buttons in der Sidebar** bricht die Produktion ab (`CancelProduction` mit Credit-RÃ¼ckerstattung).
3) **Quick-Save (`[F7]`) & Quick-Load (`[F8]`)** speichert/lÃ¤dt das laufende Match nach `saves/quicksave.json`.
4) **Live In-Game 3D-Map-Editor (`[F10]` / Toolbar)**: Erlaubt das direkte Malen von Erzfeldern, Wasser, Felsklippen und HÃ¶henplateaus im 3D-Viewport inkl. Speichern der Karte.
5) **Prozedurales Sound-/Audio-System (`audio.rs`, `[F9]` Sound Toggle)**: Erzeugt kompakte `.wav`-Klangprofile fÃ¼r SchÃ¼sse, Explosionen, Bauabschluss und UI-Feedback.

**Acceptance criteria:**
- [x] Alle neuen Hotkeys (`[U]`, `[F7]`, `[F8]`, `[F9]`, `[F10]`) und UI-Buttons funktionieren fehlerfrei.
- [x] Alle Tests und Clippy-Checks Ã¼ber den gesamten Workspace bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 35: `rts-mod-sdk` â€” `BuildingRole`-Erweiterung (`Wall`, `Gate`, `Bridge`, `Airfield`, `ServiceDepot`), Flugzeug-Munition (`max_ammo`), Erz-Regeneration (`ore_regen_per_sec`) & Kisten-Schema (`schema.rs`)

**Description:** Erweiterung des datengetriebenen Schemas in `rts-mod-sdk/src/schema.rs`:
1) **Neue universelle GebÃ¤ude-Rollen (`BuildingRole`)**:
   - `Wall` (1x1 passive Mauer), `Gate` (automatisches Tor fÃ¼r eigene/alliierte Einheiten), `Bridge` (zerstÃ¶rbare/reparierbare BrÃ¼cke Ã¼ber Wasser), `Airfield` (Flugplatz zum Auftanken/Nachladen von Luftfahrzeugen), `ServiceDepot` (Reparaturdock fÃ¼r Fahrzeuge).
2) **Flugzeug-Munition (`max_ammo: u32`)** auf `UnitDef` (`0` = unbegrenzt, `> 0` = Munitionspool mit automatischer RÃ¼ckkehr zum `Airfield`).
3) **Ressourcen-Nachwachsen (`ore_regen_per_sec: f32`) & BrÃ¼cken-Kacheln (`bridge_rects: Vec<TileRectDef>`)** in `MapDef`.

**Acceptance criteria:**
- [x] Alle neuen `BuildingRole`-Varianten, `max_ammo`, `ore_regen_per_sec` und `bridge_rects` kompilieren und serialisieren rÃ¼ckwÃ¤rtskompatibel mit `#[serde(default)]`.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-mod-sdk`

---

## Task 36: `rts-sim-core` â€” Multi-Slot Skirmish, `AiPersonality`, `GameMode`, Mauern/Tore/BrÃ¼cken, Erz-Regeneration, Bonus-Kisten (`SimCrate`), Airfield-Reload, ServiceDepot & Multi-Fabrik-Skalierung (`ai.rs`, `pathfinding.rs`, `production.rs`, `combat.rs`, `world.rs`, `command.rs`)

**Description:** Ausbau des deterministischen Simulationskerns um die 6 verbleibenden Base-Engine-SÃ¤ulen:
1) **Multi-Slot Skirmish (2..8 Spieler & Team-Victory), `AiPersonality` (`Balanced`, `Rusher`, `Turtle`, `AirForce`) & `GameMode` (`Standard`, `ShortGame`, `KingOfTheHill`)**:
   - `evaluate_victory_condition` berÃ¼cksichtigt `team_id` (Allianzen gewinnen gemeinsam) sowie `GameMode::ShortGame` (nur GebÃ¤ude zÃ¤hlen) und `GameMode::KingOfTheHill` (Kontrollzeit im Kartenzentrum).
   - `SkirmishAi` unterstÃ¼tzt unterschiedliche `AiPersonality`-Profile.
2) **Mauern (`Wall`), Tore (`Gate`) & BrÃ¼cken (`Bridge`) in `CollisionGrid` & `SimWorld`**:
   - `Gate`-Kacheln sind fÃ¼r eigene und alliierte Bodeneinheiten passierbar, blockieren aber Feinde.
   - Intakte `Bridge`-Kacheln machen darunterliegende `water_tiles` fÃ¼r Bodeneinheiten passierbar; wird eine BrÃ¼cke zerstÃ¶rt, versinken Bodeneinheiten auf der BrÃ¼cke im Wasser.
3) **Ressourcen-Regeneration (`step_ore_regeneration`) & Sammelbare Bonus-Kisten (`SimCrate`, `CrateKind`)**:
   - Periodisches Nachwachsen von Erz auf bestehenden/benachbarten Erz-Kacheln.
   - `SimCrate` (`BonusCredits`, `HealAll`, `VeterancyPromo`, `FirepowerBoost`, `SpeedBoost`, `RevealMap`) wird beim Ãœberfahren durch eine Einheit eingesammelt.
4) **Flugzeug-Munition & `Airfield`-Rearming sowie `ServiceDepot`-Auto-Repair**:
   - Einheiten mit `max_ammo > 0` verbrauchen beim Feuern Munition (`current_ammo`), fliegen bei `0` Munition automatisch zum nÃ¤chsten eigenen `Airfield` und laden dort nach.
   - Eigene `ServiceDepot`-GebÃ¤ude reparieren beschÃ¤digte Fahrzeuge im Umkreis automatisch.
5) **Multi-Fabrik-Produktionsbonus & PrimÃ¤rgebÃ¤ude (`is_primary`, `GameCommand::SetPrimaryBuilding`) + Taktische Pings (`GameCommand::PingLocation`)**:
   - Jede zusÃ¤tzliche Fabrik derselben Rolle erhÃ¶ht das Produktionstempo um `+25 %` (bis zu `+100 %`); neue Einheiten spawnen am als `is_primary` markierten ProduktionsgebÃ¤ude.

**Acceptance criteria:**
- [x] Alle 6 Systeme (`GameMode`, `AiPersonality`, `Gate`/`Bridge`, `SimCrate` + Erz-Regen, `Airfield`/`ServiceDepot`, Multi-Fabrik + `is_primary`) sind deterministisch in `rts-sim-core` implementiert und durch Unit-Tests abgesichert.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sim-core`

---

## Task 37: `rts-client-3d` â€” 3D-Wegpunkt-/Rally-Linien, Verbundene Mauern/Tore/BrÃ¼cken, Bonus-Kisten-3D-Meshes, Taktische Pings & Dynamische Wetter-/Tageszeit-AtmosphÃ¤re (`render_sync.rs`, `vfx.rs`, `main.rs`)

**Description:** Visuelle 3D-Synchronisation und AtmosphÃ¤re im Client:
1) **3D-Wegpunkt-Kettenlinien & Rally-Point-Linien**:
   - Rendert verbundene 3D-Linien fÃ¼r die `path_waypoints` / Patrouillenrouten ausgewÃ¤hlter Einheiten sowie vom ProduktionsgebÃ¤ude zum `rally_point`, plus ein `PRIMARY`-Badge Ã¼ber PrimÃ¤rfabriken und Munitions-Pips fÃ¼r Flugzeuge.
2) **3D-Darstellung von Bonus-Kisten (`SimCrate`), BrÃ¼cken (`bridge_tiles`), Toren (`Gate`) & Taktischen Pings (`ActivePing`)**:
   - Rotierende goldene/cyanfarbene 3D-Kisten auf der Karte, befahrbare 3D-BrÃ¼ckenplanken Ã¼ber Wasser und pulsierende 3D-Ping-Leuchtfeuer.
3) **Dynamische Wetter-/Tageszeit-Beleuchtung (`EnvironmentPreset`: `Day`, `Dusk`, `Night`, `IonStorm`)**:
   - Passt `DirectionalLight`, `AmbientLight` und `ClearColor` live an die gewÃ¤hlte Tageszeit bzw. laufende Superwaffen-StÃ¼rme an.
4) **Multi-Slot KI-Loop in `step_fixed_simulation`**: Tickt KI-Controller fÃ¼r alle aktiven Nicht-Spieler-Slots auf der Karte.

**Acceptance criteria:**
- [x] 3D-Wegpunktlinien, Rally-Linien, Kisten, BrÃ¼cken, Pings und Beleuchtungs-Presets werden fehlerfrei im 3D-Scene-Graph aktualisiert.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-client-3d`

---

## Task 38: `rts-client-3d` â€” HUD & Steuerung fÃ¼r PrimÃ¤rgebÃ¤ude (`[B]`), Taktische Pings (`Alt + Rechtsklick`), Spielmodus-/KI-PersÃ¶nlichkeits-/Wetter-Wahl & BrÃ¼cken-Editor-Pinsel (`input.rs`, `hud.rs`)

**Description:** VollstÃ¤ndige Bedienbarkeit aller neuen Meilenstein-10-Features in Steuerung, HUD und Engine-Studio:
1) **Hotkeys & Mausbefehle**:
   - `[B]` setzt das ausgewÃ¤hlte ProduktionsgebÃ¤ude als `PRIMARY` (`SetPrimaryBuilding`).
   - `Alt + Rechtsklick` (oder Ping-Button) sendet einen taktischen `PingLocation`-Befehl (sichtbar auf Minimap und in 3D).
   - `[L]` schaltet das `EnvironmentPreset` (`DAY` -> `DUSK` -> `NIGHT` -> `ION STORM`) um.
2) **HUD & Engine-Studio-Erweiterungen**:
   - Auswahl von `GameMode` (`STANDARD`, `SHORT GAME`, `KING OF THE HILL`), `AiPersonality` (`BALANCED`, `RUSHER`, `TURTLE`, `AIR FORCE`) und `EnvironmentPreset` im Studio/HUD.
   - Neuer Pinsel `BRIDGE` und `CRATE` im `[F10]`-Map-Editor zum Platzieren von BrÃ¼cken Ã¼ber Wasser und Bonus-Kisten.

**Acceptance criteria:**
- [x] Alle neuen Hotkeys (`[B]`, `[L]`, `Alt + Rechtsklick`), Studio-Optionen und der BrÃ¼cken-Editor funktionieren fehlerfrei.
- [x] Alle Tests und Clippy-Checks Ã¼ber den gesamten Workspace bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 55: `rts-mod-sdk` & `rust-ra2` â€” Spion-Infiltration (`infiltrates_buildings`), C4-Sprengladungen (`plants_c4`, `c4_demolition`) & EMP-Schockwellen (`emp_duration_secs`, `SupportPowerKind::EmpPulse`) (`schema.rs`)

**Description:** Erweiterung des Mod-SDK-Schemas fÃ¼r Meilenstein 15:
1) **Spion-Infiltration & Kommando-C4 (`schema.rs`)**:
   - `infiltrates_buildings: bool` und `plants_c4: bool` auf `UnitDef` (mit `#[serde(default)]`).
   - `c4_demolition: bool` und `emp_duration_secs: f32` auf `WeaponDef` (mit `#[serde(default)]`).
   - `SupportPowerKind::EmpPulse` als neuer Support-Power-Typ.
2) **Referenz-Mod (`rust-ra2`)**:
   - `allied_spy` (`infiltrates_buildings = true`, `cloaked = true`), `navy_seal` / `tanya` (`plants_c4 = true`, `c4_demolition = true`), sowie `soviet_emp_pulse` (`emp_pulse`) auf `soviet_battle_lab`.

**Acceptance criteria:**
- [x] `UnitDef`, `WeaponDef` und `SupportPowerKind` unterstÃ¼tzen Infiltration, C4 und EMP abwÃ¤rtskompatibel.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-mod-sdk`

---

## Task 56: `rts-sim-core` â€” Spion-Sabotage (`InfiltrateBuilding`), Zeitgesteuerte C4-Sprengung (`c4_timer`), Richtungspanzerung (Front/Flanke/Heck), Taktischer RÃ¼ckwÃ¤rtsgang (`ReverseMove`), Gefechtsformationen (`FormationKind`) & EMP-Ausfall (`command.rs`, `world.rs`, `combat.rs`, `production.rs`)

**Description:** Ausbau des deterministischen Simulationskerns (`rts-sim-core`):
1) **Spion-Infiltration & Sabotage (`GameCommand::InfiltrateBuilding`)**:
   - Infiltration feindlicher Raffinerien (stiehlt 50 % Credits), Kraftwerke (`power_sabotage_timer = 30.0` -> sofortiger Stromausfall) und Tech-/FabrikgebÃ¤ude (`veteran_production_unlocked = true` + Cooldown-Reset).
2) **Zeitgesteuerte C4-Sprengladungen (`c4_timer: f32`) & Ingenieur-EntschÃ¤rfung**:
   - Kommandosoldaten bringen eine `3.5s` C4-Ladung an GebÃ¤uden/Fahrzeugen an; eigene Ingenieure/Service-Depots kÃ¶nnen sie entschÃ¤rfen, andernfalls detoniert sie mit `1500.0` Schaden.
3) **RichtungsabhÃ¤ngige Fahrzeugpanzerung, Taktischer RÃ¼ckwÃ¤rtsgang (`GameCommand::ReverseMove`) & EMP (`emp_disable_timer`)**:
   - Treffer in die Flanke (`1.15x`) oder das Heck (`1.35x`) von Fahrzeugen richten mehr Schaden an als auf die Frontpanzerung (`1.0x`).
   - `ReverseMove` bewegt Fahrzeuge rÃ¼ckwÃ¤rts, wÃ¤hrend Frontpanzerung und GeschÃ¼tzturm zum Feind zeigen.
   - EMP legt Fahrzeuge und VerteidigungstÃ¼rme temporÃ¤r lahm und bringt fliegende Lufteinheiten zum Absturz.
4) **WÃ¤hlbare Gefechtsformationen (`FormationKind`: `Box`, `Line`, `Wedge`, `Column`, `GameCommand::SetFormation`) & APM-/Armeewert-Tracking**:
   - Richtet Gruppenbewegungen automatisch in SchÃ¼tzenlinie, Keil, Karree oder Marschkolonne aus und berechnet `player_army_value` sowie `commands_issued`.

**Acceptance criteria:**
- [x] Spion-Infiltration, C4-Sprengung, Richtungspanzerung, RÃ¼ckwÃ¤rtsgang, Formationen und EMP arbeiten deterministisch und bestehen alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sim-core`

---

## Task 57: `rts-client-3d` â€” Echtes `.glb`-3D-Ghost-Modell im Baumodus, Dynamische 3D-PBR-Nachtbeleuchtung (`PointLight`) sowie 3D-C4-Countdown- & EMP-Lichtbogen-Gizmos (`render_sync.rs`)

**Description:** Ausbau der 3D-Grafik- und Effekt-Engine in `render_sync.rs`:
1) **Echte `.glb`-3D-Modell-Bauvorschau (`PlacementGhostModel`)**:
   - Spawnt und aktualisiert im Baumodus das tatsÃ¤chliche `.glb`-3D-Modell des gewÃ¤hlten GebÃ¤udes am Mauszeiger (inkl. 90Â°-Drehung bei `[R]`).
2) **Dynamische 3D-PBR-Lichter (`PointLight`) bei DÃ¤mmerung, Nacht & Ionensturm**:
   - Beleuchtet GebÃ¤ude in `Dusk`, `Night` und `IonStorm` mit echten 3D-`PointLight`-Quellen.
3) **3D-C4-Countdown-Beacon & EMP-StÃ¶rlichtbÃ¶gen**:
   - Blinkender roter 3D-Countdown-Ring/Sphere bei aktivem `c4_timer > 0.0` und elektrische blaue LichtbÃ¶gen bei `emp_disable_timer > 0.0`.

**Acceptance criteria:**
- [x] `.glb`-Bauvorschau, Nacht-PBR-Lichter und C4-/EMP-Gizmos kompilieren fehlerfrei und laufen ohne Bevy-ECS-Konflikte.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-client-3d`

---

## Task 58: `rts-client-3d` â€” Sandbox-Einheiten-Spawner im `[F10]`-Editor, Steuerung fÃ¼r Infiltration, RÃ¼ckwÃ¤rtsgang (`[K]`) & Formationen (`[J]`) sowie Live-APM & Army-Value im `[Tab]`-Scoreboard (`input.rs`, `hud.rs`)

**Description:** VollstÃ¤ndige Integration aller Meilenstein-15-Systeme in Steuerung und HUD:
1) **Sandbox-Einheiten-Spawner (`MapEditorBrush::SpawnUnit`)**:
   - Erlaubt im `[F10]`-Editor das direkte Platzieren von Einheiten auf dem 3D-Schlachtfeld (mit `[N]` zum Durchschalten des Einheitentyps).
2) **Steuerung fÃ¼r Spion-Infiltration, RÃ¼ckwÃ¤rtsgang (`[K]`) & Formationen (`[J]`)**:
   - Rechtsklick mit Spion auf feindliches GebÃ¤ude sendet `InfiltrateBuilding`; `[K]` aktiviert den RÃ¼ckwÃ¤rtsgang-Modus (`ReverseMove`); `[J]` schaltet die aktive Gefechtsformation (`Box` -> `Line` -> `Wedge` -> `Column`) weiter.
3) **Live-APM & Armeewert (`Army Value`) im `[Tab]`-Scoreboard**:
   - Zeigt im Scoreboard fÃ¼r jeden Spieler den aktuellen Gesamtwert der stehenden Armee (`Army Value`) und die Aktionen pro Minute (`APM`).

**Acceptance criteria:**
- [x] Sandbox-Spawner, `[K]`-RÃ¼ckwÃ¤rtsgang, `[J]`-Formationen, Spion-Rechtsklick und Scoreboard-APM/Army-Value funktionieren fehlerfrei.
- [x] Alle Tests und Clippy-Checks Ã¼ber den gesamten Workspace bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 59: `rts-mod-sdk` & `rust-ra2` â€” In-Place GebÃ¤ude-Upgrades (`upgrades_to`, `upgrade_cost`, `upgrade_time_secs`), Ãœberlebende Besatzung/Piloten (`survivor_unit`, `survivor_chance`) & Bergungs-Schrott (`drops_salvage`, `CrateKind::SalvageScrap`) (`schema.rs`, `validator.rs`)

**Description:** Erweiterung des Mod-SDK-Schemas fÃ¼r Meilenstein 16:
1) **In-Place GebÃ¤ude-Upgrades (`schema.rs`)**:
   - `upgrades_to: Option<String>`, `upgrade_cost: u32` und `upgrade_time_secs: f32` auf `BuildingDef` (mit `#[serde(default)]`).
2) **Ãœberlebende Besatzung / Fallschirm-Piloten & Fahrzeug-Schrott (`schema.rs`)**:
   - `survivor_unit: Option<String>` und `survivor_chance: f32` auf `UnitDef` und `BuildingDef` (mit `#[serde(default)]`).
   - `drops_salvage: bool` auf `UnitDef` sowie `CrateKind::SalvageScrap`.
3) **Statische Referenz-Validierung (`validator.rs`) & Referenz-Mod (`rust-ra2`)**:
   - PrÃ¼ft `upgrades_to` und `survivor_unit` in `validate_mod` und ergÃ¤nzt Ausbaustufen/Ãœberlebende/Salvage in `rust-ra2`.

**Acceptance criteria:**
- [x] `BuildingDef`, `UnitDef`, `CrateKind` und `validate_mod` unterstÃ¼tzen alle neuen Felder abwÃ¤rtskompatibel.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-mod-sdk`

---

## Task 60: `rts-sim-core` â€” In-Place GebÃ¤ude-Ausbau (`UpgradeBuilding`), Ãœberlebende Crew & Fallschirm-Piloten, Salvage-Upgrade-Mechanik, Auto-Ingenieur-C4-EntschÃ¤rfung, `last_event_world_pos` & Taktische KI-Erweiterung (`command.rs`, `world.rs`, `combat.rs`, `ai.rs`)

**Description:** Ausbau des deterministischen Simulationskerns (`rts-sim-core`):
1) **In-Place GebÃ¤ude-Upgrades (`GameCommand::UpgradeBuilding`, `upgrading_to`)**:
   - Wertet ein bestehendes GebÃ¤ude direkt auf seinem Footprint auf (`upgrading_to: Option<(String, f32, f32)>`) und aktualisiert nach Abschluss `building_id`, `max_health`, `primary_weapon` sowie Strom-/Supply-Werte.
2) **Ãœberlebende Besatzung / Fallschirm-Piloten & Bergungs-Schrott (`CrateKind::SalvageScrap`, `salvage_upgraded`)**:
   - Spawnt bei ZerstÃ¶rung von Einheiten/GebÃ¤uden deterministisch Ã¼berlebende Infanterie (bei Lufteinheiten mit `parachute_timer > 0.0`).
   - ZerstÃ¶rte Fahrzeuge mit `drops_salvage` hinterlassen `CrateKind::SalvageScrap` (+Credits sowie `salvage_upgraded = true` mit +15 % Feuerkraft und +10 % HP fÃ¼r Fahrzeuge).
3) **Automatische Ingenieur-C4-EntschÃ¤rfung, `last_event_world_pos` & KI-Nutzung (`world.rs`, `ai.rs`)**:
   - Eigene Ingenieure entschÃ¤rfen automatisch tickendes C4 an nahen eigenen GebÃ¤uden; `last_event_world_pos` speichert den letzten Gefechts-/Alarm-Ort; die KI nutzt `UpgradeBuilding`, `EmpPulse`, `InfiltrateBuilding` und sammelt `SalvageScrap`.

**Acceptance criteria:**
- [x] GebÃ¤ude-Upgrades, Ãœberlebende, Salvage-Kisten, Auto-EntschÃ¤rfung und erweiterte KI arbeiten 100 % deterministisch und bestehen alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sim-core`

---

## Task 61: `rts-client-3d` â€” 3D-Spieler-Teamfarben (`player_team_color`), Kontinuierlicher 3D-Schadensrauch & GebÃ¤udebrÃ¤nde (`DamageSmokeVfx`) sowie 3D-GebÃ¤ude-Upgrade- & Salvage-Indikatoren (`render_sync.rs`, `vfx.rs`)

**Description:** Ausbau der 3D-Visualisierung und Partikeleffekte:
1) **3D-Spieler-Teamfarben (`player_team_color` in `render_sync.rs`)**:
   - Zeichnet fÃ¼r jede Einheit und jedes GebÃ¤ude eine eindeutige Spieler-Teamfarben-Markierung am Sockel sowie 3D-Indikatoren fÃ¼r laufende GebÃ¤ude-Upgrades (`upgrading_to`) und Salvage-Upgrades (`salvage_upgraded`).
2) **Kontinuierlicher 3D-Schadensrauch & Feuer bei angeschlagenen Strukturen/Fahrzeugen (`vfx.rs`)**:
   - Emittiert aufsteigende 3D-Rauchwolken bei `< 50 % HP` und lodernde 3D-Feuer-/Funkenpartikel bei `< 25 % HP`.

**Acceptance criteria:**
- [x] 3D-Teamfarben, 3D-Schadensrauch/-Feuer und Salvage-/Upgrade-Gizmos kompilieren fehlerfrei und laufen ohne Bevy-ECS-Konflikte.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-client-3d`

---

## Task 62: `rts-client-3d` â€” GebÃ¤ude-Upgrade-Steuerung (`[Y]`), â€žJump-to-Last-Eventâ€œ-Alarmkamera (`[Backspace]`), 3D-Einheiten-Verfolgerkamera (`[C]`) & Makro-Selektions-Hotkeys (`[.]`, `[,]`) (`input.rs`, `hud.rs`)

**Description:** VollstÃ¤ndige Integration aller Meilenstein-16-Systeme in Steuerung, Kamera und HUD:
1) **GebÃ¤ude-Upgrade per `[Y]` & Sidebar-Button (`SidebarAction::UpgradeSelectedBuilding`)**:
   - Startet `GameCommand::UpgradeBuilding` fÃ¼r ausgewÃ¤hlte ausbaufÃ¤hige GebÃ¤ude und zeigt den Ausbaufortschritt im HUD.
2) **Alarm-Kamerasprung (`[Backspace]`) & 3D-Verfolgerkamera (`[C]` / `follow_entity_id`)**:
   - `[Backspace]` springt zur letzten Alarm-/Kampf-Position (`last_event_world_pos`); `[C]` koppelt die 3D-Kamera live an die ausgewÃ¤hlte Einheit.
3) **Makro-Selektions-Hotkeys (`[.]` fÃ¼r Sammler/MCVs, `[,]` fÃ¼r beschÃ¤digte Fahrzeuge/Flugzeuge)**:
   - Schnelle Auswahl von Wirtschafts- bzw. reparaturbedÃ¼rftigen Einheiten per Tastendruck.

**Acceptance criteria:**
- [x] `[Y]`-GebÃ¤ude-Upgrade, `[Backspace]`-Alarmsprung, `[C]`-Follow-Cam sowie `[.]`/`[,]`-Makro-Hotkeys funktionieren fehlerfrei.
- [x] Alle Tests und Clippy-Checks Ã¼ber den gesamten Workspace bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 67: Workspace-Crates umbenennen (`rts-sdk`, `rts-simulation`, `rts-asset-pipeline`, `rts-network`, `rts-client`) (`engine/Cargo.toml`)

**Description:** Umbenennung der 5 Workspace-Crates auf klare, konsistente DomÃ¤nen-Namen:
- `rts-mod-sdk` -> `rts-sdk`
- `rts-sim-core` -> `rts-simulation`
- `rts-mod-tools` -> `rts-asset-pipeline`
- `rts-net` -> `rts-network`
- `rts-client-3d` -> `rts-client`

**Acceptance criteria:**
- [x] Alle 5 Crates heiÃŸen konsistent (`rts-sdk`, `rts-simulation`, `rts-asset-pipeline`, `rts-network`, `rts-client`) und kompilieren fehlerfrei.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`

---

## Task 68: `rts-sdk` Modularisierung â€” Aufspaltung von `schema.rs` & `scripting.rs` in DomÃ¤nen-Submodule (`schema/`, `scripting/`)

**Description:** Zerlegung von `schema.rs` (1.022 Zeilen) und `scripting.rs` (795 Zeilen) in saubere Submodule (~200â€“420 Zeilen pro Datei):
- `schema/mod.rs`, `schema/common.rs`, `schema/combat_defs.rs`, `schema/entities.rs`, `schema/world_defs.rs`
- `scripting/mod.rs`, `scripting/lua_runtime.rs`

**Acceptance criteria:**
- [x] Keine Datei in `rts-sdk` Ã¼berschreitet ~550 Zeilen; alle Mod-SDK-Tests bestehen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 69: `rts-simulation` Modularisierung â€” Aufspaltung von `world.rs` (5.628 Zeilen), `combat.rs` (1.466 Zeilen), `ai.rs` (966 Zeilen) & `lib.rs` (2.103 Zeilen) (`world/`, `combat/`, `ai/`, `tests/`)

**Description:** Zerlegung der monolithischen Simulationsdateien in klar abgegrenzte DomÃ¤nen-Submodule (~250â€“600 Zeilen pro Datei):
- `world/mod.rs`, `world/initialization.rs`, `world/spawning.rs`, `world/queries.rs`, `world/step_world.rs`, `world/triggers_and_fow.rs`, `world/commands_unit_orders.rs`, `world/commands_construction.rs`, `world/commands_abilities.rs`, `world/commands_tactical.rs`, `world/movement.rs`
- `combat/mod.rs`, `combat/targeting.rs`, `combat/resolution.rs`
- `ai/mod.rs`, `ai/tactics.rs`
- `tests/mod.rs`, `tests/core_mechanics.rs`, `tests/tactical_and_world.rs`, `tests/combined_arms.rs`, `tests/generals_and_stealth.rs`

**Acceptance criteria:**
- [x] Die 5.628-Zeilen-`world.rs`, 1.466-Zeilen-`combat.rs`, 966-Zeilen-`ai.rs` und 2.103-Zeilen-`lib.rs` sind in fokussierte Submodule (~250â€“600 Zeilen) zerlegt; alle 20 Simulations-Tests bestehen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 70: `rts-asset-pipeline` Modularisierung â€” Aufspaltung & Umbenennung von `convert_westwood.rs` (5.051 Zeilen), `glb_gen.rs` (2.539 Zeilen), `convert_openra.rs` (1.689 Zeilen) & `auto_import.rs` (1.223 Zeilen) (`discovery/`, `mesh_builder/`, `openra/`, `westwood/`)

**Description:** Saubere Benennung und Aufteilung der Asset-Pipeline-Module (~350â€“650 Zeilen pro Datei):
- `discovery/` (`mod.rs`, `demo_packs.rs`) statt `auto_import.rs`
- `mesh_builder/` (`mod.rs`, `primitives.rs`, `components.rs`, `buildings.rs`, `units.rs`) statt `glb_gen.rs`
- `openra/` (`mod.rs`, `miniyaml.rs`, `map_reader.rs`, `converter.rs`) statt `convert_openra.rs`
- `westwood/` (`mod.rs`, `ini_parser.rs`, `rules_converter.rs`, `bundle_writer.rs`, `voxel_vxl.rs`, `voxel_meshing.rs`, `voxel_isosurface.rs`, `sprite_shp.rs`, `sprite_relief.rs`, `sprite_turntable.rs`, `mix_and_audio.rs`) statt `convert_westwood.rs`

**Acceptance criteria:**
- [x] Alle Asset-Pipeline-Module tragen saubere DomÃ¤nen-Namen und sind in kompakte Dateien aufgeteilt; alle 9 Asset-Pipeline-Tests bestehen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-asset-pipeline`

---

## Task 71: `rts-client` Modularisierung â€” Aufspaltung & Umbenennung von `hud.rs` (4.022 Zeilen), `input.rs` (2.253 Zeilen), `render_sync.rs` (2.122 Zeilen) & `vfx.rs` (1.007 Zeilen) (`ui/`, `input/`, `rendering/`, `vfx/`)

**Description:** Saubere Modul-Struktur im 3D-Client (~300â€“600 Zeilen pro Datei):
- `ui/` (`mod.rs`, `studio_layout.rs`, `hud_update.rs`, `inspector.rs`, `sidebar.rs`, `actions.rs`, `actions_studio.rs`, `minimap.rs`) statt `hud.rs`
- `input/` (`mod.rs`, `keyboard.rs`, `hotkeys.rs`, `mouse_left.rs`, `mouse_right.rs`) statt `input.rs`
- `rendering/` (`mod.rs`, `entity_sync.rs`, `gizmos.rs`, `gizmos_world.rs`) statt `render_sync.rs`
- `vfx/` (`mod.rs`, `systems.rs`) statt `vfx.rs`

**Acceptance criteria:**
- [x] Alle Client-Module sind sauber benannt und modularisiert; alle 11 Client-Tests sowie alle Workspace-Tests und Clippy-Checks bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 72: `rts-sdk` & `rust-ra2` — Schemata für Klon-Stätte (`cloning_vats`), Tech-Einnahmeprämie (`capture_cash_bonus`), Überroll-Schutz/Mauern (`uncrushable`, `crushable`), Garnisons-Räumung (`clears_garrison`) & Force-Shield (`SupportPowerKind::ForceShield`) (`combat_defs.rs`, `entities.rs`, `rust-ra2/`)

**Description:** Erweiterung des Mod-SDKs und des Referenz-Mods `rust-ra2` für Meilenstein 18:
1) **`WeaponDef` & `SupportPowerKind` (`combat_defs.rs`)**:
   - `clears_garrison: bool` (`#[serde(default)]`) auf `WeaponDef` für Flammen-/Strahlungs-/Toxinwaffen, die verschanzte Infanterie direkt in besetzten Gebäuden ausschalten.
   - `SupportPowerKind::ForceShield` für temporäre Gebäude-Unverwundbarkeit.
2) **`BuildingDef` & `UnitDef` (`entities.rs`)**:
   - `cloning_vats: bool`, `capture_cash_bonus: u32`, `crushable: bool` (`#[serde(default)]`) auf `BuildingDef`.
   - `uncrushable: bool` (`#[serde(default)]`) auf `UnitDef` (für schwere Infanterie wie Tesla-Trooper).
3) **Referenz-Mod `rust-ra2` (`buildings.toml`, `units.toml`, `weapons.toml`)**:
   - `soviet_cloning_vats` (`cloning_vats = true`), `tech_oil_derrick` (`capture_cash_bonus = 1000`), `allied_force_shield` (`SupportPowerKind::ForceShield`), `rad_beam` (`clears_garrison = true`), `tesla_trooper` (`uncrushable = true`), `sandbag_wall` (`crushable = true`).

**Acceptance criteria:**
- [x] `rts-sdk` kompiliert fehlerfrei, validiert `rust-ra2` ohne Fehler und besteht alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 73: `rts-simulation` — Klon-Stätte & Infanterie-Recycling, Tech-Einnahme-Sofortbonus, Garnisons-Räumung, Force-Shield, Globale Superwaffen-Warnung & `production/`-Modularisierung (`production/`, `combat/`, `world/`)

**Description:** Deterministische Simulationslogik für alle Meilenstein-18-Mechaniken inkl. Aufteilung von `production.rs` in `src/production/` (`mod.rs`, `support_structures.rs`):
1) **Klon-Stätte (`cloning_vats`) & Infanterie-Recycling (`production/mod.rs`, `commands_construction.rs`)**:
   - Jede in einer Kaserne fertiggestellte Infanterie-Einheit spawnt automatisch einen kostenlosen Zwillings-Klon an einer aktiven eigenen Klon-Stätte.
   - Eigene Infanterie, die per `LoadIntoCarrier` in eine eigene Klon-Stätte geschickt wird, wird für `75%` ihrer Baukosten recycelt.
2) **Tech-Gebäude Einnahme-Sofortprämie (`capture_cash_bonus` in `combat/resolution.rs`)**:
   - Beim erstmaligen Einnehmen eines neutralen/feindlichen Gebäudes mit `capture_cash_bonus > 0` (`!capture_bonus_claimed`) erhält der Eroberer sofort den Credit-Bonus.
3) **Garnisons-Räumungswaffen (`clears_garrison` in `combat/mod.rs`, `combat/targeting.rs`)**:
   - Waffen mit `clears_garrison = true` fügen bei Treffern auf besetzte Gebäude (`!passenger_ids.is_empty()`) den verschanzten Insassen direkt Schaden zu und entfernen getötete Insassen.
4) **Erweitertes Überrollen (`crushes_infantry` vs. `uncrushable` & `b_def.crushable` in `world/movement.rs`)**:
   - Schwere Fahrzeuge überrollen verwundbare feindliche Infanterie (`!uncrushable`) und leichte Mauern/Zäune (`b_def.crushable`), erhalten dafür Veterancy- & Commander-XP.
5) **Force-Shield (`SupportPowerKind::ForceShield`) & Globale Superwaffen-Aufdeckung/Warnung (`commands_tactical.rs`, `triggers_and_fow.rs`, `queries.rs`)**:
   - `ForceShield` macht eigene Gebäude im Zielradius temporär unverwundbar (`invulnerable_timer`).
   - Aktive Superwaffen-Gebäude decken ihren Umkreis für alle Spieler im Fog of War auf und liefern globale Countdown-Warnungen (`active_superweapon_warnings`).

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; `production/` ist sauber in Submodule unter 400 Zeilen aufgeteilt; ein neuer Integrationstest in `tests/generals_and_stealth.rs` prüft alle Features.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 74: `rts-client` — 360°-Kameradrehung (`Q`/`E` Yaw-Orbit & `Home`-Reset), 3D-Wegpunkt-Ketten, Force-Shield-Kuppeln & Superwaffen-Countdown im HUD (`camera.rs`, `rendering/`, `ui/`)

**Description:** Visuelle und steuerungstechnische Integration im 3D-Client:
1) **360°-Kameradrehung (`yaw_radians` mit `Q` / `E` in `camera.rs`)**:
   - Sanfte Drehung der 3D-Kamera um den Fokuspunkt (`Q` / `E`), Kamera-Reset mit `Home` und winkelkorrigiertes WASD-/Edge-Panning.
2) **3D-Wegpunkt-Linienketten & Force-Shield-Kuppeln (`rendering/gizmos_world.rs`)**:
   - Zeichnet durchgehende 3D-Pfadlinien über alle `path_waypoints` ausgewählter Einheiten sowie leuchtende Energieschild-Kuppeln um Gebäude mit aktivem `invulnerable_timer > 0.0`.
3) **Globale Superwaffen-Warnanzeige im Top-HUD (`ui/hud_update.rs`)**:
   - Zeigt aktive Superwaffen-Countdowns (`⚠ SW: ...`) prominent in der oberen Kommandozeile an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 84: `rts-sdk` & `rust-ra2` — Schemata für Gatling-Spin-Up (`spin_up_per_shot`, `max_spin_up_multiplier`), Helden-Luftschlag-Markierung (`calls_airstrike_damage`, `airstrike_delay_secs`), Robot-Control-Link & Drohnen-Immunität (`requires_powered_building`, `immune_to_mind_control`, `immune_to_radiation`), Mastermind-Overload (`mind_control_overload_dps`), Kalmar-Grapple (`grapple_duration_secs`, `grapple_dps`, `clears_grapple`) & Mobile Raffinerie (`mobile_refinery`)

**Description:** Erweiterung des Mod-SDKs und des Referenz-Mods `rust-ra2` für Meilenstein 22:
1) **`WeaponDef` (`combat_defs.rs`)**:
   - `spin_up_per_shot: f32` und `max_spin_up_multiplier: f32` (`#[serde(default)]`) für Gatling-Feuerraten-Beschleunigung (`gatling_aa_cannon`).
   - `calls_airstrike_damage: f32` und `airstrike_delay_secs: f32` (`#[serde(default)]`) für Boris’ MiG-Laser-Zielmarkierer (`boris_laser_flare`).
   - `grapple_duration_secs: f32`, `grapple_dps: f32` und `clears_grapple: bool` (`#[serde(default)]`) für Riesenkalmar-Umschlingung (`squid_tentacle`) und Delfin-Sonarbefreiung (`sonic_pulse`).
2) **`UnitDef` (`entities.rs`)**:
   - `immune_to_mind_control: bool`, `immune_to_radiation: bool` und `requires_powered_building: Option<String>` (`#[serde(default)]`) für `robot_tank`, `terror_drone` und `chaos_drone`.
   - `mind_control_overload_dps: f32` (`#[serde(default)]`) für `mastermind`.
   - `mobile_refinery: bool` (`#[serde(default)]`) für `yuri_slave_miner`.
3) **Referenz-Mod `rust-ra2` (`units.toml`, `weapons.toml`)**:
   - Verknüpfung der Attribute auf `gattling_tank`, `boris`, `robot_tank`, `terror_drone`, `chaos_drone`, `mastermind`, `giant_squid`, `dolphin` und `yuri_slave_miner`.

**Acceptance criteria:**
- [x] `rts-sdk` kompiliert fehlerfrei, validiert `rust-ra2` mit 0 Fehlern und besteht alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 85: `rts-simulation` — Gatling-Spin-Up, Boris-MiG-Luftschlag, Robot-Control-Link & Immunitäten, Mastermind-Hirnüberlastung, Kalmar-Umschlingung & Delfin-Befreiung sowie Mobile Raffinerie (`combat/`, `world/`, `production/`)

**Description:** Deterministische Simulationslogik für alle Meilenstein-22-Systeme:
1) **Gatling-Spin-Up (`spin_up_level`, `spin_decay_timer`)**:
   - Kontinuierliches Feuern erhöht `spin_up_level` um `spin_up_per_shot` bis `max_spin_up_multiplier`, reduziert den effektiven Schuss-Cooldown entsprechend und kühlt nach Feuerpausen wieder ab.
2) **Boris-MiG-Luftschlag (`pending_airstrike_timer`, `pending_airstrike_damage`)**:
   - Gegen Gebäude nutzt `boris` automatisch seinen Laser-Designator (`boris_laser_flare`), der nach `airstrike_delay_secs` massiven Bombenschaden am Zielgebäude auslöst.
3) **Robot-Control-Link (`is_entity_control_offline`) & Drohnen-Immunitäten (`immune_to_mind_control`, `immune_to_radiation`)**:
   - Einheiten mit `requires_powered_building` schalten sich bei Verlust, EMP oder Stromausfall des Kontrollgebäudes ab; Drohnen/Roboter mit Immunitäten widerstehen Gedankenkontrolle sowie Strahlungs-/Giftwolken.
4) **Mastermind-Hirnüberlastung (`mind_control_overload_dps`)**:
   - Erlaubt Gedankenkontrolle über `mind_control_limit` hinaus, verursacht aber für jede überschüssige Einheit kontinuierlichen Selbstschaden pro Sekunde.
5) **Riesenkalmar-Umschlingung (`grappled_timer`, `grappled_dps`, `grappled_by_id`) & Delfin-Sonarbefreiung (`clears_grapple`)**:
   - Umschlungene Schiffe sind bewegungs- und feuerunfähig und erleiden Quetschschaden, bis der Kalmar getötet oder der Griff durch `sonic_pulse` gelöst wird.
6) **Mobile Raffinerie (`mobile_refinery`)**:
   - `yuri_slave_miner` wandelt geerntetes Erz direkt vor Ort am Erzfeld in Credits (inkl. Purifier-Bonus) um, ohne zur Basis zurückfahren zu müssen.

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein neuer Integrationstest in `tests/generals_and_stealth.rs` prüft Gatling-Spin-Up, Boris-Airstrike, Robot-Control-Link, Mastermind-Overload, Squid-Grapple + Dolphin-Befreiung und Mobile Raffinerie.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 86: `rts-client` — 3D-Gizmos für Gatling-Spin-Up, Laser-Airstrike-Lock, Squid-Grapple & Offline-Roboter sowie erweiterte HUD-Inspektor-Badges (`rendering/`, `ui/`)

**Description:** Visuelle Integration der Meilenstein-22-Systeme im 3D-Client:
1) **3D-Gizmos (`rendering/gizmos_world.rs`)**:
   - Visualisiert Gatling-Spin-Up-Ringe (`spin_up_level > 0.0`), rote MiG-Luftschlag-Zielmarkierungen (`pending_airstrike_timer > 0.0`), Kalmar-Tentakel-Ringe (`grappled_timer > 0.0`) und Signalverlust-Ringe bei Offline-Robotern (`is_entity_control_offline`).
2) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt `GATLING SPIN-UP`, `AIRSTRIKE LOCKED`, `GRAPPLED!`, `LINK OFFLINE!`, `PSI-OVERLOAD!`, `MIND-CTRL IMMUNE`, `RAD-IMMUNE` und `MOBILE REFINERY` im Inspektor an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 78: `rts-sdk` & `rust-ra2` — Schemata für Mobile Bunker-Feuerluken (`passengers_can_fire`), Fahrzeug-Überrollen (`crushes_vehicles`), Industrieanlage & Grinder (`vehicle_cost_discount`, `grinder`), Chaos-/Gift-/Magnetron-Waffen (`spawns_toxic_cloud`, `confuse_duration_secs`, `tractor_pull_distance`) & Drohnenträger (`spawns_drone_unit`, `max_drones`)

**Description:** Erweiterung des Mod-SDKs und des Referenz-Mods `rust-ra2` für Meilenstein 20:
1) **`UnitDef` (`entities.rs`)**:
   - `passengers_can_fire: bool` (`#[serde(default)]`) für mobile Feuerluken-Transporter (`battle_fortress`).
   - `crushes_vehicles: bool` (`#[serde(default)]`) für überschwere Panzer/Festungen (`battle_fortress`).
   - `spawns_drone_unit: Option<String>` und `max_drones: u32` (`#[serde(default)]`) für Trägerschiffe/Drohnenträger (`aircraft_carrier`).
2) **`BuildingDef` (`entities.rs`)**:
   - `grinder: bool` (`#[serde(default)]`) für Fahrzeug-/Einheiten-Recycling (`yuri_grinder`).
   - `vehicle_cost_discount: f32` (`#[serde(default)]`) für Fahrzeug-Rabatt & Produktionsbeschleunigung (`soviet_industrial_plant`).
3) **`WeaponDef` (`combat_defs.rs`)**:
   - `spawns_toxic_cloud: bool` (`#[serde(default)]`) für Bio-/Giftwolken (`virus_rifle`).
   - `confuse_duration_secs: f32` (`#[serde(default)]`) für Chaos-Gas / Raserei (`chaos_gas`).
   - `tractor_pull_distance: f32` (`#[serde(default)]`) für Magnetron-Traktorstrahlen (`magnetron_beam`).
4) **Referenz-Mod `rust-ra2` (`buildings.toml`, `units.toml`, `weapons.toml`)**:
   - Gebäude: `soviet_industrial_plant`, `yuri_grinder`.
   - Einheiten: `battle_fortress`, `yuri_magnetron`, `yuri_chaos_drone`, `aircraft_carrier`, `hornet_drone`.
   - Waffen: `magnetron_beam`, `chaos_gas`, `carrier_launch`, `hornet_bomb` sowie `spawns_toxic_cloud = true` auf `virus_rifle`.

**Acceptance criteria:**
- [x] `rts-sdk` kompiliert fehlerfrei, validiert `rust-ra2` mit 0 Fehlern und besteht alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 79: `rts-simulation` — Mobile Bunker-Feuerluken, Fahrzeug-Crusher, Industrieanlage & Grinder, Giftwolken/Chaos-Gas/Magnetron-Traktorstrahl, Träger-Drohnenstaffeln & Taktischer Karten-Ping (`combat/`, `world/`, `production/`)

**Description:** Deterministische Simulationslogik für alle Meilenstein-20-Systeme:
1) **Mobile Bunker-Feuerluken (`passengers_can_fire`) & Fahrzeug-Überrollen (`crushes_vehicles`)**:
   - Passagiere in einem Fahrzeug mit `passengers_can_fire = true` feuern im Kampftick unabhängig mit ihren eigenen Waffen von der Trägerposition aus.
   - Einheiten mit `crushes_vehicles = true` überrollen feindliche Bodenfahrzeuge (`!target.crushes_vehicles && !target.uncrushable`).
2) **Industrieanlage (`vehicle_cost_discount`) & Schredder (`grinder`)**:
   - `player_vehicle_discount_multiplier` reduziert Baukosten und Bauzeit von Fahrzeugen um bis zu `25%`, solange eine eigene Industrieanlage aktiv und mit Strom versorgt ist.
   - Einheiten (auch per Mind-Control übernommene feindliche Panzer), die in einen eigenen `grinder` geschickt werden (`LoadIntoCarrier`), werden bei Ankunft für `80%` ihrer Baukosten verschrottet.
3) **Giftwolken (`spawns_toxic_cloud`), Chaos-Gas (`confuse_duration_secs` / `confused_timer`) & Magnetron-Traktorstrahl (`tractor_pull_distance`)**:
   - `spawns_toxic_cloud` erzeugt bei Treffern/Kills eine toxische Gefahrenzone.
   - `confused_timer > 0.0` lässt verwirrte Einheiten die nächstgelegene Einheit unabhängig von der Fraktion/Allianz angreifen.
   - `tractor_pull_distance > 0.0` zieht getroffene Fahrzeuge schrittweise zum Schützen hin und legt deren Antrieb kurzzeitig lahm.
4) **Träger-Drohnenstaffeln (`spawns_drone_unit`, `max_drones`, `parent_carrier_id`)**:
   - Träger starten beim Angriff automatisch bis zu `max_drones` Kampfdrohnen (`hornet_drone`) und weisen ihnen das Angriffsziel zu.
5) **Taktischer Karten-Ping (`GameCommand::MapPing`, `recent_map_pings`)**:
   - Speichert Karten-Pings deterministisch für Multiplayer-Verbündete und 3D-/HUD-Anzeige.

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein neuer Integrationstest in `tests/generals_and_stealth.rs` prüft Battle-Fortress-Feuerluken, Fahrzeug-Überrollen, Industrial Plant, Grinder, Chaos-Gas, Magnetron-Traktorstrahl und Carrier-Drohnen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 80: `rts-client` — Taktischer Karten-Ping (`Alt+LMB`), Kamera-Ereignis-Sprung (`[Backspace]` / `[Space]`), 3D-Chaos-/Ping-Gizmos & Erweiterter HUD-Inspektor (`input/`, `rendering/`, `ui/`)

**Description:** Visuelle und steuerungstechnische Integration im 3D-Client:
1) **Taktischer Karten-Ping (`Alt + LMB`) & Kamera-Sprung zum letzten Ereignis (`[Backspace]` / `[Space]`)**:
   - `Alt + LMB` sendet `GameCommand::MapPing` an die Weltkoordinate; `[Backspace]` (oder `[Space]` ohne aktive Spezialfähigkeit) zentriert die 3D-Kamera sofort auf das letzte Kampf-/Ping-/Superwaffen-Ereignis.
2) **3D-Gizmos (`rendering/gizmos_world.rs`)**:
   - Pulsierende 3D-Signalringe für aktive `recent_map_pings` sowie violett-pulsierende Ringe über verwirrten Einheiten (`confused_timer > 0.0`).
3) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt Feuerluken-Status (`FIREPORTS`), Fahrzeug-Crusher (`CRUSHES-VEHICLES`), Träger-Drohnen, Chaos-Status, Grinder-Recycling und Industrieanlagen-Bonus an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 84: `rts-sdk` & `rust-ra2` — Schemata für Gatling-Spin-Up (`spin_up_per_shot`, `max_spin_up_multiplier`), Helden-Luftschlag-Markierung (`calls_airstrike_damage`, `airstrike_delay_secs`), Robot-Control-Link & Drohnen-Immunität (`requires_powered_building`, `immune_to_mind_control`, `immune_to_radiation`), Mastermind-Overload (`mind_control_overload_dps`), Kalmar-Grapple (`grapple_duration_secs`, `grapple_dps`, `clears_grapple`) & Mobile Raffinerie (`mobile_refinery`)

**Description:** Erweiterung des Mod-SDKs und des Referenz-Mods `rust-ra2` für Meilenstein 22:
1) **`WeaponDef` (`combat_defs.rs`)**:
   - `spin_up_per_shot: f32` und `max_spin_up_multiplier: f32` (`#[serde(default)]`) für Gatling-Feuerraten-Beschleunigung (`gatling_aa_cannon`).
   - `calls_airstrike_damage: f32` und `airstrike_delay_secs: f32` (`#[serde(default)]`) für Boris’ MiG-Laser-Zielmarkierer (`boris_laser_flare`).
   - `grapple_duration_secs: f32`, `grapple_dps: f32` und `clears_grapple: bool` (`#[serde(default)]`) für Riesenkalmar-Umschlingung (`squid_tentacle`) und Delfin-Sonarbefreiung (`sonic_pulse`).
2) **`UnitDef` (`entities.rs`)**:
   - `immune_to_mind_control: bool`, `immune_to_radiation: bool` und `requires_powered_building: Option<String>` (`#[serde(default)]`) für `robot_tank`, `terror_drone` und `chaos_drone`.
   - `mind_control_overload_dps: f32` (`#[serde(default)]`) für `mastermind`.
   - `mobile_refinery: bool` (`#[serde(default)]`) für `yuri_slave_miner`.
3) **Referenz-Mod `rust-ra2` (`units.toml`, `weapons.toml`)**:
   - Verknüpfung der Attribute auf `gattling_tank`, `boris`, `robot_tank`, `terror_drone`, `chaos_drone`, `mastermind`, `giant_squid`, `dolphin` und `yuri_slave_miner`.

**Acceptance criteria:**
- [x] `rts-sdk` kompiliert fehlerfrei, validiert `rust-ra2` mit 0 Fehlern und besteht alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 85: `rts-simulation` — Gatling-Spin-Up, Boris-MiG-Luftschlag, Robot-Control-Link & Immunitäten, Mastermind-Hirnüberlastung, Kalmar-Umschlingung & Delfin-Befreiung sowie Mobile Raffinerie (`combat/`, `world/`, `production/`)

**Description:** Deterministische Simulationslogik für alle Meilenstein-22-Systeme:
1) **Gatling-Spin-Up (`spin_up_level`, `spin_decay_timer`)**:
   - Kontinuierliches Feuern erhöht `spin_up_level` um `spin_up_per_shot` bis `max_spin_up_multiplier`, reduziert den effektiven Schuss-Cooldown entsprechend und kühlt nach Feuerpausen wieder ab.
2) **Boris-MiG-Luftschlag (`pending_airstrike_timer`, `pending_airstrike_damage`)**:
   - Gegen Gebäude nutzt `boris` automatisch seinen Laser-Designator (`boris_laser_flare`), der nach `airstrike_delay_secs` massiven Bombenschaden am Zielgebäude auslöst.
3) **Robot-Control-Link (`is_entity_control_offline`) & Drohnen-Immunitäten (`immune_to_mind_control`, `immune_to_radiation`)**:
   - Einheiten mit `requires_powered_building` schalten sich bei Verlust, EMP oder Stromausfall des Kontrollgebäudes ab; Drohnen/Roboter mit Immunitäten widerstehen Gedankenkontrolle sowie Strahlungs-/Giftwolken.
4) **Mastermind-Hirnüberlastung (`mind_control_overload_dps`)**:
   - Erlaubt Gedankenkontrolle über `mind_control_limit` hinaus, verursacht aber für jede überschüssige Einheit kontinuierlichen Selbstschaden pro Sekunde.
5) **Riesenkalmar-Umschlingung (`grappled_timer`, `grappled_dps`, `grappled_by_id`) & Delfin-Sonarbefreiung (`clears_grapple`)**:
   - Umschlungene Schiffe sind bewegungs- und feuerunfähig und erleiden Quetschschaden, bis der Kalmar getötet oder der Griff durch `sonic_pulse` gelöst wird.
6) **Mobile Raffinerie (`mobile_refinery`)**:
   - `yuri_slave_miner` wandelt geerntetes Erz direkt vor Ort am Erzfeld in Credits (inkl. Purifier-Bonus) um, ohne zur Basis zurückfahren zu müssen.

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein neuer Integrationstest in `tests/generals_and_stealth.rs` prüft Gatling-Spin-Up, Boris-Airstrike, Robot-Control-Link, Mastermind-Overload, Squid-Grapple + Dolphin-Befreiung und Mobile Raffinerie.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 86: `rts-client` — 3D-Gizmos für Gatling-Spin-Up, Laser-Airstrike-Lock, Squid-Grapple & Offline-Roboter sowie erweiterte HUD-Inspektor-Badges (`rendering/`, `ui/`)

**Description:** Visuelle Integration der Meilenstein-22-Systeme im 3D-Client:
1) **3D-Gizmos (`rendering/gizmos_world.rs`)**:
   - Visualisiert Gatling-Spin-Up-Ringe (`spin_up_level > 0.0`), rote MiG-Luftschlag-Zielmarkierungen (`pending_airstrike_timer > 0.0`), Kalmar-Tentakel-Ringe (`grappled_timer > 0.0`) und Signalverlust-Ringe bei Offline-Robotern (`is_entity_control_offline`).
2) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt `GATLING SPIN-UP`, `AIRSTRIKE LOCKED`, `GRAPPLED!`, `LINK OFFLINE!`, `PSI-OVERLOAD!`, `MIND-CTRL IMMUNE`, `RAD-IMMUNE` und `MOBILE REFINERY` im Inspektor an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`




---

## Task 75: `rts-sdk` & `rust-ra2` — Schemata für IFV-Passagier-Waffenadaption (`adapts_weapon_to_passenger`), Prisma-Strahlenkopplung (`prism_support`), Kamikaze-Einheiten (`kamikaze`) & Yuri-Superwaffen (`GeneticMutator`, `PsychicDominator`) (`common.rs`, `combat_defs.rs`, `entities.rs`, `rust-ra2/`)

**Description:** Erweiterung des Mod-SDKs und des Referenz-Mods `rust-ra2` für Meilenstein 19:
1) **`SuperweaponType` (`common.rs`)**:
   - `GeneticMutator` (verwandelt Infanterie im Zielradius in `yuri_brute`) und `PsychicDominator` (dauerhafte Gedankenkontrolle über feindliche Einheiten + Gebäudeschaden im Zielradius).
2) **`WeaponDef` (`combat_defs.rs`)**:
   - `prism_support: bool` (`#[serde(default)]`) auf `WeaponDef` für kooperative Prisma-Türme (`prism_cannon`).
3) **`UnitDef` (`entities.rs`)**:
   - `adapts_weapon_to_passenger: bool` (`#[serde(default)]`) auf `UnitDef` (für `multigunner_ifv` / `battle_fortress`).
   - `kamikaze: bool` (`#[serde(default)]`) auf `UnitDef` (für Selbstzerstörungs-/Demolitions-Einheiten wie `demolition_truck` / `terrorist`).
4) **Referenz-Mod `rust-ra2` (`buildings.toml`, `units.toml`, `weapons.toml`)**:
   - `yuri_genetic_mutator` (`superweapon = "genetic_mutator"`), `yuri_psychic_dominator` (`superweapon = "psychic_dominator"`).
   - `multigunner_ifv` (`passenger_slots = 1`, `adapts_weapon_to_passenger = true`), `demolition_truck` (`kamikaze = true`).
   - `prism_cannon` (`prism_support = true`).

**Acceptance criteria:**
- [x] `rts-sdk` kompiliert fehlerfrei, validiert `rust-ra2` ohne Fehler und besteht alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 76: `rts-simulation` — IFV-Waffenadaption & Reparatur-Modus, Prisma-Netzwerk-Strahlenkopplung, Kamikaze-Detonation, Echte Yuri-Superwaffen & Einzelgebäude-Stromabschaltung (`ToggleBuildingPower`) (`combat/`, `world/`, `production/`)

**Description:** Deterministische Simulationslogik für alle Meilenstein-19-Mechaniken:
1) **IFV Passagier-Waffenadaption & Ingenieur-Reparaturmodus (`combat/mod.rs`, `production/support_structures.rs`)**:
   - Hat ein Transporter `adapts_weapon_to_passenger == true` und einen Passagier in `passenger_ids`, übernimmt `select_weapon_for_target` automatisch die Primärwaffe des Passagiers; sitzt ein `Engineer` im IFV, agiert das IFV als mobiles Reparaturfahrzeug (`repairs_vehicles_per_sec` Aura für nahe Fahrzeuge).
2) **Kooperative Prisma-Türme (`prism_support` in `combat/targeting.rs`)**:
   - Feuert ein Gebäude mit einer `prism_support`-Waffe, bündeln bis zu 3 benachbarte freie Prisma-Türme im Umkreis von `12.0` Tiles ihren Strahl in den feuernden Turm (`prism_charged_timer`), erhöhen dessen Schussschaden um `+50%` pro Unterstützer und setzen einen leichten Nachlade-Cooldown.
3) **Kamikaze-/Demolitions-Einheiten (`kamikaze` in `combat/mod.rs`)**:
   - Feuert eine Einheit mit `u_def.kamikaze == true`, sprengt sie sich bei Trefferwirkung selbst (`att.health = 0.0`) und löst den vollen Flächen- und Strahlungsschaden ihrer Waffe aus.
4) **Echte Yuri-Superwaffen (`SuperweaponType::GeneticMutator` & `PsychicDominator` in `commands_abilities.rs`)**:
   - `GeneticMutator` verwandelt alle Infanterie-Einheiten im Zielradius (`radius = 6.0`) in eigene `yuri_brute`-Einheiten.
   - `PsychicDominator` übernimmt dauerhaft die Kontrolle über bis zu 6 feindliche Einheiten im Zielradius (`radius = 5.5`) und fügt feindlichen Gebäuden im Umkreis `450.0` Explosionsschaden zu.
5) **Einzelgebäude-Stromabschaltung (`power_disabled: bool` & `GameCommand::ToggleBuildingPower` in `command.rs`, `world/`)**:
   - Schaltet den Strom einzelner Gebäude an/aus (`power_disabled`). Abgeschaltete Gebäude verbrauchen keinen Strom (`power_delta < 0` ignoriert in `recalculate_power`), feuern nicht, produzieren nicht und gewähren keine aktiven Support-Effekte.

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein neuer Integrationstest in `tests/generals_and_stealth.rs` prüft IFV-Waffenadaption, Prisma-Kopplung, Kamikaze-Detonation, Genetic Mutator, Psychic Dominator und `ToggleBuildingPower`.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 77: `rts-client` — Hotkey `[O]` für Gebäude-Stromabschaltung, 3D-Prisma-Stützstrahlen, 3D-Power-Off-Indikator & IFV-Modus-Anzeige im HUD (`input/`, `rendering/`, `ui/`)

**Description:** Visuelle und steuerungstechnische Integration im 3D-Client:
1) **Hotkey `[O]` (`input/keyboard.rs`)**:
   - Schaltet für alle ausgewählten eigenen Gebäude den Strom per `GameCommand::ToggleBuildingPower` um.
2) **3D-Gizmos für Power-Off & Prisma-Kopplung (`rendering/gizmos_world.rs`)**:
   - Zeichnet einen bernsteinfarbenen `POWER OFF`-Warnring über stromlos geschalteten Gebäuden (`sim_ent.power_disabled`) sowie einen leuchtenden Prisma-Fokusring um gekoppelte Prisma-Türme.
3) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt im unteren Inspektor bei ausgewählten Gebäuden `[POWER OFF - Press O]` bzw. bei ausgewählten IFVs den aktiven Passagier-Waffenmodus (`IFV Turret: ...`) an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 84: `rts-sdk` & `rust-ra2` — Schemata für Gatling-Spin-Up (`spin_up_per_shot`, `max_spin_up_multiplier`), Helden-Luftschlag-Markierung (`calls_airstrike_damage`, `airstrike_delay_secs`), Robot-Control-Link & Drohnen-Immunität (`requires_powered_building`, `immune_to_mind_control`, `immune_to_radiation`), Mastermind-Overload (`mind_control_overload_dps`), Kalmar-Grapple (`grapple_duration_secs`, `grapple_dps`, `clears_grapple`) & Mobile Raffinerie (`mobile_refinery`)

**Description:** Erweiterung des Mod-SDKs und des Referenz-Mods `rust-ra2` für Meilenstein 22:
1) **`WeaponDef` (`combat_defs.rs`)**:
   - `spin_up_per_shot: f32` und `max_spin_up_multiplier: f32` (`#[serde(default)]`) für Gatling-Feuerraten-Beschleunigung (`gatling_aa_cannon`).
   - `calls_airstrike_damage: f32` und `airstrike_delay_secs: f32` (`#[serde(default)]`) für Boris’ MiG-Laser-Zielmarkierer (`boris_laser_flare`).
   - `grapple_duration_secs: f32`, `grapple_dps: f32` und `clears_grapple: bool` (`#[serde(default)]`) für Riesenkalmar-Umschlingung (`squid_tentacle`) und Delfin-Sonarbefreiung (`sonic_pulse`).
2) **`UnitDef` (`entities.rs`)**:
   - `immune_to_mind_control: bool`, `immune_to_radiation: bool` und `requires_powered_building: Option<String>` (`#[serde(default)]`) für `robot_tank`, `terror_drone` und `chaos_drone`.
   - `mind_control_overload_dps: f32` (`#[serde(default)]`) für `mastermind`.
   - `mobile_refinery: bool` (`#[serde(default)]`) für `yuri_slave_miner`.
3) **Referenz-Mod `rust-ra2` (`units.toml`, `weapons.toml`)**:
   - Verknüpfung der Attribute auf `gattling_tank`, `boris`, `robot_tank`, `terror_drone`, `chaos_drone`, `mastermind`, `giant_squid`, `dolphin` und `yuri_slave_miner`.

**Acceptance criteria:**
- [x] `rts-sdk` kompiliert fehlerfrei, validiert `rust-ra2` mit 0 Fehlern und besteht alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 85: `rts-simulation` — Gatling-Spin-Up, Boris-MiG-Luftschlag, Robot-Control-Link & Immunitäten, Mastermind-Hirnüberlastung, Kalmar-Umschlingung & Delfin-Befreiung sowie Mobile Raffinerie (`combat/`, `world/`, `production/`)

**Description:** Deterministische Simulationslogik für alle Meilenstein-22-Systeme:
1) **Gatling-Spin-Up (`spin_up_level`, `spin_decay_timer`)**:
   - Kontinuierliches Feuern erhöht `spin_up_level` um `spin_up_per_shot` bis `max_spin_up_multiplier`, reduziert den effektiven Schuss-Cooldown entsprechend und kühlt nach Feuerpausen wieder ab.
2) **Boris-MiG-Luftschlag (`pending_airstrike_timer`, `pending_airstrike_damage`)**:
   - Gegen Gebäude nutzt `boris` automatisch seinen Laser-Designator (`boris_laser_flare`), der nach `airstrike_delay_secs` massiven Bombenschaden am Zielgebäude auslöst.
3) **Robot-Control-Link (`is_entity_control_offline`) & Drohnen-Immunitäten (`immune_to_mind_control`, `immune_to_radiation`)**:
   - Einheiten mit `requires_powered_building` schalten sich bei Verlust, EMP oder Stromausfall des Kontrollgebäudes ab; Drohnen/Roboter mit Immunitäten widerstehen Gedankenkontrolle sowie Strahlungs-/Giftwolken.
4) **Mastermind-Hirnüberlastung (`mind_control_overload_dps`)**:
   - Erlaubt Gedankenkontrolle über `mind_control_limit` hinaus, verursacht aber für jede überschüssige Einheit kontinuierlichen Selbstschaden pro Sekunde.
5) **Riesenkalmar-Umschlingung (`grappled_timer`, `grappled_dps`, `grappled_by_id`) & Delfin-Sonarbefreiung (`clears_grapple`)**:
   - Umschlungene Schiffe sind bewegungs- und feuerunfähig und erleiden Quetschschaden, bis der Kalmar getötet oder der Griff durch `sonic_pulse` gelöst wird.
6) **Mobile Raffinerie (`mobile_refinery`)**:
   - `yuri_slave_miner` wandelt geerntetes Erz direkt vor Ort am Erzfeld in Credits (inkl. Purifier-Bonus) um, ohne zur Basis zurückfahren zu müssen.

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein neuer Integrationstest in `tests/generals_and_stealth.rs` prüft Gatling-Spin-Up, Boris-Airstrike, Robot-Control-Link, Mastermind-Overload, Squid-Grapple + Dolphin-Befreiung und Mobile Raffinerie.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 86: `rts-client` — 3D-Gizmos für Gatling-Spin-Up, Laser-Airstrike-Lock, Squid-Grapple & Offline-Roboter sowie erweiterte HUD-Inspektor-Badges (`rendering/`, `ui/`)

**Description:** Visuelle Integration der Meilenstein-22-Systeme im 3D-Client:
1) **3D-Gizmos (`rendering/gizmos_world.rs`)**:
   - Visualisiert Gatling-Spin-Up-Ringe (`spin_up_level > 0.0`), rote MiG-Luftschlag-Zielmarkierungen (`pending_airstrike_timer > 0.0`), Kalmar-Tentakel-Ringe (`grappled_timer > 0.0`) und Signalverlust-Ringe bei Offline-Robotern (`is_entity_control_offline`).
2) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt `GATLING SPIN-UP`, `AIRSTRIKE LOCKED`, `GRAPPLED!`, `LINK OFFLINE!`, `PSI-OVERLOAD!`, `MIND-CTRL IMMUNE`, `RAD-IMMUNE` und `MOBILE REFINERY` im Inspektor an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 78: `rts-sdk` & `rust-ra2` — Schemata für Mobile Bunker-Feuerluken (`passengers_can_fire`), Fahrzeug-Überrollen (`crushes_vehicles`), Industrieanlage & Grinder (`vehicle_cost_discount`, `grinder`), Chaos-/Gift-/Magnetron-Waffen (`spawns_toxic_cloud`, `confuse_duration_secs`, `tractor_pull_distance`) & Drohnenträger (`spawns_drone_unit`, `max_drones`)

**Description:** Erweiterung des Mod-SDKs und des Referenz-Mods `rust-ra2` für Meilenstein 20:
1) **`UnitDef` (`entities.rs`)**:
   - `passengers_can_fire: bool` (`#[serde(default)]`) für mobile Feuerluken-Transporter (`battle_fortress`).
   - `crushes_vehicles: bool` (`#[serde(default)]`) für überschwere Panzer/Festungen (`battle_fortress`).
   - `spawns_drone_unit: Option<String>` und `max_drones: u32` (`#[serde(default)]`) für Trägerschiffe/Drohnenträger (`aircraft_carrier`).
2) **`BuildingDef` (`entities.rs`)**:
   - `grinder: bool` (`#[serde(default)]`) für Fahrzeug-/Einheiten-Recycling (`yuri_grinder`).
   - `vehicle_cost_discount: f32` (`#[serde(default)]`) für Fahrzeug-Rabatt & Produktionsbeschleunigung (`soviet_industrial_plant`).
3) **`WeaponDef` (`combat_defs.rs`)**:
   - `spawns_toxic_cloud: bool` (`#[serde(default)]`) für Bio-/Giftwolken (`virus_rifle`).
   - `confuse_duration_secs: f32` (`#[serde(default)]`) für Chaos-Gas / Raserei (`chaos_gas`).
   - `tractor_pull_distance: f32` (`#[serde(default)]`) für Magnetron-Traktorstrahlen (`magnetron_beam`).
4) **Referenz-Mod `rust-ra2` (`buildings.toml`, `units.toml`, `weapons.toml`)**:
   - Gebäude: `soviet_industrial_plant`, `yuri_grinder`.
   - Einheiten: `battle_fortress`, `yuri_magnetron`, `yuri_chaos_drone`, `aircraft_carrier`, `hornet_drone`.
   - Waffen: `magnetron_beam`, `chaos_gas`, `carrier_launch`, `hornet_bomb` sowie `spawns_toxic_cloud = true` auf `virus_rifle`.

**Acceptance criteria:**
- [x] `rts-sdk` kompiliert fehlerfrei, validiert `rust-ra2` mit 0 Fehlern und besteht alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 79: `rts-simulation` — Mobile Bunker-Feuerluken, Fahrzeug-Crusher, Industrieanlage & Grinder, Giftwolken/Chaos-Gas/Magnetron-Traktorstrahl, Träger-Drohnenstaffeln & Taktischer Karten-Ping (`combat/`, `world/`, `production/`)

**Description:** Deterministische Simulationslogik für alle Meilenstein-20-Systeme:
1) **Mobile Bunker-Feuerluken (`passengers_can_fire`) & Fahrzeug-Überrollen (`crushes_vehicles`)**:
   - Passagiere in einem Fahrzeug mit `passengers_can_fire = true` feuern im Kampftick unabhängig mit ihren eigenen Waffen von der Trägerposition aus.
   - Einheiten mit `crushes_vehicles = true` überrollen feindliche Bodenfahrzeuge (`!target.crushes_vehicles && !target.uncrushable`).
2) **Industrieanlage (`vehicle_cost_discount`) & Schredder (`grinder`)**:
   - `player_vehicle_discount_multiplier` reduziert Baukosten und Bauzeit von Fahrzeugen um bis zu `25%`, solange eine eigene Industrieanlage aktiv und mit Strom versorgt ist.
   - Einheiten (auch per Mind-Control übernommene feindliche Panzer), die in einen eigenen `grinder` geschickt werden (`LoadIntoCarrier`), werden bei Ankunft für `80%` ihrer Baukosten verschrottet.
3) **Giftwolken (`spawns_toxic_cloud`), Chaos-Gas (`confuse_duration_secs` / `confused_timer`) & Magnetron-Traktorstrahl (`tractor_pull_distance`)**:
   - `spawns_toxic_cloud` erzeugt bei Treffern/Kills eine toxische Gefahrenzone.
   - `confused_timer > 0.0` lässt verwirrte Einheiten die nächstgelegene Einheit unabhängig von der Fraktion/Allianz angreifen.
   - `tractor_pull_distance > 0.0` zieht getroffene Fahrzeuge schrittweise zum Schützen hin und legt deren Antrieb kurzzeitig lahm.
4) **Träger-Drohnenstaffeln (`spawns_drone_unit`, `max_drones`, `parent_carrier_id`)**:
   - Träger starten beim Angriff automatisch bis zu `max_drones` Kampfdrohnen (`hornet_drone`) und weisen ihnen das Angriffsziel zu.
5) **Taktischer Karten-Ping (`GameCommand::MapPing`, `recent_map_pings`)**:
   - Speichert Karten-Pings deterministisch für Multiplayer-Verbündete und 3D-/HUD-Anzeige.

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein neuer Integrationstest in `tests/generals_and_stealth.rs` prüft Battle-Fortress-Feuerluken, Fahrzeug-Überrollen, Industrial Plant, Grinder, Chaos-Gas, Magnetron-Traktorstrahl und Carrier-Drohnen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 80: `rts-client` — Taktischer Karten-Ping (`Alt+LMB`), Kamera-Ereignis-Sprung (`[Backspace]` / `[Space]`), 3D-Chaos-/Ping-Gizmos & Erweiterter HUD-Inspektor (`input/`, `rendering/`, `ui/`)

**Description:** Visuelle und steuerungstechnische Integration im 3D-Client:
1) **Taktischer Karten-Ping (`Alt + LMB`) & Kamera-Sprung zum letzten Ereignis (`[Backspace]` / `[Space]`)**:
   - `Alt + LMB` sendet `GameCommand::MapPing` an die Weltkoordinate; `[Backspace]` (oder `[Space]` ohne aktive Spezialfähigkeit) zentriert die 3D-Kamera sofort auf das letzte Kampf-/Ping-/Superwaffen-Ereignis.
2) **3D-Gizmos (`rendering/gizmos_world.rs`)**:
   - Pulsierende 3D-Signalringe für aktive `recent_map_pings` sowie violett-pulsierende Ringe über verwirrten Einheiten (`confused_timer > 0.0`).
3) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt Feuerluken-Status (`FIREPORTS`), Fahrzeug-Crusher (`CRUSHES-VEHICLES`), Träger-Drohnen, Chaos-Status, Grinder-Recycling und Industrieanlagen-Bonus an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 84: `rts-sdk` & `rust-ra2` — Schemata für Gatling-Spin-Up (`spin_up_per_shot`, `max_spin_up_multiplier`), Helden-Luftschlag-Markierung (`calls_airstrike_damage`, `airstrike_delay_secs`), Robot-Control-Link & Drohnen-Immunität (`requires_powered_building`, `immune_to_mind_control`, `immune_to_radiation`), Mastermind-Overload (`mind_control_overload_dps`), Kalmar-Grapple (`grapple_duration_secs`, `grapple_dps`, `clears_grapple`) & Mobile Raffinerie (`mobile_refinery`)

**Description:** Erweiterung des Mod-SDKs und des Referenz-Mods `rust-ra2` für Meilenstein 22:
1) **`WeaponDef` (`combat_defs.rs`)**:
   - `spin_up_per_shot: f32` und `max_spin_up_multiplier: f32` (`#[serde(default)]`) für Gatling-Feuerraten-Beschleunigung (`gatling_aa_cannon`).
   - `calls_airstrike_damage: f32` und `airstrike_delay_secs: f32` (`#[serde(default)]`) für Boris’ MiG-Laser-Zielmarkierer (`boris_laser_flare`).
   - `grapple_duration_secs: f32`, `grapple_dps: f32` und `clears_grapple: bool` (`#[serde(default)]`) für Riesenkalmar-Umschlingung (`squid_tentacle`) und Delfin-Sonarbefreiung (`sonic_pulse`).
2) **`UnitDef` (`entities.rs`)**:
   - `immune_to_mind_control: bool`, `immune_to_radiation: bool` und `requires_powered_building: Option<String>` (`#[serde(default)]`) für `robot_tank`, `terror_drone` und `chaos_drone`.
   - `mind_control_overload_dps: f32` (`#[serde(default)]`) für `mastermind`.
   - `mobile_refinery: bool` (`#[serde(default)]`) für `yuri_slave_miner`.
3) **Referenz-Mod `rust-ra2` (`units.toml`, `weapons.toml`)**:
   - Verknüpfung der Attribute auf `gattling_tank`, `boris`, `robot_tank`, `terror_drone`, `chaos_drone`, `mastermind`, `giant_squid`, `dolphin` und `yuri_slave_miner`.

**Acceptance criteria:**
- [x] `rts-sdk` kompiliert fehlerfrei, validiert `rust-ra2` mit 0 Fehlern und besteht alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 85: `rts-simulation` — Gatling-Spin-Up, Boris-MiG-Luftschlag, Robot-Control-Link & Immunitäten, Mastermind-Hirnüberlastung, Kalmar-Umschlingung & Delfin-Befreiung sowie Mobile Raffinerie (`combat/`, `world/`, `production/`)

**Description:** Deterministische Simulationslogik für alle Meilenstein-22-Systeme:
1) **Gatling-Spin-Up (`spin_up_level`, `spin_decay_timer`)**:
   - Kontinuierliches Feuern erhöht `spin_up_level` um `spin_up_per_shot` bis `max_spin_up_multiplier`, reduziert den effektiven Schuss-Cooldown entsprechend und kühlt nach Feuerpausen wieder ab.
2) **Boris-MiG-Luftschlag (`pending_airstrike_timer`, `pending_airstrike_damage`)**:
   - Gegen Gebäude nutzt `boris` automatisch seinen Laser-Designator (`boris_laser_flare`), der nach `airstrike_delay_secs` massiven Bombenschaden am Zielgebäude auslöst.
3) **Robot-Control-Link (`is_entity_control_offline`) & Drohnen-Immunitäten (`immune_to_mind_control`, `immune_to_radiation`)**:
   - Einheiten mit `requires_powered_building` schalten sich bei Verlust, EMP oder Stromausfall des Kontrollgebäudes ab; Drohnen/Roboter mit Immunitäten widerstehen Gedankenkontrolle sowie Strahlungs-/Giftwolken.
4) **Mastermind-Hirnüberlastung (`mind_control_overload_dps`)**:
   - Erlaubt Gedankenkontrolle über `mind_control_limit` hinaus, verursacht aber für jede überschüssige Einheit kontinuierlichen Selbstschaden pro Sekunde.
5) **Riesenkalmar-Umschlingung (`grappled_timer`, `grappled_dps`, `grappled_by_id`) & Delfin-Sonarbefreiung (`clears_grapple`)**:
   - Umschlungene Schiffe sind bewegungs- und feuerunfähig und erleiden Quetschschaden, bis der Kalmar getötet oder der Griff durch `sonic_pulse` gelöst wird.
6) **Mobile Raffinerie (`mobile_refinery`)**:
   - `yuri_slave_miner` wandelt geerntetes Erz direkt vor Ort am Erzfeld in Credits (inkl. Purifier-Bonus) um, ohne zur Basis zurückfahren zu müssen.

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein neuer Integrationstest in `tests/generals_and_stealth.rs` prüft Gatling-Spin-Up, Boris-Airstrike, Robot-Control-Link, Mastermind-Overload, Squid-Grapple + Dolphin-Befreiung und Mobile Raffinerie.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 86: `rts-client` — 3D-Gizmos für Gatling-Spin-Up, Laser-Airstrike-Lock, Squid-Grapple & Offline-Roboter sowie erweiterte HUD-Inspektor-Badges (`rendering/`, `ui/`)

**Description:** Visuelle Integration der Meilenstein-22-Systeme im 3D-Client:
1) **3D-Gizmos (`rendering/gizmos_world.rs`)**:
   - Visualisiert Gatling-Spin-Up-Ringe (`spin_up_level > 0.0`), rote MiG-Luftschlag-Zielmarkierungen (`pending_airstrike_timer > 0.0`), Kalmar-Tentakel-Ringe (`grappled_timer > 0.0`) und Signalverlust-Ringe bei Offline-Robotern (`is_entity_control_offline`).
2) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt `GATLING SPIN-UP`, `AIRSTRIKE LOCKED`, `GRAPPLED!`, `LINK OFFLINE!`, `PSI-OVERLOAD!`, `MIND-CTRL IMMUNE`, `RAD-IMMUNE` und `MOBILE REFINERY` im Inspektor an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`




---

## Task 81: `rts-sdk` & `rust-ra2` — Schemata für Tarn-/Verkleidungssysteme (`disguises_as_terrain`, `can_disguise_as_enemy`), Aktive Raketenabwehr (`intercepts_missiles_radius`, `point_defense_cooldown_secs`), Chrono-Locomotion (`chrono_locomotion`, `temporal_erase_dps`) & Floating-Disc-Drain (`disables_building_duration_secs`, `steals_credits_per_hit`)

**Description:** Erweiterung des Mod-SDKs und des Referenz-Mods `rust-ra2` für Meilenstein 21:
1) **`UnitDef` (`entities.rs`)**:
   - `disguises_as_terrain: bool` (`#[serde(default)]`) für automatische Baum-/Umgebungstarnung im Stillstand (`mirage_tank`).
   - `can_disguise_as_enemy: bool` (`#[serde(default)]`) für Einheiten-/Fraktionsverkleidung (`allied_spy`).
   - `intercepts_missiles_radius: f32` und `point_defense_cooldown_secs: f32` (`#[serde(default)]`) für Point-Defense-Raketenabwehr (`ifv`).
   - `chrono_locomotion: bool` (`#[serde(default)]`) für Chrono-Teleport-Fortbewegung mit distanzabhängiger Phasen-Regeneration (`chrono_legionnaire`).
2) **`BuildingDef` (`entities.rs`)**:
   - `intercepts_missiles_radius: f32` und `point_defense_cooldown_secs: f32` (`#[serde(default)]`) für Gebäude-Raketenabwehr (`allied_patriot_missile`).
3) **`WeaponDef` (`combat_defs.rs`)**:
   - `temporal_erase_dps: f32` (`#[serde(default)]`) für progressive Zeit-Einfrierung und rückstandslose Auslöschung (`neutron_eraser`).
   - `disables_building_duration_secs: f32` und `steals_credits_per_hit: u32` (`#[serde(default)]`) für Gebäude-Lahmlegung und Credits-Absaugung (`saucer_laser_beam` auf `floating_disc`).
4) **Referenz-Mod `rust-ra2` (`buildings.toml`, `units.toml`, `weapons.toml`)**:
   - Verknüpfung der neuen Attribute auf `mirage_tank`, `allied_spy`, `chrono_legionnaire`, `ifv`, `allied_patriot_missile`, `neutron_eraser` und `saucer_laser_beam`.

**Acceptance criteria:**
- [x] `rts-sdk` kompiliert fehlerfrei, validiert `rust-ra2` mit 0 Fehlern und besteht alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 82: `rts-simulation` — Mirage-/Spy-Tarnung (`DisguiseUnits`), Point-Defense-Raketenabwehr, Chrono-Locomotion & Temporal-Erase, Floating-Disc-Drain, Produktions-Pause (`TogglePauseProduction`) & Mehrstufige Rally-Routen (`QueueRallyPoint`)

**Description:** Deterministische Simulationslogik für alle Meilenstein-21-Systeme:
1) **Mirage-Terrain-Tarnung (`disguises_as_terrain`) & Spion-Verkleidung (`can_disguise_as_enemy`, `GameCommand::DisguiseUnits`)**:
   - Stationäre Einheiten mit `disguises_as_terrain` sowie verkleidete Spione (`disguised_as_unit_id`, `disguised_as_player_id`) werden bei der automatischen Zielsuche von regulären Feindeinheiten ignoriert, sofern kein Detektor (`detects_cloak`) in Reichweite ist oder sie selbst feuern.
2) **Aktive Point-Defense-Raketenabwehr (`intercepts_missiles_radius`, `point_defense_cooldown_secs`, `point_defense_timer`)**:
   - Einheiten und aktive Verteidigungsgebäude fangen feindliche `DeliveryKind::Missile`-Projektile im Umkreis vor dem Einschlag in der Luft ab.
3) **Chrono-Locomotion (`chrono_locomotion`, `chrono_recovery_timer`) & Temporale Phasen-Auslöschung (`temporal_erase_dps`, `temporal_progress`)**:
   - Einheiten mit `chrono_locomotion` teleportieren bei Bewegungsbefehlen sofort zum Zielpunkt und erhalten einen distanzabhängigen `chrono_recovery_timer` (unbeweglich & feuerunfähig).
   - Waffen mit `temporal_erase_dps > 0.0` frieren das Ziel ein und bauen `temporal_progress` auf; bei `temporal_progress >= max_health` wird das Ziel ohne Überlebende/Schrott aus der Zeitlinie gelöscht; ohne weiteren Beschuss klingt `temporal_progress` wieder ab.
4) **Floating-Disc-Gebäudelahmlegung & Credits-Drain (`disables_building_duration_secs`, `steals_credits_per_hit`)**:
   - Legt getroffene feindliche Gebäude per `emp_timer` lahm und transferiert bei Treffern auf feindliche Wirtschaftsgebäude/Einheiten bis zu `steals_credits_per_hit` Credits vom Gegner zum Angreifer.
5) **Produktions-Pause (`GameCommand::TogglePauseProduction`) & Verkettete Sammelpunkt-Routen (`GameCommand::QueueRallyPoint`, `rally_waypoints`)**:
   - Pausiert/fortsetzt laufende Einheiten- und Gebäude-Bauwarteschlangen ohne Fortschrittsverlust; neu produzierte Einheiten laufen alle `rally_waypoints` der Reihe nach ab.

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein neuer Integrationstest in `tests/generals_and_stealth.rs` prüft Mirage-/Spy-Tarnung, Point-Defense-Raketenabwehr, Chrono-Locomotion & Temporal-Erase, Floating-Disc-Drain, Produktions-Pause und verkettete Rally-Routen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 83: `rts-client` — Spion-Verkleidungs-Steuerung, `Shift+RMB` Rally-Routen, Produktions-Pause-Button, 3D-Chrono-/Tarnungs-Gizmos & Inspector-Badges (`input/`, `rendering/`, `ui/`)

**Description:** Integration der Meilenstein-21-Systeme im 3D-Client:
1) **Steuerung (`input/mouse_selection.rs`, `input/hotkeys.rs`, `ui/sidebar.rs`)**:
   - Rechtsklick mit ausgewähltem Spion (`can_disguise_as_enemy`) auf eine Infanterieeinheit sendet `GameCommand::DisguiseUnits`.
   - `Shift + Rechtsklick` mit ausgewähltem Produktionsgebäude fügt Wegpunkte per `GameCommand::QueueRallyPoint` zur Sammelpunkt-Route hinzu.
   - Pause/Fortsetzen-Schaltfläche (`PAUSE` / `RESUME`) in der Produktions-Sidebar sendet `GameCommand::TogglePauseProduction`.
2) **3D-Gizmos (`rendering/gizmos_world.rs`)**:
   - Visualisiert mehrstufige `rally_waypoints`-Ketten ausgewählter Produktionsgebäude, temporale Phasen-Ringe (`temporal_progress > 0.0` / `chrono_recovery_timer > 0.0`) sowie Tarn-/Verkleidungs-Indikatoren.
3) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt `TERRAIN-MIRAGE`, `DISGUISED AS: ...`, `POINT-DEFENSE LASER`, `CHRONO-PHASED`, `TEMPORAL-INSTABILITY` und `PROD-PAUSED` im Inspektor an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 84: `rts-sdk` & `rust-ra2` — Schemata für Gatling-Spin-Up (`spin_up_per_shot`, `max_spin_up_multiplier`), Helden-Luftschlag-Markierung (`calls_airstrike_damage`, `airstrike_delay_secs`), Robot-Control-Link & Drohnen-Immunität (`requires_powered_building`, `immune_to_mind_control`, `immune_to_radiation`), Mastermind-Overload (`mind_control_overload_dps`), Kalmar-Grapple (`grapple_duration_secs`, `grapple_dps`, `clears_grapple`) & Mobile Raffinerie (`mobile_refinery`)

**Description:** Erweiterung des Mod-SDKs und des Referenz-Mods `rust-ra2` für Meilenstein 22:
1) **`WeaponDef` (`combat_defs.rs`)**:
   - `spin_up_per_shot: f32` und `max_spin_up_multiplier: f32` (`#[serde(default)]`) für Gatling-Feuerraten-Beschleunigung (`gatling_aa_cannon`).
   - `calls_airstrike_damage: f32` und `airstrike_delay_secs: f32` (`#[serde(default)]`) für Boris’ MiG-Laser-Zielmarkierer (`boris_laser_flare`).
   - `grapple_duration_secs: f32`, `grapple_dps: f32` und `clears_grapple: bool` (`#[serde(default)]`) für Riesenkalmar-Umschlingung (`squid_tentacle`) und Delfin-Sonarbefreiung (`sonic_pulse`).
2) **`UnitDef` (`entities.rs`)**:
   - `immune_to_mind_control: bool`, `immune_to_radiation: bool` und `requires_powered_building: Option<String>` (`#[serde(default)]`) für `robot_tank`, `terror_drone` und `chaos_drone`.
   - `mind_control_overload_dps: f32` (`#[serde(default)]`) für `mastermind`.
   - `mobile_refinery: bool` (`#[serde(default)]`) für `yuri_slave_miner`.
3) **Referenz-Mod `rust-ra2` (`units.toml`, `weapons.toml`)**:
   - Verknüpfung der Attribute auf `gattling_tank`, `boris`, `robot_tank`, `terror_drone`, `chaos_drone`, `mastermind`, `giant_squid`, `dolphin` und `yuri_slave_miner`.

**Acceptance criteria:**
- [x] `rts-sdk` kompiliert fehlerfrei, validiert `rust-ra2` mit 0 Fehlern und besteht alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 85: `rts-simulation` — Gatling-Spin-Up, Boris-MiG-Luftschlag, Robot-Control-Link & Immunitäten, Mastermind-Hirnüberlastung, Kalmar-Umschlingung & Delfin-Befreiung sowie Mobile Raffinerie (`combat/`, `world/`, `production/`)

**Description:** Deterministische Simulationslogik für alle Meilenstein-22-Systeme:
1) **Gatling-Spin-Up (`spin_up_level`, `spin_decay_timer`)**:
   - Kontinuierliches Feuern erhöht `spin_up_level` um `spin_up_per_shot` bis `max_spin_up_multiplier`, reduziert den effektiven Schuss-Cooldown entsprechend und kühlt nach Feuerpausen wieder ab.
2) **Boris-MiG-Luftschlag (`pending_airstrike_timer`, `pending_airstrike_damage`)**:
   - Gegen Gebäude nutzt `boris` automatisch seinen Laser-Designator (`boris_laser_flare`), der nach `airstrike_delay_secs` massiven Bombenschaden am Zielgebäude auslöst.
3) **Robot-Control-Link (`is_entity_control_offline`) & Drohnen-Immunitäten (`immune_to_mind_control`, `immune_to_radiation`)**:
   - Einheiten mit `requires_powered_building` schalten sich bei Verlust, EMP oder Stromausfall des Kontrollgebäudes ab; Drohnen/Roboter mit Immunitäten widerstehen Gedankenkontrolle sowie Strahlungs-/Giftwolken.
4) **Mastermind-Hirnüberlastung (`mind_control_overload_dps`)**:
   - Erlaubt Gedankenkontrolle über `mind_control_limit` hinaus, verursacht aber für jede überschüssige Einheit kontinuierlichen Selbstschaden pro Sekunde.
5) **Riesenkalmar-Umschlingung (`grappled_timer`, `grappled_dps`, `grappled_by_id`) & Delfin-Sonarbefreiung (`clears_grapple`)**:
   - Umschlungene Schiffe sind bewegungs- und feuerunfähig und erleiden Quetschschaden, bis der Kalmar getötet oder der Griff durch `sonic_pulse` gelöst wird.
6) **Mobile Raffinerie (`mobile_refinery`)**:
   - `yuri_slave_miner` wandelt geerntetes Erz direkt vor Ort am Erzfeld in Credits (inkl. Purifier-Bonus) um, ohne zur Basis zurückfahren zu müssen.

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein neuer Integrationstest in `tests/generals_and_stealth.rs` prüft Gatling-Spin-Up, Boris-Airstrike, Robot-Control-Link, Mastermind-Overload, Squid-Grapple + Dolphin-Befreiung und Mobile Raffinerie.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 86: `rts-client` — 3D-Gizmos für Gatling-Spin-Up, Laser-Airstrike-Lock, Squid-Grapple & Offline-Roboter sowie erweiterte HUD-Inspektor-Badges (`rendering/`, `ui/`)

**Description:** Visuelle Integration der Meilenstein-22-Systeme im 3D-Client:
1) **3D-Gizmos (`rendering/gizmos_world.rs`)**:
   - Visualisiert Gatling-Spin-Up-Ringe (`spin_up_level > 0.0`), rote MiG-Luftschlag-Zielmarkierungen (`pending_airstrike_timer > 0.0`), Kalmar-Tentakel-Ringe (`grappled_timer > 0.0`) und Signalverlust-Ringe bei Offline-Robotern (`is_entity_control_offline`).
2) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt `GATLING SPIN-UP`, `AIRSTRIKE LOCKED`, `GRAPPLED!`, `LINK OFFLINE!`, `PSI-OVERLOAD!`, `MIND-CTRL IMMUNE`, `RAD-IMMUNE` und `MOBILE REFINERY` im Inspektor an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`




---

## Task 87: `rts-sdk` & `rust-ra2` — Schemata für Bio-Reaktor-Strombonus (`power_per_occupant`), Fahrzeug-Bunker & Offene Garnisons-Feuerschlitze (`garrison_accepts_vehicles`, `passengers_can_fire`), Siege-Chopper-Landemodus (`lands_when_deployed`), Crazy-Ivan-Zeitzünder (`attaches_bomb_timer_secs`, `attaches_bomb_damage`), Nuklear-Meltdown (`meltdown_damage`, `meltdown_radius`, `meltdown_leaves_radiation`) & Werft-Schiffsreparatur (`repairs_ships_per_sec`)

**Description:** Erweiterung des Mod-SDKs und des Referenz-Mods `rust-ra2` für Meilenstein 23:
1) **`BuildingDef` (`entities.rs`)**:
   - `power_per_occupant: i32` (`#[serde(default)]`) für `yuri_bio_reactor` (`+100` Energie pro einquartierter Einheit).
   - `garrison_accepts_vehicles: bool` und `passengers_can_fire: bool` (`#[serde(default)]`) für `yuri_tank_bunker` und `soviet_battle_bunker`.
   - `meltdown_damage: f32`, `meltdown_radius: f32` und `meltdown_leaves_radiation: bool` (`#[serde(default)]`) für `soviet_nuclear_reactor`.
   - `repairs_ships_per_sec: f32` (`#[serde(default)]`) für `allied_naval_shipyard`, `soviet_naval_shipyard` und `yuri_submarine_pen`.
2) **`UnitDef` (`entities.rs`)**:
   - `lands_when_deployed: bool` (`#[serde(default)]`) für `siege_chopper`.
3) **`WeaponDef` (`combat_defs.rs`)**:
   - `attaches_bomb_timer_secs: f32` und `attaches_bomb_damage: f32` (`#[serde(default)]`) für `ivan_dynamite` (`crazy_ivan`) sowie `chopper_nose_gun` als primäre Luftwaffe für `siege_chopper`.

**Acceptance criteria:**
- [x] `rts-sdk` kompiliert fehlerfrei, validiert `rust-ra2` mit 0 Fehlern und besteht alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 88: `rts-simulation` — Bio-Reaktor-Stromeinspeisung, Tank-Bunker & Garnisons-Feuerschlitze, Siege-Chopper-Landung, Crazy-Ivan-Haftdynamit, Nuklearreaktor-Meltdown, Werft-Schiffsreparatur & Iron-Curtain-Infanterie-Schock (`world/`, `combat/`, `production/`)

**Description:** Deterministische Simulationslogik für alle Meilenstein-23-Systeme:
1) **Bio-Reaktor Infanterie-Stromeinspeisung (`power_per_occupant`)**:
   - `recalculate_power` addiert `def.power_per_occupant * ent.passenger_ids.len() as i32` für fertiggestellte Gebäude und wird beim Ein-/Aussteigen aufgerufen.
2) **Tank-Bunker Fahrzeug-Garnison (`garrison_accepts_vehicles`) & Offene Garnisons-Feuerschlitze (`passengers_can_fire`)**:
   - Bodenfahrzeuge können in Gebäude mit `garrison_accepts_vehicles` (`yuri_tank_bunker`) einfahren; Passagiere in Gebäuden mit `passengers_can_fire` (`yuri_tank_bunker`, `soviet_battle_bunker`) feuern ihre eigenen Waffen mit `+20 %` Reichweiten- und Schadensbonus aus dem Bunker heraus.
3) **Siege Chopper Lande-Artilleriemodus (`lands_when_deployed`)**:
   - Wenn `script_ctx.deployed && u_def.lands_when_deployed`, landet die Lufteinheit auf `tile_elevation`, gilt für Zielerfassung und Pathfinding als `MovementDomain::Ground`, wird stationär und nutzt `deploy_weapon` (`chopper_130mm`).
4) **Crazy Ivan Haft-Dynamit mit Zeitzünder (`attaches_bomb_timer_secs`, `attaches_bomb_damage`)**:
   - Heftet beim Angriff auf beliebige Einheiten oder Gebäude eine tickende Zeitbombe (`c4_timer`, `c4_damage`, `c4_splash_radius`, `c4_attacker_player`) an, die bei Ablauf ODER beim Tod des Trägers mit Flächenschaden detoniert (und durch eigene Pioniere entschärft werden kann).
5) **Sowjetischer Nuklearreaktor-Meltdown (`meltdown_damage`, `meltdown_radius`, `meltdown_leaves_radiation`)**:
   - Zerstörung im Kampf (nicht Verkauf!) verursacht sofortigen AoE-Schaden im `meltdown_radius` und erzeugt eine `RadiationZone`.
6) **Werft-Schiffsreparatur (`repairs_ships_per_sec`) & Iron-Curtain Infanterie-Schock**:
   - Werften reparieren beschädigte eigene Schiffe (`MovementDomain::Naval`) im Umkreis automatisch; `IronCurtain` verleiht Fahrzeugen/Gebäuden Unverwundbarkeit, tötet aber organische Infanterie im Zielradius sofort.

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein neuer Integrationstest in `tests/generals_and_stealth.rs` prüft alle 6 Systeme.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 89: `rts-client` — Fahrzeug-Bunker-Steuerung, 3D-Gizmos für Bio-Reaktor, Tank-Bunker, Siege-Chopper & Ivan-Dynamit sowie HUD-Inspektor-Badges (`input/`, `rendering/`, `ui/`)

**Description:** Integration der Meilenstein-23-Systeme im 3D-Client:
1) **Steuerung (`input/mouse_right.rs`)**:
   - Rechtsklick mit ausgewähltem Bodenfahrzeug auf ein eigenes Gebäude mit `garrison_accepts_vehicles` (`yuri_tank_bunker`) sendet `GameCommand::LoadIntoCarrier`.
   - Rechtsklick mit ausgewählter Einheit mit `attaches_bomb_timer_secs > 0.0` (`crazy_ivan`) + `Ctrl` (oder auf Feinde) erlaubt das Anheften von Dynamit auch an Einheiten.
2) **3D-Gizmos (`rendering/gizmos_world.rs`)**:
   - Visualisiert Bio-Reaktor-Energie-Ringe, besetzte Tank-Bunker-Geschützringe, ausgefahrene Artillerie-Stabilisatoren beim gelandeten Siege Chopper und pulsierende Dynamit-Zeitzünder-Ringe an Einheiten.
3) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt `BIO-REACTOR BOOST`, `TANK BUNKER`, `BUNKER FIREPORTS`, `SIEGE ARTILLERY LANDED`, `TIME-BOMB ATTACHED`, `MELTDOWN HAZARD` und `NAVAL REPAIR DOCK` im Inspektor an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`




---

## Task 90: `rts-sdk` & `rust-ra2` — Schemata für Abfangbare Marschflugkörper (`interceptable_by_aa_health`), Tech-Geheimlabor-Freischaltungen (`unlocks_units`), Psychische Druckwelle (`psi_wave_radius`, `psi_wave_damage`), Enttarnung beim Feuern (`reveal_on_fire_secs`) & Garnisons-Schadensmultiplikator (`garrison_damage_multiplier`)

**Description:** Erweiterung des Mod-SDKs und des Referenz-Mods `rust-ra2` für Meilenstein 24:
1) **`WeaponDef` (`combat_defs.rs`)**:
   - `interceptable_by_aa_health: f32` (`#[serde(default)]`) für schwere Marschflugkörper (`v3_ballistic_rocket`, `dreadnought_missile`, `boomer_torpedo_cruise`), die von regulärer Flugabwehr im Flug abgeschossen werden können.
2) **`BuildingDef` (`entities.rs`) & `validator.rs`**:
   - `unlocks_units: Vec<String>` (`#[serde(default)]`) für `tech_secret_lab` (`["crashed_ufo", "chrono_legionnaire", "desolator", "apocalypse_tank"]`), inklusive Validator-Prüfung der Einheiten-IDs.
3) **`UnitDef` (`entities.rs`)**:
   - `psi_wave_radius: f32` und `psi_wave_damage: f32` (`#[serde(default)]`) für `yuri_clone` und `yuri_prime`.
   - `reveal_on_fire_secs: f32` (`#[serde(default)]`) für U-Boote und Tarneinheiten (`typhoon_sub`, `boomer_sub`, `mirage_tank`).
   - `garrison_damage_multiplier: f32` (`#[serde(default)]`) für `yuri_initiate` (`1.6`).
   - `allied_destroyer` erhält `detects_cloak = true`, `spawns_drone_unit = "osprey_asw"`, `max_drones = 1`; `yuri_brute` erhält `immune_to_mind_control = true`, `uncrushable = true`.

**Acceptance criteria:**
- [x] `rts-sdk` kompiliert fehlerfrei, validiert `rust-ra2` mit 0 Fehlern und besteht alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 91: `rts-simulation` — Abfangbare V3-/Dreadnought-/Boomer-Marschflugkörper durch reguläre Flugabwehr, Tech-Secret-Lab-Baupläne, Yuri Psi-Wave, U-Boot-Enttarnung beim Feuern & Initiate-Garnisonsbonus (`combat/`, `world/`, `production/`)

**Description:** Deterministische Simulationslogik für alle Meilenstein-24-Systeme:
1) **Abfangbare Marschflugkörper (`interceptable_by_aa_health`)**:
   - `SimProjectile` speichert `health` und `max_health`; feindliche Flugabwehr-Einheiten und -Türme (`targets_air = true`) nehmen fliegende Marschflugkörper mit `health > 0.0` automatisch unter Feuer und zerstören sie vor dem Einschlag in der Luft.
2) **Tech-Geheimlabor (`unlocks_units`)**:
   - Hält ein Spieler ein Gebäude mit `unlocks_units` (`tech_secret_lab`), werden diese Geheim-Einheiten fraktionsübergreifend in seinen Produktionsgebäuden (`Barracks` / `WarFactory`) freigeschaltet.
3) **Yuri Clone & Yuri Prime Psychische Druckwelle (`psi_wave_radius`, `psi_wave_damage`)**:
   - `ActivateAbility` (`[G]`) löst bei Einheiten mit `psi_wave_radius > 0.0` eine psychische Schockwelle aus, die feindliche Infanterie im Umkreis ausschaltet, ohne Fahrzeuge oder Gebäude zu beschädigen.
4) **Enttarnung beim Feuern (`reveal_timer` / `reveal_on_fire_secs`) & Initiate-Garnisonsbonus (`garrison_damage_multiplier`)**:
   - Getarnte Einheiten/U-Boote setzen beim Feuern `reveal_timer` und sind währenddessen für den Gegner sichtbar; `garrison_damage_multiplier` verstärkt den Schaden garnisonierter Einheiten (`yuri_initiate`).

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein neuer Integrationstest in `tests/generals_and_stealth.rs` prüft alle Systeme.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 92: `rts-client` — 3D-Trefferpunktebalken für fliegende Marschflugkörper, Enttarnungs- & Geheimlabor-Gizmos sowie HUD-Inspektor-Badges (`rendering/gizmos_world.rs`, `ui/inspector.rs`)

**Description:** Integration der Meilenstein-24-Systeme im 3D-Client:
1) **3D-Gizmos (`rendering/gizmos_world.rs`)**:
   - Zeichnet 3D-HP-Balken über abfangbaren Marschflugkörpern (`proj.max_health > 0.0`), Enttarnungs-Ringe bei `reveal_timer > 0.0` sowie Geheimlabor-Ringe bei Gebäuden mit `unlocks_units`.
2) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt `PSI-WAVE BLAST`, `ASW OSPREY CARRIER`, `GARRISON PYRO`, `REVEALED (FIRING)` und `SECRET LAB UNLOCKS` im Inspektor an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`




---

## Task 93: `rts-sdk` & `rust-ra2` — Schemata für Passive Selbstreparatur (`self_heal_per_sec`) & Yuri Psychic Radar Sensor (`psychic_sensor_radius`) (`schema/entities.rs`)

**Description:** Erweiterung des Mod-SDKs und des Referenz-Mods `rust-ra2` für Meilenstein 25:
1) **`UnitDef` (`entities.rs`)**:
   - `self_heal_per_sec: f32` (`#[serde(default)]`) für Einheiten mit passiver Hüllen-/Gewebe-Regeneration (`kirov_airship` mit `12.0` HP/s und `yuri_prime` mit `8.0` HP/s).
2) **`BuildingDef` (`entities.rs`)**:
   - `psychic_sensor_radius: f32` (`#[serde(default)]`) für `yuri_psychic_sensor` (`28.0` m) und `psychic_beacon` (`32.0` m).

**Acceptance criteria:**
- [x] `rts-sdk` kompiliert fehlerfrei, validiert `rust-ra2` mit 0 Fehlern und besteht alle Unit-Tests.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 94: `rts-simulation` — Chrono-Miner-Notfall-/Rückkehr-Teleport, Marine-C4-Sabotage gegen Kriegsschiffe, Elite-Veteranen-Selbstheilung, Yuri Psychic Sensor Angriffswarnung, Slave-Miner-Nahbereichsernte & Taktische KI (`production/`, `combat/`, `world/`, `ai/`)

**Description:** Deterministische Simulationslogik für alle Meilenstein-25-Systeme:
1) **Chrono-Miner Rückkehr- & Notfall-Teleport (`chrono_harvester`)**:
   - Sammler mit `chrono_harvester = true` teleportieren sich sowohl bei vollem Laderaum als auch bei kritischem Schaden (`health < 45%`) sofort per Chrono-Phasensprung (`ProjectileStyle::Chrono`-CombatEvent) direkt zur nächsten Raffinerie.
2) **Marine-C4-Sabotage gegen Kriegsschiffe (`c4_demolition` vs. `MovementDomain::Naval`)**:
   - Amphibische Kommandos (`tanya`, `navy_seal`) können mit `c4_demolition = true` neben Gebäuden auch feindliche Schiffe und U-Boote (`MovementDomain::Naval`) mit einer `3.5s`-C4-Sprengladung versehen.
3) **Elite-Veteranen-Selbstheilung (`VeterancyRank::Elite`) & `self_heal_per_sec`**:
   - Alle Einheiten auf `VeterancyRank::Elite` regenerieren automatisch `+10.0` HP/s, zusätzlich zu etwaiger `u_def.self_heal_per_sec` (`kirov_airship`, `yuri_prime`).
4) **Yuri Psychic Radar Sensor (`psychic_sensor_threat_lines`) & Slave-Miner Nahbereichsernte (`mobile_refinery`)**:
   - `SimWorld::psychic_sensor_threat_lines` ermittelt alle feindlichen Angriffsbefehle auf eigene Ziele im `psychic_sensor_radius` aktiver Psycho-Sensoren.
   - Einheiten mit `mobile_refinery = true` (`yuri_slave_miner`) ernten im Umkreis von 6 Kacheln automatisch Erz und schreiben die Credits direkt gut.
5) **Taktische KI (`ai/tactics.rs`)**:
   - KI landet `siege_chopper` im `130mm`-Artilleriemodus bei Feindkontakt, zündet `yuri_clone`/`yuri_prime` `Psi-Wave` gegen nahe Infanterie und lässt Ingenieure vorrangig aktive C4-/Dynamit-Ladungen entschärfen.

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein neuer Integrationstest in `tests/generals_and_stealth.rs` prüft alle Systeme.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 95: `rts-client` — 3D-Psycho-Sensor-Warnlinien & Sensor-Ringe sowie HUD-Inspektor-Badges (`rendering/gizmos_world.rs`, `ui/inspector.rs`)

**Description:** Integration der Meilenstein-25-Systeme im 3D-Client:
1) **3D-Gizmos (`rendering/gizmos_world.rs`)**:
   - Zeichnet violette 3D-Angriffswarnlinien (`psychic_sensor_threat_lines`) und Sensor-Radien für `yuri_psychic_sensor`.
2) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt `SELF-HEAL`, `ELITE SELF-REPAIR`, `NAVAL C4 SABOTEUR` und `PSYCHIC SENSOR` im Inspektor an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`




---

## Task 96: `rts-sdk` & `rust-ra2` — Elite-Waffen-Upgrades (`elite_weapon`) für Schlüssel-Einheiten (`units.toml`, `weapons.toml`)

**Description:** Konfiguration authentischer Elite-Waffen-Upgrades (`elite_weapon`) im Referenz-Mod `rust-ra2`:
1) **`weapons.toml` & `units.toml`**:
   - Neue Elite-Waffe `elite_tesla_bolt` (rote Hochspannungs-Doppel-Tesla-Entladung mit Splash und Kettenblitz) als `elite_weapon` für `tesla_trooper` und `tesla_tank`.
   - Neue Elite-Waffe `elite_apocalypse_120mm` (verstärktes Doppel-Hochexplosivgeschütz mit erhöhtem Splash-Radius) als `elite_weapon` für `apocalypse_tank`.
   - `elite_weapon = "prism_beam"` / verstärkter Elite-Bonus für `prism_tank` und `mirage_tank`.

**Acceptance criteria:**
- [x] `rust-ra2` validiert fehlerfrei (`0 errors, 0 warnings`).

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 97: `rts-simulation` — `elite_weapon`-Auswertung & Elite-Reichweiten-/Feuerraten-Bonus, Terror-Drohnen-Wiedergeburt nach Wirts-Zerstörung, Virus-Biotoxin-Wolke (`is_toxic_cloud`), Spion-Radar-Reset, Chronosphären-Infanterie-Letalität & KI-Bunker-/Bioreaktor-Garnisonierung (`combat/`, `world/`, `ai/`)

**Description:** Deterministische Simulationslogik für alle Meilenstein-26-Systeme:
1) **`elite_weapon`-Aktivierung & Elite-Veteranen-Waffenboni (`combat/mod.rs`, `combat/targeting.rs`)**:
   - `select_weapon_for_target` prüft auf `VeterancyRank::Elite` vorrangig `u_def.elite_weapon`, sofern das Ziel damit bekämpft werden kann (auch in IFV-Geschütztürmen).
   - `VeterancyRank::Elite` gewährt zusätzlich `+20 %` Waffenreichweite (`1.2x`) und `+20 %` Feuerrate (`cooldown_secs * 0.8`).
2) **Terror-Drohnen-Wiedergeburt nach Wirtsfahrzeug-Zerstörung (`infected_parasite_unit_id`, `infected_parasite_player` in `world/mod.rs`, `combat/mod.rs`, `combat/resolution.rs`)**:
   - Wenn eine `terror_drone` (`parasite_dps > 0.0`) ein feindliches Fahrzeug befällt, werden `infected_parasite_unit_id` und `infected_parasite_player` auf dem Wirtsfahrzeug gespeichert (und bei Reparatur im Service-Depot gelöscht).
   - Stirbt ein infiziertes Fahrzeug, spawnt die Terror-Drohne unversehrt an der Wrackposition wieder in die Welt.
3) **Virus-Biotoxin-Wolke & Organik-Filter (`is_toxic_cloud` auf `RadiationZone` in `world/mod.rs`, `combat/mod.rs`, `combat/targeting.rs`)**:
   - `RadiationZone` erhält `is_toxic_cloud: bool`. Ist `is_toxic_cloud == true`, erleiden ausschließlich organische Infanteristen (`UnitRole::Infantry | UnitRole::Engineer`) Schaden, während geschlossene Fahrzeuge (`Vehicle`, `Harvester`) immun sind.
   - Bei einem Infanterie-Kill durch eine `spawns_toxic_cloud`-Waffe (`toxin_dart_rifle`) entsteht eine vergrößerte Biotoxin-Wolke am Todesort.
4) **Allied Covert Spy Radar-Blackout (`combat/resolution.rs`)**:
   - Infiltration eines Radar-/Airfield-/Psychic-Sensor-Gebäudes setzt die erkundeten Kartenkacheln (`explored_tiles`) des Opfers zurück und legt das Radar lahm.
5) **Chronosphären-Infanterie-Letalität (`SupportPowerKind::ChronoShift` in `world/commands_abilities.rs`, `world/commands_tactical.rs`)**:
   - Beim Chrono-Shift-Teleport überleben Fahrzeuge sowie Einheiten mit `chrono_locomotion = true` (`chrono_legionnaire`), während gewöhnliche organische Infanteristen durch die Raum-Zeit-Verzerrung sofort getötet werden.
6) **KI-Garnisonierung von Bio-Reaktoren & Gefechtsbunkern (`ai/tactics.rs`)**:
   - Die Skirmish-KI lädt freie Infanteristen bzw. Panzer in Basisnähe automatisch in unvollständig besetzte `yuri_bio_reactor` (bei Strombedarf), `soviet_battle_bunker` und `yuri_tank_bunker` ein (`GameCommand::LoadIntoCarrier`).

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein umfassender Integrationstest in `tests/generals_and_stealth.rs` prüft alle 6 Systeme.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 98: `rts-client` — 3D-Biotoxin-Wolken-Ringe & HUD-Inspektor-Anzeige für `ELITE WEAPON`, `BIOTOXIN CLOUD` & `PARASITE HOST` (`rendering/gizmos_world.rs`, `ui/inspector.rs`)

**Description:** Integration der Meilenstein-26-Systeme im 3D-Client:
1) **3D-Gizmos (`rendering/gizmos_world.rs`)**:
   - Unterscheidet `is_toxic_cloud`-Zonen optisch durch giftgrün-violette Biotoxin-Ringe von nuklearer Strahlung.
2) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt aktive `ELITE WEAPON` und `PARASITE HOST` im Einheiten-Inspektor an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`




---

## Task 99: `rts-sdk` & `rust-ra2` — Wahrzeichen- & Tech-Gebäude-Boni (`einstein_lab`, `great_pyramid` in `buildings.toml`)

**Description:** Konfiguration strategischer Boni für eroberbare Wahrzeichen- und Tech-Gebäude im Referenz-Mod `rust-ra2`:
1) **`buildings.toml`**:
   - `einstein_lab`: Gewährt bei Besitz/Eroberung Alliierten-Hightech-Blaupausen (`unlocks_units = ["prism_tank", "mirage_tank", "chrono_legionnaire"]`) sowie `+15 %` Erzraffinations-Bonus (`ore_refine_multiplier = 0.15`).
   - `great_pyramid`: Gewährt passiven Credit-Trickle (`cash_trickle_per_sec = 15`), Infanterie-Heilung (`heals_infantry_per_sec = 8.0`) und einmaligen Ingenieur-Eroberungsbonus (`capture_cash_bonus = 500`).

**Acceptance criteria:**
- [x] `rust-ra2` validiert fehlerfrei (`0 errors, 0 warnings`).

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 100: `rts-simulation` — 100 % Credit-Refund für gedankenkontrollierte Einheiten in Grinder/Cloning Vats, EMP-/Chrono-Lock-Befreiung von Mind-Control & Smart-Targeting, Floating-Disc-Kraftwerks-Sabotage, Tesla-Reichweiten-Überladung (+2.0 m) & Eiffelturm-Synergie sowie KI-Spezialtaktiken (`combat/`, `world/`, `ai/`)

**Description:** Deterministische Simulationslogik für alle Meilenstein-27-Systeme:
1) **100 % Credit-Recycling gedankenkontrollierter Einheiten (`commands_construction.rs`, `movement.rs`)**:
   - Werden gedankenkontrollierte Einheiten (`mind_controlled_by.is_some()`) per `LoadIntoCarrier` in `yuri_grinder` oder `yuri_cloning_vats` geschickt, erstatten sie `100 %` (`unit_cost`) ihres Kaufpreises zurück (während reguläre eigene Einheiten weiterhin `80 %` im Grinder bzw. `75 %` in den Cloning Vats erstatten).
2) **EMP- & Chrono-Lock-Befreiung gedankenkontrollierter Einheiten & Smart-Targeting (`combat/mod.rs`, `combat/targeting.rs`)**:
   - Wird ein Psycho-Controller (`yuri_clone`, `yuri_prime`, `mastermind`, `yuri_psychic_tower`) durch EMP (`emp_disable_timer > 0.0`), Chrono-Legionnaire (`temporal_lock_timer > 0.0`) oder `power_disabled` betäubt/deaktiviert, reißt die mentale Verbindung sofort ab und alle kontrollierten Einheiten kehren zu ihrem ursprünglichen Besitzer zurück.
   - Außerdem trifft EMP auch Einheiten mit `mind_control_limit > 0`, und die automatische Zielsuche von Mind-Controllern bevorzugt das teuerste verwundbare Ziel in Reichweite und stoppt Auto-Acquire bei vollem `mind_control_limit`.
3) **Floating Disc Kraftwerks-Sabotage & Wirtschafts-Credit-Siphon (`combat/mod.rs`, `world/queries.rs`)**:
   - Trifft eine Waffe mit `disables_building_duration_secs > 0.0` (`saucer_laser_beam`) ein feindliches `BuildingRole::PowerPlant`, legt sie zusätzlich für diese Dauer das feindliche Stromnetz lahm (`power_sabotage_timer` / `power_outage_timers`).
   - `steals_credits_per_hit` saugt gezielt Credits ab, wenn feindliche Wirtschaftsgebäude (`Refinery`, `ConstructionYard`, `cash_trickle_per_sec > 0`) oder Sammler (`UnitRole::Harvester`) getroffen werden.
4) **Tesla-Überladung mit `+2.0` m Reichweiten-Bonus & Pariser Eiffelturm-Synergie (`combat/targeting.rs`)**:
   - `tesla_charge_boost`-Einheiten (`tesla_trooper`, `tesla_tank`) können neben regulären Tesla-Spulen auch den Pariser `eiffel_tower` (`primary_weapon = "tesla_bolt"`) aufladen.
   - Überladene Verteidigungsgebäude (`tesla_charged_timer > 0.0`) erhalten neben `+35 %` Schaden auch `+2.0` m Zusatzreichweite.
5) **Taktische KI für Floating-Disc-Wirtschaftssabotage & Mind-Control-Recycling (`ai/tactics.rs`)**:
   - Die Skirmish-KI schickt freie `floating_disc`-Einheiten gezielt auf feindliche Raffinerien/Kraftwerke und recycelt gedankenkontrollierte Infanterie in eigenen `yuri_cloning_vats` / `yuri_grinder`.

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein umfassender Integrationstest in `src/tests/tactical_and_world.rs` prüft alle Systeme.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 101: `rts-client` — HUD-Inspektor-Anzeige für `TESLA OVERCHARGED`, `CLONING VATS` & `MIND-CONTROLLED` (`ui/inspector.rs`)

**Description:** Integration der Meilenstein-27-Systeme im 3D-Client:
1) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt `MIND-CONTROLLED (100% Grinder/Vats Refund)` bei übernommenen Einheiten sowie `TESLA OVERCHARGED (+35% DMG / +2.0m Range)` und `CLONING VATS` bei Gebäuden im HUD-Inspektor an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`




---

## Task 102: `rts-sdk` & `rust-ra2` — „Stolen Tech“-Schema (`stolen_tech_units`) & Kampagnen-Wahrzeichen-Boni (`statue_of_liberty`, `kremlin_palace`, `white_house`, `london_big_ben`, `transylvania_fortress`)

**Description:** Erweiterung des Gebäude-Schemas und Konfiguration der Kampflabor-Geheimtechnologien sowie der verbliebenen Kampagnen-Wahrzeichen:
1) **`BuildingDef` (`rts-sdk/src/schema/entities.rs` & `validator.rs`)**:
   - Neues Feld `stolen_tech_units: Vec<String>` (`#[serde(default)]`) für Einheiten, die durch Spion-Infiltration eines feindlichen `TechCenter` dauerhaft freigeschaltet werden, inkl. Referenzprüfung im Validator.
2) **`rust-ra2/buildings/buildings.toml`**:
   - `allied_battle_lab`: `stolen_tech_units = ["chrono_legionnaire", "prism_tank"]`
   - `soviet_battle_lab`: `stolen_tech_units = ["desolator", "apocalypse_tank"]`
   - `yuri_battle_lab`: `stolen_tech_units = ["yuri_prime", "mastermind"]`
   - Strategische Eroberungs- und Halte-Boni für `statue_of_liberty`, `kremlin_palace`, `white_house`, `london_big_ben` und `transylvania_fortress`.

**Acceptance criteria:**
- [x] `rust-ra2` validiert fehlerfrei (`0 errors, 0 warnings`).

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 103: `rts-simulation` — Spion-Infiltration („Stolen Tech“ & Rollen-spezifische Veteranen), Offensiver Chronoshift (Wasser-Versenkung / Land-Strandung), Genetic-Mutator-/Psychic-Dominator-Immunität, Tech-Airport-Fallschirme & KI-Spione (`combat/`, `world/`, `ai/`)

**Description:** Deterministische Simulationslogik für alle Meilenstein-28-Systeme:
1) **Spion-Infiltration: „Stolen Tech“ & Rollen-spezifische Veteranen (`combat/resolution.rs`, `world/mod.rs`, `world/spawning.rs`, `world/queries.rs`)**:
   - Infiltration einer `Barracks` setzt `veteran_infantry_unlocked = true` (Infanterie/Ingenieure spawnen als Veteranen).
   - Infiltration einer `WarFactory` setzt `veteran_vehicles_unlocked = true` (Fahrzeuge/Sammler spawnen als Veteranen).
   - Infiltration eines `TechCenter` schaltet alle in `b_def.stolen_tech_units` hinterlegten Einheiten dauerhaft in `stolen_tech_units` frei, sofern der Spieler eine entsprechende Produktionsstätte (`Barracks` bzw. `WarFactory`) besitzt, und setzt Superwaffen-/Support-Power-Timer zurück.
2) **Offensiver Chronoshift: Feindfahrzeuge im Wasser versenken & Schiffe auf Land stranden (`world/commands_tactical.rs`, `world/commands_abilities.rs`)**:
   - `SupportPowerKind::ChronoShift` und `SuperweaponType::Chronosphere` erfassen neben eigenen Fahrzeugen/Schiffen auch feindliche Fahrzeuge/Schiffe im Quellradius (sofern nicht durch `invulnerable_timer > 0.0` geschützt).
   - Werden reine Bodenfahrzeuge (`MovementDomain::Ground`) auf ein Wasserfeld (`grid.is_water` ohne Brücke) chronoportiert, versinken sie sofort (`health = 0.0`); werden Marineschiffe (`MovementDomain::Naval`) auf festes Land chronoportiert, stranden und explodieren sie sofort (`health = 0.0`). Amphibische Einheiten (`MovementDomain::Amphibious`) überleben beides.
3) **Authentische Immunitätsregeln für Genetic Mutator & Psychic Dominator + Tech-Airport-Fallschirme (`world/commands_abilities.rs`)**:
   - `SuperweaponType::GeneticMutator` überspringt bestehende `yuri_brute`-Einheiten sowie nicht-organische Einheiten (`immune_to_mind_control && immune_to_radiation`).
   - `SuperweaponType::PsychicDominator` übernimmt nur verwundbare Boden-/Marineeinheiten (`!immune_to_mind_control`), während gegen Gedankenkontrolle immune Einheiten im Zielgebiet den telekinetischen Schockwellenschaden (`450.0`) erleiden.
   - `SuperweaponType::Paratroopers` (`tech_airport`) spawnt Fallschirmjäger mit authentischem Fallschirm-Sinkflug (`parachute_timer = 3.0`).
4) **Taktische KI für Spion-Tarnung & Gebäude-Infiltration (`ai/tactics.rs`)**:
   - Die Skirmish-KI tarnt freie Spione (`spy_infiltrator = true`) per `DisguiseUnits` und schickt sie per `InfiltrateBuilding` in feindliche Kampflabore, Waffenfabriken, Raffinerien oder Kraftwerke.

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein umfassender Integrationstest in `src/tests/tactical_and_world.rs` prüft alle Systeme.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 104: `rts-client` — HUD-Inspektor-Anzeige für `STOLEN TECH` & Rollen-spezifische Veteranen-Boni (`ui/inspector.rs`)

**Description:** Integration der Meilenstein-28-Systeme im 3D-Client:
1) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt `[STOLEN TECH LAB: ...]` bei Kampflaboren sowie aktive Veteranen- & Stolen-Tech-Boni (`[VET INFANTRY UNLOCKED]`, `[VET VEHICLES UNLOCKED]`, `[STOLEN TECH UNLOCKED: N]`) bei Produktionsgebäuden an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 105: `rts-sdk` & `rust-ra2` — Prism-Refraktions-Schema (`refraction_targets`, `refraction_damage_ratio`, `refraction_range`), Gebäude-Mind-Control (`can_mind_control_buildings`), Torpedo-Wasser-Domäne (`naval_only`), Boomer-Dual-Waffen & Geheime „Stolen Tech“-Kommandos (`chrono_commando`, `chrono_ivan`, `psi_commando`)

**Description:** Erweiterung des Waffen-Schemas und der `rust-ra2`-Kataloge:
1) **`WeaponDef` (`rts-sdk/src/schema/combat_defs.rs`)**:
   - `refraction_targets: u32` (`#[serde(default)]`) — Maximale Anzahl sekundärer Ziele, auf die sich ein Laser-/Prism-Strahl vom Primärziel aus bricht.
   - `refraction_damage_ratio: f32` (`#[serde(default)]`) — Schadensanteil für sekundäre Brechungsstrahlen (z. B. `0.50` = 50% Schaden).
   - `refraction_range: f32` (`#[serde(default)]`) — Reichweite der sekundären Brechungsstrahlen vom Primärziel aus (z. B. `6.5`m).
   - `can_mind_control_buildings: bool` (`#[serde(default)]`) — Erlaubt einer Gedankenkontrollwaffe (`psi_prime_control`), feindliche Gebäude zu übernehmen.
   - `naval_only: bool` (`#[serde(default)]`) — Beschränkt Unterwasser-Torpedos (`torpedo_tube`, `boomer_torpedo`) auf Ziele im/auf dem Wasser (`MovementDomain::Naval` oder `grid.is_water`).
2) **`rust-ra2/weapons/weapons.toml`, `units/units.toml` & `buildings/buildings.toml`**:
   - `prism_beam`, `prism_tower_beam` & neues `elite_prism_beam` erhalten `refraction_targets`, `refraction_damage_ratio` und `refraction_range`.
   - `psi_prime_control` erhält `can_mind_control_buildings = true`.
   - `torpedo_tube` erhält `naval_only = true` & `clears_grapple = true`; `boomer_sub` erhält `primary_weapon = "boomer_torpedo"` (`naval_only = true`, `clears_grapple = true`) und `secondary_weapon = "boomer_cruise_missile"` (Langstrecken-Marschflugkörper gegen Landziele/Gebäude).
   - Drei geheime „Stolen Tech“-Spezialkommandos in `units.toml` (`chrono_commando`, `chrono_ivan`, `psi_commando`) sowie Zuordnung in `buildings.toml` (`allied_battle_lab`, `soviet_battle_lab`, `yuri_battle_lab`).

**Acceptance criteria:**
- [x] `rust-ra2` validiert fehlerfrei (`0 errors, 0 warnings`).

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 106: `rts-simulation` — Prism-Strahl-Refraktion, Yuri-Prime-Gebäude-Gedankenkontrolle, Boomer-Dual-Waffen & Torpedo-Wasser-Domäne, Magnetron-Wasser-Versenkung/Drop-Crush (`combat/mod.rs`, `combat/targeting.rs`)

**Description:** Deterministische Simulationslogik für alle Meilenstein-29-Systeme:
1) **Prism-Strahl-Refraktion (`combat/mod.rs`)**:
   - Trifft eine Waffe mit `refraction_targets > 0` ihr Primärziel, sucht `apply_attack_impact` bis zu `refraction_targets` feindliche Einheiten/Gebäude im Umkreis von `refraction_range` um das Primärziel, fügt ihnen `atk.damage * refraction_damage_ratio` (skaliert mit `armor_multipliers`) zu und erzeugt sekundäre `CombatEvent`-Laserstrahlen vom Primärziel zu jedem Sekundärziel.
2) **Yuri Prime Gebäude-Gedankenkontrolle (`combat/mod.rs`, `combat/targeting.rs`)**:
   - Hat eine Gedankenkontrollwaffe `can_mind_control_buildings = true`, kann sie feindliche Gebäude (außer `Wall`, `Gate`, `Bridge` und `ConstructionYard`) übernehmen (`player_id = attacker_player`, `mind_controlled_by = Some((attacker_id, orig_owner))`). Stirbt der Controller oder wird er per EMP deaktiviert, fällt das Gebäude an `orig_owner` zurück.
3) **Torpedo-Wasser-Domäne (`naval_only`) & Boomer-Dual-Waffen-Auswahl (`combat/mod.rs`, `combat/targeting.rs`)**:
   - Waffen mit `naval_only = true` werden in `select_weapon_for_target` nur gewählt, wenn das Ziel `MovementDomain::Naval` ist oder auf einem Wasserfeld (`world.grid.is_water`) steht. Gegen Landziele wählt `boomer_sub` automatisch seine `secondary_weapon` (`boomer_cruise_missile`), während `typhoon_sub` keine Torpedos auf Ziele an Land abfeuert.
4) **Magnetron-Wasser-Versenkung & Infanterie-Drop-Crush (`combat/mod.rs`)**:
   - Zieht ein Magnetron (`tractor_pull_distance > 0.0`) ein reines Bodenfahrzeug (`MovementDomain::Ground`) auf ein Wasserfeld (`grid.is_water` ohne Brücke), versinkt das Fahrzeug sofort (`health = 0.0`).
   - Wird ein Fahrzeug durch den Magnetron-Strahl auf ein Feld gezogen, auf dem feindliche, zerquetschbare Infanterie (`!uncrushable`) steht, wird diese beim Absetzen zerquetscht (`health = 0.0`).

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein umfassender Integrationstest in `src/tests/tactical_and_world.rs` prüft alle Systeme.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 107: `rts-client` — HUD-Inspektor-Anzeige für Prism-Refraktion, Gebäude-Mind-Control & Torpedo-Domäne (`ui/inspector.rs`)

**Description:** Integration der Meilenstein-29-Systeme im 3D-Client:
1) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt `[PRISM REFRACTION: Nx]` bei Einheiten/Gebäuden mit `refraction_targets > 0`, `[MIND CONTROLS BUILDINGS]` bei Yuri Prime sowie `[NAVAL TORPEDO + CRUISE MISSILE]` bei Dual-Domänen-U-Booten an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`


---

## Task 108: `rts-sdk` & `rust-ra2` — Todesexplosionen (`death_explosion_damage`, `death_explosion_radius`), Allied GI Sandsack-Stellung (`deploy_weapon = "heavy_para_mg"`, `immobile_when_deployed = true`) & Amphibische Marine-Landungstransporter (`allied_lcv`, `soviet_zubr_transport`, `yuri_hover_transport`)

**Description:** Erweiterung des Einheiten-Schemas und der `rust-ra2`-Kataloge:
1) **`UnitDef` (`rts-sdk/src/schema/entity_defs.rs`)**:
   - `death_explosion_damage: f32` (`#[serde(default)]`) — Flächenschaden bei Zerstörung einer volatilen Einheit (z. B. Crazy Ivan, Chrono Ivan, Demolition Truck).
   - `death_explosion_radius: f32` (`#[serde(default)]`) — Sprengradius bei Zerstörung der Einheit.
2) **`rust-ra2/weapons/weapons.toml` & `units/units.toml`**:
   - Neues schweres Sandsack-MG `heavy_para_mg` in `weapons.toml` (`damage = 32.0`, `range = 10.5`, `cooldown_secs = 0.55`, `burst = 2`).
   - `allied_gi` erhält `deploy_weapon = "heavy_para_mg"` und `immobile_when_deployed = true`.
   - `crazy_ivan`, `chrono_ivan` und `demolition_truck` erhalten `death_explosion_damage` und `death_explosion_radius`.
   - Drei amphibische Hovercraft-Landungstransporter (`allied_lcv`, `soviet_zubr_transport`, `yuri_hover_transport`) mit `movement_domain = "amphibious"`, `passenger_slots = 6` und `can_carry_vehicles = true`.

**Acceptance criteria:**
- [x] `rust-ra2` validiert fehlerfrei (`0 errors, 0 warnings`).

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 109: `rts-simulation` — Crazy Ivan Authentische Zeitbomben-Mechanik (Null Direktschaden beim Anbringen, Transporter-Detonation & Todesexplosion), Battle-Fortress-Omni-Crush gegen eingegrabene Einheiten & Chrono-Miner Notfall-Chronoshift (`combat/mod.rs`, `combat/targeting.rs`, `combat/resolution.rs`, `world/movement.rs`, `production.rs`)

**Description:** Deterministische Simulationslogik für Meilenstein 30:
1) **Crazy Ivan Zeitbomben-Mechanik & Todesexplosion (`combat/mod.rs`, `combat/targeting.rs`, `combat/resolution.rs`)**:
   - Waffen mit `attaches_bomb_timer_secs > 0.0` (`ivan_dynamite`) verursachen beim Anbringen keinen sofortigen Direktschaden (`eff_dmg = 0.0`), heften nur den tickenden Sprengsatz (`c4_timer`, `bomb_damage`, `bomb_splash_radius`) an und löschen danach `att.target_entity_id = None` beim Angreifer.
   - Steigt eine mit einer Ivan-Bombe versehene Einheit in einen Transporter (`transported_by = Some(carrier_id)`), läuft ihr `c4_timer` weiter; bei Ablauf explodiert die Bombe an der Position des Transporters und trifft sowohl den Transporter als auch alle Feinde im Sprengradius.
   - Stirbt eine Einheit mit `death_explosion_damage > 0.0`, löst sie in `resolve_combat_and_destructions` eine Kettenreaktions-Explosion im Umkreis von `death_explosion_radius` aus.
2) **Allied GI Sandsack-Stellung & Battle-Fortress-Omni-Crush (`world/movement.rs`)**:
   - Normale Panzer (`crushes_infantry = true`, `crushes_vehicles = false`) können eingegrabene Infanterie (`script_ctx.deployed`) nicht überrollen.
   - Schwere Omni-Crusher (`crushes_vehicles = true`, z. B. `battle_fortress`) überrollen auch eingegrabene Infanterie und eingegrabene Fahrzeuge.
3) **Chrono-Miner Notfall-Chronoshift unter Beschuss (`production.rs`)**:
   - Fällt ein sammelnder `chrono_miner` (`chrono_harvester = true`, Zustand `Harvesting` oder `MovingToOre`) unter 40 % HP (`health < max_health * 0.40`), bricht er das Sammeln ab und teleportiert sich per Notfall-Chronoshift sofort zu seiner nächsten eigenen Raffinerie.

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein umfassender Integrationstest in `src/tests/tactical_and_world.rs` prüft alle Systeme.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 110: `rts-client` — HUD-Inspektor-Anzeige für Deploy-Waffe, Amphibischen Fahrzeugtransporter & Todesexplosion (`ui/inspector.rs`)

**Description:** Integration der Meilenstein-30-Systeme im 3D-Client:
1) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt `[DEPLOY WEAPON: ...]`, `[AMPHIBIOUS VEHICLE TRANSPORT]` sowie `[VOLATILE DEATH EXPLOSION]` im HUD-Inspektor an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`



---

## Task 111: `rust-ra2` — NightHawk Stealth-Transporter (`cloaked = true`, `reveal_on_fire_secs = 2.0`) & Belagerungs-Mindestreichweiten (`min_range`) für V3, Dreadnought, Boomer & Magnetron

**Description:** Erweiterung der `rust-ra2`-Einheiten- und Waffenkataloge:
1) **`rust-ra2/units/units.toml`**:
   - `nighthawk_chopper` erhält `cloaked = true` und `reveal_on_fire_secs = 2.0` (Radar-/Stealth-Transporter).
2) **`rust-ra2/weapons/weapons.toml`**:
   - `v3_ballistic_rocket` erhält `min_range = 5.0`.
   - `dreadnought_missile` erhält `min_range = 6.0`.
   - `boomer_cruise_missile` erhält `min_range = 5.0`.
   - `magnetic_levitation_beam` erhält `min_range = 4.0`.

**Acceptance criteria:**
- [x] `rust-ra2` validiert fehlerfrei (`0 errors, 0 warnings`).

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

---

## Task 112: `rts-simulation` — Multigunner-IFV Komplett-Adaption (Exklusiver Turm, Ingenieur-Reparatur-IFV, Crazy-Ivan-Spreng-IFV, Hund/Spion-Detektor-IFV), Spurloses Chrono-Ausradieren (`temporal_erased`), Distanzbewusste Waffenwahl (`min_range`) & Aktives Tesla-Überladen per Befehl

**Description:** Deterministische Simulationslogik für Meilenstein 31:
1) **Multigunner-IFV Komplett-Adaption (`combat/mod.rs`, `world/step_world.rs`, `combat/resolution.rs`)**:
   - Exklusiver Turm-Wechsel: Sitzt ein Passagier im IFV (`adapts_weapon_to_passenger = true`), ersetzt dessen Bewaffnung den Standard-Raketenwerfer (`hover_missile`) vollständig.
   - Ingenieur-IFV (`UnitRole::Engineer`): Verwandelt das IFV in ein schnelles Reparatur- & Entschärfungsfahrzeug (`40.0 HP/s` Fahrzeugreparatur + Entfernung von Terror-Drohnen, EMP und C4-/Ivan-Bomben im Umkreis).
   - Crazy-Ivan-/Chrono-Ivan-IFV (`attaches_bomb_timer_secs > 0.0` oder `death_explosion_damage > 0.0` Passagier): Rüstet das IFV mit `nuclear_demolition` (Kamikaze-Spreng-LKW + volatile Todes-Explosion) aus.
   - Spion-/Kampfhund-IFV (`spy_infiltrator` oder `dog_bite`): Verleiht dem IFV `detects_cloak` im gesamten Sichtradius sowie ein Schnellfeuer-MG (`m60_mg`) bei Spion-Besatzung.
2) **Chrono-Legionär Spurloses Zeit-Ausradieren & Phasen-Starre (`combat/targeting.rs`, `combat/resolution.rs`, `production/mod.rs`)**:
   - Einheiten und Gebäude mit `temporal_lock_timer > 0.0` sind in der Zeit eingefroren (keine Bewegung, kein Feuern, keine Produktion).
   - Durch `temporal_progress >= max_health` ausgelöschte Einheiten/Gebäude verschwinden spurlos aus der Zeitlinie ohne Todes-Explosion (`death_explosion_damage`), ohne nukleare Kernschmelze (`meltdown_damage`), ohne Überlebende (`survivor_unit`) und ohne Schrott (`drops_salvage`).
3) **Distanzbewusste Waffenwahl (`min_range`) & Aktives Tesla-Überladen (`combat/mod.rs`, `combat/targeting.rs`)**:
   - `select_weapon_for_target` berücksichtigt die optionale Zieldistanz (`target_dist`), sodass Einheiten mit mehreren Waffen (z. B. `boomer_sub`, `guardian_gi`) automatisch eine Waffe wählen, deren `[min_range, eff_range]` die aktuelle Distanz abdeckt.
   - `tesla_trooper` und `tesla_tank` können verbündete `soviet_tesla_coil` (und `eiffel_tower`) explizit per Angriffsbefehl anvisieren, rücken in Reichweite vor und überladen das Verteidigungsgebäude (`tesla_charged_timer = 6.0`).

**Acceptance criteria:**
- [x] Alle neuen Mechaniken arbeiten deterministisch; ein umfassender Integrationstest in `src/tests/tactical_and_world.rs` prüft alle Systeme.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`

---

## Task 113: `rts-client` — HUD-Inspektor-Anzeige für Aktiven IFV-Turm-Modus (Repair / Demolition / Detector / Passenger Turret) & Mindestreichweite (`MIN RANGE`) (`ui/inspector.rs`)

**Description:** Integration der Meilenstein-31-Systeme im 3D-Client:
1) **HUD-Inspektor (`ui/inspector.rs`)**:
   - Zeigt beim Multigunner-IFV den aktuellen adaptierten Turm-Modus (`[IFV MODE: REPAIR TURRET]`, `[IFV MODE: DEMOLITION TRUCK]`, `[IFV MODE: DETECTOR TURRET]` bzw. die adaptierte Passagierwaffe) sowie `[MIN RANGE: ...]` bei Artillerie-/Marschflugkörper-Einheiten an.

**Acceptance criteria:**
- [x] Alle Workspace-Tests (`cargo test --all-targets`) und Clippy-Checks (`cargo clippy --all-targets -- -D warnings`) bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings`


---

# Phase 7: Engine/Mod-Entkopplung & Konsolidierung der Kernsysteme (`tasks/refactoring_plan.md`)

## Task 114: `rts-simulation` — Zentrale Capability-Abfragen (`can_entity_act`, `can_entity_move`, `can_entity_fire`, `is_entity_targetable`, `is_entity_invulnerable`) in `world/queries.rs` & Konsolidierung aller Subsysteme

**Description:** Einführung zentraler Capability-Methoden auf `SimWorld` / `SimEntity` in `world/queries.rs`, die alle verstreuten Einzelprüfungen auf Lähmungs-, Phasen-, Fallschirm-, Offline- und Unverwundbarkeits-Zustände an einer einzigen Stelle kapseln. Ersetzt die redundanten `if`-Ketten in `world/movement.rs`, `combat/`, `production/`, `world/commands_*.rs`, `world/triggers_and_fow.rs` und `ai/tactics.rs` und schließt dabei gleichzeitig die offenen Konsistenzlücken (Fahrzeug-Überrollen gegen phasenverschobene/neutrale Einheiten, DoT während Phasen-Lock, Boarding durch offline/fliegende Einheiten, Trigger-Zonen, Gebäude-Reparatur unter EMP/Chrono sowie KI-Befehlsfilter).

**Acceptance criteria:**
- [x] `world/queries.rs` stellt `can_entity_act`, `can_entity_move`, `can_entity_fire`, `is_entity_targetable` (bzw. `is_phased_out`) und `is_entity_invulnerable` bereit.
- [x] `movement.rs`, `combat/`, `production/`, `commands_*.rs`, `triggers_and_fow.rs` und `ai/tactics.rs` nutzen diese zentralen Abfragen statt manueller Einzel-Timer-Ketten.
- [x] Ein Integrationstest verifiziert, dass phasenverschobene/gelähmte/offline Einheiten in allen Subsystemen konsistent behandelt werden.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-simulation`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 113

**Files likely touched:**
- `engine/crates/rts-simulation/src/world/queries.rs`
- `engine/crates/rts-simulation/src/world/movement.rs`
- `engine/crates/rts-simulation/src/world/triggers_and_fow.rs`
- `engine/crates/rts-simulation/src/world/commands_abilities.rs`
- `engine/crates/rts-simulation/src/ai/tactics.rs`

**Estimated scope:** Medium (5 files)

---

## Task 115: `rts-simulation` — Zentrales `EntityLifecycle`-System (`world/lifecycle.rs`) für Despawn/Link-Cleanup, Owner-Transfer, Boarding & Entity-Transformation

**Description:** Bündelung aller Entitäts-Zustandsübergänge in einem neuen Modul `world/lifecycle.rs`. Ersetzt den bisher an ~12 Stellen duplizierten Aufräum-Code (für `target_entity_id`, `boarding_carrier_id`, `service_depot_target_id`, `infiltrate_target_id`, `guard_target_id`, `grappled_by_id`, `mind_controlled_by`, `parent_carrier_id`, `passenger_ids`, `production_queue`-Refunds und `CollisionGrid`-Footprints) durch zentrale Methoden: `cleanup_entity_links`, `remove_entity_cleanly`, `transfer_entity_owner`, `enter_container` und `transform_entity`.

**Acceptance criteria:**
- [x] Verkauf (`SellBuilding`), Zerstörung, Chrono-Erase, Grinder-Recycling, Ingenieur-/Mind-Control-/Trigger-`ChangeOwner`, Transporter-Einstieg und MCV-/Bauhof-Transformation rufen ausschließlich die zentralen Lifecycle-Methoden in `world/lifecycle.rs` auf.
- [x] Beim Einklappen eines Gebäudes zu einer Einheit (z. B. Bauhof -> MCV) werden Passagiere entladen, laufende Produktionen erstattet und aktive Links sauber gelöst.
- [x] Bei `TriggerAction::ChangeOwner` werden `is_repairing`, `rally_point` und `rally_waypoints` konsistent über `transfer_entity_owner` zurückgesetzt.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 114

**Files likely touched:**
- `engine/crates/rts-simulation/src/world/lifecycle.rs`
- `engine/crates/rts-simulation/src/world/mod.rs`
- `engine/crates/rts-simulation/src/world/commands_abilities.rs`
- `engine/crates/rts-simulation/src/world/commands_construction.rs`
- `engine/crates/rts-simulation/src/world/triggers_and_fow.rs`
- `engine/crates/rts-simulation/src/combat/resolution.rs`

**Estimated scope:** Medium (5–6 files)

---

## Checkpoint: After Tasks 114–115 (Phase 7.1: Capability-Queries & EntityLifecycle)
- [x] Alle Workspace-Tests bestehen (`cargo test --manifest-path engine/Cargo.toml --workspace`)
- [x] Clippy meldet `0` Warnungen (`cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`)
- [x] Keine verstreuten Einzel-Timer-Prüfketten oder duplizierten Link-Cleanup-Blöcke mehr in `rts-simulation`

---

## Task 116: `rts-sdk` & `rts-simulation` — Erweiterung von `StatusEffectDef` & `ActiveBuff` zum universellen `StatusSystem` inkl. Client-VFX/HUD-Metadaten

**Description:** Ausbau von `StatusEffectDef` in `rts-sdk/src/schema/common.rs` und `ActiveBuff` in `rts-simulation/src/world/mod.rs` zu einem generischen Status-System, das alle zeitbasierten Zustände abdeckt: Bewegungs-/Waffen-/Fähigkeitssperre (`stunned`, `immobilized`, `disarmed`), Phasen-Verschiebung (`phased_out`), Unverwundbarkeit (`invulnerable`), Verwirrung (`confused`), Überladung (`power_charge_boost`), periodischer Schaden (`dot_dps`), verzögerte Sprengung bei Ablauf (`delayed_burst_damage`, `delayed_burst_splash`), Spawn beim Tod (`on_kill_spawn_unit`), Quell-Verknüpfung (`source_entity_id`, `attacker_player_id`) sowie optionale Client-Darstellungsfelder (`display_label`, `vfx_ring_color`, `vfx_style`).

**Acceptance criteria:**
- [x] `StatusEffectDef` und `ActiveBuff` unterstützen alle genannten generischen Eigenschaften sowie HUD-/VFX-Metadaten.
- [x] Eine zentrale Funktion `step_entity_statuses` tickt alle `active_buffs` deterministisch (inkl. Pausierung nicht-phasenbezogener DoTs/Bomben während `phased_out`, Auslösung von `delayed_burst_damage` bei Ablauf und Bereinigung von `source_entity_id`-gebundenen Effekten wie Grapple).
- [x] Die zentralen Capability-Abfragen aus Task 114 werten `active_buffs` vollständig aus.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk -p rts-simulation`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 115

**Files likely touched:**
- `engine/crates/rts-sdk/src/schema/common.rs`
- `engine/crates/rts-simulation/src/world/mod.rs`
- `engine/crates/rts-simulation/src/world/queries.rs`
- `engine/crates/rts-simulation/src/world/step_world.rs`

**Estimated scope:** Medium (4 files)

---

## Task 117: `rts-simulation` & `rts-client` — Clean-Cut-Ablösung der ~15 Einzel-Timer auf `SimEntity` & datengetriebene Status-Anzeige in `rts-client`

**Description:** Echter Clean-Cut: Vollständige Entfernung der ~15 spezialisierten Einzel-Timer auf `SimEntity` (`emp_disable_timer`, `chrono_disable_timer`, `temporal_lock_timer`, `invulnerable_timer`, `tesla_charged_timer`, `confused_timer`, `grappled_timer`, `grappled_dps`, `grappled_by_id`, `infected_parasite_dps`, `infected_parasite_unit_id`, `infected_parasite_player`, `c4_timer`, `bomb_damage`, `bomb_splash_radius`, `bomb_attacker_player`) zugunsten standardisierter `ActiveBuff`-Einträge. Gleichzeitige Entkopplung von `rts-client/src/ui/inspector.rs` und `rts-client/src/rendering/gizmos.rs`, sodass Status-Texte (`display_label`) und 3D-Aura-Ringe (`vfx_ring_color`) generisch aus `active_buffs` statt über hartcodierte RA2-Strings (`"IRON CURTAIN"`, `"GRAPPLED BY SQUID!"`) gerendert werden.

**Acceptance criteria:**
- [x] Die ~15 redundanten Timer- und Bomben-/Parasiten-Einzelfelder sind komplett aus `SimEntity` gelöscht.
- [x] `rts-client` (`ui/inspector.rs`, `rendering/gizmos.rs`) enthält keine hartcodierten Einzel-Timer-Checks oder RA2-Status-Strings mehr.
- [x] Sämtliche bestehenden Simulations-, Netzwerk- und Client-Tests sind migriert und bestehen fehlerfrei.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 116

**Files likely touched:**
- `engine/crates/rts-simulation/src/world/mod.rs`
- `engine/crates/rts-simulation/src/world/movement.rs`
- `engine/crates/rts-simulation/src/combat/resolution.rs`
- `engine/crates/rts-simulation/src/combat/targeting.rs`
- `engine/crates/rts-client/src/rendering/gizmos.rs`
- `engine/crates/rts-client/src/ui/inspector.rs`

**Estimated scope:** Medium (6 files)

---

## Checkpoint: After Tasks 116–117 (Phase 7.2: Universelles StatusSystem & Client-Entkopplung)
- [x] `SimEntity` ist per Clean-Cut um die ~15 Spezial-Timer bereinigt
- [x] `rts-client` rendert Status-Labels und 3D-Ringe generisch über `ActiveBuff`-Metadaten
- [x] Alle Workspace-Tests (`rts-sdk`, `rts-simulation`, `rts-network`, `rts-client`, `rts-asset-pipeline`) bestehen

---

## Task 118: `rts-sdk`, `rts-simulation` & `rust-ra2` — Globale `[rules]`-Sektion (`ModRules`) in `mod.toml` & datengetriebene Transporter-/IFV-Waffenadaption

**Description:**
1) Einführung einer datengetriebenen `ModRules`-Struktur (`[rules]` in `mod.toml` mit Engine-Defaults) für globale Spielregeln: Veterancy-XP-Schwellen & Boni, Commander-Rang-Schwellen, Sell-Refund-Quote (`0.5`), Gebäude-Reparaturrate/-kosten, Low-Power-Geschwindigkeitsfaktor (`0.5`) sowie Crate-Spawn-Intervall und gewichteter Crate-Belohnungspool.
2) Auslagerung der bisher in `combat/mod.rs` und `production/support_structures.rs` hartcodierten IFV-Regeln (`"nuclear_demolition"`, `"dog_bite"`, `"m60_mg"`, Ingenieur-Reparaturrate) in datengetriebene Eigenschaften (`passenger_weapon_by_unit`, `passenger_weapon_by_role`, `passenger_engineer_repair_per_sec`) und Deklaration in `rust-ra2/units/units.toml` & `rust-ra2/mod.toml`.

**Acceptance criteria:**
- [x] `combat/mod.rs` enthält keine hartcodierten Waffen-Strings (`"nuclear_demolition"`, `"dog_bite"`, `"m60_mg"`) mehr.
- [x] Globale Konstanten (Veterancy, Sell-Refund, Reparatur, Low-Power, Crates) werden aus `loaded.manifest.rules` gelesen.
- [x] `rust-ra2` validiert fehlerfrei; alle IFV- und Wirtschaftstests bestehen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Mod validation: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`

**Dependencies:** Task 117

**Files likely touched:**
- `engine/crates/rts-sdk/src/manifest.rs`
- `engine/crates/rts-sdk/src/schema/entities.rs`
- `engine/crates/rts-simulation/src/combat/mod.rs`
- `engine/crates/rts-simulation/src/production/support_structures.rs`
- `engine/crates/rts-simulation/src/world/step_world.rs`
- `rust-ra2/mod.toml`
- `rust-ra2/units/units.toml`

**Estimated scope:** Medium (6–7 files)

---

## Task 119: `rts-sdk`, `rts-simulation` & `rust-ra2` — Kompositorisches `EffectPayload`-System für Support-Powers, Superwaffen & Infiltration

**Description:** Einführung eines generischen `EffectPayload`-Schemas in `rts-sdk` (`Damage`, `ApplyStatus`, `SpawnHazardZone`, `SpawnUnits`, `TransformUnits`, `Displace`, `ChangeOwner`, `ModifyEconomy`, `InvokeScript`), über das `SupportPowerDef`, Gebäude-Fähigkeiten und Spion-Infiltrationen in `rust-ra2/` gesteuert werden. Entfernt hartcodierte RA2-Einheiten-Fallbacks (wie `"conscript"` beim `GeneticMutator`) aus `rts-simulation`.

**Acceptance criteria:**
- [x] `SupportPowerDef` und `AbilityDef` führen ihre Effekte über `Vec<EffectPayload>` aus.
- [x] Hartcodierte Einheiten-IDs (wie `"conscript"`) in `commands_abilities.rs` sind komplett gelöscht und durch die Payload-Konfiguration in `rust-ra2/buildings/buildings.toml` ersetzt.
- [x] Alle Superwaffen-, Support-Power- und Infiltrations-Tests bestehen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 118

**Files likely touched:**
- `engine/crates/rts-sdk/src/schema/common.rs`
- `engine/crates/rts-sdk/src/schema/combat_defs.rs`
- `engine/crates/rts-simulation/src/world/commands_abilities.rs`
- `rust-ra2/buildings/buildings.toml`

**Estimated scope:** Medium (4 files)

---

## Checkpoint: After Tasks 118–119 (Phase 7.3: Engine/Mod-Trennung für Regeln & Superwaffen)
- [x] Keine hartcodierten RA2-Einheiten-/Waffen-IDs oder globalen Magic-Numbers mehr in `rts-simulation`
- [x] Alle Superwaffen, Support-Powers, IFV-Adaptionen und globalen Regeln sind datengetrieben in `rust-ra2/` definiert
- [x] Alle Workspace-Tests und Clippy bestehen fehlerfrei

---

## Task 120: `rts-sdk`, `rts-simulation` & `rust-ra2` — Clean-Cut-Konsolidierung von `WeaponDef` über generische On-Hit-Status-/Zonen-Payloads

**Description:** Echter Clean-Cut auf `WeaponDef`: Ablösung und Löschung der redundanten On-Hit-Spezialfelder (`emp_duration_secs`, `confuse_duration_secs`, `tesla_charge_boost`, `attaches_bomb_timer_secs`, `attaches_bomb_damage`, `grapple_duration_secs`, `grapple_dps`, `parasite_dps`, `leaves_radiation`, `spawns_toxic_cloud`) zugunsten generischer On-Hit-Status- und Zonen-Definitionen (`on_hit_status`, `on_hit_zone`) sowie direkte Migration von `rust-ra2/weapons/weapons.toml`, `rts-asset-pipeline` und den Tests.

**Acceptance criteria:**
- [x] Die alten On-Hit-Einzel-Booleans/Timer sind aus `WeaponDef`, `SimProjectile` und `PendingAttack` entfernt.
- [x] `combat/resolution.rs` wendet On-Hit-Status und On-Hit-Hazard-Zonen über eine einheitliche Schleife an.
- [x] `rust-ra2/weapons/weapons.toml` und alle Konverter-/Simulations-Tests sind migriert und grün.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 119

**Files likely touched:**
- `engine/crates/rts-sdk/src/schema/combat_defs.rs`
- `engine/crates/rts-simulation/src/combat/mod.rs`
- `engine/crates/rts-simulation/src/combat/resolution.rs`
- `engine/crates/rts-asset-pipeline/src/westwood/rules_converter.rs`
- `rust-ra2/weapons/weapons.toml`

**Estimated scope:** Medium (5 files)

---

## Task 121: `rts-sdk`, `rts-simulation`, `rts-client` & `rust-ra2` — Gemeinsame Trait-Substructs (`ContainerSpec`, `StealthSensorSpec`, `DeathPayloadSpec`, `PointDefenseSpec`, `SupportAuraSpec`) für `UnitDef` & `BuildingDef`

**Description:** Zusammenführung der bisher zwischen `UnitDef` und `BuildingDef` doppelt gepflegten Felder (Transport/Garnison, Tarnung/Sensoren, Todesexplosion/Meltdown/Survivors, Raketenabwehr, Heil-/Reparatur-/Einkommens-Auren) in gemeinsame Trait-Substructs (`ContainerSpec`, `StealthSensorSpec`, `DeathPayloadSpec`, `PointDefenseSpec`, `SupportAuraSpec`). Ersetzt die ~30 doppelten `match &ent.kind { EntityKind::Unit .., EntityKind::Building .. }`-Verzweigungen in `rts-simulation` und `rts-client` durch direkte Trait-Zugriffe (`world.entity_container_spec(ent, loaded)` usw.) und migriert `rust-ra2/units/units.toml` sowie `rust-ra2/buildings/buildings.toml`.

**Acceptance criteria:**
- [x] Doppelte Einzelfelder auf `UnitDef` und `BuildingDef` sind durch die gemeinsamen Trait-Substructs ersetzt.
- [x] `rts-simulation` und `rts-client/src/ui/inspector.rs` nutzen die gemeinsamen Trait-Substructs ohne doppelte Unit/Building-Fallunterscheidungen und ohne hartcodierte Einheiten-IDs (wie `"osprey_asw"`).
- [x] Alle 5 Workspace-Crates (`rts-sdk`, `rts-simulation`, `rts-network`, `rts-client`, `rts-asset-pipeline`) sowie `validate_mod("rust-ra2")` schließen mit `0` Fehlern und `0` Warnungen ab.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 120

**Files likely touched:**
- `engine/crates/rts-sdk/src/schema/entities.rs`
- `engine/crates/rts-sdk/src/validator.rs`
- `engine/crates/rts-simulation/src/world/queries.rs`
- `engine/crates/rts-client/src/ui/inspector.rs`
- `rust-ra2/units/units.toml`
- `rust-ra2/buildings/buildings.toml`

**Estimated scope:** Medium (6 files)

---

## Checkpoint: After Tasks 120–121 (Phase 7 Complete: Clean Engine / Mod Separation)
- [x] Alle Workspace-Tests (`cargo test --manifest-path engine/Cargo.toml --workspace --all-targets`) bestehen
- [x] `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings` liefert `0` Warnungen
- [x] Engine-Kern (`rts-simulation`) ist frei von hartcodierten `rust-ra2`-Regeln und nutzt durchgängig die 4 zentralen Kernsysteme (`CapabilityQueries`, `EntityLifecycle`, `StatusSystem`, `EffectPayload`)


---

# Phase 8: Komplettierung der RTS-Engine & des Dev/Mod-Kits (Tasks 122–131)

Architektur- und Begründungsdokument: [`tasks/engine_dev_mod_kit_plan.md`](file:///d:/projekt/rust-rts/tasks/engine_dev_mod_kit_plan.md)

## Task 122: `rts-sdk`, `rts-client`, `rts-asset-pipeline` & `rust-ra2` — Strikter Clean-Cut auf `UnitDef`/`BuildingDef` & `ModRules::default()` sowie datengetriebene `SubFactionDef`, `VisualDef.icon` & `MissionDef`-Medien statt Client-Hardcoding

**Description:** Führt den strikten Clean-Cut in `rts-sdk` und `rts-client` durch:
1. Entfernt die ~25 verbliebenen flachen Altlast-Felder auf `UnitDef` und `BuildingDef` in `rts-sdk/src/schema/entities.rs`, die durch `container: ContainerSpec`, `stealth: StealthSensorSpec`, `death_effect: DeathPayloadSpec`, `point_defense: PointDefenseSpec` und `support_aura: SupportAuraSpec` abgedeckt sind, und bereinigt `ModRules::default()` in `rts-sdk/src/manifest.rs` von allen hartcodierten RA2-Einheiten-IDs (`"grizzly_tank"`, `"yuri_prime"`, `"chrono_ivan"` usw.).
2. Beseitigt die ~400 Zeilen RA2-Hardcoding in `rts-client/src/input/mod.rs` und `rts-client/src/ui/actions_studio.rs` (`skirmish_country_descriptor`, `skirmish_country_sub_faction_id`, `resolve_campaign_loading_screen`, `resolve_entity_cameo_asset_path`). Erweitert `FactionDef` um `sub_factions: Vec<SubFactionDef>`, `VisualDef` um `icon: Option<String>` und `MissionDef` um `loading_screen: Option<String>`, `intro_video: Option<String>`, `victory_video: Option<String>` und migriert alle Kataloge (`rust-ra2/`, `mods/`) und Konverter (`rts-asset-pipeline`).

**Acceptance criteria:**
- [x] Die ~25 flachen Altlast-Felder auf `UnitDef` und `BuildingDef` sowie alle RA2-Strings in `ModRules::default()` sind aus `rts-sdk` entfernt; alle Mods, Konverter und Tests nutzen direkt die 5 Trait-Substructs.
- [x] `FactionDef` unterstützt `sub_factions: Vec<SubFactionDef>`, `VisualDef` unterstützt `icon: Option<String>` und `MissionDef` unterstützt `loading_screen`, `intro_video`, `victory_video`.
- [x] `rts-client/src/input/mod.rs` und `actions_studio.rs` enthalten keine hartcodierten RA2-Länder-Arrays oder Einheiten-Cameo-Match-Tabellen mehr, sondern lesen diese dynamisch aus `LoadedMod`.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 121

**Files likely touched:**
- `engine/crates/rts-sdk/src/schema/common.rs`
- `engine/crates/rts-sdk/src/schema/world_defs.rs`
- `engine/crates/rts-client/src/input/mod.rs`
- `rust-ra2/factions/factions.toml`
- `rust-ra2/units/units.toml`

**Estimated scope:** Medium (5 files)

---

## Task 123: `rts-sdk`, `rts-client` & `rust-ra2` — Datengetriebenes Audio-, Unit-Voice- & Lokalisierungs-System (`AudioCatalog` & `LocaleCatalog`)

**Description:** Führt optionale, datengetriebene Audio- und Lokalisierungs-Kataloge (`audio/audio.toml` mit `AudioCatalog`, `UnitVoiceSpec`, `MusicTrackDef` sowie `locales/<lang>.toml` mit `LocaleCatalog`) in `rts-sdk` ein. Erweitert `rts-client/src/audio.rs` (`SoundSettingsResource`), sodass Einheiten-Sprachantworten (`on_select`, `on_move`, `on_attack`, `on_die`), benutzerdefinierte Waffen-SFX und Musik-Tracks direkt aus den Mod-Katalogen geladen und abgespielt werden (mit prozeduralem Synthesizer-Fallback für Minimal-Mods).

**Acceptance criteria:**
- [x] `LoadedMod` lädt optional `audio/audio.toml` (`AudioCatalog`) und `locales/*.toml` (`LocaleCatalog`), und `validate_mod` prüft referenzierte Audiodateien.
- [x] `SoundSettingsResource` in `rts-client/src/audio.rs` triggert mod-definierte Einheiten-Voices, Waffen-SFX und Musik-Tracks.
- [x] `rust-ra2/audio/audio.toml` bindet die vorhandenen `.wav`-Assets aus `rust-ra2/assets/audio/` sauber ein.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 122

**Files likely touched:**
- `engine/crates/rts-sdk/src/schema/common.rs`
- `engine/crates/rts-sdk/src/loader.rs`
- `engine/crates/rts-sdk/src/validator.rs`
- `engine/crates/rts-client/src/audio.rs`
- `rust-ra2/audio/audio.toml`

**Estimated scope:** Medium (5 files)

---

## Checkpoint: After Tasks 122–123 (Phase 8.1: Client-Entkopplung & Presentation-Schema)
- [x] Keine hartcodierten `rust-ra2`-Asset-Pfade, Länder-Tabellen oder Cameo-Listen mehr in `rts-client`
- [x] Sub-Fraktionen, Cameos, Kampagnen-Screens, Unit-Voices und Lokalisierung sind zu 100% datengetrieben
- [x] Alle Workspace-Tests und Clippy bestehen fehlerfrei

---

## Task 124: `rts-sdk`, `rts-network` & `rts-asset-pipeline` — Feldgenaue Mod-Vererbung (`extends` mit TOML-Deep-Merge & `asset_search_dirs`), Deterministischer `content_hash()` & `.rtsmod`-Package-Bundler

**Description:** Erweitert `ModManifest` um `extends: Option<String>` und optionale Katalog-Pfade, sodass Sub-Mods, Balance-Patches oder Map-Packs auf einer Basis-Mod aufbauen und per **TOML-Deep-Merge pro `id`** gezielt einzelne Felder (z. B. nur `{ id = "grizzly_tank", cost = 650 }`) überschreiben können (inkl. Zyklenerkennung und kaskadierender Asset-Auflösung über `LoadedMod.asset_search_dirs: Vec<PathBuf>`). Fügt `LoadedMod::content_hash(&self) -> u64` (deterministischer FNV-1a-Hash über alle Kataloge, `[rules]` und Skripte) hinzu, prüft diesen beim Lockstep-Multiplayer-Handshake in `rts-network` und ergänzt `rts-mod-tools pack` / `unpack` zum Erstellen und Entpacken portabler `.rtsmod.json`-Pakete.

**Acceptance criteria:**
- [x] Eine Sub-Mod mit `extends = "rust-ra2"` führt ein feldgenaues TOML-Deep-Merge pro `id` durch und löst 3D-Modelle, Cameos und Sounds automatisch über `asset_search_dirs` aus der Basis-Mod auf; Vererbungszyklen werden erkannt und abgewiesen.
- [x] `LoadedMod::content_hash()` ändert sich deterministisch bei jeder Katalog-, Regel- oder Skript-Änderung und wird in `LockstepServer` / `LockstepClient` beim Handshake verifiziert.
- [x] `rts-mod-tools pack` und `rts-mod-tools unpack` verpacken und entpacken eine Mod verlustfrei inklusive `content_hash`-Prüfung.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 123

**Files likely touched:**
- `engine/crates/rts-sdk/src/manifest.rs`
- `engine/crates/rts-sdk/src/loader.rs`
- `engine/crates/rts-network/src/protocol.rs`
- `engine/crates/rts-network/src/transport.rs`
- `engine/crates/rts-asset-pipeline/src/main.rs`

**Estimated scope:** Medium (5 files)

---

## Task 125: `rts-sdk`, `rts-simulation` & `rts-client` — Live Hot-Reload (`[F12]`) für TOML-Kataloge, `[rules]` und `.rhai`/`.lua`-Skripte im laufenden Gefecht

**Description:** Implementiert `LoadedMod::reload_from_disk(&mut self)` (mit automatischer `validate_mod`-Prüfung und transaktionalem Rollback bei Syntax-/Validierungsfehlern) sowie `SimWorld::apply_mod_hot_reload(&mut self, loaded: &LoadedMod)`. Ermöglicht es Moddern, im laufenden 3D-Client per `[F12]` oder Studio-Button geänderte Einheiten-/Waffen-/Gebäude-TOMLs, `[rules]` und `.rhai`/`.lua`-Skripte sofort ohne Spielneustart anzuwenden.

**Acceptance criteria:**
- [x] `LoadedMod::reload_from_disk` lädt geänderte Kataloge und Skripte neu und behält bei fehlerhaften Edits den vorherigen validen Zustand bei.
- [x] `SimWorld::apply_mod_hot_reload` aktualisiert `max_health` (proportional), `ModRules` und Skript-Hooks aller lebenden `SimEntity`-Instanzen im laufenden Match.
- [x] Hotkey `[F12]` und ein Studio-Button lösen den Live-Hot-Reload im 3D-Client aus und melden die Anzahl aktualisierter Definitionen im EVA-/Studio-Banner.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 124

**Files likely touched:**
- `engine/crates/rts-sdk/src/loader.rs`
- `engine/crates/rts-simulation/src/world/mod.rs`
- `engine/crates/rts-client/src/input/keyboard.rs`
- `engine/crates/rts-client/src/ui/actions_studio.rs`

**Estimated scope:** Medium (4 files)

---

## Checkpoint: After Tasks 124–125 (Phase 8.2: Mod-Kit Core, Packaging & Hot-Reload)
- [x] Mod-Vererbung (`extends`), Lockstep-`content_hash()`, `.rtsmod`-Pack/Unpack und Live-Hot-Reload (`[F12]`) sind vollständig getestet
- [x] Alle Workspace-Tests und Clippy bestehen fehlerfrei

---

## Task 126: `rts-sdk`, `rts-simulation` & `rts-client` — 3D-Karteneditor v2: Pinselgrößen, Symmetrie-Painting, Spawn-/Trigger-Platzierung & Playability-Validator

**Description:** Baut den In-Game-Karteneditor (`MapEditorBrush` & Studio-Editor-Panel) zu einem vollwertigen Level- und Szenario-Editor aus: Unterstützung für variable Pinselgrößen (`1x1`, `3x3`, `5x5`), Karten-Symmetrie (`Off`, `MirrorX`, `MirrorZ`, `Rotational2Way`, `Rotational4Way`), katalogbasierte Platzierung von `PreplacedEntityDef`, `PlayerSpawnDef` und `TriggerZoneDef` sowie `validate_map_playability` (prüft Bauplatz am Startpunkt und A*-Pfad-Erreichbarkeit von Erzfeldern für jeden Spieler-Spawn).

**Acceptance criteria:**
- [x] Pinselgröße (`1x1`, `3x3`, `5x5`) und Symmetrie-Modus (`MirrorX`, `MirrorZ`, `Rotational2Way`, `Rotational4Way`) übertragen Terrain-/Erz-Edits automatisch auf die gespiegelten Kartenkoordinaten.
- [x] Beliebige Katalog-Einheiten/Gebäude (`PreplacedEntityDef`), Spieler-Spawns (`PlayerSpawnDef`) und Trigger-Zonen lassen sich im Editor platzieren und als `.toml` exportieren.
- [x] `validate_map_playability` erkennt blockierte Spieler-Spawns oder unerreichbare Erzfelder zuverlässig per A*-Check.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 125

**Files likely touched:**
- `engine/crates/rts-simulation/src/pathfinding.rs`
- `engine/crates/rts-simulation/src/world/map_editor.rs`
- `engine/crates/rts-client/src/input/mod.rs`
- `engine/crates/rts-client/src/input/mouse_left.rs`
- `engine/crates/rts-client/src/ui/actions_studio.rs`

**Estimated scope:** Medium (5 files)

---

## Task 127: `rts-simulation`, `rts-asset-pipeline` & `rts-client` — Deterministischer Headless Balance-Analyzer & Matchup-Simulator (`rts-mod-tools benchmark-balance`) inkl. Ausreißer-Warnungen

**Description:** Implementiert `rts-simulation::benchmark` für deterministische Einheiten-Duelle bei gleichem Kredit-Budget (Cost-Normalized Matchups inkl. Time-to-Kill, verbleibenden HP, Cost-Efficiency-Index und automatischen **Balance-Ausreißer-Warnungen** bei >2,0× Kosteneffizienz) sowie schnelle Headless-KI-gegen-KI-Skirmish-Simulationen. Stellt den Simulator sowohl als CLI-Subcommand `rts-mod-tools benchmark-balance` als auch als Analyse-Aktion im In-Game Engine Studio bereit.

**Acceptance criteria:**
- [x] `rts-simulation::benchmark::run_cost_normalized_duel` und `run_ai_balance_match` liefern deterministische TTK-, Überlebens-, Kosteneffizienz-Metriken sowie automatische Ausreißer-Warnungen.
- [x] `rts-mod-tools benchmark-balance --mod-path rust-ra2` gibt eine strukturierte Fraktions- und Matchup-Balance-Tabelle inkl. Ausreißer-Markierungen aus.
- [x] Das In-Game Engine Studio bietet einen Button zum Ausführen eines Schnell-Balance-Checks für die aktive Mod.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 126

**Files likely touched:**
- `engine/crates/rts-simulation/src/lib.rs`
- `engine/crates/rts-simulation/src/benchmark.rs`
- `engine/crates/rts-asset-pipeline/src/main.rs`
- `engine/crates/rts-client/src/ui/actions_studio.rs`

**Estimated scope:** Medium (4 files)

---

## Checkpoint: After Tasks 126–127 (Phase 8.3: 3D-Editor v2 & Balance-Prüfstand)
- [x] Symmetrie-Painting, Spawn-/Trigger-Authoring und Map-Playability-Validierung funktionieren End-to-End
- [x] `rts-mod-tools benchmark-balance` und der Studio-Balance-Prüfstand liefern deterministische Kampf- und Wirtschafts-Metriken
- [x] Alle Workspace-Tests und Clippy bestehen fehlerfrei

---

## Task 128: `rts-sdk`, `rts-simulation` & `rust-ra2` — Datengetriebene KI-Profile (`AiProfileDef`), Build-Order-Listen, Squad-Templates & Skript-KI-Hooks (`on_ai_tick`)

**Description:** Führt `AiProfileDef` (`ai/ai_profiles.toml`) in `rts-sdk` ein, womit Mods pro Fraktion und `AiPersonality` eigene Build-Orders, Einheiten-Gewichtungen (`unit_weights`), Mindest-Angriffswellengrößen, bevorzugte Superwaffen-Ziele und einen optionalen `.rhai`/`.lua`-Hook (`on_ai_tick`) definieren können. Erweitert `rts-simulation/src/ai/mod.rs` um die Auswertung dieser Profile (mit automatischem Fallback auf die generischen Rollen-Heuristiken) und hinterlegt authentische Profile für Allies, Soviets und Yuri in `rust-ra2/ai/ai_profiles.toml`.

**Acceptance criteria:**
- [x] `LoadedMod` lädt und validiert `ai/ai_profiles.toml` (`AiProfileCatalog`).
- [x] `generate_ai_commands_with_profile` priorisiert die `build_order` und `unit_weights` des passenden `AiProfileDef` und führt bei Bedarf `on_ai_tick` aus.
- [x] `rust-ra2/ai/ai_profiles.toml` definiert spezifische Profile für Allies, Soviets und Yuri.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 125

**Files likely touched:**
- `engine/crates/rts-sdk/src/schema/world_defs.rs`
- `engine/crates/rts-sdk/src/loader.rs`
- `engine/crates/rts-sdk/src/validator.rs`
- `engine/crates/rts-simulation/src/ai/mod.rs`
- `rust-ra2/ai/ai_profiles.toml`

**Estimated scope:** Medium (5 files)

---

## Task 129: `rts-simulation` & `rts-client` — Immer-aktives Group-Path-Caching & Boid-Separation-Steering + schaltbare Formations-Geschwindigkeitskopplung (`formation_locked`)

**Description:** Optimiert die Gruppenbewegung und Wegfindung in `rts-simulation`: Einheiten derselben `MovementDomain` in einem gemeinsamen Start-Cluster teilen sich bei Gruppen-Bewegungsbefehlen immer den berechneten A*-Leitpfad (Group-Path-Caching) und nutzen deterministisches lokales Boid-Separation-Steering in `world/movement.rs`, um Staus an Rampen und Brücken zu vermeiden. Zusätzlich unterstützt `MoveUnits` / `SimEntity` eine per HUD-Toggle / `Ctrl + Rechtsklick` schaltbare Formations-Geschwindigkeitskopplung (`formation_locked` / `formation_speed_cap`), sodass schnelle Eskorten im Formations-Modus beim langsamen Konvoi bleiben, während normales Micro ungedrosselt bleibt.

**Acceptance criteria:**
- [x] Gruppen-Bewegungsbefehle für nahe beieinander stehende Einheiten derselben Bewegungsdomäne nutzen immer einen gemeinsamen Leitpfad-Cache statt $N$ redundanter Voll-A*-Suchen.
- [x] Bei aktiviertem Formations-Modus (`formation_locked`) bewegen sich Einheiten kohäsiv mit dem Tempo des langsamsten Gruppenmitglieds; ohne Formations-Modus behält jede Einheit ihre natürliche Höchstgeschwindigkeit.
- [x] Lokales Separation-Steering hält Einheiten in Engpässen überlappungsfrei und wahrt zu 100% den Lockstep-Determinismus (`state_hash()`).

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 128

**Files likely touched:**
- `engine/crates/rts-simulation/src/pathfinding.rs`
- `engine/crates/rts-simulation/src/world/mod.rs`
- `engine/crates/rts-simulation/src/world/commands_tactical.rs`
- `engine/crates/rts-simulation/src/world/movement.rs`

**Estimated scope:** Medium (4 files)

---

## Checkpoint: After Tasks 128–129 (Phase 8.4: Datengetriebene KI & Formations-Pathfinding)
- [x] Datengetriebene KI-Profile (`ai_profiles.toml`) und `on_ai_tick`-Hooks steuern Fraktions-KI
- [x] Group-Path-Caching, Formations-Kohäsion und Boid-Steering laufen deterministisch ohne Desyncs
- [x] Alle Workspace-Tests und Clippy bestehen fehlerfrei

---

## Task 130: `rts-sdk`, `rts-simulation` & `rts-client` — Kampagnen-Direktor: Stateful Trigger-Variablen, Kombinatorische Bedingungen (`All`/`Any`/`Not`), Kamera-Direktor & Volle 1:1 Rhai/Lua-Parität

**Description:** Erweitert das Kampagnen- und Missions-Trigger-System um benannte Variablen (`SimWorld.trigger_vars: BTreeMap<String, i64>`), kombinatorische Trigger-Bedingungen (`VariableAtLeast`, `VariableEquals`, `All`, `Any`, `Not`), neue Direktor-Aktionen (`SetVariable`, `AddVariable`, `FocusCamera`, `EvaTransmission`) sowie eine Multi-Action-Queue (`queued_actions: Vec<TriggerAction>`) auf `WorldEventContext` mit **voller 1:1-Parität zwischen `.rhai` und `.lua`** (`lua_runtime.rs`), damit beide Skriptsprachen komplexe mehrstufige Missionsabläufe, Variablen und Kamerafahrten identisch steuern können.

**Acceptance criteria:**
- [x] `TriggerCondition` unterstützt `VariableAtLeast`, `VariableEquals`, `All`, `Any`, `Not` und `TriggerAction` unterstützt `SetVariable`, `AddVariable`, `FocusCamera`, `EvaTransmission`.
- [x] Sowohl `.rhai`- als auch `.lua`-Welt-Hooks haben volle 1:1-Parität auf `WorldEventContext` (`trigger_vars` lesen/schreiben und mehrere `TriggerAction`-Befehle in einem Aufruf zurückgeben).
- [x] `rts-client` wertet `FocusCamera`- und `EvaTransmission`-Events der Simulation für Kameraschwenks und HUD-Übertragungen aus.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 129

**Files likely touched:**
- `engine/crates/rts-sdk/src/schema/world_defs.rs`
- `engine/crates/rts-sdk/src/scripting/mod.rs`
- `engine/crates/rts-simulation/src/world/triggers.rs`
- `engine/crates/rts-client/src/ui/hud_update.rs`

**Estimated scope:** Medium (4 files)

---

## Task 131: `rts-sdk` & `rts-asset-pipeline` — Automatischer Schema-/Markdown-Referenz-Generator (`rts-mod-tools generate-docs`) & Modder-Handbuch (`docs/MOD_DEV_KIT.md`)

**Description:** Implementiert ein Dokumentations- und Schema-Generierungsmodul (`doc_gen.rs`) in `rts-asset-pipeline` sowie das CLI-Subcommand `rts-mod-tools generate-docs`. Erzeugt automatisch das umfassende Modder- und Entwickler-Handbuch `docs/MOD_DEV_KIT.md` mit sämtlichen TOML-Katalog-Schemata (`ModManifest`, `ModRules`, `UnitDef`, `BuildingDef`, `WeaponDef`, `StatusEffectDef`, `EffectPayload`, Trait-Substructs, `AiProfileDef`, `AudioCatalog`, `TriggerCondition`, `TriggerAction`), CLI-Befehlen und `.rhai`/`.lua`-Skript-APIs.

**Acceptance criteria:**
- [x] `rts-mod-tools generate-docs --output docs/MOD_DEV_KIT.md` generiert ein vollständiges, strukturiertes Markdown-Referenzhandbuch.
- [x] Alle Schema-Tabellen, Trait-Substructs, `EffectPayload`-Varianten, Trigger-Typen und Skript-Hooks sind vollständig dokumentiert und durch einen Test gegen Abweichungen geschützt.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 127, Task 130

**Files likely touched:**
- `engine/crates/rts-asset-pipeline/src/lib.rs`
- `engine/crates/rts-asset-pipeline/src/doc_gen.rs`
- `engine/crates/rts-asset-pipeline/src/main.rs`
- `docs/MOD_DEV_KIT.md`

**Estimated scope:** Medium (4 files)

---

## Checkpoint: After Tasks 130–131 (Phase 8 Complete: Full RTS Engine & Dev/Mod Kit)
- [x] Alle Tasks 122–131 sind vollständig implementiert und abgehakt
- [x] `docs/MOD_DEV_KIT.md` ist generiert und aktuell
- [x] `cargo test --manifest-path engine/Cargo.toml --workspace --all-targets` und `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings` bestehen mit `0` Fehlern und `0` Warnungen


---

# Phase 9: Studio-Grade RTS-Engine & Mod-SDK Komplettierung (Tasks 132–141)

## Task 132: `rts-asset-pipeline` & `rts-sdk` — Komplettes Mod-Projekt-Scaffolding (`rts-mod-tools new-mod`) & JSON-Schema/Taplo-Generator (`rts-mod-tools generate-schema`) für IDE-Autocomplete

**Description:** Erweitert `rts-asset-pipeline/src/scaffold.rs` um einen vollständigen Mod-Projekt-Generator (`rts-mod-tools new-mod --id <id> --name <name> [--extends <parent>] --output <dir>`), der wahlweise eine sofort spielbare Standalone-Mod oder eine Sub-Mod (`extends = "rust-ra2"`) mit komplettem Verzeichnisbaum, Startkarte, Fraktionen, Beispiel-Einheiten/Gebäuden, `.glb`-Modellen und Skripten anlegt. Ergänzt außerdem `rts-mod-tools generate-schema --output-dir schemas/`, das JSON-Schema-Dateien (`mod.schema.json`, `units.schema.json`, `buildings.schema.json`, `weapons.schema.json`, `ai_profiles.schema.json`, `map.schema.json`) und eine `.taplo.toml` für sofortiges IDE-Autocomplete erzeugt.

**Acceptance criteria:**
- [x] `rts-mod-tools new-mod` generiert ein vollständiges Mod-Verzeichnis (Standalone oder `extends`), das `LoadedMod::load_from_dir` und `validate_mod` mit `0` Fehlern besteht.
- [x] `rts-mod-tools generate-schema` erzeugt valide JSON-Schema-Dateien für alle TOML-Kataloge sowie eine `.taplo.toml`-Zuordnung.
- [x] Automatisierter Test prüft das Erstellen, Laden und Validieren einer per `new-mod` erzeugten Mod sowie die Schema-Generierung.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-asset-pipeline`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 131

**Files likely touched:**
- `engine/crates/rts-asset-pipeline/src/scaffold.rs`
- `engine/crates/rts-asset-pipeline/src/doc_gen.rs`
- `engine/crates/rts-asset-pipeline/src/main.rs`

**Estimated scope:** Medium (3 files)

---

## Task 133: `rts-sdk` — Semantischer Mod- & Tech-Tree-Linter (`UNREACHABLE_PREREQUISITE`, `ZERO_EFFECTIVE_DAMAGE`, `ORPHANED_WEAPON`, `ORPHANED_STATUS_EFFECT`)

**Description:** Erweitert `validate_mod` in `engine/crates/rts-sdk/src/validator.rs` um semantische Design- und Tech-Tree-Prüfungen: erkennt unerreichbare Tech-Tree-Abhängigkeiten (`UNREACHABLE_PREREQUISITE`, wenn ein benötigtes Gebäude weder baubar noch ein Basis-Startgebäude ist), Waffen mit `damage > 0.0`, deren sämtliche Warhead-Multiplikatoren `<= 0.0` sind (`ZERO_EFFECTIVE_DAMAGE`), sowie verwaiste Waffen (`ORPHANED_WEAPON`) und Status-Effekte (`ORPHANED_STATUS_EFFECT`), die nirgends im Mod referenziert werden.

**Acceptance criteria:**
- [x] `validate_mod` meldet `UNREACHABLE_PREREQUISITE` und `ZERO_EFFECTIVE_DAMAGE` als Fehler/Warnung sowie `ORPHANED_WEAPON` und `ORPHANED_STATUS_EFFECT` als Warnungen.
- [x] `rust-ra2` besteht `validate_mod` weiterhin mit `0` Fehlern.
- [x] Unit-Tests in `rts-sdk/src/validator.rs` verifizieren alle neuen semantischen Lint-Regeln.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml -p rts-sdk`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 132

**Files likely touched:**
- `engine/crates/rts-sdk/src/validator.rs`

**Estimated scope:** Small (1-2 files)

---

## Checkpoint: After Tasks 132–133 (Phase 9.1: Mod-Scaffolding, IDE-Schemas & Semantischer Linter)
- [x] `new-mod` erzeugt sofort valide Standalone- und Sub-Mods
- [x] JSON-Schemas und semantische Tech-Tree-Lints sind getestet und grün
- [x] Alle Workspace-Tests und Clippy bestehen fehlerfrei

---

## Task 134: `rts-simulation` & `rts-network` — Granulare `SubsystemHashes` & deterministischer Desync-Differ (`SimWorld::diff_desync`) auf Entity- und Feld-Ebene

**Description:** Erweitert `SimWorld` um `SimWorld::subsystem_hashes(&self) -> SubsystemHashes` (getrennte 64-Bit-FNV-1a-Hashes für `economy`, `entities_transform`, `entities_combat`, `resources_and_mines`, `projectiles`, `triggers`) sowie `SimWorld::diff_desync(&self, other: &SimWorld) -> Vec<String>`, das bei einem Lockstep-Desync exakt auflistet, welches Subsystem, welcher Spieler, welche `entity_id` (Position, HP, Buffs, Cooldowns) oder welche `trigger_var` zwischen zwei Peers abweicht. Integriert die Subsystem-Diagnose in `LockstepSchedule` (`DesyncAlert`).

**Acceptance criteria:**
- [x] `SimWorld::subsystem_hashes()` berechnet deterministische Teil-Hashes für alle 6 Simulations-Subsysteme.
- [x] `SimWorld::diff_desync(&other)` identifiziert gezielt abweichende Spieler-Ressourcen, Entity-Felder und Trigger-Variablen.
- [x] Unit-Test provoziert künstliche Abweichungen in einzelnen Subsystemen und prüft die exakte Diagnose-Ausgabe.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 133

**Files likely touched:**
- `engine/crates/rts-simulation/src/world/mod.rs`
- `engine/crates/rts-simulation/src/world/movement.rs`
- `engine/crates/rts-network/src/lockstep.rs`

**Estimated scope:** Medium (3 files)

---

## Task 135: `rts-network` & `rts-client` — Keyframe-Snapshotted Fast-Seek Replays, Replay-Telemetrie-Zeitreihen (APM / Economy / Army Value) & Spectator/Caster-HUD

**Description:** Erweitert `ReplayFile` in `rts-network/src/replay.rs` um `content_hash`, Spieler-Metadaten, periodische Telemetrie-Samples (`ReplayTelemetrySample`: Tick, Credits, Einkommen, Army-Value, APM pro Spieler) sowie einen In-Memory-Keyframe-Cache (`ReplayKeyframeIndex`), sodass Vor- und Zurückspulen in Replays vom nächstgelegenen Keyframe startet statt von Tick `0`. Ergänzt in `rts-client` einen Spectator/Caster-POV-Umschalter (`All Vision` vs. Spieler `P1..P8`) sowie eine Live-APM-/Army-Value-Vergleichsanzeige während Replays und im Scoreboard.

**Acceptance criteria:**
- [x] `ReplayFile` bleibt auf Disk kompakt (`tick_commands`, `content_hash`, `players_setup`, `telemetry_samples`) und baut im RAM automatisch einen `ReplayKeyframeCache` (alle 100 Ticks) für schnelles Vor-/Zurückspulen mit identischem `state_hash()` auf.
- [x] `rts-client` erlaubt im Replay- und Spectator-Modus den Perspektivwechsel (`All Vision` / Fog-Off oder einzelne Spieler-Sicht) und zeigt APM / Army-Value an.
- [x] Automatisierter Test prüft In-Memory-Keyframe-Seeking (vorwärts und rückwärts) auf 100 % Hash-Identität.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 134

**Files likely touched:**
- `engine/crates/rts-network/src/replay.rs`
- `engine/crates/rts-client/src/main.rs`
- `engine/crates/rts-client/src/ui/hud_update.rs`

**Estimated scope:** Medium (3-4 files)

---

## Checkpoint: After Tasks 134–135 (Phase 9.2: Desync-Forensik, Keyframe-Replays & Spectator-HUD)
- [x] Subsystem-Hashes und `diff_desync` lokalisieren State-Abweichungen auf Entity-Ebene
- [x] Keyframe-Seeking spult Replays deterministisch vor und zurück
- [x] Alle Workspace-Tests und Clippy bestehen fehlerfrei

---

## Task 136: `rts-simulation` & `rts-client` — Subsystem-Tick-Profiler (`SimTickProfile`), interaktive In-Game Dev-Konsole (`[F9]`) & Single-Tick-Debugger (`step [N]`)

**Description:** Fügt `SimTickProfile` in `rts-simulation` hinzu (misst Laufzeiten in Mikrosekunden für Pathfinding, Movement, Combat, Projectiles, AI und Triggers/FoW pro Tick sowie Entity-/Projektil-/Cache-Metriken). Implementiert in `rts-client` das Dev-Konsolen- & Profiler-Overlay (`[F9]`) sowie einen deterministischen Kommando-Ausführer (`execute_dev_console_command`) für `spawn <unit_id> [count] [player]`, `credits <amount> [player]`, `var <name> <val>`, `trigger <zone_id>`, `heal`, `kill`, `instant_build`, `reveal` und `step [N]` (Einzel-Tick-Stepping bei pausiertem Spiel).

**Acceptance criteria:**
- [x] `SimWorld::step` aktualisiert `last_tick_profile: SimTickProfile` mit Mikrosekunden-Messwerten pro Subsystem.
- [x] `execute_dev_console_command` führt alle Konsolenbefehle (`spawn`, `credits`, `var`, `trigger`, `step`, `instant_build`, `reveal`) im Singleplayer/Studio deterministisch aus und blockiert zustandsverändernde Befehle im Multiplayer-Lockstep (Read-Only Profiler- & Desync-Diagnose-Modus).
- [x] `[F9]` schaltet das Profiler- & Dev-Konsolen-Overlay im Client um, und im Pause-Modus erlaubt `step` / `[.]` exaktes Single-Tick-Stepping.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 135

**Files likely touched:**
- `engine/crates/rts-simulation/src/world/mod.rs`
- `engine/crates/rts-simulation/src/world/step_world.rs`
- `engine/crates/rts-client/src/input/keyboard.rs`
- `engine/crates/rts-client/src/ui/hud_update.rs`

**Estimated scope:** Medium (4 files)

---

## Task 137: `rts-client` — Schaltbare 3D-Debug-Gizmo-Layer (A*-Pfade, Waffen-/Sicht-/Stealth-Ringe, Trigger-Zonen & Footprint-Belegung)

**Description:** Erweitert `rts-client/src/rendering/` und `ClientInteractionState` um granulare 3D-Debug-Visualisierungs-Flags (`DebugGizmoFlags`: `show_paths`, `show_weapon_ranges`, `show_vision_and_stealth`, `show_trigger_zones`, `show_blocked_tiles`), die über die Dev-Konsole (`[F9]`) oder das Studio-Overlay ein- und ausgeschaltet werden können und in `draw_rts_gizmos` farbige 3D-Linien, Ringe und Zonen-Boxen rendern.

**Acceptance criteria:**
- [x] `DebugGizmoFlags` steuert die 3D-Gizmo-Darstellung für A*-Waypoints, Waffen-/Sicht-/Stealth-Radien, Trigger-Zonen und blockierte Footprint-Tiles.
- [x] Umschalten per Dev-Konsole (`debug paths`, `debug ranges`, `debug triggers`, `debug grid`, `debug all`) funktioniert im laufenden Spiel.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 136

**Files likely touched:**
- `engine/crates/rts-client/src/input/mod.rs`
- `engine/crates/rts-client/src/rendering/mod.rs`

**Estimated scope:** Small (2 files)

---

## Checkpoint: After Tasks 136–137 (Phase 9.3: Dev-Konsole, Tick-Profiler & 3D-Debug-Gizmos)
- [x] Subsystem-Tick-Profiler, Dev-Konsole (`[F9]`), Single-Tick-Stepping und 3D-Debug-Gizmos funktionieren End-to-End
- [x] Alle Workspace-Tests und Clippy bestehen fehlerfrei

---

## Task 138: `rts-simulation`, `rts-asset-pipeline` & `rts-client` — Deterministischer Seed-basierter Skirmish-Map-Generator (`generate_procedural_map` & `rts-mod-tools generate-map`)

**Description:** Implementiert `engine/crates/rts-simulation/src/map_gen.rs` mit `generate_procedural_map(config: &MapGenConfig, loaded: &LoadedMod) -> MapDef`. Erzeugt aus einem deterministischen `u64`-Seed, Kartengröße, Spieleranzahl (`2`, `4`, `8`), `MapSymmetryMode` (`MirrorX`, `MirrorZ`, `Rotational2Way`, `Rotational4Way`) und Terrain-Parametern eine sofort spielbare Skirmish-Karte inklusive Startbasen, Erz-/Gem-Feldern, Höhenplateaus mit Rampen, Wasserläufen mit Brücken und Props, die `SimWorld::validate_map_playability` garantiert besteht. Bindet den Generator als `rts-mod-tools generate-map` und im In-Game-Karteneditor ein.

**Acceptance criteria:**
- [x] Gleicher `seed` + `MapGenConfig` erzeugt bit-identische `MapDef`-Strukturen.
- [x] Jede generierte 2P/4P-Karte besteht `SimWorld::validate_map_playability` ohne Fehler (faire Ressourcendistanz und gültige A*-Landwege zwischen allen Spawns).
- [x] `rts-mod-tools generate-map --seed <N> --players <P> --symmetry <mode> --output <path>` speichert die generierte Karte als `.toml`.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 137

**Files likely touched:**
- `engine/crates/rts-simulation/src/lib.rs`
- `engine/crates/rts-simulation/src/map_gen.rs`
- `engine/crates/rts-asset-pipeline/src/main.rs`
- `engine/crates/rts-client/src/ui/actions_studio.rs`

**Estimated scope:** Medium (4 files)

---

## Task 139: `rts-sdk`, `rts-simulation` & `rts-client` — Persistentes Kampagnen-Profil (`CampaignSaveProfile`) mit Missions-Freischaltung sowie Credits- & Veteranen-Carry-Over

**Description:** Führt `CampaignSaveProfile` (`saves/campaign_profile.json`) und `CarryOverUnitSpec` (`unit_id`, `veterancy`, `experience`) ein. Wenn eine Kampagnenmission gewonnen wird (`winner_player_id == Some(local_player_id)`), speichert die Engine automatisch die freigeschaltete Folgemission (`next_mission_id`), berechnet die übernommenen Bonus-Credits (`carry_over_credits_ratio`) und überträgt überlebende Veteranen-/Elite-Einheiten des Spielers (`SimWorld::start_campaign_mission_with_carryover`) in die nächste Mission.

**Acceptance criteria:**
- [x] `SimWorld::collect_mission_carryover` und `SimWorld::apply_mission_carryover` übertragen anteilige Credits (`carry_over_credits_ratio`) und überlebende Veteranen-/Elite-Einheiten deterministisch in die Folgemission.
- [x] `CampaignSaveProfile` lässt sich als JSON speichern/laden und verfolgt abgeschlossene/freigeschaltete Missionen pro Kampagne.
- [x] Unit-Test prüft den kompletten Übergang von Mission 1 zu Mission 2 inklusive Credits- und Elite-Einheiten-Transfer.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 138

**Files likely touched:**
- `engine/crates/rts-sdk/src/schema/world_defs.rs`
- `engine/crates/rts-simulation/src/world/initialization.rs`
- `engine/crates/rts-client/src/ui/menu_screens.rs`

**Estimated scope:** Medium (3 files)

---

## Checkpoint: After Tasks 138–139 (Phase 9.4: Prozeduraler Map-Generator & Kampagnen-Carry-Over)
- [x] Seed-basierter Map-Generator erzeugt playability-validierte symmetrische Karten
- [x] Kampagnen-Missionsfortschritt sowie Credits- und Veteranen-Carry-Over bestehen alle Tests
- [x] Alle Workspace-Tests und Clippy bestehen fehlerfrei

---

## Task 140: `rts-sdk`, `rts-asset-pipeline` & `rts-client` — Datengetriebene 3D-Turret-Rotation, Barrel-Recoil & Rotor/Radar-Spin-Animationen

**Description:** Erweitert `VisualDef` in `rts-sdk/src/schema/common.rs` um optionale artikulierte 3D-Animationsfelder (`#[serde(default)] pub has_turret: bool`, `#[serde(default)] pub barrel_recoil_distance: f32`, `#[serde(default)] pub spin_speed_deg_per_sec: f32`). In `rts-client/src/rendering/` erhalten Einheiten und Verteidigungsgebäude mit Turret/Rotor ein artikuliertes Child-Sub-Mesh, dessen Ausrichtung unabhängig vom Wannen-Heading aus `SimEntity.turret_facing_radians`, `SimEntity.recoil_timer` und kontinuierlicher Rotor-/Radar-Rotation (`spin_speed_deg_per_sec`) animiert wird.

**Acceptance criteria:**
- [x] `VisualDef` unterstützt `has_turret`, `barrel_recoil_distance` und `spin_speed_deg_per_sec` rückwärtskompatibel (`#[serde(default)]`).
- [x] Hybride 3D-Sub-Mesh-Steuerung in `rts-client`: erzeugt/steuert automatisch artikulierte Bevy-Child-Sub-Meshes (`TurretVisualChild` / `RotorVisualChild`) anhand von `turret_facing_radians`, `recoil_timer` und `spin_speed_deg_per_sec` und unterstützt zusätzlich benannte glTF-Nodes (`"Turret"`, `"Barrel"`, `"Rotor"`).
- [x] `rust-ra2` nutzt die neuen Felder für Kampfpanzer, Geschütztürme, Helikopter und Radaranlagen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 139

**Files likely touched:**
- `engine/crates/rts-sdk/src/schema/common.rs`
- `engine/crates/rts-client/src/rendering/mod.rs`
- `rust-ra2/units/units.toml`
- `rust-ra2/buildings/buildings.toml`

**Estimated scope:** Medium (4 files)

---

## Task 141: `rts-simulation`, `rts-asset-pipeline` & `docs/MOD_DEV_KIT.md` — Deklarativer Mod-Szenario-Test-Runner (`rts-mod-tools test-mod`) & Handbuch-Update

**Description:** Implementiert einen deklarativen Szenario-Test-Runner (`rts_simulation::scenario_runner` & `rts-mod-tools test-mod --mod-path <dir>`), mit dem Modder eigene automatisierte Integrations- und Balance-Tests in `tests/*.toml` innerhalb ihres Mod-Ordners definieren können (Spawns, geplante Befehle pro Tick, Simulationsdauer und Assertions wie `assert_winner`, `assert_unit_alive`, `assert_trigger_fired`, `assert_var_equals`, `assert_lockstep_determinism`). Hinterlegt eine Beispiel-Testsuite in `rust-ra2/tests/mod_scenarios.toml` und aktualisiert `doc_gen.rs` sowie `docs/MOD_DEV_KIT.md` um alle neuen Phase-9-Features.

**Acceptance criteria:**
- [x] `rts-mod-tools test-mod --mod-path rust-ra2` führt alle deklarativen Szenarien in `rust-ra2/tests/*.toml` aus und prüft alle Assertions inkl. Lockstep-Determinismus.
- [x] `rts-mod-tools generate-docs --output docs/MOD_DEV_KIT.md` dokumentiert alle Phase-9-Systeme (`new-mod`, `generate-schema`, `generate-map`, `test-mod`, Dev-Konsole `[F9]`, Turret-Sub-Meshes, Kampagnen-Carry-Over).
- [x] Alle Workspace-Tests und Clippy bestehen ohne Warnungen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path engine/Cargo.toml --workspace --all-targets`
- [x] Clippy clean: `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings`

**Dependencies:** Task 140

**Files likely touched:**
- `engine/crates/rts-simulation/src/lib.rs`
- `engine/crates/rts-simulation/src/scenario_runner.rs`
- `engine/crates/rts-asset-pipeline/src/doc_gen.rs`
- `engine/crates/rts-asset-pipeline/src/main.rs`
- `rust-ra2/tests/mod_scenarios.toml`
- `docs/MOD_DEV_KIT.md`

**Estimated scope:** Medium (5-6 files)

---

## Checkpoint: After Tasks 140–141 (Phase 9 Complete: Studio-Grade RTS Engine & Mod SDK)
- [x] Alle Tasks 132–141 sind vollständig implementiert und abgehakt
- [x] `rust-ra2/tests/mod_scenarios.toml` und `docs/MOD_DEV_KIT.md` sind generiert und aktuell
- [x] `cargo test --manifest-path engine/Cargo.toml --workspace --all-targets` und `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings` bestehen mit `0` Fehlern und `0` Warnungen


---

# Phase 10: `rust-ra2` Content-Completion & Client-Polish (Tasks 142–149)

## Task 142: `rust-ra2` — Bereinigung der 2 Linter-Warnungen (`gatling_tank`, `boomer_torpedo_cruise`) & vollständige `[catalogs]`-Deklaration in `rust-ra2/mod.toml`

**Description:** Ergänzt alle expliziten Katalog-Pfade (`upgrades`, `campaigns`, `ai_profiles`, `audio`, `locales_dir`) in `rust-ra2/mod.toml`, benennt `[unit_voices.yuri_gatling_tank]` in `rust-ra2/audio/audio.toml` auf `[unit_voices.gatling_tank]` um und verknüpft `boomer_torpedo_cruise` als `secondary_weapon` auf `boomer_sub` in `rust-ra2/units/units.toml`.

**Acceptance criteria:**
- [x] `rts-mod-tools validate --mod-path rust-ra2` meldet `0 error(s), 0 warning(s)`.
- [x] `rust-ra2/mod.toml` deklariert alle Kataloge explizit.

---

## Task 143: `rust-ra2/campaigns/campaigns.toml` — Vollständige Verdrahtung aller 5 Kampagnen & 40 Missionen

**Description:** Erweitert `rust-ra2/campaigns/campaigns.toml` auf alle 5 Kampagnen (`allied_campaign` mit 12 RA2-Missionen, `soviet_campaign` mit 12 RA2-Missionen, `yr_allied_campaign` mit 7 YR-Missionen, `yr_soviet_campaign` mit 7 YR-Missionen, `bootcamp_campaign` mit 2 Trainingsmissionen) und verknüpft jede der 40 Missionen mit ihrer echten Karte, dem 4K-Loading-Screen (`assets/ui/loading_screens/*.png`), dem 4K-Briefing-/Outro-FMV (`assets/videos/*.mp4`), dem Radar-PIP-Video (`assets/videos/radar_pip/*.mp4`), `next_mission_id` und `carry_over_credits_ratio`.

**Acceptance criteria:**
- [x] Alle 40 Kampagnenkarten in `rust-ra2/maps/` sind in `campaigns.toml` eingebunden.
- [x] Alle referenzierten Karten-, Loading-Screen- und Video-Pfade existieren auf Disk und bestehen `rts-mod-tools validate --mod-path rust-ra2`.

---

## Task 144: `rust-ra2/maps/` — Mehrstufige Kampagnen-Trigger, Spezialziele & Verstärkungswellen

**Description:** Erweitert die Kampagnenkarten in `rust-ra2/maps/` um mehrstufige Trigger-Zonen/Bedingungen (`All`/`Any`/`Not`, `ElapsedSecondsAtLeast`, `PlayerOwnsBuildingId`, `MissionVarEquals`), EVA-/Kamera-Direktor-Schritte und Verstärkungswellen (`SpawnReinforcements`, `GrantCredits`, `SetMissionVar`).

**Acceptance criteria:**
- [x] Kampagnenkarten besitzen mehrstufige Story-/Verstärkungs-Trigger und valide Sieg-/Niederlage-Bedingungen.

---

## Checkpoint: After Tasks 142–144 (Phase 10.1 & 10.2: `rust-ra2` Zero-Warnings & 40-Missionen-Kampagnen)
- [x] `rts-mod-tools validate --mod-path rust-ra2` liefert `0 error(s), 0 warning(s)`
- [x] Alle 40 Kampagnenmissionen sind vollständig in `rust-ra2/campaigns/campaigns.toml` verdrahtet

---

## Task 145: `rts-client` — Kampagnen-Menü mit 5-Kampagnen-Wähler, Missions-Pagination, Fortschritts-Badges & FMV-Briefing-Buttons

**Description:** Erweitert `MenuScreen::Campaign` in `rts-client/src/ui/menu_screens.rs` um die direkte Auswahl aller 5 Kampagnen in `loaded.campaigns`, seitenweises Durchblättern der Missionen (`CycleCampaignMissionPage`), Anzeige des gespeicherten Kampagnen-Fortschritts (`CampaignSaveProfile`: abgeschlossene Missionen, Bonus-Credits, übernommene Veteranen-Einheiten) sowie Buttons zum Abspielen des Briefing-FMVs und Radar-PIP-Videos.

**Acceptance criteria:**
- [x] Alle 5 Kampagnen und 40 Missionen sind im Kampagnen-Menü erreichbar und startbar.
- [x] Fortschritts-Badges und Briefing-/Radar-Video-Buttons werden angezeigt.

---

## Task 146: `rts-client` — Skirmish-Karten-Browser (14 Skirmish-Karten + Blättern) & 1-Klick Prozeduraler Zufallskarten-Generator

**Description:** Filtert die Kartenliste in `MenuScreen::Skirmish` auf Skirmish-/Multiplayer-Karten (mit Vor-/Zurück-Blättern `CycleSkirmishMapPage` und optionalem Toggle für alle Karten) und fügt den Button `🎲 Prozedurale Skirmish-Karte würfeln (Seed)` (`SidebarAction::GenerateProceduralSkirmishMap`) hinzu, der per `generate_procedural_map` sofort eine neue symmetrische Karte erzeugt und im 3D-Hintergrund-Diorama lädt.

**Acceptance criteria:**
- [x] Alle 14 Skirmish/MP-Karten sind im Skirmish-Menü durchblätterbar.
- [x] `GenerateProceduralSkirmishMap` würfelt deterministisch eine neue symmetrische Karte und aktualisiert die 3D-Vorschau.

---

## Task 147: `rts-client` — In-Game FMV-Video-Starter & Soundtrack-Jukebox in Briefing & Mediathek (`MenuScreen::Theater`)

**Description:** Implementiert `SidebarAction::PlayModVideo(String)` und `SidebarAction::PlayModMusicTrack(String)` in `rts-client` und erweitert `5.3 MOD-ARCHIV & MEDIATHEK` (`MenuScreen::Theater`) um eine interaktive Soundtrack-Jukebox (für die Musiktracks aus `loaded.audio.music_tracks`) sowie einen Cutscene-Browser für die 4K-FMV-Videos.

**Acceptance criteria:**
- [x] Soundtrack-Tracks können direkt aus der Mediathek gestartet werden.
- [x] FMV-/Radar-Videos können aus dem Kampagnen-Briefing und der Mediathek gestartet werden.

---

## Task 148: `rust-ra2/tests/mod_scenarios.toml` — Ausbau der deklarativen Mod-Testsuite (Allies, Soviets, Yuri, Kampagnen-Trigger)

**Description:** Erweitert `rust-ra2/tests/mod_scenarios.toml` auf 4 umfassende Szenarien, die alle 3 Fraktionen (Allies, Soviets, Yuri) sowie Kampagnen-Trigger/Variablen und den Lockstep-Determinismus automatisiert über `rts-mod-tools test-mod --mod-path rust-ra2` testen.

**Acceptance criteria:**
- [x] `rts-mod-tools test-mod --mod-path rust-ra2` führt alle 4 Szenarien erfolgreich aus (`PASS`).

---

## Task 149 / Checkpoint: Phase 10 Complete (`rust-ra2` & Engine Release-Ready)
- [x] `rts-mod-tools validate --mod-path rust-ra2` liefert `0 error(s), 0 warning(s)`
- [x] `rts-mod-tools test-mod --mod-path rust-ra2` besteht alle 4 Szenarien
- [x] `cargo test --manifest-path engine/Cargo.toml --workspace --all-targets` und `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings` bestehen mit `0` Fehlern und `0` Warnungen

---

## Task 150: `rts-sdk` — Datengetriebenes `FactionHudSkinDef`-Schema (`factions.toml`), `VfxFlipbookDef` (`vfx_catalog`) & Validator-Prüfung

**Description:** Erweitert `FactionDef` in `engine/crates/rts-sdk/src/schema/common.rs` um `hud_skin: Option<FactionHudSkinDef>` für fraktionsspezifische HUD-Texturen (`credits_frame`, `top_buttons_frame`, `options_button`, `diplomacy_button`, `radar_frame`, `sidebar_top`, `repair_button`, `sell_button`, `tab_structure`, `tab_defense`, `tab_infantry`, `tab_unit`, `sidebar_middle`, `sidebar_bottom`, `sidebar_addon`, `power_bar`, `dialog_background`), lädt `assets/vfx/vfx_catalog.json` in `LoadedMod.vfx_catalog` und prüft alle Pfade in `validator.rs` (`MISSING_HUD_ASSET`, `MISSING_VFX_SPRITE_SHEET`).

**Acceptance criteria:**
- [x] `FactionHudSkinDef` und `VfxFlipbookDef` werden geladen und durch `validate_mod` auf existierende Dateien geprüft.

---

## Task 151: `rust-ra2` — Vollständiges Fraktions-HUD-Skinning (`allies`, `soviets`, `yuri`) in `factions.toml`

**Description:** Verdrahtet alle 36 extrahierten Original-RA2-HUD-Texturen aus `rust-ra2/assets/ui/hud/{allied,soviet,yuri}/` (`credits.png`, `top.png`, `optbtn.png`, `diplobtn.png`, `radar.png`/`radary.png`, `side1.png`, `repair.png`, `sell.png`, `tab01..03.png`, `side2.png`, `side3.png`, `addon.png`, `power.png`, `bkgdlg.png`/`bkgdlgy.png`) in `rust-ra2/factions/factions.toml`.

**Acceptance criteria:**
- [x] `rts-mod-tools validate --mod-path rust-ra2` besteht mit `0 error(s), 0 warning(s)`.

---

## Task 152: `rts-client` — Authentische rechte RA2-Kommando-Sidebar (`credits` → `top` → `radar` → `side1` → `side2` → `side3`), vertikaler Power-Balken & Full-Bleed Cameos

**Description:** Baut die rechte Kommando-Sidebar (`268px` Breite) in `rts-client/src/ui/` originalgetreu nach dem Vorbild von *Alarmstufe Rot 2* auf:
1. Oben `credits.png` mit digitalem `$ 10000` Credits-Zähler,
2. `top.png` mit `optbtn.png` (`OPTIONEN`) und `diplobtn.png` (`DIPLOM.`),
3. `radar.png` / `radary.png` Fraktions-Radar-Rahmen um `RadarMinimapNode` (mit durchscheinendem Fraktions-Emblem),
4. `side1.png` mit `repair.png` (`REPAR. [R]`) / `sell.png` (`VERK. [$]`) und 4 grafischen Kategorie-Tabs (`tab01..03.png`),
5. `side2.png` mit vertikalem `power.png` Energie-Balken links und 2-Spalten **Full-Bleed (`104×74px`) Cameo-Icons**, sodass die eingebrannten Original-RA2-Namen (`BARRACKS`, `POWER PLANT`, `ORE REFINERY`, `WAR FACTORY`, `GRIZZLY TANK`, `CHRONO MINER`) scharf lesbar sind,
6. `side3.png` Abschlussrahmen unten.

**Acceptance criteria:**
- [x] Die rechte Sidebar nutzt die Texturen aus `FactionDef.hud_skin` und zeigt Full-Bleed Cameos ohne überlappende Textboxen.

---

## Task 153: `rts-client` — 100% Vertikale Gefechtsfeld-Höhe im Player Game Mode & `bkgdlg.png` Menü-Karten

**Description:** Blendet im Player Game Mode (`!dev_mode_enabled`) `TopBarRootNode` und `BottomCommandBarRootNode` aus, sodass das 3D-Gefechtsfeld links wie in RA2 100% der vertikalen Bildschirmhöhe einnimmt (während `[F12]` Engine Developer Mode beide Leisten einblendet), und hinterlegt das Hauptmenü mit `hud_skin.dialog_background` (`bkgdlg.png`).

**Acceptance criteria:**
- [x] Im Player Game Mode nimmt das 3D-Gefechtsfeld links die volle Fensterhöhe ein; `[F12]` schaltet Top-Bar und Bottom-Command-Bar im Developer Mode hinzu.

---

## Task 154: `rts-client` — Isometrischer Kamera-Winkel, Gebäude-Ausrichtung (`+38°` `.shp`-Kompensation), Terrain-Schattierung & goldene RA2-Erzkristall-Felder

**Description:** Kompensiert in `entity_sync.rs` den aus `sprite_relief.rs` stammenden `38°`-Yaw-Winkel der `.shp`-Gebäudemodelle (`+38.0_f32.to_radians()`), sodass alle RA2-Gebäude exakt im selben Winkel wie die `.vxl`-Fahrzeuge zur Kamera ausgerichtet sind, verbessert die Terrain-Schattierung (urbane Beton-Basisplatten + mehrstufiges Gras) und rendert jedes Erzkristall-Feld über `build_crystal_cluster_mesh` (`Mesh::ATTRIBUTE_COLOR` + `unlit: true`) als funkelnden goldenen 5-Kristall-Teppich ohne Schatten-Artefakte.

**Acceptance criteria:**
- [x] Alle Gebäude stehen unverzerrt zur Kamera ausgerichtet; alle Erz-Tiles glänzen in kräftigem Gold/Bernstein.

---

## Task 155: `rts-client` — Aufgeräumte Auswahl-Indikatoren im Player Mode & 3D-Sprite-Sheet-VFX (`vfx_catalog.json`)

**Description:** Blendet im Player Game Mode rote Waffenreichweiten-Ringe (`w_def.range`) und gelbe Primärgebäude-Ringe aus (weiterhin aktiv im `[F12]` Developer Mode), behebt Z-Fighting an vollen grünen Lebensbalken in `gizmos_world.rs` und lädt die 16 Sprite-Sheets aus `vfx_catalog.json` als animierte 3D-Flipbook-Billboards (`VfxFlipbookBillboard`) in `vfx/mod.rs` und `vfx/systems.rs`.

**Acceptance criteria:**
- [x] Saubere grüne Auswahlringe und volle grüne HP-Balken im Player Game Mode; 3D-Flipbook-Billboards für Explosionen und Effekte.

---

## Task 156 / Checkpoint: Phase 11 Complete (Authentisches RA2-HUD & Visuelle Validierung)
- [x] Live-GPU-Lauftest (`--visual-smoke-test`) auf AMD Radeon RX 7900 XTX ausgeführt und alle 5 Screenshots (`01`–`05`) visuell verifiziert
- [x] `rts-mod-tools validate --mod-path rust-ra2` liefert `0 error(s), 0 warning(s)`
- [x] `cargo test --manifest-path engine/Cargo.toml --workspace --all-targets` und `cargo clippy --manifest-path engine/Cargo.toml --workspace --all-targets -- -D warnings` bestehen mit `0` Fehlern und `0` Warnungen

