#!/usr/bin/env python3
"""
Headless Blender 5.3 + Cycles HIP GPU Wrapper for `rust-ra2` 3D Asset Factory.

Replaces the legacy procedural NumPy box/voxel generator by delegating all 187 `.glb`
builds (`pilot_6` + 5 Wave-2 subpackages) to `tools/blender_ra2_factory/cli.py`.

Examples:
    python scripts/build_ra2_remastered_models.py --verify-only
    python scripts/build_ra2_remastered_models.py --bucket w2_veh_ground --only prism_tank --no-sheet
    python scripts/build_ra2_remastered_models.py --all --no-sheet
"""

from __future__ import annotations

import os
import subprocess
import sys

DEFAULT_BLENDER_EXE = r"C:\Program Files\Blender Foundation\Blender 5.3\blender.exe"


def main(argv: list[str] | None = None) -> int:
    raw_args = list(sys.argv[1:] if argv is None else argv)
    script_dir = os.path.dirname(os.path.abspath(__file__))
    repo_root = os.path.dirname(script_dir)
    factory_cli = os.path.join(repo_root, "tools", "blender_ra2_factory", "cli.py")
    blender_exe = os.environ.get("BLENDER_EXE", DEFAULT_BLENDER_EXE)

    if not os.path.isfile(blender_exe):
        sys.stderr.write(
            f"[build_ra2_remastered_models] ERROR: Blender 5.3 executable not found at '{blender_exe}'. "
            "Set BLENDER_EXE to override.\n"
        )
        return 1

    if not raw_args:
        forwarded = ["--repo-root", repo_root, "--all", "--no-sheet"]
    elif len(raw_args) == 1 and not raw_args[0].startswith("-") and os.path.isdir(raw_args[0]):
        forwarded = ["--repo-root", os.path.abspath(raw_args[0]), "--all", "--no-sheet"]
    else:
        forwarded = raw_args

    cmd = [
        blender_exe,
        "--background",
        "--factory-startup",
        "--python",
        factory_cli,
        "--",
        *forwarded,
    ]
    print(f"[build_ra2_remastered_models] Launching: {' '.join(cmd)}", flush=True)
    completed = subprocess.run(cmd, cwd=repo_root, check=False)
    return int(completed.returncode)


if __name__ == "__main__":
    raise SystemExit(main())
