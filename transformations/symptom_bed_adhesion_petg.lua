-- Symptom 3: Poor Bed Adhesion with PETG on Kobra S1
-- Increases first-layer squish and thermal bonding on Textured Gold PEI spring steel plates.
-- Widens initial layer bead relatively by +10% (1.10x, or 0.55mm on 0.4mm nozzle), slows first layer speed by -50% (0.50x),
-- and raises first layer bed temperature by +5°C (up to 85°C for PETG).

function apply(config, ctx)
  if ctx and ctx.filament_type ~= nil then
    local ft = string.upper(tostring(ctx.filament_type))
    if ft ~= "" then
      local is_copolyester = string.find(ft, "PETG", 1, true) ~= nil
        or string.find(ft, "PCTG", 1, true) ~= nil
        or string.find(ft, "CPE", 1, true) ~= nil
        or string.find(ft, "COPE", 1, true) ~= nil
        or string.find(ft, "PET-CF", 1, true) ~= nil
        or string.find(ft, "PET-GF", 1, true) ~= nil
      if not is_copolyester then
        return
      end
    end
  end

  local noz = scalar_num(ctx and ctx.nozzle_diameter, nil)
  local base_w = scalar_num(config["initial_layer_line_width"], noz and (noz * 1.25) or 0.50)
  config["initial_layer_line_width"] = fmt_num(base_w * 1.10, "%.2f")
  if not (ctx and ctx._process_seeded) then
    local base_spd = scalar_num(config["initial_layer_speed"], 50)
    config["initial_layer_speed"] = fmt_num(math.max(15, base_spd * 0.50), "%.0f")
  end

  if config["textured_plate_temp_initial_layer"] ~= nil then
    local tbt = scalar_num(config["textured_plate_temp_initial_layer"], 80) + 5
    local tbt_str = fmt_num(tbt, "%.0f")
    if type(config["textured_plate_temp_initial_layer"]) == "string" then
      config["textured_plate_temp_initial_layer"] = tbt_str
    else
      config["textured_plate_temp_initial_layer"] = { tbt_str }
    end
  end

  if config["hot_plate_temp_initial_layer"] ~= nil then
    local bt = scalar_num(config["hot_plate_temp_initial_layer"], 80)
    local new_bt = bt + 5
    local bt_str = fmt_num(new_bt, "%.0f")
    if type(config["hot_plate_temp_initial_layer"]) == "string" then
      config["hot_plate_temp_initial_layer"] = bt_str
    else
      config["hot_plate_temp_initial_layer"] = { bt_str }
    end
    config["_lua_init_bed_delta_c"] = fmt_num(scalar_num(config["_lua_init_bed_delta_c"], 0) + 5, "%.0f")
    print("[Symptom:BedAdhesion] Initial layer width +10% -> " .. config["initial_layer_line_width"] .. "mm, bed initial temp +5C -> " .. string.format("%.0f", new_bt) .. "C")
  else
    config["hot_plate_temp_initial_layer"] = { "85" }
    config["_lua_init_bed_delta_c"] = fmt_num(scalar_num(config["_lua_init_bed_delta_c"], 0) + 5, "%.0f")
    print("[Symptom:BedAdhesion] Initial layer width +10% -> " .. config["initial_layer_line_width"] .. "mm, bed initial temp -> 85C")
  end
end
