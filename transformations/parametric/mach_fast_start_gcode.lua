-- ⚠️ POTENTIALLY DANGEROUS: Startup G-Code (invoked at print start) & Machine G-Code Optimizer
-- The printer family is ALWAYS derived from the active rig's `printer_model` (never chosen manually),
-- because start/end/toolchange G-code is firmware- and geometry-specific:
--   • Anycubic Kobra S1      -> full optimization (unpacked `G9111` sub-macros, X44/Y270 throw position,
--                               Y255 layer-0 rear purge removal, E-14 ACE Pro pre-cut retract, PETG/ABS fix).
--   • Anycubic Kobra S1 Max  -> keeps the stock `G9111` wrapper + S1 Max bed metadata lines, adds ABS/ASA
--                               heat-soak (bed-centre via placeholders), plate/PETG Z-offset, anti-ooze end.
--                               The S1-only sub-macros, throw position, purge & toolchange scripts are NOT used.
--   • Bambu Lab X1 / P1      -> Bambu fast-start, E-18 AMS pre-cut retract, anti-ooze end.
--   • Everything else        -> (Kobra 3 / 4 / X, Kobra 1 / 2 Marlin, Bambu A1 / H2D, other brands)
--                               no optimized G-code: the stock machine G-code is left untouched.
--
-- ⚠️ SAFETY WARNING — WHY THIS TRANSFORMATION IS POTENTIALLY DANGEROUS:
-- 1. NEVER Home Z (`G28 W` / `G28 Z`) Above 140°C ("The Golden Rule"):
--    Both Anycubic Kobra S1 (LeviQ 2.0 strain gauge) and Bambu Lab (load-cell / eddy-current tap) tap
--    the nozzle directly against the PEI build plate to find Z=0. Above 140°C, molten plastic oozes
--    create a false early trigger (printing in mid-air) or melt permanent dimples into PEI plates.
--    This rule ALWAYS enforces `M109 S140` on the silicone pad (`WIPE_STOP`) before `G28 W` / `G28 Z`.
-- 2. Skipping `LEVIQ2_AUTO_ZOFFSET` (`run_leviq2_zoffset = "0"`):
--    Saves ~45–60s per print by reusing the last saved Z-offset. However, if you swap between a
--    Textured PEI plate and a Smooth PEI / CryoGrip plate, or change nozzles without re-running
--    `LEVIQ2_AUTO_ZOFFSET`, the nozzle can gouge the build plate or print in mid-air!
-- 3. Conditional Plate/PETG Z-Offset Placement on Kobra S1:
--    `SET_GCODE_OFFSET` MUST execute AFTER `G9111` (or after `LEVIQ2_AUTO_ZOFFSET`), because `G9111`
--    resets the Z-offset during its probing phase.
-- 4. Adaptive KAMP-Style Prime Line (`prime_line_mode = "adaptive_kamp"`):
--    Draws a 30mm prime strip at `X = first_layer_print_min[0] - 5`. Ensure your model is placed at
--    least 5mm inside the left bed edge (`X >= 5mm`).

meta = {
  id = "mach_fast_start_gcode",
  name = "⚠️ Startup G-Code — Fast-Start, Unpacked G9111 & Multi-Color Poop-Saver (POTENTIALLY DANGEROUS)",
  icon = "⚡",
  category = "Machine Start/End/Toolchange G-Code (⚠️ Potentially Dangerous)",
  tree_category = "3. First Layer & Bed Adhesion",
  tree_subcategory = "Fast-Start G-Code, Z-Offset & Poop-Saver (⚠️ Dangerous)",
  slicer_menu_path = "Printer Settings ➔ Machine G-code (Start / End / Change Filament / Before Layer)",
  when_to_run = "⚠️ POTENTIALLY DANGEROUS: Rig-specific startup G-code (invoked at print start). Automatically picks Kobra S1, Kobra S1 Max, or Bambu Lab X1/P1 G-code from the active rig (other printers keep their stock G-code untouched), and can unpack the Kobra S1 `G9111` startup macro, cut multi-color filament poop by ~45% (`E-14`/`E-18` pre-cut retract), prevent nozzle crust (`E-5`/`E-6` end retract), auto-adjust Textured PEI / PETG Z-offset, auto heat-soak ABS/ASA, and uncap Kobra S1 PETG/ABS flow.",
  formula_summary = "Start: 140°C Z-home guard + N× wipe + Plate/PETG Z-offset (-0.03/+0.02mm) + ABS/ASA 4-5min heat-soak • Change Filament: E-14 (Kobra 0.65× flush) / E-18 (Bambu 0.60× flush) before cut • End: E-5/E-6 anti-ooze retract + silence M106 P2/P3 fans + Z150 smart park • Kobra Fix: PETG 0.70mm retract @ 19 mm³/s, ABS/ASA @ 23 mm³/s",
  default_condition_expr = "all",
  how_to_read_print = {
    "⚠️ DANGER 1 — The Golden Rule (140°C Z-Homing Ceiling) & `LEVIQ2_AUTO_ZOFFSET`: Never home Z above 140°C. Even when Pre-Wipe Temp is 170°C, this rule forces `M109 S140` on the silicone pad (`WIPE_STOP`) before `G28 W` / `G28 Z`. Keep `LEVIQ2_AUTO_ZOFFSET = 1` whenever swapping build plates or nozzles!",
    "💩 Long Retract Before Cut (`change_filament_gcode` — 40–45% Less Multi-Color Poop): Pulling 14mm (Kobra S1 ACE Pro before `T[next_extruder]`) or 18mm (Bambu AMS before cut) of molten filament out of the melt zone leaves the nozzle nearly empty for the next color, safely dropping `flush_multiplier` to `0.65×` (Kobra S1) or `0.60×` (Bambu Lab).",
    "🧲 Conditional Plate & PETG Z-Offset + ABS/ASA Heat-Soak (`machine_start_gcode`): Automatically squishes `-0.03mm` closer into Textured PEI valleys and raises `+0.02mm` for PETG (placed *after* `G9111` / `LEVIQ2_AUTO_ZOFFSET` using Klipper `SET_GCODE_OFFSET` on Kobra S1 or `G29.1` on Bambu), and runs a 4–5 min chamber & bed expansion heat-soak (`M106 P2 S180` 70% Aux fan) *before* probing ONLY when slicing ABS or ASA.",
    "🧼 Anti-Ooze End Retract (`E-5`/`E-6`), Silence Fans & Kobra S1 PETG/ABS Unbottleneck: Immediately retracts 5–6mm at the top of `machine_end_gcode` so the cooling nozzle doesn't drool a hardened crust that ruins the next print's strain-gauge Z-home, silences `M106 P2/P3` chamber/aux fans, parks at a comfortable `Z150`, removes Kobra S1's redundant `Y255` rear purge strip, and fixes Anycubic's stock PETG (`1.5mm` → `0.70mm` retract, `12` → `19 mm³/s` MVS) & ABS/ASA (`23 mm³/s` MVS) bottlenecks.",
  },
  hardware_checklist = {
    "⚠️ Verify Build Plate & Nozzle Before Skipping Z-Offset: If you swapped between Textured PEI, Smooth PEI, or CryoGrip plates, or changed nozzles, keep `5. Strain-Gauge Auto Z-Offset (LEVIQ2_AUTO_ZOFFSET)` set to `1 (Run Every Print — Safe)`!",
    "🔧 Kobra S1 External Spool Mid-Print `PAUSE` (`M600`) Workaround: Stock Kobra OS greys out touchscreen Extrude/Retract buttons during a slicer pause. Either press the right-side lower cutter lever + hold the upper tension lever to manually push new filament through, or install the **Rinkhals** community overlay to unlock Mainsail/Fluidd pause controls.",
    "🎛️ Zero-Code Touch-Screen Checkboxes: Uncheck 'Bed Leveling' (since `BED_MESH_PROFILE LOAD=\"default\"` / `G29 A` loads your saved mesh in 0.1s) and uncheck 'Flow Calibration' once Pressure Advance (`k`) and Flow Ratio are saved in your filament profile.",
  },
}

