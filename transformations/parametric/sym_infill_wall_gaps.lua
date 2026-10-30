-- Symptom Fix: Pinhole Gaps Between Top Infill and Outer Perimeter Walls
meta = {
  id = "sym_infill_wall_gaps",
  name = "Pinhole Gaps Between Top Infill and Outer Perimeter Walls",
  icon = "🔍",
  category = "5. Top/Bottom Surfaces & Infill (Infill-to-Wall Overlap)",
  tree_category = "5. Top/Bottom Surfaces & Infill",
  tree_subcategory = "Perimeter-to-Infill Overlap",
  slicer_menu_path = "Process → Strength → Relative Infill/Wall Overlap Boost + Relative Top Surface Speed Scaling + Arachne",
  when_to_run = "Apply when diagonal top surface lines pull away slightly from the inner perimeter wall, leaving a ring of tiny pinholes around holes or edges.",
  formula_summary = "Overlap_new = Overlap_old + Overlap_Boost%  •  wall_generator = arachne  •  Top_Speed_new = Top_Speed_old × (1 - Slowdown%)",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. When the nozzle reverses direction at the end of a diagonal top-infill stroke, elastic melt tension can pull the line slightly away from the perimeter wall if overlap is too small.",
    "2. Boosting `infill_wall_overlap` by +8% relative to current, slowing `top_surface_speed` by -25% (0.75×), and enabling Arachne variable-width walls anchors every infill line firmly into the wall.",
  },
  hardware_checklist = {
    "🎸 Check CoreXY Belt Backlash: If gaps appear only on Two opposite diagonal quadrants of a circle, one of the two CoreXY belts has slack/hysteresis.",
  },
}

params = {
  {
    id = "overlap_pct",
    label = "1. How much should we increase Infill/Wall Overlap percentage (infill_wall_overlap) relative to current?",
    description = "Adding +8% (e.g. 15% → 23%) fuses top/internal infill lines into the inner wall without gaps.",
    type = "range",
    min = 0,
    max = 35,
    step = 1,
    default = "+8%",
    unit = "%",
    options = {
      { value = "+5%", label = "+5% Overlap Boost (Mild)" },
      { value = "+8%", label = "+8% Overlap Boost (Recommended)" },
      { value = "+12%", label = "+12% Overlap Boost (Maximum Perimeter Bond)" },
    },
  },
  {
    id = "top_spd",
    label = "2. By what percentage should we slow Top Surface Speed (top_surface_speed) relative to current?",
    description = "Scaling top surface speed by -25% (0.75× of your current top speed) gives top skin lines time to bond cleanly at the perimeter turnaround, scaling properly with high-flow CHT nozzles.",
    type = "range",
    min = -60,
    max = 0,
    step = 5,
    default = "-25%",
    unit = "%",
    options = {
      { value = "-15%", label = "-15% (0.85× Mild Top Speed Trim)" },
      { value = "-25%", label = "-25% (0.75× Recommended Perimeter Bond)" },
      { value = "-40%", label = "-40% (0.60× Ultra-Precise Top Skin)" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local ov_boost = math.floor(tonumber(p.overlap_pct) or 8)
  local spd_pct = tonumber(p.top_spd) or -25.0
  local spd_scale = 0.75
  if spd_pct < 0 then
    spd_scale = math.max(0.40, math.min(1.0, (100.0 + spd_pct) / 100.0))
  else
    spd_scale = math.max(0.40, math.min(1.0, (100.0 - spd_pct) / 100.0))
  end
  local spd_drop_label = math.floor((1.0 - spd_scale) * 100.0 + 0.5)
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("infill_wall_overlap %+d%% • wall_generator = arachne • Top Speed -%.0f%% (%.2f×)", ov_boost, spd_drop_label, spd_scale)
  return {
    title = string.format("Symptom Fix: Infill-to-Wall Pinholes (%+d%% Overlap, -%.0f%% Top Speed)", ov_boost, spd_drop_label),
    description = string.format("Closes pinhole gaps between infill and walls for %s: boosts infill_wall_overlap by %+d%%, enables arachne walls, and scales top_surface_speed by %.2f× (-%.0f%%).", scope, ov_boost, spd_scale, spd_drop_label),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local ov_boost = math.floor(tonumber(p and p.overlap_pct) or 8)
  local spd_pct = tonumber(p and p.top_spd) or -25.0
  local spd_scale = 0.75
  if spd_pct < 0 then
    spd_scale = math.max(0.40, math.min(1.0, (100.0 + spd_pct) / 100.0))
  else
    spd_scale = math.max(0.40, math.min(1.0, (100.0 - spd_pct) / 100.0))
  end

  local cur_ov = tonumber(config["infill_wall_overlap"]) or 15
  config["infill_wall_overlap"] = tostring(math.min(50, math.floor(cur_ov + ov_boost))) .. "%"
  config["wall_generator"] = "arachne"
  local cur_ts = tonumber(config["top_surface_speed"]) or 80
  config["top_surface_speed"] = tostring(math.max(20, math.floor(cur_ts * spd_scale + 0.5)))
  local d = describe(p or {}, ctx)
  print("[Symptom:InfillWallGaps] " .. (d.badge or ""))
end
