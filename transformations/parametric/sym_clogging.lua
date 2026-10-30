-- Symptom Fix: Clogging, Heat Creep, Extruder Clicking & AMS Tip Jams
meta = {
  id = "sym_clogging",
  name = "Fix Clogging, Heat Creep, Extruder Clicking & AMS Tip Jams",
  icon = "🔥",
  category = "Filament Symptom Fix (Clogging & Extruder Skipping)",
  tree_category = "1. Extrusion & Flow",
  tree_subcategory = "Hotend & Throat",
  slicer_menu_path = "Relative Retraction Trim + Melt-Zone Temp / MVS Scaling + AMS Cut Retraction",
  when_to_run = "Apply when the nozzle clogs mid-print, the extruder clicks/skips during fast infill, PLA softens from heat creep, or ACE Pro swaps jam.",
  formula_summary = "Retraction_new = Retraction_old × (1 - Retract_Trim%)  •  MVS_new = MVS_old × (1 - MVS_Drop%)  •  Nozzle {:+}°C",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Heat-Creep Clog (clogging 30–90 min into a print): retraction is pulling softened filament too high into the cold throat — shorten retraction distance by -15% to -25% and avoid excessive chamber heat on PLA.",
    "2. Extruder Clicking on Fast Infill: volumetric flow exceeds melt capacity — add +5°C to +10°C nozzle temp and scale Max Volumetric Speed by -15% to -25% (0.85×–0.75×).",
    "3. ACE Pro / AMS Swap Jam: long cut retraction (> 14mm) pulls molten strands into the selector — scale cut retraction down by -15% (≤ 12.0mm).",
  },
  hardware_checklist = {
    "🧲 Cold Pull (Atomic Pull) Procedure: Heat nozzle to 250°C, push 20mm of natural PLA or Nylon through manually, cool to 95°C (PLA) or 130°C (PETG/PCTG), release the extruder tension lever, and pull firmly straight up to extract carbonized debris.",
    "📍 0.35mm Acupuncture Needle: Heat nozzle to printing temp and gently insert the cleaning needle 15mm up the nozzle orifice to break up bridged carbon-fiber (-CF) clumps.",
    "🚪 Open Door/Lid for PLA Heat Creep: When printing PLA/PLA+ with bed ≥ 60°C, crack the Kobra S1 top glass cover or front door open so chamber temperature stays below 38°C.",
  },
}

params = {
  {
    id = "clog_mode",
    label = "1. What type of clogging or extruder issue are you experiencing?",
    description = "Select the symptom that best matches when the failure happens.",
    type = "enum",
    default = "heat_creep",
    unit = "",
    options = {
      { value = "heat_creep", label = "Heat Creep (Clogs 30+ min into print)" },
      { value = "melt_underrun", label = "Extruder Clicking on Fast Infill (Melt Underrun)" },
      { value = "ams_swap", label = "ACE Pro / AMS Filament Swap Tip Jam" },
      { value = "cf_particle", label = "Carbon / Glass Fiber (-CF/-GF) Orifice Bridging" },
    },
  },
  {
    id = "temp_adj",
    label = "2. How should we adjust the nozzle temperature (°C) relative to current?",
    description = "Use +5°C to +10°C if the extruder clicks during fast extrusion or with -CF filament; use -5°C if PLA is softening from heat creep.",
    type = "range",
    min = -15,
    max = 20,
    step = 1,
    default = "+5",
    unit = "°C",
    options = {
      { value = "-5", label = "-5°C (Cooler — PLA Heat Creep Fix)" },
      { value = "0", label = "0°C (Keep Current Temp)" },
      { value = "+5", label = "+5°C (Lower Viscosity / Easier Flow)" },
      { value = "+10", label = "+10°C (High-Flow / Carbon Fiber Boost)" },
    },
  },
  {
    id = "max_retract",
    label = "3. By what percentage should we shorten Retraction Distance (filament_retraction_length) to keep molten polymer out of the cold throat?",
    description = "Scaling retraction distance by -20% (0.80× of your current retraction) prevents molten polymer from being pulled into the cold heatbreak throat.",
    type = "range",
    min = -50,
    max = 0,
    step = 5,
    default = "-20%",
    unit = "%",
    options = {
      { value = "-10%", label = "-10% (0.90× Subtle Retraction Trim)" },
      { value = "-20%", label = "-20% (0.80× Recommended All-Metal Safe)" },
      { value = "-30%", label = "-30% (0.70× Strict Heat-Creep / CF Trim)" },
    },
  },
  {
    id = "mvs_drop_pct",
    label = "4. By what percentage should we scale down Max Volumetric Speed (mm³/s) for safety?",
    description = "Scaling MVS by -15% to -25% (0.85×–0.75× of your current hotend/filament MVS) stops the extruder from outrunning the heater block.",
    type = "range",
    min = 0,
    max = 50,
    step = 5,
    default = "15",
    unit = "%",
    options = {
      { value = "0", label = "0% (1.00× Keep Current MVS)" },
      { value = "15", label = "-15% MVS (0.85× Recommended Safety Cushion)" },
      { value = "25", label = "-25% MVS (0.75× Stop Extruder Clicking / CF Safe)" },
    },
  },
}

