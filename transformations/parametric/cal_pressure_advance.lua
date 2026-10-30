-- Calibration Print #3: Pressure Advance (PA) Pattern / Line Calibration
meta = {
  id = "cal_pressure_advance",
  name = "Pressure Advance (PA) Pattern / Line Calibration",
  icon = "📐",
  category = "Calibration Print #3 (Pressure Advance)",
  tree_category = "Post-Print Calibration",
  tree_subcategory = "Calibration Test",
  slicer_menu_path = "Calibration → Pressure Advance → PA Pattern (Recommended) / PA Line / PA Tower",
  when_to_run = "Run 3rd (after Flow Rate is dialed in) to eliminate bulging or gaps at sharp 90° corners and seam starts.",
  formula_summary = "Sets enable_pressure_advance = 1 and pressure_advance = Measured_Best_Corner_PA",
  default_condition_expr = "filament:PCTG",
  how_to_read_print = {
    "1. Click Calibration → Pressure Advance → PA Pattern (Direct Drive: 0.000 to 0.080, step 0.002) and print the chevron pattern.",
    "2. Inspect the sharp 90° corner apex of each numbered chevron: too LOW PA leaves a swollen, rounded bulge; too HIGH PA thins the corner or leaves a pinhole gap.",
    "3. Select the PA number next to the sharpest, most uniform corner below.",
  },
  hardware_checklist = {
    "🖨️ Slice and print the built-in test model from the AnycubicSlicerNext / OrcaSlicer top 'Calibration' menu before entering your measurements below.",
  },
}

params = {
  {
    id = "pa_val",
    label = "1. Which PA value on the test print produced the cleanest, sharpest corner?",
    description = "For Kobra S1 Direct Drive: PLA is typically 0.028–0.038, PETG is 0.040–0.050, and PCTG is 0.044–0.052.",
    type = "range",
    min = 0.000,
    max = 0.250,
    step = 0.002,
    default = "0.048",
    unit = "s",
    options = {
      { value = "0.028", label = "0.028 (Fast PLA)" },
      { value = "0.034", label = "0.034 (Standard PLA+)" },
      { value = "0.040", label = "0.040 (PLA-CF / PETG)" },
      { value = "0.045", label = "0.045 (PETG)" },
      { value = "0.048", label = "0.048 (PCTG Sweet Spot)" },
      { value = "0.052", label = "0.052 (PCTG / TPU)" },
      { value = "0.060", label = "0.060 (High Elasticity)" },
    },
  },
  {
    id = "adaptive_pa",
    label = "2. Enable Adaptive Pressure Advance on Overhangs & Bridges?",
    description = "Adaptive PA lowers PA during slow overhangs/bridges so steep edges don't curl.",
    type = "enum",
    default = "0",
    unit = "",
    options = {
      { value = "0", label = "Off — Fixed PA everywhere (Recommended)" },
      { value = "1", label = "On — Enable Adaptive PA on overhangs & bridges" },
    },
  },
}

function validate(p, ctx)
  local pa = tonumber(p.pa_val)
  if pa == nil or pa < 0.0 or pa > 0.25 then
    return false, "Pressure Advance (pa_val) must be between 0.000 and 0.250 s for Direct Drive extruders."
  end
  return true
end

function describe(p, ctx)
  local pa_num = math.max(0.0, math.min(0.25, tonumber(p.pa_val) or 0.048))
  local adaptive = p.adaptive_pa or "0"
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local adaptive_lbl = "Off"
  if adaptive == "1" then
    adaptive_lbl = "On"
  end
  local badge = string.format("pressure_advance = %.3f (Adaptive PA: %s)", pa_num, adaptive_lbl)
  return {
    title = string.format("Calibration: Pressure Advance = %.3f", pa_num),
    description = string.format("Post-print PA Pattern/Line calibration for %s: sets pressure_advance = %.3f (enable_pressure_advance = 1, adaptive = %s).", scope, pa_num, adaptive),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local pa_num = math.max(0.0, math.min(0.25, tonumber(p and p.pa_val) or 0.048))
  local adaptive = (p and p.adaptive_pa) or "0"
  config["enable_pressure_advance"] = "1"
  config["pressure_advance"] = string.format("%.3f", pa_num)
  config["adaptive_pressure_advance"] = adaptive
  local d = describe(p or {}, ctx)
  print("[Calibration:PA] " .. (d.badge or ""))
end
