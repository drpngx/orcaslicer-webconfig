-- Symptom 4: Spaghettization with PLA (Nozzle Knock / Detachment)
-- Eliminates nozzle collisions with curled overhangs or grid infill intersections.
-- Switches sparse infill pattern from Grid to non-crossing Gyroid, enables 2.0x layer_height Slope Z-Hop,
-- and enables reduce_crossing_wall = 1.

function apply(config, ctx)
  config["sparse_infill_pattern"] = "gyroid"
  local lh = tonumber(config["layer_height"]) or 0.20
  local zhop_mm = string.format("%.2f", lh * 2.0)
  config["z_hop"] = { zhop_mm }
  config["z_hop_types"] = { "Slope Lift" }
  config["reduce_crossing_wall"] = "1"

  print("[Symptom:SpaghettiGuard] Sparse infill -> gyroid, Z-Hop -> " .. zhop_mm .. "mm (2.0x layer_height) Slope, reduce_crossing_wall -> 1")
end
