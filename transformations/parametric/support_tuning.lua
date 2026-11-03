-- Core Slicer Config Pack #1: Support Detachability & Tree Tuning
-- Layer-height & polymer-aware `support_top_z_distance` (`layer_height + 0.02mm` for PLA vs.
-- `layer_height + 0.06mm` for sticky PETG/PCTG/TPU so supports snap off cleanly),
-- `support_type` (`tree(auto)` vs. `normal(auto)`), `support_style` (`tree_slim`, `tree_hybrid`, `snug`),
-- `support_interface_top_layers`, `support_interface_spacing`, and `support_interface_speed`.

meta = {
  id = "support_tuning",
  name = "Support Detachability & Tree Tuning Pack",
  icon = "🏔️",
  category = "Core Slicer Config Pack (Supports & Interface)",
  tree_category = "6. Overhangs, Bridging & Supports",
  tree_subcategory = "Support Detachability & Tree Geometry",
  slicer_menu_path = "Process → Support → Type (`tree(auto)` / `normal(auto)`) + Style (`tree_slim` / `tree_hybrid` / `snug`) + Polymer-Aware Top Z-Distance + Interface Layers/Spacing/Speed",
  when_to_run = "Enable whenever slicing models with overhangs that require supports, or to automatically adjust support Z-gap between PLA (+0.02mm) and sticky PETG/PCTG/TPU (+0.06mm).",
  formula_summary = "support_top_z_distance = layer_height + Polymer_Z_Offset (+0.02mm PLA / +0.06mm PETG·PCTG·TPU / 0.00mm Dissimilar)  •  Interface = 3 layers @ 0.15mm spacing",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Same-material supports weld onto sticky polymers (PETG, PCTG, TPU, PA) when `support_top_z_distance` equals a single layer height. Adding +0.06mm over `layer_height` with a 3-layer concentric/rectilinear roof (`support_interface_spacing = 0.15mm`) lets supports pop off cleanly by hand.",
    "2. Rigid, low-tack polymers (PLA, Matte PLA, ABS, ASA) release cleanly at `layer_height + 0.02mm`, preserving a flatter underside without droop.",
    "3. Because `support_top_z_distance` is computed relative to each exported Process profile's `layer_height`, `0.12mm HQ`, `0.20mm Std`, and `0.28mm Speed` each receive their own exact Z-gap automatically on export!",
  },
  hardware_checklist = {
    "💡 Multi-Material Zero-Gap Trick (ACE Pro / AMS): If you assign PLA as the Support Interface filament for a PETG/PCTG model (or vice versa), select the 'Zero-Gap Dissimilar Interface (0.00mm)' preset for mirror-smooth undersides.",
  },
}

