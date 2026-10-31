-- Symptom Fix & Auto-Tuner: Wet / Damp Filament — Drybox Humidity (% RH) Auto-Tuner
-- Automatically computes anti-steam nozzle temperature trim (upper layers only), first-layer pinhole
-- & bed-adhesion recovery (+4..+6% bottom solid infill flow, wider 0.56..0.60mm 1st-layer bead, slower
-- 30..35mm/s 1st-layer speed, +4..+6°C 1st-layer bed temp), upper-layer anti-foam flow trim, and
-- steam-pressure retraction/wipe boost directly from a single Filament / Drybox Humidity (% RH) slider.

meta = {
  id = "sym_wet_filament",
  name = "Wet / Damp Filament — Humidity (% RH) Auto-Tuner (1st-Layer Holes, Mis-Adhesion & Stringing)",
  icon = "💧",
  category = "1. Extrusion & Flow (Moisture & Drying)",
  tree_category = "1. Extrusion & Flow",
  tree_subcategory = "Moisture & Active Drying",
  slicer_menu_path = "Filament Humidity (% RH) Slider ➔ Auto Nozzle Trim + 1st-Layer Pinhole/Adhesion Fix + Anti-Steam Retraction",
  when_to_run = "Enable and set the Filament / Drybox Humidity (% RH) slider (e.g. 40% RH) when your drybox or ACE Pro hygrometer reads >20% RH and you see tiny holes/gaps or slight mis-adhesion on the first layer along with mild stringing.",
  formula_summary = "Auto from % RH (>20% dry baseline): ΔNozzle_upper = -round((RH-20)/6)°C (1st layer stays full temp) • Bed_1st = +4..+6°C • Bottom_Flow = 1.04..1.06 • 1st_Layer_Speed = 30..35mm/s • ΔRetract = +0.10..+0.30mm • ΔFlow_upper = -0.01..-0.03",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Why 40% RH causes 1st-layer holes & mis-adhesion: Inside an ACE Pro or filament drybox, bone-dry filament sits at 10–18% RH. At 35–45% RH, absorbed moisture flashes into microscopic steam bubbles at 240–255°C. Each bubble momentarily breaks the molten bead on Layer 1 (leaving a tiny pinhole/gap) and puffs steam under the line edge so it fails to wet the Textured PEI plate.",
    "2. Why upper layers string mildly at 40% RH: Superheated steam inside the nozzle melt zone acts like a tiny pneumatic spring, pushing a thin whisker of plastic out of the orifice during travel moves even after normal retraction.",
    "3. How the Humidity Slider fixes all three automatically: It lowers upper-layer nozzle temp (-3°C at 40% RH) to calm steam boiling while keeping Layer 1 at full nozzle temp + +4°C hotter bed, slows Layer 1 to 35 mm/s with 1.04× bottom flow to flood and seal steam pinholes, and adds +0.15mm retraction with 45% retract-before-wipe.",
  },
  hardware_checklist = {
    "🔥 Turn On ACE Pro Active Dryer While Printing: Set ACE Pro dryer to 55°C (PETG/PCTG/TPU) or 45°C (PLA) so the spool dries down from 40% RH toward 15% RH over the course of the print.",
    "🧼 Wipe Nozzle Tip Before Auto-Leveling: Damp filament oozes a tiny foam droplet during preheat that can skew the Kobra S1 Z-offset probe slightly high, worsening first-layer gaps.",
  },
}

