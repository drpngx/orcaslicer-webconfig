-- Symptom Fix: Z-Banding, Inconsistent Layer Lines & 'Benchy Hull Line'
meta = {
  id = "sym_z_banding_hull_line",
  name = "Z-Banding, Inconsistent Layer Lines & 'Benchy Hull Line'",
  icon = "📏",
  category = "4. Walls, Seams & Dimensional (Z-Axis & Cooling Shrinkage)",
  tree_category = "4. Walls, Seams & Dimensional",
  tree_subcategory = "Z-Lead Screw & Layer-Time Shrinkage",
  slicer_menu_path = "Don't Slow Down Outer Walls + Consistent Wall Order + Z-Axis Lead Screw Check",
  when_to_run = "Apply when horizontal bands or a pronounced step line appear at the height where an internal floor/deck transitions to hollow walls (the classic Benchy hull line).",
  formula_summary = "dont_slow_down_outer_wall = 1  •  wall_sequence = inner-outer-inner wall",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. When layer print time changes suddenly (e.g. transitioning from sparse infill to a solid deck), cooling slowdown changes the outer wall speed — changing wall glossiness and thermal shrinkage!",
    "2. Enabling `dont_slow_down_outer_wall = 1` keeps the outer perimeter at its exact configured speed and glossiness from bottom to top.",
  },
  hardware_checklist = {
    "🧴 Clean & Lubricate the 3 Z-Axis Lead Screws: Wipe old dark grease off the Kobra S1 Z-axis lead screws and guide rods with a lint-free cloth and apply a light coat of PTFE/lithium grease.",
  },
}

params = {
  {
    id = "lock_outer_spd",
    label = "1. Lock Outer Wall Speed (dont_slow_down_outer_wall) so layer-time cooling never changes outer wall sheen?",
    description = "Enabling this keeps the outer wall speed identical on every layer, eliminating gloss bands and hull-line steps.",
    type = "enum",
    default = "1",
    unit = "",
    options = {
      { value = "1", label = "1 — Lock Constant Outer Wall Speed (Recommended)" },
      { value = "0", label = "0 — Allow Outer Wall Slowdown" },
    },
  },
  {
    id = "wall_order",
    label = "2. Which Wall Printing Sequence should be used for maximum Z-wall consistency?",
    description = "'inner-outer-inner wall' isolates the outer wall from internal infill/deck expansion.",
    type = "enum",
    default = "inner-outer-inner wall",
    unit = "",
    options = {
      { value = "inner-outer-inner wall", label = "inner-outer-inner wall (Best Dimensional Z-Consistency)" },
      { value = "inner wall/outer wall", label = "inner wall/outer wall (Standard)" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local lock_spd = p.lock_outer_spd or "1"
  local w_ord = p.wall_order or "inner-outer-inner wall"
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("dont_slow_down_outer_wall = %s • wall_sequence = %s", lock_spd, w_ord)
  return {
    title = string.format("Symptom Fix: Z-Banding & Hull Line (Lock Outer Speed=%s, %s)", lock_spd, w_ord),
    description = string.format("Eliminates Benchy hull-line shrinkage bands and Z-wall gloss changes for %s: sets dont_slow_down_outer_wall = %s and wall_sequence = %s.", scope, lock_spd, w_ord),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local lock_spd = (p and p.lock_outer_spd) or "1"
  local w_ord = (p and p.wall_order) or "inner-outer-inner wall"
  config["dont_slow_down_outer_wall"] = lock_spd
  config["wall_sequence"] = w_ord
  local d = describe(p or {}, ctx)
  print("[Symptom:ZBanding] " .. (d.badge or ""))
end
