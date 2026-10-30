-- Parameterized Lua Transformation: Weather & Ambient Climate Adjustment (Relative & Nozzle-Aware)
-- Stacks relatively (+ / - offsets and × multipliers) on top of prior nozzle, hardware add-on,
-- and filament transformations. Checks `ctx.nozzle_type` / `ctx.nozzle_material` / `ctx.addons`
-- so heat-creep-resistant nozzles (MicroSwiss FlowTech / TwinClad / CM2 / CHT / Diamondback)
-- receive half the hot-weather heat-creep MVS & temperature penalty.

meta = {
  id = "sym_weather_climate",
  name = "Weather & Ambient Climate Adjustment (Tropical / Hot & Dry / Cold & Dry / Cold & Humid)",
  icon = "🌦️",
  category = "Weather & Ambient Climate Rule",
  tree_category = "6. Overhangs, Bridging & Supports",
  tree_subcategory = "6.4 Ambient Weather, Humidity & Chamber Climate",
  slicer_menu_path = "Rig Selection (Weather Dials) & Filament Settings ➔ Cooling / Overrides / Volumetric Speed",
  when_to_run = "Run (or set directly in Rig Selection) when printing in Tropical heat & humidity (e.g. Puerto Rico 28–33°C, 75–85% RH), Hot & Dry summer shops, Cold & Dry winter garages (<18°C / 65°F), or Cold & Humid coastal/AC rooms.",
  formula_summary = "Relative stackable offsets/scales: ΔNozzle °C, ΔBed °C (+5°C 1st layer cold), ×MVS (halved heat-creep penalty on MicroSwiss), ΔRetract mm, ×Fan & +No-Fan Layers, Spiral Z-Hop + Wipe + Avoid Crossing Walls",
  default_condition_expr = "all",
  how_to_read_print = {
    "🌴 Tropical (Hot & Humid, ≥26°C & ≥60% RH — e.g. Puerto Rico): Applies relative anti-heat-creep & anti-steam offsets on top of your active nozzle/filament profile: lowers nozzle temp (-10°C stock / -5°C MicroSwiss), lowers PLA/TPU bed temp (-5°C stock / -3°C MicroSwiss), scales Max Volumetric Speed (0.85× stock / 0.94× MicroSwiss for PLA; 0.78× stock / 0.90× MicroSwiss for Silk/PETG), boosts exhaust fan (+30% up to 90–100% PLA/TPU, 40% PETG), and adds humid anti-stringing (+0.15..+0.35mm retraction, 0.20mm Spiral Z-Hop, Top Surfaces Only for PETG, 0mm Z-Hop for TPU, 1.5mm Wipe While Retracting, Avoid Crossing Walls).",
    "☀️ Hot & Dry (≥26°C & <45% RH — Desert / Summer Shop): Applies relative hot-ambient heat-creep protection (lower nozzle & PLA bed offsets, MVS scaling with MicroSwiss heat-break awareness, boosted exhaust fan) while keeping dry-air retraction (-0.05mm stock / 0.00mm MicroSwiss) so softened plastic isn't pulled into the cold zone.",
    "❄️ Cold & Dry (≤18°C / 65°F & <60% RH — Winter / Garage): Fights thermal shock, corner warping, and cold-filament viscosity with relative boosts: +5..+8°C nozzle (+3..+5°C on high-conductivity MicroSwiss/Diamondback), +5°C bed (+10°C on 1st layer), 0× rear exhaust fan (0% Off), +2..+3 no-cooling initial layers, 0.65× part fan scale, 0.70× first-layer speed, and +5..+10mm Outer Brim or Mouse Ears.",
    "🌧️ Cold & Humid (≤18°C / 65°F & ≥60% RH — Coastal Winter / Heavy AC Room): Stacks Cold-Weather chamber heat retention (+5°C bed / +10°C 1st layer, 0% exhaust fan, +2..+3 no-fan layers, 0.70× 1st-layer speed, Brim) with High-Humidity anti-stringing (+0.15..+0.30mm retraction, 0.20mm Spiral Z-Hop, 1.5mm Wipe, Avoid Crossing Walls, and -4°C anti-steam nozzle trim).",
  },
  hardware_checklist = {
    "🌴 Tropical Enclosure & Pre-Drying: PLA/TPU = Door & Top Lid OPEN (45–50°C 4–6h dry; TPU 55–60°C 8+h on External Spool drybox); Silk/Matte PLA = WIDE OPEN; PETG = VENTED top lid propped (60–65°C 6+h standalone dryer then ACE Pro); ABS/ASA (65–70°C 6h) & PA/PC (70–80°C 12+h) = FULLY CLOSED.",
    "🔬 Nozzle Heat-Creep Check (MicroSwiss vs Stock): MicroSwiss FlowTech / TwinClad / CM2 / CHT / Diamondback hotends isolate the cold zone far better than stock brass/steel, needing only half the MVS and temperature drop in tropical heat.",
    "🧡 ACE Pro Desiccant Boxes & Pre-Print Wipe (75%+ RH): Print silica gel cages for inside the ACE Pro to hold <15–20% RH when heater turns off. Before bed probing, flick away any humid ooze droplet from the nozzle tip so Z-offset calibration isn't skewed high.",
    "❄️ Cold-Room 'Heat-Soak' Trick (<18°C / 65°F): Close the Kobra S1 door & top lid and preheat the bed to 80–100°C for 15–20 minutes (part fan at 50% to circulate warm air) before starting PETG, ABS, ASA, or large PLA jobs.",
  },
}

