# OrcaSlicer WebConfig Development Guidelines

## Core Principles

1. **Rust / WebAssembly Only**:
   - Do NOT use JavaScript for application logic, state, UI, or transformations.
   - Always prefer Rust compiled to WebAssembly (`wasm32-unknown-unknown`) using the Yew framework.
   - Only minimal browser bindings via `wasm-bindgen` / `web-sys` (e.g., File System Access API) are permitted.

2. **Bazel-First Execution**:
   - Everything is Bazel-based.
   - Run all builds, tests, and execution under Bazel (`bazel build ...`, `bazel run ...`, `bazel test ...`).
   - Do NOT run runaway, unmanaged `cargo` commands directly for builds or running the application.

3. **Client-Side Only (Zero Server Storage)**:
   - All profile data, machine rigs, filaments, and Lua transformations are stored locally in the browser / loaded from and saved to local files.
   - Nothing is persisted to a remote server or database.

4. **Transformation-Based Architecture**:
   - Configs are not stored as absolute values; they are modeled as stacks of transformations (e.g. Diamondback nozzle temperature adjustments, PETG stringing mitigation, bed adhesion tuning).
   - Transformations are written in Lua for safe, sandboxed execution and extensibility.
   - Includes conflict/drift resolution: when an external tool modifies a value touched by a transformation, the user can accept or discard the change.
   - One-click export and import of all transformations and configuration stacks.

5. **Direct File System Access**:
   - Uses the File System Access API (`showOpenFilePicker`, `showDirectoryPicker`, `FileSystemFileHandle`) to read and edit OrcaSlicer configuration profiles directly.
   - Supports disconnecting after saves to release file locks.
