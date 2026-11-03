-- Core Slicer Config Pack: Structural Strength & Z-Axis Inter-Layer Bonding
-- Coordinates thermal, cooling, and perimeter settings simultaneously for load-bearing functional parts:
--   • Nozzle temperature +5°C to +8°C (clamped per spool to the polymer's safe max on export)
--   • Max part-cooling fan -20% to -35% (longer polymer chain diffusion across layer lines)
--   • alternate_extra_wall = 1 (extra inner perimeter every other layer keys infill into the walls)
--   • wall_loops 4–6, top/bottom shell layers 5–6, infill_wall_overlap 20–25%
--   • Isotropic 3D infill (gyroid or crosshatch)
--
-- Filament-level changes (temperature boost, fan cut) are exported per spool through the
-- `_structural_temp_boost_c` / `_structural_fan_cut_pct` side-channel so every exported filament
-- preset is clamped against its OWN polymer's safe nozzle range (PLA, PETG, ASA, PA, PC, ...).

meta = {
  id = "structural_strength",
  name = "Structural Strength & Z-Axis Inter-Layer Bonding Pack",
  icon = "💪",
  category = "Core Slicer Config Pack (Structural Strength)",
  tree_category = "7. Structural, Mechanical & Multi-Color",
  tree_subcategory = "Load-Bearing Functional Parts",
  slicer_menu_path = "Filament → Nozzle Temp (+5–8°C) & Max Fan (-20–35%) • Process → Walls (4–6 loops, Alternate Extra Wall) • Top/Bottom Shells (5–6) • Infill/Wall Overlap (20–25%) • Gyroid / Crosshatch Infill",
  when_to_run = "Enable for brackets, mounts, hooks, gears, clamps, and any part that carries load or snaps along layer lines.",
  formula_summary = "Nozzle += Temp_Boost (≤ polymer max) • Max_Fan ×= (1 - Fan_Cut%) • wall_loops = 4–6 • alternate_extra_wall = 1 • shells = 5–6 • infill_wall_overlap = 20–25% • gyroid / crosshatch",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. FDM parts are weakest across layer lines (Z-axis). A hotter nozzle and less part-cooling keep the previous layer above its glass transition longer, so polymer chains diffuse deeper across the layer boundary.",
    "2. Extra perimeters carry most of the bending load; `alternate_extra_wall` adds an extra inner perimeter on every other layer, interlocking walls and infill like brickwork.",
    "3. Gyroid and crosshatch are isotropic 3D infills: they resist load equally in every direction instead of having weak axes like grid or lines.",
  },
  hardware_checklist = {
    "🚪 Keep the chamber closed for structural prints in ASA / ABS / PA / PC: warmer ambient air further improves layer bonding and reduces residual stress.",
    "⚠️ Less cooling reduces overhang & bridge quality — combine with the Bridges & Steep Overhangs pack (100% overhang fan) if the part has steep overhangs.",
  },
}

params = {
  {
    id = "strength_preset",
    label = "1. Structural Strength Preset (Snaps Dials Below)",
    description = "Functional is a balanced upgrade for everyday brackets; Load-Bearing maxes out bonding and perimeters for parts under real load.",
    type = "enum",
    default = "functional",
    options = {
      {
        value = "functional",
        label = "🔧 Functional Parts (+5°C • Fan -20% • 4 Walls • 5 Shells • 20% Overlap • Gyroid)",
        sets = { temp_boost_c = "5", fan_cut_pct = "20", wall_loops = "4", shell_layers = "5", wall_overlap_pct = "20", infill_pattern = "gyroid", alternate_extra_wall = "1" },
      },
      {
        value = "load_bearing",
        label = "🏗️ Load-Bearing Max (+8°C • Fan -35% • 6 Walls • 6 Shells • 25% Overlap • Gyroid)",
        sets = { temp_boost_c = "8", fan_cut_pct = "35", wall_loops = "6", shell_layers = "6", wall_overlap_pct = "25", infill_pattern = "gyroid", alternate_extra_wall = "1" },
      },
      {
        value = "stiff_crosshatch",
        label = "📐 Stiff & Fast (+5°C • Fan -25% • 4 Walls • 5 Shells • 20% Overlap • Crosshatch)",
        sets = { temp_boost_c = "5", fan_cut_pct = "25", wall_loops = "4", shell_layers = "5", wall_overlap_pct = "20", infill_pattern = "crosshatch", alternate_extra_wall = "1" },
      },
      {
        value = "custom",
        label = "🎛️ Custom Strength Dials",
      },
    },
  },
  {
    id = "temp_boost_c",
    label = "2. Nozzle Temperature Boost (°C, clamped to each polymer's safe max)",
    description = "+5°C to +8°C deepens inter-layer fusion. Each exported filament preset is clamped against its own polymer's safe nozzle maximum.",
    type = "range",
    min = 0,
    max = 10,
    step = 1,
    default = "5",
    unit = "°C",
  },
  {
    id = "fan_cut_pct",
    label = "3. Max Part-Cooling Fan Reduction (%)",
    description = "Scales fan_max_speed down by this percentage (-20% to -35% recommended).",
    type = "range",
    min = 0,
    max = 50,
    step = 5,
    default = "20",
    unit = "%",
  },
  {
    id = "wall_loops",
    label = "4. Wall Loops (Perimeters)",
    description = "Perimeters carry most bending load: 4–6 for functional parts.",
    type = "range",
    min = 2,
    max = 8,
    step = 1,
    default = "4",
    unit = "loops",
  },
  {
    id = "shell_layers",
    label = "5. Top & Bottom Shell Layers",
    description = "Thicker top/bottom skins (5–6 layers) for stiff, crush-resistant faces.",
    type = "range",
    min = 3,
    max = 10,
    step = 1,
    default = "5",
    unit = "layers",
  },
  {
    id = "wall_overlap_pct",
    label = "6. Infill / Wall Overlap (infill_wall_overlap, %)",
    description = "20–25% welds the infill firmly into the inner perimeter.",
    type = "range",
    min = 10,
    max = 35,
    step = 1,
    default = "20",
    unit = "%",
  },
  {
    id = "infill_pattern",
    label = "7. Isotropic Infill Pattern (sparse_infill_pattern)",
    description = "Gyroid and crosshatch resist load equally in all directions.",
    type = "enum",
    default = "gyroid",
    options = {
      { value = "gyroid", label = "gyroid — Isotropic 3D Gyroid (Recommended)" },
      { value = "crosshatch", label = "crosshatch — Isotropic Crosshatch (Faster)" },
      { value = "cubic", label = "cubic — 3D Cubic" },
    },
  },
  {
    id = "alternate_extra_wall",
    label = "8. Alternate Extra Wall (alternate_extra_wall)",
    description = "Adds an extra inner perimeter on every other layer to interlock walls and infill.",
    type = "enum",
    default = "1",
    options = {
      { value = "1", label = "Enabled (1)" },
      { value = "0", label = "Disabled (0)" },
    },
  },
}

