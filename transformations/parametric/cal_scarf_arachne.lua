-- Calibration Print #7: Scarf Joint Seam & Arachne Wall Parameterized Tuning
meta = {
  id = "cal_scarf_arachne",
  name = "Scarf Joint Seam & Arachne Wall Parameterized Tuning",
  icon = "🪡",
  category = "Calibration Print #7 (Seam & Wall Quality)",
  tree_category = "Post-Print Calibration",
  tree_subcategory = "Calibration Test",
  slicer_menu_path = "Process → Quality → Scarf Joint Seam & Wall Generator",
  when_to_run = "Run when tuning Z-seam visibility on cylinders/round parts or thin tapered walls.",
  formula_summary = "Configures seam_slope_type, seam_slope_min_length, seam_gap, and wall_generator",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Print a test cylinder (25 mm diameter) and inspect the vertical Z-seam joint.",
    "2. If you see a ridge at the seam, increase Scarf Ramp Length to 20–25 mm; if inner bore holes have a seam bump, switch Scarf Mode from 'contour' to 'all'.",
  },
  hardware_checklist = {
    "🖨️ Slice and print the built-in test model from the AnycubicSlicerNext / OrcaSlicer top 'Calibration' menu before entering your measurements below.",
  },
}

params = {
  {
    id = "scarf_mode",
    label = "1. Where should Scarf Joint Seams be applied (seam_slope_type)?",
    description = "'external' ('Contour') applies scarf ramps to visible outer walls; 'all' ('Contour and hole') applies to both outer walls and inner holes.",
    type = "enum",
    default = "all",
    unit = "",
    options = {
      { value = "external", label = "Contour (Outer Walls Only)" },
      { value = "all", label = "Contour and Hole (Outer + Inner Walls)" },
      { value = "none", label = "None (Disable Scarf Seams)" },
    },
  },
  {
    id = "scarf_len",
    label = "2. What Scarf Ramp Length (mm) should be used?",
    description = "15 mm is standard; 20–25 mm gives an even flatter ramp on glossy PETG/PCTG cylinders.",
    type = "range",
    min = 5,
    max = 40,
    step = 1,
    default = "20",
    unit = "mm",
    options = {
      { value = "15", label = "15 mm (Standard)" },
      { value = "20", label = "20 mm (Smooth Ramp)" },
      { value = "25", label = "25 mm (Extra-Gradual Ramp)" },
    },
  },
  {
    id = "seam_gap",
    label = "3. What Seam Gap (% of nozzle diameter) should be left at loop closure?",
    description = "10% prevents a raised bump where the end of the perimeter overlaps the start.",
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
    id = "wall_gen",
    label = "4. Which Wall Generator Engine should be used?",
    description = "Arachne dynamically varies extrusion width to fill thin walls and sharp text without gap infill.",
    type = "enum",
    default = "arachne",
    unit = "",
    options = {
      { value = "arachne", label = "Arachne Variable-Width Walls (Recommended)" },
      { value = "classic", label = "Classic Fixed-Width Walls" },
    },
  },
}

function validate(p, ctx)
  local scarf_len = tonumber(p.scarf_len) or 20
  if scarf_len < 5 or scarf_len > 40 then
    return false, "Scarf ramp length must be between 5 mm and 40 mm."
  end
  return true
end

function describe(p, ctx)
  local scarf_mode = p.scarf_mode or "all"
  if scarf_mode == "contour" then scarf_mode = "external" end
  local scarf_len = p.scarf_len or "20"
  local seam_gap = p.seam_gap or "10%"
  if not string.find(seam_gap, "%%") then seam_gap = seam_gap .. "%" end
  local wall_gen = p.wall_gen or "arachne"
  local badge = string.format("Scarf = %s (%smm ramp, %s gap) • Wall Engine = %s", scarf_mode, scarf_len, seam_gap, wall_gen)
  return {
    title = string.format("Tuning: Scarf Seam (%s, %smm) + %s Walls", scarf_mode, scarf_len, wall_gen),
    description = string.format("Parameterized Scarf Seam & Wall tuning: seam_slope_type = %s, ramp = %smm, seam_gap = %s, wall_generator = %s.", scarf_mode, scarf_len, seam_gap, wall_gen),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local scarf_mode = (p and p.scarf_mode) or "all"
  if scarf_mode == "contour" then scarf_mode = "external" end
  local scarf_len = tostring(math.floor(tonumber(p and p.scarf_len) or 20))
  local seam_gap = (p and p.seam_gap) or "10%"
  if not string.find(seam_gap, "%%") then seam_gap = seam_gap .. "%" end
  local wall_gen = (p and p.wall_gen) or "arachne"

  config["wall_generator"] = wall_gen
  config["seam_slope_type"] = scarf_mode
  config["seam_slope_conditional"] = "1"
  config["seam_slope_start_height"] = "10%"
  config["seam_slope_min_length"] = scarf_len
  config["seam_slope_steps"] = "15"
  config["seam_gap"] = seam_gap
  config["scarf_angle_threshold"] = "155"
  config["wipe_on_loops"] = "1"
  local d = describe(p or {}, ctx)
  print("[Tuning:ScarfArachne] " .. (d.badge or ""))
end
