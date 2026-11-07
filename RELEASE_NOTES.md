## What's New in This Release

* **Single-File Portable Desktop App (Tauri v2):** Self-contained portable executables for Windows (`x64`), Linux (`x64`), and macOS (`Apple Silicon`) with embedded WebAssembly UI, catalog, and capability-sandboxed (`cap-std`) local filesystem scanning (`%APPDATA%\AnycubicSlicerNext` & `%APPDATA%\OrcaSlicer`).
* **3-Phase Transformation Pipeline, PWL Nozzle Curves & Coupled Parameter Physics:**
  * **100% Lua Domain Physics & Deferred Epilogue Quantization:** Evaluates piecewise-linear (PWL) nozzle thermal conductivity (`P10`), FlowTech viscoelastic retraction (`P12`), and DiamondBack PCD anti-stringing (`P25`) curves over an immutable `base[key]` (`x0`) snapshot at full IEEE-754 precision, quantizing once at the end of the Phase 3 Epilogue.
  * **Telescoping Coupled Physics (`sync_coupled_physics`, `coupled_purge_factor`):** Automatically synchronizes Pressure Advance $\text{PA}(T, d, \mu)$, Graetz Max Volumetric Speed $Q_{\text{max}}(T, d, k)$, Nozzle Tip Radiant Part Cooling, and Residence-Time Multi-Color Purge Volumes $\gamma_{\text{purge}}$ across orifice diameters (`0.20mm–1.00mm`), nozzle materials, and temperature shifts.
  * **Phase 3 Epilogue Constraint Projection (`Priority = ∞`):** Enforces rule-declared box bounds (`constrain(key, min, max, reason)`) and cross-key relational invariants (First-Layer Thermal Continuity, Prime $\le$ Pull Speed, Spiral Z-Hop Floor, Fan Max $\ge$ Min, and CoreXY Outer-Wall VFA Guard).
* **Core Slicer Config Packs, Declarative Scoping & Provenance Inspector (`🧬`):**
  * Built-in packs for **Nozzle-Proportional Line Widths**, **Support Detachability & Tree Tuning**, **Top Surface Ironing**, **Dimensional Tolerance**, **Bridges & Overhangs**, **Structural Strength**, **CoreXY Outer-Wall VFA Consistency**, and **Rig-Specific Fast-Start G-Code** (`Anycubic Kobra S1`, `Kobra S1 Max`, and `Bambu Lab X1/P1`).
  * Declarative Polymer / Nozzle / Process Tier scope filters (`RuleScopeFilter`), static Lua rule linting (`lint_lua_rule`), and counterfactual shadow detection (`⚡ Overwritten` vs. `🛡️ Clamped`).
* **Wet / Damp Filament Humidity (`% RH`) Auto-Tuner & Weather Compensation:** Single-slider drybox humidity compensation (`10%–80% RH`) and relative Tropical / Cold weather rules with MicroSwiss heat-break awareness.
* **380+ Printers & Centralized Polymer Catalog (`constants.json`):** Printer-agnostic kinematics classes (`CoreXY` vs. `Bed-Slinger`), per-machine hotend thermal ceilings, `.3mf` ACE Pro / AMS multi-spool I/O, and checked-in Siraya Tech, SUNLU, and Polymaker catalogs.

### Installation Instructions

Download the single portable executable for your platform under **Assets** below and run it directly — no installer or sidecar configuration files required:

* **Windows (`x64`):** Download `orcaslicer-webconfig-portable-windows-x64.exe` and double-click to launch.
* **Linux (`x64`):** Download `orcaslicer-webconfig-portable-linux-x64`, mark it executable (`chmod +x orcaslicer-webconfig-portable-linux-x64`), and run it.
* **macOS (`Apple Silicon`):** Download `orcaslicer-webconfig-portable-macos`, mark it executable (`chmod +x orcaslicer-webconfig-portable-macos`), and run it.
