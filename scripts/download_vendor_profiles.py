#!/usr/bin/env python3
"""
Download official vendor filament presets into `vendor_profiles/`:
1. Siraya Tech `.3mf` filament archives from https://siraya.tech/pages/print-settings-download
   - Anycubic (Anycubic S1 Max 0.4mm, Anycubic Kobra X 0.4mm)
   - Bambu Lab X1C 0.4mm (all 24 Siraya Tech materials)
2. SUNLU Preset Manager `.json` presets from https://www.sunlu.com/api/preset/list & download
3. Polymaker Preset Manager `.json` presets from https://presets.polymaker.com/index.json
"""

import io
import json
import os
import re
import urllib.request
import zipfile
from pathlib import Path

WORKSPACE_DIR = os.environ.get("BUILD_WORKSPACE_DIRECTORY")
ROOT_DIR = Path(WORKSPACE_DIR) if WORKSPACE_DIR else Path(__file__).resolve().parent.parent
VENDOR_DIR = ROOT_DIR / "vendor_profiles"

SIRAYA_TECH_ARCHIVES = [
  # Anycubic S1 Max 0.4mm (Anycubic Slicer Next) — includes 250C, 260C, 270C TPU Air foaming .3mf files
  ("Anycubic_S1_Max_0.4mm", "TPU Air", "1RN9AJcsc8Z3ZVvFlBIIWhY8-Ph1jG5OC"),
  # Anycubic Kobra X 0.4mm (OrcaSlicer)
  ("Anycubic_Kobra_X_0.4mm", "TPU 85A", "1wUp1bIDD0WIld9Qj4IrqhN5RRx2VexHg"),
  ("Anycubic_Kobra_X_0.4mm", "TPU 95A", "1sV-RQDZuENs0vl1ke-RM_Q_jWYtvAd-3"),
  ("Anycubic_Kobra_X_0.4mm", "TPU 64D", "15FDaewPVhh4kVCq8yD-EhRAGSnMJWTr2"),
  ("Anycubic_Kobra_X_0.4mm", "TPU-GF", "1Dw70JTwqyTyCajSsn0XyBJqgLBtu11pS"),
  ("Anycubic_Kobra_X_0.4mm", "PEBA 95A", "1vLWuo_oL_M-zxkSFXGBWC36KI2C_iw0M"),
  ("Anycubic_Kobra_X_0.4mm", "PEBA 85A", "151NdcgVctL4H827-SHaYNlqROqKUjSUj"),
  ("Anycubic_Kobra_X_0.4mm", "PET-CF", "1b3f7Tpm5s5iEm6VxRepxTAkAZIchr5LY"),
  ("Anycubic_Kobra_X_0.4mm", "PETG-CF Pro", "1p9Cuxggqo9MbuY25JEGLZR7WK7XCtqd-"),
  # Bambu Lab X1C 0.4mm (Complete 24-filament Siraya Tech catalog)
  ("Bambu_X1C_0.4mm", "ABS HT HF", "1XZBF6NYLNYmKwIxLeLIhsDuhiJN6q1iV"),
  ("Bambu_X1C_0.4mm", "ABS-CF Core Black", "161m_7lpprlCmDP42irCoHmkNmAVSlh90"),
  ("Bambu_X1C_0.4mm", "ABS-CF", "18kXzJ0inRebZczV1in522Q9Vrmv7FLEx"),
  ("Bambu_X1C_0.4mm", "ABS-GF", "1reo_ijgY2MJ_VIzf_gVzQdeTysKGCoJ-"),
  ("Bambu_X1C_0.4mm", "ASA-GF", "1y_M3-Q6tDVIa4UuX1vjgvjgLmszxGeR6"),
  ("Bambu_X1C_0.4mm", "PPA", "1JcKQakNyEDMVfOEt-79rkbaPS0GKm4FV"),
  ("Bambu_X1C_0.4mm", "PPA-CF Core", "1kvKDgRL00c6Xhd4D2dRWXdp6VaBLuooK"),
  ("Bambu_X1C_0.4mm", "PPA-CF", "1moyezCIIAJNya11C8tw2LRUmFXRLhzzh"),
  ("Bambu_X1C_0.4mm", "PPA-GF", "1q7Sf3NkSPrl-KbnH1pIqO0-T4K03WYBH"),
  ("Bambu_X1C_0.4mm", "PET-CF", "1-2-iIJzF4F2YavdfSIYqPCtHvr5XtjC4"),
  ("Bambu_X1C_0.4mm", "PET-GF", "147Lp7Wl5mRrZ3hfsw4C9i4RaBTOhw5u1"),
  ("Bambu_X1C_0.4mm", "PETG-CF", "1FDcG62kozd_TSJ7yAZ-GdPHUwGYip3OQ"),
  ("Bambu_X1C_0.4mm", "PETG-CF Pro", "1pp5pavQiwk0FKsAvvQjMijyXjFmWraz_"),
  ("Bambu_X1C_0.4mm", "PEBA 95A", "1QDi7aRM-0-AUE1yo45g2zW-jFXoZzGjq"),
  ("Bambu_X1C_0.4mm", "PEBA 85A", "1yev5Ml8OAkvngV6kk1l4T-3aQvvu9Qhi"),
  ("Bambu_X1C_0.4mm", "PEBA Air 95A", "15-B5gMR-f6qOGF4hbn8fdr_0z8UQtchE"),
  ("Bambu_X1C_0.4mm", "TPU 85A HF", "1w9q3Ouc6p8U2cBaBCvAVBbkrXdbn-mZw"),
  ("Bambu_X1C_0.4mm", "TPU 95A HF", "13aVEb3RdD5mY92CQ-Jb5R47T1nPkNu-9"),
  ("Bambu_X1C_0.4mm", "TPU 64D HF", "1u3ytmse420NytD41pxf3BpR2L5p9E_JG"),
  ("Bambu_X1C_0.4mm", "TPU Air 85A", "1tRTXE7ICwjzNKktM_DkRkTBcwh10JyJJ"),
  ("Bambu_P1S_0.4mm", "TPU Air 95A", "1i_oB3m1MYgjicgZegu2kD08gqdhJZhVS"),
  ("Bambu_X1C_0.4mm", "TPU-GF", "13-oDg-ky2HNU6LtjADmhXjGe3dUJspZB"),
  ("Bambu_X1C_0.4mm", "Roamr TPU Air HR 80A", "1K4vcOSU5IEc0SnH_lr88xeM3sCufVI7D"),
  ("Bambu_X1C_0.4mm", "Roamr TPU Air HR 85A", "1vD7dftXYcnE2nvNGBK0qnLFjNFIGx4-2"),
]


