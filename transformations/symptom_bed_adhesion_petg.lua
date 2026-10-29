-- Symptom 3: Poor Bed Adhesion with PETG on Kobra S1
-- Increases first-layer squish and thermal bonding on Textured Gold PEI spring steel plates.
-- Widens initial layer bead relatively by +10% (1.10x, or 0.55mm on 0.4mm nozzle), slows first layer speed by -50% (0.50x),
-- and raises first layer bed temperature by +5°C (up to 85°C for PETG).

function apply(config, ctx)
  local base_w = tonumber(config["initial_layer_line_width"]) or (tonumber(ctx.nozzle_diameter) and (tonumber(ctx.nozzle_diameter) * 1.25) or 0.50)
  config["initial_layer_line_width"] = string.format("%.2f", base_w * 1.10)
  local base_spd = tonumber(config["initial_layer_speed"]) or 50
  config["initial_layer_speed"] = tostring(math.max(15, math.floor(base_spd * 0.50 + 0.5)))

  if config["hot_plate_temp_initial_layer"] and config["hot_plate_temp_initial_layer"][1] then
    local bt = tonumber(config["hot_plate_temp_initial_layer"][1]) or 80
    local new_bt = bt + 5
    config["hot_plate_temp_initial_layer"] = { tostring(new_bt) }
    print("[Symptom:BedAdhesion] Initial layer width +10% -> " .. config["initial_layer_line_width"] .. "mm, speed -50% -> " .. config["initial_layer_speed"] .. "mm/s, bed initial temp +5C -> " .. tostring(new_bt) .. "C")
  else
    config["hot_plate_temp_initial_layer"] = { "85" }
    print("[Symptom:BedAdhesion] Initial layer width +10% -> " .. config["initial_layer_line_width"] .. "mm, speed -50% -> " .. config["initial_layer_speed"] .. "mm/s, bed initial temp -> 85C")
  end
end
