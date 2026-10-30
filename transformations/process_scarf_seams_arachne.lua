-- Process Quality Rule: Scarf Joint Seams & Arachne Variable-Width Walls
-- 1. Switches perimeter generator from Classic to Arachne so thin walls and fine details
--    use single variable-width extrusions instead of gap-fill jitter.
-- 2. Enables OrcaSlicer Scarf Joint Seams (contour + inner scarf, 15 steps, 10% start height,
--    10% seam gap, 155 deg angle threshold) to ramp layer start/end overlap and hide Z-seams.

params = {
  {
    id = "scarf_mode",
    label = "Scarf Joint Seam Mode (seam_slope_type)",
    description = "'all' ('Contour and hole') hides seams on both outer walls and inner holes; 'external' ('Contour') applies only to outer walls.",
    type = "enum",
    default = "all",
    unit = "",
    options = {
      { value = "all", label = "Contour and Hole (Outer + Inner Walls)" },
      { value = "external", label = "Contour (Outer Walls Only)" },
      { value = "none", label = "None (Disable Scarf Seams)" },
    },
  },
  {
    id = "scarf_length",
    label = "Scarf Ramp Minimum Length (mm)",
    description = "Horizontal ramp distance over which the layer start/end tapers (15–25 mm recommended).",
    type = "range",
    min = 5,
    max = 40,
    step = 1,
    default = "15",
    unit = "mm",
    options = {
      { value = "15", label = "15 mm (Standard)" },
      { value = "20", label = "20 mm (Smooth Cylinder Ramp)" },
      { value = "25", label = "25 mm (Extra-Gradual Ramp)" },
    },
  },
  {
    id = "seam_gap",
    label = "Loop Closure Seam Gap (%)",
    description = "Percentage of nozzle diameter left at loop closure so the end doesn't bulge over the start.",
    type = "range",
    min = 0,
    max = 25,
    step = 1,
    default = "10%",
    unit = "%",
    options = {
      { value = "5%", label = "5% (Tight)" },
      { value = "10%", label = "10% (Recommended)" },
      { value = "15%", label = "15% (Extra Wipe Clearance)" },
    },
  },
  {
    id = "wall_generator",
    label = "Perimeter Wall Generator",
    description = "Arachne dynamically varies extrusion width on thin walls and text.",
    type = "enum",
    default = "arachne",
    unit = "",
    options = {
      { value = "arachne", label = "Arachne Variable-Width Walls" },
      { value = "classic", label = "Classic Fixed-Width Walls" },
    },
  },
}

function validate(p, ctx)
  local sl = tonumber(p.scarf_length) or 15
  if sl < 5 or sl > 40 then
    return false, "Scarf ramp length must be between 5 mm and 40 mm."
  end
  return true
end

function apply(config, ctx, p)
  local wall_gen = (p and p.wall_generator) or "arachne"
  local scarf_mode = (p and p.scarf_mode) or "all"
  if scarf_mode == "contour" then scarf_mode = "external" end
  local scarf_len = tostring(math.floor(tonumber(p and p.scarf_length) or 15))
  local seam_gap = (p and p.seam_gap) or "10%"
  if not string.find(seam_gap, "%%") then seam_gap = seam_gap .. "%" end

  config["wall_generator"] = wall_gen
  config["wall_transition_angle"] = "10"
  config["wall_transition_length"] = "100%"
  config["min_feature_size"] = "25%"
  config["min_bead_width"] = "85%"

  config["seam_slope_type"] = scarf_mode
  config["seam_slope_conditional"] = "1"
  config["seam_slope_start_height"] = "10%"
  config["seam_slope_entire_loop"] = "0"
  config["seam_slope_inner_walls"] = "1"
  config["seam_slope_min_length"] = scarf_len
  config["seam_slope_steps"] = "15"
  config["seam_gap"] = seam_gap
  config["scarf_angle_threshold"] = "155"
  config["wipe_on_loops"] = "1"

  print("[Process:ScarfAndArachne] wall_generator -> " .. wall_gen .. ", seam_slope_type -> " .. scarf_mode .. " (" .. scarf_len .. "mm ramp, " .. seam_gap .. " seam gap)")
end