params = {
  {
    id = "startup_preset",
    label = "1. Complete Machine G-Code Optimization Preset (⚠️ Read Safety Warnings)",
    description = "Curated bundles. The G-code generated always matches the active rig (Kobra S1, Kobra S1 Max, or Bambu Lab X1/P1); knobs that do not apply to the rig are ignored.",
    type = "enum",
    default = "kobra_s1_trimmed_safe",
    options = {
      {
        value = "kobra_s1_trimmed_safe",
        label = "🛡️ Recommended: Safe Fast-Start + Poop-Saver Cut Retract + Anti-Ooze End (Rig-Specific)",
        sets = {
          preheat_wipe_temp_c = "140",
          wipe_passes = "2",
          run_leviq2_zoffset = "1",
          conditional_z_offset = "1",
          chamber_heat_soak_abs = "1",
          prime_line_mode = "stock_front",
          long_retract_before_cut = "auto",
          anti_ooze_end_retract = "1",
          smart_end_bed_park = "1",
          keep_ams_loaded_at_end = "0",
          remove_kobra_rear_purge_line = "1",
          kobra_unbottleneck_petg_abs = "1",
        },
      },
      {
        value = "kobra_s1_ultra_fast_no_zoffset",
        label = "⚠️ Ultra-Fast Same-Plate (Kobra S1: Skips LEVIQ2 Z-Offset — Saves ~2.5 min)",
        sets = {
          preheat_wipe_temp_c = "140",
          wipe_passes = "2",
          run_leviq2_zoffset = "0",
          conditional_z_offset = "1",
          chamber_heat_soak_abs = "1",
          prime_line_mode = "stock_front",
          long_retract_before_cut = "auto",
          anti_ooze_end_retract = "1",
          smart_end_bed_park = "1",
          keep_ams_loaded_at_end = "0",
          remove_kobra_rear_purge_line = "1",
          kobra_unbottleneck_petg_abs = "1",
        },
      },
      {
        value = "kobra_s1_unpacked_1to1",
        label = "🔬 Full 1:1 Unpacked Stock Startup (Kobra S1: 170°C→140°C, 14× Wipe, LEVIQ2 ON)",
        sets = {
          preheat_wipe_temp_c = "170",
          wipe_passes = "14",
          run_leviq2_zoffset = "1",
          conditional_z_offset = "1",
          chamber_heat_soak_abs = "1",
          prime_line_mode = "stock_front",
          long_retract_before_cut = "auto",
          anti_ooze_end_retract = "1",
          smart_end_bed_park = "1",
          keep_ams_loaded_at_end = "0",
          remove_kobra_rear_purge_line = "1",
          kobra_unbottleneck_petg_abs = "1",
        },
      },
      {
        value = "stock_g9111",
        label = "🏭 Keep Stock Startup Macro + Add ABS Heat-Soak, Plate/PETG Z-Offset, Cut Retract & Anti-Ooze End",
        sets = {
          preheat_wipe_temp_c = "170",
          wipe_passes = "14",
          run_leviq2_zoffset = "1",
          conditional_z_offset = "1",
          chamber_heat_soak_abs = "1",
          prime_line_mode = "stock_front",
          long_retract_before_cut = "auto",
          anti_ooze_end_retract = "1",
          smart_end_bed_park = "1",
          keep_ams_loaded_at_end = "0",
          remove_kobra_rear_purge_line = "1",
          kobra_unbottleneck_petg_abs = "1",
        },
      },
      {
        value = "bambu_fast_start",
        label = "🚀 Fast-Start + Adaptive Prime Line Next to Part + Cut Retract + Anti-Ooze End",
        sets = {
          preheat_wipe_temp_c = "140",
          wipe_passes = "2",
          run_leviq2_zoffset = "1",
          conditional_z_offset = "1",
          chamber_heat_soak_abs = "1",
          prime_line_mode = "adaptive_kamp",
          bambu_skip_mech_check = "1",
          bambu_parallel_bed_heat = "1",
          long_retract_before_cut = "auto",
          anti_ooze_end_retract = "1",
          smart_end_bed_park = "1",
          keep_ams_loaded_at_end = "0",
        },
      },
      {
        value = "bambu_fast_same_spool",
        label = "⚠️ Fast Same-Spool Re-Print: Keep Filament Loaded at End (Bambu AMS) + Cut Retract",
        sets = {
          preheat_wipe_temp_c = "140",
          wipe_passes = "2",
          run_leviq2_zoffset = "1",
          conditional_z_offset = "1",
          chamber_heat_soak_abs = "1",
          prime_line_mode = "adaptive_kamp",
          bambu_skip_mech_check = "1",
          bambu_parallel_bed_heat = "1",
          long_retract_before_cut = "auto",
          anti_ooze_end_retract = "1",
          smart_end_bed_park = "1",
          keep_ams_loaded_at_end = "1",
        },
      },
      {
        value = "custom",
        label = "🎛️ Custom Machine G-Code Knobs",
      },
    },
  },
  {
    id = "preheat_wipe_temp_c",
    label = "3. Start G-Code (Kobra S1 / Bambu X1·P1): Initial Pre-Wipe Nozzle Temp (°C) — Enforces 140°C Z-Home Ceiling",
    description = "140°C heats directly to probing temp (skipping the slow 170°C→140°C cooldown). 170°C softens stubborn residue before wiping, then ALWAYS cools back to 140°C on the silicone pad before G28 Z-homing.",
    type = "range",
    min = 135,
    max = 170,
    step = 5,
    default = "140",
    unit = "°C",
    options = {
      { value = "140", label = "140°C Direct (Skips 170°C→140°C Cooldown — Saves ~60s)" },
      { value = "150", label = "150°C Soft-Wipe → 140°C Z-Home Cooldown" },
      { value = "170", label = "170°C Stock Residue Soften → 140°C Z-Home Cooldown" },
    },
  },
  {
    id = "wipe_passes",
    label = "4. Start G-Code (Kobra S1 / Bambu X1·P1): Rear Nozzle Wiper Passes (1–14 Passes)",
    description = "Stock G9111 runs `WIPE_NOZZLE` (14 rapid passes). Setting 2–3 passes calls `WIPE_NOZZLE1` 2–3 times (saving ~25s while still cleaning the tip).",
    type = "range",
    min = 1,
    max = 14,
    step = 1,
    default = "2",
    unit = "passes",
    options = {
      { value = "1", label = "1 Pass (WIPE_NOZZLE1 × 1 — Fastest)" },
      { value = "2", label = "2 Passes (WIPE_NOZZLE1 × 2 — Recommended)" },
      { value = "3", label = "3 Passes (WIPE_NOZZLE1 × 3 — Extra Clean for PETG)" },
      { value = "14", label = "14 Passes (Native WIPE_NOZZLE Full Scrub)" },
    },
  },
  {
    id = "run_leviq2_zoffset",
    label = "5. ⚠️ Start G-Code (Kobra S1 only): Strain-Gauge Auto Z-Offset (`LEVIQ2_AUTO_ZOFFSET`)",
    description = "⚠️ DANGEROUS IF DISABLED: Keep ON (1) to measure exact nozzle Z-offset on every print. Set to OFF (0) ONLY when repeating prints on the exact same build plate and nozzle to save ~45–60s.",
    type = "enum",
    default = "1",
    options = {
      { value = "1", label = "1 — Run LEVIQ2_AUTO_ZOFFSET Every Print (Safe Default)" },
      { value = "0", label = "0 — ⚠️ Skip LEVIQ2_AUTO_ZOFFSET (DANGEROUS if Plate/Nozzle Changed — Saves ~50s)" },
    },
  },
  {
    id = "conditional_z_offset",
    label = "6. Start G-Code: Automatic Textured PEI (-0.03mm) & PETG (+0.02mm) Z-Offset",
    description = "Placed AFTER G9111 / LEVIQ2_AUTO_ZOFFSET / G28 Z. Uses `SET_GCODE_OFFSET Z_ADJUST` on Kobra S1 or `G29.1` on Bambu Lab to squish 0.03mm closer into Textured PEI valleys and raise 0.02mm for PETG.",
    type = "enum",
    default = "1",
    options = {
      { value = "1", label = "1 — Enable Automatic Textured PEI (-0.03mm) & PETG (+0.02mm) Z-Offset" },
      { value = "0", label = "0 — Off (No Conditional Z-Offset Micro-Stepping)" },
    },
  },
  {
    id = "chamber_heat_soak_abs",
    label = "7. Start G-Code: Conditional Chamber Heat-Soak for ABS / ASA (Before Probing)",
    description = "Placed BEFORE G9111 / Z-probing. Runs Aux fan (`M106 P2 S180` at 70%) with nozzle centered over the 100°C+ bed for 4 min (Kobra S1) or 5 min (Bambu) ONLY when slicing ABS or ASA so bed thermal bow stabilizes before probing. Zero time added to PLA/PETG!",
    type = "enum",
    default = "1",
    options = {
      { value = "1", label = "1 — Auto 4–5 Min Chamber Heat-Soak for ABS / ASA Only (0s Penalty on PLA/PETG)" },
      { value = "0", label = "0 — Off (Start Probing Immediately When Bed Sensor Hits Temp)" },
    },
  },
  {
    id = "prime_line_mode",
    label = "8. Start G-Code (Kobra S1 / Bambu X1·P1): Purge / Prime Line Strategy",
    description = "Stock Front Purge Line (`Y1 X180 E40`) or Adaptive KAMP-Style 30mm Prime Line 5mm left of the print (`first_layer_print_min[0]-5`).",
    type = "enum",
    default = "stock_front",
    options = {
      { value = "stock_front", label = "Stock Front Purge Line (Y1 X20→X180 E40 — Safe Anywhere on Bed)" },
      { value = "adaptive_kamp", label = "⚠️ Adaptive KAMP 30mm Prime Next to Part (Requires Model X >= 5mm)" },
    },
  },
  {
    id = "long_retract_before_cut",
    label = "9. Change Filament G-Code (Kobra S1 / Bambu X1·P1): Long Retract Before Cut (Cuts Multi-Color Poop by ~45%)",
    description = "Pulls 14mm (Kobra S1 before `T[next_extruder]`, sets 0.65× flush multiplier) or 18mm (Bambu Lab before cutter, sets 0.60× flush multiplier) of molten filament out of the melt zone before cutting.",
    type = "enum",
    default = "auto",
    options = {
      { value = "auto", label = "Auto by Rig (Kobra S1: E-14 & 0.65× Flush • Bambu X1/P1: E-18 & 0.60× Flush)" },
      { value = "off", label = "Off (Stock Toolchange G-Code & Flush Multiplier)" },
    },
  },
  {
    id = "anti_ooze_end_retract",
    label = "10. End G-Code: Anti-Ooze Retraction (`E-5` / `E-6`) & Silence All Fans (`M106 P2/P3 S0`)",
    description = "Retracts 5mm (Kobra S1) or 6mm (Bambu) + hops Z+2mm at the exact millisecond the print finishes so the nozzle tip stays bone-dry for the next print's strain-gauge Z-home, and silences loud Aux/Chamber exhaust fans.",
    type = "enum",
    default = "1",
    options = {
      { value = "1", label = "1 — Enable Immediate E-5/E-6 Anti-Ooze Retract + Silence Aux/Chamber Fans" },
      { value = "0", label = "0 — Off (Stock 2mm End Retract)" },
    },
  },
  {
    id = "smart_end_bed_park",
    label = "11. End G-Code: Smart `Z150` Bed Drop Height",
    description = "Drops the bed to a comfortable 150mm reach height (`{if max_layer_z < 140}G1 Z150{elsif max_layer_z < 240}G1 Z{max_layer_z+10}{endif}`) without bottoming out the lead screws.",
    type = "enum",
    default = "1",
    options = {
      { value = "1", label = "1 — Smart Z150 Bed Drop (Easy Plate Reach Without Bottoming Out)" },
      { value = "0", label = "0 — Stock Z Lift (max_layer_z + 2mm)" },
    },
  },
  {
    id = "keep_ams_loaded_at_end",
    label = "12. ⚠️ End G-Code (Bambu X1·P1 AMS): Keep Filament Loaded at Print End",
    description = "⚠️ When enabled (1), skips `M620 S255` / `T255` / `M621 S255` unload at the end of a print so the active spool stays in the hotend for back-to-back same-spool prints (saves ~90s + 50mm poop purge).",
    type = "enum",
    default = "0",
    options = {
      { value = "0", label = "0 — Normal Cut & Unload Spool at End of Print (Safe Multi-Color Default)" },
      { value = "1", label = "1 — ⚠️ Keep Filament Loaded at Print End (Fast Same-Spool Re-Print)" },
    },
  },
  {
    id = "remove_kobra_rear_purge_line",
    label = "13. Before Layer Change G-Code (Kobra S1 only): Remove Redundant Rear Purge Line (`Y255`)",
    description = "Recent Anycubic Slicer Next updates added a second `{if layer_num==0} ; PURGE LINE ... Y255` strip inside `before_layer_change_gcode`. Enabling this removes that redundant rear strip.",
    type = "enum",
    default = "1",
    options = {
      { value = "1", label = "1 — Remove Redundant Layer-0 Rear Purge Line (`Y255`) on Kobra S1" },
      { value = "0", label = "0 — Keep Stock `before_layer_change_gcode`" },
    },
  },
  {
    id = "kobra_unbottleneck_petg_abs",
    label = "14. Non-G-Code Fix (Kobra S1 only): Unbottleneck PETG & ABS/ASA Flow + 0.70mm PETG Retract",
    description = "Fixes Anycubic's two biggest factory filament bottlenecks (MPC561 / Kreativling fix): lowers PETG retraction from 1.5mm to 0.70mm (stopping Z-seam holes) and uncaps Max Volumetric Speed to 19 mm³/s on PETG and 23 mm³/s on ABS/ASA.",
    type = "enum",
    default = "1",
    options = {
      { value = "1", label = "1 — Enable Kobra S1 PETG (0.70mm Retract, 19 mm³/s) & ABS/ASA (23 mm³/s) Fix" },
      { value = "0", label = "0 — Off (Leave Filament Retraction & Volumetric Flow Unchanged)" },
    },
  },
  {
    id = "bambu_skip_mech_check",
    label = "15. Start G-Code (Bambu X1·P1 only): Skip Resonance Buzz Sweep (`mech mode fast check`)",
    description = "Skips the loud XY vibration frequency sweep before every print on Bambu Lab X1 / P1 (saves ~20s).",
    type = "enum",
    default = "1",
    options = {
      { value = "1", label = "1 — Skip Mech Mode Buzz Sweep (Quiet & Saves ~20s)" },
      { value = "0", label = "0 — Keep Stock Mech Mode Resonance Check" },
    },
  },
  {
    id = "bambu_parallel_bed_heat",
    label = "16. Start G-Code (Bambu X1·P1 only): Parallel Bed Preheat During Homing & Wipe",
    description = "Starts bed heating (`M140`) in the background while homing XY and wiping the nozzle, waiting (`M190`) only right before `G28 Z` (saves ~45–60s).",
    type = "enum",
    default = "1",
    options = {
      { value = "1", label = "1 — Parallel Bed Heating (`M140` + `M190` Before G28 Z)" },
      { value = "0", label = "0 — Sequential Wait for Bed Temp Before Homing" },
    },
  },
}

