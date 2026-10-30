-- Symptom Fix: Grinding Filament, Stripped Gears & Extruder Tension (Kobra S1 Screw)
meta = {
  id = "sym_extruder_grinding",
  name = "Grinding Filament, Stripped Gears & Extruder Tension (Kobra S1 Screw)",
  icon = "⚙️",
  category = "1. Extrusion & Flow (Extruder Drive & Tension)",
  tree_category = "1. Extrusion & Flow",
  tree_subcategory = "Extruder Drive & Tension Screw",
  slicer_menu_path = "Hardware Tension Screw + Relative Retraction Speed Scaling + Minimum Travel Distance Boost",
  when_to_run = "Apply when extruder gears chew a half-moon notch into the filament, plastic dust builds up in the gears, or the print stops extruding mid-job.",
  formula_summary = "Retract_Speed_new = Retract_Speed_old × (1 - Slowdown%)  •  Min_Travel_new = Min_Travel_old + Travel_Boost  •  Nozzle {:+}°C",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Extruder grinding happens when (a) the Kobra S1 extruder idler spring tension screw is too loose (gears slip) or too tight (crushes filament over repeated retractions), OR (b) dozens of rapid retractions happen over the exact same 1mm section of filament.",
    "2. Follow the Kobra S1 Extruder Tension Screw diagram first, then apply this rule to scale down retraction speed and increase minimum travel distance between retractions.",
  },
  hardware_checklist = {
    "🔧 Anycubic Kobra S1 Extruder Tension Screw: Locate the recessed 2.0mm hex idler tension screw on the RIGHT side of the printhead cover. Turn CLOCKWISE (CW) 1/4 to 1/2 turn if gears slip on PLA/PETG/PCTG, or COUNTER-CLOCKWISE (CCW) 1/2 turn for soft TPU so the gears don't crush the filament.",
    "🪥 Clean Hobbed Drive Gears: Pop off the magnetic front toolhead cover and use a stiff nylon/brass brush or compressed air to clear shaved plastic dust packed inside the dual extruder teeth.",
    "🧵 Check Spool & ACE Pro Buffer Friction: Verify the filament isn't cross-over tangled on the spool and that the PTFE bowden tube entering the top of the Kobra S1 printhead has a smooth bend radius.",
  },
}

params = {
  {
    id = "retract_spd_cap",
    label = "1. By what percentage should we slow Retraction & Deretraction Speed so the hobbed gears don't strip the filament?",
    description = "Scaling retraction speed by -25% (0.75× of current speed) prevents the drive teeth from shearing the filament surface during rapid reversals.",
    type = "range",
    min = -60,
    max = 0,
    step = 5,
    default = "-25%",
    unit = "%",
    options = {
      { value = "-15%", label = "-15% (0.85× Gentle Gear Engagement)" },
      { value = "-25%", label = "-25% (0.75× Recommended Anti-Grind)" },
      { value = "-35%", label = "-35% (0.65× Ultra-Gentle / Soft TPU)" },
    },
  },
  {
    id = "min_travel",
    label = "2. How much extra Minimum Travel Distance (filament_retraction_minimum_travel, mm) should we add before allowing a retraction?",
    description = "Adding +1.0mm to +1.5mm to minimum travel distance stops the extruder from retracting 10 times on the same tiny detail.",
    type = "range",
    min = 0.0,
    max = 5.0,
    step = 0.5,
    default = "+1.0",
    unit = "mm",
    options = {
      { value = "+0.5", label = "+0.5 mm Extra Min Travel" },
      { value = "+1.0", label = "+1.0 mm Extra Min Travel (Recommended Anti-Chew)" },
      { value = "+1.5", label = "+1.5 mm Extra Min Travel (Dense Details / Mesh)" },
    },
  },
  {
    id = "temp_relief",
    label = "3. Add nozzle temperature (°C) relative to current to reduce hotend back-pressure?",
    description = "+5°C reduces the force required by the extruder gears to push filament through the nozzle.",
    type = "range",
    min = 0,
    max = 15,
    step = 1,
    default = "+5",
    unit = "°C",
    options = {
      { value = "0", label = "0°C (Keep Current)" },
      { value = "+5", label = "+5°C (Recommended Back-Pressure Relief)" },
      { value = "+10", label = "+10°C (High Back-Pressure Fix)" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local r_pct = tonumber(p.retract_spd_cap) or -25.0
  local r_scale = 0.75
  if r_pct < 0 then
    r_scale = math.max(0.40, math.min(1.0, (100.0 + r_pct) / 100.0))
  else
    r_scale = math.max(0.40, math.min(1.0, (100.0 - r_pct) / 100.0))
  end
  local r_drop_label = math.floor((1.0 - r_scale) * 100.0 + 0.5)
  local min_trv_boost = tonumber(p.min_travel) or 1.0
  local t_rel = math.floor(tonumber(p.temp_relief) or 5)
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Retract Speed -%.0f%% (%.2f×) • Min Travel %+.1fmm • Nozzle %+d°C", r_drop_label, r_scale, min_trv_boost, t_rel)
  return {
    title = string.format("Symptom Fix: Extruder Anti-Grind (-%.0f%% Retract Speed, %+.1fmm Min Travel)", r_drop_label, min_trv_boost),
    description = string.format("Stops extruder gears from grinding filament for %s: scales retraction & deretraction speed by %.2f× (-%.0f%%), adds %+.1fmm minimum travel before retracting, and adds %+d°C back-pressure relief.", scope, r_scale, r_drop_label, min_trv_boost, t_rel),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local r_pct = tonumber(p and p.retract_spd_cap) or -25.0
  local r_scale = 0.75
  if r_pct < 0 then
    r_scale = math.max(0.40, math.min(1.0, (100.0 + r_pct) / 100.0))
  else
    r_scale = math.max(0.40, math.min(1.0, (100.0 - r_pct) / 100.0))
  end
  local min_trv_boost = tonumber(p and p.min_travel) or 1.0
  local t_rel = math.floor(tonumber(p and p.temp_relief) or 5)

  local cur_rs = tonumber(config["filament_retraction_speed"]) or 45
  config["filament_retraction_speed"] = tostring(math.max(15, math.floor(cur_rs * r_scale + 0.5)))
  local cur_ds = tonumber(config["filament_deretraction_speed"]) or 35
  config["filament_deretraction_speed"] = tostring(math.max(15, math.floor(cur_ds * r_scale + 0.5)))
  local cur_mt = tonumber(config["filament_retraction_minimum_travel"]) or 1.5
  config["filament_retraction_minimum_travel"] = string.format("%.1f", cur_mt + min_trv_boost)
  local cur_t = tonumber(config["nozzle_temperature"]) or 230
  config["nozzle_temperature"] = tostring(cur_t + t_rel)
  local d = describe(p or {}, ctx)
  print("[Symptom:ExtruderGrind] " .. (d.badge or ""))
end
