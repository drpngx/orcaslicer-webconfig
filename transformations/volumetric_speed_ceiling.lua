-- Dynamic Combination Rule 2: Volumetric Speed Ceiling Rule
-- Caps maximum print speed dynamically so a wider nozzle or faster process
-- never outruns the hotend or filament volumetric melt limit:
--
--   Safe Speed Ceiling (mm/s) = min(Hotend_max_flow, Filament_max_flow) / (Layer_Height * Line_Width)

function apply(config, ctx)
  local hotend_flow = tonumber(ctx.hotend_max_flow) or 22.0
  local filament_flow = tonumber(ctx.filament_max_flow) or 18.0
  if config["filament_max_volumetric_speed"] and config["filament_max_volumetric_speed"][1] then
    filament_flow = tonumber(config["filament_max_volumetric_speed"][1]) or filament_flow
  end

  local layer_h = tonumber(config["layer_height"]) or tonumber(ctx.layer_height) or 0.20
  local line_w = tonumber(config["outer_wall_line_width"]) or tonumber(ctx.line_width) or 0.42

  local effective_flow = math.min(hotend_flow, filament_flow)
  local cross_section = layer_h * line_w
  if cross_section <= 0 then
    cross_section = 0.084
  end

  local safe_ceiling = math.floor(effective_flow / cross_section)
  config["computed_safe_speed_ceiling"] = tostring(safe_ceiling)

  local outer_spd = tonumber(config["outer_wall_speed"]) or 200
  local inner_spd = tonumber(config["inner_wall_speed"]) or 300
  local infill_spd = tonumber(config["sparse_infill_speed"]) or 350

  if outer_spd > safe_ceiling then
    config["outer_wall_speed"] = tostring(safe_ceiling)
  end
  if inner_spd > safe_ceiling then
    config["inner_wall_speed"] = tostring(safe_ceiling)
  end
  if infill_spd > safe_ceiling then
    config["sparse_infill_speed"] = tostring(safe_ceiling)
  end

  print("[VolumetricCeiling] min(" .. string.format("%.1f", hotend_flow) .. ", " .. string.format("%.1f", filament_flow) .. ") / (" .. string.format("%.2f", layer_h) .. " * " .. string.format("%.2f", line_w) .. ") = " .. tostring(safe_ceiling) .. " mm/s safe speed ceiling")
end
