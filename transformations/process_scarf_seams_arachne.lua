-- Process Quality Rule: Scarf Joint Seams & Arachne Variable-Width Walls
-- 1. Switches perimeter generator from Classic to Arachne so thin walls and fine details
--    use single variable-width extrusions instead of gap-fill jitter.
-- 2. Enables OrcaSlicer Scarf Joint Seams (contour + inner scarf, 15 steps, 10% start height,
--    10% seam gap, 155 deg angle threshold) to ramp layer start/end overlap and hide Z-seams.

function apply(config, ctx)
  config["wall_generator"] = "arachne"
  config["wall_transition_angle"] = "10"
  config["wall_transition_length"] = "100%"
  config["min_feature_size"] = "25%"
  config["min_bead_width"] = "85%"

  config["seam_slope_type"] = "all"
  config["seam_slope_conditional"] = "1"
  config["seam_slope_start_height"] = "10%"
  config["seam_slope_entire_loop"] = "0"
  config["seam_slope_min_length"] = "15"
  config["seam_slope_steps"] = "15"
  config["seam_gap"] = "10%"
  config["scarf_angle_threshold"] = "155"
  config["wipe_on_loops"] = "1"

  print("[Process:ScarfAndArachne] wall_generator -> arachne, seam_slope_type -> all (15 steps, 10% start height, 10% seam gap)")
end
