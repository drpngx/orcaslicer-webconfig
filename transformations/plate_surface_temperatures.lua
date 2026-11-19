-- Dynamic Plate Surface Temperature Schedule (Parent-Preset Relative Deltas + Coating Protection)
-- Preserves each resolved parent system preset's per-plate baseline (`_parent_plate_*` from
-- `AnycubicSlicerNext` / `@orcaslicer_repo`) and applies active bed transformation deltas
-- (`delta_cont`, `delta_init`) relative to the spool's baseline bed temperature without:
--   1. Stamping high-temp PEI targets (e.g. 110°C) onto low-temp coated plates (`cool_plate_temp <= 60°C`,
--      `textured_cool_plate_temp <= 65°C`, `supertack_plate_temp <= 70°C`).
--   2. Allowing >5°C Layer-1 -> Layer-2 bed temperature drops that contract spring-steel PEI mid-print.
--   3. Leaving unsupported low-temp Cool/SuperTack plates enabled on warp-prone ABS/ASA/PC/PA (`0` = unsupported).
--   4. Overheating TPU on Smooth PEI / Cool coatings (>40°C welds TPU permanently to the sheet).
--   5. Corrupting intrinsic polymer `temperature_vitrification` (Tg) or stalling 24V PWM fans at 1-9%.

local function set_plate_pair(config, key, bed_val, init_val)
  local use_tbl = (type(config[key]) == "table") or (type(config["textured_plate_temp"]) == "table")
  local b_str = fmt_num(bed_val, "%.0f")
  local i_str = fmt_num(init_val, "%.0f")
  if use_tbl then
    config[key] = { b_str }
    config[key .. "_initial_layer"] = { i_str }
  else
    config[key] = b_str
    config[key .. "_initial_layer"] = i_str
  end
end

local function sanitize_pwm_fan(val)
  if val <= 0 then
    return 0
  elseif val < 5 then
    return 0
  elseif val < 10 then
    return 10
  elseif val > 100 then
    return 100
  end
  return val
end