params = {
  {
    id = "support_preset",
    label = "1. Support Detachability & Geometry Preset (Snaps Dials Below)",
    description = "Choose Polymer-Aware Auto-Tuned (+0.02mm PLA / +0.06mm PETG·PCTG·TPU), Slim Organic Tree, Hybrid Heavy-Duty Tree, Snug Mechanical Grid, or 0.00mm Dissimilar Interface.",
    type = "enum",
    default = "auto_polymer",
    options = {
      {
        value = "auto_polymer",
        label = "🧬 Polymer & Layer-Height Auto-Tuned (+0.02mm PLA / +0.06mm PETG·PCTG·TPU • Tree Slim)",
        sets = {
          support_type = "tree(auto)",
          support_style = "tree_slim",
          top_z_offset_mm = "0.04",
          interface_top_layers = "3",
          interface_spacing_mm = "0.15",
          interface_speed_mms = "60",
        },
      },
      {
        value = "tree_easy_snap",
        label = "🌳 Organic Tree — Easy Snap-Off (+0.06mm Z-Gap • tree_slim • 0.18mm Spacing)",
        sets = {
          support_type = "tree(auto)",
          support_style = "tree_slim",
          top_z_offset_mm = "0.06",
          interface_top_layers = "3",
          interface_spacing_mm = "0.18",
          interface_speed_mms = "60",
        },
      },
      {
        value = "tree_heavy_duty",
        label = "🌲 Hybrid Tree — Large Flat Overhangs (+0.03mm Z-Gap • tree_hybrid • 0.12mm Spacing)",
        sets = {
          support_type = "tree(auto)",
          support_style = "tree_hybrid",
          top_z_offset_mm = "0.03",
          interface_top_layers = "3",
          interface_spacing_mm = "0.12",
          interface_speed_mms = "55",
        },
      },
      {
        value = "snug_grid_mechanical",
        label = "📐 Snug Normal — Precision Mechanical Ledges (+0.04mm Z-Gap • normal(auto) + snug)",
        sets = {
          support_type = "normal(auto)",
          support_style = "snug",
          top_z_offset_mm = "0.04",
          interface_top_layers = "3",
          interface_spacing_mm = "0.15",
          interface_speed_mms = "60",
        },
      },
      {
        value = "zero_gap_dissimilar",
        label = "🪞 Zero-Gap Dissimilar Interface (0.00mm Z-Gap • PLA↔PETG Multi-Material Only)",
        sets = {
          support_type = "tree(auto)",
          support_style = "tree_hybrid",
          top_z_offset_mm = "0.00",
          interface_top_layers = "3",
          interface_spacing_mm = "0.00",
          interface_speed_mms = "45",
        },
      },
      {
        value = "custom",
        label = "🎛️ Custom Support Geometry & Z-Gap Dials",
      },
    },
  },
  {
    id = "support_type",
    label = "2. Support Structure Type (support_type)",
    description = "tree(auto) grows organic trunks around the part; normal(auto) builds vertical columns directly beneath overhangs.",
    type = "enum",
    default = "tree(auto)",
    options = {
      { value = "tree(auto)", label = "tree(auto) — Automatic Organic Tree Supports" },
      { value = "normal(auto)", label = "normal(auto) — Automatic Normal Supports" },
      { value = "tree(manual)", label = "tree(manual) — Painted Manual Tree Supports" },
      { value = "normal(manual)", label = "normal(manual) — Painted Manual Normal Supports" },
    },
  },
  {
    id = "support_style",
    label = "3. Support Branch / Column Style (support_style)",
    description = "tree_slim saves filament and avoids body contact; tree_hybrid merges lattice with trunks for flat roofs; snug hugs vertical walls tightly.",
    type = "enum",
    default = "tree_slim",
    options = {
      { value = "tree_slim", label = "tree_slim — Slim Organic Branches (Low Material)" },
      { value = "tree_hybrid", label = "tree_hybrid — Hybrid Tree + Lattice (Flat Roofs)" },
      { value = "tree_strong", label = "tree_strong — Thick Reinforced Tree Trunks" },
      { value = "snug", label = "snug — Snug Normal Columns (No Footprint Spill)" },
      { value = "grid", label = "grid — Classic Normal Grid" },
      { value = "default", label = "default — Slicer Auto Style" },
    },
  },
  {
    id = "top_z_offset_mm",
    label = "4. Extra Top/Bottom Z-Distance Offset Above Layer Height (mm)",
    description = "Added on top of `layer_height` (e.g. 0.20mm + 0.06mm = 0.26mm). In 'auto_polymer' mode, PLA uses +0.02mm and sticky PETG/PCTG/TPU uses +0.06mm automatically.",
    type = "range",
    min = 0.00,
    max = 0.16,
    step = 0.01,
    default = "0.04",
    unit = "mm",
    options = {
      { value = "0.00", label = "0.00 mm (Zero Z-Gap — Dissimilar PLA↔PETG Only)" },
      { value = "0.02", label = "+0.02 mm (PLA / ABS / ASA Crisp Release)" },
      { value = "0.04", label = "+0.04 mm (Balanced All-Rounder)" },
      { value = "0.06", label = "+0.06 mm (Sticky PETG / PCTG / TPU Clean Snap)" },
      { value = "0.08", label = "+0.08 mm (Effortless Hand Removal)" },
    },
  },
  {
    id = "interface_top_layers",
    label = "5. Support Interface Top Layers (support_interface_top_layers)",
    description = "Dense roof layers at the tip of the support that form a smooth cushion beneath the model.",
    type = "range",
    min = 1,
    max = 5,
    step = 1,
    default = "3",
    unit = "layers",
    options = {
      { value = "2", label = "2 Interface Layers (Fast)" },
      { value = "3", label = "3 Interface Layers (Recommended Dense Roof)" },
      { value = "4", label = "4 Interface Layers (Ultra-Rigid Roof)" },
    },
  },
  {
    id = "interface_spacing_mm",
    label = "6. Support Interface Line Spacing (support_interface_spacing, mm)",
    description = "Spacing between support roof lines: 0.00mm = 100% solid roof (best for dissimilar PLA↔PETG); 0.15–0.20mm = easy peel for same-material supports.",
    type = "range",
    min = 0.00,
    max = 0.40,
    step = 0.02,
    default = "0.15",
    unit = "mm",
    options = {
      { value = "0.00", label = "0.00 mm (100% Solid Roof — Dissimilar Interface)" },
      { value = "0.12", label = "0.12 mm (Dense Roof)" },
      { value = "0.15", label = "0.15 mm (Recommended Easy-Release Roof)" },
      { value = "0.20", label = "0.20 mm (Light Contact Roof)" },
    },
  },
  {
    id = "interface_speed_mms",
    label = "7. Support Interface Print Speed (support_interface_speed, mm/s)",
    description = "Slower support roof speed ensures flat, uncurled interface strands before the overhang layer bridges across them.",
    type = "range",
    min = 20,
    max = 120,
    step = 5,
    default = "60",
    unit = "mm/s",
    options = {
      { value = "45", label = "45 mm/s (Precision Slow Roof)" },
      { value = "60", label = "60 mm/s (Recommended)" },
      { value = "80", label = "80 mm/s (Fast)" },
    },
  },
}

