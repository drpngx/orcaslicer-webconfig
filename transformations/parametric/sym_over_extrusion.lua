-- Symptom Fix: Over-Extrusion (Plowed Top Ridges, Rough Surface & Dimensional Bloat)
meta = {
  id = "sym_over_extrusion",
  name = "Over-Extrusion (Plowed Top Ridges, Rough Surface & Dimensional Bloat)",
  icon = "📈",
  category = "1. Extrusion & Flow (Flow & Volume)",
  tree_category = "1. Extrusion & Flow",
  tree_subcategory = "Flow & Volume",
  slicer_menu_path = "Filament Flow Ratio Trim + Relative Top Surface Flow Trim",
  when_to_run = "Apply when the nozzle plows up sharp ridges between solid top lines, corners bulge with excess plastic, or parts fit too tight.",
  formula_summary = "Flow_new = Flow_old + Flow_Drop  •  Top_Flow_new = Top_Flow_old + Top_Flow_Trim",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. If solid infill layers feel like rough sandpaper with raised ridges where the nozzle squeezes excess plastic upward, flow is 2%–5% too high.",
    "2. If walls look great and only the final top skin looks slightly over-stuffed, trim Top Surface Flow Ratio (`top_surface_flow_ratio`) by -0.03 to -0.05 relative to current.",
  },
  hardware_checklist = {
    "📐 Measure Filament Diameter with Calipers: Check 3 points along your spool (should be 1.75mm ± 0.02mm) and run the YOLO Flow Rate chip calibration test if the spool runs thick.",
  },
}

params = {
  {
    id = "flow_drop",
    label = "1. How much should we reduce overall Filament Flow Ratio relative to current?",
    description = "-0.02 (-2%) to -0.03 (-3%) stops plastic from piling up around the nozzle tip.",
    type = "range",
    min = -0.12,
    max = 0.00,
    step = 0.01,
    default = "-0.02",
    unit = "ratio",
    options = {
      { value = "-0.01", label = "-0.01 (-1% Subtle)" },
      { value = "-0.02", label = "-0.02 (-2% Recommended)" },
      { value = "-0.03", label = "-0.03 (-3% Moderate)" },
      { value = "-0.05", label = "-0.05 (-5% Heavy Over-Extrusion)" },
    },
  },
  {
    id = "top_flow",
    label = "2. How much should we trim Top Surface Flow Ratio (top_surface_flow_ratio) relative to current for a glass-smooth top skin?",
    description = "-0.04 (-4% relative to current top_surface_flow_ratio) leaves zero plowed ridges on the final visible top layer.",
    type = "range",
    min = -0.15,
    max = 0.00,
    step = 0.01,
    default = "-0.04",
    unit = "ratio",
    options = {
      { value = "0.00", label = "0.00 (Keep Current Top Surface Flow)" },
      { value = "-0.02", label = "-0.02 (-2% Smooth Top Skin)" },
      { value = "-0.04", label = "-0.04 (-4% Recommended Ultra-Flat Top Skin)" },
      { value = "-0.06", label = "-0.06 (-6% Heavy Top Ridge Fix)" },
    },
  },
}

function validate(p, ctx)
  local fd = tonumber(p.flow_drop) or -0.02
  local tf = tonumber(p.top_flow) or -0.04
  if fd > 0.0 or tf > 0.0 then
    return false, "Over-extrusion flow adjustments must be <= 0.00."
  end
  return true
end

function describe(p, ctx)
  local f_drop = tonumber(p.flow_drop) or -0.02
  local top_f_trim = tonumber(p.top_flow) or -0.04
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Flow %+.2f • Top Surface Flow %+.2f", f_drop, top_f_trim)
  return {
    title = string.format("Symptom Fix: Over-Extrusion (Flow %+.2f, Top Flow %+.2f)", f_drop, top_f_trim),
    description = string.format("Eliminates plowed top ridges and dimensional bloat for %s: reduces filament flow by %+.2f and trims top_surface_flow_ratio by %+.2f.", scope, f_drop, top_f_trim),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local f_drop = tonumber(p and p.flow_drop) or -0.02
  local top_f_trim = tonumber(p and p.top_flow) or -0.04

  local cur_f = tonumber(config["filament_flow_ratio"]) or 0.98
  config["filament_flow_ratio"] = string.format("%.3f", cur_f + f_drop)
  local cur_tf = tonumber(config["top_surface_flow_ratio"]) or 1.00
  config["top_surface_flow_ratio"] = string.format("%.2f", cur_tf + top_f_trim)
  local d = describe(p or {}, ctx)
  print("[Symptom:OverExtrusion] " .. (d.badge or ""))
end
