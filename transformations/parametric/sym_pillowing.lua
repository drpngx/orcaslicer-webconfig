-- Symptom Fix: Pillowing (Bumps, Holes or Craters in Top Solid Surface)
meta = {
  id = "sym_pillowing",
  name = "Pillowing (Bumps, Holes or Craters in Top Solid Surface)",
  icon = "🕳️",
  category = "5. Top/Bottom Surfaces & Infill (Top Shell & Bridging)",
  tree_category = "5. Top/Bottom Surfaces & Infill",
  tree_subcategory = "Top Shell Roof & Bridging",
  slicer_menu_path = "Process → Strength → Extra Top Shell Layers (+1 to +3) & Sparse Infill Density Boost (+5%)",
  when_to_run = "Apply when the flat top roof of a print has pillow-like bumps, pinholes, or sagged craters over the sparse infill pockets.",
  formula_summary = "Top_Layers_new = Top_Layers_old + Extra_Layers  •  Top_Thickness += Extra_Layers × Layer_Height  •  Infill_Density += Boost%",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Pillowing happens when the first solid layer bridging across sparse infill sags or curls upward, and there aren't enough subsequent top layers to cover the gaps.",
    "2. Adding +2 extra `top_shell_layers` (and scaling `top_shell_thickness` by +2 × layer_height) + boosting `sparse_infill_density` by +5% gives a flat, hole-free top roof on any layer height.",
  },
  hardware_checklist = {
    "🌀 Verify Part Cooling Fan Duct Airflow: Ensure both outlets of the toolhead part-cooling fan duct are clear of strings and blowing directly below the nozzle tip.",
  },
}

params = {
  {
    id = "top_layers",
    label = "1. How many EXTRA Top Solid Shell Layers (top_shell_layers) should we add?",
    description = "Adding +2 top layers completely bridges and smooths out sparse infill pockets regardless of your layer height.",
    type = "range",
    min = 1,
    max = 8,
    step = 1,
    default = "+2",
    unit = "layers",
    options = {
      { value = "+1", label = "+1 Extra Top Layer" },
      { value = "+2", label = "+2 Extra Top Layers (Recommended)" },
      { value = "+3", label = "+3 Extra Top Layers (Heavy Duty Roof)" },
    },
  },
  {
    id = "infill_pct",
    label = "2. How much should we boost Sparse Infill Density (sparse_infill_density) relative to current to support the top roof?",
    description = "Adding +5% infill density shrinks the span distance that the first top bridge layer has to cross.",
    type = "range",
    min = 0,
    max = 40,
    step = 1,
    default = "+5%",
    unit = "%",
    options = {
      { value = "+3%", label = "+3% Infill Density Boost (Subtle)" },
      { value = "+5%", label = "+5% Infill Density Boost (Recommended)" },
      { value = "+10%", label = "+10% Infill Density Boost (Dense Roof Support)" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local extra_layers = math.floor(tonumber(p.top_layers) or 2)
  local inf_boost = math.floor(tonumber(p.infill_pct) or 5)
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Top Shell %+d Layers • Sparse Infill %+d%%", extra_layers, inf_boost)
  return {
    title = string.format("Symptom Fix: Top Surface Pillowing (%+d Top Layers, %+d%% Infill)", extra_layers, inf_boost),
    description = string.format("Eliminates bumps and holes in top solid surfaces for %s: adds %+d top_shell_layers (and scales top_shell_thickness by layer_height) and boosts sparse_infill_density by %+d%%.", scope, extra_layers, inf_boost),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local extra_layers = math.floor(tonumber(p and p.top_layers) or 2)
  local inf_boost = math.floor(tonumber(p and p.infill_pct) or 5)

  local cur_tl = tonumber(config["top_shell_layers"]) or 3
  local new_tl = math.max(3, math.floor(cur_tl + extra_layers))
  config["top_shell_layers"] = tostring(new_tl)
  local lh = tonumber(config["layer_height"]) or 0.20
  local cur_tt = tonumber(config["top_shell_thickness"]) or (cur_tl * lh)
  config["top_shell_thickness"] = string.format("%.2f", cur_tt + (extra_layers * lh))
  local cur_inf = tonumber(config["sparse_infill_density"]) or 15
  config["sparse_infill_density"] = tostring(math.min(100, math.floor(cur_inf + inf_boost))) .. "%"
  local d = describe(p or {}, ctx)
  print("[Symptom:Pillowing] " .. (d.badge or ""))
end