function apply(config, ctx)
  local tex = scalar_num(config["textured_plate_temp"], scalar_num(config["hot_plate_temp"], 60))
  local tex_init = scalar_num(
    config["textured_plate_temp_initial_layer"],
    scalar_num(config["hot_plate_temp_initial_layer"], tex)
  )
  local min_bed = scalar_num(ctx and ctx.filament_bed_min_c, 0)
  local max_bed = scalar_num(
    ctx and ctx.filament_bed_max_c,
    scalar_num(ctx and ctx.max_bed_temp_c, 120)
  )
  local ambient_c = scalar_num(ctx and ctx.ambient_temp_c, 22)

  local ft = string.upper(tostring((ctx and ctx.filament_type) or ""))
  if ft == "" and type(config["filament_type"]) == "table" and config["filament_type"][1] then
    ft = string.upper(tostring(config["filament_type"][1]))
  elseif ft == "" and type(config["filament_type"]) == "string" then
    ft = string.upper(config["filament_type"])
  end
  local fin = string.upper(tostring((ctx and (ctx.filament_finish or ctx.finish_type)) or ""))

  local is_elastomer = (ctx and ctx.filament_is_elastomer == true)
    or (string.find(ft, "TPU", 1, true) ~= nil)
    or (string.find(ft, "TPE", 1, true) ~= nil)
    or (string.find(ft, "PEBA", 1, true) ~= nil)
    or (string.find(ft, "FLEX", 1, true) ~= nil)

  local is_abs_asa = (string.find(ft, "ABS", 1, true) ~= nil and string.find(ft, "PC", 1, true) == nil)
    or (string.find(ft, "ASA", 1, true) ~= nil)

  local is_pa_family = (ft == "PA")
    or (string.find(ft, "PA-", 1, true) ~= nil)
    or (string.find(ft, "PA6", 1, true) ~= nil)
    or (string.find(ft, "PA12", 1, true) ~= nil)
    or (string.find(ft, "PAHT", 1, true) ~= nil)
    or (string.find(ft, "PPA", 1, true) ~= nil)
    or (string.find(ft, "NYLON", 1, true) ~= nil)

  local is_pc_family = (string.find(ft, "PC", 1, true) ~= nil) and (string.find(ft, "PCTG", 1, true) == nil) and (string.find(ft, "PCL", 1, true) == nil)
  local is_pctg = string.find(ft, "PCTG", 1, true) ~= nil
  local is_pha = string.find(ft, "PHA", 1, true) ~= nil
  local is_soluble_support = (string.find(ft, "PVA", 1, true) ~= nil)
    or (string.find(ft, "BVOH", 1, true) ~= nil)
    or (string.find(ft, "SUPPORT", 1, true) ~= nil)

  local is_high_temp_or_warp = (ctx and ctx.filament_is_warp_prone == true)
    or is_abs_asa
    or is_pc_family
    or is_pa_family
    or (string.find(ft, "PET-", 1, true) ~= nil)
    or (string.find(ft, "PET ", 1, true) ~= nil)
    or (string.find(ft, "HIPS", 1, true) ~= nil)
    or (string.find(ft, "PEEK", 1, true) ~= nil)
    or (string.find(ft, "PPS", 1, true) ~= nil)
    or (ft == "PP" or string.find(ft, "PP-", 1, true) ~= nil)
    or (not is_pctg and tex >= 88)

  local is_copolyester = (not is_high_temp_or_warp)
    and (not is_elastomer)
    and (
      (string.find(ft, "PETG", 1, true) ~= nil)
      or is_pctg
      or (string.find(ft, "CPE", 1, true) ~= nil)
    )

  local is_fiber_pla = (string.find(ft, "PLA-CF", 1, true) ~= nil)
    or (string.find(ft, "PLA-GF", 1, true) ~= nil)
    or (string.find(ft, "PLA", 1, true) ~= nil and (string.find(fin, "CARBON", 1, true) ~= nil or string.find(fin, "GLASS", 1, true) ~= nil))

  local is_pla_or_support = (not is_high_temp_or_warp) and (not is_elastomer) and (not is_copolyester)

  local cont_bed_cap = max_bed
  if is_elastomer then
    cont_bed_cap = math.min(max_bed, 45)
    max_bed = math.min(max_bed, 45)
  elseif is_pha then
    cont_bed_cap = math.min(max_bed, 40)
    max_bed = math.min(max_bed, 43)
  elseif is_fiber_pla then
    cont_bed_cap = math.min(max_bed, 65)
    max_bed = math.min(max_bed, 70)
  elseif is_pla_or_support then
    cont_bed_cap = math.min(max_bed, 60)
    max_bed = math.min(max_bed, 65)
  end
  if min_bed > cont_bed_cap then
    min_bed = cont_bed_cap
  end

  tex = clamp(tex, min_bed, cont_bed_cap)
  tex_init = clamp(math.max(tex_init, tex), min_bed, max_bed)
  -- Universal Layer-1 -> Layer-2 thermal step cap (<= 5°C across ALL polymers; <= 3°C on elastomers/PHA)
  -- so cooling the spring-steel PEI sheet on Layer 2 never shears interfacial bed adhesion!
  local max_init_step = (is_elastomer or is_pha) and 3 or 5
  if (tex_init - tex) > max_init_step then
    tex_init = math.min(max_bed, tex + max_init_step)
  end

  local p_cool = scalar_num(ctx and ctx._parent_plate_cool, -1)
  local p_tex_cool = scalar_num(ctx and ctx._parent_plate_tex_cool, -1)
  local p_supertack = scalar_num(ctx and ctx._parent_plate_supertack, -1)
  local p_eng = scalar_num(ctx and ctx._parent_plate_eng, -1)
  local p_hot = scalar_num(ctx and ctx._parent_plate_hot, -1)
  local p_tex = scalar_num(ctx and ctx._parent_plate_tex, -1)

  local ref_base_tex = scalar_num(ctx and ctx.spool_base_bed_c, (p_tex > 0 and p_tex or tex))
  local delta_cont = tex - ref_base_tex
  local init_step = math.max(0, math.min(max_init_step, tex_init - tex))
  local warm_floor = (ambient_c >= 26) and math.min(tex, ambient_c + 5) or 30

  -- 1. Textured PEI (`textured_plate_temp` — canonical reference)
  set_plate_pair(config, "textured_plate_temp", tex, tex_init)

  -- 2. Smooth PEI / High Temp (`hot_plate_temp`)
  local hot = tex
  local hot_init = tex_init
  if is_elastomer then
    hot = clamp(math.min(tex, 40), 30, 40)
    hot_init = clamp(math.min(tex_init, hot + 3), hot, 42)
  elseif is_pc_family then
    hot = clamp(tex, min_bed, math.min(max_bed, 110))
    hot_init = clamp(hot + init_step, hot, math.min(max_bed, 112))
  elseif is_copolyester then
    local smooth_cap = is_pctg and 80 or 75
    hot = clamp(math.min(tex, smooth_cap), min_bed, smooth_cap)
    hot_init = clamp(hot + init_step, hot, smooth_cap + 5)
  elseif p_hot > 0 and p_tex > 0 and p_hot ~= p_tex then
    local hot_offset = p_hot - p_tex
    if is_abs_asa and tex <= 100 and hot_offset < 0 then
      hot_offset = 0
    end
    hot = clamp(tex + hot_offset, min_bed, math.min(max_bed, 112))
    hot_init = clamp(hot + init_step, hot, math.min(max_bed, 115))
  elseif is_abs_asa and tex > 100 then
    hot = clamp(tex - 5, 100, math.min(max_bed, 110))
    hot_init = clamp(hot + init_step, hot, math.min(max_bed, 112))
  end
  set_plate_pair(config, "hot_plate_temp", hot, hot_init)

  -- 3. Engineering Plate (`eng_plate_temp`)
  if p_eng == 0 then
    set_plate_pair(config, "eng_plate_temp", 0, 0)
  elseif p_eng > 0 then
    local eng = clamp(p_eng + delta_cont, min_bed, math.min(max_bed, 112))
    if is_elastomer then
      eng = clamp(math.min(eng, 40), 30, 40)
    elseif is_pla_or_support and eng > tex then
      eng = tex
    end
    local eng_init = clamp(eng + init_step, eng, math.min(max_bed, 115))
    set_plate_pair(config, "eng_plate_temp", eng, eng_init)
  elseif is_pla_or_support then
    set_plate_pair(config, "eng_plate_temp", 0, 0)
  else
    set_plate_pair(config, "eng_plate_temp", hot, hot_init)
  end

  -- 4. Cool Plate (`cool_plate_temp` — coating max 60°C; 0 = unsupported on ABS/ASA/PC/PA/PCTG/PETG-CF)
  if p_cool == 0 or is_high_temp_or_warp or is_pctg then
    set_plate_pair(config, "cool_plate_temp", 0, 0)
  elseif is_elastomer then
    local ec = clamp(math.min(tex, 35), warm_floor, 38)
    set_plate_pair(config, "cool_plate_temp", ec, clamp(ec + math.min(3, init_step), ec, 40))
  else
    local fallback_c = is_copolyester and clamp(ref_base_tex - 25, 45, 52) or 35
    local base_c = (p_cool > 0 and p_cool <= 55) and p_cool or fallback_c
    local c_cont = clamp(math.min(tex, base_c + delta_cont), warm_floor, math.min(tex, 55))
    local c_init = clamp(c_cont + init_step, c_cont, math.min(tex_init, 60))
    set_plate_pair(config, "cool_plate_temp", c_cont, c_init)
  end

  -- 5. Textured Cool / CryoGrip (`textured_cool_plate_temp` — coating max 65°C; 0 on ABS/ASA/PC/PA/PCTG)
  if p_tex_cool == 0 or is_high_temp_or_warp or is_pctg then
    set_plate_pair(config, "textured_cool_plate_temp", 0, 0)
  elseif is_elastomer then
    local etc = clamp(math.min(tex, 38), warm_floor, 40)
    set_plate_pair(config, "textured_cool_plate_temp", etc, clamp(etc + math.min(3, init_step), etc, 42))
  else
    local fallback_tc = is_copolyester and clamp(ref_base_tex - 20, 45, 55) or 40
    local base_tc = (p_tex_cool > 0 and p_tex_cool <= 60) and p_tex_cool or fallback_tc
    local tc_cont = clamp(math.min(tex, base_tc + delta_cont), math.max(35, warm_floor), math.min(tex, 60))
    local tc_init = clamp(tc_cont + init_step, tc_cont, math.min(tex_init, 65))
    set_plate_pair(config, "textured_cool_plate_temp", tc_cont, tc_init)
  end

  -- 6. Cool Plate SuperTack (`supertack_plate_temp` — 0 on elastomers and high-temp warp polymers)
  if is_elastomer or p_supertack == 0 or is_high_temp_or_warp or (p_cool == 0 and string.find(ft, "-CF", 1, true) ~= nil) then
    set_plate_pair(config, "supertack_plate_temp", 0, 0)
  else
    local fallback_st = is_pctg and 65 or (is_copolyester and clamp(ref_base_tex - 10, 55, 65) or 45)
    local base_st = (p_supertack > 0 and p_supertack <= 70) and p_supertack or fallback_st
    local st_cont = clamp(math.min(tex, base_st + delta_cont), math.max(35, warm_floor), math.min(tex, 70))
    local st_init = clamp(st_cont + init_step, st_cont, math.min(tex_init, 70))
    set_plate_pair(config, "supertack_plate_temp", st_cont, st_init)
  end

  -- 7. Intrinsic Polymer `temperature_vitrification` (Tg):
  --    - Enclosed warp-prone polymers (ABS/ASA/PA/PC) MUST stay strictly above `max(tex_init, hot_init)`
  --      so OrcaSlicer NEVER warns the user to open the chamber door during PC/ABS/PA printing!
  --    - Low-Tg PLA/PHA/TPU stay at true polymer Tg (45-60°C) so enclosed printers properly remind
  --      the user to vent the lid/exhaust when printing PLA on a heated bed.
  local vit_c = 55
  if is_pc_family then
    vit_c = math.max(120, math.max(tex_init, hot_init) + 5)
  elseif is_pa_family then
    vit_c = math.max(105, math.max(tex_init, hot_init) + 5)
  elseif is_high_temp_or_warp then
    vit_c = math.max(100, math.max(tex_init, hot_init) + 5)
  elseif is_pctg then
    vit_c = math.max(80, math.max(tex_init, hot_init) + 3)
  elseif is_copolyester then
    vit_c = 70
  elseif is_fiber_pla then
    vit_c = 60
  elseif is_elastomer or is_pha or string.find(ft, "SILK", 1, true) ~= nil or string.find(fin, "SILK", 1, true) ~= nil then
    vit_c = 45
  else
    vit_c = 55
  end
  local vit_str = fmt_num(vit_c, "%.0f")
  if type(config["textured_plate_temp"]) == "table" then
    config["temperature_vitrification"] = { vit_str }
  else
    config["temperature_vitrification"] = vit_str
  end

  -- 8. Fan PWM Stall Elimination (never command 1-9% PWM where 24V blowers stall) & Monotonicity
  local fmin = sanitize_pwm_fan(scalar_num(config["fan_min_speed"], 0))
  local fmax = sanitize_pwm_fan(scalar_num(config["fan_max_speed"], 0))
  if fmin > fmax then
    fmin = fmax
  end
  if ctx and ctx.filament_is_warp_prone == true then
    if fmax > 0 and fmin >= fmax then
      fmin = (fmax >= 18) and sanitize_pwm_fan(math.floor(fmax * 0.5)) or 0
      if fmin >= fmax then
        fmin = 0
      end
    end
  else
    if config["overhang_fan_speed"] ~= nil and scalar_num(config["overhang_fan_speed"], 100) < fmax then
      local ovh_str = fmt_num(fmax, "%.0f")
      config["overhang_fan_speed"] = type(config["overhang_fan_speed"]) == "table" and { ovh_str } or ovh_str
    end
  end
  local fmin_str = fmt_num(fmin, "%.0f")
  local fmax_str = fmt_num(fmax, "%.0f")
  config["fan_min_speed"] = type(config["fan_min_speed"]) == "table" and { fmin_str } or fmin_str
  config["fan_max_speed"] = type(config["fan_max_speed"]) == "table" and { fmax_str } or fmax_str

  -- 9. Chimney-Melter Guard: NEVER lock `dont_slow_down_outer_wall = "1"` on low-fan warp polymers,
  --    elastomers, PHA, or structurally fan-throttled filaments (`fmax <= 70%`) where small towers need
  --    minimum layer-time slowdown to solidify!
  if is_high_temp_or_warp or is_elastomer or is_pha or is_soluble_support or fmax <= 70 then
    if config["dont_slow_down_outer_wall"] ~= nil then
      config["dont_slow_down_outer_wall"] = type(config["dont_slow_down_outer_wall"]) == "table" and { "0" } or "0"
    end
  end

  -- 10. Sync AnycubicSlicerNext `_BRASS` / `_HS` Sibling Keys ONLY on Anycubic printers
  local is_anycubic = (ctx and ctx._is_anycubic_printer == true)
    or (string.find(string.upper(tostring((ctx and ctx.printer_model) or "")), "ANYCUBIC", 1, true) ~= nil)
  if is_anycubic then
    local function sync_siblings(canon_key, sib_a, sib_b, fmt_spec)
      if config[canon_key] ~= nil then
        local n = scalar_num(config[canon_key], nil)
        if n ~= nil then
          local s = fmt_num(n, fmt_spec)
          if type(config[canon_key]) == "table" then
            config[sib_a] = { s }
            config[sib_b] = { s }
          else
            config[sib_a] = s
            config[sib_b] = s
          end
        end
      end
    end
    sync_siblings("nozzle_temperature", "nozzle_temperature_BRASS", "nozzle_temperature_HS", "%.0f")
    sync_siblings(
      "nozzle_temperature_initial_layer",
      "nozzle_temperature_initial_layer_BRASS",
      "nozzle_temperature_initial_layer_HS",
      "%.0f"
    )
    sync_siblings("fan_min_speed", "fan_min_speed_BRASS", "fan_min_speed_HS", "%.0f")
    sync_siblings("fan_max_speed", "fan_max_speed_BRASS", "fan_max_speed_HS", "%.0f")
  else
    config["nozzle_temperature_BRASS"] = nil
    config["nozzle_temperature_HS"] = nil
    config["nozzle_temperature_initial_layer_BRASS"] = nil
    config["nozzle_temperature_initial_layer_HS"] = nil
    config["fan_min_speed_BRASS"] = nil
    config["fan_min_speed_HS"] = nil
    config["fan_max_speed_BRASS"] = nil
    config["fan_max_speed_HS"] = nil
  end

  if is_fiber_pla then
    config["additional_cooling_fan_speed"] = type(config["textured_plate_temp"]) == "table" and { "0" } or "0"
    local cur_nfl = scalar_num(config["close_fan_the_first_x_layers"], 0)
    if cur_nfl < 2 then
      config["close_fan_the_first_x_layers"] = type(config["textured_plate_temp"]) == "table" and { "2" } or "2"
    end
  end

  if ctx and ctx.filament_is_warp_prone == true then
    if config["additional_cooling_fan_speed"] ~= nil then
      config["additional_cooling_fan_speed"] = type(config["additional_cooling_fan_speed"]) == "table" and { "0" } or "0"
    end
    if config["during_print_exhaust_fan_speed"] ~= nil then
      config["during_print_exhaust_fan_speed"] = type(config["during_print_exhaust_fan_speed"]) == "table" and { "0" } or "0"
    end
    if config["overhang_fan_speed"] ~= nil and scalar_num(config["overhang_fan_speed"], 0) > 35 then
      config["overhang_fan_speed"] = type(config["overhang_fan_speed"]) == "table" and { "35" } or "35"
    end
  end
end
