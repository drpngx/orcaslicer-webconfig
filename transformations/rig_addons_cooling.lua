-- Dynamic Combination Rule 4: Physical Rig Add-Ons & Cooling Schedule Modifiers
-- Adjusts part cooling fan schedules, overhang fan thresholds, exhaust fan speeds,
-- and retraction based on physical mods installed on the rig:
--   - "hollow_front_back_covers": Hollow Front/Back Covers -> increases ambient airflow,
--     lowers min fan speed -15%, extends layer cooling time, and lowers exhaust fan to 40%.
--   - "microswiss_duct": MicroSwiss High-Velocity Dual-Fang Duct -> concentrates static pressure,
--     scales max fan speed by 0.85x to prevent over-cooling warping while keeping 100% overhang bridge cooling.
--   - "flowtech_hotend": MicroSwiss FlowTech Hotend -> sets melt_zone_multiplier = 1.15x,
--     compresses retraction through a viscoelastic PWL curve (rigid ~0.88-0.95x, TPU travel
--     preserved) scaled by nozzle orifice, and re-shapes retraction speed through a PWL
--     (20->20, 25->26, 35->40, 40->44, 50->46 mm/s).

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
    -- FlowTech unitary all-metal thermal cylinder: retraction must be compressed to keep molten
    -- filament out of the sharp, short cold zone, but the compression is viscoelastic — a stiff
    -- PETG/PLA column (r0 0.6-0.9mm) compresses ~10-13%, while an elastomer (TPU, r0 >= 1.1mm)
    -- wastes its first ~0.4mm relieving axial spring compression and must KEEP its travel.
    -- Orifice scaling S_d = 0.88 + 0.12*sqrt(d/0.4) compensates the molten meniscus volume that
    -- grows with nozzle area. All relative (PWL) over the spool base — never an absolute write.
    local d = scalar_num(ctx.nozzle_diameter, 0.4)
    local d_scale = 0.88 + 0.12 * math.sqrt(d / 0.4)

    local flowtech_retract_pwl = {
      { 0.40, 0.36 }, -- ultra-short rigid PLA
      { 0.80, 0.70 }, -- PETG / ABS standard band
      { 1.00, 0.86 }, -- upper rigid ceiling
      { 1.50, 1.35 }  -- elastomeric TPU decompression travel preserved
    }
    local r0_base = scalar_num(base["filament_retraction_length"], scalar_num(config["filament_retraction_length"], 0.80))
    if config["filament_retraction_length"] then
      local r1_target = pwl(r0_base, flowtech_retract_pwl) * d_scale
      local r_scale = r1_target / math.max(r0_base, 0.01)
      config["filament_retraction_length"] = map_num(config["filament_retraction_length"], function(r_cur)
        return clamp(r_cur * r_scale, 0.35, 1.40)
      end, "%.2f")
    end

    -- Speed PWL: +4..+5 mm/s responsiveness boost through the crisp 30-44 band; excessive
    -- speeds (>= 50) clamped down to 46. Elastomer families skip the boost entirely —
    -- flexible filament must never be yanked faster than its calibrated speed (gear slip /
    -- cavitation). The flag comes from Rust (`ctx.filament_is_elastomer`, derived from
    -- constants.json `hardware_limits.elastomer_family_tokens`) so the token list has a
    -- single source; nil (standalone rule testing without a rig ctx) means rigid.
    local is_elastomer = ctx.filament_is_elastomer == true
    local is_elastomer_retract = is_elastomer or r0_base >= 1.15
    local max_retract = is_elastomer_retract and 1.45 or (1.15 * math.sqrt(d / 0.4))
    constrain("filament_retraction_length", 0.35, max_retract, "FlowTech all-metal heat-creep ceiling")
    constrain("filament_retraction_speed", is_elastomer and 15 or 20, 46, "Direct-drive + all-metal meniscus speed limit")
    local flowtech_speed_pwl = {
      { 20, 20 },
      { 25, 26 },
      { 35, 40 },
      { 40, 44 },
      { 50, 46 }
    }
    if config["filament_retraction_speed"] and not is_elastomer then
      local s0_base = scalar_num(base["filament_retraction_speed"], scalar_num(config["filament_retraction_speed"], 40))
      local ds = pwl(s0_base, flowtech_speed_pwl) - s0_base
      config["filament_retraction_speed"] = map_num(config["filament_retraction_speed"], function(s_cur)
        return clamp(s_cur + ds, 20, 46)
      end, "%.0f")
    end

    print("[AddOn:FlowTechHotend] All-metal viscoelastic retraction curve (0.80 -> 0.70mm on a 0.4 nozzle, x" .. string.format("%.3f", d_scale) .. " orifice scale), speed PWL 20..50 -> 20..46 mm/s")
  end
end
