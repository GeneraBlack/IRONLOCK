# Implementierungsplan (Phase 10): `rust-ra2` Content-Completion & Client-Polish

## Phasen- & Task-Übersicht (Tasks 142–149)

- [x] **Task 142:** `rust-ra2` — Bereinigung der 2 Linter-Warnungen (`gatling_tank`, `boomer_torpedo_cruise`) & vollständige `[catalogs]`-Deklaration in `rust-ra2/mod.toml`
- [x] **Task 143:** `rust-ra2/campaigns/campaigns.toml` — Vollständige Verdrahtung aller 5 Kampagnen und 40 Missionen (*RA2 Allies*, *RA2 Soviets*, *YR Allies*, *YR Soviets*, *Bootcamp*) inkl. 4K-Ladebildern, FMV-/Radar-Videos und Carry-Over
- [x] **Task 144:** `rust-ra2/maps/` — Mehrstufige Kampagnen-Trigger, Spezialziele & Verstärkungswellen für die Kampagnenmissionen
- [x] **Checkpoint 10.1:** `rts-mod-tools validate --mod-path rust-ra2` besteht mit `0` Fehlern und `0` Warnungen bei 40 verdrahteten Kampagnenmissionen
- [x] **Task 145:** `rts-client` — Kampagnen-Menü (`MenuScreen::Campaign`) mit 5-Kampagnen-Wähler, Missions-Seitenblättern, `CampaignSaveProfile`-Fortschritt & FMV-Briefing-Starter
- [x] **Task 146:** `rts-client` — Skirmish-Karten-Browser (Filter auf 14 Skirmish/MP-Karten + Seitenblättern) & 1-Klick Prozeduraler Zufallskarten-Generator (`GenerateProceduralSkirmishMap`)
- [x] **Task 147:** `rts-client` — In-Game FMV-Video- & Soundtrack-Jukebox-Player im Briefing und in `MenuScreen::Theater`
- [x] **Task 148:** `rust-ra2/tests/mod_scenarios.toml` — Ausbau der deklarativen Mod-Testsuite (Allies, Soviets, Yuri, Kampagnen-Trigger)
- [x] **Task 149 / Checkpoint 10.2:** End-to-End-Verifikation (`validate`, `test-mod`, `cargo test`, `cargo clippy -- -D warnings` mit `0` Fehlern und `0` Warnungen)
