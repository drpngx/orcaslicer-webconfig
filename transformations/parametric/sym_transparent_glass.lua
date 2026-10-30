-- Symptom Fix: Transparent / Glass-Like Clarity Tuning (Slow Speed, 0% Fan, High Flow & Aligned Infill)
meta = {
  id = "sym_transparent_glass",
  name = "Transparent / Glass-Like Clarity Tuning (Slow Speed, 0% Fan, High Flow & Aligned Infill)",
  icon = "💎",
  category = "5. Top/Bottom Surfaces & Infill (Optical Clarity & Glass Mode)",
  tree_category = "5. Top/Bottom Surfaces & Infill",
  tree_subcategory = "Optical Clarity & Glass Mode",
  slicer_menu_path = "Speed (15–30 mm/s) + Flow Boost (+5% Void Fill) + 0% Part Cooling Fan + Aligned Rectilinear 0° Infill + 65°C PEI Bed",
  when_to_run = "Apply when printing Translucent / Transparent PETG, PCTG, or PLA to achieve ice-clear or glass-like optical clarity instead of frosty/cloudy internal refraction, while keeping stringing and Textured PEI adhesion under control.",
  formula_summary = "Speed = 15–30 mm/s (MVS ≤ 5–7 mm³/s)  •  Flow_new = Flow_old + 0.05  •  Fan = 0%  •  100% Aligned Rectilinear (0°)  •  PEI Bed = 65°C",
  default_condition_expr = "filament:PETG",
  how_to_read_print = {
    "1. Why translucent prints look cloudy/white: FDM prints look frosted because of thousands of microscopic air gaps between adjacent extrusion lines, criss-crossing infill angles (90° grid refraction), and rapid fan cooling haze.",
    "2. The Rygar1432 ('How to Print Glass') & CNC Kitchen formula: print ultra-slow (15–20 mm/s, ≤5.0 mm³/s MVS), boost Flow Ratio by +0.05 (+5% → ~1.01–1.02) so molten plastic slightly over-packs and floods every inter-bead void, turn Part Cooling Fan OFF (0%), and lay all lines in one parallel direction (`alignedrectilinear` at 0° with 0 top/bottom shells) or Spiral Vase mode.",
    "3. Textured PEI Release & Stringing: Un-pigmented clear PETG lacks TiO2 mineral filler and bonds aggressively to PEI powder-coat; keep Textured PEI at 65°C with 0°C first-layer nozzle boost and let the plate cool to room temperature (<30°C) before flexing.",
  },
  hardware_checklist = {
    "🔥 Dry Clear PETG/PCTG in ACE Pro at 55°C for 6+ Hours First: Even 0.1% absorbed moisture flashes into microscopic steam bubbles inside the melt zone, turning crystal-clear PETG permanently milky/cloudy regardless of slicer settings.",
    "🧼 Clean Nozzle Exterior Before Printing: With +5% over-pack flow (`~1.01–1.02`), any stray plastic film on the nozzle cone can accumulate; brush the hot nozzle tip clean before starting and ensure `avoid_crossing_perimeters = 1` is active.",
    "❄️ Textured PEI Release Tip: Wait until the Textured PEI build plate cools below 30°C (or place the spring-steel sheet in the freezer for 3 minutes) so clear PETG pops off effortlessly without stressing the PEI coating.",
  },
}

