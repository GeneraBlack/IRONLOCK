#!/usr/bin/env python3
"""
Validates NethServer 8 module structure, JSON schemas, Python actions,
systemd unit rules, SemVer versioning, and enforces the strict ban on ':latest'.
"""

import ast
import glob
import json
import pathlib
import re
import sys

SEMVER_RE = re.compile(
    r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z.-]+))?$"
)


def fail(msg: str) -> None:
    print(f"FEHLER: {msg}", file=sys.stderr)
    sys.exit(1)


def main() -> int:
    root = pathlib.Path(__file__).resolve().parent.parent

    # 1. Validate VERSION file is strict SemVer
    version_file = root / "VERSION"
    if not version_file.is_file():
        fail("VERSION Datei fehlt!")
    version_str = version_file.read_text(encoding="utf-8").strip()
    if version_str.lower() == "latest" or not SEMVER_RE.match(version_str):
        fail(f"VERSION '{version_str}' ist keine gültige SemVer-Version (Format X.Y.Z, kein 'latest')!")
    print(f"  [OK] VERSION SemVer valide: {version_str}")

    # 2. Rule 1: Never override get-status
    if (root / "imageroot" / "actions" / "get-status").exists():
        fail("imageroot/actions/get-status darf nicht überschrieben werden!")

    # 3. Validate all JSON files
    json_files = (
        glob.glob(str(root / "imageroot/**/*.json"), recursive=True)
        + glob.glob(str(root / "ui/public/**/*.json"), recursive=True)
        + glob.glob(str(root / "repository/**/*.json"), recursive=True)
        + [
            str(root / "public/project-catalog.json"),
            str(root / "package.json"),
            str(root / "ui/package.json"),
        ]
    )
    for jf in json_files:
        with open(jf, "r", encoding="utf-8") as fh:
            json.load(fh)
        print(f"  [OK] JSON valide: {pathlib.Path(jf).relative_to(root)}")

    # 4. Validate Python action & backup scripts + LF line endings
    py_files = [
        root / "imageroot/actions/create-module/10create_environment",
        root / "imageroot/actions/create-module/20initialize",
        root / "imageroot/actions/configure-module/01hostname-validation",
        root / "imageroot/actions/configure-module/20configure",
        root / "imageroot/actions/configure-module/80start_services",
        root / "imageroot/actions/get-configuration/20read",
        root / "imageroot/actions/destroy-module/20destroy",
        root / "imageroot/actions/restore-module/06copyenv",
        root / "imageroot/actions/restore-module/50configure",
        root / "imageroot/actions/clone-module/50configure",
        root / "imageroot/bin/module-dump-state",
        root / "imageroot/bin/module-cleanup-state",
        root / "imageroot/update-module.d/10fix_environment",
    ]
    for pf in py_files:
        content = pf.read_text(encoding="utf-8")
        if "\r\n" in content:
            fail(f"CRLF Zeilenenden in {pf.relative_to(root)} gefunden!")
        ast.parse(content, filename=str(pf))
        if "agent.set_env(" in content and "agent.dump_env()" not in content:
            fail(f"{pf.relative_to(root)} ruft agent.set_env() ohne agent.dump_env() auf!")
        print(f"  [OK] Python Action-/Backup-Skript valide: {pf.relative_to(root)}")

    # 5. Validate systemd unit
    service_path = root / "imageroot/systemd/user/rust-rts.service"
    service_text = service_path.read_text(encoding="utf-8")
    if "127.0.0.1:${TCP_PORT}:8080" not in service_text:
        fail("rust-rts.service muss an 127.0.0.1:${TCP_PORT}:8080 binden!")
    if "--pull=missing" not in service_text:
        fail("rust-rts.service muss --pull=missing verwenden!")
    print("  [OK] systemd Unit imageroot/systemd/user/rust-rts.service valide")

    # 6. Enforce ban on ':latest' tag across build, CI, imageroot, and repodata files
    checked_files = [
        root / "Dockerfile",
        root / "Containerfile.ns8",
        root / ".gitlab-ci.yml",
        root / "repository/repodata.json",
        *list((root / "imageroot").rglob("*")),
    ]
    for cf in checked_files:
        if not cf.is_file():
            continue
        lines = [
            line
            for line in cf.read_text(encoding="utf-8").splitlines()
            if not line.lstrip().startswith("#")
        ]
        text = "\n".join(lines)
        if ":latest" in text:
            fail(f"Verbotener ':latest'-Tag in {cf.relative_to(root)} gefunden! Nur SemVer-Tags sind erlaubt.")
        if 'org.nethserver.flags="core_module"' in text or "core_module" in text:
            fail(f"'{cf.relative_to(root)}' enthält 'core_module' — verhindert Updates über das NethServer 8 UI!")
    print("  [OK] Kein ':latest'-Tag und kein 'core_module'-Flag gefunden (100% SemVer & NS8-UI-updatefähig)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
