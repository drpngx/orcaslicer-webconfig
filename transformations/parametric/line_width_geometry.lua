-- Core Slicer Config Pack: Nozzle-Proportional Line Width & Extrusion Geometry Optimizer
-- Computes every per-feature extrusion width as a percentage of the active `nozzle_diameter`,
-- so swapping a 0.4mm nozzle for a 0.6mm / 0.8mm nozzle automatically rescales every line width:
--   • Initial Layer (120–125%)        → wide first-layer bead for bed adhesion
--   • Outer Wall (100–105%)           → crisp, dimensionally accurate visible perimeter
--   • Top Surface (90–95%)            → crisp top surfaces and sharp lettering
--   • Inner Wall & Sparse Infill (120–135%) → thicker internal ribs, faster & stronger parts

meta = {
  id = "line_width_geometry",
  name = "Nozzle-Proportional Line Width & Extrusion Geometry Optimizer",
  icon = "📏",
  category = "Core Slicer Config Pack (Line Width Geometry)",
  tree_category = "4. Walls, Seams & Dimensional",
  tree_subcategory = "Nozzle-Proportional Line Widths",
  slicer_menu_path = "Process → Quality → Line Width (Initial Layer / Outer Wall / Inner Wall / Top Surface / Internal Solid / Sparse Infill / Support)",
  when_to_run = "Enabled by default. Keeps every per-feature line width proportional to the installed nozzle diameter (0.2mm – 1.0mm), so nozzle swaps never leave 0.4mm-sized widths on a 0.6mm or 0.8mm nozzle.",
  formula_summary = "Width = nozzle_diameter × Feature% (Initial 125% • Outer 105% • Top 95% • Inner Wall 125% • Sparse Infill 125% • Internal Solid 105% • Support 100%)",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. A slightly narrower top surface line (90–95% of the nozzle) gives a smoother top skin and sharper embossed lettering.",
    "2. Wider inner walls and sparse infill (120–135%) lay down fewer, thicker beads: fewer line moves = shorter print time, and thicker ribs = stronger parts. The visible outer wall stays at 105% so dimensions and surface finish are unchanged.",
    "3. A wide first-layer bead (120–125%) presses more polymer into the build plate texture for reliable adhesion.",
  },
  hardware_checklist = {
    "⚠️ Wider internal lines carry more plastic per mm: the slicer automatically clamps speed to your filament's Max Volumetric Speed, so watch print-time estimates on low-MVS filaments (TPU, Silk PLA).",
  },
}

params = {
  {
    id = "geometry_preset",
    label = "1. Line Width Geometry Preset (Snaps Dials Below)",
    description = "Balanced is the default; Strength widens internal ribs to 135%; Fine Detail narrows widths for miniatures and lettering.",
    type = "enum",
    default = "balanced",
    options = {
      {
        value = "balanced",
        label = "⚖️ Balanced (Initial 125% • Outer 105% • Top 95% • Inner/Infill 125%)",
        sets = { initial_pct = "125", outer_pct = "105", top_pct = "95", inner_pct = "125", infill_pct = "125" },
      },
      {
        value = "strength",
        label = "💪 Strength & Speed (Initial 125% • Outer 105% • Top 95% • Inner/Infill 135%)",
        sets = { initial_pct = "125", outer_pct = "105", top_pct = "95", inner_pct = "135", infill_pct = "135" },
      },
      {
        value = "fine_detail",
        label = "🔍 Fine Detail & Lettering (Initial 120% • Outer 100% • Top 90% • Inner 112% • Infill 120%)",
        sets = { initial_pct = "120", outer_pct = "100", top_pct = "90", inner_pct = "112", infill_pct = "120" },
      },
      {
        value = "custom",
        label = "🎛️ Custom Per-Feature Percentages",
      },
    },
  },
  {
    id = "initial_pct",
    label = "2. Initial Layer Line Width (% of nozzle diameter)",
    description = "Wide first-layer bead for bed adhesion (120–125% recommended).",
    type = "range",
    min = 100,
    max = 150,
    step = 1,
    default = "125",
    unit = "%",
  },
  {
    id = "outer_pct",
    label = "3. Outer Wall Line Width (% of nozzle diameter)",
    description = "Visible perimeter width (100–105% keeps dimensions and surface finish crisp).",
    type = "range",
    min = 95,
    max = 120,
    step = 1,
    default = "105",
    unit = "%",
  },
  {
    id = "top_pct",
    label = "4. Top Surface Line Width (% of nozzle diameter)",
    description = "Narrower top surface lines (90–95%) for a smooth top skin and sharp lettering.",
    type = "range",
    min = 85,
    max = 110,
    step = 1,
    default = "95",
    unit = "%",
  },
  {
    id = "inner_pct",
    label = "5. Inner Wall Line Width (% of nozzle diameter)",
    description = "Thicker internal perimeters (120–135%) for stronger, faster parts.",
    type = "range",
    min = 100,
    max = 150,
    step = 1,
    default = "125",
    unit = "%",
  },
  {
    id = "infill_pct",
    label = "6. Sparse Infill Line Width (% of nozzle diameter)",
    description = "Thicker infill ribs (120–135%) mean fewer moves and stiffer infill.",
    type = "range",
    min = 100,
    max = 150,
    step = 1,
    default = "125",
    unit = "%",
  },
}

