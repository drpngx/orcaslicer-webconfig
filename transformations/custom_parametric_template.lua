-- Custom Parametric Lua Rule Bootstrap Template
-- Edit `params`, `validate(p, ctx)`, `describe(p, ctx)`, and `apply(config, ctx, p)` below.
-- Parameter UI controls and validation checks are generated automatically from this file.

meta = {
  id = "custom_parametric_rule",
  name = "Custom Parametric Lua Rule",
  category = "Custom Parametric Rule",
  icon = "⚙️",
  default_scope = "all",
  formula_summary = "config[key] = f(params, config, ctx) with Lua range/enum & combination validation",
}

params = {
  {
    id = "mode",
    label = "1. Tuning Profile Mode (enum)",
    description = "Select the overall tuning intent for this custom transformation.",
    type = "enum",
    default = "balanced",
    options = {
      { value = "quality", label = "Surface Quality Priority (Slower Outer Walls)" },
      { value = "balanced", label = "Balanced Daily Driver (1.00x Baseline)" },
      { value = "speed", label = "High-Throughput Speed Mode" },
    },
  },
  {
    id = "temp_offset",
    label = "2. Nozzle Temperature Offset (range)",
    description = "Relative temperature adjustment applied to nozzle_temperature (°C).",
    type = "range",
    min = -20,
    max = 20,
    step = 1,
    default = "0",
    unit = "°C",
    options = {
      { value = "-5", label = "-5 °C (Reduce Oozing / Stringing)" },
      { value = "0", label = "0 °C (Keep Base Temp)" },
      { value = "5", label = "+5 °C (Boost Layer Bonding / Flow)" },
      { value = "10", label = "+10 °C (High-Speed Melt Boost)" },
    },
  },
  {
    id = "outer_wall_scale",
    label = "3. Outer Wall Speed Multiplier (range)",
    description = "Relative multiplier applied to outer_wall_speed (e.g. 0.75 = -25% slower).",
    type = "range",
    min = 0.25,
    max = 1.50,
    step = 0.05,
    default = "0.85",
    unit = "×",
    options = {
      { value = "0.65", label = "0.65× (-35% Showpiece Surface Quality)" },
      { value = "0.85", label = "0.85× (-15% Clean Perimeters)" },
      { value = "1.00", label = "1.00× (Unchanged)" },
      { value = "1.15", label = "1.15× (+15% Faster Draft)" },
    },
  },
  {
    id = "enable_wipe",
    label = "4. Enable Wipe on Loops (boolean)",
    description = "Toggles wipe_on_loops to hide seam termination inside the perimeter loop.",
    type = "boolean",
    default = "1",
    options = {
      { value = "1", label = "Enabled (1)" },
      { value = "0", label = "Disabled (0)" },
    },
  },
}

function validate(p, ctx)
  local mode = p.mode or "balanced"
  local temp_off = tonumber(p.temp_offset) or 0
  local scale = tonumber(p.outer_wall_scale) or 0.85

  -- Cross-parameter validation in Lua (blocks saving invalid combinations):
  if mode == "speed" and scale < 0.90 then
    return false, "High-Throughput Speed Mode ('speed') conflicts with slowing outer_wall_scale below 0.90x."
  end
  if scale > 1.10 and temp_off < 0 then
    return false, "Increasing outer wall speed (> 1.10x) while lowering nozzle temperature (< 0 °C) risks under-extrusion and poor layer adhesion."
  end
  if scale < 0.50 and temp_off > 10 then
    return false, "Ultra-slow speed (< 0.50x) combined with > +10 °C temperature boost risks heat creep in the hotend throat."
  end
  return true
end

function describe(p, ctx)
  local scope = (ctx and ctx.scope) or "all"
  local mode = p.mode or "balanced"
  local temp_off = math.floor((tonumber(p.temp_offset) or 0) + 0.5)
  local scale = tonumber(p.outer_wall_scale) or 0.85
  local wipe = (p.enable_wipe or "1") == "1" and "wipe=ON" or "wipe=OFF"

  return {
    title = string.format("Custom Rule (%s): %+.0f°C Temp & %.2fx Outer Wall", mode, temp_off, scale),
    description = string.format(
      "Custom parametric rule for %s (mode=%s): nozzle_temperature %+d°C, outer_wall_speed × %.2f, %s.",
      scope, mode, temp_off, scale, wipe
    ),
    badge = string.format("mode = %s • temp %+d°C • outer_wall × %.2f • %s", mode, temp_off, scale, wipe),
  }
end

function apply(config, ctx, p)
  local temp_off = math.floor((tonumber(p.temp_offset) or 0) + 0.5)
  local scale = tonumber(p.outer_wall_scale) or 0.85
  local wipe = p.enable_wipe or "1"

  -- 1. Apply relative nozzle temperature offset if non-zero
  if temp_off ~= 0 and config["nozzle_temperature"] ~= nil then
    local is_arr = type(config["nozzle_temperature"]) == "table"
    local cur = tonumber(is_arr and config["nozzle_temperature"][1] or config["nozzle_temperature"]) or 220
    local next_t = tostring(math.floor(cur + temp_off + 0.5))
    config["nozzle_temperature"] = is_arr and { next_t } or next_t
  end

  -- 2. Scale outer_wall_speed proportionally
  if math.abs(scale - 1.0) > 0.001 and config["outer_wall_speed"] ~= nil then
    local is_arr = type(config["outer_wall_speed"]) == "table"
    local cur_spd = tonumber(is_arr and config["outer_wall_speed"][1] or config["outer_wall_speed"]) or 120
    local next_spd = tostring(math.max(15, math.floor(cur_spd * scale + 0.5)))
    config["outer_wall_speed"] = is_arr and { next_spd } or next_spd
  end

  -- 3. Set boolean flag
  config["wipe_on_loops"] = tostring(wipe)

  print(string.format("[CustomRule] mode=%s, temp_offset=%+dC, outer_wall_scale=%.2fx, wipe_on_loops=%s", tostring(p.mode), temp_off, scale, tostring(wipe)))
end
