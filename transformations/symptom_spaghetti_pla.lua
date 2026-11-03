-- Symptom 4: Spaghettization with PLA (Nozzle Knock / Detachment)
-- Eliminates nozzle collisions with curled overhangs or grid infill intersections.
-- Switches sparse infill pattern from Grid to non-crossing Gyroid and enables reduce_crossing_wall = 1.

function apply(config, ctx)
  config["sparse_infill_pattern"] = "gyroid"
  config["reduce_crossing_wall"] = "1"

  print("[Symptom:SpaghettiGuard] Sparse infill -> gyroid, reduce_crossing_wall -> 1")
end
