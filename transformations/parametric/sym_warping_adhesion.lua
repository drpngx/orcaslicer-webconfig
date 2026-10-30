-- Symptom Fix: Bed Adhesion, First-Layer Peeling & Corner Warping
meta = {
  id = "sym_warping_adhesion",
  name = "Fix Bed Adhesion, First-Layer Peeling & Corner Warping",
  icon = "🧲",
  category = "Filament Symptom Fix (Bed Adhesion & Warping)",
  tree_category = "3. First Layer & Bed Adhesion",
  tree_subcategory = "Thermal Shrinkage & Brim",
  slicer_menu_path = "Bed Temp Boost + Relative First Layer Speed Scaling + Initial Cooling Fan Delay + Brim",
  when_to_run = "Apply when corners lift off the build plate, first-layer lines don't stick to Textured PEI, or tall parts get knocked loose.",
  formula_summary = "Bed_new = Bed_old + Bed_Boost  •  Layer1_Speed_new = Layer1_Speed_old × (1 - Slowdown%)  •  Fan_Off_Layers += Extra_Layers",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. First-layer peeling on Textured PEI happens when the first layer is printed too fast for the polymer to wet the surface, the bed is 5°C too cool, or the part cooling fan turns on too early.",
    "2. Corner warping on large/boxy parts happens when upper layers cool and contract before the base has locked into the PEI texture.",
  },
  hardware_checklist = {
    "🚪 Close Kobra S1 Door & Top Lid for PETG/PCTG/ASA/ABS: Cold room drafts or AC vents blowing across the print bed cause rapid thermal contraction that peels corners upward.",
    "🧼 Scrub Corners of Textured PEI Plate: We habitually grab the front corners of the spring-steel plate with bare thumbs — scrub the plate edges with dish soap.",
  },
}

params = {
  {
    id = "bed_boost",
    label = "1. How much should we increase the build plate temperature (°C) relative to current?",
    description = "+5°C (e.g. 60→65°C for PLA, 70→75°C for PETG, 75→80°C for PCTG) dramatically improves Textured PEI grip.",
    type = "range",
    min = 0,
    max = 20,
    step = 1,
    default = "+5",
    unit = "°C",
    options = {
      { value = "0", label = "0°C (Keep Current Bed Temp)" },
      { value = "+5", label = "+5°C (Recommended PEI Grip Boost)" },
      { value = "+10", label = "+10°C (Maximum Anti-Warp Tack)" },
    },
  },
  {
    id = "first_layer_spd",
    label = "2. By what percentage should we slow First Layer Print Speed (initial_layer_speed & initial_layer_infill_speed)?",
    description = "Scaling first-layer speed by -30% (0.70× of your current first-layer speed) gives molten plastic time to flow deep into Textured PEI micro-valleys on any nozzle.",
    type = "range",
    min = -65,
    max = 0,
    step = 5,
    default = "-30%",
    unit = "%",
    options = {
      { value = "-20%", label = "-20% (0.80× Moderate First-Layer Slowdown)" },
      { value = "-30%", label = "-30% (0.70× Recommended PEI Grip)" },
      { value = "-45%", label = "-45% (0.55× Ultra-Slow Maximum Tack)" },
    },
  },
  {
    id = "fan_off_layers",
    label = "3. How many EXTRA initial layers should we keep the part cooling fan completely OFF?",
    description = "Adding +2 extra fan-off layers prevents early thermal shrinkage from curling bottom edges on PETG/PCTG/ASA.",
    type = "range",
    min = 0,
    max = 6,
    step = 1,
    default = "+2",
    unit = "layers",
    options = {
      { value = "+0", label = "+0 Layers (Keep Current Fan Delay)" },
      { value = "+1", label = "+1 Extra Layer Fan Off" },
      { value = "+2", label = "+2 Extra Layers Fan Off (Recommended for PETG / PCTG)" },
      { value = "+3", label = "+3 Extra Layers Fan Off (Maximum Anti-Curl)" },
    },
  },
  {
    id = "brim_type",
    label = "4. Which Brim mode (brim_type) should we enable to anchor corners?",
    description = "outer_only adds a clean outer brim around the part footprint.",
    type = "enum",
    default = "outer_only",
    unit = "",
    options = {
      { value = "outer_only", label = "outer_only (Outer Brim — Recommended)" },
      { value = "auto_brim", label = "auto_brim (Slicer Automatic)" },
      { value = "no_brim", label = "no_brim (No Brim)" },
    },
  },
}

