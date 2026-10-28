-- Dynamic Combination Rule 1: Thermal Conductivity & Nozzle Material Compensation
-- Adjusts nozzle temperature, flow ratio, and retraction based on the installed nozzle material:
--   - "MicroSwiss Diamondback (PCD)" / "Diamondback PCD": ~500 W/(m*K) thermal conductivity -> drops nozzle temp by -10°C,
--     optimizes flow ratio to 0.94, reduces retraction length to 0.60mm @ 45mm/s.
--   - "Brass (MicroSwiss TwinClad XT)": Low-friction TwinClad XT nickel-composite coating -> 0°C temp offset,
--     reduces retraction length to 0.70mm @ 45mm/s and boosts flow ratio to 0.99.
--   - "Ruby Tip (Brass Body)" (`ruby_brass`): Brass body (~115 W/(m*K)) with synthetic Ruby gemstone tip (9.0 Mohs) ->
--     0°C Brass thermal baseline (no +5°C lag offset) while marking `nozzle_type = "hardened_steel"` so Slicer allows -CF and -GF filaments.
--   - "MicroSwiss CM2 Hardened": Copper core + M2 HSS tip -> 0°C offset, 0.60mm retraction @ 45mm/s.
--   - "Hardened Steel" / "Stainless Steel": Low thermal conductivity (~20 W/(m*K)) -> adds +5°C to nozzle temp
--     so the polymer core reaches full melt viscosity.
--   - "MicroSwiss FlowTech CHT" / "Bi-Metal CHT" / "E3D ObXidian High-Flow": Split-core high-flow geometry -> boosts max volumetric speed +25%.

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
    local new_temp = cur_temp - 10
    local new_init = cur_init_temp - 10
    config["nozzle_type"] = "brass"
    config["nozzle_hrc"] = "68"
    config["nozzle_temperature"] = { tostring(new_temp) }
    config["nozzle_temperature_initial_layer"] = { tostring(new_init) }
    config["nozzle_temperature_HS"] = { tostring(new_temp) }
    config["nozzle_temperature_initial_layer_HS"] = { tostring(new_init) }
    config["filament_flow_ratio"] = { "0.94" }
    config["filament_retraction_length"] = { "0.60" }
    config["filament_retraction_speed"] = { "45" }
    config["pressure_advance"] = { "0.024" }
    print("[ThermalConductivity] MicroSwiss Diamondback PCD (~500 W/mK): nozzle_type=brass + nozzle_hrc=68, Nozzle temp -10C -> " .. tostring(new_temp) .. "C, Flow -> 0.94, Retract -> 0.60mm @ 45mm/s")
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
