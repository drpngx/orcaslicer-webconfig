#!/usr/bin/env python3
"""
Import checked-in vendor filament profiles from `vendor_profiles/`:
- `vendor_profiles/siraya_tech/3mf/*.3mf` (reads `Metadata/project_settings.config` directly from the checked-in `.3mf` ZIP containers!)
- `vendor_profiles/sunlu/SUNLU_*.json`
- `vendor_profiles/polymaker/Polymaker_*.json`

Produces `vendor_profiles/imported_vendor_catalog.json` with:
- `filaments`: list of `CatalogFilament` entries with `brand` (`"Siraya Tech"`, `"SUNLU"`, `"Polymaker"`), `code`, `label`, `category`, `default_finish`, `base_nozzle_c`, `base_bed_c`, `max_vol_mm3s`, `fan_pct`, `default_flow_ratio`, `default_retraction_mm`, `dry_temp_c`, `dry_hr`, `requires_hardened`, `source`
- `vendor_filament_bases`: map of `"<Brand>::<Code>"` -> sanitized OrcaSlicer filament JSON profile map
"""

import argparse
import io
import json
import os
import re
import zipfile
from pathlib import Path

WORKSPACE_DIR = os.environ.get("BUILD_WORKSPACE_DIRECTORY")
ROOT_DIR = Path(WORKSPACE_DIR) if WORKSPACE_DIR else Path(__file__).resolve().parent.parent
VENDOR_DIR = ROOT_DIR / "vendor_profiles"


def first_val(obj: dict, key: str) -> str | None:
  val = obj.get(key)
  if val is None:
    return None
  if isinstance(val, list):
    for item in val:
      if item is not None:
        s = str(item).strip()
        if s and s.lower() != "nil":
          return s
    return None
  s = str(val).strip()
  return s if (s and s.lower() != "nil") else None


def parse_float(s: str | None, default: float) -> float:
  if not s:
    return default
  try:
    return float(s)
  except ValueError:
    return default


def parse_int(s: str | None, default: int) -> int:
  if not s:
    return default
  try:
    return int(round(float(s)))
  except ValueError:
    return default


def sanitize_profile_map(raw: dict) -> dict:
  skip_keys = {
    "setting_id",
    "filament_id",
    "instantiation",
    "compatible_printers_condition",
    "compatible_prints_condition",
    "compatible_prints",
    "compatible_printers",
    "filament_ramming_parameters",
    "small_area_infill_flow_compensation_model",
    "adaptive_pressure_advance_model",
    "head_wrap_detect_zone",
    "bed_exclude_area",
    "post_process",
    "upward_compatible_machine",
    "wiping_volumes_extruders",
    "filament_notes",
    "filename_format",
    "thumbnails",
  }
  keep_prefixes = (
    "nozzle_temperature",
    "hot_plate_temp",
    "textured_plate_temp",
    "cool_plate_temp",
    "eng_plate_temp",
    "filament_",
    "fan_",
    "overhang_fan_",
    "close_fan_the_first_x_layers",
    "full_fan_speed_layer",
    "reduce_fan_stop_start_freq",
    "slow_down_",
    "Activate_air_filtration",
    "complete_print_exhaust_fan_speed",
    " during_print_exhaust_fan_speed",
    "during_print_exhaust_fan_speed",
    "additional_cooling_fan_speed",
    "chamber_temperature",
    "pressure_advance",
    "enable_pressure_advance",
    "required_nozzle_HRC",
  )
  out = {}
  for k, v in sorted(raw.items()):
    if k in skip_keys or k.endswith("_gcode"):
      continue
    if not k.startswith(keep_prefixes) and k not in ("type", "from", "inherits", "name"):
      continue
    if v is None:
      continue
    if isinstance(v, str) and v.strip().lower() == "nil":
      continue
    if isinstance(v, list):
      cleaned = [x for x in v if str(x).strip().lower() != "nil"]
      if not cleaned:
        continue
      out[k] = [str(cleaned[0])]
    else:
      out[k] = v
  return out