params = {
  {
    id = "humidity_pct",
    label = "1. Filament / Drybox Humidity (% RH) — Auto-Computes All Adjustments",
    description = "Set your ACE Pro / drybox hygrometer reading (10%–80% RH). ≤20% RH is bone-dry; 40% RH automatically applies mild dampness recovery (-3°C upper nozzle, +4°C 1st-layer bed, 1.04× bottom flow, 35 mm/s 1st layer, -0.01 upper flow, +0.15mm retraction).",
    type = "range",
    min = 10,
    max = 80,
    step = 5,
    default = "40",
    unit = "% RH",
    options = {
      { value = "15", label = "15% RH (Bone-Dry Drybox — No Adjustment)" },
      { value = "30", label = "30% RH (Slightly Damp — -2°C Nozzle, +0.10mm Retract)" },
      { value = "40", label = "40% RH (Wet Drybox / ACE Pro — -3°C, 1st-Layer Hole Fix, +0.15mm)" },
      { value = "55", label = "55% RH (Very Wet Spool — -5°C, 1.05× 1st-Layer Flood, +0.22mm)" },
      { value = "70", label = "70% RH (Saturated Open Air — -8°C, 1.06× 1st-Layer Flood, +0.30mm)" },
    },
  },
  {
    id = "first_layer_recovery",
    label = "2. First-Layer Pinhole & Adhesion Recovery",
    description = "Automatically boosts 1st-layer bed temp (+4..+6°C), widens 1st-layer bead (0.56..0.60mm), boosts bottom solid flow (1.04..1.06×), and slows 1st-layer speed (30..35 mm/s) to seal steam-pop holes.",
    type = "enum",
    default = "auto",
    options = {
      { value = "auto", label = "Auto-Scale from Humidity Slider (Recommended)" },
      { value = "off", label = "Off (Only Adjust Upper-Layer Temp, Flow & Retraction)" },
    },
  },
}

local function compute_moisture_curve(rh_raw)
  local rh = math.max(10, math.min(80, tonumber(rh_raw) or 40))
  if rh <= 20 then
    return {
      rh = rh,
      temp_drop = 0,
      init_bed_boost = 0,
      flow_trim = 0.0,
      bottom_flow = 1.00,
      init_width = 0.55,
      init_speed = 50,
      init_infill_speed = 80,
      retract_add = 0.0,
      retract_before_wipe = 40,
    }
  end
  local excess = rh - 20.0
  local temp_drop = -math.max(1, math.min(10, math.floor((excess / 6.0) + 0.5)))
  local init_bed_boost = math.max(3, math.min(7, math.floor((excess / 5.0) + 0.5)))
  local flow_trim = 0.0
  if rh >= 65 then
    flow_trim = -0.03
  elseif rh >= 50 then
    flow_trim = -0.02
  elseif rh >= 35 then
    flow_trim = -0.01
  end
  local bottom_flow = 1.00 + math.min(0.06, math.max(0.02, math.floor((excess * 0.2) + 0.5) / 100.0))
  local init_width = 0.55
  if rh >= 60 then
    init_width = 0.60
  elseif rh >= 45 then
    init_width = 0.58
  elseif rh >= 30 then
    init_width = 0.56
  end
  local init_speed = 35
  local init_infill_speed = 55
  if rh >= 55 then
    init_speed = 30
    init_infill_speed = 45
  elseif rh < 35 then
    init_speed = 40
    init_infill_speed = 65
  end
  local retract_add = math.min(0.30, math.floor(((excess * 0.006) + 0.03) * 100.0 + 0.5) / 100.0)
  local retract_before_wipe = math.min(60, 40 + math.floor((excess / 4.0) + 0.5))

  return {
    rh = rh,
    temp_drop = temp_drop,
    init_bed_boost = init_bed_boost,
    flow_trim = flow_trim,
    bottom_flow = bottom_flow,
    init_width = init_width,
    init_speed = init_speed,
    init_infill_speed = init_infill_speed,
    retract_add = retract_add,
    retract_before_wipe = retract_before_wipe,
  }
end

function validate(p, ctx)
  local rh = tonumber(p and p.humidity_pct) or 40
  if rh < 10 or rh > 95 then
    return false, "Filament humidity must be between 10% and 95% RH."
  end
  return true
end

