# OrcaSlicer WebConfig (for v2.2)

**OrcaSlicer WebConfig** is a transformation-based configuration engine and recipe manager for **OrcaSlicer v2.2** (and OrcaSlicer-derived slicers such as **Anycubic Slicer Next**).

Instead of storing hundreds of static, fragile `.json` profiles with hardcoded numbers, OrcaSlicer WebConfig models your slicer setup as **composable stacks of computational Lua transformations** applied on top of clean vendor and system base profiles.

---

## What This Does

1. **Configures OrcaSlicer & Anycubic Slicer Next**:
   - Generates clean, ready-to-import `.zip` configuration bundles (`Machine`, `Process`, and `Filament` presets), multi-material `.3mf` project loadouts with pre-computed 4-slot / 8-slot flushing volume matrices, and versioned `orcaslicer_webconfig.json` snapshots.
   - In the **Tauri Desktop App**, reads and writes directly to your local slicer directory using capability-based filesystem sandboxing (`cap-std` and `tauri-plugin-fs`).
2. **Collects Proven Slicer Tuning Recipes**:
   - Bundles battle-tested Lua recipes for common 3D printing problems: **PETG & PCTG anti-stringing** (spiral/slope Z-hop, inward wipe, avoid crossing walls), **Scarf Joint Contour Seams & Arachne walls**, **Thermal Conductivity & Nozzle Material compensation**, **Active Foaming LW-PLA / Varioshore TPU Shore hardness control**, and **Pigment & Opacity-Aware Flushing Matrices**.
3. **Solves the Combinatorial Explosion of Slicer Profiles**:
   - Decouples **Machine Rig**, **Nozzle Material & Diameter**, **Bed Plate**, **Ambient Weather & Humidity**, **Filament Brand / Polymer / Specialty Finish**, and **Print Speed / Quality Tier** into independent, stackable layers.

---

## The Motivational Problem: Why Static Slicer Profiles Break Down

Suppose you own an **Anycubic Kobra S1** (or Bambu Lab X1C / Voron 2.4) with an 8-slot multi-material unit, and you print across several real-world dimensions:

- **Nozzle Swaps**: You swap back and forth between a **0.4mm Hardened Steel** nozzle (low thermal conductivity, needs `+5°C`), a **0.4mm or 0.25mm MicroSwiss Diamondback PCD** nozzle (extreme diamond thermal conductivity, runs `-10°C` cooler with `0.94×` flow and shorter retraction), and a **0.6mm MicroSwiss CHT CM2** high-flow nozzle.
- **Filament Library**: You keep 12+ spools across **Siraya Tech**, **SUNLU**, **Polymaker**, and **Anycubic**—spanning PLA, Silk PLA, Translucent PETG, PCTG, TPU 95A, Foaming TPU, ABS/ASA, and PA-CF.
- **Weather & Seasonal Drift**:
  - In **Tropical Heat & Humidity** (`30°C / 80% RH`), enclosed PLA and PETG suffer from heat creep and moisture stringing unless nozzle temperatures are lowered `-8°C`, volumetric flow is capped `-15%` (unless using a heat-creep-resistant MicroSwiss FlowTech hotend), and Z-hop is switched from vertical `Normal Lift` to `Spiral Lift`.
  - In a **Cold Winter Garage or Chilled Room** (`14°C / 25% RH`), the exact same filaments warp off the plate and delaminate unless bed temperatures are boosted `+5°C to +10°C`, cooling fans are scaled back `-25%`, and draft shields / brims are enabled.

### Why Absolute Values Are Unmanageable

In standard OrcaSlicer, every profile stores **absolute numbers** (`nozzle_temperature = 235`, `hot_plate_temp = 75`, `max_volumetric_speed = 14`).

If you have:
- `4` Nozzle types/sizes × `3` Bed plates × `3` Weather conditions × `15` Filaments = **`540` separate filament profiles**!