-- Returns the rig's G-code family (single return value; see `rig_model_label` for the display name):
--   "kobra_s1"     Anycubic Kobra S1 (full optimization)
--   "kobra_s1_max" Anycubic Kobra S1 Max (stock G9111 wrapper + generic add-ons only)
--   "bambu_lab"    Bambu Lab X1 / X1 Carbon / X1E / P1S / P1P
--   "none"         any other printer (stock machine G-code left untouched)
--   "unknown"      no rig printer model in context (describe-time only)
local function rig_model_label(ctx)
  local raw = (ctx and ctx.printer_model) or ""
  if raw == "" then
    return "unknown printer"
  end
  return raw
end

local function resolve_printer_family(p, ctx)
  local model = string.lower((ctx and ctx.printer_model) or "")
  if model == "" then
    return "unknown"
  end
  if string.find(model, "kobra s1 max", 1, true) then
    return "kobra_s1_max"
  end
  if string.find(model, "kobra s1", 1, true) then
    return "kobra_s1"
  end
  if string.find(model, "bambu", 1, true) then
    if string.find(model, "x1", 1, true) or string.find(model, "p1", 1, true) then
      return "bambu_lab"
    end
  end
  return "none"
end

local function build_abs_heat_soak_block(p, fam)
  if ((p and p.chamber_heat_soak_abs) or "1") == "0" then
    return nil
  end
  if fam == "bambu_lab" then
    return table.concat({
      "; --- Conditional Chamber Heat-Soak Only for ABS / ASA (5 min) ---",
      "{if filament_type[initial_extruder]==\"ABS\" or filament_type[initial_extruder]==\"ASA\"}",
      "  M140 S[first_layer_bed_temperature] ; Set bed to 100C+",
      "  M106 P2 S180 ; Turn on Aux fan at 70% to circulate warm air off the bed",
      "  G28 X Y",
      "  G1 X125 Y125 Z10 F6000 ; Park nozzle close to center of bed",
      "  M190 S[first_layer_bed_temperature] ; Wait for bed to reach target",
      "  G4 S300 ; Dwell 5 minutes (300s) for chamber & bed thermal expansion to stabilize",
      "  M106 P2 S0 ; Turn off Aux fan before homing Z",
      "{endif}",
    }, "\n")
  end
  if fam == "kobra_s1_max" then
    return table.concat({
      "; --- Conditional Chamber Heat-Soak BEFORE G9111 / Probing for ABS / ASA (4 min) ---",
      "{if filament_type[initial_tool]==\"ABS\" or filament_type[initial_tool]==\"ASA\"}",
      "  M140 S[first_layer_bed_temperature] ; Start heating bed to 100C+",
      "  M106 P2 S180 ; Run Aux Fan at 70% to circulate heat off the bed",
      "  G28 ; Quick home so we can park near the bed",
      "  G1 X{print_bed_max[0]*0.5} Y{print_bed_max[1]*0.5} Z15 F6000 ; Center toolhead over bed",
      "  M190 S[first_layer_bed_temperature] ; Wait for bed sensor to hit target",
      "  G4 S240 ; Heat-soak for 4 minutes (240s) so bed expansion stabilizes before probing",
      "  M106 P2 S0 ; Turn off Aux fan before G9111 probing",
      "{endif}",
    }, "\n")
  end
  return table.concat({
    "; --- Conditional Chamber Heat-Soak BEFORE G9111 / Probing for ABS / ASA (4 min) ---",
    "{if filament_type[initial_tool]==\"ABS\" or filament_type[initial_tool]==\"ASA\"}",
    "  M140 S[first_layer_bed_temperature] ; Start heating bed to 100C+",
    "  M106 P2 S180 ; Run Kobra S1 Aux Fan at 70% to circulate heat off the bed",
    "  G28 ; Quick home so we can park near the bed",
    "  G1 X125 Y125 Z15 F6000 ; Center toolhead over bed",
    "  M190 S[first_layer_bed_temperature] ; Wait for bed sensor to hit target",
    "  G4 S240 ; Heat-soak for 4 minutes (240s) so aluminum bed expansion stabilizes before probing",
    "  M106 P2 S0 ; Turn off Aux fan before starting Z-offset probing",
    "{endif}",
  }, "\n")
