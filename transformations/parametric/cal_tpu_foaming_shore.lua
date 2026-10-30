-- Active-Foaming TPU Shore Hardness & Expansion Tuner (colorFabb varioShore / eSUN eTPU-LW / Siraya Air)
-- Dynamically couples Target Shore Hardness (55A–92A) to Nozzle Temperature (195–250°C Brass baseline +
-- nozzle thermal offset), Compensated Flow Ratio (0.58–0.98), Equalized Residence-Time Print Speeds,
-- Zero Z-Hop, and Minimal/Zero Retraction.

meta = {
  id = "cal_tpu_foaming_shore",
  name = "Active-Foaming TPU Shore Hardness & Expansion Tuner (60A–92A)",
  category = "filament",
  icon = "🧽",
  default_scope = "filament:TPU",
  summary = "Tunes active-foaming TPU (varioShore / eTPU-LW) Shore hardness (60A–92A) via nozzle temperature, expansion-compensated flow ratio, equalized wall/infill speeds, and zero Z-hop.",
  guide = "Active-foaming TPU contains a chemical blowing agent that activates around 200°C and expands up to 1.6× volume at 240–250°C. Raising nozzle temperature lowers Shore hardness (92A -> 60A) and density, requiring a proportional reduction in flow_ratio (1.00 -> 0.60) and equalized print speeds so residence time in the melt zone stays uniform.",
}

params = {
  {
    id = "shore_preset",
    label = "1. Target Shore Hardness Preset (Snaps Shore, Flow & MVS Dials)",
    description = "Pick a Shore hardness preset to snap the Shore A and Volumetric Speed dials, or drag any dial below for custom intermediate hardness.",
    type = "enum",
    default = "75a_midsole",
    options = {
      {
        value = "92a_solid",
        label = "🪨 92A Unfoamed Solid (195°C Brass • 0.98 Flow • 4.5 mm³/s • 100% Density)",
        sets = { target_shore_a = "92", mvs_ceiling = "4.5", caliper_flow_trim = "0.00" },
      },
      {
        value = "85a_firm",
        label = "👟 85A Light Foam / Firm Cushion (210°C Brass • 0.85 Flow • 4.0 mm³/s • -15% Weight)",
        sets = { target_shore_a = "85", mvs_ceiling = "4.0", caliper_flow_trim = "0.00" },
      },
      {
        value = "75a_midsole",
        label = "🧽 75A Medium Foam / Shoe Midsole (220°C Brass • 0.75 Flow • 3.5 mm³/s • -25% Weight)",
        sets = { target_shore_a = "75", mvs_ceiling = "3.5", caliper_flow_trim = "0.00" },
      },
      {
        value = "65a_soft",
        label = "🛞 65A High Foam / Soft Grip & Tire (235°C Brass • 0.65 Flow • 3.0 mm³/s • -35% Weight)",
        sets = { target_shore_a = "65", mvs_ceiling = "3.0", caliper_flow_trim = "0.00" },
      },
      {
        value = "60a_max",
        label = "☁️ 60A Max Expansion Ultra-Soft Foam (245°C Brass • 0.60 Flow • 2.5 mm³/s • -40% Weight)",
        sets = { target_shore_a = "60", mvs_ceiling = "2.5", caliper_flow_trim = "0.00" },
      },
      {
        value = "custom",
        label = "🎛️ Custom Shore Hardness & Caliper Flow Dials",
      },
    },
  },
  {
    id = "target_shore_a",
    label = "2. Target Shore A Hardness Dial (55A – 92A)",
    description = "Controls the foaming activation temperature and expansion-compensated flow ratio. Lower Shore A = hotter nozzle + more foaming expansion + softer feel.",
    type = "range",
    min = 55,
    max = 92,
    step = 1,
    default = "75",
    unit = "A",
    options = {
      { value = "92", label = "92A (Unfoamed Solid — 195°C Brass, 0.98 Flow)" },
      { value = "85", label = "85A (Light Foam — 210°C Brass, 0.85 Flow)" },
      { value = "75", label = "75A (Medium Foam — 220°C Brass, 0.75 Flow)" },
      { value = "65", label = "65A (High Foam — 235°C Brass, 0.65 Flow)" },
      { value = "60", label = "60A (Max Foam — 245°C Brass, 0.60 Flow)" },
    },
  },
  {
    id = "caliper_flow_trim",
    label = "3. Single-Wall Caliper Flow Trim Offset (Δ Flow Ratio)",
    description = "Fine-tune the computed foaming flow ratio after measuring a 1-wall calibration cube with calipers: Δ = (Target_Width / Measured_Width - 1) × Flow.",
    type = "range",
    min = -0.10,
    max = 0.10,
    step = 0.01,
    default = "0.00",
    unit = "",
    options = {
      { value = "-0.05", label = "-0.05 (Wall measured ~7% too thick)" },
      { value = "-0.02", label = "-0.02 (Wall measured ~3% too thick)" },
      { value = "0.00", label = "0.00 (Use exact Shore expansion curve)" },
      { value = "0.02", label = "+0.02 (Wall measured ~3% too thin)" },
      { value = "0.05", label = "+0.05 (Wall measured ~7% too thin)" },
    },
  },
  {
    id = "mvs_ceiling",
    label = "4. Residence-Time Max Volumetric Speed Ceiling (mm³/s)",
    description = "Caps volumetric flow and equalizes outer wall, inner wall, and infill speeds so the foaming agent receives identical heat soak across the entire part.",
    type = "range",
    min = 2.0,
    max = 5.0,
    step = 0.1,
    default = "3.5",
    unit = "mm³/s",
    options = {
      { value = "2.5", label = "2.5 mm³/s (~30 mm/s — Best for 60A–65A Max Foam)" },
      { value = "3.0", label = "3.0 mm³/s (~35 mm/s — Best for 65A–70A High Foam)" },
      { value = "3.5", label = "3.5 mm³/s (~40 mm/s — Best for 75A Medium Foam)" },
      { value = "4.5", label = "4.5 mm³/s (~50 mm/s — 85A–92A Low/No Foam)" },
    },
  },
  {
    id = "retraction_mode",
    label = "5. Foaming Melt-Zone Retraction Mode",
    description = "Foaming gas continues expanding inside the nozzle during travel. Use Minimal (0.35mm, 0mm Z-Hop) or Zero Retraction (0.00mm continuous gasket/vase mode).",
    type = "enum",
    default = "minimal_035",
    options = {
      { value = "minimal_035", label = "Minimal Retraction (0.35 mm @ 25 mm/s • 0.00 mm Z-Hop • Avoid Crossing Walls)" },
      { value = "zero_continuous", label = "Zero Retraction (0.00 mm Continuous / Vase / Gasket Mode)" },
    },
  },
}