params = {
  {
    id = "climate_preset",
    label = "1. Weather / Climate Preset",
    description = "Select a weather preset to snap the Ambient Temperature and Relative Humidity dials below, or dial your exact room conditions.",
    type = "enum",
    default = "tropical",
    options = {
      {
        value = "tropical",
        label = "🌴 Tropical — Hot & Humid (30°C / 86°F • 80% RH)",
        sets = { ambient_temp_c = "30", humidity_pct = "80" },
      },
      {
        value = "hot_dry",
        label = "☀️ Hot & Dry — Desert / Summer Shop (33°C / 91°F • 20% RH)",
        sets = { ambient_temp_c = "33", humidity_pct = "20" },
      },
      {
        value = "cold_dry",
        label = "❄️ Cold & Dry — Winter / Garage (14°C / 57°F • 25% RH)",
        sets = { ambient_temp_c = "14", humidity_pct = "25" },
      },
      {
        value = "cold_humid",
        label = "🌧️ Cold & Humid — Coastal Winter / Heavy AC (15°C / 59°F • 80% RH)",
        sets = { ambient_temp_c = "15", humidity_pct = "80" },
      },
      {
        value = "temperate",
        label = "🌤️ Temperate Indoor (22°C / 72°F • 45% RH)",
        sets = { ambient_temp_c = "22", humidity_pct = "45" },
      },
      {
        value = "custom",
        label = "🎛️ Custom Ambient Temp & Humidity Dials",
      },
    },
  },
  {
    id = "ambient_temp_c",
    label = "2. Ambient Temperature Dial (°C)",
    description = "Room or workshop ambient temperature (8°C–40°C / 46°F–104°F). ≤18°C (65°F) activates relative Cold-Weather anti-warping boosts; ≥26°C (79°F) activates relative Hot/Tropical anti-heat-creep offsets.",
    type = "range",
    min = 8,
    max = 40,
    step = 1,
    default = "30",
    unit = "°C",
    options = {
      { value = "10", label = "10°C (50°F Cold Garage)" },
      { value = "14", label = "14°C (57°F Winter Room)" },
      { value = "16", label = "16°C (61°F Heavy AC)" },
      { value = "22", label = "22°C (72°F Temperate)" },
      { value = "28", label = "28°C (82°F Warm Tropical)" },
      { value = "30", label = "30°C (86°F PR Tropical)" },
      { value = "33", label = "33°C (91°F Hot Shop)" },
    },
  },
  {
    id = "humidity_pct",
    label = "3. Relative Humidity Dial (% RH)",
    description = "Ambient relative humidity (10%–95% RH). ≥60% RH stacks relative anti-steam stringing tweaks: +0.15..+0.35mm retraction, 0.20mm Spiral/Slope Z-Hop (Top Only on PETG, 0mm on TPU), 1.5mm Wipe While Retracting, and Avoid Crossing Walls.",
    type = "range",
    min = 10,
    max = 95,
    step = 5,
    default = "80",
    unit = "% RH",
    options = {
      { value = "20", label = "20% RH (Arid / Bone-Dry)" },
      { value = "35", label = "35% RH (Dry Indoor)" },
      { value = "45", label = "45% RH (Ideal Indoor)" },
      { value = "65", label = "65% RH (Humid)" },
      { value = "80", label = "80% RH (Tropical / AC Spike)" },
      { value = "90", label = "90% RH (Rainforest / Storm)" },
    },
  },
  {
    id = "material_family",
    label = "4. Filament Polymer Family",
    description = "Auto-detects PLA, Silk/Matte PLA, PETG/PCTG, TPU (95A), ABS/ASA, or PA/PC from the active filament profile, or lock to a specific polymer.",
    type = "enum",
    default = "auto",
    options = {
      { value = "auto", label = "Auto-Detect Polymer from Active Spool / Profile" },
      { value = "PLA", label = "PLA / PLA+ (Hot: -10°C/-5°C MVS×0.85 • Cold: +6°C/+5°C Bed)" },
      { value = "PLA_SILK", label = "Silk / Matte PLA (Hot: -10°C MVS×0.76 • Cold: +6°C/+5°C Bed)" },
      { value = "PETG", label = "PETG / Rapid PETG / PCTG (Hot: -10°C MVS×0.82 • Cold: +6°C/+5°C)" },
      { value = "TPU", label = "TPU 95A External Spool (Hot: -6°C 0 Z-Hop • Cold: +5°C/+5°C)" },
      { value = "ABS_ASA", label = "ABS / ASA (Hot: -4°C Closed • Cold: +6°C/+5°C + 8mm Brim)" },
      { value = "PA_PC", label = "PA (Nylon) / PC (Hot: -4°C • Cold: +8°C/+5°C + 10mm Brim)" },
    },
  },
  {
    id = "cold_brim_mode",
    label = "5. Cold-Weather Corner Anti-Warp Adhesion",
    description = "When Ambient Temp ≤ 18°C (65°F), sharp 90° corners concentrate shrinkage forces. Select Outer Brim (+5–10mm) or Mouse Ears.",
    type = "enum",
    default = "outer_only",
    options = {
      { value = "outer_only", label = "Outer Brim Only (+5mm PLA/PETG, +8mm ABS/ASA, +10mm PA/PC)" },
      { value = "ears", label = "Mouse Ears (Corner Anti-Warp Discs)" },
      { value = "keep", label = "Keep Existing Brim Setting" },
    },
  },
}

