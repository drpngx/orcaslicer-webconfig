#!/usr/bin/env python3
"""Bazel runner for building the portable Tauri v2 desktop application around the Bazel-built WASM dist/ bundle."""

import os
import shutil
import struct
import subprocess
import sys
import zlib
from pathlib import Path


def make_png(width: int, height: int) -> bytes:
    raw = b"".join(b"\x00" + b"\xf9\x73\x16\xff" * width for _ in range(height))

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw))
        + chunk(b"IEND", b"")
    )


def find_runfile_or_bazel_bin(runfiles_dir: Path, root_dir: Path, rel_path: str) -> Path:
    candidates = [
        runfiles_dir / rel_path,
        runfiles_dir / "_main" / rel_path,
        runfiles_dir / "orcaslicer_webconfig" / rel_path,
        root_dir / "bazel-bin" / rel_path,
        root_dir / rel_path,
    ]
    for candidate in candidates:
        if candidate.exists():
            return candidate.resolve()
    raise FileNotFoundError(f"Could not locate Bazel artifact '{rel_path}' in {candidates}")


def stage_dist(runfiles_dir: Path, root_dir: Path) -> dict[str, str]:
    dist_dir = root_dir / "dist"
    (dist_dir / "doc" / "images").mkdir(parents=True, exist_ok=True)
    shutil.copy2(
        find_runfile_or_bazel_bin(runfiles_dir, root_dir, "static/index.html"),
        dist_dir / "index.html",
    )
    shutil.copy2(
        find_runfile_or_bazel_bin(runfiles_dir, root_dir, "app_wasm/app_wasm.js"),
        dist_dir / "app_wasm.js",
    )

    try:
        wasm_file = find_runfile_or_bazel_bin(
            runfiles_dir, root_dir, "app_wasm/app_wasm_bg_opt.wasm"
        )
    except FileNotFoundError:
        wasm_file = find_runfile_or_bazel_bin(
            runfiles_dir, root_dir, "app_wasm/app_wasm_bg.wasm"
        )
    shutil.copy2(wasm_file, dist_dir / "app_wasm_bg.wasm")

    try:
        wasm_br = find_runfile_or_bazel_bin(
            runfiles_dir, root_dir, "app_wasm/app_wasm_bg_opt.wasm.br"
        )
        shutil.copy2(wasm_br, dist_dir / "app_wasm_bg_opt.wasm.br")
    except FileNotFoundError:
        pass

    doc_images = root_dir / "doc" / "images"
    if doc_images.is_dir():
        for item in doc_images.iterdir():
            if item.is_file():
                shutil.copy2(item, dist_dir / "doc" / "images" / item.name)

    (dist_dir / ".nojekyll").write_text("", encoding="utf-8")
    (dist_dir / "_headers").write_text(
        "/*.wasm\n  Content-Type: application/wasm\n  Cache-Control: public, max-age=3600, must-revalidate\n",
        encoding="utf-8",
    )
    print(f"Staged Bazel WASM frontend into {dist_dir}")

    return {
        "ORCASLICER_GENERATED_CATALOG": str(
            find_runfile_or_bazel_bin(
                runfiles_dir, root_dir, "orcaslicer_generated_catalog.json"
            )
        ),
        "IMPORTED_VENDOR_CATALOG": str(
            find_runfile_or_bazel_bin(
                runfiles_dir, root_dir, "imported_vendor_catalog.json"
            )
        ),
        "COMPILED_DOCS_JSON": str(
            find_runfile_or_bazel_bin(runfiles_dir, root_dir, "compiled_docs.json")
        ),
    }


def main() -> int:
    runfiles_dir = Path.cwd().resolve()
    workspace = os.environ.get("BUILD_WORKSPACE_DIRECTORY")
    root_dir = Path(workspace).resolve() if workspace else Path(__file__).resolve().parent.parent
    os.chdir(root_dir)

    catalog_env = stage_dist(runfiles_dir, root_dir)
    env = os.environ.copy()
    env.update(catalog_env)

    icon_ico = root_dir / "src-tauri" / "icons" / "icon.ico"
    if not icon_ico.exists():
        icons_dir = root_dir / "src-tauri" / "icons"
        icons_dir.mkdir(parents=True, exist_ok=True)
        base_png = icons_dir / "icon-base.png"
        base_png.write_bytes(make_png(512, 512))
        subprocess.run(["cargo", "tauri", "icon", str(base_png)], env=env, check=True)

    cmd = [
        "cargo",
        "tauri",
        "build",
        "--config",
        '{"build":{"beforeBuildCommand":""}}',
        *sys.argv[1:],
    ]
    return subprocess.call(cmd, env=env)


if __name__ == "__main__":
    sys.exit(main())