local function get_nozzle_thermal_offset_c(ctx)
  local nt = string.lower((ctx and ctx.nozzle_type and tostring(ctx.nozzle_type)) or "")
  local nm = string.lower((ctx and ctx.nozzle_material and tostring(ctx.nozzle_material)) or "")
  if string.find(nt, "diamondback") or string.find(nm, "diamondback") then
    return -10, "Diamondback PCD (-10°C)"
  elseif (string.find(nt, "hardened") or string.find(nm, "hardened") or string.find(nt, "stainless") or string.find(nm, "stainless"))
    and not string.find(nt, "cm2") and not string.find(nm, "cm2")
  then
    return 5, "Hardened Steel (+5°C)"
  else
    return 0, "Brass / Copper Core (0°C)"
  end
end

-- Piecewise-linear foaming curve matching colorFabb varioShore TPU / CNC Kitchen empirical curve:
--   92A -> 195°C Brass, 0.98 flow, 70% fan
--   85A -> 210°C Brass, 0.85 flow, 50% fan
--   75A -> 220°C Brass, 0.75 flow, 40% fan
--   65A -> 235°C Brass, 0.65 flow, 25% fan
--   60A -> 245°C Brass, 0.60 flow, 15% fan
--   55A -> 250°C Brass, 0.56 flow, 10% fan
local function compute_foaming_curve(shore_a, trim, noz_offset_c)
  local s = math.max(55, math.min(92, shore_a))
  local brass_c = 220
  local base_flow = 0.75
  local fan_max = 40

  if s >= 85 then
    local t = (92 - s) / 7.0
    brass_c = math.floor(195 + t * 15 + 0.5)
    base_flow = 0.98 - t * 0.13
    fan_max = math.floor(70 - t * 20 + 0.5)
  elseif s >= 75 then
    local t = (85 - s) / 10.0
    brass_c = math.floor(210 + t * 10 + 0.5)
    base_flow = 0.85 - t * 0.10
    fan_max = math.floor(50 - t * 10 + 0.5)
  elseif s >= 65 then
    local t = (75 - s) / 10.0
    brass_c = math.floor(220 + t * 15 + 0.5)
    base_flow = 0.75 - t * 0.10
    fan_max = math.floor(40 - t * 15 + 0.5)
  elseif s >= 60 then
    local t = (65 - s) / 5.0
    brass_c = math.floor(235 + t * 10 + 0.5)
    base_flow = 0.65 - t * 0.05
    fan_max = math.floor(25 - t * 10 + 0.5)
  else
    local t = (60 - s) / 5.0
    brass_c = math.floor(245 + t * 5 + 0.5)
    base_flow = 0.60 - t * 0.04
    fan_max = math.floor(15 - t * 5 + 0.5)
  end

  local effective_nozzle_c = math.max(180, math.min(265, brass_c + noz_offset_c))
  local effective_flow = math.max(0.50, math.min(1.05, base_flow + trim))
  local density_pct = math.floor(effective_flow * 100 + 0.5)
  return {
    shore_a = math.floor(s + 0.5),
    brass_c = brass_c,
    nozzle_c = effective_nozzle_c,
    base_flow = base_flow,
    effective_flow = effective_flow,
    density_pct = density_pct,
    fan_min = math.max(0, fan_max - 15),
    fan_max = fan_max,
  }
