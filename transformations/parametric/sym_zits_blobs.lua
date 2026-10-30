-- Symptom Fix: Blobs, Zits & Pimples Scattered on Outer Walls
meta = {
  id = "sym_zits_blobs",
  name = "Blobs, Zits & Pimples Scattered on Outer Walls",
  icon = "🫧",
  category = "2. Oozing, Stringing & Blobs (Surface Blemishes)",
  tree_category = "2. Oozing, Stringing & Blobs",
  tree_subcategory = "Outer Wall Blemishes",
  slicer_menu_path = "Seam Position (Aligned) + Scarf Seam + Relative Deretraction Speed Scaling + Wipe on Loops",
  when_to_run = "Apply when small pimples, zits, or raised bumps appear across smooth curved surfaces or at layer start/stop points.",
  formula_summary = "seam_position = aligned  •  Scarf Seam = contour  •  Deretract_new = Deretract_old × (1 - Slowdown%)  •  seam_gap = 10%",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Check where the zits appear: if they are scattered randomly all over the model, `seam_position` is set to 'random' OR printer Power-Loss Recovery is pausing briefly to write to flash.",
    "2. Aligning seams (`aligned`), enabling Scarf Joint ramp (`contour` or `all`), and scaling deretraction reload speed by -25% (0.75×) eliminates start/stop pimples.",
  },
  hardware_checklist = {
    "⚡ Check Power-Loss Recovery (PLR) Pause Blobs: If zits appear in a spiral pattern on Vase Mode or curved cylinders, firmware SD-card state saving is briefly pausing the toolhead at each layer.",
    "🔥 Check for Moisture Popping: Random crater-like zits halfway along a wall (away from the Z-seam) are burst steam bubbles from wet filament.",
  },
}

params = {
  {
    id = "seam_pos",
    label = "1. Where should the Z-Seam be placed (seam_position)?",
    description = "'aligned' or 'back' groups all layer starts into a single hidden corner instead of scattering random pimples.",
    type = "enum",
    default = "aligned",
    unit = "",
    options = {
      { value = "aligned", label = "aligned (Sharpest Corner — Recommended)" },
      { value = "back", label = "back (Rear of Model)" },
      { value = "nearest", label = "nearest (Fastest Travel)" },
    },
  },
  {
    id = "deretract_spd",
    label = "2. By what percentage should we slow Deretraction (Reload) Speed to stop pressure burps at layer start?",
    description = "Scaling deretraction speed by -25% (0.75× of your current reload speed) reloads the nozzle smoothly without spitting a zit at the start of the perimeter.",
    type = "range",
    min = -60,
    max = 0,
    step = 5,
    default = "-25%",
    unit = "%",
    options = {
      { value = "-15%", label = "-15% (0.85× Moderate Reload Slowdown)" },
      { value = "-25%", label = "-25% (0.75× Recommended Anti-Zit)" },
      { value = "-35%", label = "-35% (0.65× Maximum Anti-Zit Smoothness)" },
    },
  },
  {
    id = "enable_scarf",
    label = "3. Enable Scarf Joint Seam Ramp to feather the loop start/end?",
    description = "Scarf joints ramp layer height over 20mm so there is no step or pimple at loop closure.",
    type = "enum",
    default = "external",
    unit = "",
    options = {
      { value = "external", label = "Contour (Outer Walls — Recommended)" },
      { value = "all", label = "Contour and Hole (Outer + Inner Walls)" },
      { value = "none", label = "None (Standard Seam)" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local seam_pos = p.seam_pos or "aligned"
  local deret_pct = tonumber(p.deretract_spd) or -25.0
  local d_scale = 0.75
  if deret_pct < 0 then
    d_scale = math.max(0.40, math.min(1.0, (100.0 + deret_pct) / 100.0))
  else
    d_scale = math.max(0.40, math.min(1.0, (100.0 - deret_pct) / 100.0))
  end
  local drop_label = math.floor((1.0 - d_scale) * 100.0 + 0.5)
  local scarf = p.enable_scarf or "external"
  if scarf == "contour" then scarf = "external" end
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Seam=%s • Deretract -%.0f%% (%.2f×) • Scarf=%s • Wipe on Loops", seam_pos, drop_label, d_scale, scarf)
  return {
    title = string.format("Symptom Fix: Wall Zits & Blobs (Seam=%s, Deretract -%.0f%%)", seam_pos, drop_label),
    description = string.format("Eliminates pimples and zits on outer walls for %s: sets seam_position=%s, scales deretraction speed by %.2f× (-%.0f%%), scarf=%s, and wipe_on_loops=1.", scope, seam_pos, d_scale, drop_label, scarf),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local seam_pos = (p and p.seam_pos) or "aligned"
  local deret_pct = tonumber(p and p.deretract_spd) or -25.0
  local d_scale = 0.75
  if deret_pct < 0 then
    d_scale = math.max(0.40, math.min(1.0, (100.0 + deret_pct) / 100.0))
  else
    d_scale = math.max(0.40, math.min(1.0, (100.0 - deret_pct) / 100.0))
  end
  local scarf = (p and p.enable_scarf) or "external"
  if scarf == "contour" then scarf = "external" end

  config["seam_position"] = seam_pos
  local cur_d = tonumber(config["filament_deretraction_speed"]) or 35
  config["filament_deretraction_speed"] = tostring(math.max(15, math.floor(cur_d * d_scale + 0.5)))
  config["seam_slope_type"] = scarf
  local cur_sl = tonumber(config["seam_slope_min_length"]) or 15
  config["seam_slope_min_length"] = tostring(math.floor(cur_sl * 1.25 + 0.5))
  config["seam_gap"] = "10%"
  config["wipe_on_loops"] = "1"
  local d = describe(p or {}, ctx)
  print("[Symptom:ZitsBlobs] " .. (d.badge or ""))
end
