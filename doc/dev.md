# Developer, Contributor & Release Guide

This guide covers building, testing, and running **OrcaSlicer WebConfig** locally with Bazel, adding polymers/nozzles/Lua transformations, building the single-file portable Tauri desktop application, and publishing permanent GitHub Releases.

---

## 1. Core Architecture & Development Principles

1. **Rust / WebAssembly Only (`wasm32-unknown-unknown`)**:
   * All UI components, state management, profile parsing, `.3mf` / `.zip` packaging, and Lua transformation evaluation are written in Rust using the **Yew** framework and our pure-Rust sandboxed Lua interpreter ([`src/lua_engine.rs`](../src/lua_engine.rs)).
   * Do not introduce JavaScript application logic; only minimal browser bindings via `wasm-bindgen` / `web-sys` (e.g., standard file input/download blobs and `localStorage`) are used.
   * Because browser sandboxes block access to system configuration directories (`%APPDATA%`), the static web app uses standard file download/upload bundles (`.zip`, `.3mf`, `.json`), while direct local slicer directory access is handled by the Tauri desktop app and local dev server via `cap-std` **strictly on explicit user Import or Save actions** (the app defaults to nothing loaded on startup and never reads from disk implicitly at runtime). Vendor system profile overrides (such as [`vendor_profiles/anycubic_slicer_next/anycubic_slicer_next_profiles.zip`](../vendor_profiles/anycubic_slicer_next/anycubic_slicer_next_profiles.zip)) are extracted at Bazel build time into `generated_orca_catalog.json` and embedded statically via `include_str!` + Brotli compression.
2. **Bazel-First Execution**:
   * All builds, code generators, documentation compilers, tests, and servers run through **Bazel** (`bazel build`, `bazel test`, `bazel run`).
   * Never invoke raw `./bazel-bin/...` binaries or unmanaged `cargo` build commands directly.
3. **100% Lua Domain Physics & Deferred Epilogue Quantization**:
   * All domain physics and parameter transformations live in [`transformations/*.lua`](../transformations/), never in Rust (`src/*.rs`).
   * During Phase 2 (`P10 .. P98` Lua rules) and Phase 3.1/3.2 (Epilogue Box & Relational Constraint Projections), numeric values remain at full IEEE-754 floating-point precision (`_defer_quantization = "1"`) via `map_num` and `fmt_num`, and are quantized **once** at the very end of `TransformationEngine::evaluate_stack` via `format_orca_schema_scalar`.
4. **Data-Driven Polymer & Hardware Baseline Constants (`constants.json`)**:
   * Polymer families, safe nozzle/bed temperature bounds, volumetric flow ceilings, kinematics motion-class policies (`corexy` vs. `bedslinger`), and vendor filament catalogs live in [`constants.json`](../constants.json) and are accessed through [`src/constants.rs`](../src/constants.rs).
   * Avoid hardcoding polymer names or magic numbers in Rust code.
5. **2-Space Rust Indentation**:
   * All Rust source files (`*.rs`) use **2 spaces** for indentation (`tab_spaces = 2` in [`rustfmt.toml`](../rustfmt.toml)).

---

## 2. Building, Testing & Running Locally

### Run the Test Suite, Ground-Truth C++ Schema Gate & Inspection Pack
```bash
# Run all unit tests, C++ ground-truth schema validation, and random config audits
bazel test -c opt //... --test_output=errors

# Dump bounded side-by-side (Stock Parent -> Exported Child -> Effective Merged + C++ Tooltip)
# inspection packs for genuine line-by-line human/agent reading
bazel run -c opt //:audit_random_configs -- --dump-inspection-pack /tmp/inspection_pack
```
This executes:
* `//:slicer_ground_truth_gen` ([`scripts/build_slicer_ground_truth.py`](../scripts/build_slicer_ground_truth.py)) — Parses OrcaSlicer `src/libslic3r/PrintConfig.cpp` + `src/libslic3r/Preset.cpp` and `AnycubicSlicerNext` `anycubic_slicer_next_profiles.zip` at build time into `slicer_ground_truth.json` (`1,010` C++ options, exact `machine`/`process`/`filament` scopes, vector-vs-scalar types, `72` C++ enums, Anycubic-fork-only `*_BRASS`/`*_HS` scope, and `12,745` resolved system presets).
* `//:orcaslicer_webconfig_test` — Unit tests for profile import/export, `.3mf` container generation, Lua rules, and catalog round-tripping.
* `//:audit_random_configs_test` — Validates every exported preset against `GroundTruthSlicerSchema` plus parent-anchored hardware, thermal, cooling, and kinematics physics invariants.
* `//:requirements.test` — Python dependency lockfile verification.

