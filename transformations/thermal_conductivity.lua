-- Dynamic Combination Rule 1: Thermal Conductivity & Nozzle Material Compensation
-- Adjusts nozzle temperature, flow ratio, and retraction based on the installed nozzle material:
--   - "MicroSwiss Diamondback (PCD)" / "Diamondback PCD": 543 W/(m*K) PCD tip (49.8 GPa hardness, 0.01 CoF) ->
--     piecewise-linear temperature curve dT(T0) by brass base temp (-11C on PLA fans-on profiles,
--     -10C at 235-240C PETG, tapering to -5C at 300C to preserve inter-layer welds on ABS/ASA/PA-CF/PC),
--     plus a quadratic die-swell flow trim (0 to -1.2%) that never touches pre-thinned matte/CF
--     calibrations. Retraction is NOT written here: it is owned by the FlowTech hotend rule (P12)
--     and the stringing mitigation (P25) as relative curves over the spool base.
--   - "Brass (MicroSwiss TwinClad XT)": Low-friction TwinClad XT nickel-composite coating -> 0°C temp offset,
--     reduces retraction length to 0.70mm @ 45mm/s and boosts flow ratio to 0.99.
--   - "Ruby Tip (Brass Body)" (`ruby_brass`): Brass body (~115 W/(m*K)) with synthetic Ruby gemstone tip (9.0 Mohs) ->
--     0°C Brass thermal baseline (no +5°C lag offset) while marking `nozzle_type = "hardened_steel"` so Slicer allows -CF and -GF filaments.
--   - "MicroSwiss CM2 Hardened": Copper core + M2 HSS tip -> 0°C offset, 0.60mm retraction @ 45mm/s.
--   - "Hardened Steel" / "Stainless Steel": Low thermal conductivity (~20 W/(m*K)) -> adds +5°C to nozzle temp
--     so the polymer core reaches full melt viscosity.
--   - "MicroSwiss FlowTech CHT" / "Bi-Metal CHT" / "E3D ObXidian High-Flow": Split-core high-flow geometry -> boosts max volumetric speed +25%.
--
-- Curve helpers `pwl` / `clamp` / `math.sqrt` / `^` are Rust-native builtins of the Lua engine;
-- `scalar_num` / `map_num` come from the Rust-owned prelude (src/lua_engine.rs LUA_RULE_PRELUDE).

