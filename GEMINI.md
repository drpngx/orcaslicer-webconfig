# OrcaSlicer WebConfig Development Guidelines

## Core Principles

1. **Rust / WebAssembly Only**:
   - Do NOT use JavaScript for application logic, state, UI, or transformations.
   - Always prefer Rust compiled to WebAssembly (`wasm32-unknown-unknown`) using the Yew framework.
   - Only minimal browser bindings via `wasm-bindgen` / `web-sys` (e.g., standard file input/download blobs and `localStorage`) are permitted.

2. **Bazel-First Execution**:
   - Everything is Bazel-based.
   - Run all builds, tests, and execution under Bazel (`bazel build ...`, `bazel run ...`, `bazel test ...`).
   - Always launch the web server via `HTTP_LISTEN_ADDR=0.0.0.0:8003 bazel run //server:server` (or `HTTP_LISTEN_ADDR=0.0.0.0:8003 bazel run -c opt //server:server`). Never invoke raw `./bazel-bin/server/server` binaries directly.
   - Do NOT run runaway, unmanaged `cargo` commands directly for builds or running the application.

3. **Client-Side Only (Zero Server Storage)**:
   - All profile data, machine rigs, filaments, and Lua transformations are stored locally in the browser / loaded from and saved to local files.
   - Nothing is persisted to a remote server or database.

4. **Transformation-Based Architecture (100% Lua Domain Physics & Deferred Epilogue Quantization)**:
   - Configs are not stored as absolute values; they are modeled as stacks of transformations (e.g. Diamondback nozzle temperature adjustments, PETG stringing mitigation, bed adhesion tuning, weather/climate adjustments, wet filament tuning).
   - **All domain physics and parameter transformations MUST live in Lua (`transformations/*.lua`), never in Rust (`src/*.rs`)**:
     - Do NOT duplicate or hardcode weather/climate deltas, wet-filament offsets, nozzle conductivity curves, or process/filament parameter math in Rust (`src/transformations.rs`, `src/constants.rs`, `src/threemf.rs`).
     - Rust is strictly responsible for UI state, File System I/O, executing the Lua transformation stack, enforcing Phase 3 Epilogue box/relational constraints declared by Lua rules, and final schema quantization.
   - **Deferred Epilogue Quantization Only (No Intermediate `string.format` Rounding)**:
     - During Phase 2 (`P1 .. PN` Lua rules) and Phase 3 Epilogue constraint projections, keep all numeric config values at full IEEE-754 floating-point precision (`map_num` / `fmt_num` / `tostring(x)` with `_defer_quantization = "1"`).
     - Never quantize or format numbers early inside Lua `apply(config, ctx)` rules (do not write `string.format("%.0f", ...)` or `string.format("%.2f", ...)` onto `config` keys) or mid-pipeline in Rust; format/quantize ONCE at the very end of `TransformationEngine::evaluate_stack` via `format_orca_schema_scalar`.
   - Includes conflict/drift resolution: when an external tool modifies a value touched by a transformation, the user can accept or discard the change.
   - One-click export and import of all transformations and configuration stacks.

5. **File Import/Export & Local Slicer Directory Access**:
   - In the Static Browser Web App, uses standard `<input type="file">` uploads and `Blob` downloads (`.zip`, `.3mf`, `.json`) because browser sandboxes block access to system configuration directories (`%APPDATA%`). Do NOT use or document the Chromium File System Access API (`showOpenFilePicker`, `showDirectoryPicker`, `FileSystemFileHandle`).
   - In the Portable Desktop App (Tauri) and local dev server, reads and writes directly to the local slicer directory using capability-based filesystem sandboxing (`cap-std`).

6. **2-Space Rust Indentation**:
   - All Rust files (`*.rs`) must use **2 spaces** for indentation (`tab_spaces = 2` in `rustfmt.toml`).
   - Do not use 4-space indentation in Rust files.

7. **Local Slicer Configuration Reference Paths & Clean Startup**:
   - The user's real AnycubicSlicerNext configuration directory on this machine is `/mnt/c/Users/drpng/AppData/Roaming/AnycubicSlicerNext/` (Windows path: `C:\Users\drpng\AppData\Roaming\AnycubicSlicerNext\` or `%APPDATA%\AnycubicSlicerNext\`).
   - The web application must **default to nothing loaded** on initial startup (no pre-imported user profiles), clearly prompting the user to connect/bind their local slicer directory first.
   - Always distinguish between **our internal transformation-based data structures** (machine rigs, nozzles, add-ons, base filaments + Lua transformation stacks) and **the raw configuration files on the user's disk**.
