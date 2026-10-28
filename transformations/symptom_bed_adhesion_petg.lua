-- Symptom 3: Poor Bed Adhesion with PETG on Kobra S1
-- Increases first-layer squish and thermal bonding on Textured Gold PEI spring steel plates.
-- Widens initial layer bead to 0.55mm, slows first layer speed to 25mm/s,
-- and raises first layer bed temperature by +5°C (up to 85°C for PETG).

function apply(config, ctx)
  config["initial_layer_line_width"] = "0.55"
  config["initial_layer_speed"] = "25"

  if config["hot_plate_temp_initial_layer"] and config["hot_plate_temp_initial_layer"][1] then
    local bt = tonumber(config["hot_plate_temp_initial_layer"][1]) or 80
    local new_bt = bt + 5
    config["hot_plate_temp_initial_layer"] = { tostring(new_bt) }
    print("[Symptom:BedAdhesion] Initial layer width -> 0.55mm, speed -> 25mm/s, bed initial temp -> " .. tostring(new_bt) .. "C")
  else
    config["hot_plate_temp_initial_layer"] = { "85" }
    print("[Symptom:BedAdhesion] Initial layer width -> 0.55mm, speed -> 25mm/s, bed initial temp -> 85C")
  end
end