function validate(p, ctx)
  local preset = p.climate_preset or "tropical"
  local temp_c = tonumber(p.ambient_temp_c) or 30
  local rh_pct = tonumber(p.humidity_pct) or 80
  local mat = p.material_family or "auto"

  if temp_c < 8 then
    return false, "Ambient temperature below 8°C (46°F) risks Anycubic Kobra S1 firmware 'Heating Failed / Thermal Runaway' timeouts. Pre-warm the room or chamber above 10°C."
  end
  if temp_c > 38 and (mat == "PLA" or mat == "PLA_SILK") then
    return false, "Ambient temperature above 38°C (100°F) exceeds PLA's cold-end heat-creep threshold and risks extruder gear softening even with the enclosure open."
  end
  if preset == "tropical" and (temp_c < 24 or rh_pct < 55) then
    return false, "Tropical preset expects warm & humid conditions (≥ 24°C and ≥ 55% RH); switch Preset to 'Custom Dials' or another climate preset."
  end
  if preset == "hot_dry" and (temp_c < 24 or rh_pct > 50) then
    return false, "Hot & Dry preset expects ≥ 24°C and ≤ 50% RH; switch Preset to 'Custom Dials' or 'Tropical' for high humidity."
  end
  if preset == "cold_dry" and (temp_c > 20 or rh_pct > 55) then
    return false, "Cold & Dry preset expects ≤ 20°C and ≤ 55% RH; switch Preset to 'Cold & Humid' or 'Custom Dials'."
  end
  if preset == "cold_humid" and (temp_c > 20 or rh_pct < 55) then
    return false, "Cold & Humid preset expects ≤ 20°C and ≥ 55% RH; switch Preset to 'Cold & Dry' or 'Custom Dials'."
  end
  return true
end

function is_heat_creep_resistant_nozzle(ctx)
  if not ctx then
    return false
  end
  local noz = string.lower(tostring(ctx.nozzle_type or "") .. " " .. tostring(ctx.nozzle_material or "") .. " " .. tostring(ctx.hotend or ""))
  if string.find(noz, "microswiss")
    or string.find(noz, "flowtech")
    or string.find(noz, "diamondback")
    or string.find(noz, "twinclad")
    or string.find(noz, "cm2")
    or string.find(noz, "cht")
  then
    return true
  end
  if ctx.addons and type(ctx.addons) == "table" then
    for _, addon in pairs(ctx.addons) do
      local a_low = string.lower(tostring(addon))
      if string.find(a_low, "flowtech") or string.find(a_low, "microswiss") or string.find(a_low, "hollow") then
        return true
      end
    end
  end
  return false
end

