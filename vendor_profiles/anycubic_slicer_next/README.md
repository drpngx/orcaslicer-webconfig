# AnycubicSlicerNext System Profiles Override (`v2.0.0.3` — All Anycubic Machines)

## Why We Package These Profiles

Anycubic maintains and actively updates machine, process, and filament presets for **all Anycubic printers** (`Anycubic Kobra 1` / `Plus` / `Max`, `Kobra 2` / `Neo` / `Pro` / `Plus` / `Max`, `Kobra 3` / `V2` / `Max` / `Max V2`, `Kobra 4`, `Kobra X`, `Kobra S1`, and `Kobra S1 Max`) inside their own **AnycubicSlicerNext** fork rather than upstreaming updates back to `OrcaSlicer/OrcaSlicer`.

Even in upstream `OrcaSlicer v2.5.1`, Anycubic presets remain incomplete or stale early drafts — for example:
- `Anycubic ABS @Anycubic Kobra S1 0.4 nozzle.json` in upstream `OrcaSlicer` omits `textured_plate_temp` completely and sets `"bed_type": ["Cool Plate"]` with `"hot_plate_temp": ["90"]`, whereas `AnycubicSlicerNext` sets `textured_plate_temp = 110°C`, `hot_plate_temp = 100°C`, `nozzle_temperature_BRASS = 260°C`, and `fan_max_speed_BRASS = 20%`.
- Newer Anycubic models (`Kobra 3 V2`, `Kobra 3 Max V2`, `Kobra 4`) and explicit `"printer_structure"` kinematics (`"i3"` bed-slingers vs `"corexy"` on `Kobra S1` / `Kobra S1 Max`) are defined in `AnycubicSlicerNext`.
- `AnycubicSlicerNext` provides dedicated `_BRASS` / `_HS` nozzle curves and expanded filament presets across Kobra S1, Kobra S1 Max, Kobra 4, Kobra X, and Kobra 3 (`PA`, `PAHT-CF`, `PC-CF`, `PEBA`, `PET-CF`, `PLA Wood`, `PLA Metal`, `PLA Marble`, `PLA Translucent`, etc.).

## Hermetic Build-Time Architecture (Static-Only Serving & Small WASM Binary)

1. **No Implicit Runtime Disk Reads**: At runtime, the app never reads local system directories unless the user explicitly triggers directory import in Tauri / local dev mode.
2. **Build-Time Genrule Unzip + `include_str!` + Brotli**:
   - `anycubic_slicer_next_profiles.zip` contains all 630 `.json` presets from `AnycubicSlicerNext/system/` (excluding `.stl`/`.png`/`.svg` 3D assets).
   - During `bazel build`, `orcaslicer_catalog_gen` (`BUILD`) unzips this archive in a temporary sandbox directory and passes it to `:generate_orca_catalog`, replacing upstream `@orcaslicer_repo`'s `Anycubic/` directory across **all Anycubic machines, processes, and filaments** when generating `orcaslicer_generated_catalog.json`.
   - `src/catalog.rs` embeds only the distilled `orcaslicer_generated_catalog.json` via `include_str!`, which `app_wasm_opt_br` compresses with `@brotli -9` — keeping static-only browser serving with zero runtime ZIP overhead and minimal WASM binary size.
