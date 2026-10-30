-- Symptom Fix: Scars, Nozzle Drag Marks & Diagonal Scratches Across Top Surfaces
meta = {
  id = "sym_top_surface_scars",
  name = "Scars, Nozzle Drag Marks & Diagonal Scratches Across Top Surfaces",
  icon = "🗡️",
  category = "5. Top/Bottom Surfaces & Infill (Travel & Monotonic Top)",
  tree_category = "5. Top/Bottom Surfaces & Infill",
  tree_subcategory = "Monotonic Skin & Travel Lift",
  slicer_menu_path = "Monotonic Top Surface + Avoid Crossing Walls + Z-Hop on Top Surface + Ironing",
  when_to_run = "Apply when a finished flat top surface has a shiny diagonal drag line where the hot nozzle traveled across it without lifting.",
  formula_summary = "top_surface_pattern = monotonicline  •  reduce_infill_retraction = 0  •  reduce_crossing_wall = 1",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Diagonal scars across top surfaces happen when `reduce_infill_retraction` skips retraction/Z-hop on internal moves, allowing the hot nozzle tip to drag across the finished skin.",
    "2. Disabling `reduce_infill_retraction`, using `monotonicline` top pattern, and enabling `reduce_crossing_wall = 1` prevents the nozzle from ever dragging across finished top layers.",
  },
  hardware_checklist = {
    "📐 Verify Bed Level & Extruder Flow: If the top layer is slightly over-extruded (+3%), the raised ridges sit higher than the nozzle tip during travel moves.",
  },
}

params = {
  {
    id = "top_pattern",
    label = "1. Which Top Surface Pattern (top_surface_pattern) should we use?",
    description = "'monotonicline' prints all top lines in the exact same left-to-right overlap direction so there are no visible ridge seams around holes.",
    type = "enum",
    default = "monotonicline",
    unit = "",
    options = {
      { value = "monotonicline", label = "monotonicline (Uniform Sheen — Recommended)" },
      { value = "monotonic", label = "monotonic (Filled Monotonic)" },
      { value = "concentric", label = "concentric (Rings)" },
    },
  },
  {
    id = "ironing_mode",
    label = "2. Enable Top Surface Ironing (ironing_type)?",
    description = "Leave off ('no ironing') for PETG/PCTG (which sticks to the nozzle); 'top' works well on flat PLA badges.",
    type = "enum",
    default = "no ironing",
    unit = "",
    options = {
      { value = "no ironing", label = "no ironing (Recommended for PETG/PCTG/General)" },
      { value = "top", label = "top (Iron All Top Surfaces — PLA Only)" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local pat = p.top_pattern or "monotonicline"
  local iron = p.ironing_mode or "no ironing"
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Top Pattern = %s • Avoid Crossing Walls • Ironing = %s", pat, iron)
  return {
    title = string.format("Symptom Fix: Top Surface Scars & Drag Marks (%s, %s)", pat, iron),
    description = string.format("Prevents nozzle travel drag marks across top skins for %s: sets top_surface_pattern = %s, reduce_infill_retraction = 0, reduce_crossing_wall = 1, and ironing_type = %s.", scope, pat, iron),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local pat = (p and p.top_pattern) or "monotonicline"
  local iron = (p and p.ironing_mode) or "no ironing"
  config["top_surface_pattern"] = pat
  config["reduce_infill_retraction"] = "0"
  config["reduce_crossing_wall"] = "1"
  config["ironing_type"] = iron
  local d = describe(p or {}, ctx)
  print("[Symptom:TopScars] " .. (d.badge or ""))
end
