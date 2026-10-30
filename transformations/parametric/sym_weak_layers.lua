-- Symptom Fix: Weak Layer Adhesion, Delamination & Brittle Parts
meta = {
  id = "sym_weak_layers",
  name = "Fix Weak Layer Adhesion, Delamination & Brittle Parts",
  icon = "💪",
  category = "Filament Symptom Fix (Strength & Layer Bonding)",
  tree_category = "7. Structural, Mechanical & Multi-Color",
  tree_subcategory = "Inter-Layer Polymer Weld",
  slicer_menu_path = "Nozzle Temp Boost + Relative Max Fan Reduction + Extra Structural Wall Perimeters",
  when_to_run = "Apply when functional parts snap along layer lines, tall thin walls delaminate, or PETG/PCTG/ASA parts feel brittle.",
  formula_summary = "Nozzle_new = Nozzle_old + Temp_Boost  •  Max_Fan_new = Max_Fan_old × (1 - Fan_Trim%)  •  Wall_Loops_new = Wall_Loops_old + Extra_Walls",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Weak inter-layer bonding is caused by excessive part cooling fan freezing the previous layer before polymer chains can entangle, or printing at the bottom of the filament's temp range.",
    "2. Adding +5°C nozzle temperature, scaling continuous max fan down by -25% (0.75×), and adding +1 to +2 extra wall loops dramatically increases Z-axis tensile strength.",
  },
  hardware_checklist = {
    "🚪 Keep Chamber Closed for Structural Prints: Blocking cool room air drafts raises ambient chamber temperature by 10–15°C, dramatically reducing internal residual stress.",
  },
}

params = {
  {
    id = "temp_boost",
    label = "1. How much should we raise the nozzle temperature (°C) relative to current for deeper inter-layer fusion?",
    description = "+5°C to +8°C significantly improves Z-axis layer weld strength.",
    type = "range",
    min = 0,
    max = 20,
    step = 1,
    default = "+5",
    unit = "°C",
    options = {
      { value = "+5", label = "+5°C (Recommended Structural Boost)" },
      { value = "+8", label = "+8°C (Maximum Polymer Fusion)" },
      { value = "0", label = "0°C (Keep Current Temp)" },
    },
  },
  {
    id = "fan_cap",
    label = "2. By what percentage should we scale down continuous part cooling fan speed (fan_max_speed)?",
    description = "Scaling fan_max_speed by -25% (0.75× of your filament's current max fan) prevents over-cooling the weld zone on any polymer.",
    type = "range",
    min = -70,
    max = 0,
    step = 5,
    default = "-25%",
    unit = "%",
    options = {
      { value = "-15%", label = "-15% (0.85× Mild Fan Reduction)" },
      { value = "-25%", label = "-25% (0.75× Recommended Structural Weld)" },
      { value = "-45%", label = "-45% (0.55× Maximum Layer Fusion / Engineering)" },
    },
  },
  {
    id = "wall_loops",
    label = "3. How many EXTRA Wall Perimeters (wall_loops) should we add?",
    description = "Adding +1 to +2 extra walls adds far more bending and impact strength than increasing sparse infill.",
    type = "range",
    min = 0,
    max = 6,
    step = 1,
    default = "+2",
    unit = "walls",
    options = {
      { value = "+1", label = "+1 Extra Wall Loop (Strong)" },
      { value = "+2", label = "+2 Extra Wall Loops (Recommended Heavy-Duty)" },
      { value = "+3", label = "+3 Extra Wall Loops (Maximum Shell Strength)" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local t_boost = math.floor(tonumber(p.temp_boost) or 5)
  local fan_pct = tonumber(p.fan_cap) or -25.0
  local fan_scale = 0.75
  if fan_pct < 0 then
    fan_scale = math.max(0.30, math.min(1.0, (100.0 + fan_pct) / 100.0))
  else
    fan_scale = math.max(0.30, math.min(1.0, (100.0 - fan_pct) / 100.0))
  end
  local fan_drop_label = math.floor((1.0 - fan_scale) * 100.0 + 0.5)
  local extra_loops = math.floor(tonumber(p.wall_loops) or 2)
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Nozzle %+d°C • Max Fan -%.0f%% (%.2f×) • Wall Loops %+d", t_boost, fan_drop_label, fan_scale, extra_loops)
  return {
    title = string.format("Symptom Fix: Layer Bond Strength (%+d°C, -%.0f%% Fan, %+d Walls)", t_boost, fan_drop_label, extra_loops),
    description = string.format("Increases Z-axis layer adhesion and part strength for %s: nozzle %+d°C, scales max fan by %.2f× (-%.0f%%), and adds %+d wall loops.", scope, t_boost, fan_scale, fan_drop_label, extra_loops),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local t_boost = math.floor(tonumber(p and p.temp_boost) or 5)
  local fan_pct = tonumber(p and p.fan_cap) or -25.0
  local fan_scale = 0.75
  if fan_pct < 0 then
    fan_scale = math.max(0.30, math.min(1.0, (100.0 + fan_pct) / 100.0))
  else
    fan_scale = math.max(0.30, math.min(1.0, (100.0 - fan_pct) / 100.0))
  end
  local extra_loops = math.floor(tonumber(p and p.wall_loops) or 2)

  local cur_t = tonumber(config["nozzle_temperature"]) or 240
  config["nozzle_temperature"] = tostring(cur_t + t_boost)
  local cur_fan = tonumber(config["fan_max_speed"]) or 50
  config["fan_max_speed"] = tostring(math.max(10, math.floor(cur_fan * fan_scale + 0.5)))
  local cur_w = tonumber(config["wall_loops"]) or 2
  config["wall_loops"] = tostring(math.max(2, math.floor(cur_w + extra_loops)))
  local d = describe(p or {}, ctx)
  print("[Symptom:LayerStrength] " .. (d.badge or ""))
end
