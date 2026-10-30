-- Calibration Print #4: Retraction & Anti-Stringing Tower Calibration
meta = {
  id = "cal_retraction_tower",
  name = "Retraction & Anti-Stringing Tower Calibration",
  icon = "🕸️",
  category = "Calibration Print #4 (Retraction & Wipe)",
  tree_category = "Post-Print Calibration",
  tree_subcategory = "Calibration Test",
  slicer_menu_path = "Calibration → Retraction Test",
  when_to_run = "Run 4th (only if fine cobwebs/stringing remain after drying filament and calibrating Temp + PA).",
  formula_summary = "Retraction_mm = Start + (Z_height × Step) • Pairs with Spiral Z-Hop & 70% Retract-Before-Wipe",
  default_condition_expr = "filament:PCTG",
  how_to_read_print = {
    "1. Print the Retraction Tower (default: Start = 0.2 mm, Step = 0.1 mm per 1 mm of Z height).",
    "2. Measure the height Z (in mm) from the bottom where stringing between the two posts disappears: Retraction_mm = Start + (Z × Step).",
    "3. Choose the shortest retraction length that stops stringing (keep ≤ 1.4 mm on Kobra S1 all-metal direct drive to avoid pulling molten filament into the cold heat-break).",
  },
  hardware_checklist = {
    "🖨️ Slice and print the built-in test model from the AnycubicSlicerNext / OrcaSlicer top 'Calibration' menu before entering your measurements below.",
  },
}

params = {
  {
    id = "retract_len",
    label = "1. What was the lowest Retraction Length (mm) on the tower that stopped stringing?",
    description = "For Kobra S1 Direct Drive: PLA = 0.6–0.8 mm, PETG/PCTG = 0.85–1.10 mm.",
    type = "range",
    min = 0.10,
    max = 2.00,
    step = 0.05,
    default = "0.95",
    unit = "mm",
    options = {
      { value = "0.60", label = "0.60 mm (DiamondBack / PLA)" },
      { value = "0.80", label = "0.80 mm (Stock Kobra S1)" },
      { value = "0.95", label = "0.95 mm (PCTG / PETG Balanced)" },
      { value = "1.10", label = "1.10 mm (PCTG Anti-Stringing)" },
      { value = "1.25", label = "1.25 mm (Max Direct Drive)" },
    },
  },
  {
    id = "retract_speed",
    label = "2. What Retraction Speed (mm/s) should be used?",
    description = "40–45 mm/s snaps the molten strand cleanly; speeds >55 mm/s risk grinding filament.",
    type = "range",
    min = 15,
    max = 70,
    step = 5,
    default = "45",
    unit = "mm/s",
    options = {
      { value = "35", label = "35 mm/s (Gentle)" },
      { value = "40", label = "40 mm/s (Standard)" },
      { value = "45", label = "45 mm/s (Recommended for PCTG/PETG)" },
      { value = "50", label = "50 mm/s (Fast)" },
    },
  },
  {
    id = "z_hop_style",
    label = "3. Which Z-Hop Lift Style should be used during travel moves?",
    description = "Spiral lift corkscrews sideways as it rises, shearing sticky PCTG/PETG hairs instead of pulling them straight up.",
    type = "enum",
    default = "Spiral Lift",
    unit = "",
    options = {
      { value = "Spiral Lift", label = "Spiral Lift (0.15mm — Best for PCTG/PETG)" },
      { value = "Slope Lift", label = "Slope Lift (0.15mm)" },
      { value = "Normal Lift", label = "Normal Vertical Lift (0.20mm)" },
    },
  },
  {
    id = "retract_before_wipe",
    label = "4. What percentage of retraction should occur BEFORE the 2mm wipe?",
    description = "70% relieves nozzle pressure before the wipe motion starts so the tip wipes clean.",
    type = "enum",
    default = "70%",
    unit = "",
    options = {
      { value = "70%", label = "70% (Recommended for PCTG/PETG)" },
      { value = "50%", label = "50% (Moderate)" },
      { value = "0%", label = "0% (Standard PLA)" },
    },
  },
}

function validate(p, ctx)
  local len_num = tonumber(p.retract_len) or 0.95
  local spd_num = tonumber(p.retract_speed) or 45
  if len_num > 1.5 and spd_num > 55 then
    return false, "Retraction length > 1.5mm combined with speed > 55mm/s risks heat-creep clogging and filament grinding on all-metal direct drive."
  end
  return true
end

function describe(p, ctx)
  local len_num = tonumber(p.retract_len) or 0.95
  local spd_num = math.floor(tonumber(p.retract_speed) or 45)
  local deretract_spd = math.max(25, spd_num - 15)
  local z_hop = p.z_hop_style or "Spiral Lift"
  if z_hop == "Spiral" then z_hop = "Spiral Lift" end
  if z_hop == "Slope" then z_hop = "Slope Lift" end
  if z_hop == "Normal" then z_hop = "Normal Lift" end
  local rbw = p.retract_before_wipe or "70%"
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("%.2fmm @ %dmm/s (Deretract %dmm/s) • %s Z-Hop • %s Pre-Wipe", len_num, spd_num, deretract_spd, z_hop, rbw)
  return {
    title = string.format("Calibration: Retraction %.2fmm @ %dmm/s (%s)", len_num, spd_num, z_hop),
    description = string.format("Post-print Retraction Tower calibration for %s: %.2fmm @ %dmm/s, %dmm/s deretract, %s Z-Hop, and %s retract-before-wipe.", scope, len_num, spd_num, deretract_spd, z_hop, rbw),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local len_num = tonumber(p and p.retract_len) or 0.95
  local spd_num = math.floor(tonumber(p and p.retract_speed) or 45)
  local deretract_spd = math.max(25, spd_num - 15)
  local z_hop = (p and p.z_hop_style) or "Spiral Lift"
  if z_hop == "Spiral" then z_hop = "Spiral Lift" end
  if z_hop == "Slope" then z_hop = "Slope Lift" end
  if z_hop == "Normal" then z_hop = "Normal Lift" end
  local rbw = (p and p.retract_before_wipe) or "70%"

  config["filament_retraction_length"] = string.format("%.2f", len_num)
  config["filament_retraction_speed"] = tostring(spd_num)
  config["filament_deretraction_speed"] = tostring(deretract_spd)
  config["filament_z_hop"] = "0.15"
  config["filament_z_hop_types"] = z_hop
  config["filament_wipe"] = "1"
  config["filament_wipe_distance"] = "2.0"
  config["filament_retract_before_wipe"] = rbw
  local d = describe(p or {}, ctx)
  print("[Calibration:Retraction] " .. (d.badge or ""))
end
