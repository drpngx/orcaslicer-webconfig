-- Calibration Print #1: Temperature Tower Calibration
meta = {
  id = "cal_temp_tower",
  name = "Temperature Tower Calibration",
  icon = "🌡️",
  category = "Calibration Print #1 (Temperature)",
  tree_category = "Post-Print Calibration",
  tree_subcategory = "Calibration Test",
  slicer_menu_path = "Calibration → Temperature",
  when_to_run = "Run 1st (before Flow Rate or Pressure Advance) to find the optimal melt temperature for layer strength vs. stringing.",
  formula_summary = "Sets nozzle_temperature = Best_Floor_Temp and nozzle_temperature_initial_layer = Best_Floor_Temp + First_Layer_Boost",
  default_condition_expr = "filament:PCTG",
  how_to_read_print = {
    "1. Print the Temperature Tower for your polymer (e.g. 265°C down to 240°C for PCTG/PETG, or 230°C down to 195°C for PLA).",
    "2. Inspect each floor's overhang curve, bridge underside, and stringing cones.",
    "3. Try flexing/snapping the tower between floors: pick the lowest temperature floor that still has strong layer-to-layer bonding and a consistent surface sheen.",
  },
  hardware_checklist = {
    "🖨️ Slice and print the built-in test model from the AnycubicSlicerNext / OrcaSlicer top 'Calibration' menu before entering your measurements below.",
  },
}

params = {
  {
    id = "best_temp",
    label = "1. Which temperature floor (°C) had the best balance of strong layer bonding and low stringing?",
    description = "Remember: Hardened Steel nozzles typically read ~5°C higher than Brass for the same polymer melt temperature.",
    type = "range",
    min = 175,
    max = 320,
    step = 1,
    default = "255",
    unit = "°C",
    options = {
      { value = "205", label = "205°C (PLA Brass)" },
      { value = "215", label = "215°C (PLA+ / PLA-CF HS)" },
      { value = "220", label = "220°C (PLA+ HS)" },
      { value = "235", label = "235°C (PETG HS)" },
      { value = "255", label = "255°C (PCTG Low-Stringing HS)" },
      { value = "260", label = "260°C (PCTG High-Strength / ABS HS)" },
      { value = "275", label = "275°C (PA6 Nylon HS)" },
    },
  },
  {
    id = "first_layer_boost",
    label = "2. How much hotter should Layer 1 be for bed adhesion (°C)?",
    description = "+5°C on the initial layer lowers melt viscosity so the first layer wets into the Textured PEI plate.",
    type = "range",
    min = -10,
    max = 20,
    step = 1,
    default = "5",
    unit = "°C",
    options = {
      { value = "0", label = "+0°C (Same as continuous)" },
      { value = "5", label = "+5°C (Recommended)" },
      { value = "10", label = "+10°C (Extra first-layer tack)" },
    },
  },
}

function validate(p, ctx)
  local best_temp = tonumber(p.best_temp) or 255
  local boost = tonumber(p.first_layer_boost) or 5
  local init_temp = best_temp + boost
  if init_temp > 320 then
    return false, string.format("Initial layer temperature (%d°C) exceeds the 320°C hotend thermal safety limit.", init_temp)
  end
  if init_temp < 175 then
    return false, string.format("Initial layer temperature (%d°C) is below the 175°C cold-extrusion threshold.", init_temp)
  end
  return true
end

function describe(p, ctx)
  local best_temp = math.floor(tonumber(p.best_temp) or 255)
  local boost = math.floor(tonumber(p.first_layer_boost) or 5)
  local init_temp = math.max(180, math.min(350, best_temp + boost))
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Nozzle = %d°C Continuous / %d°C First Layer (%+d°C)", best_temp, init_temp, boost)
  return {
    title = string.format("Calibration: Temp Tower %d°C (%d°C Layer 1)", best_temp, init_temp),
    description = string.format("Post-print Temperature Tower calibration for %s: sets nozzle_temperature = %d°C and initial layer = %d°C.", scope, best_temp, init_temp),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local best_temp = math.floor(tonumber(p and p.best_temp) or 255)
  local boost = math.floor(tonumber(p and p.first_layer_boost) or 5)
  local init_temp = math.max(180, math.min(350, best_temp + boost))
  config["nozzle_temperature"] = tostring(best_temp)
  config["nozzle_temperature_initial_layer"] = tostring(init_temp)
  local d = describe(p or {}, ctx)
  print("[Calibration:TempTower] " .. (d.badge or ""))
end
