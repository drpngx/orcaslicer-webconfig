-- Dynamic Plate Surface Temperature Schedule (Parent-Preset Relative Deltas + Coating Protection)
-- Preserves each resolved parent system preset's per-plate baseline (`_parent_plate_*` from
-- `AnycubicSlicerNext` / `@orcaslicer_repo`) and applies active bed transformation deltas
-- (`delta_cont`, `delta_init`) relative to the spool's baseline bed temperature without:
--   1. Stamping high-temp PEI targets (e.g. 110°C) onto low-temp coated plates (`cool_plate_temp <= 60°C`,
--      `textured_cool_plate_temp <= 65°C`, `supertack_plate_temp <= 70°C`).
--   2. Clobbering valid vendor low-temp plate baselines (such as AnycubicSlicerNext's `35°C` Cool Plate /
--      `40°C` Textured Cool Plate on Kobra S1) down to `0°C`, while preserving `"0"` where the parent
--      preset itself declares `"0"` (such as `eng_plate_temp = 0` on Kobra S1, `supertack_plate_temp = 0`
--      on TPU/PEBA elastomers, or `cool_plate_temp = 0` on Bambu `fdm_filament_abs`).

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
  tex = clamp(tex, min_bed, max_bed)
  tex_init = clamp(math.max(tex_init, tex), min_bed, max_bed)
  if (ctx and ctx.filament_is_warp_prone == true) and (tex_init - tex > 5) then
    tex_init = math.min(max_bed, tex + 5)
  end

  local ft = string.upper(tostring((ctx and ctx.filament_type) or scalar_num(config["filament_type"], "") or ""))
  if ft == "" and type(config["filament_type"]) == "table" and config["filament_type"][1] then
    ft = string.upper(tostring(config["filament_type"][1]))
  elseif ft == "" and type(config["filament_type"]) == "string" then
    ft = string.upper(config["filament_type"])
  end

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

  local is_high_temp_or_warp = (ctx and ctx.filament_is_warp_prone == true)
    or is_abs_asa
    or (string.find(ft, "PC", 1, true) ~= nil)
    or is_pa_family
    or (string.find(ft, "PET-", 1, true) ~= nil)
    or (string.find(ft, "PET ", 1, true) ~= nil)
    or (string.find(ft, "HIPS", 1, true) ~= nil)
    or (string.find(ft, "PEEK", 1, true) ~= nil)
    or (string.find(ft, "PPS", 1, true) ~= nil)
    or (ft == "PP" or string.find(ft, "PP-", 1, true) ~= nil)
    or (tex >= 85)

  local is_copolyester = (not is_high_temp_or_warp)
    and (not is_elastomer)
    and (
      (string.find(ft, "PETG", 1, true) ~= nil)
      or (string.find(ft, "PCTG", 1, true) ~= nil)
      or (string.find(ft, "CPE", 1, true) ~= nil)
    )

  local is_pla_or_support = (not is_high_temp_or_warp) and (not is_elastomer) and (not is_copolyester)

  local p_cool = scalar_num(ctx and ctx._parent_plate_cool, -1)
  local p_tex_cool = scalar_num(ctx and ctx._parent_plate_tex_cool, -1)
  local p_supertack = scalar_num(ctx and ctx._parent_plate_supertack, -1)
  local p_eng = scalar_num(ctx and ctx._parent_plate_eng, -1)
  local p_hot = scalar_num(ctx and ctx._parent_plate_hot, -1)
  local p_tex = scalar_num(ctx and ctx._parent_plate_tex, -1)

  local ref_base_tex = scalar_num(ctx and ctx.spool_base_bed_c, (p_tex > 0 and p_tex or tex))
  local delta_cont = tex - ref_base_tex
  local delta_init = tex_init - ref_base_tex
  local init_step = math.max(0, tex_init - tex)

  -- 1. Textured PEI (`textured_plate_temp` — canonical reference)
  set_plate_pair(config, "textured_plate_temp", tex, tex_init)

  -- 2. Smooth PEI / High Temp (`hot_plate_temp`)
  local hot = tex
  local hot_init = tex_init
  if p_hot > 0 and p_tex > 0 and p_hot ~= p_tex then
    -- Preserve parent Smooth-vs-Textured offset (e.g. Anycubic ABS 100°C Smooth vs 110°C Textured,
    -- Anycubic PC 120°C High-Temp vs 100°C Textured) and shift by active transformation deltas.
    local hot_offset = p_hot - p_tex
    if is_abs_asa and tex <= 100 and hot_offset < 0 then
      hot_offset = 0
    end
    hot = clamp(tex + hot_offset, min_bed, max_bed)
    hot_init = clamp(hot + init_step, hot, max_bed)
  elseif is_abs_asa and tex > 100 then
    hot = clamp(tex - 10, 100, max_bed)
    hot_init = clamp(hot + init_step, hot, max_bed)
  end
  set_plate_pair(config, "hot_plate_temp", hot, hot_init)

  -- 3. Engineering Plate (`eng_plate_temp`)
  if p_eng == 0 then
    set_plate_pair(config, "eng_plate_temp", 0, 0)
  elseif p_eng > 0 then
    local eng = clamp(p_eng + delta_cont, min_bed, max_bed)
    local eng_init = clamp(math.max(eng, p_eng + delta_init), eng, max_bed)
    set_plate_pair(config, "eng_plate_temp", eng, eng_init)
  elseif is_pla_or_support then
    set_plate_pair(config, "eng_plate_temp", 0, 0)
  else
    set_plate_pair(config, "eng_plate_temp", tex, tex_init)
  end

  -- 4. Cool Plate (`cool_plate_temp` — coating max 60°C)
  if p_cool == 0 then
    set_plate_pair(config, "cool_plate_temp", 0, 0)
  elseif is_high_temp_or_warp then
    if p_cool > 0 then
      local base_c = (p_cool <= 60) and p_cool or 35
      local c_init = clamp(base_c + math.min(5, init_step), base_c, 55)
      set_plate_pair(config, "cool_plate_temp", base_c, c_init)
    else
      set_plate_pair(config, "cool_plate_temp", 0, 0)
    end
  else
    local fallback_c = is_copolyester and clamp(ref_base_tex - 20, 45, 55) or 35
    local base_c = (p_cool > 0 and p_cool <= 60) and p_cool or fallback_c
    local c_cont = clamp(base_c + delta_cont, 30, 60)
    local c_init = clamp(math.max(c_cont, base_c + delta_init), c_cont, 60)
    set_plate_pair(config, "cool_plate_temp", c_cont, c_init)
  end

  -- 5. Textured Cool / CryoGrip (`textured_cool_plate_temp` — coating max 65°C)
  if p_tex_cool == 0 then
    set_plate_pair(config, "textured_cool_plate_temp", 0, 0)
  elseif is_high_temp_or_warp then
    if p_tex_cool > 0 then
      local base_tc = (p_tex_cool <= 65) and p_tex_cool or 40
      local tc_init = clamp(base_tc + math.min(5, init_step), base_tc, 60)
      set_plate_pair(config, "textured_cool_plate_temp", base_tc, tc_init)
    elseif p_cool > 0 and p_cool <= 60 then
      local base_tc = clamp(p_cool + 5, 35, 50)
      set_plate_pair(config, "textured_cool_plate_temp", base_tc, clamp(base_tc + math.min(5, init_step), base_tc, 55))
    else
      set_plate_pair(config, "textured_cool_plate_temp", 0, 0)
    end
  else
    local fallback_tc = is_copolyester and clamp(ref_base_tex - 15, 45, 60) or 40
    local base_tc = (p_tex_cool > 0 and p_tex_cool <= 65) and p_tex_cool or fallback_tc
    local tc_cont = clamp(base_tc + delta_cont, 35, 65)
    local tc_init = clamp(math.max(tc_cont, base_tc + delta_init), tc_cont, 65)
    set_plate_pair(config, "textured_cool_plate_temp", tc_cont, tc_init)
  end

  -- 6. Cool Plate SuperTack (`supertack_plate_temp` — coating max 70°C, always 0 on TPU/PEBA elastomers)
  if is_elastomer or p_supertack == 0 then
    set_plate_pair(config, "supertack_plate_temp", 0, 0)
  elseif is_high_temp_or_warp then
    if p_supertack > 0 and p_supertack <= 60 then
      set_plate_pair(config, "supertack_plate_temp", p_supertack, clamp(p_supertack + math.min(5, init_step), p_supertack, 55))
    elseif p_cool > 0 and p_cool <= 60 then
      set_plate_pair(config, "supertack_plate_temp", 35, clamp(35 + math.min(5, init_step), 35, 50))
    else
      set_plate_pair(config, "supertack_plate_temp", 0, 0)
    end
  else
    local fallback_st = is_copolyester and clamp(ref_base_tex - 10, 55, 65) or 45
    local base_st = (p_supertack > 0 and p_supertack <= 70) and p_supertack or fallback_st
    local st_cont = clamp(base_st + delta_cont, 35, 70)
    local st_init = clamp(math.max(st_cont, base_st + delta_init), st_cont, 70)
    set_plate_pair(config, "supertack_plate_temp", st_cont, st_init)
  end

  -- 7. Synchronize Vitrification Tg, Fan Monotonicity, and AnycubicSlicerNext `_BRASS` / `_HS` Sibling Keys
  local vit_str = fmt_num(math.max(55, tex + 10), "%.0f")
  if type(config["textured_plate_temp"]) == "table" then
    config["temperature_vitrification"] = { vit_str }
  else
    config["temperature_vitrification"] = vit_str
  end
  if ctx and ctx.filament_is_warp_prone == true then
    local fmin = scalar_num(config["fan_min_speed"], 0)
    local fmax = scalar_num(config["fan_max_speed"], 0)
    if fmax > 0 and fmin >= fmax then
      local stepped_min = fmt_num(math.max(0, math.min(math.floor(fmax * 0.5), fmax - 1)), "%.0f")
      config["fan_min_speed"] = type(config["fan_min_speed"]) == "table" and { stepped_min } or stepped_min
    end
  else
    local fmax = scalar_num(config["fan_max_speed"], 0)
    if config["overhang_fan_speed"] ~= nil and scalar_num(config["overhang_fan_speed"], 100) < fmax then
      local ovh_str = fmt_num(fmax, "%.0f")
      config["overhang_fan_speed"] = type(config["overhang_fan_speed"]) == "table" and { ovh_str } or ovh_str
    end
  end

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

  if ctx and ctx.filament_is_warp_prone == true then
    if config["additional_cooling_fan_speed"] ~= nil then
      config["additional_cooling_fan_speed"] = type(config["additional_cooling_fan_speed"]) == "table" and { "0" } or "0"
    end
    if config["chamber_fan_speed"] ~= nil then
      config["chamber_fan_speed"] = type(config["chamber_fan_speed"]) == "table" and { "0" } or "0"
    end
    if config["during_print_exhaust_fan_speed"] ~= nil then
      config["during_print_exhaust_fan_speed"] = type(config["during_print_exhaust_fan_speed"]) == "table" and { "0" } or "0"
    end
    if config["overhang_fan_speed"] ~= nil and scalar_num(config["overhang_fan_speed"], 0) > 35 then
      config["overhang_fan_speed"] = type(config["overhang_fan_speed"]) == "table" and { "35" } or "35"
    end
  end
end
