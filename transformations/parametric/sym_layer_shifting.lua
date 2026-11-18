-- Symptom Fix: Layer Shifting / Staircase Step Loss Mid-Print
meta = {
  id = "sym_layer_shifting",
  name = "Layer Shifting / Staircase Step Loss Mid-Print",
  icon = "🪜",
  category = "4. Walls, Seams & Dimensional (Motion & Step Loss)",
  tree_category = "4. Walls, Seams & Dimensional",
  tree_subcategory = "Stepper Torque & Collision Clearance",
  slicer_menu_path = "Relative Travel Acceleration & Jerk Scaling + Non-Crossing Infill + Relative Z-Hop Boost",
  when_to_run = "Apply when the entire upper portion of a print suddenly shifts sideways in X or Y like a staircase step.",
  formula_summary = "Travel_Accel_new = Travel_Accel_old × (1 - Accel_Drop%)  •  Z_Hop_new = Z_Hop_old + Clearance_Boost  •  Infill = gyroid",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Layer shifts on CoreXY printers happen for two reasons: (a) the nozzle collides with a curled overhang or crossing Grid infill intersection during a fast travel move, OR (b) Travel Acceleration / Jerk exceeds stepper motor torque.",
    "2. Check belt tension and pulley grub screws first! Then scale Travel Acceleration & Jerk down by -30% (0.70×), add +0.10mm extra Z-Hop clearance, and switch away from Grid infill.",
  },
  hardware_checklist = {
    "🔩 Check Motor Pulley Grub Screws (Set Screws): Inspect the A/B CoreXY stepper pulleys to ensure the small hex grub screws are tight against the flat of the motor D-shaft.",
    "🎸 Check Belt Teeth & Idler Bearings: Ensure no stray filament poop or debris from the purge chute has fallen into the Y-axis rails or belt teeth.",
  },
}

params = {
  {
    id = "travel_accel",
    label = "1. By what percentage should we reduce Travel Acceleration & Jerk (travel_acceleration, travel_jerk)?",
    description = "Scaling travel acceleration & jerk by -30% (0.70× of current) prevents X/Y stepper motors from losing steps during high-speed travel reversals.",
    type = "range",
    min = -65,
    max = 0,
    step = 5,
    default = "-30%",
    unit = "%",
    options = {
      { value = "-20%", label = "-20% (0.80× Moderate Step Margin)" },
      { value = "-30%", label = "-30% (0.70× Recommended CoreXY Safe)" },
      { value = "-45%", label = "-45% (0.55× Maximum Step Torque Safety)" },
    },
  },
  {
    id = "zhop_height",
    label = "2. How much extra Z-Hop Clearance Height (filament_z_hop, mm) should we add so the nozzle clears curled edges?",
    description = "Adding +0.10 mm to +0.20 mm relative to your current Z-Hop lifts the nozzle safely above any curled overhang during travel.",
    type = "range",
    min = 0.00,
    max = 0.60,
    step = 0.05,
    default = "+0.10",
    unit = "mm",
    options = {
      { value = "+0.05", label = "+0.05 mm Extra Z-Hop Clearance" },
      { value = "+0.10", label = "+0.10 mm Extra Z-Hop Clearance (Recommended)" },
      { value = "+0.20", label = "+0.20 mm Extra Z-Hop Clearance (Tall / Curled Parts)" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local t_pct = tonumber(p.travel_accel) or -30.0
  local t_scale = 0.70
  if t_pct < 0 then
    t_scale = math.max(0.35, math.min(1.0, (100.0 + t_pct) / 100.0))
  else
    t_scale = math.max(0.35, math.min(1.0, (100.0 - t_pct) / 100.0))
  end
  local t_drop_label = math.floor((1.0 - t_scale) * 100.0 + 0.5)
  local zh_boost = tonumber(p.zhop_height) or 0.10
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Travel Accel/Jerk -%.0f%% (%.2f×) • Z-Hop %+.2fmm • Non-Crossing Gyroid", t_drop_label, t_scale, zh_boost)
  return {
    title = string.format("Symptom Fix: Anti-Layer-Shift (-%.0f%% Travel Accel, %+.2fmm Z-Hop)", t_drop_label, zh_boost),
    description = string.format("Prevents stepper step loss and nozzle overhang collisions for %s: scales travel_acceleration & travel_jerk by %.2f× (-%.0f%%), adds %+.2fmm Z-Hop, and uses gyroid infill.", scope, t_scale, t_drop_label, zh_boost),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local t_pct = tonumber(p and p.travel_accel) or -30.0
  local t_scale = 0.70
  if t_pct < 0 then
    t_scale = math.max(0.35, math.min(1.0, (100.0 + t_pct) / 100.0))
  else
    t_scale = math.max(0.35, math.min(1.0, (100.0 - t_pct) / 100.0))
  end
  local zh_boost = tonumber(p and p.zhop_height) or 0.10

  local function set_arr(key, val)
    if type(config[key]) == "string" then
      config[key] = val
    else
      config[key] = { val }
    end
  end

  local ft_raw = (ctx and ctx.filament_type) or (type(config["filament_type"]) == "table" and config["filament_type"][1] or config["filament_type"]) or ""
  local ft_up = string.upper(tostring(ft_raw))
  local is_elastomer = (ctx and ctx.filament_is_elastomer == true)
    or string.find(ft_up, "TPU", 1, true) ~= nil
    or string.find(ft_up, "PEBA", 1, true) ~= nil
    or string.find(ft_up, "FLEX", 1, true) ~= nil

  local cur_ta = scalar_num(config["travel_acceleration"], 10000)
  config["travel_acceleration"] = fmt_num(math.max(2500, cur_ta * t_scale), "%.0f")
  local cur_tj = scalar_num(config["travel_jerk"], 12)
  config["travel_jerk"] = fmt_num(math.max(5, cur_tj * t_scale), "%.0f")
  if not is_elastomer then
    local cur_zh = scalar_num(config["filament_z_hop"], 0.15)
    set_arr("filament_z_hop", fmt_num(cur_zh + zh_boost, "%.2f"))
    local cur_zht = config["filament_z_hop_types"]
    local zht_str = (type(cur_zht) == "table" and cur_zht[1]) or (type(cur_zht) == "string" and cur_zht) or "Spiral Lift"
    set_arr("filament_z_hop_types", zht_str)
  end
  config["sparse_infill_pattern"] = "gyroid"
  config["reduce_infill_retraction"] = "0"
  local d = describe(p or {}, ctx)
  print("[Symptom:LayerShift] " .. (d.badge or ""))
end