end

local function build_conditional_z_offset_block(p, fam)
  if ((p and p.conditional_z_offset) or "1") == "0" then
    return nil
  end
  if fam == "bambu_lab" then
    return table.concat({
      "; --- Automatic Plate & Filament Z-Offset Tuning (After G28 Z) ---",
      "{if curr_bed_type==\"Textured PEI Plate\"}",
      "  G29.1 Z-0.03 ; Squish 0.03mm closer into textured PEI valleys",
      "{endif}",
      "{if filament_type[initial_extruder]==\"PETG\"}",
      "  G29.1 Z+0.02 ; Raise nozzle 0.02mm for PETG to prevent nozzle buildup",
      "{endif}",
    }, "\n")
  end
  return table.concat({
    (fam == "kobra_s1_max")
      and "; --- Automatic Plate & PETG Z-Offset Tuning (MUST be AFTER G9111, which resets the Z-offset) ---"
      or "; --- Automatic Plate & PETG Z-Offset Tuning (MUST be AFTER G9111 / LEVIQ2_AUTO_ZOFFSET) ---",
    "{if curr_bed_type==\"Textured PEI Plate\"}",
    "  SET_GCODE_OFFSET Z_ADJUST=-0.03 MOVE=1 ; Squish 0.03mm closer into textured PEI valleys",
    "{endif}",
    "{if filament_type[initial_tool]==\"PETG\"}",
    "  SET_GCODE_OFFSET Z_ADJUST=+0.02 MOVE=1 ; Raise 0.02mm so PETG doesn't stick to nozzle",
    "{endif}",
  }, "\n")
end

local function build_prime_line_gcode(prime_mode, fam)
  if prime_mode == "adaptive_kamp" then
    return table.concat({
      "; 5. ⚠️ Adaptive KAMP-Style Short Prime Line (5mm left of part — requires model X >= 5mm)",
      "G90",
      "M83",
      "G1 Z2.0 F1200",
      "G1 X{first_layer_print_min[0] - 5} Y{first_layer_print_min[1]} F12000",
      "G1 Z0.8 F1200",
      "G92 E0",
      "G1 Y{first_layer_print_min[1] + 30} E35 F360",
      "G1 E-0.5 F1800",
      "G1 Y{first_layer_print_min[1] + 35} F6000",
      "G92 E0",
      "M400",
    }, "\n")
  end

  if fam == "bambu_lab" then
    return table.concat({
      "; 5. Stock Front Prime Line",
      "G90",
      "M83",
      "G1 X20 Y2 Z0.28 F12000",
      "G92 E0",
      "G1 X160 E30 F600",
      "G1 E-0.5 F1800",
      "G92 E0",
      "M400",
    }, "\n")
  end

  return table.concat({
    "; 5. Stock Anycubic Front Purge Line",
    "G90",
    "G1 Z0.28 F1200",
    "G92 E0",
    "G1 X20 Y1 F6000",
    "G1 X180 E40 F500",
    "M400",
    "G92 E0",
    "G1 E-0.5 F300",
    "M400",
  }, "\n")
