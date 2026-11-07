# Developer, Contributor & Release Guide

This guide covers building, testing, and running **OrcaSlicer WebConfig** locally with Bazel, adding polymers/nozzles/Lua transformations, building the single-file portable Tauri desktop application, and publishing permanent GitHub Releases.

---

## 1. Core Architecture & Development Principles

1. **Rust / WebAssembly Only (`wasm32-unknown-unknown`)**:
   * All UI components, state management, profile parsing, `.3mf` / `.zip` packaging, and Lua transformation evaluation are written in Rust using the **Yew** framework and our pure-Rust sandboxed Lua interpreter ([`src/lua_engine.rs`](../src/lua_engine.rs)).
   * Do not introduce JavaScript application logic; only minimal browser bindings via `wasm-bindgen` / `web-sys` (e.g., standard file input/download blobs and `localStorage`) are used.
   * Because browser sandboxes block access to system configuration directories (`%APPDATA%`), the static web app uses standard file download/upload bundles (`.zip`, `.3mf`, `.json`), while direct local slicer directory access is handled by the Tauri desktop app and local dev server via `cap-std`.
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
* By default, [`server/src/lib.rs`](../server/src/lib.rs) auto-discovers your local `AnycubicSlicerNext` or `OrcaSlicer` configuration directory inside `%APPDATA%` (Windows / WSL `/mnt/c/Users/<user>/AppData/Roaming/`), `~/Library/Application Support/` (macOS), or `~/.config/` (Linux).
* You can override the target directory at any time via `ORCASLICER_LOCAL_DIR=/path/to/slicer` or [`local_slicer_config.json`](../local_slicer_config.json).

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