function validate(p, ctx)
  local clog_mode = p.clog_mode or "heat_creep"
  local temp_adj = tonumber(p.temp_adj) or 5
  if clog_mode == "melt_underrun" and temp_adj < 0 then
    return false, "For melt_underrun (extruder clicking on fast infill), lowering nozzle temperature will worsen the clog. Choose temp_adj >= 0°C."
  end
  return true
end

function describe(p, ctx)
  local clog_mode = p.clog_mode or "heat_creep"
  local temp_adj = math.floor(tonumber(p.temp_adj) or 5)
  local ret_pct = tonumber(p.max_retract) or -20.0
  local ret_scale = 0.80
  if ret_pct < 0 then
    ret_scale = math.max(0.50, math.min(1.0, (100.0 + ret_pct) / 100.0))
  else
    ret_scale = math.max(0.50, math.min(1.0, (100.0 - ret_pct) / 100.0))
  end
  local ret_drop_label = math.floor((1.0 - ret_scale) * 100.0 + 0.5)
  local mvs_drop = math.abs(tonumber(p.mvs_drop_pct) or 15.0)
  local scale = math.max(0.5, math.min(1.0, (100.0 - mvs_drop) / 100.0))
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Mode=%s • Nozzle %+d°C • Retract -%.0f%% (%.2f×) • MVS -%.0f%% (%.2f×)", clog_mode, temp_adj, ret_drop_label, ret_scale, mvs_drop, scale)
  return {
    title = string.format("Symptom Fix: Anti-Clog (%+d°C, -%.0f%% Retract, -%.0f%% MVS)", temp_adj, ret_drop_label, mvs_drop),
    description = string.format("Prevents clogging & extruder skipping (%s) for %s: nozzle %+d°C, scales retraction by %.2f× (-%.0f%%), scales MVS by %.2f× (-%.0f%%), and trims AMS cut retraction.", clog_mode, scope, temp_adj, ret_scale, ret_drop_label, scale, mvs_drop),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local temp_adj = math.floor(tonumber(p and p.temp_adj) or 5)
  local ret_pct = tonumber(p and p.max_retract) or -20.0
  local ret_scale = 0.80
  if ret_pct < 0 then
    ret_scale = math.max(0.50, math.min(1.0, (100.0 + ret_pct) / 100.0))
  else
    ret_scale = math.max(0.50, math.min(1.0, (100.0 - ret_pct) / 100.0))
  end
  local mvs_drop = math.abs(tonumber(p and p.mvs_drop_pct) or 15.0)
  local scale = math.max(0.5, math.min(1.0, (100.0 - mvs_drop) / 100.0))

  local cur_t = tonumber(config["nozzle_temperature"]) or 230
  local new_t = cur_t + temp_adj
  config["nozzle_temperature"] = tostring(new_t)
  local cur_r = tonumber(config["filament_retraction_length"]) or 0.80
  config["filament_retraction_length"] = string.format("%.2f", cur_r * ret_scale)
  local cur_mvs = tonumber(config["filament_max_volumetric_speed"]) or 15.0
  config["filament_max_volumetric_speed"] = string.format("%.1f", cur_mvs * scale)
  local cur_cut = tonumber(config["retraction_distances_when_cut"]) or 14.0
  config["retraction_distances_when_cut"] = string.format("%.1f", math.min(12.0, cur_cut * 0.85))
  config["flush_temperature"] = tostring(new_t)
  local d = describe(p or {}, ctx)
  print("[Symptom:AntiClog] " .. (d.badge or ""))
end