end

function validate(p, ctx)
  local shore = tonumber(p.target_shore_a) or 75
  local trim = tonumber(p.caliper_flow_trim) or 0.0
  local mvs = tonumber(p.mvs_ceiling) or 3.5

  if shore < 55 or shore > 92 then
    return false, "Target Shore A hardness must be between 55A and 92A."
  end
  if trim < -0.15 or trim > 0.15 then
    return false, "Caliper flow trim offset must be between -0.15 and +0.15."
  end
  if mvs < 1.5 or mvs > 6.0 then
    return false, "Max Volumetric Speed ceiling for foaming TPU must be between 1.5 and 6.0 mm³/s."
  end

  local noz_off, _ = get_nozzle_thermal_offset_c(ctx)
  local c = compute_foaming_curve(shore, trim, noz_off)
  if shore <= 70 and c.effective_flow > 0.80 then
    return false, string.format(
      "At %dA (%d°C), foaming TPU expands ~1.4x–1.6x; an effective flow ratio of %.2f (> 0.80) will severely over-extrude and jam the nozzle.",
      c.shore_a, c.nozzle_c, c.effective_flow
    )
  end
  return true
end

function describe(p, ctx)
  local shore = tonumber(p.target_shore_a) or 75
  local trim = tonumber(p.caliper_flow_trim) or 0.0
  local mvs = tonumber(p.mvs_ceiling) or 3.5
  local ret_mode = p.retraction_mode or "minimal_035"
  local noz_off, noz_label = get_nozzle_thermal_offset_c(ctx)
  local c = compute_foaming_curve(shore, trim, noz_off)
  local eq_speed = math.max(20, math.min(55, math.floor(mvs / (0.20 * 0.42) + 0.5)))
  local ret_str = (ret_mode == "zero_continuous") and "0.00mm Retract" or "0.35mm @ 25mm/s Retract"

  local badge = string.format(
    "Target %dA (%d%% Density) • Nozzle %d°C [%s] • Flow %.2f • MVS %.1f mm³/s (~%d mm/s Equalized) • %s • 0.00mm Z-Hop",
    c.shore_a, c.density_pct, c.nozzle_c, noz_label, c.effective_flow, mvs, eq_speed, ret_str
  )
  return {
    title = string.format(
      "Active-Foaming TPU: %dA Shore (%d°C, %.2f Flow, %.1f mm³/s)",
      c.shore_a, c.nozzle_c, c.effective_flow, mvs
    ),
    description = string.format(
      "Tunes active-foaming TPU to ~%dA Shore hardness (%d%% part density): sets nozzle to %d°C (%d°C Brass curve %+d°C nozzle offset), compensated flow_ratio = %.2f (base %.2f %+.2f trim), MVS = %.1f mm³/s (~%d mm/s equalized wall/infill speeds), %s, and 0.00mm Z-Hop.",
      c.shore_a, c.density_pct, c.nozzle_c, c.brass_c, noz_off, c.effective_flow, c.base_flow, trim, mvs, eq_speed, ret_str
    ),
    badge = badge,
    checklist = {
      string.format("🧽 Shore %dA Expansion Target: Nozzle %d°C (%s) activates foaming expansion to ~%.2fx volume -> compensated flow_ratio = %.2f (%d%% weight/density).", c.shore_a, c.nozzle_c, noz_label, 1.0 / c.base_flow, c.effective_flow, c.density_pct),
      string.format("⏱️ Equalized Melt-Zone Residence Time: Outer wall, inner wall, and infill speeds are equalized to ~%d mm/s (MVS %.1f mm³/s) so Shore hardness stays uniform across walls and infill.", eq_speed, mvs),
      "📏 Single-Wall Caliper Verification: Print a 1-wall hollow cube (0% infill, 0 top layers) and measure wall thickness with calipers; adjust Dial #3 if measured thickness differs from line_width.",
      "🧵 External Spool / Low-Friction Feed: Feed foaming TPU from a low-friction external drybox (55–60°C for 6+ hrs); keep Z-Hop at 0.00mm and Avoid Crossing Walls enabled.",
    },
  }
