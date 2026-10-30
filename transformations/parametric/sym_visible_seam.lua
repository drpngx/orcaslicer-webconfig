-- Symptom Fix: Visible Z-Seam Scar, Raised Ridge or Seam Pinhole Gap
meta = {
  id = "sym_visible_seam",
  name = "Visible Z-Seam Scar, Raised Ridge or Seam Pinhole Gap",
  icon = "🪡",
  category = "4. Walls, Seams & Dimensional (Z-Seam & Scarf Joint)",
  tree_category = "4. Walls, Seams & Dimensional",
  tree_subcategory = "Scarf Joint & Loop Closure",
  slicer_menu_path = "Process → Quality → Seam (Scarf Joint Seam, Relative Seam Gap Offset, Wipe on Loops)",
  when_to_run = "Apply when a vertical zipper scar, raised ridge, or pinhole trench is visible where each outer perimeter loop starts and ends.",
  formula_summary = "Scarf_Length_new = Scarf_Length_old × (1 + Ramp_Boost%)  •  Seam_Gap_new = Seam_Gap_old ± Offset%  •  wipe_on_loops = 1",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Inspect the vertical Z-seam: if it sticks OUT as a raised ridge, Seam Gap is too small (boost seam_gap by +2% to +4%) or Scarf Ramp is too short.",
    "2. If the seam is a sunken groove/pinhole gap, Pressure Advance or Seam Gap is too large (trim seam_gap by -3% to -5%), starving the end of the loop.",
  },
  hardware_checklist = {
    "📐 Calibrate Pressure Advance (PA) First: Even with a Scarf Joint Seam enabled, an uncalibrated PA leaves a bulge (PA too low) or pinhole (PA too high) at the seam ramp start.",
  },
}

params = {
  {
    id = "defect_type",
    label = "1. Does your Z-Seam look like a RAISED ridge/bump or a SUNKEN pinhole gap?",
    description = "Raised ridges widen seam_gap by +2%; sunken gaps tighten seam_gap by -4% relative to current.",
    type = "enum",
    default = "raised",
    unit = "",
    options = {
      { value = "raised", label = "Raised Ridge / Outward Bump (+2% Seam Gap Relief)" },
      { value = "sunken", label = "Sunken Groove / Pinhole Gap (-4% Seam Gap Tightening)" },
    },
  },
  {
    id = "scarf_len",
    label = "2. By what percentage should we lengthen the Scarf Joint Ramp (seam_slope_min_length)?",
    description = "Scaling scarf ramp length by +25% (1.25×) overlaps the start and end of the perimeter at a shallower wedge angle.",
    type = "range",
    min = 0,
    max = 100,
    step = 5,
    default = "+25%",
    unit = "%",
    options = {
      { value = "+0%", label = "+0% (1.00× Keep Current Ramp Length)" },
      { value = "+25%", label = "+25% (1.25× Recommended Smooth Scarf)" },
      { value = "+50%", label = "+50% (1.50× Ultra-Gradual Cylinder Scarf)" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local def_type = p.defect_type or "raised"
  local s_pct = math.abs(tonumber(p.scarf_len) or 25.0)
  local s_scale = math.max(1.0, math.min(2.0, (100.0 + s_pct) / 100.0))
  local gap_delta = 2
  if def_type == "sunken" then gap_delta = -4 end
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Scarf Ramp +%.0f%% (%.2f×, All Walls) • Seam Gap %+d%% (%s)", s_pct, s_scale, gap_delta, def_type)
  return {
    title = string.format("Symptom Fix: Z-Seam Scar (+%.0f%% Scarf Ramp, %+d%% Gap)", s_pct, gap_delta),
    description = string.format("Smooths visible Z-seam (%s) for %s: enables all-wall Scarf Joint (+%.0f%% ramp length), adjusts seam_gap by %+d%%, and sets wipe_on_loops = 1.", def_type, scope, s_pct, gap_delta),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local def_type = (p and p.defect_type) or "raised"
  local s_pct = math.abs(tonumber(p and p.scarf_len) or 25.0)
  local s_scale = math.max(1.0, math.min(2.0, (100.0 + s_pct) / 100.0))
  local gap_delta = 2
  if def_type == "sunken" then gap_delta = -4 end

  config["seam_slope_type"] = "all"
  config["seam_slope_conditional"] = "1"
  local cur_sl = tonumber(config["seam_slope_min_length"]) or 16
  config["seam_slope_min_length"] = tostring(math.max(10, math.floor(cur_sl * s_scale + 0.5)))
  config["seam_slope_steps"] = "15"
  local cur_gap = tonumber(config["seam_gap"]) or 8
  config["seam_gap"] = tostring(math.max(2, math.min(20, math.floor(cur_gap + gap_delta)))) .. "%"
  config["wipe_on_loops"] = "1"
  local d = describe(p or {}, ctx)
  print("[Symptom:VisibleSeam] " .. (d.badge or ""))
end
