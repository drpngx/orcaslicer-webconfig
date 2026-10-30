-- Symptom Fix: Rough Waves, Ripples & Tiger-Stripe Furrows on First Layer
meta = {
  id = "sym_first_layer_ripples",
  name = "Rough Waves, Ripples & Tiger-Stripe Furrows on First Layer",
  icon = "🌊",
  category = "3. First Layer & Bed Adhesion (First-Layer Squish)",
  tree_category = "3. First Layer & Bed Adhesion",
  tree_subcategory = "Z-Offset & Plowing Waves",
  slicer_menu_path = "Relative Initial Layer Height Boost (+20%) + Relative First Layer Line Width Trim (-10%)",
  when_to_run = "Apply when a large flat first layer develops diagonal wavy ripples, rough tiger-stripe ridges, or translucent thin spots.",
  formula_summary = "Layer1_Height_new = Layer1_Height_old × (1 + Height_Boost%)  •  Layer1_Width_new = Layer1_Width_old × (1 - Width_Trim%)",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Wavy 'tiger-stripe' ripples on Layer 1 mean the nozzle is slightly TOO CLOSE to the build plate or extruding too much plastic for the gap: excess plastic builds up sideways until the nozzle hops over it in a repeating wave.",
    "2. Scaling Initial Layer Height (`initial_layer_print_height`) up by +20% (1.20×) and trimming initial layer line width by -10% (0.90×) gives the first layer breathing room on any nozzle size.",
  },
  hardware_checklist = {
    "⬆️ Nudge Z-Offset Up +0.02mm to +0.04mm on Touchscreen: Diagonal 'tiger-stripe' waves on Layer 1 mean the nozzle is plowing too close to the plate, forcing excess plastic sideways until it hops over a ridge.",
  },
}

params = {
  {
    id = "fl_height",
    label = "1. By what percentage should we increase Initial Layer Height (initial_layer_print_height) to absorb bed mesh micro-variations?",
    description = "+20% (1.20× of your current initial layer height, e.g. 0.20→0.24mm on 0.4mm nozzle or 0.30→0.36mm on 0.6mm nozzle) absorbs Z-offset tightness.",
    type = "range",
    min = 0,
    max = 50,
    step = 5,
    default = "+20%",
    unit = "%",
    options = {
      { value = "+10%", label = "+10% (1.10× Mild Buffer)" },
      { value = "+20%", label = "+20% (1.20× Recommended Anti-Ripple)" },
      { value = "+30%", label = "+30% (1.30× Thick Buffer Layer)" },
    },
  },
  {
    id = "fl_width",
    label = "2. By what percentage should we trim Initial Layer Line Width (initial_layer_line_width) to stop adjacent lines from crowding?",
    description = "Scaling initial layer line width by -10% (0.90× of current initial layer width) stops adjacent lines from snow-plowing into each other.",
    type = "range",
    min = -25,
    max = 0,
    step = 5,
    default = "-10%",
    unit = "%",
    options = {
      { value = "-5%", label = "-5% (0.95× Subtle Trim)" },
      { value = "-10%", label = "-10% (0.90× Recommended Anti-Crowding)" },
      { value = "-15%", label = "-15% (0.85× Strong De-Crowding)" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local h_pct = math.abs(tonumber(p.fl_height) or 20.0)
  local h_scale = math.max(1.0, math.min(1.5, (100.0 + h_pct) / 100.0))
  local w_pct = tonumber(p.fl_width) or -10.0
  local w_scale = 0.90
  if w_pct < 0 then
    w_scale = math.max(0.75, math.min(1.0, (100.0 + w_pct) / 100.0))
  else
    w_scale = math.max(0.75, math.min(1.0, (100.0 - w_pct) / 100.0))
  end
  local w_drop_label = math.floor((1.0 - w_scale) * 100.0 + 0.5)
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Initial Layer Height +%.0f%% (%.2f×) • Initial Line Width -%.0f%% (%.2f×)", h_pct, h_scale, w_drop_label, w_scale)
  return {
    title = string.format("Symptom Fix: First-Layer Ripples (+%.0f%% Height, -%.0f%% Width)", h_pct, w_drop_label),
    description = string.format("Stops wavy plowing ripples on Layer 1 for %s: scales initial_layer_print_height by %.2f× (+%.0f%%) and initial_layer_line_width by %.2f× (-%.0f%%).", scope, h_scale, h_pct, w_scale, w_drop_label),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local h_pct = math.abs(tonumber(p and p.fl_height) or 20.0)
  local h_scale = math.max(1.0, math.min(1.5, (100.0 + h_pct) / 100.0))
  local w_pct = tonumber(p and p.fl_width) or -10.0
  local w_scale = 0.90
  if w_pct < 0 then
    w_scale = math.max(0.75, math.min(1.0, (100.0 + w_pct) / 100.0))
  else
    w_scale = math.max(0.75, math.min(1.0, (100.0 - w_pct) / 100.0))
  end

  local cur_lh = tonumber(config["initial_layer_print_height"]) or tonumber(config["layer_height"]) or 0.20
  config["initial_layer_print_height"] = string.format("%.2f", cur_lh * h_scale)
  local cur_w = tonumber(config["initial_layer_line_width"]) or 0.45
  config["initial_layer_line_width"] = string.format("%.2f", cur_w * w_scale)
  local d = describe(p or {}, ctx)
  print("[Symptom:FirstLayerRipples] " .. (d.badge or ""))
end