### Build All Targets
```bash
# Standard build (WASM frontend, precompiled docs, vendor catalogs, and Axum server)
bazel build //...

# Optimized release build (-c opt with wasm-opt + Brotli compression)
bazel build -c opt //:app_wasm_opt //server:server
```

### Run the Local Development Server
Always launch the local server via `bazel run` with `HTTP_LISTEN_ADDR`:
```bash
# Fast development server on http://127.0.0.1:8003
HTTP_LISTEN_ADDR=0.0.0.0:8003 bazel run //server:server

# Optimized production server on http://127.0.0.1:8003
HTTP_LISTEN_ADDR=0.0.0.0:8003 bazel run -c opt //server:server
```
* When the user explicitly clicks **Connect / Import Local Slicer Directory** or **Save to Slicer Directory**, [`server/src/lib.rs`](../server/src/lib.rs) can auto-discover the local `AnycubicSlicerNext` or `OrcaSlicer` configuration directory inside `%APPDATA%` (Windows / WSL `/mnt/c/Users/<user>/AppData/Roaming/`), `~/Library/Application Support/` (macOS), or `~/.config/` (Linux). On initial startup, nothing is loaded from disk automatically.
* You can override the target directory path via `ORCASLICER_LOCAL_DIR=/path/to/slicer` or [`local_slicer_config.json`](../local_slicer_config.json).

---

## 3. Contributing: Filaments, Nozzles, Lua Rules & Docs

* **Updating Polymer / Nozzle / Finish Constants**:
  * Edit [`constants.json`](../constants.json) to tune polymer temperature ranges, default flow ratios, retraction lengths, Z-hop heights/types, or nozzle thermal offsets.
* **Adding or Editing Lua Transformations**:
  * Core hardware & process rules live in [`transformations/`](../transformations/).
  * Interactive parametric Symptom Auto-Tuner & Calibration rules live in [`transformations/parametric/`](../transformations/parametric/).
  * Each parametric script defines `meta`, `params` (sliders/enums with optional `sets` presets), `validate(p, ctx)`, `describe(p, ctx)`, and `apply(config, ctx, p)`.
* **Updating Vendor Filament Catalogs & Screenshots**:
  ```bash
  # Download & re-import vendor filament profiles (Siraya Tech, SUNLU, Polymaker)
  bazel run //scripts:download_vendor_profiles
  bazel run //scripts:import_vendor_profiles

  # Regenerate cropped documentation screenshots in doc/images/
  bazel run //scripts:capture_doc_screenshots
  ```

---

## 4. Building the Single-File Portable Desktop App (Tauri v2)

To build the self-contained single-file portable desktop executable locally via Bazel:
```bash
bazel run -c opt //:tauri_build
```
This stages the optimized WASM bundle (`dist/`), generated OrcaSlicer & vendor catalogs, and precompiled documentation (`compiled_docs.json`), then invokes `cargo tauri build --no-bundle` inside [`src-tauri/`](../src-tauri/) to produce a single standalone binary with zero external sidecar files.

---

## 5. Releasing & Publishing Permanent GitHub Releases

Permanent GitHub Releases are driven by [`RELEASE_NOTES.md`](../RELEASE_NOTES.md) and [`.github/workflows/release.yml`](../.github/workflows/release.yml).

### Step 1: Update `RELEASE_NOTES.md`
1. Edit [`RELEASE_NOTES.md`](../RELEASE_NOTES.md) in the repository root with the notes for the new release.
2. Commit and push [`RELEASE_NOTES.md`](../RELEASE_NOTES.md) to `main` **before** triggering the release:
   ```bash
   git add RELEASE_NOTES.md
   git commit -m "Update release notes"
   git push origin main
   ```

### Step 2: Trigger the Release Build
You can publish a release using either method below:

#### Option A: Manual Trigger from GitHub Actions UI (Recommended)
1. Open the repository on GitHub and click the **Actions** tab.
2. Select **Build and Publish Release** ([`.github/workflows/release.yml`](../.github/workflows/release.yml)) in the left sidebar.
3. Click **Run workflow** on the right and fill in:
   * **Release Tag (`tag_name`):** Version tag to create/publish (e.g., `v0.1.0`).
   * **Release Title (`release_name`):** Optional custom title (defaults to the tag name if left blank).
   * **Also append auto-generated commit list (`append_changelog`):** Check to append GitHub's commit/PR list below `RELEASE_NOTES.md`, or leave unchecked to show only `RELEASE_NOTES.md`.
4. Click **Run workflow**.

#### Option B: Push a `v*` Git Tag from the Terminal
```bash
git tag v0.1.0
git push origin v0.1.0
```

