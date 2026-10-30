-- Symptom Fix: Under-Extrusion (Gaps Between Lines & Thin/Spongy Walls)
meta = {
  id = "sym_under_extrusion",
  name = "Under-Extrusion (Gaps Between Lines & Thin/Spongy Walls)",
  icon = "📉",
  category = "1. Extrusion & Flow (Flow & Volume)",
  tree_category = "1. Extrusion & Flow",
  tree_subcategory = "Flow & Volume",
  slicer_menu_path = "Filament Flow Ratio + Max Volumetric Speed + Nozzle Temperature",
  when_to_run = "Apply when adjacent lines on top/bottom surfaces don't touch, perimeter walls have gaps, or layers look spongy and porous.",
  formula_summary = "Flow_new = Flow_old + Flow_Boost  •  MVS_new = MVS_old × (1 - Speed_Trim%)  •  Nozzle {:+}°C",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Inspect flat top surfaces and perimeters: if you can see the layer below through gaps between parallel lines, the extruder is pushing less plastic than the slicer expects.",
    "2. Check if under-extrusion happens EVERYWHERE (Flow Ratio too low or filament diameter undersized) or ONLY on fast long lines (Max Volumetric Speed outrunning the heater block).",
  },
  hardware_checklist = {
    "📏 Check Extruder Tension & Gear Teeth: If the Kobra S1 extruder idler screw on the right side of the toolhead is too loose, the drive gears slip under back-pressure — tighten 1/4 turn clockwise.",
    "🌡️ Verify Nozzle Silicone Sock & Thermistor: Make sure the silicone sock is seated around the heater block so part-cooling air doesn't chill the nozzle tip.",
  },
}

params = {
  {
    id = "flow_boost",
    label = "1. How much should we increase the Flow Ratio (filament_flow_ratio) relative to current?",
    description = "+0.02 (+2%) closes minor line gaps; +0.04 (+4%) fixes noticeable under-extrusion.",
    type = "range",
    min = 0.00,
    max = 0.12,
    step = 0.01,
    default = "+0.03",
    unit = "ratio",
    options = {
      { value = "+0.02", label = "+0.02 (+2% Mild Gaps)" },
      { value = "+0.03", label = "+0.03 (+3% Recommended)" },
      { value = "+0.05", label = "+0.05 (+5% Heavy Under-Extrusion)" },
    },
  },
  {
    id = "mvs_trim",
    label = "2. Reduce Max Volumetric Speed (%) relative to current so fast infill/walls don't outrun the melt zone?",
    description = "Scaling MVS by -15% (0.85×) gives the polymer longer residence time in the heater block.",
    type = "range",
    min = 0,
    max = 50,
    step = 5,
    default = "15",
    unit = "%",
    options = {
      { value = "0", label = "0% (1.00× Keep Current MVS)" },
      { value = "15", label = "-15% MVS (0.85× Recommended)" },
      { value = "25", label = "-25% MVS (0.75× High-Viscosity Fix)" },
    },
  },
  {
    id = "temp_boost",
    label = "3. Raise nozzle temperature (°C) relative to current to lower melt viscosity?",
    description = "+5°C reduces back-pressure inside the nozzle orifice.",
    type = "range",
    min = 0,
    max = 20,
    step = 1,
    default = "+5",
    unit = "°C",
    options = {
      { value = "0", label = "0°C (Keep Current)" },
      { value = "+5", label = "+5°C (Easier Melt Flow)" },
      { value = "+10", label = "+10°C (High-Speed Boost)" },
    },
  },
}

function validate(p, ctx)
  local fb = tonumber(p.flow_boost) or 0.03
  if fb < 0 then
    return false, "Under-extrusion flow_boost must be >= 0.00."
  end
  return true
end

function describe(p, ctx)
  local f_boost = tonumber(p.flow_boost) or 0.03
  local mvs_trim = math.abs(tonumber(p.mvs_trim) or 15.0)
  local t_boost = math.floor(tonumber(p.temp_boost) or 5)
  local scale = math.max(0.5, math.min(1.0, (100.0 - mvs_trim) / 100.0))
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Flow %+.2f • MVS -%.0f%% (%.2f×) • Nozzle %+d°C", f_boost, mvs_trim, scale, t_boost)
  return {
    title = string.format("Symptom Fix: Under-Extrusion (Flow %+.2f, -%.0f%% MVS, %+d°C)", f_boost, mvs_trim, t_boost),
    description = string.format("Closes under-extrusion line gaps for %s: boosts flow ratio by %+.2f, scales MVS by %.2f×, and adds %+d°C nozzle temp.", scope, f_boost, scale, t_boost),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local f_boost = tonumber(p and p.flow_boost) or 0.03
  local mvs_trim = math.abs(tonumber(p and p.mvs_trim) or 15.0)
  local t_boost = math.floor(tonumber(p and p.temp_boost) or 5)
  local scale = math.max(0.5, math.min(1.0, (100.0 - mvs_trim) / 100.0))

  local cur_f = tonumber(config["filament_flow_ratio"]) or 0.96
  config["filament_flow_ratio"] = string.format("%.3f", cur_f + f_boost)
  local cur_mvs = tonumber(config["filament_max_volumetric_speed"]) or 15.0
  config["filament_max_volumetric_speed"] = string.format("%.1f", cur_mvs * scale)
  local cur_t = tonumber(config["nozzle_temperature"]) or 230
  config["nozzle_temperature"] = tostring(cur_t + t_boost)
  local d = describe(p or {}, ctx)
  print("[Symptom:UnderExtrusion] " .. (d.badge or ""))
end
