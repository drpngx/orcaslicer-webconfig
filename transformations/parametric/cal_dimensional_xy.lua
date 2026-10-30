-- Calibration Print #6: Dimensional Accuracy & X-Y Hole / Contour Compensation
meta = {
  id = "cal_dimensional_xy",
  name = "Dimensional Accuracy & X-Y Hole / Contour Compensation",
  icon = "📏",
  category = "Calibration Print #6 (Dimensional Fit)",
  tree_category = "Post-Print Calibration",
  tree_subcategory = "Calibration Test",
  slicer_menu_path = "Calibration → Tolerance Test / 20×20mm Cube with 10mm Center Bore",
  when_to_run = "Run when press-fit bearings, dowels, or M3/M5 screw holes print slightly undersized or oversized.",
  formula_summary = "xy_contour_compensation = (20.00 - Measured_Outer) / 2  •  xy_hole_compensation = (10.00 - Measured_Hole) / 2",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Print a 20.00 mm outer calibration cube that has a 10.00 mm vertical hole in the center.",
    "2. Wait for the part to cool to room temperature (20°C), then measure the outer width and inner bore diameter with calipers.",
    "3. Enter both caliper readings below: we calculate xy_contour_compensation = (20.00 - Outer) / 2 and xy_hole_compensation = (10.00 - Hole) / 2.",
  },
  hardware_checklist = {
    "🖨️ Slice and print the built-in test model from the AnycubicSlicerNext / OrcaSlicer top 'Calibration' menu before entering your measurements below.",
  },
}

params = {
  {
    id = "measured_outer",
    label = "1. What did your calipers measure for the 20.00 mm outer cube width (mm)?",
    description = "If it measured 20.08 mm (slightly fat), xy_contour_compensation will be -0.04 mm.",
    type = "range",
    min = 19.00,
    max = 21.00,
    step = 0.01,
    default = "20.04",
    unit = "mm",
    options = {
      { value = "19.90", label = "19.90 mm (+0.05mm comp)" },
      { value = "19.96", label = "19.96 mm (+0.02mm comp)" },
      { value = "20.00", label = "20.00 mm (0.00mm exact)" },
      { value = "20.04", label = "20.04 mm (-0.02mm comp)" },
      { value = "20.08", label = "20.08 mm (-0.04mm comp)" },
    },
  },
  {
    id = "measured_hole",
    label = "2. What did your calipers measure for the 10.00 mm inner center hole (mm)?",
    description = "Inner holes almost always shrink slightly in FDM (e.g. 9.84 mm → +0.08 mm xy_hole_compensation).",
    type = "range",
    min = 9.00,
    max = 11.00,
    step = 0.01,
    default = "9.84",
    unit = "mm",
    options = {
      { value = "9.76", label = "9.76 mm (+0.12mm hole comp)" },
      { value = "9.80", label = "9.80 mm (+0.10mm hole comp)" },
      { value = "9.84", label = "9.84 mm (+0.08mm hole comp)" },
      { value = "9.90", label = "9.90 mm (+0.05mm hole comp)" },
      { value = "10.00", label = "10.00 mm (0.00mm exact)" },
    },
  },
}

function validate(p, ctx)
  local outer = tonumber(p.measured_outer) or 20.04
  local hole = tonumber(p.measured_hole) or 9.84
  if math.abs(20.00 - outer) > 1.0 then
    return false, "Measured outer cube width differs from 20.00mm by more than 1.0mm — check Flow Rate and belt tension before applying X-Y compensation."
  end
  if math.abs(10.00 - hole) > 1.0 then
    return false, "Measured inner bore diameter differs from 10.00mm by more than 1.0mm."
  end
  return true
end

function describe(p, ctx)
  local outer = tonumber(p.measured_outer) or 20.04
  local hole = tonumber(p.measured_hole) or 9.84
  local contour_comp = math.max(-0.5, math.min(0.5, (20.00 - outer) / 2.0))
  local hole_comp = math.max(-0.5, math.min(0.5, (10.00 - hole) / 2.0))
  local badge = string.format("Contour Comp = %+.3f mm (from %.2fmm cube) • Hole Comp = %+.3f mm (from %.2fmm bore)", contour_comp, outer, hole_comp, hole)
  return {
    title = string.format("Calibration: X-Y Contour (%+.2fmm) & Hole (%+.2fmm) Comp", contour_comp, hole_comp),
    description = string.format("Caliper dimensional compensation: 20mm cube measured %.2fmm (xy_contour_compensation = %+.3fmm), 10mm bore measured %.2fmm (xy_hole_compensation = %+.3fmm).", outer, contour_comp, hole, hole_comp),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local outer = tonumber(p and p.measured_outer) or 20.04
  local hole = tonumber(p and p.measured_hole) or 9.84
  local contour_comp = math.max(-0.5, math.min(0.5, (20.00 - outer) / 2.0))
  local hole_comp = math.max(-0.5, math.min(0.5, (10.00 - hole) / 2.0))
  config["xy_contour_compensation"] = string.format("%.3f", contour_comp)
  config["xy_hole_compensation"] = string.format("%.3f", hole_comp)
  local d = describe(p or {}, ctx)
  print("[Calibration:DimensionalXY] " .. (d.badge or ""))
end