local function resolve_effective_z_offset(p, ctx)
  local preset = (p and p.support_preset) or "auto_polymer"
  if preset == "zero_gap_dissimilar" then
    return { z_off = 0.00, is_zero = true, mode_tag = "Dissimilar 0.00mm" }
  end
  local raw_off = tonumber(p and p.top_z_offset_mm) or 0.04
  if preset == "auto_polymer" then
    local fil = string.upper(tostring((ctx and ctx.filament_type) or "PETG"))
    if string.find(fil, "PETG") or string.find(fil, "PCTG") or string.find(fil, "TPU") or string.find(fil, "PA") or string.find(fil, "CPE") then
      return { z_off = 0.06, is_zero = false, mode_tag = string.format("Auto %s (+0.06mm)", fil) }
    elseif string.find(fil, "PLA") or string.find(fil, "ABS") or string.find(fil, "ASA") then
      return { z_off = 0.02, is_zero = false, mode_tag = string.format("Auto %s (+0.02mm)", fil) }
    else
      return { z_off = 0.04, is_zero = false, mode_tag = string.format("Auto %s (+0.04mm)", fil) }
    end
  end
  if raw_off <= 0.001 then
    return { z_off = 0.00, is_zero = true, mode_tag = "Zero-Gap 0.00mm" }
  end
  return { z_off = raw_off, is_zero = false, mode_tag = string.format("Custom %+.2fmm", raw_off) }
end

function validate(p, ctx)
  local stype = (p and p.support_type) or "tree(auto)"
  local sstyle = (p and p.support_style) or "tree_slim"
  local is_tree_type = string.find(stype, "tree") ~= nil
  local is_tree_style = (sstyle == "tree_slim" or sstyle == "tree_hybrid" or sstyle == "tree_strong")
  local is_normal_style = (sstyle == "snug" or sstyle == "grid")

  if is_tree_type and is_normal_style then
    return false, string.format("Support type '%s' cannot use normal column style '%s'. Select tree_slim, tree_hybrid, tree_strong, or default.", stype, sstyle)
  end
  if (not is_tree_type) and is_tree_style then
    return false, string.format("Normal support type '%s' cannot use tree branch style '%s'. Select snug, grid, or default.", stype, sstyle)
  end
  return true
end

