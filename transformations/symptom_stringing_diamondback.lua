-- Symptom 1: Stringing & Oozing with MicroSwiss + Diamondback
-- PCD (543 W/m*K, 0.01 CoF) holds heat to the very exit plane, so low-viscosity melts form
-- fine capillary hairs on travel moves. Everything here composes RELATIVELY on top of the
-- P10 thermal curve and the P12 FlowTech retraction curve — no absolute writes, so per-spool
-- brand calibrations always remain the foundation:
--   1. Temperature trim T2(T1, r1): -2 C inside the sticky PLA/PETG band only, tapering to 0 C
--      above 248 C (inter-layer welds on ABS/ASA/PC/PA) and gated to 0 C for elastomers
--      (r1 >= 1.0 mm: cooling TPU further buckles it in the direct-drive gears).
--   2. Retraction recovery r2(r1): clamped diminishing-returns bump (+<=0.07 mm) that tapers
--      to zero at the 1.25 mm all-metal heat-creep ceiling.
--   3. Speed damping s2(s1): PCD's near-zero wall drag means a fast yank snaps the melt column
--      and leaves a drooling droplet — quadratic damper pulls 44-46 back to 36-38 mm/s and
--      leaves gentle TPU speeds untouched.
--   4. Z-hop: orifice power-law lift on the per-filament key (filament_z_hop overrides the
--      process-level z_hop in OrcaSlicer precedence), with Spiral Lift + wipe.
--   5. Travel acceleration: asymptotic boost toward 12000 mm/s^2 (floor 9000) for a crisp
--      breakaway at the perimeter exit — always clears the <8000 -> 10000 export clamp.
--
-- `pwl` / `clamp` are Rust-native builtins; `scalar_num` / `map_num` come from the Rust-owned
-- prelude (src/lua_engine.rs LUA_RULE_PRELUDE).