function detect_material_family(p, config, ctx)
  local explicit = (p and p.material_family) or "auto"
  if explicit ~= "auto" and explicit ~= "" then
    return explicit
  end

  local raw_mat = ""
  if ctx and ctx.filament_type then
    raw_mat = tostring(ctx.filament_type)
  elseif config and config["filament_type"] then
    local ft = config["filament_type"]
    raw_mat = type(ft) == "table" and tostring(ft[1] or "") or tostring(ft)
  end
  local raw_name = ""
  if config and config["name"] then
    raw_name = tostring(config["name"])
  end
  local finish = (ctx and ctx.finish_type and tostring(ctx.finish_type)) or ""
  local combined = string.upper(raw_mat .. " " .. raw_name .. " " .. finish)

  if string.find(combined, "PETG") or string.find(combined, "PCTG") or string.find(combined, "PET") then
    return "PETG"
  elseif string.find(combined, "TPU") or string.find(combined, "FLEX") or string.find(combined, "PEBA") then
    return "TPU"
  elseif string.find(combined, "ABS") or string.find(combined, "ASA") then
    return "ABS_ASA"
  elseif string.find(combined, "PA") or string.find(combined, "NYLON") or string.find(combined, "PC") or string.find(combined, "PPA") then
    return "PA_PC"
  elseif string.find(combined, "SILK") or string.find(combined, "MATTE") then
    return "PLA_SILK"
  else
    return "PLA"
  end
end

