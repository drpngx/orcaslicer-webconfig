-- Symptom Fix: Wet Filament (Steam Popping, Pockmarks, Bubbles & Matte/Foamy Walls)
meta = {
  id = "sym_wet_filament",
  name = "Wet Filament (Steam Popping, Pockmarks, Bubbles & Matte/Foamy Walls)",
  icon = "💧",
  category = "1. Extrusion & Flow (Moisture & Drying)",
  tree_category = "1. Extrusion & Flow",
  tree_subcategory = "Moisture & Active Drying",
  slicer_menu_path = "ACE Pro Active Dryer + Nozzle Temp Trim + Flow & Retraction Compensation",
  when_to_run = "Apply when you hear tiny 'pop/crackle' sounds from the nozzle, walls have random crater pockmarks, or PETG/PCTG/TPU/Nylon oozes uncontrollably.",
  formula_summary = "Nozzle_new = Nozzle_old + Temp_Drop (Reduces Steam Boil)  •  Flow_new = Flow_old + Flow_Trim  •  Spiral Wipe",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Absorbed moisture turns to superheated steam at 200°C+, exploding into micro-bubbles as it exits the nozzle tip — leaving crater pockmarks and pushing molten strings out during travel.",
    "2. Always dry the spool in the ACE Pro active dryer first! If you must finish a print on slightly damp filament, lowering nozzle temp by -5°C and trimming flow by -0.02 reduces steam expansion.",
  },
  hardware_checklist = {
    "🔥 Run ACE Pro Active Filament Dryer: Enable ACE Pro drying before printing — PLA/PLA+: 45°C for 4h • PETG/PCTG/TPU: 55°C for 6h • ABS/ASA: 60°C for 6h.",
    "👂 Listen at the Nozzle Tip: Steam moisture makes an unmistakable 'pop / crackle / hiss' sound and puffs tiny steam clouds out of the 0.4mm orifice.",
  },
}

params = {
  {
    id = "temp_drop",
    label = "1. Lower nozzle temperature (°C) relative to current to reduce explosive steam boiling inside the melt zone?",
    description = "-5°C noticeably reduces steam popping and foaming when printing slightly damp PETG/PCTG/PLA.",
    type = "range",
    min = -15,
    max = 0,
    step = 1,
    default = "-5",
    unit = "°C",
    options = {
      { value = "-3", label = "-3°C (Mild Dampness)" },
      { value = "-5", label = "-5°C (Recommended Steam Reduction)" },
      { value = "-8", label = "-8°C (Heavy Moisture Mitigation)" },
    },
  },
  {
    id = "flow_trim",
    label = "2. Trim Flow Ratio relative to current to compensate for micro-bubble foaming expansion?",
    description = "-0.02 (-2%) prevents foamy over-extrusion roughness on walls.",
    type = "range",
    min = -0.08,
    max = 0.00,
    step = 0.01,
    default = "-0.02",
    unit = "ratio",
    options = {
      { value = "0.00", label = "0.00 (Unchanged)" },
      { value = "-0.02", label = "-0.02 (-2% Anti-Foam Trim)" },
      { value = "-0.03", label = "-0.03 (-3% Strong)" },
    },
  },
}

function validate(p, ctx)
  local td = tonumber(p.temp_drop) or -5
  if td > 0 then
    return false, "Wet filament temperature mitigation (temp_drop) must be <= 0°C."
  end
  return true
end

function describe(p, ctx)
  local t_drop = math.floor(tonumber(p.temp_drop) or -5)
  local f_trim = tonumber(p.flow_trim) or -0.02
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Nozzle %+d°C (Anti-Steam) • Flow %+.2f (Anti-Foam)", t_drop, f_trim)
  return {
    title = string.format("Symptom Fix: Damp Filament Mitigation (%+d°C, Flow %+.2f)", t_drop, f_trim),
    description = string.format("Reduces steam popping and foaming on slightly damp filament for %s: lowers nozzle %+d°C and trims flow %+.2f.", scope, t_drop, f_trim),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local t_drop = math.floor(tonumber(p and p.temp_drop) or -5)
  local f_trim = tonumber(p and p.flow_trim) or -0.02

  local cur_t = tonumber(config["nozzle_temperature"]) or 240
  config["nozzle_temperature"] = tostring(cur_t + t_drop)
  local cur_f = tonumber(config["filament_flow_ratio"]) or 0.96
  config["filament_flow_ratio"] = string.format("%.3f", cur_f + f_trim)
  config["filament_z_hop_types"] = "Spiral Lift"
  local d = describe(p or {}, ctx)
  print("[Symptom:WetFilament] " .. (d.badge or ""))
end