def categorize_polymer(code: str, finish_hint: str | None = None) -> tuple[str, str, int, int]:
  up = code.upper()
  # Determine category & drying (dry_temp_c, dry_hr)
  if any(x in up for x in ("TPU", "PEBA", "TPE", "FLEX")):
    cat = "TPU / TPE / PEBA Flex"
    dry_c, dry_h = (60, 8)
  elif any(x in up for x in ("PPA", "PAHT", "PA6", "PA12", "PA612", "EASY PA", "NYLON")) or up.startswith("PA"):
    cat = "Nylon (PA / PAHT / PPA)"
    dry_c, dry_h = (80, 12)
  elif any(x in up for x in ("PEEK", "PPS", "PC", "PP")):
    cat = "PC / Engineering"
    dry_c, dry_h = (85, 8)
  elif any(x in up for x in ("ABS", "ASA")):
    cat = "ABS / ASA"
    dry_c, dry_h = (65, 6)
  elif any(x in up for x in ("PETG", "PCTG", "PET-", "PET ", "COPE")):
    cat = "PETG / PCTG / PET"
    dry_c, dry_h = (60, 6)
  elif any(x in up for x in ("PVA", "BVOH", "SUPPORT", "POLYCAST")):
    cat = "PVA / Support"
    dry_c, dry_h = (50, 8)
  else:
    cat = "PLA Family"
    dry_c, dry_h = (45, 4)

  if finish_hint:
    fin = finish_hint
  elif "AIR" in up or "LW-" in up or "FOAM" in up:
    fin = "Foaming"
  elif "-CF" in up or "RCF" in up:
    fin = "Carbon Fiber"
  elif "-GF" in up:
    fin = "Glass Fiber"
  elif "ESD" in up or "CONDUCTIVE" in up:
    fin = "Conductive"
  elif "GLOW" in up or "LUMINOUS" in up:
    fin = "Glow-in-the-Dark"
  elif "UV SHIFT" in up or "PHOTOCHROMIC" in up:
    fin = "Photochromic"
  elif "MARBLE" in up or "STONE" in up or "TERRACOTTA" in up:
    fin = "Marble/Stone/Terracotta"
  elif "WOOD" in up:
    fin = "Wood-Filled"
  elif "METAL" in up:
    fin = "Metal-Filled"
  elif "SILK" in up:
    fin = "Silk"
  elif "MATTE" in up or "POLYTERRA" in up or "SATIN" in up:
    fin = "Matte"
  elif "TRANSLUCENT" in up or "TRANSPARENT" in up or "CLEAR" in up or "PVB" in up or "POLYSMOOTH" in up:
    fin = "Translucent"
  else:
    fin = "Basic"

  return cat, fin, dry_c, dry_h


def build_entry(
  brand: str,
  code: str,
  label: str,
  raw_cfg: dict,
  source_label: str,
  finish_override: str | None = None,
) -> tuple[dict, dict]:
  cat, fin, dry_c, dry_h = categorize_polymer(code, finish_override)
  # Many Anycubic Kobra S1 Max / Kobra X .3mf files from Siraya Tech put the actual print temperature
  # in `nozzle_temperature_HS` or `nozzle_temperature` while leaving `nozzle_temperature_BRASS` at a
  # generic 205/220°C template default. Prefer `nozzle_temperature` or `nozzle_temperature_HS` first.
  nozzle_c = parse_int(
    first_val(raw_cfg, "nozzle_temperature")
    or first_val(raw_cfg, "nozzle_temperature_HS")
    or first_val(raw_cfg, "nozzle_temperature_BRASS"),
    220,
  )
  nozzle_c = max(165, min(350, nozzle_c))

  bed_c = parse_int(
    first_val(raw_cfg, "textured_plate_temp")
    or first_val(raw_cfg, "hot_plate_temp")
    or first_val(raw_cfg, "eng_plate_temp"),
    60,
  )
  bed_c = max(30, min(125, bed_c))

  flow = parse_float(first_val(raw_cfg, "filament_flow_ratio"), 0.96)
  flow = round(max(0.45, min(1.10, flow)), 3)

  mvs = parse_float(first_val(raw_cfg, "filament_max_volumetric_speed"), 12.0)
  mvs = round(max(1.5, min(40.0, mvs)), 1)

  fan_pct = parse_int(first_val(raw_cfg, "fan_max_speed"), 80)
  fan_pct = max(0, min(100, fan_pct))

  ret_mm = parse_float(
    first_val(raw_cfg, "filament_retraction_length") or first_val(raw_cfg, "retraction_length"),
    0.8 if "PET" in code.upper() else (0.0 if fin == "Foaming" else 0.6),
  )
  ret_mm = round(max(0.0, min(3.0, ret_mm)), 2)

  hrc = parse_int(first_val(raw_cfg, "required_nozzle_HRC"), 3)
  requires_hardened = (
    hrc > 10
    or fin in ("Carbon Fiber", "Glass Fiber", "Glow-in-the-Dark", "Metal-Filled", "Wood-Filled", "Conductive", "Marble/Stone/Terracotta")
    or "-CF" in code.upper()
    or "-GF" in code.upper()
  )

  entry = {
    "brand": brand,
    "code": code,
    "label": label,
    "category": cat,
    "default_finish": fin,
    "default_color": "#8E9299",
    "base_nozzle_c": nozzle_c,
    "base_bed_c": bed_c,
    "max_vol_mm3s": mvs,
    "fan_pct": fan_pct,
    "default_flow_ratio": flow,
    "default_retraction_mm": ret_mm,
    "dry_temp_c": dry_c,
    "dry_hr": dry_h,
    "requires_hardened": requires_hardened,
    "source": source_label,
  }
  return entry, sanitize_profile_map(raw_cfg)


