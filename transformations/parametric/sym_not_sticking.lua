-- Symptom Fix: Print Not Sticking to Build Plate (First-Layer Detachment / Spaghetti)
meta = {
  id = "sym_not_sticking",
  name = "Print Not Sticking to Build Plate (First-Layer Detachment / Spaghetti)",
  icon = "🧼",
  category = "3. First Layer & Bed Adhesion (Bed Grip & Z-Offset)",
  tree_category = "3. First Layer & Bed Adhesion",
  tree_subcategory = "Bed Cleaning & First-Layer Squish",
  slicer_menu_path = "Relative Initial Layer Line Width (+20%) + Relative First Layer Speed (-35%) + Initial Layer Temp (+5°C)",
  when_to_run = "Apply when the first layer fails to stick to the Textured PEI plate, lines drag behind the nozzle, or parts detach mid-print.",
  formula_summary = "Layer1_Width_new = Layer1_Width_old × (1 + Width_Boost%)  •  Layer1_Speed_new = Layer1_Speed_old × (1 - Slowdown%)  •  Layer 1 Nozzle/Bed +5°C",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Look at the underside of the first layer: if individual lines look round like spaghetti with gaps between them, the nozzle is too high from the bed or initial layer width/squish is too narrow.",
    "2. Wash the Textured PEI plate with warm water and dish soap first! Then scale First Layer Line Width up by +20% (1.20×), add +5°C bed/nozzle boost, and scale first-layer speed down by -35% (0.65×).",
  },
  hardware_checklist = {
    "🧼 Wash Textured PEI Plate with Warm Dish Soap (Not Just IPA!): Isopropyl alcohol (IPA) only spreads finger oils and PLA/PETG sugar residues thin across PEI. Wash at the sink with warm water + Dawn dish soap and a clean sponge.",
    "🔍 Inspect Nozzle Tip Before Auto-Leveling: If a tiny hardened plastic droplet is stuck to the nozzle tip during LeviQ bed probing, the printer will calibrate Z-offset +0.10mm too high in the air!",
  },
}

params = {
  {
    id = "fl_width",
    label = "1. By what percentage should we widen First Layer Line Width (initial_layer_line_width) to force polymer into the PEI texture?",
    description = "Scaling initial layer line width by +20% (1.20× of your current initial layer width / nozzle diameter) creates higher downward squish pressure on any nozzle size.",
    type = "range",
    min = 0,
    max = 50,
    step = 5,
    default = "+20%",
    unit = "%",
    options = {
      { value = "+10%", label = "+10% (1.10× Mild Squish Boost)" },
      { value = "+20%", label = "+20% (1.20× High-Squish — Recommended)" },
      { value = "+30%", label = "+30% (1.30× Maximum PEI Grip)" },
    },
  },
  {
    id = "fl_speed",
    label = "2. By what percentage should we slow First Layer Print Speed (initial_layer_speed & initial_layer_infill_speed)?",
    description = "Scaling first-layer speed by -35% (0.65× of your current first-layer speed) prevents the nozzle from tearing freshly laid lines off the PEI plate around small holes.",
    type = "range",
    min = -65,
    max = 0,
    step = 5,
    default = "-35%",
    unit = "%",
    options = {
      { value = "-20%", label = "-20% (0.80× Standard Slowdown)" },
      { value = "-35%", label = "-35% (0.65× Recommended PEI Grip)" },
      { value = "-50%", label = "-50% (0.50× Intricate / Small Parts)" },
    },
  },
  {
    id = "fl_temp_boost",
    label = "3. Boost Initial Layer Nozzle & Bed Temperature (°C) relative to current?",
    description = "+5°C on Layer 1 lowers polymer viscosity so it wets the PEI surface.",
    type = "range",
    min = 0,
    max = 15,
    step = 1,
    default = "+5",
    unit = "°C",
    options = {
      { value = "+5", label = "+5°C Nozzle & Bed on Layer 1 (Recommended)" },
      { value = "+10", label = "+10°C Extra Tack" },
      { value = "0", label = "0°C (Keep Current)" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local w_pct = math.abs(tonumber(p.fl_width) or 20.0)
  local w_scale = math.max(1.0, math.min(1.5, (100.0 + w_pct) / 100.0))
  local s_pct = tonumber(p.fl_speed) or -35.0
  local s_scale = 0.65
  if s_pct < 0 then
    s_scale = math.max(0.35, math.min(1.0, (100.0 + s_pct) / 100.0))
  else
    s_scale = math.max(0.35, math.min(1.0, (100.0 - s_pct) / 100.0))
  end
  local s_drop_label = math.floor((1.0 - s_scale) * 100.0 + 0.5)
  local fl_t = math.floor(tonumber(p.fl_temp_boost) or 5)
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Layer 1 Width +%.0f%% (%.2f×) • Speed -%.0f%% (%.2f×) • L1 Temp %+d°C", w_pct, w_scale, s_drop_label, s_scale, fl_t)
  return {
    title = string.format("Symptom Fix: First-Layer Bed Grip (+%.0f%% Width, -%.0f%% Speed, %+d°C)", w_pct, s_drop_label, fl_t),
    description = string.format("Fixes first-layer detachment on Textured PEI for %s: scales initial line width by %.2f× (+%.0f%%), scales speed by %.2f× (-%.0f%%), and adds %+d°C initial layer temp.", scope, w_scale, w_pct, s_scale, s_drop_label, fl_t),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local w_pct = math.abs(tonumber(p and p.fl_width) or 20.0)
  local w_scale = math.max(1.0, math.min(1.5, (100.0 + w_pct) / 100.0))
  local s_pct = tonumber(p and p.fl_speed) or -35.0
  local s_scale = 0.65
  if s_pct < 0 then
    s_scale = math.max(0.35, math.min(1.0, (100.0 + s_pct) / 100.0))
  else
    s_scale = math.max(0.35, math.min(1.0, (100.0 - s_pct) / 100.0))
  end
  local fl_t = math.floor(tonumber(p and p.fl_temp_boost) or 5)

  local noz = tonumber(ctx and ctx.nozzle_diameter) or 0.42
  local cur_w = tonumber(config["initial_layer_line_width"]) or noz
  config["initial_layer_line_width"] = string.format("%.2f", cur_w * w_scale)
  local cur_s = tonumber(config["initial_layer_speed"]) or 45
  config["initial_layer_speed"] = tostring(math.max(15, math.floor(cur_s * s_scale + 0.5)))
  local cur_is = tonumber(config["initial_layer_infill_speed"]) or cur_s
  config["initial_layer_infill_speed"] = tostring(math.max(15, math.floor(cur_is * s_scale + 0.5)))
  local cur_nt = tonumber(config["nozzle_temperature_initial_layer"]) or 230
  config["nozzle_temperature_initial_layer"] = tostring(cur_nt + fl_t)
  local cur_bt = tonumber(config["textured_pei_plate_temp_initial_layer"]) or 65
  config["textured_pei_plate_temp_initial_layer"] = tostring(cur_bt + fl_t)
  local cur_fo = tonumber(config["close_fan_the_first_x_layers"]) or 1
  config["close_fan_the_first_x_layers"] = tostring(math.max(2, cur_fo + 2))
  local d = describe(p or {}, ctx)
  print("[Symptom:FirstLayerStick] " .. (d.badge or ""))
end
