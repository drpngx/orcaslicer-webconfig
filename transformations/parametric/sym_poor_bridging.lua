-- Symptom Fix: Sagging, Drooping or Snapped Horizontal Bridges
meta = {
  id = "sym_poor_bridging",
  name = "Sagging, Drooping or Snapped Horizontal Bridges",
  icon = "🌉",
  category = "6. Overhangs, Bridging & Supports (Bridge Tension & Flow)",
  tree_category = "6. Overhangs, Bridging & Supports",
  tree_subcategory = "Bridge Strand Tension",
  slicer_menu_path = "Relative Bridge Flow Scaling (-5% to -12%) + Relative Bridge Speed Scaling (-20% to -50%) + 100% Bridge Fan",
  when_to_run = "Apply when unsupported horizontal spans sag downward in loops, fail to anchor at the far pillar, or look stringy underneath.",
  formula_summary = "Bridge_Flow_new = Bridge_Flow_old × (1 - Flow_Trim%)  •  Bridge_Speed_new = Bridge_Speed_old × (1 - Speed_Trim%)  •  overhang_fan_speed = 100%",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. A good bridge works by stretching a slightly under-extruded strand (`bridge_flow` scaled by 0.88×–0.95×) at a proportionally reduced speed (`bridge_speed` scaled by 0.50×–0.80×) under 100% cooling airflow so tension pulls it taut like a guitar string.",
    "2. Because both Bridge Flow and Bridge Speed are applied as relative multipliers (`×`) on your current profile, a MicroSwiss High-Flow CHT or wider nozzle keeps its proportional speed advantage instead of being clamped to a slow fixed mm/s.",
  },
  hardware_checklist = {
    "🧭 Check Bridge Angle Orientation: In Slicer Preview, verify that the bridge lines anchor across the SHORTEST distance between support walls rather than diagonally.",
  },
}

params = {
  {
    id = "bridge_flow",
    label = "1. By what percentage should we reduce Bridge Flow Ratio (bridge_flow) relative to current to keep bridged strands taut?",
    description = "-8% (0.92× of current bridge_flow) stretches the extruded strand so it doesn't droop under its own weight.",
    type = "range",
    min = -25,
    max = 0,
    step = 1,
    default = "-8%",
    unit = "%",
    options = {
      { value = "-5%", label = "-5% (0.95× Mild Strand Tension)" },
      { value = "-8%", label = "-8% (0.92× Recommended Strand Tension)" },
      { value = "-12%", label = "-12% (0.88× High Tension for Long Spans)" },
    },
  },
  {
    id = "bridge_spd",
    label = "2. By what percentage should we scale Bridge Print Speed (bridge_speed) relative to your current baseline?",
    description = "Scaling bridge_speed by -35% (0.65× of your current bridge speed) gives the part cooling fan time to freeze the strand in mid-air while scaling naturally with MicroSwiss CHT / high-flow setups.",
    type = "range",
    min = -65,
    max = 0,
    step = 5,
    default = "-35%",
    unit = "%",
    options = {
      { value = "-20%", label = "-20% (0.80× Fast High-Flow CHT / PLA Standard)" },
      { value = "-35%", label = "-35% (0.65× Recommended Proportional Bridge Slowdown)" },
      { value = "-50%", label = "-50% (0.50× Slow Freeze — Sticky PETG/PCTG Long Spans)" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local bf_pct = tonumber(p.bridge_flow) or -8.0
  local bf_scale = 0.92
  if bf_pct < 0 then
    bf_scale = math.max(0.70, math.min(1.0, (100.0 + bf_pct) / 100.0))
  elseif bf_pct <= 1.2 then
    bf_scale = math.max(0.70, math.min(1.0, bf_pct))
  else
    bf_scale = math.max(0.70, math.min(1.0, (100.0 - bf_pct) / 100.0))
  end
  local bf_drop_label = math.floor((1.0 - bf_scale) * 100.0 + 0.5)
  local bs_pct = tonumber(p.bridge_spd) or -35.0
  local bs_scale = 0.65
  if bs_pct < 0 then
    bs_scale = math.max(0.30, math.min(1.0, (100.0 + bs_pct) / 100.0))
  else
    bs_scale = math.max(0.30, math.min(1.0, (100.0 - bs_pct) / 100.0))
  end
  local bs_drop_label = math.floor((1.0 - bs_scale) * 100.0 + 0.5)
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Bridge Flow -%.0f%% (%.2f×) • Bridge Speed -%.0f%% (%.2f×) • Bridge Fan = 100%%", bf_drop_label, bf_scale, bs_drop_label, bs_scale)
  return {
    title = string.format("Symptom Fix: Taut Bridging (Flow -%.0f%%, Speed -%.0f%%, 100%% Fan)", bf_drop_label, bs_drop_label),
    description = string.format("Stops bridge sagging and drooping for %s: scales bridge_flow by %.2f× (-%.0f%%), scales bridge_speed by %.2f× (-%.0f%%), and sets overhang_fan_speed = 100%%.", scope, bf_scale, bf_drop_label, bs_scale, bs_drop_label),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local bf_pct = tonumber(p and p.bridge_flow) or -8.0
  local bf_scale = 0.92
  if bf_pct < 0 then
    bf_scale = math.max(0.70, math.min(1.0, (100.0 + bf_pct) / 100.0))
  elseif bf_pct <= 1.2 then
    bf_scale = math.max(0.70, math.min(1.0, bf_pct))
  else
    bf_scale = math.max(0.70, math.min(1.0, (100.0 - bf_pct) / 100.0))
  end
  local bs_pct = tonumber(p and p.bridge_spd) or -35.0
  local bs_scale = 0.65
  if bs_pct < 0 then
    bs_scale = math.max(0.30, math.min(1.0, (100.0 + bs_pct) / 100.0))
  else
    bs_scale = math.max(0.30, math.min(1.0, (100.0 - bs_pct) / 100.0))
  end

  local cur_bf = tonumber(config["bridge_flow"]) or 1.00
  config["bridge_flow"] = string.format("%.2f", cur_bf * bf_scale)
  local cur_bs = tonumber(config["bridge_speed"]) or 45
  config["bridge_speed"] = tostring(math.max(15, math.floor(cur_bs * bs_scale + 0.5)))
  config["overhang_fan_speed"] = "100"
  local d = describe(p or {}, ctx)
  print("[Symptom:Bridging] " .. (d.badge or ""))
end
