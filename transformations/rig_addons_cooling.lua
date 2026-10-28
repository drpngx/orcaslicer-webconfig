-- Dynamic Combination Rule 4: Physical Rig Add-Ons & Cooling Schedule Modifiers
-- Adjusts part cooling fan schedules, overhang fan thresholds, exhaust fan speeds,
-- and retraction based on physical mods installed on the rig:
--   - "hollow_front_back_covers": Hollow Front/Back Covers -> increases ambient airflow,
--     lowers min fan speed -15%, extends layer cooling time, and lowers exhaust fan to 40%.
--   - "microswiss_duct": MicroSwiss High-Velocity Dual-Fang Duct -> concentrates static pressure,
--     scales max fan speed by 0.85x to prevent over-cooling warping while keeping 100% overhang bridge cooling.
--   - "flowtech_hotend": MicroSwiss FlowTech Hotend -> sets melt_zone_multiplier = 1.15x,
--     tightens retraction length to 0.65mm @ 50mm/s.

local function has_addon(addons, target)
  if not addons then
    return false
  end
  local n = #addons
  for i = 1, n do
    if addons[i] == target then
      return true
    end
  end
  return false
end

function apply(config, ctx)
  local addons = ctx.addons
  if not addons then
    return
  end

  if has_addon(addons, "hollow_front_back_covers") then
    local fmin = 30
    if config["fan_min_speed"] and config["fan_min_speed"][1] then
      fmin = tonumber(config["fan_min_speed"][1]) or 30
    end
    local new_min = math.max(10, fmin - 15)
    config["fan_min_speed"] = { tostring(new_min) }
    config["during_print_exhaust_fan_speed"] = { "40" }
    config["complete_print_exhaust_fan_speed"] = { "50" }
    print("[AddOn:HollowCovers] Lowered fan_min_speed to " .. tostring(new_min) .. "%, exhaust fan to 40% (open-frame cooling schedule)")
  end

  if has_addon(addons, "microswiss_duct") then
    local fmax = 90
    if config["fan_max_speed"] and config["fan_max_speed"][1] then
      fmax = tonumber(config["fan_max_speed"][1]) or 90
    end
    local new_max = math.floor(fmax * 0.85)
    config["fan_max_speed"] = { tostring(new_max) }
    config["overhang_fan_speed"] = { "100" }
    config["enable_overhang_bridge_fan"] = { "1" }
    print("[AddOn:MicroSwissDuct] Scaled fan_max_speed to " .. tostring(new_max) .. "% (high static pressure duct), overhang fan -> 100%")
  end

  if has_addon(addons, "flowtech_hotend") then
    config["filament_retraction_length"] = { "0.65" }
    config["filament_retraction_speed"] = { "50" }
    print("[AddOn:FlowTechHotend] Set retraction to 0.65mm @ 50mm/s for leak-proof all-metal thermal break")
  end
end