Every time you swap from a Hardened Steel nozzle to a Diamondback nozzle—or the season shifts from temperate to humid summer—you would have to manually edit dozens of profiles by hand. Worse, after a few manual edits, you lose track of *why* a filament is set to `222°C`: was that `230°C base - 8°C tropical`, or `225°C base + 5°C hardened steel - 8°C tropical`? Editing absolute values quickly becomes error-prone and unmanageable.

### The Solution: Stacking Computational Transforms

OrcaSlicer WebConfig keeps your base filament and machine profiles **pure** and computes the final slicer parameters by evaluating an ordered stack of **sandboxed Lua transformations**:

```text
Base Filament (e.g. Siraya Tech PETG Brass Baseline: 230°C Nozzle, 70°C Bed, 16 mm³/s)
  │
  ├─► [Rule 1: Nozzle Material & Orifice]  Hardened Steel 0.4mm  ──► Nozzle +5°C (235°C)
  ├─► [Rule 2: Bed Plate Surface]          Textured PEI Plate    ──► Bed +5°C (75°C)
  ├─► [Rule 3: Ambient Weather Adaptation] Tropical (30°C/80%RH) ──► Nozzle -5°C (230°C), Spiral Lift 0.2mm, Max Flow ×0.85
  └─► [Rule 4: Specialty Finish / Recipe]  Translucent Glass     ──► Fan ≤ 25%, Flow Ratio 1.01
  ═══════════════════════════════════════════════════════════════════════════════════════════
  ══► Final Computed OrcaSlicer Profile (with full per-parameter audit trail & JSON diff)
```

When you switch your rig from **Hardened Steel** to **Diamondback PCD** or change the weather from **Tropical** to **Temperate**, every filament, process speed ceiling, and multi-material flushing volume updates instantaneously.

---

## Documentation (`doc/`)

All documentation is located in the [`doc/`](doc/) directory, linked below for browsing directly on GitHub, and precompiled via Bazel into the application's built-in **Help** viewer:

1. [**Architecture & Core Concepts (`doc/index.md`)**](doc/index.md) — How combinatorial transformation stacking replaces static profile sprawl.
2. [**Getting Started & Initialization (`doc/getting-started.md`)**](doc/getting-started.md) — Using the **Start From Scratch** 5-step wizard, loading versioned `orcaslicer_webconfig.json` snapshots, and **Expert Mode** imports.
3. [**Printer Rigs, Nozzles & Weather Adaptation (`doc/hardware-and-weather.md`)**](doc/hardware-and-weather.md) — Configuring MicroSwiss FlowTech / Diamondback / CHT CM2 nozzles, orifice diameters (`0.2mm`–`1.0mm`), cooling add-ons, and Tropical vs. Cold weather rules.
4. [**Filaments, Vendor Brands & Flushing Matrix (`doc/filaments-and-brands.md`)**](doc/filaments-and-brands.md) — Browsing Siraya Tech, SUNLU, and Polymaker presets, specialty finishes, active TPU foaming Shore hardness, and the physics-based 8×8 flushing matrix.
5. [**Lua Recipes, Symptom Wizard & Diff Viewer (`doc/lua-recipes-and-wizard.md`)**](doc/lua-recipes-and-wizard.md) — Using the Symptom Auto-Tuner, writing custom sandboxed Lua rules, and inspecting colored GitHub/Side-by-Side JSON diffs.

---

## Building, Testing & Running (Bazel-First)

All builds, catalog generators, documentation precompilation, screenshot generation, and unit tests run under **Bazel**:

```bash
# Run all unit tests, Lua transformation safety audits, and Python requirement checks
bazel test //...

# Build the WebAssembly frontend, precompiled docs, vendor catalog, and server
bazel build //...

# Run the local development server on http://127.0.0.1:8003
HTTP_LISTEN_ADDR=0.0.0.0:8003 bazel run //server:server

# Regenerate cropped documentation screenshots in doc/images/
bazel run //scripts:capture_doc_screenshots
```
