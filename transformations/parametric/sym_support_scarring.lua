-- Symptom Fix: Support Interface Scarring or Fused / Hard-to-Remove Supports
meta = {
  id = "sym_support_scarring",
  name = "Support Interface Scarring or Fused / Hard-to-Remove Supports",
  icon = "🌳",
  category = "6. Overhangs, Bridging & Supports (Support Z-Distance)",
  tree_category = "6. Overhangs, Bridging & Supports",
  tree_subcategory = "Support Top Z-Gap & Interface",
  slicer_menu_path = "Process → Support → Relative Top Z Distance Offset (+0.02 to +0.08mm) + Support Interface Layers + Tree Supports",
  when_to_run = "Apply when supports weld permanently onto the part (especially PETG/PCTG), require pliers that break the model, or leave ugly scarred undersides.",
  formula_summary = "Support_Z_Gap_new = Support_Z_Gap_old + Z_Gap_Offset  •  support_interface_top_layers = 3  •  support_type = tree(auto)",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. When supporting a part with the SAME material (especially sticky PETG or PCTG), a tight `support_top_z_distance` fuses the support interface directly to the part.",
    "2. Adding +0.04mm to +0.06mm relative to your current `support_top_z_distance` (or `layer_height`) with 3 support interface layers (`support_interface_top_layers = 3`) lets supports pop off cleanly by hand on any nozzle size!",
  },
  hardware_checklist = {
    "💡 Pro Tip — Dissimilar Support Interface (PLA ↔ PETG): With an ACE Pro, you can assign PLA as the Support Interface filament for a PETG/PCTG print (or vice versa) with `support_top_z_distance = 0.00mm` — they don't bond to each other and peel off with a mirror-smooth underside!",
  },
}

params = {
  {
    id = "top_z_dist",
    label = "1. How much extra Support Top/Bottom Z-Distance (support_top_z_distance, mm) should we add between the support roof and the model?",
    description = "+0.04mm relative to your current Z-gap (or layer height) is ideal for sticky PETG/PCTG so supports snap off cleanly across 0.2mm–0.8mm nozzles.",
    type = "range",
    min = 0.00,
    max = 0.25,
    step = 0.02,
    default = "+0.04",
    unit = "mm",
    options = {
      { value = "+0.02", label = "+0.02 mm Extra Z-Gap (PLA Clean Release)" },
      { value = "+0.04", label = "+0.04 mm Extra Z-Gap (Recommended for PETG / PCTG)" },
      { value = "+0.08", label = "+0.08 mm Extra Z-Gap (Easy Peel)" },
      { value = "0.00", label = "0.00 mm Zero Z-Gap (ONLY for Dissimilar PLA↔PETG Interface!)" },
    },
  },
  {
    id = "sup_style",
    label = "2. Which Support Structure Type (support_type) should be used?",
    description = "tree(auto) branches around the model without resting on lower model surfaces.",
    type = "enum",
    default = "tree(auto)",
    unit = "",
    options = {
      { value = "tree(auto)", label = "tree(auto) — Organic Tree Supports (Recommended)" },
      { value = "normal(auto)", label = "normal(auto) — Snug Grid Supports" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local zd_raw = p.top_z_dist or "+0.04"
  local is_zero = (zd_raw == "0.00" or zd_raw == "0")
  local zd_offset = tonumber(zd_raw) or 0.04
  local st = p.sup_style or "tree(auto)"
  local scope = (ctx and ctx.scope) or "Selected Profile"
  if is_zero then
    local badge = string.format("support_top_z_distance = 0.00mm (Dissimilar Interface) • Type = %s • 3 Interface Layers", st)
    return {
      title = string.format("Symptom Fix: Zero-Gap Dissimilar Supports (0.00mm, %s)", st),
      description = string.format("Configures zero-gap dissimilar support interface for %s: sets support_top_z_distance = 0.00mm, support_type = %s, and 3 interface layers.", scope, st),
      badge = badge,
    }
  else
    local badge = string.format("Support Z-Distance %+.2fmm • Type = %s • 3 Interface Layers", zd_offset, st)
    return {
      title = string.format("Symptom Fix: Easy-Release Supports (%+.2fmm Z-Gap, %s)", zd_offset, st),
      description = string.format("Prevents supports from fusing to the model for %s: adds %+.2fmm to support_top_z_distance (and bottom Z-distance), sets support_type = %s, and adds 3 interface layers.", scope, zd_offset, st),
      badge = badge,
    }
  end
end

function apply(config, ctx, p)
  local zd_raw = (p and p.top_z_dist) or "+0.04"
  local is_zero = (zd_raw == "0.00" or zd_raw == "0")
  local zd_offset = tonumber(zd_raw) or 0.04
  local st = (p and p.sup_style) or "tree(auto)"

  if is_zero then
    config["support_top_z_distance"] = "0.00"
    config["support_bottom_z_distance"] = "0.00"
  else
    local lh = tonumber(config["layer_height"]) or 0.20
    local cur_zd = tonumber(config["support_top_z_distance"]) or lh
    local new_zd = string.format("%.2f", cur_zd + zd_offset)
    config["support_top_z_distance"] = new_zd
    config["support_bottom_z_distance"] = new_zd
  end
  config["support_type"] = st
  config["support_interface_top_layers"] = "3"
  local d = describe(p or {}, ctx)
  print("[Symptom:Supports] " .. (d.badge or ""))
end