function describe(p, ctx)
  local c = compute_moisture_curve(p and p.humidity_pct)
  local fl_mode = (p and p.first_layer_recovery) or "auto"
  local scope = (ctx and ctx.scope) or "All Active Filaments"
  if c.rh <= 20 then
    return {
      title = string.format("Wet Filament Auto-Tuner (%d%% RH — Bone-Dry Baseline)", c.rh),
      description = string.format("Filament humidity is %d%% RH (≤20%% dry threshold) for %s: zero moisture overrides needed.", c.rh, scope),
      badge = string.format("%d%% RH (Bone-Dry • 0 Offsets)", c.rh),
    }
  end
  local fl_tag = (fl_mode ~= "off")
    and string.format(" • 1st Layer: +%d°C Bed, %.2f× Flow, %dmm/s", c.init_bed_boost, c.bottom_flow, c.init_speed)
    or ""
  local badge = string.format(
    "%d%% RH ➔ Nozzle %+d°C (Upper) • Retract +%.2fmm (%d%% Wipe) • Flow %+.2f%s",
    c.rh,
    c.temp_drop,
    c.retract_add,
    c.retract_before_wipe,
    c.flow_trim,
    fl_tag
  )
  return {
    title = string.format("Wet Filament Auto-Tuner (%d%% RH: %+d°C Upper Nozzle, +%.2fmm Retract%s)", c.rh, c.temp_drop, c.retract_add, fl_tag),
    description = string.format(
      "Auto-compensates for %d%% RH damp filament on %s: lowers upper-layer nozzle %+d°C (keeping 1st layer hot), adds +%.2fmm retraction (%d%% pre-wipe, Spiral Z-Hop), trims upper flow %+.2f%s.",
      c.rh,
      scope,
      c.temp_drop,
      c.retract_add,
      c.retract_before_wipe,
      c.flow_trim,
      (fl_mode ~= "off") and string.format(", and seals 1st-layer steam pinholes (+%d°C 1st-layer bed, %.2fmm width @ %dmm/s, %.2f× bottom flow)", c.init_bed_boost, c.init_width, c.init_speed, c.bottom_flow) or ""
    ),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local c = compute_moisture_curve(p and p.humidity_pct)
  local fl_mode = (p and p.first_layer_recovery) or "auto"
  if c.rh <= 20 then
    return
  end

  local cur_t = tonumber(config["nozzle_temperature"]) or 250
  local cur_init_t = tonumber(config["nozzle_temperature_initial_layer"]) or cur_t
  local new_upper_t = tostring(cur_t + c.temp_drop)
  -- Lower upper-layer nozzle temp to stop steam boiling, but preserve full initial layer temp for PEI adhesion!
  config["nozzle_temperature"] = new_upper_t
  config["nozzle_temperature_HS"] = new_upper_t
  config["nozzle_temperature_BRASS"] = tostring((tonumber(config["nozzle_temperature_BRASS"]) or (cur_t - 5)) + c.temp_drop)
  config["nozzle_temperature_initial_layer"] = tostring(cur_init_t)

  local cur_f = tonumber(config["filament_flow_ratio"]) or 0.94
  config["filament_flow_ratio"] = string.format("%.2f", cur_f + c.flow_trim)

  local cur_ret = tonumber(config["filament_retraction_length"]) or 1.00
  config["filament_retraction_length"] = string.format("%.2f", math.min(1.45, cur_ret + c.retract_add))
  config["filament_retract_before_wipe"] = string.format("%d%%", c.retract_before_wipe)
  config["filament_wipe"] = "1"
  config["filament_wipe_distance"] = "2"
  config["filament_z_hop"] = "0.1"
  config["filament_z_hop_types"] = "Spiral Lift"
  config["reduce_crossing_wall"] = "1"

  if fl_mode ~= "off" then
    local cur_bed = tonumber(config["textured_plate_temp"]) or tonumber(config["hot_plate_temp"]) or 85
    local init_bed = tostring(math.min(120, cur_bed + c.init_bed_boost))
    config["textured_plate_temp_initial_layer"] = init_bed
    config["hot_plate_temp_initial_layer"] = init_bed
    config["bottom_solid_infill_flow_ratio"] = string.format("%.2f", c.bottom_flow)
    config["initial_layer_line_width"] = string.format("%.2f", c.init_width)
    config["initial_layer_speed"] = tostring(c.init_speed)
    config["initial_layer_infill_speed"] = tostring(c.init_infill_speed)
  end

  -- Export relative metadata markers so standalone filament exports apply the exact per-spool deltas
  config["_wet_filament_rh"] = tostring(c.rh)
  config["_wet_filament_temp_drop_c"] = tostring(c.temp_drop)
  config["_wet_filament_init_bed_boost_c"] = tostring((fl_mode ~= "off") and c.init_bed_boost or 0)
  config["_wet_filament_flow_trim"] = string.format("%.2f", c.flow_trim)
  config["_wet_filament_retract_add_mm"] = string.format("%.2f", c.retract_add)
  config["_wet_filament_retract_before_wipe_pct"] = tostring(c.retract_before_wipe)

  local d = describe(p or {}, ctx)
  print("[Symptom:WetFilament] " .. (d.badge or ""))
end