function describe(p, ctx)
  local stype = (p and p.support_type) or "tree(auto)"
  local sstyle = (p and p.support_style) or "tree_slim"
  local top_layers = math.floor((tonumber(p and p.interface_top_layers) or 3) + 0.5)
  local spacing = tonumber(p and p.interface_spacing_mm) or 0.15
  local spd = math.floor((tonumber(p and p.interface_speed_mms) or 60) + 0.5)
  local eff = resolve_effective_z_offset(p, ctx)
  local z_off = eff.z_off
  local is_zero = eff.is_zero
  local mode_tag = eff.mode_tag
  local lh = tonumber(ctx and ctx.layer_height) or 0.20

  if is_zero then
    local badge = string.format("%s (%s) • Top Z = 0.00mm (Dissimilar) • %d Roof Layers @ %.2fmm / %d mm/s", stype, sstyle, top_layers, spacing, spd)
    return {
      title = string.format("Support Pack: Zero-Gap Dissimilar (%s / %s, 0.00mm Z)", stype, sstyle),
      description = string.format("Configures zero-gap multi-material support interface: support_type = %s (%s), support_top_z_distance = 0.00mm, %d interface layers @ %.2fmm spacing and %d mm/s.", stype, sstyle, top_layers, spacing, spd),
      badge = badge,
    }
  else
    local eff_zd = lh + z_off
    local badge = string.format("%s (%s) • Top Z = %.2fmm (LH %+.2fmm [%s]) • %d Roof Layers @ %.2fmm / %d mm/s", stype, sstyle, eff_zd, z_off, mode_tag, top_layers, spacing, spd)
    return {
      title = string.format("Support Pack: %s (%s, LH %+.2fmm [%s])", stype, sstyle, z_off, mode_tag),
      description = string.format("Layer-height & polymer-aware supports: sets support_type = %s (%s), support_top_z_distance = layer_height %+.2fmm (%.2fmm at %.2fmm LH), %d interface layers @ %.2fmm spacing, and %d mm/s roof speed.", stype, sstyle, z_off, eff_zd, lh, top_layers, spacing, spd),
      badge = badge,
    }
  end
end

function apply(config, ctx, p)
  local stype = (p and p.support_type) or "tree(auto)"
  local sstyle = (p and p.support_style) or "tree_slim"
  local top_layers = math.floor((tonumber(p and p.interface_top_layers) or 3) + 0.5)
  local spacing = tonumber(p and p.interface_spacing_mm) or 0.15
  local spd = math.floor((tonumber(p and p.interface_speed_mms) or 60) + 0.5)
  local eff = resolve_effective_z_offset(p, ctx)
  local z_off = eff.z_off
  local is_zero = eff.is_zero

  local lh = tonumber(config["layer_height"]) or tonumber(ctx and ctx.layer_height) or 0.20
  local nd = tonumber(ctx and ctx.nozzle_diameter) or 0.40
  local preset = (p and p.support_preset) or "auto_polymer"
  local eff_spacing = spacing
  if (not is_zero) and preset == "auto_polymer" and math.abs(nd - 0.40) > 0.01 then
    eff_spacing = math.max(0.10, math.min(0.40, math.floor(spacing * math.sqrt(nd / 0.40) * 100.0 + 0.5) / 100.0))
  end

  config["enable_support"] = "1"
  config["support_type"] = stype
  config["support_style"] = sstyle

  if is_zero then
    config["support_top_z_distance"] = "0.00"
    config["support_bottom_z_distance"] = "0.00"
  else
    local new_zd = fmt_num(lh + z_off, "%.2f")
    config["support_top_z_distance"] = new_zd
    config["support_bottom_z_distance"] = new_zd
  end

  if nd > 0.42 then
    config["support_object_xy_distance"] = fmt_num(math.max(0.35, nd * 0.85), "%.2f")
  end

  config["support_interface_top_layers"] = tostring(top_layers)
  config["support_interface_bottom_layers"] = "2"
  config["support_interface_spacing"] = fmt_num(eff_spacing, "%.2f")
  config["support_interface_speed"] = tostring(spd)

  local d = describe(p or {}, ctx)
  print("[Pack:SupportTuning] " .. (d.badge or ""))
end
