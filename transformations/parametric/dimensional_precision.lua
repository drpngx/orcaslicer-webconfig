-- Core Slicer Config Pack #3: Dimensional Tolerance & Mechanical Fit
-- `wall_sequence` (`inner-outer-inner` for surface finish & overhang support vs. `outer-inner`
-- for exact bearing/screw tolerances), `precise_outer_wall = "1"`, `elefant_foot_compensation`,
-- `xy_hole_compensation`, and `xy_contour_compensation`.

meta = {
  id = "dimensional_precision",
  name = "Dimensional Tolerance & Mechanical Fit Pack",
  icon = "📐",
  category = "Core Slicer Config Pack (Dimensional Fit & Walls)",
  tree_category = "4. Walls, Seams & Dimensional",
  tree_subcategory = "Mechanical Fit & Wall Sequence",
  slicer_menu_path = "Process → Quality → Wall Printing Order (`inner-outer-inner` / `outer-inner`) + Precise Wall (`precise_outer_wall = 1`) + X-Y Hole/Contour & Elephant Foot Compensation",
  when_to_run = "Enable when printing functional brackets, 608/MR85 bearing bores, M3/M5 clearance holes, dovetail joints, or print-in-place mechanisms.",
  formula_summary = "wall_sequence = inner-outer-inner (or outer-inner)  •  precise_outer_wall = 1  •  xy_hole_compensation = +0.08mm  •  xy_contour_compensation = -0.02mm  •  elefant_foot_compensation = 0.15mm",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. In FDM printing, vertical holes shrink inward due to chordal polygon approximation and polymer cooling tension (`xy_hole_compensation = +0.08mm` restores true nominal bore diameter).",
    "2. `inner-outer-inner` prints the middle wall first, then the outer cosmetic wall (before inner infill pressure can push it outward), and finishes with the innermost wall—giving the best balance of dimensional accuracy AND overhang support.",
    "3. `outer-inner` locks the outer perimeter in free space on the very first pass for the tightest possible caliper tolerances on vertical bosses and bearing pockets (best on parts without steep >50° unsupported overhangs).",
  },
  hardware_checklist = {
    "📏 High-Shrinkage Polymers (ABS / ASA / PA / PC): When printing uncompensated CAD models in ABS/ASA/PA, add an extra +0.04mm to `xy_hole_compensation` to offset thermal contraction.",
  },
}

params = {
  {
    id = "fit_preset",
    label = "1. Mechanical Fit & Wall Order Preset (Snaps Dials Below)",
    description = "Choose Balanced Precision + Overhangs (inner-outer-inner), Exact Bearing/Boss Fit (outer-inner), Clearance Slip-Fit / Print-in-Place, or Custom Dials.",
    type = "enum",
    default = "surface_and_overhang",
    options = {
      {
        value = "surface_and_overhang",
        label = "🎯 Balanced Precision & Overhang Safe (inner-outer-inner • +0.08mm Hole • -0.02mm Contour • 0.15mm Elephant)",
        sets = {
          wall_sequence = "inner-outer-inner",
          precise_outer_wall = "1",
          elefant_foot_compensation = "0.15",
          xy_hole_compensation = "0.08",
          xy_contour_compensation = "-0.02",
        },
      },
      {
        value = "mechanical_exact",
        label = "⚙️ Exact Bearing & Boss Fit (outer-inner • +0.08mm Hole • -0.02mm Contour • 0.15mm Elephant)",
        sets = {
          wall_sequence = "outer-inner",
          precise_outer_wall = "1",
          elefant_foot_compensation = "0.15",
          xy_hole_compensation = "0.08",
          xy_contour_compensation = "-0.02",
        },
      },
      {
        value = "clearance_slip_fit",
        label = "🔓 Slip-Fit Pins, Threads & Print-in-Place Hinges (outer-inner • +0.12mm Hole • -0.04mm Contour • 0.18mm Elephant)",
        sets = {
          wall_sequence = "outer-inner",
          precise_outer_wall = "1",
          elefant_foot_compensation = "0.18",
          xy_hole_compensation = "0.12",
          xy_contour_compensation = "-0.04",
        },
      },
      {
        value = "press_fit_tight",
        label = "🔩 Tight Interference / Dowel Press-Fit (inner-outer-inner • +0.04mm Hole • 0.00mm Contour • 0.12mm Elephant)",
        sets = {
          wall_sequence = "inner-outer-inner",
          precise_outer_wall = "1",
          elefant_foot_compensation = "0.12",
          xy_hole_compensation = "0.04",
          xy_contour_compensation = "0.00",
        },
      },
      {
        value = "custom",
        label = "🎛️ Custom Wall Sequence & X-Y Compensation Dials",
      },
    },
  },
  {
    id = "wall_sequence",
    label = "2. Perimeter Wall Printing Sequence (wall_sequence)",
    description = "inner-outer-inner isolates the outer wall from internal infill push-out while still supporting overhangs; outer-inner prints the outer wall first for exact vertical tolerances.",
    type = "enum",
    default = "inner-outer-inner",
    options = {
      { value = "inner-outer-inner", label = "inner-outer-inner — High Surface Quality + Dimensional Fit (Recommended)" },
      { value = "outer-inner", label = "outer-inner — Exact Bearing / Caliper Fit (Vertical Walls)" },
      { value = "inner-outer", label = "inner-outer — Classic Steep Overhang Priority" },
    },
  },
  {
    id = "precise_outer_wall",
    label = "3. Precise Outer Wall Spacing (precise_outer_wall)",
    description = "Recalculates outer-to-inner wall center-line spacing to eliminate the ~0.04mm semicircular overlap bulge.",
    type = "enum",
    default = "1",
    options = {
      { value = "1", label = "Enabled (1) — Exact Outer Wall Centerline Spacing" },
      { value = "0", label = "Disabled (0) — Standard Overlap" },
    },
  },
  {
    id = "xy_hole_compensation",
    label = "4. X-Y Inner Hole Compensation (xy_hole_compensation, mm)",
    description = "Positive values widen internal vertical holes to counteract FDM chordal & thermal bore shrinkage (+0.08mm typical for 0.4mm nozzle).",
    type = "range",
    min = -0.20,
    max = 0.35,
    step = 0.01,
    default = "0.08",
    unit = "mm",
    options = {
      { value = "0.00", label = "0.00 mm (No Hole Compensation)" },
      { value = "0.04", label = "+0.04 mm (Tight Press-Fit)" },
      { value = "0.08", label = "+0.08 mm (Recommended Nominal Bore Fit)" },
      { value = "0.12", label = "+0.12 mm (Free Slip-Fit / Print-in-Place)" },
      { value = "0.16", label = "+0.16 mm (High-Shrinkage ABS / ASA / PA)" },
    },
  },
  {
    id = "xy_contour_compensation",
    label = "5. X-Y Outer Contour Compensation (xy_contour_compensation, mm)",
    description = "Negative values slightly inset outer perimeters so mating tabs, dovetails, and external widths don't print fat (-0.02mm typical).",
    type = "range",
    min = -0.25,
    max = 0.25,
    step = 0.01,
    default = "-0.02",
    unit = "mm",
    options = {
      { value = "-0.06", label = "-0.06 mm (Loose External Clearance)" },
      { value = "-0.04", label = "-0.04 mm (Slip-Fit Tab / Hinge Clearance)" },
      { value = "-0.02", label = "-0.02 mm (Recommended Exact Outer Width)" },
      { value = "0.00", label = "0.00 mm (Nominal CAD Contour)" },
      { value = "0.04", label = "+0.04 mm (Shrinkage Expansion Offset)" },
    },
  },
  {
    id = "elefant_foot_compensation",
    label = "6. First-Layer Elephant's Foot Compensation (elefant_foot_compensation, mm)",
    description = "Insets the first layer perimeter inward so bed-squish doesn't create a flared bottom lip around holes and edges.",
    type = "range",
    min = 0.00,
    max = 0.40,
    step = 0.01,
    default = "0.15",
    unit = "mm",
    options = {
      { value = "0.00", label = "0.00 mm (Disabled)" },
      { value = "0.10", label = "0.10 mm (Light Chamfer)" },
      { value = "0.15", label = "0.15 mm (Recommended Standard)" },
      { value = "0.20", label = "0.20 mm (High-Squish PETG / Textured PEI)" },
    },
  },
}