end

local function build_kobra_s1_start_gcode(p)
  local preset = (p and p.startup_preset) or "kobra_s1_trimmed_safe"
  local soak_block = build_abs_heat_soak_block(p, "kobra_s1")
  local z_block = build_conditional_z_offset_block(p, "kobra_s1")

  if preset == "stock_g9111" then
    local stock_lines = {}
    if soak_block then
      table.insert(stock_lines, soak_block)
    end
    table.insert(stock_lines, "G9111 bedTemp=[first_layer_bed_temperature] extruderTemp=[first_layer_temperature[initial_tool]]")
    if z_block then
      table.insert(stock_lines, z_block)
    end
    table.insert(stock_lines, "M117")
    return table.concat(stock_lines, "\n")
  end

  local preheat_t = math.max(135, math.min(170, math.floor((tonumber(p and p.preheat_wipe_temp_c) or 140) + 0.5)))
  local wipe_n = math.max(1, math.min(14, math.floor((tonumber(p and p.wipe_passes) or 2) + 0.5)))
  local run_zoffset = ((p and p.run_leviq2_zoffset) or "1") ~= "0"
  local prime_mode = (p and p.prime_line_mode) or "stock_front"

  local lines = {}
  if preset == "kobra_s1_unpacked_1to1" then
    table.insert(lines, "; --- 1:1 Unpacked G9111 Sequence for Anycubic Kobra S1 ---")
  else
    table.insert(lines, "; --- ⚠️ Fast-Start Trimmed G9111 for Anycubic Kobra S1 ---")
  end
  table.insert(lines, "G90 ; Absolute positioning")
  table.insert(lines, "M83 ; Relative extrusion")
  if soak_block then
    table.insert(lines, "")
    table.insert(lines, soak_block)
  end
  table.insert(lines, "")
  table.insert(lines, "; 1. Park over poop chute and preheat")
  table.insert(lines, "M140 S[first_layer_bed_temperature] ; Start heating bed in background")
  table.insert(lines, "TO_THROW_POSITION ; Move to poop chute (X47 Y276)")
  if preheat_t > 140 then
    table.insert(lines, string.format("M109 S%d ; Heat nozzle to %dC to soften residue before wipe", preheat_t, preheat_t))
  else
    table.insert(lines, string.format("M109 S%d ; Heat directly to %dC probing temp (skips 170C->140C cycle)", preheat_t, preheat_t))
  end
  table.insert(lines, "M190 S[first_layer_bed_temperature] ; Wait for bed temp")
  table.insert(lines, "")
  table.insert(lines, "; 2. Native Rear Wiper & Silicone Pad Routine")
  table.insert(lines, "CLOSE_STEALTHCHOP STEPPER=stepper_x")
  table.insert(lines, "CLOSE_STEALTHCHOP STEPPER=stepper_y")
  table.insert(lines, "WIPE_ENTER ; Move to wiper entry (X81 Y273)")
  if wipe_n >= 14 then
    table.insert(lines, "WIPE_NOZZLE ; Execute 14x native brush wipe")
  else
    for i = 1, wipe_n do
      table.insert(lines, string.format("WIPE_NOZZLE1 ; Single brush wipe pass (%d/%d)", i, wipe_n))
    end
  end
  table.insert(lines, "WIPE_STOP ; Park on silicone pad (X75 Y273) to seal nozzle against oozing")
  if preheat_t > 140 then
    table.insert(lines, "M109 S140 ; GOLDEN RULE: Cool nozzle to 140C on silicone pad before Z-homing!")
  end
  table.insert(lines, "WIPE_EXIT ; Exit wiper area (X83 Y250)")
  table.insert(lines, "OPEN_STEALTHCHOP STEPPER=stepper_x")
  table.insert(lines, "OPEN_STEALTHCHOP STEPPER=stepper_y")
  table.insert(lines, "")
  table.insert(lines, "; 3. Homing, Auto Z-Offset & Saved Bed Mesh")
  table.insert(lines, "BED_MESH_CLEAR ; Clear active mesh before probing")
  table.insert(lines, "G28 W ; Home all axes with clean 140C nozzle")
  if run_zoffset then
    table.insert(lines, "LEVIQ2_AUTO_ZOFFSET ; Measure & apply strain-gauge Z-offset on clean 140C nozzle")
  else
    table.insert(lines, "; ⚠️ LEVIQ2_AUTO_ZOFFSET skipped by user knob (saves ~50s — DO NOT swap build plates without re-enabling!)")
  end
  table.insert(lines, "M400 ; Wait for moves to finish")
  table.insert(lines, "BED_MESH_PROFILE LOAD=\"default\" ; Load saved bed mesh in 0.1s")
  if z_block then
    table.insert(lines, "")
    table.insert(lines, z_block)
  end
  table.insert(lines, "")
  table.insert(lines, "; 4. Final Heat to First-Layer Print Temp")
  table.insert(lines, "TO_THROW_POSITION ; Park over poop chute during final heat")
  table.insert(lines, "M109 S[first_layer_temperature[initial_tool]] I1 ; Wait for 1st-layer nozzle temp")
  table.insert(lines, "")
  table.insert(lines, build_prime_line_gcode(prime_mode, "kobra_s1"))
  table.insert(lines, "M117")
  table.insert(lines, "; --- End Kobra S1 Startup G-Code ---")
  return table.concat(lines, "\n")
end

-- Kobra S1 Max: keep the stock G9111 wrapper and the stock bed/first-layer metadata comment lines
-- (Kobra OS reads them), adding only firmware-agnostic add-ons around it.
local function build_kobra_s1_max_start_gcode(p)
  local lines = { "; --- Kobra S1 Max: Stock G9111 Startup Macro + Add-Ons ---" }
  local soak_block = build_abs_heat_soak_block(p, "kobra_s1_max")
  if soak_block then
    table.insert(lines, soak_block)
  end
  table.insert(lines, "G9111 bedTemp=[first_layer_bed_temperature] extruderTemp=[first_layer_temperature[initial_tool]]")
  table.insert(lines, "M117")
  local z_block = build_conditional_z_offset_block(p, "kobra_s1_max")
  if z_block then
    table.insert(lines, z_block)
  end
  table.insert(lines, "; print_bed_min = {print_bed_min[0]},{print_bed_min[1]}")
  table.insert(lines, "; print_bed_max = {print_bed_max[0]},{print_bed_max[1]}")
  table.insert(lines, "; print_bed_size = {print_bed_size[0]},{print_bed_size[1]}")
  table.insert(lines, "; first_layer_print_min = {first_layer_print_min[0]},{first_layer_print_min[1]}")
  table.insert(lines, "; first_layer_print_max = {first_layer_print_max[0]},{first_layer_print_max[1]}")
  table.insert(lines, "; first_layer_print_size = {first_layer_print_size[0]},{first_layer_print_size[1]}")
  table.insert(lines, "; first_layer_fan_speed = 2, {int(additional_cooling_fan_speed[initial_tool] * 255 / 100)}")
  return table.concat(lines, "\n")
end

