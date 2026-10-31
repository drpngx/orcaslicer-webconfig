## What's New in This Release

* **Single-File Portable Desktop App (Tauri v2):** Self-contained portable executables for Windows (`x64`), Linux (`x64`), and macOS (`Apple Silicon`) with embedded WebAssembly UI, catalog, and capability-sandboxed local filesystem scanning (`%APPDATA%\AnycubicSlicerNext` & `%APPDATA%\OrcaSlicer`).
* **Transformation-Based Slicer Architecture (Lua 5.4):** Stackable, sandboxed Lua transformations for nozzle thermal conductivity (`Brass`, `Hardened Steel`, `Diamondback PCD`, `Ruby`, `MicroSwiss CM2/CHT`), volumetric flow ceilings, Scarf Joint seams & Arachne variable walls, multi-material ACE Pro / AMS purge matrix synthesis, and active-foaming TPU Shore A hardness tuning (`60A–92A`).
* **Wet / Damp Filament Humidity (`% RH`) Auto-Tuner & Weather Compensation:** Single-slider drybox humidity compensation (`10%–80% RH`) that automatically trims upper-layer nozzle temperature to stop steam boiling while preserving first-layer adhesion (`+4..+6°C` first-layer bed boost, `1.04–1.06×` bottom flow to seal steam pinholes, slowed `30–35 mm/s` first layer, and anti-steam retraction/wipe).
* **Centralized Polymer & Hardware Constants (`constants.json`):** Printer-agnostic, data-driven polymer physics, safe temperature/bed/flow bounds, Z-hop & retract-before-wipe schedules, and vendor filament catalogs.

### Installation Instructions

Download the single portable executable for your platform under **Assets** below and run it directly — no installer or sidecar configuration files required:

* **Windows (`x64`):** Download `orcaslicer-webconfig-portable-windows-x64.exe` and double-click to launch.
* **Linux (`x64`):** Download `orcaslicer-webconfig-portable-linux-x64`, mark it executable (`chmod +x orcaslicer-webconfig-portable-linux-x64`), and run it.
* **macOS (`Apple Silicon`):** Download `orcaslicer-webconfig-portable-macos`, mark it executable (`chmod +x orcaslicer-webconfig-portable-macos`), and run it.