local function resolve(p)
  return {
    boost = math.floor((tonumber(p and p.temp_boost_c) or 5) + 0.5),
    fan_cut = math.floor((tonumber(p and p.fan_cut_pct) or 20) + 0.5),
    walls = math.floor((tonumber(p and p.wall_loops) or 4) + 0.5),
    shells = math.floor((tonumber(p and p.shell_layers) or 5) + 0.5),
    overlap = math.floor((tonumber(p and p.wall_overlap_pct) or 20) + 0.5),
    pattern = (p and p.infill_pattern) or "gyroid",
    alt_wall = (p and p.alternate_extra_wall) or "1",
  }
end

local function set_like(config, key, val)
  if type(config[key]) == "table" then
    config[key] = { val }
  else
    config[key] = val
  end
end

function validate(p, ctx)
  local c = resolve(p)
  local fil = string.upper(tostring((ctx and ctx.filament_type) or ""))
  if string.find(fil, "PLA") and c.fan_cut > 35 then
    return false, "PLA softens quickly: cutting max part-cooling fan by more than 35% causes sagging walls and drooping overhangs. Use 35% or less."
  end
  if c.walls >= 7 and c.overlap >= 30 then
    return false, "7+ wall loops with ≥30% infill/wall overlap over-packs the perimeter zone and causes bulging walls. Reduce overlap to 25% or less."
  end
  return true
end

function describe(p, ctx)
  local c = resolve(p)
  local badge = string.format(
    "Nozzle +%d°C (≤ polymer max) • Max Fan -%d%% • %d Walls%s • %d Top/Bottom Shells • %d%% Overlap • %s",
    c.boost, c.fan_cut, c.walls, (c.alt_wall == "1") and " + Alternate Extra Wall" or "", c.shells, c.overlap, c.pattern
  )
  return {
    title = string.format("Structural Strength: +%d°C • Fan -%d%% • %d Walls • %d Shells • %s", c.boost, c.fan_cut, c.walls, c.shells, c.pattern),
    description = "Load-bearing part tuning: " .. badge .. ".",
    badge = badge,
  }
end

function apply(config, ctx, p)
  local c = resolve(p)

  -- Filament-level side-channel (applied per spool & clamped per polymer on export)
  config["_structural_temp_boost_c"] = tostring(c.boost)
  config["_structural_fan_cut_pct"] = tostring(c.fan_cut)

  -- Live preview of the active filament (clamped to the active polymer's safe max when known)
  local noz_max = tonumber(ctx and ctx.filament_nozzle_max_c) or 400
  local cur_noz = tonumber(config["nozzle_temperature"])
  if cur_noz then
    set_like(config, "nozzle_temperature", tostring(math.min(noz_max, cur_noz + c.boost)))
  end
  local cur_init = tonumber(config["nozzle_temperature_initial_layer"])
  if cur_init then
    set_like(config, "nozzle_temperature_initial_layer", tostring(math.min(noz_max, cur_init + c.boost)))
  end
  local cur_fan = tonumber(config["fan_max_speed"])
  if cur_fan then
    set_like(config, "fan_max_speed", tostring(math.floor(cur_fan * (100 - c.fan_cut) / 100.0 + 0.5)))
  end

  -- Process-level structure
  config["wall_loops"] = tostring(c.walls)
  config["alternate_extra_wall"] = c.alt_wall
  config["top_shell_layers"] = tostring(c.shells)
  config["bottom_shell_layers"] = tostring(c.shells)
  local lh = tonumber(config["layer_height"]) or tonumber(ctx and ctx.layer_height) or 0.20
  local shell_mm = string.format("%.2f", c.shells * lh)
  if config["top_shell_thickness"] ~= nil then
    config["top_shell_thickness"] = shell_mm
  end
  if config["bottom_shell_thickness"] ~= nil then
    config["bottom_shell_thickness"] = shell_mm
  end
  config["infill_wall_overlap"] = string.format("%d%%", c.overlap)
  config["sparse_infill_pattern"] = c.pattern
  sync_coupled_physics(config, ctx)

  local d = describe(p or {}, ctx)
  print("[Pack:StructuralStrength] " .. (d.badge or ""))
end