function validate(p, ctx)
  local hole = tonumber(p and p.xy_hole_compensation) or 0.08
  local contour = tonumber(p and p.xy_contour_compensation) or -0.02
  local ef = tonumber(p and p.elefant_foot_compensation) or 0.15

  if contour < -0.18 and hole > 0.25 then
    return false, "Combining aggressive negative contour inset (< -0.18mm) with large hole expansion (> +0.25mm) can eliminate thin walls between bores and outer edges."
  end
  if ef > 0.35 then
    return false, "Elephant's foot compensation > 0.35mm exceeds a 0.4mm nozzle perimeter width and risks detaching the first layer outer wall."
  end
  return true
end

function describe(p, ctx)
  local seq = (p and p.wall_sequence) or "inner-outer-inner"
  local pow = (p and p.precise_outer_wall) or "1"
  local hole = tonumber(p and p.xy_hole_compensation) or 0.08
  local contour = tonumber(p and p.xy_contour_compensation) or -0.02
  local ef = tonumber(p and p.elefant_foot_compensation) or 0.15
  local pow_lbl = (pow == "1") and "Precise Wall ON" or "Precise Wall OFF"

  local badge = string.format("Walls = %s (%s) • Hole = %+.2fmm • Contour = %+.2fmm • Elephant = %.2fmm", seq, pow_lbl, hole, contour, ef)
  return {
    title = string.format("Dimensional Fit Pack: %s (Hole %+.2fmm, Contour %+.2fmm)", seq, hole, contour),
    description = string.format("Mechanical tolerance tuning: sets wall_sequence = %s, precise_outer_wall = %s, xy_hole_compensation = %+.3fmm, xy_contour_compensation = %+.3fmm, and elefant_foot_compensation = %.2fmm.", seq, pow, hole, contour, ef),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local seq = (p and p.wall_sequence) or "inner-outer-inner"
  local pow = (p and p.precise_outer_wall) or "1"
  local hole = tonumber(p and p.xy_hole_compensation) or 0.08
  local contour = tonumber(p and p.xy_contour_compensation) or -0.02
  local ef = tonumber(p and p.elefant_foot_compensation) or 0.15

  config["wall_sequence"] = seq
  config["precise_outer_wall"] = pow
  config["xy_hole_compensation"] = string.format("%.3f", hole)
  config["xy_contour_compensation"] = string.format("%.3f", contour)
  config["elefant_foot_compensation"] = string.format("%.2f", ef)

  local d = describe(p or {}, ctx)
  print("[Pack:DimensionalPrecision] " .. (d.badge or ""))
end