-- Computes RELATIVE deltas & multipliers that stack cleanly on top of any prior nozzle/filament rules:
function compute_relative_climate_deltas(temp_c, rh_pct, fam, is_ms_nozzle, cold_brim_mode)
  local is_hot = temp_c >= 26
  local is_cold = temp_c <= 18
  local is_humid = rh_pct >= 60

  local hot_extra_c = math.max(0, temp_c - 30)
  local cold_extra_c = math.max(0, 14 - temp_c)
  local humid_extra = math.max(0, rh_pct - 80)

  -- MicroSwiss / FlowTech / Diamondback nozzles are much less prone to heat creep in hot weather
  -- and transfer heat more efficiently in cold weather:
  local hc_factor = is_ms_nozzle and 0.50 or 1.00

  local d = {
    enclosure = "Vented / Standard",
    nozzle_delta_c = 0,
    bed_delta_c = 0,
    first_layer_bed_extra_c = 0,
    retract_delta_mm = 0.0,
    retract_speed_scale = 1.00,
    fan_min_delta = 0,
    fan_max_delta = 0,
    fan_scale = 1.00,
    exhaust_target = -1,
    no_fan_layers_delta = 0,
    mvs_scale = 1.00,
    print_speed_scale = 1.00,
    first_layer_speed_scale = 1.00,
    zhop_mm = is_humid and 0.20 or -1.0,
    zhop_type = "Spiral Lift",
    zhop_enforce = "All Surfaces",
    wipe = is_humid,
    wipe_dist = is_humid and (humid_extra >= 10 and "2.0" or "1.5") or "1.0",
    avoid_crossing = is_humid,
    brim_type = "keep",
    brim_width_add = 0,
  }

  -- 1. Humidity Axis (Applies in Tropical, Warm & Humid, AND Cold & Humid)
  if is_humid then
    local extra_r = (humid_extra >= 10) and 0.05 or 0.0
    if fam == "PLA" then
      d.retract_delta_mm = 0.15 + extra_r
    elseif fam == "PLA_SILK" then
      d.retract_delta_mm = 0.20 + extra_r
    elseif fam == "PETG" then
      d.retract_delta_mm = 0.35 + extra_r
      d.zhop_enforce = "Top Surfaces Only"
    elseif fam == "TPU" then
      d.retract_delta_mm = 0.35 + extra_r
      d.retract_speed_scale = 0.75
      d.zhop_mm = 0.0
      d.zhop_type = "Normal Lift"
    elseif fam == "ABS_ASA" then
      d.retract_delta_mm = 0.05 + extra_r
      d.zhop_type = "Slope Lift"
    elseif fam == "PA_PC" then
      d.retract_delta_mm = 0.20 + extra_r
      d.retract_speed_scale = 0.88
      d.zhop_type = "Slope Lift"
    end
  end

  -- 2. Thermal Axis (Hot >= 26°C vs Cold <= 18°C vs Temperate)
  if is_hot then
    local extra_drop = math.floor(hot_extra_c * 0.5 + 0.5)
    if not is_humid and not is_ms_nozzle then
      -- Hot & Dry on stock hotend: slightly shorter retraction to keep softened plastic out of heatbreak
      d.retract_delta_mm = -0.05
    end

    if fam == "PLA" then
      d.enclosure = "Open (Prop top lid & crack door)"
      d.nozzle_delta_c = -math.floor((12 * hc_factor) + extra_drop + (is_humid and 2 or 0) + 0.5)
      d.bed_delta_c = is_ms_nozzle and -3 or -5
      d.fan_min_delta = 20
      d.fan_max_delta = 10
      d.exhaust_target = 90
      d.mvs_scale = is_ms_nozzle and 0.94 or 0.85
    elseif fam == "PLA_SILK" then
      d.enclosure = "Wide Open (Softens easily)"
      d.nozzle_delta_c = -math.floor((10 * hc_factor) + extra_drop + (is_humid and 2 or 0) + 0.5)
      d.bed_delta_c = is_ms_nozzle and -3 or -5
      d.fan_min_delta = 25
      d.fan_max_delta = 15
      d.exhaust_target = 100
      d.mvs_scale = is_ms_nozzle and 0.88 or 0.76
    elseif fam == "PETG" then
      d.enclosure = "Vented (Top lid propped, door closed)"
      d.nozzle_delta_c = -math.floor((10 * hc_factor) + extra_drop + (is_humid and 2 or 0) + 0.5)
      d.bed_delta_c = is_ms_nozzle and -2 or -3
      d.fan_min_delta = 15
      d.fan_max_delta = 20
      d.exhaust_target = 40
      d.mvs_scale = is_ms_nozzle and 0.92 or 0.82
      d.print_speed_scale = is_ms_nozzle and 0.92 or 0.85
    elseif fam == "TPU" then
      d.enclosure = "Open (External Spool Drybox)"
      d.nozzle_delta_c = -math.floor((6 * hc_factor) + extra_drop + (is_humid and 2 or 0) + 0.5)
      d.bed_delta_c = is_ms_nozzle and -3 or -5
      d.fan_min_delta = 20
      d.fan_max_delta = 20
      d.exhaust_target = 80
      d.mvs_scale = is_ms_nozzle and 0.93 or 0.85
      d.print_speed_scale = is_ms_nozzle and 0.92 or 0.85
    elseif fam == "ABS_ASA" then
      d.enclosure = "Fully Closed (Tropical heat helps)"
      d.nozzle_delta_c = -math.floor((4 * hc_factor) + 0.5)
      d.bed_delta_c = 0
      d.exhaust_target = 10
      d.mvs_scale = is_ms_nozzle and 0.97 or 0.92
    elseif fam == "PA_PC" then
      d.enclosure = "Fully Closed"
      d.nozzle_delta_c = -math.floor((4 * hc_factor) + 0.5)
      d.bed_delta_c = 0
      d.exhaust_target = 0
      d.mvs_scale = is_ms_nozzle and 0.97 or 0.92
    end
  elseif is_cold then
    local cold_boost = math.floor(cold_extra_c * 0.5 + 0.5)
    local humid_steam_trim = is_humid and 4 or 0
    local noz_cond_scale = is_ms_nozzle and 0.70 or 1.00

    d.exhaust_target = 0
    d.first_layer_bed_extra_c = 5
    d.first_layer_speed_scale = 0.70
    d.mvs_scale = is_ms_nozzle and 1.00 or 0.95

    if fam == "PLA" then
      d.enclosure = "Fully Closed (Traps 28–32°C bed heat)"
      d.nozzle_delta_c = math.floor((6 * noz_cond_scale) + cold_boost + 0.5) - humid_steam_trim
      d.bed_delta_c = 5
      d.fan_scale = 0.70
      d.no_fan_layers_delta = 2
      d.brim_type = cold_brim_mode
      d.brim_width_add = 5
    elseif fam == "PLA_SILK" then
      d.enclosure = "Fully Closed"
      d.nozzle_delta_c = math.floor((6 * noz_cond_scale) + cold_boost + 0.5) - humid_steam_trim
      d.bed_delta_c = 5
      d.fan_scale = 0.65
      d.no_fan_layers_delta = 2
      d.brim_type = cold_brim_mode
      d.brim_width_add = 5
    elseif fam == "PETG" then
      d.enclosure = "Fully Closed + 15min Bed Heat-Soak"
      d.nozzle_delta_c = math.floor((6 * noz_cond_scale) + cold_boost + 0.5) - humid_steam_trim
      d.bed_delta_c = 5
      d.fan_scale = 0.50
      d.no_fan_layers_delta = 2
      d.brim_type = cold_brim_mode
      d.brim_width_add = 5
    elseif fam == "TPU" then
      d.enclosure = "Fully Closed (Cold stiffens TPU feed)"
      d.nozzle_delta_c = math.floor((5 * noz_cond_scale) + cold_boost + 0.5) - humid_steam_trim
      d.bed_delta_c = 5
      d.fan_scale = 0.55
      d.no_fan_layers_delta = 2
      d.brim_type = "keep"
      d.brim_width_add = 0
    elseif fam == "ABS_ASA" then
      d.enclosure = "Fully Closed + 20min Bed Preheat (105–110°C)"
      d.nozzle_delta_c = math.floor((6 * noz_cond_scale) + cold_boost + 0.5)
      d.bed_delta_c = 5
      d.fan_scale = 0.40
      d.no_fan_layers_delta = 3
      d.brim_type = cold_brim_mode
      d.brim_width_add = 8
    elseif fam == "PA_PC" then
      d.enclosure = "Fully Closed + Insulated + 20min Preheat"
      d.nozzle_delta_c = math.floor((8 * noz_cond_scale) + cold_boost + 0.5)
      d.bed_delta_c = 5
      d.fan_scale = 0.0
      d.no_fan_layers_delta = 3
      d.brim_type = cold_brim_mode
      d.brim_width_add = 10
    end
  else
    -- Temperate (19..25°C): only apply humidity anti-stringing trim if RH >= 60%
    if is_humid then
      d.nozzle_delta_c = -3
    end
  end

  return d
