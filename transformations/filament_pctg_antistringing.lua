-- Filament Rule: PCTG Stringing & Ooze Mitigation
-- PCTG has higher melt elasticity and chain entanglement than standard PETG, making it prone
-- to fine cobwebbing and nozzle tip build-up during travel moves.
-- Applies snappy 1.00mm @ 45mm/s retraction, 0% retract-before-wipe (so full retraction completes
-- before the 2.0mm internal wipe), 0.15mm Spiral Z-Hop, and 8000 mm/s2 travel acceleration.

params = {
  {
    id = "retract_len",
    label = "Retraction Length",
    description = "Direct-drive retraction length for PCTG (0.40 - 1.50 mm).",
    type = "range",
    min = 0.40,
    max = 1.50,
    step = 0.05,
    default = "1.00",
    unit = "mm",
    options = {
      { value = "0.85", label = "0.85 mm" },
      { value = "1.00", label = "1.00 mm (Recommended PCTG)" },
      { value = "1.15", label = "1.15 mm (Humid / Tropical)" },
    },
  },
  {
    id = "retract_speed",
    label = "Retraction Speed",
    description = "Retraction speed in mm/s (25 - 55 mm/s).",
    type = "range",
    min = 25,
    max = 55,
    step = 5,
    default = "45",
    unit = "mm/s",
    options = {
      { value = "35", label = "35 mm/s" },
      { value = "45", label = "45 mm/s (Recommended)" },
      { value = "50", label = "50 mm/s" },
    },
  },
  {
    id = "zhop_type",
    label = "Z-Hop Lift Type",
    description = "Spiral Lift breaks elastic PCTG strings cleanly during travel.",
    type = "enum",
    default = "Spiral Lift",
    options = {
      { value = "Spiral Lift", label = "Spiral Lift (Recommended)" },
      { value = "Slope Lift", label = "Slope Lift" },
      { value = "Normal Lift", label = "Normal Vertical Lift" },
    },
  },
}

function validate(p, ctx)
  local r_len = tonumber(p.retract_len) or 1.00
  local r_spd = tonumber(p.retract_speed) or 45
  if r_len > 1.25 and r_spd > 50 then
    return false, "Retraction > 1.25 mm at > 50 mm/s risks pulling molten PCTG into the cold zone."
  end
  return true
end

function apply(config, ctx, p)
  local r_len = string.format("%.2f", tonumber(p and p.retract_len) or 1.00)
  local r_spd = tostring(math.floor((tonumber(p and p.retract_speed) or 45) + 0.5))
  local z_type = (p and p.zhop_type) or "Spiral Lift"

  if ctx.filament_type == "PCTG" then
    if config["nozzle_temperature"] and config["nozzle_temperature"][1] then
      local t = tonumber(config["nozzle_temperature"][1]) or 260
      if t >= 258 then
        local trimmed = tostring(t - 3)
        config["nozzle_temperature"] = { trimmed }
        config["nozzle_temperature_HS"] = { trimmed }
        config["nozzle_temperature_BRASS"] = { trimmed }
        config["nozzle_temperature_initial_layer"] = { trimmed }
        config["nozzle_temperature_initial_layer_HS"] = { trimmed }
        config["nozzle_temperature_initial_layer_BRASS"] = { trimmed }
      end
    end
  end

  config["filament_retraction_length"] = { r_len }
  config["filament_retraction_speed"] = { r_spd }
  config["filament_deretraction_speed"] = { "0" }
  config["filament_retract_before_wipe"] = { "0%" }
  config["filament_wipe"] = { "1" }
  config["filament_wipe_distance"] = { "2.0" }
  config["z_hop"] = { "0.15" }
  config["z_hop_types"] = { z_type }
  config["filament_z_hop"] = { "0.15" }
  config["filament_z_hop_types"] = { z_type }
  config["travel_acceleration"] = "10000"
  config["wipe_on_loops"] = "1"

  print(string.format("[Filament:PCTG-AntiStringing] Retract -> %smm @ %smm/s (0%% retract-before-wipe, 2.0mm wipe), 0.15mm %s, 10000 mm/s2 travel accel", r_len, r_spd, z_type))
end