def safe_filename(s: str) -> str:
  s = s.replace("°C", "C").replace("/", "_").replace("\\", "_")
  s = re.sub(r"[^A-Za-z0-9._\- ]+", "_", s)
  return re.sub(r"\s+", "_", s.strip())


def fetch_bytes(url: str, headers: dict | None = None) -> bytes:
  hdrs = {
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) OrcaSlicerWebConfig/1.0",
  }
  if headers:
    hdrs.update(headers)
  req = urllib.request.Request(url, headers=hdrs)
  with urllib.request.urlopen(req, timeout=30) as resp:
    return resp.read()


def download_siraya_tech() -> list[dict]:
  out_3mf_dir = VENDOR_DIR / "siraya_tech" / "3mf"
  out_json_dir = VENDOR_DIR / "siraya_tech" / "extracted_json"
  out_3mf_dir.mkdir(parents=True, exist_ok=True)
  out_json_dir.mkdir(parents=True, exist_ok=True)

  manifest = []
  print(f"=== Downloading Siraya Tech .3mf archives ({len(SIRAYA_TECH_ARCHIVES)} sources) ===")
  for printer_tag, material_label, drive_id in SIRAYA_TECH_ARCHIVES:
    url = f"https://drive.google.com/uc?export=download&id={drive_id}"
    try:
      raw = fetch_bytes(url)
    except Exception as e:
      print(f"  [WARN] Failed {printer_tag} / {material_label}: {e}")
      continue

    try:
      zf = zipfile.ZipFile(io.BytesIO(raw))
    except zipfile.BadZipFile:
      print(f"  [WARN] Not a zip for {printer_tag} / {material_label}")
      continue

    names = zf.namelist()
    # Case 1: The downloaded archive IS directly a .3mf file (contains Metadata/project_settings.config)
    if "Metadata/project_settings.config" in names:
      fname_base = safe_filename(f"Siraya_Tech_{material_label}_{printer_tag}")
      threemf_path = out_3mf_dir / f"{fname_base}.3mf"
      threemf_path.write_bytes(raw)
      cfg_bytes = zf.read("Metadata/project_settings.config")
      cfg_json = json.loads(cfg_bytes.decode("utf-8", errors="replace"))
      json_path = out_json_dir / f"{fname_base}.json"
      json_path.write_text(json.dumps(cfg_json, indent=2), encoding="utf-8")
      manifest.append({
        "printer_tag": printer_tag,
        "material": material_label,
        "variant": material_label,
        "drive_id": drive_id,
        "threemf_file": str(threemf_path.relative_to(ROOT_DIR)),
        "json_file": str(json_path.relative_to(ROOT_DIR)),
      })
      print(f"  [OK] Direct .3mf -> {threemf_path.name}")
    else:
      # Case 2: ZIP archive containing one or more .3mf files
      for member in names:
        if member.lower().endswith(".3mf") and not member.startswith("__MACOSX"):
          member_bytes = zf.read(member)
          sub_name = Path(member).stem
          fname_base = safe_filename(f"{printer_tag}__{sub_name}")
          threemf_path = out_3mf_dir / f"{fname_base}.3mf"
          threemf_path.write_bytes(member_bytes)
          try:
            sub_zf = zipfile.ZipFile(io.BytesIO(member_bytes))
            if "Metadata/project_settings.config" in sub_zf.namelist():
              cfg_bytes = sub_zf.read("Metadata/project_settings.config")
              cfg_json = json.loads(cfg_bytes.decode("utf-8", errors="replace"))
              json_path = out_json_dir / f"{fname_base}.json"
              json_path.write_text(json.dumps(cfg_json, indent=2), encoding="utf-8")
              manifest.append({
                "printer_tag": printer_tag,
                "material": material_label,
                "variant": sub_name,
                "drive_id": drive_id,
                "threemf_file": str(threemf_path.relative_to(ROOT_DIR)),
                "json_file": str(json_path.relative_to(ROOT_DIR)),
              })
              print(f"  [OK] Nested .3mf -> {threemf_path.name}")
          except Exception as e:
            print(f"  [WARN] Could not parse nested .3mf {member}: {e}")

  manifest_path = VENDOR_DIR / "siraya_tech" / "manifest.json"
  manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
  return manifest


