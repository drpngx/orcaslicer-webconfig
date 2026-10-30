-- Symptom Fix: Weak, Snapping, Stringy or Under-Extruded Internal Infill
meta = {
  id = "sym_weak_infill",
  name = "Weak, Snapping, Stringy or Under-Extruded Internal Infill",
  icon = "🕸️",
  category = "5. Top/Bottom Surfaces & Infill (Infill Pattern & Speed)",
  tree_category = "5. Top/Bottom Surfaces & Infill",
  tree_subcategory = "Non-Crossing Infill & Flow",
  slicer_menu_path = "Sparse Infill Pattern (Gyroid/Crosshatch) + Relative Infill Speed Scaling + Relative Infill Width Boost",
  when_to_run = "Apply when internal sparse infill looks like broken cobwebs, snaps with a crunching sound while printing, or leaves weak hollow voids.",
  formula_summary = "sparse_infill_pattern = gyroid/crosshatch  •  Infill_Speed_new = Infill_Speed_old × (1 - Slowdown%)  •  Infill_Width × 1.12",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Broken/stringy infill happens when (a) `sparse_infill_speed` exceeds the filament's melt rate, OR (b) Grid/Triangles infill collides with itself at every intersection.",
    "2. Switching to `gyroid` or `crosshatch`, scaling `sparse_infill_line_width` up by +12% (1.12×), and scaling `sparse_infill_speed` down by -25% (0.75×) produces rock-solid internal structure on any nozzle or hotend.",
  },
  hardware_checklist = {
    "🔧 Check Kobra S1 Extruder Idler Tension: If sparse infill looks like thin cobwebs while slow outer walls look fine, the extruder gears may be slipping at high volumetric flow — turn the right-side extruder tension screw 1/4 turn clockwise.",
  },
}

params = {
  {
    id = "pattern",
    label = "1. Which Non-Crossing Infill Pattern should we use?",
    description = "Gyroid distributes stress in 3D without ever crossing on the same layer; Crosshatch is fast and non-crossing.",
    type = "enum",
    default = "gyroid",
    unit = "",
    options = {
      { value = "gyroid", label = "gyroid (Maximum 3D Isotropic Strength)" },
      { value = "crosshatch", label = "crosshatch (Fast Non-Crossing)" },
      { value = "honeycomb", label = "honeycomb (High Compression Strength)" },
    },
  },
  {
    id = "infill_spd",
    label = "2. By what percentage should we scale down Sparse Infill Speed (sparse_infill_speed) relative to current?",
    description = "Scaling sparse_infill_speed by -25% (0.75× of your current infill speed) ensures every infill line fuses completely while preserving high-flow CHT speed advantages.",
    type = "range",
    min = -60,
    max = 0,
    step = 5,
    default = "-25%",
    unit = "%",
    options = {
      { value = "-15%", label = "-15% (0.85× Mild Infill Slowdown)" },
      { value = "-25%", label = "-25% (0.75× Recommended Structural Fusion)" },
      { value = "-35%", label = "-35% (0.65× Maximum Heavy-Duty Infill Bond)" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local pat = p.pattern or "gyroid"
  local spd_pct = tonumber(p.infill_spd) or -25.0
  local spd_scale = 0.75
  if spd_pct < 0 then
    spd_scale = math.max(0.40, math.min(1.0, (100.0 + spd_pct) / 100.0))
  else
    spd_scale = math.max(0.40, math.min(1.0, (100.0 - spd_pct) / 100.0))
  end
  local spd_drop_label = math.floor((1.0 - spd_scale) * 100.0 + 0.5)
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Infill = %s • Speed -%.0f%% (%.2f×) • Width +12%% (1.12×)", pat, spd_drop_label, spd_scale)
  return {
    title = string.format("Symptom Fix: Strong Non-Crossing Infill (%s, -%.0f%% Speed)", pat, spd_drop_label),
    description = string.format("Eliminates weak/snapping internal infill for %s: switches to %s pattern, scales sparse_infill_speed by %.2f× (-%.0f%%), and widens sparse_infill_line_width by 1.12×.", scope, pat, spd_scale, spd_drop_label),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local pat = (p and p.pattern) or "gyroid"
  local spd_pct = tonumber(p and p.infill_spd) or -25.0
  local spd_scale = 0.75
  if spd_pct < 0 then
    spd_scale = math.max(0.40, math.min(1.0, (100.0 + spd_pct) / 100.0))
  else
    spd_scale = math.max(0.40, math.min(1.0, (100.0 - spd_pct) / 100.0))
  end

  config["sparse_infill_pattern"] = pat
  local cur_s = tonumber(config["sparse_infill_speed"]) or 240
  config["sparse_infill_speed"] = tostring(math.max(40, math.floor(cur_s * spd_scale + 0.5)))
  local noz = tonumber(ctx and ctx.nozzle_diameter) or 0.40
  local cur_w = tonumber(config["sparse_infill_line_width"]) or noz
  config["sparse_infill_line_width"] = string.format("%.2f", cur_w * 1.12)
  local d = describe(p or {}, ctx)
  print("[Symptom:WeakInfill] " .. (d.badge or ""))
end
