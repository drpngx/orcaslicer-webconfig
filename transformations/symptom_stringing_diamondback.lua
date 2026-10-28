-- Symptom 1: Stringing & Oozing with MicroSwiss + Diamondback
-- High thermal conductivity of PCD Diamond keeps the melt zone slightly less viscous at the tip.
-- Increases retraction distance to 0.95mm @ 50mm/s, switches to 0.20mm Spiral Z-Hop,
-- boosts travel acceleration to 7000 mm/s2, and drops nozzle temperature by -4°C.

function apply(config, ctx)
  if config["nozzle_temperature"] and config["nozzle_temperature"][1] then
    local t = tonumber(config["nozzle_temperature"][1]) or 235
    config["nozzle_temperature"] = { tostring(t - 4) }
  end

  config["filament_retraction_length"] = { "0.95" }
  config["filament_retraction_speed"] = { "50" }
  config["z_hop"] = { "0.20" }
  config["z_hop_types"] = { "Spiral" }
  config["travel_acceleration"] = "7000"
  config["wipe_on_loops"] = "1"

  print("[Symptom:Stringing] Retract -> 0.95mm @ 50mm/s, Spiral Z-Hop 0.20mm, Travel Accel -> 7000 mm/s2, Temp -4C")
end