local function resolve_nozzle(config, ctx)
  local d = tonumber(ctx and ctx.nozzle_diameter)
  if not d then
    d = tonumber(config["nozzle_diameter"])
  end
  if not d or d <= 0.05 then
    d = 0.4
  end
  return d
end

local function resolve_pcts(p)
  return {
    initial = tonumber(p and p.initial_pct) or 125,
    outer = tonumber(p and p.outer_pct) or 105,
    top = tonumber(p and p.top_pct) or 95,
    inner = tonumber(p and p.inner_pct) or 125,
    infill = tonumber(p and p.infill_pct) or 125,
  }
end

local function w(d, pct)
  return string.format("%.2f", d * pct / 100.0)
end

function validate(p, ctx)
  local c = resolve_pcts(p)
  if c.top > c.outer + 5 then
    return false, "Top surface width should not exceed the outer wall width by more than 5% — wide top lines leave ridged, uneven top skins."
  end
  if c.inner < c.outer then
    return false, "Inner wall width should be at least the outer wall width, otherwise inner perimeters under-fill behind the visible wall."
  end
  return true
end

function describe(p, ctx)
  local c = resolve_pcts(p)
  local d = tonumber(ctx and ctx.nozzle_diameter) or 0.4
  local badge = string.format(
    "Ø%.2fmm • Initial %d%% (%smm) • Outer %d%% (%smm) • Top %d%% (%smm) • Inner %d%% (%smm) • Infill %d%% (%smm)",
    d, c.initial, w(d, c.initial), c.outer, w(d, c.outer), c.top, w(d, c.top), c.inner, w(d, c.inner), c.infill, w(d, c.infill)
  )
  return {
    title = string.format("Line Width Geometry: Initial %d%% • Outer %d%% • Top %d%% • Inner %d%% • Infill %d%%", c.initial, c.outer, c.top, c.inner, c.infill),
    description = "Sets every per-feature line width proportional to the active nozzle diameter: " .. badge .. ".",
    badge = badge,
  }
end

function apply(config, ctx, p)
  local c = resolve_pcts(p)
  local d = resolve_nozzle(config, ctx)

  config["initial_layer_line_width"] = fmt_num(d * c.initial / 100.0, "%.2f")
  local target_outer = d * c.outer / 100.0
  if (ctx and ctx._process_seeded) and c.outer == 105 then
    local cur_lw = tonumber(config["line_width"]) or target_outer
    if cur_lw > target_outer then
      target_outer = cur_lw
    end
  end
  config["outer_wall_line_width"] = fmt_num(target_outer, "%.2f")
  config["line_width"] = fmt_num(target_outer, "%.2f")
  config["top_surface_line_width"] = fmt_num(d * c.top / 100.0, "%.2f")
  config["inner_wall_line_width"] = fmt_num(d * c.inner / 100.0, "%.2f")
  config["sparse_infill_line_width"] = fmt_num(d * c.infill / 100.0, "%.2f")
  config["internal_solid_infill_line_width"] = fmt_num(d * c.outer / 100.0, "%.2f")
  config["support_line_width"] = fmt_num(d, "%.2f")

  local lw_keys = {
    "initial_layer_line_width",
    "outer_wall_line_width",
    "line_width",
    "top_surface_line_width",
    "inner_wall_line_width",
    "sparse_infill_line_width",
    "internal_solid_infill_line_width",
    "support_line_width",
  }
  for _, k in ipairs(lw_keys) do
    constrain(k, d * 0.85, d * 1.60, "Nozzle orifice line-width envelope [0.85d, 1.60d]")
  end

  print(string.format(
    "[Pack:LineWidthGeometry] Ø%.2fmm -> initial %s • outer %s • top %s • inner %s • infill %s • solid %s • support %s",
    d,
    config["initial_layer_line_width"],
    config["outer_wall_line_width"],
    config["top_surface_line_width"],
    config["inner_wall_line_width"],
    config["sparse_infill_line_width"],
    config["internal_solid_infill_line_width"],
    config["support_line_width"]
  ))
end
