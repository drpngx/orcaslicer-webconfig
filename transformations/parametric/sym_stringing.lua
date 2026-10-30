-- Symptom Fix: Stringing, Cobwebs & Travel Oozing
meta = {
  id = "sym_stringing",
  name = "Fix Stringing, Cobwebs & Travel Oozing",
  icon = "🕸️",
  category = "Filament Symptom Fix (Stringing & Oozing)",
  tree_category = "2. Oozing, Stringing & Blobs",
  tree_subcategory = "Travel Retraction & Z-Hop",
  slicer_menu_path = "Filament Settings → Setting Overrides (Retraction, Z-Hop, Wipe) & Cooling",
  when_to_run = "Apply when a filament (especially PETG, PCTG, PLA+, or TPU) leaves fine hairs, cobwebs, or branching strings across travel moves.",
  formula_summary = "Nozzle_new = Nozzle_old + Temp_Drop  •  Retraction_new = Retraction_old + Retract_Boost  •  Deretract × 0.75  •  Spiral Z-Hop (0.75× Layer_Height)",
  default_condition_expr = "filament:PCTG",
  how_to_read_print = {
    "1. Fine hair-like cobwebs are usually caused by the nozzle lifting straight up (Normal Z-Hop) pulling a molten strand — switching to Spiral Z-Hop + 70% Retract-Before-Wipe shears the strand cleanly.",
    "2. Thick branching strings or heavy oozing happen when melt-zone viscosity is too low (nozzle temp 3–8°C too hot) or retraction length is slightly too short.",
    "3. Answer the questions below to generate a relative anti-stringing Lua rule that scales cleanly across any nozzle or hotend.",
  },
  hardware_checklist = {
    "🔥 Dry Filament First in ACE Pro: Even perfect retraction settings cannot stop stringing if absorbed moisture is boiling into steam pressure inside the melt zone (dry PETG/PCTG at 55°C for 4–6h).",
    "🧽 Wipe Nozzle Exterior: Heat nozzle to 220°C and wipe any coated polymer film off the outside of the brass/steel tip with a folded paper towel or brass brush.",
  },
}

params = {
  {
    id = "temp_drop",
    label = "1. How much should we lower the nozzle temperature (°C) relative to current?",
    description = "-3°C is ideal for mild cobwebs (like PCTG); -5°C to -8°C stops heavy branching strings.",
    type = "range",
    min = -20,
    max = 0,
    step = 1,
    default = "-3",
    unit = "°C",
    options = {
      { value = "0", label = "0°C (Keep Current Temp)" },
      { value = "-3", label = "-3°C (Mild Stringing / PCTG)" },
      { value = "-5", label = "-5°C (Moderate Stringing / PETG)" },
      { value = "-8", label = "-8°C (Heavy Oozing)" },
    },
  },
  {
    id = "retract_boost",
    label = "2. How much extra retraction distance (mm) should we add to your current retraction?",
    description = "Adding +0.15mm to +0.25mm relieves nozzle tip pressure before travel without pulling molten plastic into the cold heatbreak.",
    type = "range",
    min = 0.00,
    max = 0.80,
    step = 0.05,
    default = "+0.15",
    unit = "mm",
    options = {
      { value = "+0.00", label = "+0.00 mm (Keep Current)" },
      { value = "+0.10", label = "+0.10 mm (Subtle)" },
      { value = "+0.15", label = "+0.15 mm (Recommended)" },
      { value = "+0.25", label = "+0.25 mm (Strong)" },
      { value = "+0.35", label = "+0.35 mm (Max Direct-Drive Boost)" },
    },
  },
  {
    id = "z_hop_style",
    label = "3. Which Z-Hop trajectory should be used on travel moves?",
    description = "Spiral (Corkscrew) and Slope wipe sideways as the nozzle lifts (0.75× layer height) so sticky PETG/PCTG strands snap off cleanly.",
    type = "enum",
    default = "Spiral Lift",
    unit = "",
    options = {
      { value = "Spiral Lift", label = "Spiral / Corkscrew (0.75× Layer Height — Recommended)" },
      { value = "Slope Lift", label = "Slope / Diagonal Ramp (0.90× Layer Height)" },
      { value = "Normal Lift", label = "Normal Vertical Lift (1.00× Layer Height)" },
    },
  },
  {
    id = "pre_wipe",
    label = "4. What percentage of retraction should happen BEFORE the wipe motion (filament_retract_before_wipe)?",
    description = "70%–80% relieves pressure while the nozzle is still wiping inside the inner wall.",
    type = "enum",
    default = "70%",
    unit = "",
    options = {
      { value = "50%", label = "50% (Light)" },
      { value = "70%", label = "70% (Recommended for PCTG/PETG)" },
      { value = "85%", label = "85% (Aggressive Anti-Stringing)" },
      { value = "100%", label = "100% (Full Pre-Wipe)" },
    },
  },
}

