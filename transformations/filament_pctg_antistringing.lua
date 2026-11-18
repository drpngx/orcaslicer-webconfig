-- Filament Rule: PCTG Stringing & Ooze Mitigation
-- PCTG has higher melt elasticity and chain entanglement than standard PETG, making it prone
-- to fine cobwebbing and nozzle tip build-up during travel moves.
-- Applies snappy 1.00mm @ 45mm/s retraction, 40% retract-before-wipe (relieving 40% of nozzle
-- pressure before the 2.0mm internal wipe and completing 60% during the wipe), 0.10mm Spiral Z-Hop,
-- and 10000 mm/s2 travel acceleration.

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
  local r_len_num = tonumber(p and p.retract_len) or 1.00
  local r_spd_num = tonumber(p and p.retract_speed) or 45
  local r_spd = fmt_num(r_spd_num, "%.0f")
  local z_type = (p and p.zhop_type) or "Spiral Lift"
  local ptype = (config and config["type"]) or ""

  if ptype ~= "process" then
    if ctx.filament_type == "PCTG" then
      if config["nozzle_temperature"] ~= nil then
        local t = scalar_num(config["nozzle_temperature"], 260)
        local t0 = scalar_num(base["nozzle_temperature"], t)
        if t0 >= 258 or t >= 258 then
          local trimmed = fmt_num(t - 3, "%.0f")
          config["nozzle_temperature"] = { trimmed }
          config["nozzle_temperature_HS"] = { trimmed }
          config["nozzle_temperature_BRASS"] = { trimmed }
          config["nozzle_temperature_initial_layer"] = { trimmed }
          config["nozzle_temperature_initial_layer_HS"] = { trimmed }
          config["nozzle_temperature_initial_layer_BRASS"] = { trimmed }
        end
      end
    end

    local cur_ret = scalar_num(config["filament_retraction_length"], r_len_num)
    local base_ret = scalar_num(base["filament_retraction_length"], cur_ret)
    local target_ret = r_len_num
    if cur_ret >= 2.0 then
      -- Bowden extruder (>= 2.0mm): preserve Bowden tube pullback and add only relative PCTG anti-stringing delta
      local bowden_delta = math.max(0.0, r_len_num - 0.80)
      target_ret = math.min(cur_ret + 0.80, math.max(cur_ret, base_ret + bowden_delta))
    elseif cur_ret < base_ret and base_ret > 0.40 then
      -- Earlier hardware rule (e.g. FlowTech 0.85x or CM2 0.60mm) shortened retraction for an all-metal throat:
      -- scale proportionally instead of clobbering the throat heat-creep protection
      target_ret = math.min(1.10, cur_ret * (r_len_num / base_ret))
    else
      target_ret = r_len_num
    end
    local r_len = fmt_num(target_ret, "%.2f")
    config["filament_retraction_length"] = { r_len }
    config["filament_retraction_speed"] = { r_spd }
    config["filament_deretraction_speed"] = { "0" }
    config["filament_retract_before_wipe"] = { "40%" }
    config["filament_wipe"] = { "1" }
    config["filament_wipe_distance"] = { "2.0" }
    local cur_zhop = scalar_num(config["filament_z_hop"], 0.0)
    if cur_zhop < 0.10 then
      config["filament_z_hop"] = { "0.1" }
    end
    config["filament_z_hop_types"] = { z_type }
  end

  if ptype ~= "filament" then
    local kin = string.lower((ctx and ctx.kinematics) or "")
    if kin == "corexy" or kin == "" then
      local cur_ta = scalar_num(config["travel_acceleration"], 10000)
      if cur_ta < 10000 then
        config["travel_acceleration"] = "10000"
      end
    end
    config["wipe_on_loops"] = "1"
  end
  sync_coupled_physics(config, ctx)

  local logged_ret = scalar_num(config["filament_retraction_length"], r_len_num)
  print(string.format("[Filament:PCTG-AntiStringing] Retract -> %.2fmm @ %smm/s (40%% retract-before-wipe, 2.0mm wipe), 0.10mm %s", logged_ret, r_spd, z_type))
end