end

function apply(config, ctx, p)
  local shore = tonumber(p and p.target_shore_a) or 75
  local trim = tonumber(p and p.caliper_flow_trim) or 0.0
  local mvs = tonumber(p and p.mvs_ceiling) or 3.5
  local ret_mode = (p and p.retraction_mode) or "minimal_035"
  local noz_off, noz_label = get_nozzle_thermal_offset_c(ctx)
  local c = compute_foaming_curve(shore, trim, noz_off)

  local function set_val(key, val_str)
    if type(config[key]) == "table" then
      config[key] = { val_str }
    else
      config[key] = val_str
    end
  end

  -- 1. Nozzle Temperature (Shore-driven Brass baseline + Nozzle Thermal Conductivity Offset)
  set_val("nozzle_temperature", tostring(c.nozzle_c))
  set_val("nozzle_temperature_initial_layer", tostring(c.nozzle_c))
  set_val("nozzle_temperature_BRASS", tostring(c.brass_c))
  set_val("nozzle_temperature_HS", tostring(c.brass_c + 5))

  -- 2. Expansion-Compensated Flow Ratio
  set_val("filament_flow_ratio", string.format("%.3f", c.effective_flow))

  -- 3. Residence-Time Volumetric Speed Ceiling & Equalized Wall/Infill Speeds
  set_val("filament_max_volumetric_speed", string.format("%.1f", mvs))
  local lh = (ctx and tonumber(ctx.layer_height)) or tonumber(config["layer_height"]) or 0.20
  local lw = (ctx and tonumber(ctx.line_width)) or tonumber(config["outer_wall_line_width"]) or 0.42
  local eq_speed = math.max(20, math.min(55, math.floor(mvs / math.max(0.04, lh * lw) + 0.5)))
  config["outer_wall_speed"] = tostring(eq_speed)
  config["inner_wall_speed"] = tostring(eq_speed)
  config["sparse_infill_speed"] = tostring(eq_speed)
  config["internal_solid_infill_speed"] = tostring(eq_speed)
  config["top_surface_speed"] = tostring(math.max(20, eq_speed - 5))

  -- 4. Part Cooling Fan Schedule (Lower fan at higher foaming temperatures so micro-cells expand evenly)
  set_val("fan_min_speed", tostring(c.fan_min))
  set_val("fan_max_speed", tostring(c.fan_max))

  -- 5. Zero Z-Hop, Minimal/Zero Retraction, and Avoid Crossing Walls
  set_val("z_hop", "0.00")
  set_val("filament_z_hop", "0.00")
  set_val("z_hop_types", "Normal Lift")
  set_val("filament_z_hop_types", "Normal Lift")
  if ret_mode == "zero_continuous" then
    set_val("filament_retraction_length", "0.00")
  else
    set_val("filament_retraction_length", "0.35")
    set_val("filament_retraction_speed", "25")
    set_val("filament_deretraction_speed", "20")
  end
  config["reduce_crossing_wall"] = "1"
  config["max_travel_detour_distance"] = "100"

  print(string.format(
    "[FoamingTPU:%dA] Nozzle=%d°C (%s), Flow=%.3f (%d%% density), MVS=%.1f mm³/s (~%d mm/s equalized), Retract=%s, Z-Hop=0.00mm",
    c.shore_a, c.nozzle_c, noz_label, c.effective_flow, c.density_pct, mvs, eq_speed, ret_mode
  ))
end
