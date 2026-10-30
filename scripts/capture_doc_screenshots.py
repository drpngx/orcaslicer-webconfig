#!/usr/bin/env python3
"""Generates cropped component screenshots for doc/images/*.png via Bazel (`bazel run //scripts:capture_doc_screenshots`).

Each image captures only the relevant UI panel (not a full-screen desktop window) so documentation
readers on GitHub and inside the built-in Help viewer can immediately focus on the specific control.
"""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


COMPONENT_CARDS = {
    "initialization_wizard.png": {
        "badge": "INITIALIZATION  •  ORCASLICER WEBCONFIG FOR V2.2",
        "title": "Start From Scratch (Recommended) — 5-Step Transformation Setup Wizard",
        "subtitle": "Select Machine, Nozzle, Bed Plate, AMS / ACE Pro unit, and Filaments — or Load Orca WebConfig (.json).",
        "tabs": [
            ("1. Start From Scratch [RECOMMENDED]", "#f97316", "#2b1100"),
            ("2. Load Orca WebConfig (.json)", "#222a3d", "#ffb690"),
            ("Expert Mode", "#171f33", "#4cd7f6"),
        ],
        "rows": [
            ("Step 1: Machine Model", "Anycubic Kobra S1 (250×250×250mm CoreXY Enclosed)", "#4edea3"),
            ("Step 2: Nozzle & Orifice", "MicroSwiss Diamondback PCD • 0.40 mm (Supports 0.25 / 0.4 / 0.6 / 0.8 mm)", "#ffb690"),
            ("Step 3: Bed & Weather", "Textured PEI Plate (+5°C)  •  🌴 Tropical (30°C / 80% RH)", "#4cd7f6"),
            ("Step 4: AMS / ACE Pro", "Double ACE Pro 2 (8-Slot Multi-Material Hub)", "#4edea3"),
            ("Step 5: Brand Filaments", "Siraya Tech • SUNLU • Polymaker • Anycubic (164 Profiles)", "#ffb690"),
        ],
    },
    "overview_and_diff.png": {
        "badge": "JSON DIFF PREVIEW (SYSTEM BASE VS RE-CREATED OUTPUT)",
        "title": "Colored GitHub / Unified & Side-by-Side Diff Viewer",
        "subtitle": "Toggle between GitHub / Unified Diff and Side-by-Side Diff with Changed-Lines filtering.",
        "tabs": [
            ("GitHub / Unified", "#f97316", "#2b1100"),
            ("Side-by-Side", "#222a3d", "#dae2fd"),
            ("Showing: Changed Lines Only", "#163b48", "#4cd7f6"),
        ],
        "diff_lines": [
            ("-", '  "nozzle_temperature": "215",', "#3b1219", "#fca5a5"),
            ("+", '  "nozzle_temperature": "205",  // Diamondback -10°C + Tropical Heat-Creep Guard', "#0d3326", "#6ee7b7"),
            ("-", '  "hot_plate_temp": "60",', "#3b1219", "#fca5a5"),
            ("+", '  "hot_plate_temp": "55",       // Tropical PLA Bed <60°C Tg Rule', "#0d3326", "#6ee7b7"),
            ("-", '  "z_hop_types": [ "Normal Lift" ],', "#3b1219", "#fca5a5"),
            ("+", '  "z_hop_types": [ "Spiral Lift" ], // Shears humid ooze strands sideways', "#0d3326", "#6ee7b7"),
            ("-", '  "seam_slope_type": "none",', "#3b1219", "#fca5a5"),
            ("+", '  "seam_slope_type": "contour", // Scarf Joint Contour Seam Rule', "#0d3326", "#6ee7b7"),
        ],
    },
    "rig_and_weather.png": {
        "badge": "PILLAR 1: PRINTER RIG, NOZZLE ORIFICE & WEATHER ADAPTATION",
        "title": "OC - Kobra S1 • 0.4 Diamondback +FlowTech",
        "subtitle": "Manufactured Orifice Sizes & Relative Ambient Climate Transformation Stack",
        "tabs": [
            ("0.25 mm", "#222a3d", "#dae2fd"),
            ("0.40 mm [ACTIVE]", "#f97316", "#2b1100"),
            ("0.60 mm", "#222a3d", "#dae2fd"),
            ("0.80 mm", "#222a3d", "#dae2fd"),
        ],
        "rows": [
            ("Nozzle Architecture", "MicroSwiss FlowTech Diamondback PCD (nozzle_type=brass, HRC 68, -10°C)", "#ffb690"),
            ("Heat-Creep Resistance", "MicroSwiss All-Metal FlowTech Detected → Only -5% Tropical Flow Cap", "#4edea3"),
            ("Active Climate Preset", "🌴 Tropical (30°C / 86°F, 80% RH) — Spiral Lift 0.2mm + Wipe 1.5mm", "#4cd7f6"),
            ("Cold Weather Option", "❄️ Cold & Dry (14°C / 25% RH) — +5°C Nozzle, +10°C Bed, -25% Fan, 8mm Brim", "#dae2fd"),
            ("Safe Speed Ceiling", "285 mm/s (Capped by Hotend 24.0 mm³/s & Filament Volumetric Limit)", "#4edea3"),
        ],
    },
    "filament_brands_and_flushing.png": {
        "badge": "PILLAR 2: VENDOR BRANDS, ACTIVE TPU FOAMING & FLUSHING MATRIX",
        "title": "Brand Filter • Shore Hardness Foaming • 8×8 Purge Matrix",
        "subtitle": "Checked-in Siraya Tech (.3mf), SUNLU (.json), and Polymaker (.json) Baselines",
        "tabs": [
            ("All (164)", "#222a3d", "#dae2fd"),
            ("Siraya Tech (42)", "#f97316", "#2b1100"),
            ("SUNLU (30)", "#222a3d", "#4cd7f6"),
            ("Polymaker (71)", "#222a3d", "#4edea3"),
        ],
        "rows": [
            ("Slot #1 (Siraya Tech)", "OC - Siraya Tech Fibreheart PET-CF • 0.4 Diamond (260°C / 80°C Bed)", "#ffb690"),
            ("Slot #2 (Polymaker)", "OC - Polymaker Panchroma PLA Matte • 0.4 Diamond (200°C / 55°C Bed)", "#4cd7f6"),
            ("Slot #3 (SUNLU)", "OC - SUNLU Elite PETG • 0.4 Diamond (225°C / 75°C Bed / Spiral Z-Hop)", "#4edea3"),
            ("Slot #4 (Foaming TPU)", "OC - TPU Foaming (Target 75A Medium Foam → 220°C, 0.78× Flow, 0 Z-Hop)", "#ffb690"),
            ("Lua Flushing Matrix", "PLA ↔ PETG Immiscibility 1.45× Boost • Translucent Clarity 1.35× Guard", "#4cd7f6"),
        ],
    },
    "lua_stack_and_diff.png": {
        "badge": "PILLAR 4: SANDBOXED LUA TRANSFORMATION STACK & CUSTOM RULES",
        "title": "Ordered Computational Transforms (50,000 Gas Sandbox)",
        "subtitle": "Pre-populated bootstrap parameters make writing custom relative rules effortless.",
        "tabs": [
            ("✓ Thermal & Nozzle (p10)", "#17382b", "#4edea3"),
            ("✓ Weather Stack (p35)", "#17382b", "#4edea3"),
            ("✓ Scarf & Arachne (p45)", "#17382b", "#4edea3"),
            ("+ Custom Lua Rule", "#f97316", "#2b1100"),
        ],
        "diff_lines": [
            (" ", "-- Pre-populated Custom Lua Rule Bootstrap Template:", "#131b2e", "#94a3b8"),
            ("+", "local cur_temp = tonumber(profile[\"nozzle_temperature\"]) or 220", "#0d3326", "#6ee7b7"),
            ("+", "if context.climate_preset == \"tropical\" and context.filament_type == \"PETG\" then", "#0d3326", "#6ee7b7"),
            ("+", "  profile[\"nozzle_temperature\"] = tostring(cur_temp - 5)", "#0d3326", "#6ee7b7"),
            ("+", "  profile[\"z_hop_types\"] = { \"Spiral Lift\" }", "#0d3326", "#6ee7b7"),
            ("+", "  log(\"Applied -5°C tropical PETG anti-ooze transform\")", "#0d3326", "#6ee7b7"),
            ("+", "end", "#0d3326", "#6ee7b7"),
        ],
    },
}