local function build_bambu_start_gcode(p)
  local preheat_t = math.max(135, math.min(170, math.floor((tonumber(p and p.preheat_wipe_temp_c) or 140) + 0.5)))
  local wipe_n = math.max(1, math.min(14, math.floor((tonumber(p and p.wipe_passes) or 2) + 0.5)))
  local skip_mech = ((p and p.bambu_skip_mech_check) or "1") ~= "0"
  local parallel_bed = ((p and p.bambu_parallel_bed_heat) or "1") ~= "0"
  local prime_mode = (p and p.prime_line_mode) or "adaptive_kamp"
  local soak_block = build_abs_heat_soak_block(p, "bambu_lab")
  local z_block = build_conditional_z_offset_block(p, "bambu_lab")

  local lines = {
    "; --- ⚠️ Bambu Lab (X1 / P1) Fast-Start G-Code ---",
    "G90",
    "M83",
  }
  if soak_block then
    table.insert(lines, "")
    table.insert(lines, soak_block)
  end
  if parallel_bed then
    table.insert(lines, "; 1. Parallel Bed + Nozzle Preheat (saves ~45-60s)")
    table.insert(lines, "M140 S[bed_temperature_initial_layer_single] ; Start bed heat in background")
    table.insert(lines, string.format("M104 S%d ; Start nozzle preheat to %dC", preheat_t, preheat_t))
  else
    table.insert(lines, "; 1. Sequential Bed Preheat")
    table.insert(lines, "M190 S[bed_temperature_initial_layer_single] ; Wait for bed temp first")
    table.insert(lines, string.format("M109 S%d ; Wait for nozzle preheat %dC", preheat_t, preheat_t))
  end
  table.insert(lines, "")
  table.insert(lines, "; 2. XY Homing & Rear Wiper Passes")
  table.insert(lines, "G28 X Y ; Home XY while bed heats")
  table.insert(lines, string.format("M109 S%d ; Ensure nozzle at %dC for rear wipe", preheat_t, preheat_t))
  for i = 1, math.min(wipe_n, 6) do
    table.insert(lines, string.format("G1 X95 Y255 F12000 ; Rear nozzle wiper pass (%d/%d)", i, math.min(wipe_n, 6)))
    table.insert(lines, "G1 X65 Y255 F12000")
  end
  if preheat_t > 140 then
    table.insert(lines, "M109 S140 ; GOLDEN RULE: Cool nozzle to 140C before Z-homing on PEI plate!")
  end
  table.insert(lines, "")
  if skip_mech then
    table.insert(lines, "; 3. Mech Mode Fast Check Skipped (saves ~20s + eliminates resonance buzz)")
  else
    table.insert(lines, ";===== mech mode fast check =====")
    table.insert(lines, "M1002 gcode_claim_action : 3")
    table.insert(lines, "G1 X128 Y128 Z10 F18000")
    table.insert(lines, "M400 P200")
  end
  table.insert(lines, "")
  table.insert(lines, "; 4. Wait for Bed Temp & Home Z at Clean 140C Nozzle")
  table.insert(lines, "M190 S[bed_temperature_initial_layer_single] ; Ensure bed is at target temp before Z probe")
  table.insert(lines, "G28 Z ; Home Z with clean 140C nozzle")
  table.insert(lines, "G29 A ; Activate saved bed mesh compensation")
  if z_block then
    table.insert(lines, "")
    table.insert(lines, z_block)
  end
  table.insert(lines, "G1 X65 Y250 F12000 ; Park over rear chute for final heat")
  table.insert(lines, "M109 S[nozzle_temperature_initial_layer] ; Heat to 1st-layer print temp")
  table.insert(lines, "")
  table.insert(lines, build_prime_line_gcode(prime_mode, "bambu_lab"))
  table.insert(lines, "; --- End Bambu Lab Fast-Start G-Code ---")
  return table.concat(lines, "\n")
end

local function build_end_gcode(p, fam)
  local anti_ooze = ((p and p.anti_ooze_end_retract) or "1") ~= "0"
  local smart_park = ((p and p.smart_end_bed_park) or "1") ~= "0"
  local keep_ams = ((p and p.keep_ams_loaded_at_end) or "0") == "1"
  if not anti_ooze and not smart_park and not keep_ams then
    return nil
  end

  local lines = {}
  if fam == "bambu_lab" then
    table.insert(lines, "; --- Bambu Lab Optimized Machine End G-Code ---")
    if anti_ooze then
      table.insert(lines, "; 1. Anti-Ooze End Retract (Stops nozzle crust on next print's G28 Z)")
      table.insert(lines, "M400 ; Wait for final print move to finish")
      table.insert(lines, "G92 E0")
      table.insert(lines, "G1 E-6 F1800 ; Quickly retract 6mm to relieve all nozzle pressure")
      table.insert(lines, "G91 ; Relative positioning")
      table.insert(lines, "G1 Z2 F3000 ; Hop Z up 2mm cleanly off the part")
      table.insert(lines, "G90 ; Back to absolute positioning")
    else
      table.insert(lines, "M400")
      table.insert(lines, "G92 E0")
      table.insert(lines, "G1 E-1.5 F2400")
    end
    if smart_park then
      table.insert(lines, "; 2. Smart Z150 Bed Park Height")
      table.insert(lines, "{if max_layer_z < 140}")
      table.insert(lines, "  G1 Z150 F3000 ; Drop bed to comfortable 150mm reach height")
      table.insert(lines, "{elsif max_layer_z < 240}")
      table.insert(lines, "  G1 Z{max_layer_z + 10} F3000 ; Drop 10mm below tall print")
      table.insert(lines, "{endif}")
    else
      table.insert(lines, "G1 Z{min(max_layer_z + 3, printable_height)} F900")
    end
    table.insert(lines, "G1 X65 Y245 F12000 ; Park at rear")
    if keep_ams then
      table.insert(lines, "; ⚠️ M620 S255 / T255 / M621 S255 AMS cut & unload intentionally skipped (Keep AMS Loaded)")
    else
      table.insert(lines, "; Standard AMS unload on multi-material print completion")
      table.insert(lines, "G92 E0")
      table.insert(lines, "G1 E-18 F1200 ; Long retract out of melt zone before cut")
      table.insert(lines, "M620 S255")
      table.insert(lines, "T255")
      table.insert(lines, "M621 S255")
    end
    table.insert(lines, "; 3. Silence All Fans & Heaters Immediately")
    table.insert(lines, "M104 S0 ; Turn off nozzle heater")
    table.insert(lines, "M140 S0 ; Turn off bed heater")
    table.insert(lines, "M106 S0 ; Turn off part cooling fan")
    table.insert(lines, "M106 P2 S0 ; Turn off auxiliary side fan")
    table.insert(lines, "M106 P3 S0 ; Turn off rear chamber exhaust fan")
    table.insert(lines, "M84 ; Disable steppers")
    return table.concat(lines, "\n")
  end

  if fam == "kobra_s1_max" then
    if not anti_ooze and not smart_park then
      return nil
    end
    table.insert(lines, "; --- Anycubic Kobra S1 Max Optimized Machine End G-Code ---")
    table.insert(lines, "M400 ; Wait for final print move to complete")
    if anti_ooze then
      table.insert(lines, "; 1. Anti-Ooze End Retract")
      table.insert(lines, "G92 E0")
      table.insert(lines, "G1 E-5 F1800 ; Retract 5mm immediately to prevent nozzle oozing while cooling")
      table.insert(lines, "G91 ; Relative positioning")
      table.insert(lines, "G1 Z2 F2400 ; Lift Z 2mm cleanly off the print")
      table.insert(lines, "G90 ; Back to absolute positioning")
    end
    if smart_park then
      table.insert(lines, "; 2. Smart Z150 Bed Drop Height")
      table.insert(lines, "{if max_layer_z < 140}")
      table.insert(lines, "  G1 Z150 F3000 ; Drop bed to clean, comfortable 150mm reach height")
      table.insert(lines, "{elsif max_layer_z < max_print_height-1}")
      table.insert(lines, "  G1 Z{z_offset+min(max_layer_z+10, max_print_height)} F900 ; Drop 10mm below tall print")
      table.insert(lines, "{endif}")
    end
    table.insert(lines, "; 3. Stock Kobra S1 Max Heater & Fan Shutdown")
    table.insert(lines, "M140 S0 ; turn off heatbed")
    table.insert(lines, "M104 S0 ; turn off temperature")
    table.insert(lines, "M141 S0 ; turn off box temperature")
    table.insert(lines, "M106 P1 S0 ; turn off fan")
    table.insert(lines, "M106 P2 S0")
    table.insert(lines, "M106 P3 S0")
    table.insert(lines, "M84 ; disable stepper motors")
    return table.concat(lines, "\n")
  end

  table.insert(lines, "; --- Anycubic Kobra S1 Optimized Machine End G-Code ---")
  if anti_ooze then
    table.insert(lines, "; 1. Anti-Ooze End Retract (Put at very top before Kobra S1 parks at X44 Y270)")
    table.insert(lines, "M400 ; Wait for final print move to complete")
    table.insert(lines, "G92 E0")
    table.insert(lines, "G1 E-5 F1800 ; Retract 5mm immediately to prevent nozzle oozing while cooling")
    table.insert(lines, "G91 ; Relative positioning")
    table.insert(lines, "G1 Z2 F2400 ; Lift Z 2mm cleanly off the print")
    table.insert(lines, "G90 ; Back to absolute positioning")
  else
    table.insert(lines, "G92 E0")
    table.insert(lines, "G1 E-2 F3000")
  end
  if smart_park then
    table.insert(lines, "; 2. Smart Z150 Bed Drop Height")
    table.insert(lines, "{if max_layer_z < 140}")
    table.insert(lines, "  G1 Z150 F3000 ; Drop bed to clean, comfortable 150mm reach height")
    table.insert(lines, "{elsif max_layer_z < max_print_height-1}")
    table.insert(lines, "  G1 Z{z_offset+min(max_layer_z+10, max_print_height)} F900 ; Drop 10mm below tall print")
    table.insert(lines, "{endif}")
  else
    table.insert(lines, "{if max_layer_z < max_print_height-1}G1 Z{z_offset+min(max_layer_z+2, max_print_height)} F900 ; Move print head further up")
    table.insert(lines, "{endif}")
  end
  table.insert(lines, "G1 F12000 ; present print")
  table.insert(lines, "G1 X44 ; throw_position_x")
  table.insert(lines, "G1 Y270 ; throw_position_y")
  if keep_ams then
    table.insert(lines, "; ⚠️ Spool kept staged in toolhead for rapid same-spool restart (Rinkhals / single-color)")
  end
  table.insert(lines, "; 3. Silence All Fans & Heaters Immediately")
  table.insert(lines, "M140 S0 ; turn off heatbed")
  table.insert(lines, "M104 S0 ; turn off temperature")
  table.insert(lines, "M106 S0 ; turn off part cooling fan")
  table.insert(lines, "M106 P1 S0 ; turn off fan P1")
  table.insert(lines, "M106 P2 S0 ; turn off Aux fan")
  table.insert(lines, "M106 P3 S0 ; turn off Chamber exhaust/filter fan")
  table.insert(lines, "M84 ; disable stepper motors")
  return table.concat(lines, "\n")