def import_siraya_tech(root_dir: Path, vendor_dir: Path) -> tuple[list[dict], dict[str, dict]]:
  manifest_path = vendor_dir / "siraya_tech" / "manifest.json"
  manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
  by_variant: dict[str, dict] = {}

  # Prioritize Anycubic Kobra X / S1 Max .3mf over Bambu X1C .3mf when both exist,
  # while keeping all distinct foaming temperature variants and all 24 materials!
  for item in manifest:
    threemf_path = root_dir / item["threemf_file"]
    if not threemf_path.exists():
      continue
    with zipfile.ZipFile(threemf_path, "r") as zf:
      cfg = json.loads(zf.read("Metadata/project_settings.config").decode("utf-8", errors="replace"))
    variant = item["variant"]
    mat = item["material"]
    ptag = item["printer_tag"]

    # Map to canonical Siraya Tech material code + label
    v_up = variant.upper()
    if "TPU AIR" in v_up and "95A" in v_up:
      if "240" in v_up:
        code, label = ("TPU Air 95A (240°C -> 90A)", "Siraya Tech Flex TPU Air 95A @ 240°C (90A Shore • 0.80 Flow)")
      elif "250" in v_up:
        code, label = ("TPU Air 95A (250°C -> 85A)", "Siraya Tech Flex TPU Air 95A @ 250°C (85A Shore • 0.64 Flow)")
      elif "260" in v_up:
        code, label = ("TPU Air 95A (260°C -> 80A)", "Siraya Tech Flex TPU Air 95A @ 260°C (80A Shore • 0.56 Flow)")
      elif "270" in v_up:
        code, label = ("TPU Air 95A (270°C -> 75A)", "Siraya Tech Flex TPU Air 95A @ 270°C (75A Shore • 0.51 Flow)")
      else:
        code, label = ("TPU Air 95A", "Siraya Tech Flex TPU Air 95A (Active Foaming 75A–90A)")
    elif "ROAMR" in v_up and "80A" in v_up:
      code, label = ("Roamr TPU Air HR 80A", "Siraya Tech Roamr TPU Air HR 80A (High-Rebound Foaming @ 250°C • 0.70 Flow)")
    elif "ROAMR" in v_up and "85A" in v_up:
      code, label = ("Roamr TPU Air HR 85A", "Siraya Tech Roamr TPU Air HR 85A (High-Rebound Foaming @ 250°C • 0.72 Flow)")
    elif "TPU AIR" in v_up:
      if "250" in v_up:
        code, label = ("TPU Air 85A (250°C -> 75A)", "Siraya Tech Flex TPU Air 85A @ 250°C (75A Shore • 0.67 Flow • 0mm Retract)")
      elif "260" in v_up:
        code, label = ("TPU Air 85A (260°C -> 70A)", "Siraya Tech Flex TPU Air 85A @ 260°C (70A Shore • 0.56 Flow • 0mm Retract)")
      elif "270" in v_up:
        code, label = ("TPU Air 85A (270°C -> 65A)", "Siraya Tech Flex TPU Air 85A @ 270°C (65A Shore • 0.52 Flow • 0mm Retract)")
      else:
        code, label = ("TPU Air 85A", "Siraya Tech Flex TPU Air 85A (Active Foaming 65A–85A)")
    elif "PEBA AIR" in v_up:
      if "250" in v_up:
        code, label = ("Rebound PEBA Air (250°C -> 76A)", "Siraya Tech Rebound PEBA Air 95A @ 250°C (76A Shore • 0.82 Flow)")
      elif "260" in v_up:
        code, label = ("Rebound PEBA Air (260°C -> 74A)", "Siraya Tech Rebound PEBA Air 95A @ 260°C (74A Shore • 0.72 Flow)")
      elif "270" in v_up:
        code, label = ("Rebound PEBA Air (270°C -> 70A)", "Siraya Tech Rebound PEBA Air 95A @ 270°C (70A Shore • 0.60 Flow)")
      else:
        code, label = ("Rebound PEBA Air 95A", "Siraya Tech Rebound PEBA Air 95A (Foaming PEBA)")
    elif "PEBA" in v_up and "85A" in v_up:
      code, label = ("Rebound PEBA 85A", "Siraya Tech Rebound PEBA 85A (Ultra-High Energy Return Elastomer)")
    elif "PEBA" in v_up and "95A" in v_up:
      code, label = ("Rebound PEBA 95A", "Siraya Tech Rebound PEBA 95A (High-Speed Energy Return Elastomer)")
    elif "TPU-GF" in v_up or "TPU GF" in v_up:
      code, label = ("Fibreheart TPU-GF", "Siraya Tech Fibreheart TPU-GF (Glass-Fiber Structural Flexible TPU)")
    elif "TPU" in v_up and "85A" in v_up:
      code, label = ("Flex TPU 85A HF", "Siraya Tech Flex TPU 85A HF (High-Flow Soft Elastomer)")
    elif "TPU" in v_up and "95A" in v_up:
      code, label = ("Flex TPU 95A HF", "Siraya Tech Flex TPU 95A HF (High-Flow Medium-Flex Elastomer)")
    elif "TPU" in v_up and "64D" in v_up:
      code, label = ("Flex TPU 64D HF", "Siraya Tech Flex TPU 64D HF (Semi-Rigid High-Impact Elastomer)")
    elif "PETG-CF PRO" in v_up:
      code, label = ("Fibreheart PETG-CF Pro", "Siraya Tech Fibreheart PETG-CF Pro (255°C High-Strength Carbon PETG)")
    elif "PETG-CF" in v_up:
      code, label = ("Fibreheart PETG-CF", "Siraya Tech Fibreheart PETG-CF (235°C Carbon Fiber PETG)")
    elif "PET-CF" in v_up:
      code, label = ("Fibreheart PET-CF", "Siraya Tech Fibreheart PET-CF (300°C Annealable Carbon Fiber PET)")
    elif "PET-GF" in v_up:
      code, label = ("Fibreheart PET-GF", "Siraya Tech Fibreheart PET-GF (300°C Electrically Insulating Glass Fiber PET)")
    elif "PPA-CF CORE" in v_up:
      code, label = ("Fibreheart PPA-CF Core", "Siraya Tech Fibreheart PPA-CF Core (300°C Core-Sheath Carbon Polyphthalamide)")
    elif "PPA-CF" in v_up:
      code, label = ("Fibreheart PPA-CF", "Siraya Tech Fibreheart PPA-CF (300°C High-Temp Carbon Fiber Nylon)")
    elif "PPA-GF" in v_up:
      code, label = ("Fibreheart PPA-GF", "Siraya Tech Fibreheart PPA-GF (300°C High-Temp Glass Fiber Nylon)")
    elif "PPA" in v_up:
      code, label = ("Fibreheart PPA", "Siraya Tech Fibreheart PPA (300°C Neat Polyphthalamide High-Temp Nylon)")
    elif "ABS-CF CORE" in v_up:
      code, label = ("Fibreheart ABS-CF Core", "Siraya Tech Fibreheart ABS-CF Core Black (280°C Core-Encapsulated Carbon ABS)")
    elif "ABS-CF" in v_up:
      code, label = ("Fibreheart ABS-CF", "Siraya Tech Fibreheart ABS-CF (270°C Carbon Fiber ABS)")
    elif "ABS-GF" in v_up:
      code, label = ("Fibreheart ABS-GF", "Siraya Tech Fibreheart ABS-GF (270°C Glass Fiber ABS)")
    elif "ABS HT" in v_up:
      code, label = ("Fibreheart ABS HT HF", "Siraya Tech Fibreheart ABS HT HF (260°C High-Heat High-Flow ABS)")
    elif "ASA-GF" in v_up:
      code, label = ("Fibreheart ASA-GF", "Siraya Tech Fibreheart ASA-GF (270°C UV-Weatherproof Glass Fiber ASA)")
    else:
      code, label = (mat, f"Siraya Tech {mat}")

    priority = 2 if ptag.startswith("Anycubic") else 1
    prev = by_variant.get(code)
    if prev is None or priority > prev[0]:
      entry, sanitized = build_entry(
        brand="Siraya Tech",
        code=code,
        label=label,
        raw_cfg=cfg,
        source_label=f"Siraya Tech .3mf ({threemf_path.name})",
      )
      by_variant[code] = (priority, entry, sanitized)

  # Also add a canonical "TPU Air 85A" entry pointing to 250°C (75A) so selecting "TPU Air 85A" works seamlessly with the Shore Hardness dial
  if "TPU Air 85A (250°C -> 75A)" in by_variant and "TPU Air 85A" not in by_variant:
    p, e, s = by_variant["TPU Air 85A (250°C -> 75A)"]
    e_copy = dict(e)
    e_copy["code"] = "TPU Air 85A"
    e_copy["label"] = "Siraya Tech Flex TPU Air 85A (Active Foaming 65A–85A • 240–270°C)"
    by_variant["TPU Air 85A"] = (p, e_copy, dict(s))

  fils = [v[1] for _, v in sorted(by_variant.items())]
  bases = {f"Siraya Tech::{v[1]['code']}": v[2] for v in by_variant.values()}
  return fils, bases