end

function describe(p, ctx)
  local scope = (ctx and ctx.scope) or "all"
  local temp_c = math.floor((tonumber(p.ambient_temp_c) or (ctx and tonumber(ctx.ambient_temp_c)) or 30) + 0.5)
  local temp_f = math.floor(temp_c * 1.8 + 32 + 0.5)
  local rh_pct = math.floor((tonumber(p.humidity_pct) or (ctx and tonumber(ctx.humidity_pct)) or 80) + 0.5)
  local fam_sel = p.material_family or "auto"
  local fam_for_preview = fam_sel == "auto" and "PLA" or fam_sel
  local is_ms = is_heat_creep_resistant_nozzle(ctx)
  local d = compute_relative_climate_deltas(temp_c, rh_pct, fam_for_preview, is_ms, p.cold_brim_mode or "outer_only")

  local regime_lbl = "🌤️ Temperate"
  if temp_c >= 26 and rh_pct >= 60 then
    regime_lbl = "🌴 Tropical (Hot & Humid)"
  elseif temp_c >= 26 then
    regime_lbl = "☀️ Hot & Dry"
  elseif temp_c <= 18 and rh_pct >= 60 then
    regime_lbl = "🌧️ Cold & Humid"
  elseif temp_c <= 18 then
    regime_lbl = "❄️ Cold & Dry"
  elseif rh_pct >= 60 then
    regime_lbl = "💧 Warm & Humid"
  end

  local noz_tag = is_ms and "MicroSwiss/Hi-Cond" or "Standard Nozzle"
  return {
    title = string.format(
      "Weather Fix: %s (%d°C/%d°F, %d%% RH • %+d°C Nozzle, %.2f× MVS)",
      regime_lbl, temp_c, temp_f, rh_pct, d.nozzle_delta_c, d.mvs_scale
    ),
    description = string.format(
      "Relative %s climate transformation for %s (%d°C/%d°F, %d%% RH, %s, %s): Enclosure %s, Nozzle %+d°C, Bed %+d°C (1st layer %+d°C), Retract %+.2fmm, MVS ×%.2f, Exhaust %s%s.",
      regime_lbl,
      scope,
      temp_c,
      temp_f,
      rh_pct,
      fam_sel == "auto" and "Auto-Material" or fam_sel,
      noz_tag,
      d.enclosure,
      d.nozzle_delta_c,
      d.bed_delta_c,
      d.bed_delta_c + d.first_layer_bed_extra_c,
      d.retract_delta_mm,
      d.mvs_scale,
      d.exhaust_target >= 0 and (tostring(d.exhaust_target) .. "%") or "unchanged",
      d.avoid_crossing and ", Spiral Z-Hop 0.20mm + Wipe + Avoid Crossing Walls" or ""
    ),
    badge = string.format(
      "%s (%d°C / %d%% RH • %s) → Nozzle %+d°C, Bed %+d°C, Retract %+.2fmm, MVS ×%.2f, %s",
      regime_lbl,
      temp_c,
      rh_pct,
      noz_tag,
      d.nozzle_delta_c,
      d.bed_delta_c,
      d.retract_delta_mm,
      d.mvs_scale,
      d.enclosure
    ),
  }
end

function get_orca_num(config, key, fallback)
  local v = config[key]
  if v == nil then
    return fallback
  end
  if type(v) == "table" then
    return tonumber(v[1]) or fallback
  end
  return tonumber(v) or fallback
end

function set_orca_val(config, key, val_str)
  if config[key] ~= nil and type(config[key]) == "table" then
    config[key] = { val_str }
  else
    config[key] = val_str
  end
end