def download_sunlu() -> list[dict]:
  out_dir = VENDOR_DIR / "sunlu"
  out_dir.mkdir(parents=True, exist_ok=True)
  headers = {
    "lang": "en",
    "X-CDN-Origin": "true",
    "Referer": "https://www.sunlu.com/materials-lab/printer-presets",
  }
  print("=== Downloading SUNLU Preset Manager JSONs ===")
  # Query Bambu Studio / OrcaSlicer (slicerId=1) and Creality/Orca (slicerId=4)
  raw_list = fetch_bytes("https://www.sunlu.com/api/preset/list?slicerId=1", headers=headers)
  resp = json.loads(raw_list.decode("utf-8"))
  data_obj = resp.get("data", {})
  items = data_obj.get("list", []) if isinstance(data_obj, dict) else data_obj
  manifest = []

  for item in items:
    mat_name = (item.get("name") or "").strip()
    files = item.get("files") or []
    if not mat_name or not files:
      continue
    # Prefer X1C 0.4 nozzle or general 0.4 nozzle JSON preset
    chosen = None
    for f in files:
      fn = (f.get("fileName") or "")
      if fn.endswith(".json") and ("X1C 0.4" in fn or "0.4 nozzle" in fn):
        chosen = f
        break
    if not chosen:
      for f in files:
        if (f.get("fileName") or "").endswith(".json"):
          chosen = f
          break
    if not chosen:
      continue

    file_id = chosen.get("id")
    file_name = chosen.get("fileName")
    dl_url = f"https://www.sunlu.com/api/preset/download?fileId={file_id}"
    try:
      data_bytes = fetch_bytes(dl_url, headers=headers)
      parsed = json.loads(data_bytes.decode("utf-8", errors="replace"))
      safe_mat = safe_filename(f"SUNLU_{mat_name}")
      out_path = out_dir / f"{safe_mat}.json"
      out_path.write_text(json.dumps(parsed, indent=2), encoding="utf-8")
      manifest.append({
        "material": mat_name,
        "file_id": file_id,
        "original_file_name": file_name,
        "json_file": str(out_path.relative_to(ROOT_DIR)),
      })
      print(f"  [OK] SUNLU {mat_name} -> {out_path.name}")
    except Exception as e:
      print(f"  [WARN] Failed SUNLU {mat_name} (fileId={file_id}): {e}")

  (out_dir / "manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
  return manifest


def download_polymaker() -> list[dict]:
  out_dir = VENDOR_DIR / "polymaker"
  out_dir.mkdir(parents=True, exist_ok=True)
  print("=== Downloading Polymaker Preset Manager JSONs ===")
  raw_idx = fetch_bytes("https://presets.polymaker.com/index.json")
  idx_data = json.loads(raw_idx.decode("utf-8"))
  materials = idx_data.get("materials", [])
  all_presets = idx_data.get("presets", [])
  manifest = []

  by_mat: dict[str, list[dict]] = {}
  for p in all_presets:
    m = (p.get("material") or "").strip()
    if m and (p.get("path") or "").endswith(".json"):
      by_mat.setdefault(m, []).append(p)

  for product in materials:
    product = (product or "").strip()
    presets = by_mat.get(product, [])
    if not product or not presets:
      continue

    # Prefer Anycubic Kobra S1 OrcaSlicer preset first, else BBL X1C OrcaSlicer preset, else any OrcaSlicer/BambuStudio .json
    chosen = None
    for p in presets:
      if p.get("brand") == "Anycubic" and p.get("model") == "Kobra S1" and p.get("slicer") == "OrcaSlicer":
        chosen = p
        break
    if not chosen:
      for p in presets:
        if p.get("brand") == "BBL" and "X1" in (p.get("model") or "") and p.get("slicer") == "OrcaSlicer":
          chosen = p
          break
    if not chosen:
      for p in presets:
        if p.get("slicer") in ("OrcaSlicer", "BambuStudio"):
          chosen = p
          break
    if not chosen:
      chosen = presets[0]

    rel_path = chosen["path"]
    encoded_path = urllib.parse.quote(rel_path, safe="/")
    dl_url = f"https://presets.polymaker.com/{encoded_path}"
    try:
      data_bytes = fetch_bytes(dl_url)
      parsed = json.loads(data_bytes.decode("utf-8", errors="replace"))
      safe_prod = safe_filename(f"Polymaker_{product}")
      out_path = out_dir / f"{safe_prod}.json"
      out_path.write_text(json.dumps(parsed, indent=2), encoding="utf-8")
      manifest.append({
        "product": product,
        "printer": f"{chosen.get('brand', '')} {chosen.get('model', '')}".strip(),
        "slicer": chosen.get("slicer"),
        "source_path": rel_path,
        "json_file": str(out_path.relative_to(ROOT_DIR)),
      })
      print(f"  [OK] Polymaker {product} ({chosen.get('brand')} {chosen.get('model')}) -> {out_path.name}")
    except Exception as e:
      print(f"  [WARN] Failed Polymaker {product} ({rel_path}): {e}")

  (out_dir / "manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
  return manifest


if __name__ == "__main__":
  st = download_siraya_tech()
  sl = download_sunlu()
  pm = download_polymaker()
  print(
    f"\nCompleted: {len(st)} Siraya Tech .3mf profiles, {len(sl)} SUNLU presets, {len(pm)} Polymaker presets."
  )
