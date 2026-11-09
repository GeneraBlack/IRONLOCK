# Spec: `rts-network` (`engine/crates/rts-network`) & Central Webserver (`centralwebserver/`)

## Objective
Bereitstellung eines deterministischen Lockstep-Multiplayer-, Replay-, NAT/UPnP-Traversal- und Online-Lobby-Subsystems ([`engine/crates/rts-network`](file:///d:/projekt/rust-rts/engine/crates/rts-network/src/lib.rs)), das nahtlos mit dem zentralen Node.js/TypeScript-Webserver ([`centralwebserver/`](file:///d:/projekt/rust-rts/centralwebserver/src/server.ts)) unter **`https://rust-rts.bls-isp.net/`** (bzw. lokalem Fallback `http://127.0.0.1:8080`) zusammenarbeitet.

## Kernfunktionen
1. **Mod-Kompatibilitäts-Handshake (`ClientHello` / `ServerWelcome` in [`protocol.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-network/src/protocol.rs)):**
   - Prüft beim Verbindungsaufbau automatisch, ob Host und Client exakt dieselbe Mod (`mod_id` + `mod_version`) geladen haben, und weist dem Client eine `player_id` zu.
2. **Deterministischer Lockstep-Scheduler ([`lockstep.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-network/src/lockstep.rs)):**
   - Puffert lokale Befehle mit konfigurierbarem `input_delay_ticks` (Standard: 2 Ticks = 100 ms bei 20 Hz).
   - Führt einen Simulations-Tick erst aus, wenn das autoritative `ServerTickBundle` vorliegt, und gleicht den 64-Bit-`SimWorld::state_hash()` aller Peers zur sofortigen Desync-Erkennung ab.
3. **Non-Blocking Host & Client Transport ([`transport.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-network/src/transport.rs)):**
   - Längenpräfixiertes JSON-Protokoll über nicht-blockierende TCP-Sockets (`set_nonblocking(true)`, `set_nodelay(true)`).
   - Unterstützt Multi-Endpoint-Smart-Connect ([`LockstepClient::connect_smart`](file:///d:/projekt/rust-rts/engine/crates/rts-network/src/transport.rs#L172-L202)) sowie ausgehende Reverse-TCP-Relay-Uplinks ([`LockstepServer::enable_relay_uplink`](file:///d:/projekt/rust-rts/engine/crates/rts-network/src/transport.rs#L110-L120)).
4. **Replay-Aufzeichnung & -Wiedergabe ([`replay.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-network/src/replay.rs)):**
   - Speichert alle `GameCommand`-Bundles pro Tick sowie den finalen `state_hash` in `.replay.json` und verifiziert Matches deterministisch.
5. **Online Master-Lobby-Register & Match-Historie ([`lobby_registry.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-network/src/lobby_registry.rs) & [`centralwebserver/`](file:///d:/projekt/rust-rts/centralwebserver/src/server.ts)):**
   - Primäre Produktions-URL: **`http://rust-rts.bls-isp.net`** ([`DEFAULT_CENTRAL_LOBBY_URL`](file:///d:/projekt/rust-rts/engine/crates/rts-network/src/lobby_registry.rs#L17)) mit automatischem Offline-Fallback auf **`http://127.0.0.1:8080`** ([`LOCAL_FALLBACK_LOBBY_URL`](file:///d:/projekt/rust-rts/engine/crates/rts-network/src/lobby_registry.rs#L21)).
   - REST-Endpunkte:
     - `GET /api/health` & `GET /api/stats` — Serverstatus, aktive Lobbys, gespielte Matches.
     - `GET /api/lobbies?mod_id=rust-ra2&only_open=true` — Offene Lobbys abrufen.
     - `POST /api/lobbies` — Lobby registrieren (wertet `Host`, `X-Forwarded-Host`, `X-Forwarded-For` und `X-Real-IP` hinter Reverse-Proxies aus und verhindert Loopback-Kollisionen bei `127.0.0.1:7878`).
     - `POST /api/lobbies/{id}/heartbeat` & `DELETE /api/lobbies/{id}` — Heartbeat & Abmeldung.
     - `GET /api/matches` & `POST /api/matches` — Verifizierte Lockstep-Match- & Replay-Historie ([`LobbyRegistryClient::report_completed_match`](file:///d:/projekt/rust-rts/engine/crates/rts-network/src/lobby_registry.rs#L892-L900)).
6. **3-Stufen NAT- & UPnP-Traversal + Reverse-TCP-Relay-Tunnel ([`nat_upnp.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-network/src/nat_upnp.rs), [`lobby_registry.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-network/src/lobby_registry.rs), [`centralwebserver/src/relay.ts`](file:///d:/projekt/rust-rts/centralwebserver/src/relay.ts)):**
   - **Stufe 1 — Automatische UPnP-IGD-Portfreigabe:** SSDP `M-SEARCH` + SOAP `AddPortMapping` + CGNAT-Erkennung (`100.64.0.0/10`).
   - **Stufe 2 — Multi-Endpoint-Registrierung (`candidate_connect_addrs`):** Priorisiert direkte Verbindung (`external_addr` → `host_addr`) vor Relay (`relay_addr`).
   - **Stufe 3 — Reverse-TCP-Relay-Tunnel (`RelayTunnelRoom`):** Allokiert auf `rust-rts.bls-isp.net` deterministische Relay-Ports im Bereich **`27900–27999`** (`--relay-port-range 27900-27999`) und koppelt Client- und Host-Uplink-Streams nach dem `'R'`-Wakeup-Byte verzögerungsfrei per TCP-Splicing.

## Commands
```powershell
# Rust-Netzwerk- & Relay-Tests ausführen
cargo test --manifest-path engine/Cargo.toml -p rts-network

# Zentralen Node.js/TypeScript Webserver (rust-rts.bls-isp.net) bauen & testen
cd centralwebserver
npm test

# Standalone Rust-Fallback-Lobby-Server starten
cargo run --manifest-path engine/Cargo.toml -p rts-network --bin rts-lobby-server -- --bind 0.0.0.0:8080
```
