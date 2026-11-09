#!/usr/bin/env python3
"""
Generates NethServer 8 Software Center repository metadata (repodata.json)
with strict SemVer versioning (no :latest allowed).
"""

import argparse
import json
import re
import sys
from pathlib import Path

SEMVER_RE = re.compile(
    r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z.-]+))?$"
)


def parse_semver_key(tag: str):
    m = SEMVER_RE.match(tag)
    if not m:
        raise ValueError(f"Ungültige SemVer-Version '{tag}' ('latest' ist verboten)")
    major, minor, patch = int(m.group(1)), int(m.group(2)), int(m.group(3))
    pre = m.group(4)
    return (major, minor, patch, 0 if pre else 1, pre or "")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", default=None, help="SemVer release version (e.g. 1.0.0)")
    parser.add_argument(
        "--source",
        default="reg.bls-isp.net/genera/rust-rts-webserver/rust-rts",
        help="Module image repository",
    )
    parser.add_argument("--server-image", default=None, help="Server image reference with SemVer tag")
    args = parser.parse_args()

    root = Path(__file__).resolve().parent.parent
    version_str = (args.version or (root / "VERSION").read_text(encoding="utf-8")).strip().lstrip("v")
    if version_str.lower() == "latest" or not SEMVER_RE.match(version_str):
        print(f"FEHLER: Ungültige SemVer-Version '{version_str}' ('latest' ist verboten)!", file=sys.stderr)
        return 1

    source = args.source.strip().lower()
    default_srv_base = source.rsplit("/", 1)[0] if "/" in source else source
    server_image = (args.server_image or f"{default_srv_base}/rust-rts-server:{version_str}").strip().lower()
    if ":latest" in server_image or server_image.endswith(":"):
        print(f"FEHLER: Server-Image '{server_image}' darf nicht ':latest' verwenden!", file=sys.stderr)
        return 1

    meta_path = root / "repository" / "rust-rts" / "metadata.json"
    metadata = json.loads(meta_path.read_text(encoding="utf-8"))
    metadata["source"] = source
    metadata["upstream_name"] = f"IRONLOCK // RUST-RTS {version_str}"

    # Keep existing SemVer history from repodata.json if present
    repodata_path = root / "repository" / "repodata.json"
    existing_versions: dict[str, dict] = {}
    if repodata_path.is_file():
        try:
            current_data = json.loads(repodata_path.read_text(encoding="utf-8"))
            if isinstance(current_data, list) and current_data:
                for v in current_data[0].get("versions", []):
                    v_tag = v.get("tag", "")
                    if SEMVER_RE.match(v_tag):
                        existing_versions[v_tag] = v
        except Exception:
            pass

    existing_versions[version_str] = {
        "tag": version_str,
        "testing": "-" in version_str,
        "labels": {
            "org.nethserver.authorizations": "traefik@node:routeadm",
            "org.nethserver.images": server_image,
            "org.nethserver.min-core": "3.0.0",
            "org.nethserver.rootfull": "0",
            "org.nethserver.tcp-ports-demand": "1",
            "org.nethserver.volumes": "rust-rts-data",
            "org.opencontainers.image.version": version_str,
        },
    }

    sorted_versions = [
        existing_versions[k]
        for k in sorted(existing_versions.keys(), key=parse_semver_key, reverse=True)
    ]

    entry = {
        "name": metadata["name"],
        "upstream_name": metadata["upstream_name"],
        "description": metadata["description"],
        "categories": metadata["categories"],
        "authors": metadata["authors"],
        "docs": metadata["docs"],
        "source": source,
        "id": "rust-rts",
        "logo": "logo.png",
        "screenshots": [],
        "versions": sorted_versions,
    }

    rendered_repodata = json.dumps([entry], indent=2, ensure_ascii=False) + "\n"
    rendered_meta = json.dumps(metadata, indent=2, ensure_ascii=False) + "\n"

    logo_src = root / "repository" / "rust-rts" / "logo.png"
    logo_bytes = logo_src.read_bytes() if logo_src.is_file() else None

    for target_meta in [
        root / "ui" / "public" / "metadata.json",
        root / "repository" / "rust-rts" / "metadata.json",
        root / "public" / "repository" / "rust-rts" / "metadata.json",
        root / "rust-rts" / "metadata.json",
    ]:
        target_meta.parent.mkdir(parents=True, exist_ok=True)
        target_meta.write_text(rendered_meta, encoding="utf-8", newline="\n")
        if logo_bytes and target_meta.parent.name == "rust-rts":
            target_logo = target_meta.parent / "logo.png"
            if target_logo != logo_src:
                target_logo.write_bytes(logo_bytes)

    for target_repodata in [
        root / "repository" / "repodata.json",
        root / "public" / "repository" / "repodata.json",
        root / "repodata.json",
    ]:
        target_repodata.parent.mkdir(parents=True, exist_ok=True)
        target_repodata.write_text(rendered_repodata, encoding="utf-8", newline="\n")

    print(f"  [OK] repodata.json aktualisiert für {source}:{version_str} (Server-Image: {server_image})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