function validate(p, ctx)
  local temp_drop = tonumber(p.temp_drop) or -3
  local retract_boost = tonumber(p.retract_boost) or 0.15
  if temp_drop > 0 then
    return false, "Anti-stringing temperature adjustment (temp_drop) must be <= 0°C."
  end
  if retract_boost > 0.80 then
    return false, "Retraction boost > +0.80mm risks pulling molten polymer into the cold throat."
  end
  return true
end

function describe(p, ctx)
  local temp_drop = math.floor(tonumber(p.temp_drop) or -3)
  local retract_boost = tonumber(p.retract_boost) or 0.15
  local z_hop = p.z_hop_style or "Spiral Lift"
  if z_hop == "Spiral" then z_hop = "Spiral Lift" end
  if z_hop == "Slope" then z_hop = "Slope Lift" end
  if z_hop == "Normal" then z_hop = "Normal Lift" end
  local pre_wipe = p.pre_wipe or "70%"
  local zh_mult = 0.75
  if z_hop == "Slope Lift" then zh_mult = 0.90 end
  if z_hop == "Normal Lift" then zh_mult = 1.00 end
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Nozzle %+d°C • Retract %+.2fmm • Deretract ×0.75 • %s Z-Hop (%.2f× LH) • %s Pre-Wipe", temp_drop, retract_boost, z_hop, zh_mult, pre_wipe)
  return {
    title = string.format("Symptom Fix: Stringing (%+d°C, %+.2fmm, %s Z-Hop)", temp_drop, retract_boost, z_hop),
    description = string.format("Relative anti-stringing adjustment for %s: nozzle %+d°C, retraction %+.2fmm, deretract ×0.75, %s Z-Hop (%.2f× layer_height), and %s retract-before-wipe.", scope, temp_drop, retract_boost, z_hop, zh_mult, pre_wipe),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local temp_drop = math.floor(tonumber(p and p.temp_drop) or -3)
  local retract_boost = tonumber(p and p.retract_boost) or 0.15
  local z_hop = (p and p.z_hop_style) or "Spiral Lift"
  if z_hop == "Spiral" then z_hop = "Spiral Lift" end
  if z_hop == "Slope" then z_hop = "Slope Lift" end
  if z_hop == "Normal" then z_hop = "Normal Lift" end
  local pre_wipe = (p and p.pre_wipe) or "70%"
  local zh_mult = 0.75
  if z_hop == "Slope Lift" then zh_mult = 0.90 end
  if z_hop == "Normal Lift" then zh_mult = 1.00 end

  local cur_t = tonumber(config["nozzle_temperature"]) or 255
  config["nozzle_temperature"] = tostring(cur_t + temp_drop)
  local cur_r = tonumber(config["filament_retraction_length"]) or 0.80
  config["filament_retraction_length"] = string.format("%.2f", cur_r + retract_boost)
  local cur_d = tonumber(config["filament_deretraction_speed"]) or 40
  config["filament_deretraction_speed"] = tostring(math.max(15, math.floor(cur_d * 0.75 + 0.5)))
  local cur_lh = tonumber(config["layer_height"]) or 0.20
  config["filament_z_hop"] = string.format("%.2f", cur_lh * zh_mult)
  config["filament_z_hop_types"] = z_hop
  config["filament_wipe"] = "1"
  local noz = tonumber(ctx and ctx.nozzle_diameter) or 0.40
  config["filament_wipe_distance"] = string.format("%.1f", noz * 5.0)
  config["filament_retract_before_wipe"] = pre_wipe
  local d = describe(p or {}, ctx)
  print("[Symptom:Stringing] " .. (d.badge or ""))
end
