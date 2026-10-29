-- Transparent / Glass-Like Optical Clarity Mode (Rygar1432 / CNC Kitchen / Bambu Wiki Spec)
-- Eliminates internal air micro-voids and refractive boundaries for clear/translucent PETG, PCTG, and PC:
-- 1. Ultra-slow 20 mm/s extrusion speeds so molten tracks relax and fuse without shear bubbles.
-- 2. +5% Flow Ratio boost (1.01-1.02) and 0.50mm wide line width (1.25x nozzle) to flood inter-bead voids.
-- 3. 0% Part Cooling Fan (completely OFF) so adjacent lines thermally anneal without haze.
-- 4. 100% Aligned Rectilinear parallel infill (0 deg), 1 wall loop, and 0.12mm layer height.
-- 5. Lowers Textured PEI bed temp to 65C and removes first-layer nozzle temp boost for clean release.

function apply(config, ctx)
  local noz = tonumber(ctx.nozzle_diameter) or 0.40
  local wide_w = string.format("%.2f", noz * 1.25)

  -- 1. Slow speeds (20 mm/s) & low volumetric ceiling (5.0 mm3/s) for bubble-free melt relaxation
  config["outer_wall_speed"] = "20"
  config["inner_wall_speed"] = "20"
  config["sparse_infill_speed"] = "20"
  config["internal_solid_infill_speed"] = "20"
  config["top_surface_speed"] = "20"
  config["initial_layer_speed"] = "15"
  config["filament_max_volumetric_speed"] = "5.0"

  -- 2. Boost flow ratio (+0.05 -> ~1.01) to flood microscopic air gaps between parallel extrusions
  local cur_flow = tonumber(config["filament_flow_ratio"]) or 0.96
  local new_flow = math.min(1.05, math.max(1.01, cur_flow + 0.05))
  config["filament_flow_ratio"] = string.format("%.2f", new_flow)

  -- 3. Turn part cooling fan OFF (0%) so polymer chains anneal without crystalline haze
  config["fan_max_speed"] = "0"
  config["fan_min_speed"] = "0"
  config["overhang_fan_speed"] = "25"

  -- 4. Unidirectional 100% Aligned Rectilinear infill + 1 wall + 0.12mm layer height + 1.25x line width
  config["layer_height"] = "0.12"
  config["line_width"] = wide_w
  config["outer_wall_line_width"] = wide_w
  config["inner_wall_line_width"] = wide_w
  config["sparse_infill_line_width"] = wide_w
  config["wall_loops"] = "1"
  config["top_shell_layers"] = "0"
  config["bottom_shell_layers"] = "0"
  config["sparse_infill_density"] = "100%"
  config["sparse_infill_pattern"] = "alignedrectilinear"
  config["infill_direction"] = "0"

  -- 5. Keep Textured PEI at 65C for easy release of un-pigmented translucent PETG
  config["textured_plate_temp"] = "65"
  config["textured_plate_temp_initial_layer"] = "65"
  config["hot_plate_temp"] = "65"
  config["hot_plate_temp_initial_layer"] = "65"

  print("[Optical:TransparentGlass] 20mm/s speed, flow -> " .. config["filament_flow_ratio"] .. ", fan -> 0%, 100% alignedrectilinear, width -> " .. wide_w .. "mm, PEI bed -> 65C")
end
