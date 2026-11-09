# IRONLOCK // RUST-RTS (`rust-rts.bls-isp.net`)

**Zentrale Projekt-Homepage, deterministisches 20-Hz Lockstep-Multiplayer-Lobby-Register, Reverse-TCP NAT-Traversal-Relay und vollwertiges NethServer 8 (NS8) Rootless-Modul mit strikter SemVer-Versionierung.**

---

## 1. Versionierung & NethServer 8 UI-Updatefähigkeit (Kein `:latest`)

- **Strikte SemVer-Pflicht (`X.Y.Z`)**: Der Tag `:latest` ist in der gesamten Build-Pipeline, in allen Containerfiles, in `imageroot/` und in `repodata.json` **strikt verboten** (wird durch [`scripts/validate_ns8_module.py`](file:///d:/projekt/rust-rts/centralwebserver/scripts/validate_ns8_module.py) automatisiert geprüft).
- **Basis-Version ([`VERSION`](file:///d:/projekt/rust-rts/centralwebserver/VERSION))**: Definiert die aktuelle `MAJOR.MINOR.PATCH`-Basis (z. B. `1.0.0`).
- **Automatische CI-SemVer-Inkrementierung**:
  - Bei einem Git-Tag (z. B. `1.2.0` oder `v1.2.0`) wird exakt diese SemVer-Version gebaut.
  - Bei jedem Push auf `main` vergibt GitLab CI automatisch die streng monoton steigende SemVer-Version `${MAJOR}.${MINOR}.${CI_PIPELINE_IID}` (z. B. `1.0.2`, `1.0.3`, `1.0.4`), sodass sowohl das **NS8-Modul-Image** als auch das eingebettete **Server-Image** (`org.nethserver.images`) denselben eindeutigen SemVer-Tag tragen.
- **2 Wege für das Update über das NethServer 8 UI**:
  1. **Direkt im Modul-UI (`Status`-Seite)**: Im NethServer 8 Cockpit unter *IRONLOCK // RUST-RTS -> Status* zeigt die Karte **„Modul-Update über NethServer 8 UI“** die aktuell installierte SemVer-Version an und erlaubt per 1-Klick das Aktualisieren auf eine neue SemVer-Version (z. B. `1.0.3`).
  2. **Über das NethServer 8 Software Center (`repodata.json`)**: [`scripts/generate_repodata.py`](file:///d:/projekt/rust-rts/centralwebserver/scripts/generate_repodata.py) erzeugt bei jedem Build ein vollständiges NS8-Repository-Metadatenverzeichnis in [`repository/repodata.json`](file:///d:/projekt/rust-rts/centralwebserver/repository/repodata.json) (auch ausgeliefert unter `https://rust-rts.bls-isp.net/repository/repodata.json`).

---

## 2. Installation & Update auf NethServer 8

```bash
# 1. Erstinstallation einer spezifischen SemVer-Version auf Node 1
add-module <registry-url>/rust-rts:1.0.3 1

# 2. Modul für rust-rts.bls-isp.net + Let's Encrypt konfigurieren
#    (über das NethServer 8 UI unter Einstellungen oder per CLI):
api-cli run configure-module --agent module/rust-rts1 --data '{
  "host": "rust-rts.bls-isp.net",
  "lets_encrypt": true,
  "http2https": true,
  "ttl_secs": 45,
  "node_id": "bls-isp-eu-central-01",
  "relay_port_start": 30100,
  "relay_port_end": 30139
}'

# 3. Update auf eine neue SemVer-Version (über das NS8 UI oder per CLI):
api-cli run update-module --data '{
  "module_url": "<registry-url>/rust-rts:1.0.4",
  "instances": ["rust-rts1"],
  "force": false
}'
```

---

## 3. Lokale Entwicklung & Manuelles Bauen

```bash
# Abhängigkeiten installieren, bauen & Integrationstests ausführen
npm ci
npm test

# NS8-Modul & SemVer-Regeln validieren
python3 scripts/validate_ns8_module.py

# Container-Images mit expliziter SemVer-Version bauen (kein :latest!)
REPOBASE=ghcr.io/bls-isp IMAGETAG=1.0.0 PUSH_IMAGES=1 ./build-images.sh
```