function apply(config, ctx, p)
  local temp_c = tonumber(p.ambient_temp_c) or (ctx and tonumber(ctx.ambient_temp_c)) or 30
  local rh_pct = tonumber(p.humidity_pct) or (ctx and tonumber(ctx.humidity_pct)) or 80
  local fam = detect_material_family(p, config, ctx)
  local is_ms = is_heat_creep_resistant_nozzle(ctx)
  local brim_mode = p.cold_brim_mode or "outer_only"
  local d = compute_relative_climate_deltas(temp_c, rh_pct, fam, is_ms, brim_mode)

  -- Default polymer baselines only used if `config` does not yet have the key populated
  local base_noz = (fam == "PA_PC" and 275) or (fam == "ABS_ASA" and 255) or (fam == "PETG" and 240) or (fam == "TPU" and 220) or (fam == "PLA_SILK" and 215) or 212
  local base_bed = (fam == "PA_PC" and 105) or (fam == "ABS_ASA" and 100) or (fam == "PETG" and 80) or (fam == "TPU" and 52) or 60
  local base_retract = (fam == "TPU" and 1.00) or (fam == "PETG" and 0.95) or (fam == "PA_PC" and 0.90) or (fam == "PLA_SILK" and 0.80) or 0.75
  local base_mvs = (fam == "TPU" and 4.8) or (fam == "PA_PC" and 12.0) or (fam == "PETG" and 16.0) or (fam == "PLA_SILK" and 16.5) or (fam == "ABS_ASA" and 18.0) or 20.0
  local base_fan_min = (fam == "PA_PC" and 0) or (fam == "ABS_ASA" and 10) or (fam == "PETG" and 30) or (fam == "TPU" and 60) or 80
  local base_fan_max = (fam == "PA_PC" and 15) or (fam == "ABS_ASA" and 25) or (fam == "PETG" and 50) or 100

  -- 1. Relative Nozzle Temperature Offset (stacked on top of nozzle thermal_conductivity rules)
  if d.nozzle_delta_c ~= 0 or config["nozzle_temperature"] ~= nil then
    local cur_noz = get_orca_num(config, "nozzle_temperature", base_noz)
    local cur_noz_init = get_orca_num(config, "nozzle_temperature_initial_layer", cur_noz)
    local next_noz = math.max(185, math.min(320, math.floor(cur_noz + d.nozzle_delta_c + 0.5)))
    local next_noz_init = math.max(185, math.min(320, math.floor(cur_noz_init + d.nozzle_delta_c + 0.5)))
    set_orca_val(config, "nozzle_temperature", tostring(next_noz))
    set_orca_val(config, "nozzle_temperature_initial_layer", tostring(next_noz_init))
  end

  -- 2. Relative Bed Temperature Offset (+ extra +5°C on first layer in Cold weather)
  local total_init_bed_delta = d.bed_delta_c + d.first_layer_bed_extra_c
  if d.bed_delta_c ~= 0 or total_init_bed_delta ~= 0 or config["textured_plate_temp"] ~= nil then
    for _, pk in ipairs({ "hot_plate_temp", "textured_plate_temp", "cool_plate_temp" }) do
      local cur_b = get_orca_num(config, pk, base_bed)
      local init_k = pk .. "_initial_layer"
      local cur_b_init = get_orca_num(config, init_k, cur_b)
      local next_b = math.max(35, math.min(115, math.floor(cur_b + d.bed_delta_c + 0.5)))
      local next_b_init = math.max(35, math.min(115, math.floor(cur_b_init + total_init_bed_delta + 0.5)))
      set_orca_val(config, pk, tostring(next_b))
      set_orca_val(config, init_k, tostring(next_b_init))
    end
  end

  -- 3. Relative Retraction Length (+Δmm) & Speed (×scale) stacked on top of nozzle retraction
  local max_safe_retract = is_ms and 1.45 or 1.40
  local cur_ret = get_orca_num(config, "filament_retraction_length", base_retract)
  local next_ret = math.max(0.30, math.min(max_safe_retract, cur_ret + d.retract_delta_mm))
  set_orca_val(config, "filament_retraction_length", string.format("%.2f", next_ret))

  local cur_ret_spd = get_orca_num(config, "filament_retraction_speed", 40)
  local next_ret_spd = math.max(20, math.min(55, math.floor(cur_ret_spd * d.retract_speed_scale + 0.5)))
  set_orca_val(config, "filament_retraction_speed", tostring(next_ret_spd))

  -- 4. Z-Hop & Humid Stringing Mitigation (Spiral/Slope Lift, Wipe While Retracting, Avoid Crossing Walls)
  -- Written to both machine/process keys AND per-filament Setting Overrides (`filament_*`) so a single
  -- Process profile works across PLA/PETG (Spiral Lift), ABS/ASA/PA (Slope Lift), and TPU (0.00mm Z-Hop)!
  if d.zhop_mm >= 0.0 then
    local zhop_str = string.format("%.2f", d.zhop_mm)
    set_orca_val(config, "z_hop", zhop_str)
    set_orca_val(config, "filament_z_hop", zhop_str)
    set_orca_val(config, "z_hop_types", d.zhop_type)
    set_orca_val(config, "filament_z_hop_types", d.zhop_type)
    set_orca_val(config, "retract_lift_enforce", d.zhop_enforce)
    set_orca_val(config, "filament_retract_lift_enforce", d.zhop_enforce)
  end
  if d.wipe then
    set_orca_val(config, "wipe", "1")
    set_orca_val(config, "filament_wipe", "1")
    set_orca_val(config, "filament_retract_before_wipe", "70%")
    set_orca_val(config, "filament_wipe_distance", d.wipe_dist)
    config["wipe_on_loops"] = "1"
  end
  if d.avoid_crossing then
    config["reduce_crossing_wall"] = "1"
    config["max_travel_detour_distance"] = "100"
  end

  -- 5. Relative Part Cooling Fan, No-Cooling Initial Layers, and Chamber Exhaust Fan
  local cur_fmin = get_orca_num(config, "fan_min_speed", base_fan_min)
  local cur_fmax = get_orca_num(config, "fan_max_speed", base_fan_max)
  local next_fmin = math.max(0, math.min(100, math.floor((cur_fmin + d.fan_min_delta) * d.fan_scale + 0.5)))
  local next_fmax = math.max(next_fmin, math.min(100, math.floor((cur_fmax + d.fan_max_delta) * d.fan_scale + 0.5)))
  set_orca_val(config, "fan_min_speed", tostring(next_fmin))
  set_orca_val(config, "fan_max_speed", tostring(next_fmax))

  if d.no_fan_layers_delta ~= 0 or config["close_fan_the_first_x_layers"] ~= nil then
    local cur_nfl = get_orca_num(config, "close_fan_the_first_x_layers", 1)
    local next_nfl = math.max(1, math.min(8, math.floor(cur_nfl + d.no_fan_layers_delta + 0.5)))
    set_orca_val(config, "close_fan_the_first_x_layers", tostring(next_nfl))
  end

  if d.exhaust_target >= 0 then
    set_orca_val(config, "additional_cooling_fan_speed", tostring(d.exhaust_target))
    set_orca_val(config, "chamber_fan_speed", tostring(d.exhaust_target))
  end

  -- 6. Relative Max Volumetric Speed Multiplier (×mvs_scale, stacked on CHT/nozzle flow)
  local cur_mvs = get_orca_num(config, "filament_max_volumetric_speed", base_mvs)
  local next_mvs = math.max(2.5, cur_mvs * d.mvs_scale)
  set_orca_val(config, "filament_max_volumetric_speed", string.format("%.1f", next_mvs))

  -- 7. Relative Print Speed & First-Layer Speed Multipliers + Cold Corner Brim
  if math.abs(d.print_speed_scale - 1.0) > 0.001 then
    for _, sk in ipairs({ "outer_wall_speed", "inner_wall_speed", "sparse_infill_speed", "internal_solid_infill_speed" }) do
      if config[sk] ~= nil then
        local cur_s = get_orca_num(config, sk, 150)
        set_orca_val(config, sk, tostring(math.max(20, math.floor(cur_s * d.print_speed_scale + 0.5))))
      end
    end
  end

  if math.abs(d.first_layer_speed_scale - 1.0) > 0.001 then
    local cur_fls = get_orca_num(config, "initial_layer_speed", 50)
    local cur_flis = get_orca_num(config, "initial_layer_infill_speed", 60)
    set_orca_val(config, "initial_layer_speed", tostring(math.max(20, math.floor(cur_fls * d.first_layer_speed_scale + 0.5))))
    set_orca_val(config, "initial_layer_infill_speed", tostring(math.max(25, math.floor(cur_flis * d.first_layer_speed_scale + 0.5))))
  end

  if d.brim_type ~= "keep" and d.brim_width_add > 0 then
    config["brim_type"] = d.brim_type
    local cur_bw = get_orca_num(config, "brim_width", 0)
    config["brim_width"] = tostring(math.max(d.brim_width_add, math.floor(cur_bw + d.brim_width_add + 0.5)))
  end

  print(string.format(
    "[WeatherClimate:%s] Ambient=%dC, RH=%d%%, MicroSwiss=%s -> dNozzle=%+dC, dBed=%+dC (1st=%+dC), dRetract=%+.2fmm, MVS*%.2f (%.1f->%.1f), Exhaust=%d%%, Enclosure=%s",
    fam, temp_c, rh_pct, tostring(is_ms), d.nozzle_delta_c, d.bed_delta_c,
    d.bed_delta_c + d.first_layer_bed_extra_c, d.retract_delta_mm, d.mvs_scale,
    cur_mvs, next_mvs, d.exhaust_target, d.enclosure
  ))
end