def import_sunlu(root_dir: Path, vendor_dir: Path) -> tuple[list[dict], dict[str, dict]]:
  manifest_path = vendor_dir / "sunlu" / "manifest.json"
  manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
  fils = []
  bases = {}

  for item in manifest:
    json_path = root_dir / item["json_file"]
    if not json_path.exists():
      continue
    cfg = json.loads(json_path.read_text(encoding="utf-8"))
    mat = item["material"]
    label = f"SUNLU {mat} (SUNLU Preset Manager)"
    entry, sanitized = build_entry(
      brand="SUNLU",
      code=mat,
      label=label,
      raw_cfg=cfg,
      source_label=f"SUNLU Preset Manager ({json_path.name})",
    )
    fils.append(entry)
    bases[f"SUNLU::{mat}"] = sanitized

  return fils, bases


def import_polymaker(root_dir: Path, vendor_dir: Path) -> tuple[list[dict], dict[str, dict]]:
  manifest_path = vendor_dir / "polymaker" / "manifest.json"
  manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
  fils = []
  bases = {}

  for item in manifest:
    prod = item["product"]
    if "TYC Americas" in prod:
      continue
    json_path = root_dir / item["json_file"]
    if not json_path.exists():
      continue
    cfg = json.loads(json_path.read_text(encoding="utf-8"))
    printer = item.get("printer") or "OrcaSlicer"
    label = f"Polymaker {prod} ({printer} Preset)"
    entry, sanitized = build_entry(
      brand="Polymaker",
      code=prod,
      label=label,
      raw_cfg=cfg,
      source_label=f"Polymaker Preset Manager ({json_path.name})",
    )
    fils.append(entry)
    bases[f"Polymaker::{prod}"] = sanitized

  return fils, bases