end

local function build_change_filament_gcode(p, fam)
  local mode = (p and p.long_retract_before_cut) or "auto"
  if mode == "off" or (fam ~= "kobra_s1" and fam ~= "bambu_lab") then
    return nil
  end
  if fam == "bambu_lab" then
    local gcode = table.concat({
      "; --- Long Retract Before Filament Cut (Bambu AMS — Cuts Multi-Color Poop by ~45%) ---",
      "M400",
      "G92 E0",
      "G1 E-18 F1200 ; Pull 18mm of molten filament out of the melt zone before cutting",
      "G92 E0",
      "M620 S[next_extruder]A",
      "M204 S9000",
      "G1 Z{max_layer_z + 3.0} F1200",
      "G1 X70 F21000",
      "G1 Y245",
      "G1 Y265 F3000",
      "M400",
      "M106 P1 S0",
      "M106 P2 S0",
      "T[next_extruder]",
      "M621 S[next_extruder]A",
    }, "\n")
    return { gcode = gcode, flush = "0.60" }
  end

  local gcode = table.concat({
    "; FLUSH_START",
    "; --- Long Retract Before ACE Pro Cut (Kobra S1 — Pulls 14mm out of melt zone before T[next_extruder]) ---",
    "M400",
    "G92 E0",
    "G1 E-14 F1200 ; Pull 14mm of molten filament up out of the melt zone before cutter strikes",
    "G92 E0",
    "T[next_extruder] ; change extruder (ACE Pro internal cut & purge)",
    "; FLUSH_END",
  }, "\n")
  return { gcode = gcode, flush = "0.65" }
end

function validate(p, ctx)
  local preheat_t = tonumber(p and p.preheat_wipe_temp_c) or 140
  if preheat_t < 130 or preheat_t > 180 then
    return false, "Pre-wipe nozzle temperature must be between 130°C and 180°C (Z-homing always cools to 140°C)."
  end
  local wipe_n = tonumber(p and p.wipe_passes) or 2
  if wipe_n < 1 or wipe_n > 14 then
    return false, "Wipe passes must be between 1 and 14."
  end
  return true
end

