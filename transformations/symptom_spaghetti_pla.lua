-- Symptom 4: Spaghettization with PLA (Nozzle Knock / Detachment)
-- Eliminates nozzle collisions with curled overhangs or grid infill intersections.
-- Switches sparse infill pattern from Grid to non-crossing Gyroid, enables 0.40mm Slope Z-Hop,
-- and enables reduce_crossing_wall = 1.

function apply(config, ctx)
  config["sparse_infill_pattern"] = "gyroid"
  config["z_hop"] = { "0.40" }
  config["z_hop_types"] = { "Slope" }
  config["reduce_crossing_wall"] = "1"

  print("[Symptom:SpaghettiGuard] Sparse infill -> gyroid, Z-Hop -> 0.40mm Slope, reduce_crossing_wall -> 1")
end
