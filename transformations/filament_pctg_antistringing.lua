-- Filament Rule: PCTG Stringing & Ooze Mitigation
-- PCTG has higher melt elasticity and chain entanglement than standard PETG, making it prone
-- to fine cobwebbing and nozzle tip build-up during travel moves.
-- Applies snappy 0.85mm @ 45mm/s retraction, 35mm/s deretraction, 70% retract-before-wipe
-- over a 2.0mm wipe distance, 0.20mm Spiral Z-Hop, and 8000 mm/s2 travel acceleration.

params = {
  {
    id = "retract_len",
    label = "Retraction Length",
    description = "Direct-drive retraction length for PCTG (0.40 - 1.50 mm).",
    type = "range",
    min = 0.40,
    max = 1.50,
    step = 0.05,
    default = "0.85",
    unit = "mm",
    options = {
      { value = "0.70", label = "0.70 mm" },
      { value = "0.85", label = "0.85 mm (Recommended PCTG)" },
      { value = "1.00", label = "1.00 mm" },
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
  local r_len = tonumber(p.retract_len) or 0.85
  local r_spd = tonumber(p.retract_speed) or 45
  if r_len > 1.20 and r_spd > 50 then
    return false, "Retraction > 1.20 mm at > 50 mm/s risks pulling molten PCTG into the cold zone."
  end
  return true
end

function apply(config, ctx, p)
  local r_len = string.format("%.2f", tonumber(p and p.retract_len) or 0.85)
  local r_spd = tostring(math.floor((tonumber(p and p.retract_speed) or 45) + 0.5))
  local z_type = (p and p.zhop_type) or "Spiral Lift"

  if ctx.filament_type == "PCTG" then
    if config["nozzle_temperature"] and config["nozzle_temperature"][1] then
      local t = tonumber(config["nozzle_temperature"][1]) or 260
      if t >= 258 then
        config["nozzle_temperature"] = { tostring(t - 3) }
      end
    end
  end

  config["filament_retraction_length"] = { r_len }
  config["filament_retraction_speed"] = { r_spd }
  config["filament_deretraction_speed"] = { "35" }
  config["filament_retract_before_wipe"] = { "70%" }
  config["filament_wipe_distance"] = { "2.0" }
  config["z_hop"] = { "0.20" }
  config["z_hop_types"] = { z_type }
  config["travel_acceleration"] = "8000"
  config["wipe_on_loops"] = "1"

  print(string.format("[Filament:PCTG-AntiStringing] Retract -> %smm @ %smm/s (35mm/s deretract), 70%% retract-before-wipe (2.0mm), 0.20mm %s, 8000 mm/s2 travel accel", r_len, r_spd, z_type))
end