### Published Release Assets
Once the workflow completes, the release is published under **Releases** on the repository's main page with `RELEASE_NOTES.md` as its body and three single-file portable executables attached under **Assets**:
* `orcaslicer-webconfig-portable-windows-x64.exe` (Windows x64)
* `orcaslicer-webconfig-portable-linux-x64` (Linux x64)
* `orcaslicer-webconfig-portable-macos` (macOS Apple Silicon)

### How to Test-Build Portable Executables *Without* Releasing
If you want to build and download the portable executables from CI **without** creating a public GitHub Release or tag:
1. Open the **Actions** tab on GitHub and select **Build Portable Tauri Desktop App (Windows, Linux x64, macOS)** ([`.github/workflows/tauri-build.yml`](../.github/workflows/tauri-build.yml)).
2. Click **Run workflow** and leave **Optional GitHub Release tag (`upload_to_release_tag`)** **blank**.

---

## 6. Slicer Configuration Architecture, Lua Physics & Audit Best Practices

1. **Single Canonical Evaluation Pipeline & Foreign-Key Inheritance (No Post-Lua Overwrites)**:
   * Machine, process, and filament `inherits` (and `compatible_printers`) must **never** be fabricated from string templates (e.g., `"{vendor} {mat} @{printer} 0.4 nozzle"`), because OrcaSlicer vendor trees use irregular preset names (`@BBL X1C`, `@Creality Ender3 0.4`, `@MK4S`, `Generic PETG @System`). Always resolve preset inheritance strictly by foreign-key lookup against verified `instantiation == "true"` presets in [`CombinedCatalog`](../src/catalog.rs).
   * Do not cache synthesized JSON blobs in `FilamentSpool::raw_disk_json` (`raw_disk_json` is reserved strictly for actual user files imported from disk).
   * Per-spool and per-process exports in [`src/threemf.rs`](../src/threemf.rs) must seed baseline values (`x0`) and `_bounds` **before** calling `TransformationEngine::evaluate_stack`, and project `eval.resulting_profile.values` **directly** into standalone `.json` and `.3mf` outputs with zero post-Lua physics overwrites. This guarantees `x_provenance == x_JSON == x_3MF` by construction and ensures new Lua-written keys are never dropped.