function describe(p, ctx)
  local fam = resolve_printer_family(p, ctx)
  local model = rig_model_label(ctx)
  local preset = (p and p.startup_preset) or "kobra_s1_trimmed_safe"
  local preheat_t = math.max(135, math.min(170, math.floor((tonumber(p and p.preheat_wipe_temp_c) or 140) + 0.5)))
  local wipe_n = math.max(1, math.min(14, math.floor((tonumber(p and p.wipe_passes) or 2) + 0.5)))
  local run_zoffset = ((p and p.run_leviq2_zoffset) or "1") ~= "0"
  local cond_z = ((p and p.conditional_z_offset) or "1") ~= "0"
  local soak_abs = ((p and p.chamber_heat_soak_abs) or "1") ~= "0"
  local cut_mode = (p and p.long_retract_before_cut) or "auto"
  local anti_ooze = ((p and p.anti_ooze_end_retract) or "1") ~= "0"
  local keep_ams = ((p and p.keep_ams_loaded_at_end) or "0") == "1"
  local unbottleneck = ((p and p.kobra_unbottleneck_petg_abs) or "1") ~= "0"

  if fam == "unknown" then
    return {
      title = "⚠️ Startup G-Code — Rig-Specific Fast-Start & Poop-Saver (POTENTIALLY DANGEROUS)",
      description = "⚠️ POTENTIALLY DANGEROUS: Generates startup / end / toolchange G-code matched to the active rig: Anycubic Kobra S1 (unpacked G9111), Kobra S1 Max (stock G9111 + add-ons), or Bambu Lab X1/P1. Any other printer keeps its stock machine G-code unchanged.",
      badge = "⚠️ Startup G-Code: Rig-Specific (Kobra S1 / S1 Max / Bambu X1·P1)",
    }
  end

  if fam == "none" then
    return {
      title = string.format("Startup G-Code — No Optimized G-Code for %s", model),
      description = string.format(
        "No optimized startup / end / toolchange G-code is available for %s, so its stock machine G-code is left unchanged. Optimized G-code exists for Anycubic Kobra S1, Kobra S1 Max, and Bambu Lab X1 / P1.",
        model
      ),
      badge = string.format("Startup G-Code: Stock (no optimized G-code for %s)", model),
    }
  end

  if fam == "kobra_s1_max" then
    local badge = string.format(
      "⚠️ Kobra S1 Max: Stock G9111%s%s%s",
      soak_abs and " • ABS Heat-Soak" or "",
      cond_z and " • Plate/PETG Z-Offset" or "",
      anti_ooze and " • E-5 Anti-Ooze End" or ""
    )
    return {
      title = "⚠️ Startup G-Code: Kobra S1 Max Stock G9111 + Add-Ons",
      description = string.format(
        "⚠️ POTENTIALLY DANGEROUS: Keeps the Kobra S1 Max stock G9111 startup macro and bed metadata lines%s%s%s. Kobra S1-only sub-macros, throw position, rear-purge removal, toolchange script, and PETG/ABS fix are NOT applied to the S1 Max.",
        soak_abs and ", adds a 4-min ABS/ASA bed heat-soak before probing" or "",
        cond_z and ", SET_GCODE_OFFSET (-0.03mm Textured PEI / +0.02mm PETG) after G9111" or "",
        anti_ooze and ", E-5 anti-ooze end retract + Z150 park" or ""
      ),
      badge = badge,
    }
  end

  if fam == "bambu_lab" then
    local skip_mech = ((p and p.bambu_skip_mech_check) or "1") ~= "0"
    local parallel_bed = ((p and p.bambu_parallel_bed_heat) or "1") ~= "0"
    local cut_tag = (cut_mode ~= "off") and " • E-18 Cut (0.60× Flush)" or ""
    local badge = string.format(
      "⚠️ Bambu Fast-Start: %d°C→140°C Z-Home • %d× Wipe%s%s%s%s",
      preheat_t,
      math.min(wipe_n, 6),
      parallel_bed and " • Parallel Bed" or "",
      skip_mech and " • No Mech Buzz" or "",
      cut_tag,
      keep_ams and " • Keep AMS Loaded" or ""
    )
    return {
      title = string.format("⚠️ Startup G-Code: Bambu Lab X1/P1 Fast-Start & Poop-Saver (%d°C Wipe, %d× Pass%s)", preheat_t, math.min(wipe_n, 6), cut_tag),
      description = string.format(
        "⚠️ POTENTIALLY DANGEROUS: Custom Bambu Lab start/end/toolchange G-code (%s, %d× rear wipe at %d°C with 140°C Z-home guard, %s%s%s%s).",
        parallel_bed and "parallel M140 bed heat" or "sequential bed heat",
        math.min(wipe_n, 6),
        preheat_t,
        skip_mech and "mech resonance buzz skipped" or "stock mech check",
        cond_z and ", Textured PEI -0.03mm / PETG +0.02mm Z-offset" or "",
        (cut_mode ~= "off") and ", E-18 pre-cut retract (0.60× flush)" or "",
        anti_ooze and ", E-6 anti-ooze end retract + Z150 park" or ""
      ),
      badge = badge,
    }
  end

  local z_tag = (preset == "stock_g9111") and "Stock G9111" or (run_zoffset and "LEVIQ2 Z-Offset ON" or "⚠️ NO Z-OFFSET (DANGEROUS)")
  local wipe_tag = (preset == "stock_g9111") and "14× Wipe" or ((wipe_n >= 14) and "14× WIPE_NOZZLE" or string.format("%d× WIPE_NOZZLE1", wipe_n))
  local cut_tag = (cut_mode ~= "off") and " • E-14 Cut (0.65×)" or ""
  local fix_tag = unbottleneck and " • PETG 0.7mm/19mm³s" or ""
  local probe_tag = (preheat_t > 140) and string.format("%d°C Wipe → 140°C Z-Probe → Print Temp", preheat_t) or "140°C Z-Probe → Print Temp"
  local badge = string.format(
    "⚠️ Kobra S1 G9111: %s • %s • %s%s%s",
    probe_tag,
    wipe_tag,
    z_tag,
    cut_tag,
    fix_tag
  )
  return {
    title = string.format("⚠️ Startup G-Code: Kobra S1 Unpacked G9111 & Poop-Saver (%s, %s, %s%s)", probe_tag, wipe_tag, z_tag, cut_tag),
    description = string.format(
      "⚠️ POTENTIALLY DANGEROUS: Tunes Kobra S1 start/end/toolchange G-code (%s, %s%s%s%s%s%s).",
      (preset == "stock_g9111") and "stock G9111 wrapper" or string.format("%d°C pre-wipe -> 140°C silicone-pad Z-home guard, %s", preheat_t, wipe_tag),
      z_tag,
      cond_z and ", SET_GCODE_OFFSET (-0.03mm Textured PEI / +0.02mm PETG)" or "",
      soak_abs and ", 4-min ABS/ASA chamber heat-soak" or "",
      (cut_mode ~= "off") and ", E-14 pre-cut retract (0.65× flush)" or "",
      anti_ooze and ", E-5 anti-ooze end retract + Z150 park" or "",
      unbottleneck and ", PETG 0.70mm retract @ 19 mm³/s & ABS 23 mm³/s" or ""
    ),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local fam = resolve_printer_family(p, ctx)
  if fam == "none" or fam == "unknown" then
    print(string.format(
      "[Machine:StartupGCode] No optimized G-code for '%s' — stock machine G-code left unchanged",
      rig_model_label(ctx)
    ))
    return
  end
  local start_gcode = ""
  if fam == "bambu_lab" then
    start_gcode = build_bambu_start_gcode(p)
  elseif fam == "kobra_s1_max" then
    start_gcode = build_kobra_s1_max_start_gcode(p)
  else
    start_gcode = build_kobra_s1_start_gcode(p)
  end
  config["machine_start_gcode"] = start_gcode

  local end_gcode = build_end_gcode(p, fam)
  if end_gcode then
    config["machine_end_gcode"] = end_gcode
  end

  local change_res = build_change_filament_gcode(p, fam)
  if change_res then
    config["change_filament_gcode"] = change_res.gcode
    if change_res.flush then
      config["flush_multiplier"] = change_res.flush
    end
  end

  if fam == "kobra_s1" and ((p and p.remove_kobra_rear_purge_line) or "1") ~= "0" then
    config["before_layer_change_gcode"] = "; BEFORE_LAYER_CHANGE [layer_num] @ [layer_z]mm\nG92 E0"
  end

  if fam == "kobra_s1" and ((p and p.kobra_unbottleneck_petg_abs) or "1") ~= "0" then
    config["_kobra_unbottleneck_petg_abs"] = "1"
    local ftype = string.upper((ctx and ctx.filament_type) or "")
    if string.find(ftype, "PETG", 1, true) then
      local cur_ret = tonumber(config["filament_retraction_length"]) or 1.0
      if cur_ret > 0.80 then
        config["filament_retraction_length"] = "0.70"
      end
      local cur_mvs = tonumber(config["filament_max_volumetric_speed"]) or 12.0
      if cur_mvs < 18.0 then
        config["filament_max_volumetric_speed"] = "19.0"
      end
    elseif string.find(ftype, "ABS", 1, true) or string.find(ftype, "ASA", 1, true) then
      local cur_ret = tonumber(config["filament_retraction_length"]) or 0.80
      if cur_ret > 0.80 then
        config["filament_retraction_length"] = "0.70"
      end
      local cur_mvs = tonumber(config["filament_max_volumetric_speed"]) or 15.0
      if cur_mvs < 22.0 then
        config["filament_max_volumetric_speed"] = "23.0"
      end
    end
  end

  local desc = describe(p, ctx)
  print(string.format("[Machine:StartupGCode] %s", desc.badge))
end
