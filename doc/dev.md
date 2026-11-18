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

### Run the Test Suite & Random Configuration Safety Audits
```bash
# Run all unit tests, Lua transformation stack audits, and Python requirement checks
bazel test //...
```
This executes:
* `//:orcaslicer_webconfig_test` — Unit tests for profile import/export, `.3mf` container generation, Lua rules, and catalog round-tripping.
* `//:audit_random_configs_test` — Monte Carlo safety & invariant auditor across randomized combinations of printers, nozzles, filaments, finishes, weather presets, and Lua rule stacks.
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
   * Never stamp a single `bed_temp` across all six OrcaSlicer plate keys (`textured_plate_temp`, `hot_plate_temp`, `eng_plate_temp`, `cool_plate_temp`, `textured_cool_plate_temp`, `supertack_plate_temp`), and never overwrite valid vendor low-temp plate baselines (such as `AnycubicSlicerNext`'s `35°C` `cool_plate_temp` and `40°C` `textured_cool_plate_temp` on `Kobra S1`) with `"0"`.
   * Preserve each resolved parent preset's per-plate availability and Smooth-vs-Textured offset via `instantiated_filament_plates` (`_parent_plate_*`):
     * Low-temp coated plates remain strictly bounded by coating survival limits (`cool_plate_temp <= 60°C`, `textured_cool_plate_temp <= 65°C`, `supertack_plate_temp <= 70°C`), keeping `"0"` where the parent preset declares `"0"` (e.g., Bambu `fdm_filament_abs` `cool_plate_temp = 0`, `Anycubic Kobra S1` `eng_plate_temp = 0`) and always `"0"` on `supertack_plate_temp` for `TPU`/`PEBA` elastomers.
     * Active bed transformations (`sym_weather_climate`, `sym_wet_filament`, `sym_warping_adhesion`) apply as signed deltas relative to `spool_base_bed_c`, with `Kobra S1` / `Kobra S1 Max` / `X1C` / `Voron 2.4` `max_bed_temp_c = 120°C` and `ABS`/`ASA` `safe_bed_max_c = 115°C` (`PC`/`PC-CF` `120°C`) so `+4°C..+5°C` transformations on `110°C` `Anycubic ABS` take full effect (`115°C / 110°C` Textured PEI, `105°C / 100°C` Smooth PEI) instead of being silently clamped to `110 / 110`.
     * Exported `.3mf` `project_settings.config` files explicitly declare `"curr_bed_type": "Textured PEI Plate"`.

3. **Polymer Thermal, Chamber & Kinematics Invariants**:
   * **Warp-Prone Polymers (`ABS`, `ASA`, `PC`, `PA`, `PET-CF`, `HIPS`)**: Keep first-to-continuous bed drop `<= 5°C` (never drop across $T_g$ on Layer 2), `close_fan_the_first_x_layers >= 3` (`>= 1` on all other polymers so Layer 1 never has fan on), `additional_cooling_fan_speed == 0`, `chamber_fan_speed == 0`, `during_print_exhaust_fan_speed == 0` (retain chamber heat during printing), `overhang_fan_speed <= 35%`, and `fan_min_speed < fan_max_speed` (e.g., `10%` continuous perimeter cooling vs `20%` short-layer max).
   * **Elastomers & Extruder Path**: `TPU`/`PEBA` enforce `filament_z_hop = 0` (`Normal Lift`) and `filament_retraction_speed <= 35 mm/s`. Bowden machines (`base_retraction_length >= 2.0 mm`, e.g., `Creality Ender-3` at `4.0 mm`) must scale per-filament retraction to the Bowden path rather than clobbering `filament_retraction_length` to direct-drive `0.8 mm` defaults.
   * **Process Speed Monotonicity & Hardware Ceilings**: Overhang (`overhang_1_4..4_4_speed`) and `bridge_speed` must never exceed the active tier's `outer_wall_speed`. Nozzle and bed temperatures must be clamped to the active machine's hardware ratings (`max_hotend_temp_c`, `max_bed_temp_c`), emitting `SafetySeverity::Danger` warnings when a loaded polymer exceeds the printer's hardware rating.

4. **Lua Array-Shape Hygiene & Deferred Epilogue Quantization**:
   * Always read numeric values via `scalar_num(config[key], default)` (never bare `tonumber(config[key])`, which evaluates to `nil` on OrcaSlicer single-element array tables `{"220"}` and `%`-suffixed strings `"15%"`).
   * Preserve single-element table shape (`{ val }`) on filament/machine array keys, treat `filament_deretraction_speed = 0` as `"inherit retraction speed"` in OrcaSlicer, and format numbers strictly via `fmt_num` / `fmt_pct` so quantization happens once in Phase 3.3 Epilogue (`format_orca_schema_scalar`).

5. **Non-Tautological Auditing & Mandatory Side-by-Side Config Inspection**:
   * **Never mirror generator logic in audit assertions**: Never write a synthetic rule in Lua/Rust and simultaneously assert that same invented value in [`src/bin/audit_random_configs.rs`](../src/bin/audit_random_configs.rs) without first inspecting the actual vendor system presets (`AnycubicSlicerNext` / `@orcaslicer_repo`). Anchor expectations to the resolved `inherits` parent preset and physical hardware specs.
   * **Inspect the effective merged profile (`merge(parent_chain, child_json)`), not child JSON in isolation**: User `.json` files in OrcaSlicer / `AnycubicSlicerNext` are delta overlays on top of `inherits` (and the slicer strips keys equal to the parent on save). Always diff generated child `.json` files **side-by-side against their resolved `inherits` parent preset** to catch both (a) unintended overrides fighting the parent (e.g., clobbering a `35°C` Cool Plate to `0°C`) and (b) omitted optional keys that silently fall back to unsafe parent values (e.g., inheriting `80%` `overhang_fan_speed` or `70%` `during_print_exhaust_fan_speed` on `ABS`).
   * **Verify non-zero transformation deltas at upper/lower boundary presets**: Spot-checking only mid-range vendor spools (e.g., `90–100°C` ABS) misses silent ceiling clamping on boundary presets (such as `110°C` `Anycubic ABS` when `max_bed_temp_c` was mistakenly `110°C`, turning `+4°C..+5°C` rules into `0°C` no-ops). Always verify `x_transformed != x_base` when active rules target a parameter, including at the highest-temperature vendor presets.
   * **Read full diversity across polymer families & vendor imports**: In every 50-config batch (`bazel run -c opt //:audit_random_configs -- --dir /tmp/audit_50 --count 50`), manually open and read complete `machine/*.json`, `process/*.json`, `filament/*.json`, and `.3mf` `project_settings.config` files spanning PLA, PLA-CF, PETG, PETG-CF, PCTG, ABS/ASA, PC/PA, and TPU across both `AnycubicSlicerNext` and upstream `OrcaSlicer` printers (including third-party SUNLU / Siraya Tech / Polymaker vendor entries).