params = {
  {
    id = "optical_mode",
    label = "1. Which Transparent / Optical Clarity geometry mode are you printing?",
    description = "'glass_solid' uses 100% Aligned Rectilinear infill at 0° with 1 wall and 0 top/bottom shells (Rygar1432 / CNC Kitchen solid optical glass). 'clear_shell' keeps 2 walls + monotonic top/bottom shells for functional clear parts. 'vase_crystal' configures single-wall Spiral Vase mode with 1.50× line width for lampshades and diffusers.",
    type = "enum",
    default = "glass_solid",
    unit = "",
    options = {
      { value = "glass_solid", label = "glass_solid — 100% Aligned Rectilinear Glass Block (1 Wall, 0 Top/Bottom Shells — Rygar1432 Spec)" },
      { value = "clear_shell", label = "clear_shell — Clear Functional Part (2 Walls, Monotonic Shells, Aligned Infill)" },
      { value = "vase_crystal", label = "vase_crystal — Crystal Spiral Vase / Light Diffuser (1 Wall, 0% Fan, 1.50× Width)" },
    },
  },
  {
    id = "print_speed",
    label = "2. What Print Speed (mm/s) should be used for walls and infill?",
    description = "15–20 mm/s lets molten PETG/PCTG relax and Degas at the nozzle tip without trapping micro-bubbles or shear haze.",
    type = "range",
    min = 10,
    max = 60,
    step = 5,
    default = "20",
    unit = "mm/s",
    options = {
      { value = "15", label = "15 mm/s (Ultra-Clear Optical Glass — Rygar1432 Spec)" },
      { value = "20", label = "20 mm/s (Recommended Glass Clarity)" },
      { value = "30", label = "30 mm/s (Balanced Translucent Functional Part)" },
    },
  },
  {
    id = "flow_boost",
    label = "3. How much should we boost Flow Ratio (filament_flow_ratio) relative to current to flood inter-bead air voids?",
    description = "+0.05 (+5%, e.g. 0.96 → 1.01) forces extruded plastic into the microscopic triangular valleys between parallel lines, eliminating internal light refraction.",
    type = "range",
    min = 0.00,
    max = 0.12,
    step = 0.01,
    default = "+0.05",
    unit = "ratio",
    options = {
      { value = "+0.03", label = "+0.03 (+3% Subtle Over-Pack — Functional Clear Shells)" },
      { value = "+0.05", label = "+0.05 (+5% Recommended Void Fill — ~1.01 Flow)" },
      { value = "+0.07", label = "+0.07 (+7% Maximum Optical Void Fill — ~1.03 Flow)" },
    },
  },
  {
    id = "fan_cap",
    label = "4. What Part Cooling Fan Speed (%) should be used?",
    description = "0% (Fan OFF) allows adjacent lines to thermally fuse into a homogeneous clear mass without cooling stress haze. Use 15–25% only if the model has overhangs.",
    type = "range",
    min = 0,
    max = 50,
    step = 5,
    default = "0",
    unit = "%",
    options = {
      { value = "0", label = "0% (Fan Completely OFF — Maximum Clarity & Slow Annealing)" },
      { value = "15", label = "15% (Minimal Cooling for Mild Overhangs)" },
      { value = "25", label = "25% (Functional Translucent Parts with Bridges)" },
    },
  },
  {
    id = "bed_temp_pei",
    label = "5. What Textured PEI Bed Temperature (°C) should we set for easy release without tearing?",
    description = "Clear un-pigmented PETG bonds much harder to Textured PEI than opaque PETG. 65°C holds securely during slow printing and releases cleanly once cooled below 30°C.",
    type = "range",
    min = 50,
    max = 85,
    step = 1,
    default = "65",
    unit = "°C",
    options = {
      { value = "62", label = "62°C (Maximum Release Relief on Textured PEI)" },
      { value = "65", label = "65°C (Recommended Textured PEI Easy-Release for Clear PETG)" },
      { value = "70", label = "70°C (Standard Smooth / Engineering Plate with Glue Stick)" },
    },
  },
}

function validate(p, ctx)
  local mode = p.optical_mode or "glass_solid"
  local spd = tonumber(p.print_speed) or 20
  local fan = tonumber(p.fan_cap) or 0
  if mode == "glass_solid" and spd > 35 then
    return false, "In glass_solid optical mode, print_speed above 35 mm/s traps micro-bubbles and causes shear cloudiness. Use 15–30 mm/s."
  end
  if mode == "glass_solid" and fan > 25 then
    return false, "In glass_solid optical mode, fan_cap above 25% causes rapid cooling haze between layers. Use 0%–25%."
  end
  return true
end

