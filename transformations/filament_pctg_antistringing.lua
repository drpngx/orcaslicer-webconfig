-- Filament Rule: PCTG Stringing & Ooze Mitigation
-- PCTG has higher melt elasticity and chain entanglement than standard PETG, making it prone
-- to fine cobwebbing and nozzle tip build-up during travel moves.
-- Applies snappy 0.85mm @ 45mm/s retraction, 35mm/s deretraction, 70% retract-before-wipe
-- over a 2.0mm wipe distance, 0.20mm Spiral Z-Hop, and 8000 mm/s2 travel acceleration.

function apply(config, ctx)
  if ctx.filament_type == "PCTG" then
    if config["nozzle_temperature"] and config["nozzle_temperature"][1] then
      local t = tonumber(config["nozzle_temperature"][1]) or 260
      if t >= 258 then
        config["nozzle_temperature"] = { tostring(t - 3) }
      end
    end
  end

  config["filament_retraction_length"] = { "0.85" }
  config["filament_retraction_speed"] = { "45" }
  config["filament_deretraction_speed"] = { "35" }
  config["filament_retract_before_wipe"] = { "70%" }
  config["filament_wipe_distance"] = { "2.0" }
  config["z_hop"] = { "0.20" }
  config["z_hop_types"] = { "Spiral" }
  config["travel_acceleration"] = "8000"
  config["wipe_on_loops"] = "1"

  print("[Filament:PCTG-AntiStringing] Retract -> 0.85mm @ 45mm/s (35mm/s deretract), 70% retract-before-wipe (2.0mm), 0.20mm Spiral Z-Hop, 8000 mm/s2 travel accel")
end
