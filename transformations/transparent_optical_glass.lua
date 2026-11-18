-- Transparent / Glass-Like Optical Clarity Mode (Rygar1432 / CNC Kitchen / Bambu Wiki Spec)
-- Eliminates internal air micro-voids and refractive boundaries for clear/translucent PETG, PCTG, and PC:
-- 1. Ultra-slow 20 mm/s extrusion speeds so molten tracks relax and fuse without shear bubbles.
-- 2. +5% Flow Ratio boost (1.01-1.02) and 0.50mm wide line width (1.25x nozzle) to flood inter-bead voids.
-- 3. 0% Part Cooling Fan (completely OFF) so adjacent lines thermally anneal without haze.
-- 4. 100% Aligned Rectilinear parallel infill (0 deg), 1 wall loop, and 0.12mm layer height.
-- 5. Lowers Textured PEI bed temp to 65C and removes first-layer nozzle temp boost for clean release.

params = {
  {
    id = "print_speed",
    label = "Optical Print Speed (mm/s)",
    description = "15–20 mm/s allows molten tracks to relax and degas without trapping micro-bubbles.",
    type = "range",
    min = 10,
    max = 45,
    step = 5,
    default = "20",
    unit = "mm/s",
    options = {
      { value = "15", label = "15 mm/s (Maximum Optical Glass Clarity)" },
      { value = "20", label = "20 mm/s (Recommended Rygar1432 Spec)" },
      { value = "30", label = "30 mm/s (Balanced Translucent)" },
    },
  },
  {
    id = "flow_boost",
    label = "Void-Fill Flow Boost (ratio)",
    description = "+0.05 (+5%) over-packs parallel lines to eliminate internal air-gap refraction.",
    type = "range",
    min = 0.00,
    max = 0.10,
    step = 0.01,
    default = "+0.05",
    unit = "ratio",
    options = {
      { value = "+0.03", label = "+0.03 (+3% Subtle)" },
      { value = "+0.05", label = "+0.05 (+5% Recommended Glass Fill)" },
      { value = "+0.07", label = "+0.07 (+7% Maximum Void Fill)" },
    },
  },
  {
    id = "fan_speed",
    label = "Part Cooling Fan Speed (%)",
    description = "0% (Fan OFF) allows polymer chains to thermally fuse without cooling haze.",
    type = "range",
    min = 0,
    max = 35,
    step = 5,
    default = "0",
    unit = "%",
    options = {
      { value = "0", label = "0% (Fan OFF — Maximum Clarity)" },
      { value = "15", label = "15% (Mild Overhang Support)" },
    },
  },
}

function validate(p, ctx)
  local spd = tonumber(p.print_speed) or 20
  local fan = tonumber(p.fan_speed) or 0
  if spd > 35 and fan > 20 then
    return false, "Combining print_speed > 35 mm/s with fan_speed > 20% produces frosty shear haze instead of optical clarity."
  end
  return true
end

function apply(config, ctx, p)
  local noz = scalar_num(ctx and ctx.nozzle_diameter, 0.40)
  local wide_w = fmt_num(noz * 1.25, "%.2f")
  local spd = fmt_num(tonumber(p and p.print_speed) or 20, "%.0f")
  local f_boost = tonumber(p and p.flow_boost) or 0.05
  local fan_spd = fmt_num(tonumber(p and p.fan_speed) or 0, "%.0f")

  local function set_arr(key, val)
    if type(config[key]) == "string" then
      config[key] = val
    else
      config[key] = { val }
    end
  end

  -- 1. Slow speeds (20 mm/s) & low volumetric ceiling (5.0 mm3/s) for bubble-free melt relaxation
  config["outer_wall_speed"] = spd
  config["inner_wall_speed"] = spd
  config["sparse_infill_speed"] = spd
  config["internal_solid_infill_speed"] = spd
  config["top_surface_speed"] = spd
  config["initial_layer_speed"] = "15"
  set_arr("filament_max_volumetric_speed", fmt_num(5.0, "%.1f"))

  -- 2. Boost flow ratio (+0.05 -> ~1.01) to flood microscopic air gaps between parallel extrusions
  local cur_flow = scalar_num(config["filament_flow_ratio"], 0.96)
  local new_flow = math.min(1.05, math.max(1.01, cur_flow + f_boost))
  local flow_str = fmt_num(new_flow, "%.2f")
  set_arr("filament_flow_ratio", flow_str)

  -- 3. Turn part cooling fan OFF (0%) so polymer chains anneal without crystalline haze
  set_arr("fan_max_speed", fan_spd)
  set_arr("fan_min_speed", fan_spd)
  if config["fan_max_speed_HS"] ~= nil then set_arr("fan_max_speed_HS", fan_spd) end
  if config["fan_max_speed_BRASS"] ~= nil then set_arr("fan_max_speed_BRASS", fan_spd) end
  if config["fan_min_speed_HS"] ~= nil then set_arr("fan_min_speed_HS", fan_spd) end
  if config["fan_min_speed_BRASS"] ~= nil then set_arr("fan_min_speed_BRASS", fan_spd) end
  set_arr("overhang_fan_speed", "25")

  -- 4. Unidirectional 100% Aligned Rectilinear infill + 1 wall + 0.12mm layer height + 1.25x line width
  config["layer_height"] = "0.12"
  config["line_width"] = wide_w
  config["outer_wall_line_width"] = wide_w
  config["inner_wall_line_width"] = wide_w
  config["sparse_infill_line_width"] = wide_w
  config["wall_loops"] = "1"
  config["top_shell_layers"] = "0"
  config["bottom_shell_layers"] = "0"
  config["top_shell_thickness"] = "0.00"
  config["bottom_shell_thickness"] = "0.00"
  config["sparse_infill_density"] = fmt_pct(100, "%.0f")
  config["sparse_infill_pattern"] = "alignedrectilinear"
  config["infill_direction"] = "0"

  -- 5. Keep Textured PEI at 65C for easy release of un-pigmented translucent PETG
  set_arr("textured_plate_temp", "65")
  set_arr("textured_plate_temp_initial_layer", "65")
  set_arr("hot_plate_temp", "65")
  set_arr("hot_plate_temp_initial_layer", "65")

  print("[Optical:TransparentGlass] " .. spd .. "mm/s speed, flow -> " .. flow_str .. ", fan -> " .. fan_spd .. "%, 100% alignedrectilinear, width -> " .. wide_w .. "mm, PEI bed -> 65C")
end