function describe(p, ctx)
  local mode = p.optical_mode or "glass_solid"
  local spd = math.max(10, math.min(60, math.floor(tonumber(p.print_speed) or 20)))
  local flow_boost = math.max(0.0, math.min(0.12, tonumber(p.flow_boost) or 0.05))
  local fan_cap = math.max(0, math.min(100, math.floor(tonumber(p.fan_cap) or 0)))
  local bed_pei = math.max(50, math.min(90, math.floor(tonumber(p.bed_temp_pei) or 65)))
  local mvs = 7.0
  if spd <= 15 then
    mvs = 4.0
  elseif spd <= 20 then
    mvs = 5.0
  end
  local mode_label = "100% Aligned Rectilinear Glass (1 Wall, 0 Shells)"
  if mode == "vase_crystal" then
    mode_label = "Crystal Spiral Vase (1 Wall, 1.50× Width)"
  elseif mode == "clear_shell" then
    mode_label = "Clear Functional Shell (2 Walls, Monotonic)"
  end
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("%s • %d mm/s (MVS %.1f mm³/s) • Flow %+.2f • Fan %d%% • PEI Bed %d°C", mode_label, spd, mvs, flow_boost, fan_cap, bed_pei)
  return {
    title = string.format("Optical Clarity: %s (%dmm/s, %+.2f Flow, %d%% Fan)", mode_label, spd, flow_boost, fan_cap),
    description = string.format("Tunes %s for transparent / glass-like optical clarity (%s): sets print speeds to %d mm/s (%.1f mm³/s MVS ceiling), boosts filament_flow_ratio by %+.2f to flood inter-bead voids, caps part cooling fan at %d%%, and sets Textured PEI bed to %d°C with anti-stringing retraction.", scope, mode_label, spd, mvs, flow_boost, fan_cap, bed_pei),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local mode = (p and p.optical_mode) or "glass_solid"
  local spd = math.max(10, math.min(60, math.floor(tonumber(p and p.print_speed) or 20)))
  local init_spd = math.max(12, spd - 5)
  local flow_boost = math.max(0.0, math.min(0.12, tonumber(p and p.flow_boost) or 0.05))
  local fan_cap = math.max(0, math.min(100, math.floor(tonumber(p and p.fan_cap) or 0)))
  local bed_pei = math.max(50, math.min(90, math.floor(tonumber(p and p.bed_temp_pei) or 65)))
  local mvs = 7.0
  if spd <= 15 then
    mvs = 4.0
  elseif spd <= 20 then
    mvs = 5.0
  end

  local is_filament = (config["type"] == "filament") or (config["filament_flow_ratio"] ~= nil) or (config["nozzle_temperature"] ~= nil)
  local is_process = (config["type"] == "process") or (config["sparse_infill_speed"] ~= nil) or (config["wall_loops"] ~= nil)

  if is_filament then
    local cur_flow = tonumber(config["filament_flow_ratio"]) or 0.96
    config["filament_flow_ratio"] = string.format("%.3f", math.min(1.06, cur_flow + flow_boost))
    config["fan_max_speed"] = tostring(fan_cap)
    config["fan_min_speed"] = tostring(fan_cap)
    if config["fan_max_speed_BRASS"] ~= nil then config["fan_max_speed_BRASS"] = tostring(fan_cap) end
    if config["fan_max_speed_HS"] ~= nil then config["fan_max_speed_HS"] = tostring(fan_cap) end
    config["slow_down_for_layer_cooling"] = "0"
    if fan_cap == 0 then
      config["overhang_fan_speed"] = "25"
    end
    config["filament_max_volumetric_speed"] = string.format("%.1f", mvs)
    config["textured_plate_temp"] = tostring(bed_pei)
    config["textured_plate_temp_initial_layer"] = tostring(bed_pei)
    config["hot_plate_temp"] = tostring(bed_pei)
    config["hot_plate_temp_initial_layer"] = tostring(bed_pei)
    config["filament_retraction_length"] = "0.8"
    config["filament_retraction_speed"] = "35"
    config["filament_deretraction_speed"] = "30"
    config["filament_retract_before_wipe"] = "70%"
    config["filament_wipe"] = "1"
    config["filament_wipe_distance"] = "2"
    config["filament_z_hop"] = "0.10"
    config["filament_z_hop_types"] = "Spiral Lift"
  end

  if is_process then
    local nd = tonumber(ctx and ctx.nozzle_diameter) or 0.4
    if mode == "vase_crystal" then
      local lw = string.format("%.2f", nd * 1.50)
      config["spiral_mode"] = "1"
      config["wall_loops"] = "1"
      config["top_shell_layers"] = "0"
      config["bottom_shell_layers"] = "3"
      config["sparse_infill_density"] = "0%"
      config["line_width"] = lw
      config["outer_wall_line_width"] = lw
    elseif mode == "clear_shell" then
      local lw = string.format("%.2f", nd * 1.25)
      config["wall_loops"] = "2"
      config["top_surface_pattern"] = "monotonicline"
      config["bottom_surface_pattern"] = "monotonicline"
      config["sparse_infill_pattern"] = "alignedrectilinear"
      config["infill_direction"] = "0"
      config["infill_wall_overlap"] = "25%"
      config["line_width"] = lw
      config["outer_wall_line_width"] = lw
      config["inner_wall_line_width"] = lw
      config["sparse_infill_line_width"] = lw
    else
      local lw = string.format("%.2f", nd * 1.25)
      local lh = string.format("%.2f", math.max(0.10, math.floor(nd * 0.30 * 100 + 0.5) / 100))
      config["layer_height"] = lh
      config["initial_layer_print_height"] = string.format("%.2f", math.min(0.20, nd * 0.50))
      config["wall_loops"] = "1"
      config["top_shell_layers"] = "0"
      config["bottom_shell_layers"] = "0"
      config["sparse_infill_density"] = "100%"
      config["sparse_infill_pattern"] = "alignedrectilinear"
      config["internal_solid_infill_pattern"] = "monotonicline"
      config["top_surface_pattern"] = "monotonicline"
      config["bottom_surface_pattern"] = "monotonicline"
      config["infill_direction"] = "0"
      config["infill_wall_overlap"] = "25%"
      config["line_width"] = lw
      config["outer_wall_line_width"] = lw
      config["inner_wall_line_width"] = lw
      config["sparse_infill_line_width"] = lw
      config["internal_solid_infill_line_width"] = lw
      config["top_surface_line_width"] = lw
    end
    config["outer_wall_speed"] = tostring(spd)
    config["inner_wall_speed"] = tostring(spd)
    config["sparse_infill_speed"] = tostring(spd)
    config["internal_solid_infill_speed"] = tostring(spd)
    config["top_surface_speed"] = tostring(spd)
    config["gap_infill_speed"] = tostring(spd)
    config["initial_layer_speed"] = tostring(init_spd)
    config["initial_layer_infill_speed"] = tostring(init_spd)
    config["reduce_crossing_wall"] = "1"
    config["avoid_crossing_perimeters"] = "1"
  end

  local d = describe(p or {}, ctx)
  print("[OpticalClarity] " .. (d.badge or ""))
end