function validate(p, ctx)
  local bed_boost = tonumber(p.bed_boost) or 5
  if bed_boost < 0 then
    return false, "Bed adhesion boost (bed_boost) must be >= 0°C."
  end
  return true
end

function describe(p, ctx)
  local bed_boost = math.floor(tonumber(p.bed_boost) or 5)
  local spd_pct = tonumber(p.first_layer_spd) or -30.0
  local spd_scale = 0.70
  if spd_pct < 0 then
    spd_scale = math.max(0.35, math.min(1.0, (100.0 + spd_pct) / 100.0))
  else
    spd_scale = math.max(0.35, math.min(1.0, (100.0 - spd_pct) / 100.0))
  end
  local spd_drop_label = math.floor((1.0 - spd_scale) * 100.0 + 0.5)
  local extra_fan_off = math.floor(tonumber(p.fan_off_layers) or 2)
  local brim = p.brim_type or "outer_only"
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Bed %+d°C • Layer 1 Speed -%.0f%% (%.2f×) • Fan Off %+d Layers • Brim=%s", bed_boost, spd_drop_label, spd_scale, extra_fan_off, brim)
  return {
    title = string.format("Symptom Fix: Bed Adhesion & Anti-Warp (Bed %+d°C, -%.0f%% L1 Speed)", bed_boost, spd_drop_label),
    description = string.format("Eliminates corner warping and first-layer peeling for %s: bed %+d°C, scales first-layer speed by %.2f× (-%.0f%%), adds %+d fan-off layers, and brim_type = %s.", scope, bed_boost, spd_scale, spd_drop_label, extra_fan_off, brim),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local bed_boost = math.floor(tonumber(p and p.bed_boost) or 5)
  local spd_pct = tonumber(p and p.first_layer_spd) or -30.0
  local spd_scale = 0.70
  if spd_pct < 0 then
    spd_scale = math.max(0.35, math.min(1.0, (100.0 + spd_pct) / 100.0))
  else
    spd_scale = math.max(0.35, math.min(1.0, (100.0 - spd_pct) / 100.0))
  end
  local extra_fan_off = math.floor(tonumber(p and p.fan_off_layers) or 2)
  local brim = (p and p.brim_type) or "outer_only"

  local cur_b = tonumber(config["textured_pei_plate_temp"]) or 65
  config["textured_pei_plate_temp"] = tostring(cur_b + bed_boost)
  config["textured_pei_plate_temp_initial_layer"] = tostring(cur_b + bed_boost)
  local cur_fl_s = tonumber(config["initial_layer_speed"]) or 50
  config["initial_layer_speed"] = tostring(math.max(15, math.floor(cur_fl_s * spd_scale + 0.5)))
  local cur_fl_is = tonumber(config["initial_layer_infill_speed"]) or cur_fl_s
  config["initial_layer_infill_speed"] = tostring(math.max(15, math.floor(cur_fl_is * spd_scale + 0.5)))
  local cur_fo = tonumber(config["close_fan_the_first_x_layers"]) or 1
  config["close_fan_the_first_x_layers"] = tostring(math.max(1, math.floor(cur_fo + extra_fan_off)))
  config["brim_type"] = brim
  local d = describe(p or {}, ctx)
  print("[Symptom:Adhesion] " .. (d.badge or ""))
end