2. **Six-Plate Surface Chemistry & Parent-Preset Schedule ([`plate_surface_temperatures.lua`](../transformations/plate_surface_temperatures.lua))**:
   * Never stamp a single `bed_temp` across all six OrcaSlicer plate keys (`textured_plate_temp`, `hot_plate_temp`, `eng_plate_temp`, `cool_plate_temp`, `textured_cool_plate_temp`, `supertack_plate_temp`).
   * Across **all** six plate pairs (`*_plate_temp_initial_layer` vs `*_plate_temp`) and **all** polymers, keep Layer-1 -> Layer-2 bed drop `<= 5°C` (`<= 3°C` on elastomers/PHA) so PEI spring steel never contracts violently and pops Layer 1 off the bed on Layer 2.
   * High-temperature warp-prone polymers (`ABS`, `ASA`, `PC`, `PC-CF`, `PA*`, `PET-CF`, `HIPS`) and `PCTG` explicitly set `cool_plate_temp` and `textured_cool_plate_temp` to `"0"` (unsupported — overriding `AnycubicSlicerNext`'s un-overridden `35°C`/`40°C` `fdm_filament_common` inheritance so selecting a Cool Plate never prints ABS/PC/PA at `35°C`), while low-temperature polymers (`PLA`, `PETG`, `TPU`, `PHA`, `PVA`) preserve non-zero Cool Plate support (`<= 60°C` Cool, `<= 65°C` Textured Cool, `<= 70°C` SuperTack, and `"0"` SuperTack on `TPU`/`PEBA`).
   * Exported `.3mf` `project_settings.config` files explicitly declare `"curr_bed_type": "Textured PEI Plate"`.

3. **Polymer Thermal, Chamber & Kinematics Invariants**:
   * **Warp-Prone Polymers (`ABS`, `ASA`, `PC`, `PA`, `PET-CF`, `HIPS`)**: Keep first-to-continuous bed drop `<= 5°C`, `close_fan_the_first_x_layers >= 3` (`>= 2` on fiber-reinforced PLA, `>= 1` on all other polymers), `additional_cooling_fan_speed == 0`, `during_print_exhaust_fan_speed == 0` (retain chamber heat during printing), `overhang_fan_speed <= 35%`, `temperature_vitrification > max(textured_plate_temp_initial_layer, hot_plate_temp_initial_layer)` (so OrcaSlicer never warns the user to open the chamber door during ABS/PC/PA printing), and `fan_min_speed < fan_max_speed`.
   * **Zero 24V PWM Fan Stall Zone & G-Code Activation Flags**: Never emit fan percentages in `1%–9%` (where brushless 24V blowers stall); snap to `0%` or `>= 10%`. On enclosed printers, always emit `"activate_air_filtration": ["1"]` whenever exhaust filtration is active (`GCode.cpp` ignores `during_print_exhaust_fan_speed` when `activate_air_filtration` is `false`), and omit chamber exhaust keys on open-frame bed-slingers. On Direct-Drive Klipper rigs, always emit `"enable_pressure_advance": ["1"]` alongside `pressure_advance`. Gate Anycubic-fork `*_BRASS` / `*_HS` keys strictly to Anycubic printers.
   * **Elastomers & Extruder Path**: `TPU`/`PEBA` enforce `filament_z_hop = 0` (`Normal Lift`), `filament_retraction_speed <= 35 mm/s` (`<= 25 mm/s` on Bowden), and skip moisture flow/retraction trims. Bowden machines (`base_retraction_length >= 2.0 mm`, e.g., `Creality Ender-3` at `4.0 mm`) keep rigid retraction at `base_retraction_length` (`<= base + 0.5 mm` ceiling) and disable scarf seams by default.
   * **Process Geometry, Optical Glass & Hardware Ceilings**: `top_surface_line_width >= nozzle_diameter` (`>= 0.40 mm` on `0.4 mm`, `>= 0.60 mm` on `0.6 mm`). In `100%` `alignedrectilinear` Optical Glass mode, set `internal_solid_infill_pattern`, `top_surface_pattern`, and `bottom_surface_pattern` to `"alignedrectilinear"` (OrcaSlicer converts `100%` sparse infill into `internal_solid_infill_pattern`, where `"monotonicline"` rotates 90° every layer). Overhang (`overhang_1_4..4_4_speed`, where `"0"` in C++ means disabled/inherit `outer_wall_speed`) and `bridge_speed` must never exceed `outer_wall_speed`, and `travel_acceleration >= default_acceleration`.

4. **Lua Array-Shape Hygiene & Deferred Epilogue Quantization**:
   * Always read numeric values via `scalar_num(config[key], default)` (never bare `tonumber(config[key])`, which evaluates to `nil` on OrcaSlicer single-element array tables `{"220"}` and `%`-suffixed strings `"15%"`).
   * Preserve single-element table shape (`{ val }`) on filament/machine array keys, treat `filament_deretraction_speed = 0` as `"inherit retraction speed"` in OrcaSlicer, and format numbers strictly via `fmt_num` / `fmt_pct` so quantization happens once in Phase 3.3 Epilogue (`format_orca_schema_scalar`).

5. **Permanent Two-Layer Verification Protocol (Ground-Truth C++ Gate + Un-Fakeable Line-by-Line Reading)**:
   * **Layer 1 — Automated Ground-Truth C++ & Vendor Slicer Gate (`//:slicer_ground_truth_gen`)**: Every `bazel test` run validates all exported `machine/*.json`, `process/*.json`, and `filament/*.json` files against `GroundTruthSlicerSchema` compiled directly from OrcaSlicer `PrintConfig.cpp` + `Preset.cpp` and `AnycubicSlicerNext` system profiles (rejecting phantom keys, wrong preset scopes, scalar/vector mismatches, invalid C++ enums, cross-vendor `*_BRASS`/`*_HS` leaks, and unresolved `inherits` parents) plus parent-anchored hardware physics bounds. Never mirror generator formulas in audit assertions.
   * **Layer 2 — Mandatory Line-by-Line Manual Inspection (`--dump-inspection-pack`)**: Automated assertions check only what was anticipated; they **never** replace reading the actual configs. After any change to `constants.json`, `transformations/**/*.lua`, or export logic in `src/threemf.rs` / `src/transformations.rs`:
     1. Run `bazel run -c opt //:audit_random_configs -- --dump-inspection-pack <dir>` to generate bounded `SIDE_BY_SIDE_DIFFS.md` tables (`Stock Parent -> Exported Child -> Effective Merged + C++ Tooltip`) across diverse printers, nozzles, climates, and polymers.
     2. **Read 100% of `SIDE_BY_SIDE_DIFFS.md` line-by-line using `read_file`** (and optionally a read-only `research` subagent with no shell/Python access for adversarial review).
     3. **Never substitute a Python/shell summary script for reading the full config diffs**, and never claim manual inspection unless every line of `SIDE_BY_SIDE_DIFFS.md` was actually read via `read_file`.
