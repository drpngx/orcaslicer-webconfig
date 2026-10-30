-- Symptom Fix: Swollen / Bulging 90° Corners or Rounded Sharp Edges
meta = {
  id = "sym_corner_bulge",
  name = "Swollen / Bulging 90° Corners or Rounded Sharp Edges",
  icon = "📐",
  category = "4. Walls, Seams & Dimensional (Pressure Advance & Jerk)",
  tree_category = "4. Walls, Seams & Dimensional",
  tree_subcategory = "Pressure Advance & Corner Jerk",
  slicer_menu_path = "Relative Pressure Advance Offset + Relative Outer Wall Jerk Scaling",
  when_to_run = "Apply when 90° cube corners bulge outward with thicker plastic than the flat walls, preventing box lids or dovetails from sliding together.",
  formula_summary = "enable_pressure_advance = 1  •  PA_new = PA_old + PA_Offset  •  Outer_Jerk_new = Outer_Jerk_old × (1 ± Jerk_Adj%)",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. As the toolhead decelerates into a 90° corner and accelerates out, molten plastic continues oozing under nozzle pressure unless Pressure Advance (PA) relieves pressure ahead of the corner.",
    "2. Boosting Pressure Advance by +0.006 to +0.012 relative to your nozzle/filament baseline and tuning `outer_wall_jerk` keeps corners crisp and square.",
  },
  hardware_checklist = {
    "📐 Print the PA Pattern Test (`Calibration → Pressure Advance`): Direct visual verification of the 90° chevron corner gives the exact PA value for your nozzle + filament.",
  },
}

params = {
  {
    id = "pa_val",
    label = "1. How much should we adjust Pressure Advance (pressure_advance) relative to your current baseline?",
    description = "Adding +0.008 relieves corner deceleration pressure earlier so 90° corners stop bulging outward (works across Brass, DiamondBack, and CHT nozzles).",
    type = "range",
    min = -0.050,
    max = 0.100,
    step = 0.002,
    default = "+0.008",
    unit = "s",
    options = {
      { value = "+0.004", label = "+0.004 (Subtle Corner Tightening)" },
      { value = "+0.008", label = "+0.008 (Recommended Anti-Bulge Boost)" },
      { value = "+0.014", label = "+0.014 (Strong Boost for High-Elasticity PETG/PCTG/TPU)" },
      { value = "-0.006", label = "-0.006 (Reduce PA if Corners Have Pinhole Gaps)" },
    },
  },
  {
    id = "outer_jerk",
    label = "2. How should we scale Outer Wall Jerk (outer_wall_jerk) relative to current?",
    description = "-15% (0.85×) smooths corner ringing; +15% (1.15×) keeps the nozzle moving briskly through corners with less dwell time.",
    type = "range",
    min = -50,
    max = 50,
    step = 5,
    default = "-15%",
    unit = "%",
    options = {
      { value = "-15%", label = "-15% (0.85× Smooth Corner Turnaround — Recommended)" },
      { value = "-25%", label = "-25% (0.75× Gentle Anti-Ringing Corner)" },
      { value = "+15%", label = "+15% (1.15× Brisk Corner / Less Dwell)" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local pa_delta = math.max(-0.05, math.min(0.10, tonumber(p.pa_val) or 0.008))
  local j_pct = tonumber(p.outer_jerk) or -15.0
  local j_scale = math.max(0.50, math.min(1.50, (100.0 + j_pct) / 100.0))
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("PA %+.3f • Outer Wall Jerk %+.0f%% (%.2f×)", pa_delta, j_pct, j_scale)
  return {
    title = string.format("Symptom Fix: Corner Bulge (PA %+.3f, Jerk %+.0f%%)", pa_delta, j_pct),
    description = string.format("Sharpens 90° corners and eliminates corner swelling for %s: adjusts pressure_advance by %+.3f and scales outer_wall_jerk by %.2f× (%+.0f%%).", scope, pa_delta, j_scale, j_pct),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local pa_delta = math.max(-0.05, math.min(0.10, tonumber(p and p.pa_val) or 0.008))
  local j_pct = tonumber(p and p.outer_jerk) or -15.0
  local j_scale = math.max(0.50, math.min(1.50, (100.0 + j_pct) / 100.0))

  config["enable_pressure_advance"] = "1"
  local cur_pa = tonumber(config["pressure_advance"]) or 0.036
  config["pressure_advance"] = string.format("%.3f", math.max(0.005, cur_pa + pa_delta))
  local cur_j = tonumber(config["outer_wall_jerk"]) or 9
  config["outer_wall_jerk"] = tostring(math.max(4, math.floor(cur_j * j_scale + 0.5)))
  local d = describe(p or {}, ctx)
  print("[Symptom:CornerBulge] " .. (d.badge or ""))
end