function apply(config, ctx)
  local d = scalar_num(ctx.nozzle_diameter, 0.4)
  -- Read `r0_ref` from the immutable `base` (x0) snapshot so the elastomer/long-travel gate is
  -- anchored to the polymer's true brass baseline rather than drifting when P12 or another
  -- earlier rule compresses `config["filament_retraction_length"]`.
  local r0_ref = scalar_num(base["filament_retraction_length"], scalar_num(config["filament_retraction_length"], 0.80))
  -- PCD-specific physics (temp trim, friction-based speed damping) applies ONLY to actual
  -- PCD/diamondback nozzles — the rule is enabled for every MicroSwiss rig for its generic
  -- anti-stringing writes (recovery bump, z-hop, travel accel, wipe), but a CM2/CHT copper
  -- or steel nozzle is not a 543 W/m*K near-zero-friction tip and must not be cooled or
  -- damped as one.
  local nt = string.lower(tostring(ctx.nozzle_type or ""))
  local is_pcd = string.find(nt, "diamondback", 1, true) ~= nil

  -- 1. Viscosity- & elastomer-gated extra temperature trim evaluated on the immutable brass
  --    baseline `base[key]` (T0) and applied relatively to `config[key]` (Tk):
  --    - Full -2 C inside the sticky PLA/PETG brass baseline band (213 - 236 C)
  --    - Tapering to 0 C at >= 258 C (preserving inter-layer welds on ABS/ASA/PC/PA)
  --    - Gated to 0 C for elastomers (`ctx.filament_is_elastomer` or long-travel r0 >= 1.22 mm)
  local stringing_temp_pwl_t0 = {
    { 190, -1.0 },
    { 213, -2.0 }, -- PLA brass baseline band (~210-215 C)
    { 236, -2.0 }, -- PETG brass baseline band (~230-236 C)
    { 258,  0.0 }, -- engineering polymers (ASA/ABS/PC/PA >= 258 C): zero extra drop
    { 305,  0.0 }
  }
  local is_elastomer = ctx.filament_is_elastomer == true
  local rigid_gate = 0.0
  if not is_elastomer then
    rigid_gate = clamp((1.22 - r0_ref) / 0.25, 0.0, 1.0)
  end
  if is_pcd then
    for _, key in ipairs({ "nozzle_temperature", "nozzle_temperature_HS" }) do
      if config[key] then
        local t0_ref = scalar_num(base[key], scalar_num(config[key], 220))
        local dt_trim = pwl(t0_ref, stringing_temp_pwl_t0) * rigid_gate
        config[key] = map_num(config[key], function(t_cur)
          return math.max(t_cur + dt_trim, 175)
        end, "%.0f")
      end
    end
  end

  -- 2. Clamped diminishing-returns retraction recovery over P12's compressed value.
  if config["filament_retraction_length"] then
    config["filament_retraction_length"] = map_num(config["filament_retraction_length"], function(r1)
      local headroom = clamp((1.25 - r1) / 0.75, 0.0, 1.0)
      return math.min(r1 + 0.07 * headroom, 1.35)
    end, "%.2f")
  end

  -- 3. Quadratic retraction speed damping: s2 = s1 - 0.018 * max(0, s1 - 25)^2, floor 22.
  --    PCD-only (wall-drag physics; CM2/CHT steel nozzles keep P12's speed) and skipped for
  --    elastomers, whose calibrated speed must pass through untouched (TPU for AMS ships
  --    35 mm/s and must export 35, not a damped 33).
  if is_pcd and not is_elastomer and config["filament_retraction_speed"] then
    config["filament_retraction_speed"] = map_num(config["filament_retraction_speed"], function(s1)
      local excess = math.max(0.0, s1 - 25.0)
      return math.max(22.0, s1 - 0.018 * excess * excess)
    end, "%.0f")
  end

  -- 4. Orifice-scaled power-law Z-hop on the per-filament key (slicer-precedence winner).
  --    Ceiling max(z0 + 0.04, 0.65*d) is anchored to `base["filament_z_hop"]` (z0) so it never
  --    ratchets upward if multiple Z-hop rules run, while guaranteeing a small-orifice spool's
  --    existing z0 is never reduced.
  if config["filament_z_hop"] then
    local z0_ref = scalar_num(base["filament_z_hop"], scalar_num(config["filament_z_hop"], 0.15))
    local max_hop = math.max(z0_ref + 0.04, d * 0.65)
    local hop_bump = 0.02 + 0.05 * ((d / 0.4) ^ 1.15)
    config["filament_z_hop"] = map_num(config["filament_z_hop"], function(z_cur)
      return clamp(z_cur + hop_bump, 0.12, max_hop)
    end, "%.2f")
    constrain("filament_z_hop", z0_ref > 0 and 0.10 or 0.0, max_hop, "Orifice-scaled Z-hop envelope")
  end
  config["filament_z_hop_types"] = { "Spiral Lift" }

  -- 5. Asymptotic breakaway travel acceleration toward the printer-class target
  --    (ctx.travel_accel_target_mm2 / _floor_mm2 from constants.json `printer_classes`:
  --    12000/9000 on a CoreXY, gentler on a bed-slinger). Values above the target pass
  --    through unchanged; the floor always clears the export-side clamp.
  local a0_base = scalar_num(base["travel_acceleration"], scalar_num(config["travel_acceleration"], 10000))
  local a_cur = scalar_num(config["travel_acceleration"], a0_base)
  local ta_target = scalar_num(ctx.travel_accel_target_mm2, 12000)
  local ta_floor = scalar_num(ctx.travel_accel_floor_mm2, 9000)
  local a_new = a_cur + 0.35 * math.max(0, ta_target - a_cur)
  config["travel_acceleration"] = tostring(math.floor(math.max(a_new, ta_floor)))
  constrain("travel_acceleration", ta_floor, math.max(15000, a0_base), "Printer kinematics travel acceleration envelope")

  -- 6. Mechanical inward wipe before the travel move shears the strand off the tip.
  config["wipe_on_loops"] = "1"
  sync_coupled_physics(config, ctx)

  print("[Symptom:Stringing] PCD anti-stringing: gated -2C trim (rigid_gate " .. string.format("%.2f", rigid_gate) .. "), retraction recovery <= +0.07mm, speed damp -> " .. tostring(scalar_num(config["filament_retraction_speed"], 0)) .. " mm/s, filament_z_hop +orifice bump, travel accel -> " .. config["travel_acceleration"] .. " mm/s2, wipe=1")
end