function apply(config, ctx)
  local mat = ctx.nozzle_material or "Brass"
  local ntype = ctx.nozzle_type or "orca_brass"
  local cur_temp = 220
  if config["nozzle_temperature"] and config["nozzle_temperature"][1] then
    cur_temp = tonumber(config["nozzle_temperature"][1]) or 220
  end

  local cur_init_temp = cur_temp + 5
  if config["nozzle_temperature_initial_layer"] and config["nozzle_temperature_initial_layer"][1] then
    cur_init_temp = tonumber(config["nozzle_temperature_initial_layer"][1]) or (cur_temp + 5)
  end

  if mat == "Diamondback PCD" or mat == "MicroSwiss Diamondback (PCD)" or ntype == "diamondback" or ntype == "microswiss_diamondback" then
    config["nozzle_type"] = "brass"
    config["nozzle_hrc"] = "68"
    local d = scalar_num(ctx.nozzle_diameter, 0.4)

    -- 1. Piecewise-linear PCD temperature curve dT(T0), applied per temperature key over its
    --    own brass base value (full -10/-11 C in the high-fan PLA/PETG band, tapering to -5 C
    --    at 300 C where inter-layer chain diffusion needs the enthalpy). Ceiling is the
    --    FlowTech assembly rating (300 C — PCD nozzles only fit FlowTech) intersected with
    --    this printer's hotend rating (ctx.hotend_max_temp_c from constants.json machines[]);
    --    floor is the generic 175 C extrusion minimum.
    local hotend_ceiling = math.min(300, scalar_num(ctx.hotend_max_temp_c, 300))
    local pcd_temp_pwl = {
      { 190, -8.0 },
      { 215, -11.0 },
      { 240, -10.0 },
      { 265, -7.5 },
      { 300, -5.0 }
    }
    local temp_keys = { "nozzle_temperature", "nozzle_temperature_initial_layer", "nozzle_temperature_HS", "nozzle_temperature_initial_layer_HS" }
    for _, key in ipairs(temp_keys) do
      if config[key] then
        config[key] = map_num(config[key], function(t0)
          return clamp(t0 + pwl(t0, pcd_temp_pwl), 175, hotend_ceiling)
        end, "%.0f")
      end
    end

    -- 2. Quadratic die-swell flow trim f1(f0, d): PCD's 0.01 CoF introduces partial wall slip
    --    (less die swell -> slightly less flow needed), scaled by orifice sqrt(0.4/d). The u^2
    --    normalization leaves low-flow matte/CF calibrations (f0 <= 0.90) virtually untouched
    --    and only trims glossy high-flow profiles (f0 ~ 0.96-1.00) by up to ~1.2%. Output band
    --    [0.900, 1.050] matches the app-wide flow sanity band (audit + pipeline-A clamp).
    if config["filament_flow_ratio"] then
      local orifice_factor = math.sqrt(0.4 / math.max(d, 0.2))
      config["filament_flow_ratio"] = map_num(config["filament_flow_ratio"], function(f0)
        local u = clamp((f0 - 0.85) / 0.15, 0.0, 1.2)
        local trim = 0.012 * orifice_factor * u * u
        return clamp(f0 * (1.0 - trim), 0.900, 1.050)
      end, "%.3f")
    end

    local new_temp = scalar_num(config["nozzle_temperature"], cur_temp)
    print("[ThermalConductivity] MicroSwiss Diamondback PCD (543 W/mK): nozzle_type=brass + nozzle_hrc=68, PWL temp curve -5..-11C by base (-> " .. tostring(new_temp) .. "C), quadratic die-swell flow trim <= 1.2%")
  elseif mat == "Brass (MicroSwiss TwinClad XT)" or ntype == "microswiss_brass" or ntype == "microswiss_mk8_brass" then
    config["nozzle_type"] = "brass"
    config["nozzle_hrc"] = "35"
    config["filament_flow_ratio"] = { "0.99" }
    config["filament_retraction_length"] = { "0.70" }
    config["filament_retraction_speed"] = { "45" }
    print("[ThermalConductivity] MicroSwiss Plated Brass (TwinClad XT): nozzle_type=brass + nozzle_hrc=35, Flow 0.99, Retract 0.70mm @ 45mm/s")
  elseif mat == "Ruby Tip (Brass Body)" or ntype == "ruby_brass" then
    config["nozzle_type"] = "brass"
    config["nozzle_hrc"] = "60"
    config["nozzle_temperature_HS"] = { tostring(cur_temp) }
    config["nozzle_temperature_initial_layer_HS"] = { tostring(cur_init_temp) }
    print("[ThermalConductivity] Brass-Body Ruby Tip Nozzle: nozzle_type=brass + nozzle_hrc=60 (0C Brass thermal conductivity + HRC 60 Ruby tip for -CF/-GF composites)")
  elseif mat == "MicroSwiss CM2 Hardened" or ntype == "microswiss_cm2" then
    config["nozzle_type"] = "hardened_steel"
    config["nozzle_hrc"] = "55"
    config["filament_retraction_length"] = { "0.60" }
    config["filament_retraction_speed"] = { "45" }
    print("[ThermalConductivity] MicroSwiss CM2 Hardened (Copper Core + M2 HSS): nozzle_type=hardened_steel + nozzle_hrc=55, 0C thermal offset, Retract -> 0.60mm @ 45mm/s")
  elseif mat == "Hardened Steel" or mat == "Stainless Steel" or ntype == "hardened_steel" or ntype == "orca_hardened_steel" or ntype == "orca_stainless_steel" then
    local new_temp = cur_temp + 5
    local new_init = cur_init_temp + 5
    config["nozzle_type"] = "hardened_steel"
    config["nozzle_hrc"] = "55"
    config["nozzle_temperature"] = { tostring(new_temp) }
    config["nozzle_temperature_initial_layer"] = { tostring(new_init) }
    config["nozzle_temperature_HS"] = { tostring(new_temp) }
    config["nozzle_temperature_initial_layer_HS"] = { tostring(new_init) }
    print("[ThermalConductivity] " .. mat .. ": nozzle_type=hardened_steel + nozzle_hrc=55, Added +5C thermal compensation -> " .. tostring(new_temp) .. "C")
  elseif mat == "Bi-Metal CHT" or mat == "MicroSwiss FlowTech CHT" or mat == "E3D ObXidian High-Flow" or ntype == "cht" or ntype == "microswiss_cht" or ntype == "bondtech_cht_bimetal" or ntype == "e3d_obxidian" then
    config["nozzle_type"] = "brass"
    config["nozzle_hrc"] = "55"
    if config["filament_max_volumetric_speed"] and config["filament_max_volumetric_speed"][1] then
      local mvs = tonumber(config["filament_max_volumetric_speed"][1]) or 15.0
      local boosted = mvs * 1.25
      config["filament_max_volumetric_speed"] = { string.format("%.1f", boosted) }
      config["filament_retraction_length"] = { "0.60" }
      print("[ThermalConductivity] High-Flow CHT/Splitter (" .. mat .. "): Boosted max volumetric speed +25% -> " .. string.format("%.1f", boosted) .. " mm3/s, Retract -> 0.60mm")
    end
  else
    config["nozzle_type"] = "brass"
    config["nozzle_hrc"] = "0"
    print("[ThermalConductivity] Standard Brass Nozzle: nozzle_type=brass + nozzle_hrc=0 (0C baseline factory thermal reference)")
  end
end