def main():
  parser = argparse.ArgumentParser(description="Import checked-in vendor filament profiles (.3mf & .json)")
  parser.add_argument(
    "--vendor-dir",
    type=Path,
    default=VENDOR_DIR,
    help="Path to the vendor_profiles directory",
  )
  parser.add_argument(
    "--output",
    type=Path,
    default=VENDOR_DIR / "imported_vendor_catalog.json",
    help="Path to output imported_vendor_catalog.json",
  )
  args = parser.parse_args()
  vendor_dir = args.vendor_dir.resolve()
  root_dir = vendor_dir.parent

  st_fils, st_bases = import_siraya_tech(root_dir, vendor_dir)
  sl_fils, sl_bases = import_sunlu(root_dir, vendor_dir)
  pm_fils, pm_bases = import_polymaker(root_dir, vendor_dir)

  all_fils = st_fils + sl_fils + pm_fils
  all_bases = {}
  all_bases.update(st_bases)
  all_bases.update(sl_bases)
  all_bases.update(pm_bases)

  out_obj = {
    "filaments": all_fils,
    "vendor_filament_bases": all_bases,
  }
  out_path = args.output
  out_path.parent.mkdir(parents=True, exist_ok=True)
  out_path.write_text(json.dumps(out_obj, indent=2), encoding="utf-8")
  print(
    f"Imported {len(all_fils)} vendor filament profiles ({len(st_fils)} Siraya Tech, {len(sl_fils)} SUNLU, {len(pm_fils)} Polymaker) -> {out_path}"
  )


if __name__ == "__main__":
  main()