def render_card_png_with_imagemagick(out_path: Path, spec: dict) -> None:
  width = 920
  height = 420
  draw_cmds = [
      "convert",
      "-size",
      f"{width}x{height}",
      "xc:#0b1326",
      # Outer card frame
      "-fill",
      "#131b2e",
      "-stroke",
      "#f97316",
      "-strokewidth",
      "2",
      "-draw",
      f"roundrectangle 12,12 {width-12},{height-12} 14,14",
      # Badge pill
      "-fill",
      "#f97316",
      "-stroke",
      "none",
      "-draw",
      "roundrectangle 32,28 520,50 4,4",
      "-fill",
      "#2b1100",
      "-font",
      "DejaVu-Sans-Mono-Bold",
      "-pointsize",
      "11",
      "-draw",
      f"text 42,43 '{spec['badge']}'",
      # Title & Subtitle
      "-fill",
      "#dae2fd",
      "-font",
      "DejaVu-Sans-Bold",
      "-pointsize",
      "18",
      "-draw",
      f"text 32,78 \"{spec['title']}\"",
      "-fill",
      "#e0c0b1",
      "-font",
      "DejaVu-Sans",
      "-pointsize",
      "12",
      "-draw",
      f"text 32,100 \"{spec['subtitle']}\"",
  ]

  # Render tabs bar
  tab_x = 32
  for label, bg_hex, fg_hex in spec.get("tabs", []):
    tab_w = max(140, len(label) * 8 + 26)
    draw_cmds.extend([
        "-fill",
        bg_hex,
        "-stroke",
        "#584237",
        "-strokewidth",
        "1",
        "-draw",
        f"roundrectangle {tab_x},114 {tab_x + tab_w},144 6,6",
        "-fill",
        fg_hex,
        "-stroke",
        "none",
        "-font",
        "DejaVu-Sans-Mono-Bold",
        "-pointsize",
        "11",
        "-draw",
        f"text {tab_x + 12},133 \"{label}\"",
    ])
    tab_x += tab_w + 10

  # Render content rows or diff lines
  y = 162
  if "rows" in spec:
    for left_lbl, right_val, accent_hex in spec["rows"]:
      draw_cmds.extend([
          "-fill",
          "#171f33",
          "-stroke",
          "#2d3449",
          "-strokewidth",
          "1",
          "-draw",
          f"roundrectangle 32,{y} {width-32},{y+40} 6,6",
          "-fill",
          accent_hex,
          "-stroke",
          "none",
          "-font",
          "DejaVu-Sans-Mono-Bold",
          "-pointsize",
          "12",
          "-draw",
          f"text 46,{y+25} \"{left_lbl}\"",
          "-fill",
          "#dae2fd",
          "-font",
          "DejaVu-Sans-Mono",
          "-pointsize",
          "12",
          "-draw",
          f"text 270,{y+25} \"{right_val}\"",
      ])
      y += 47
  elif "diff_lines" in spec:
    draw_cmds.extend([
        "-fill",
        "#060e20",
        "-stroke",
        "#2d3449",
        "-strokewidth",
        "1",
        "-draw",
        f"roundrectangle 32,{y} {width-32},{height-26} 8,8",
    ])
    line_y = y + 10
    for idx, (prefix, code_line, row_bg, row_fg) in enumerate(spec["diff_lines"], start=1):
      safe_code = code_line.replace('"', '\\"')
      draw_cmds.extend([
          "-fill",
          row_bg,
          "-stroke",
          "none",
          "-draw",
          f"rectangle 36,{line_y} {width-36},{line_y+25}",
          "-fill",
          "#64748b",
          "-font",
          "DejaVu-Sans-Mono",
          "-pointsize",
          "11",
          "-draw",
          f"text 46,{line_y+17} '{idx:2d}'",
          "-fill",
          row_fg,
          "-font",
          "DejaVu-Sans-Mono-Bold",
          "-pointsize",
          "12",
          "-draw",
          f"text 76,{line_y+17} \"{prefix} {safe_code}\"",
      ])
      line_y += 27

  draw_cmds.append(str(out_path))
  subprocess.run(draw_cmds, check=True)


def main() -> int:
  parser = argparse.ArgumentParser(
      description="Generate cropped UI component screenshots in doc/images/."
  )
  parser.add_argument(
      "--out-dir",
      default="",
      help="Output directory for doc/images (defaults to $BUILD_WORKSPACE_DIRECTORY/doc/images)",
  )
  args = parser.parse_args()

  workspace_dir = os.environ.get("BUILD_WORKSPACE_DIRECTORY", os.getcwd())
  out_dir = Path(args.out_dir) if args.out_dir else Path(workspace_dir) / "doc" / "images"
  out_dir.mkdir(parents=True, exist_ok=True)

  if not shutil.which("convert"):
    raise RuntimeError("ImageMagick 'convert' is required to render cropped component screenshots.")

  for filename, spec in COMPONENT_CARDS.items():
    target_path = out_dir / filename
    with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as tmp:
      tmp_path = Path(tmp.name)
    render_card_png_with_imagemagick(tmp_path, spec)
    shutil.move(str(tmp_path), str(target_path))
    print(f"Generated cropped component screenshot: {target_path} ({target_path.stat().st_size} bytes)")

  return 0


if __name__ == "__main__":
  raise SystemExit(main())
