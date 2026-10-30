-- Symptom Fix: Elephant's Foot (Bulging / Flared Outward First Layers)
meta = {
  id = "sym_elephants_foot",
  name = "Elephant's Foot (Bulging / Flared Outward First Layers)",
  icon = "🐘",
  category = "3. First Layer & Bed Adhesion (First-Layer Dimensional)",
  tree_category = "3. First Layer & Bed Adhesion",
  tree_subcategory = "Bottom Lip Inset",
  slicer_menu_path = "Process → Quality → Precision → Relative Elephant Foot Compensation Boost + Bed Temp Trim",
  when_to_run = "Apply when the bottom 1–3 layers of a print flare outward wider than the rest of the wall, preventing mating parts or gears from fitting.",
  formula_summary = "EF_Comp_new = EF_Comp_old + Inset_Boost_mm  •  elefant_foot_compensation_layers = 1  •  Bed Temp {:+}°C",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Elephant's foot occurs when the weight of the print + high first-layer squish + a hot bed (above glass transition Tg) squashes the first layer outward by 0.10–0.25mm.",
    "2. Adding +0.10mm to +0.15mm to `elefant_foot_compensation` insets the first layer contour inward by that exact extra amount so the finished bottom edge is perfectly flush!",
  },
  hardware_checklist = {
    "📏 Re-Run Auto Bed Leveling with Clean Nozzle: If the first layer is squished paper-thin and translucent, the Z-offset is slightly too close to the PEI plate.",
  },
}

params = {
  {
    id = "ef_comp",
    label = "1. How much extra Elephant Foot Compensation inset (elefant_foot_compensation, mm) should we add?",
    description = "Measure the bottom lip flare with calipers and divide by 2 (+0.10 mm to +0.15 mm extra inset fixes most first-layer flare).",
    type = "range",
    min = 0.00,
    max = 0.40,
    step = 0.02,
    default = "+0.10",
    unit = "mm",
    options = {
      { value = "+0.05", label = "+0.05 mm Extra Inset (Subtle)" },
      { value = "+0.10", label = "+0.10 mm Extra Inset (Recommended)" },
      { value = "+0.15", label = "+0.15 mm Extra Inset (Strong Flare Fix)" },
      { value = "+0.20", label = "+0.20 mm Extra Inset (Heavy Squish)" },
    },
  },
  {
    id = "bed_trim",
    label = "2. Lower continuous bed temperature after Layer 1 (°C) relative to current so the base stays rigid?",
    description = "Keeping Layer 1 hot for grip, then dropping continuous bed temp by -5°C stops bottom layers from creeping.",
    type = "range",
    min = -15,
    max = 0,
    step = 1,
    default = "-5",
    unit = "°C",
    options = {
      { value = "0", label = "0°C (Keep Continuous Bed Temp)" },
      { value = "-5", label = "-5°C Continuous Bed (Recommended)" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local ef_boost = tonumber(p.ef_comp) or 0.10
  local b_trim = math.floor(tonumber(p.bed_trim) or -5)
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("elefant_foot_compensation %+.2fmm • Continuous Bed %+d°C", ef_boost, b_trim)
  return {
    title = string.format("Symptom Fix: Elephant's Foot (%+.2fmm Inset, Bed %+d°C)", ef_boost, b_trim),
    description = string.format("Eliminates bottom-layer outward flare for %s: adds %+.2fmm to elefant_foot_compensation and trims continuous bed temp by %+d°C.", scope, ef_boost, b_trim),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local ef_boost = tonumber(p and p.ef_comp) or 0.10
  local b_trim = math.floor(tonumber(p and p.bed_trim) or -5)

  local cur_ef = tonumber(config["elefant_foot_compensation"]) or 0.05
  config["elefant_foot_compensation"] = string.format("%.2f", cur_ef + ef_boost)
  config["elefant_foot_compensation_layers"] = "1"
  local cur_b = tonumber(config["textured_pei_plate_temp"]) or 65
  config["textured_pei_plate_temp"] = tostring(cur_b + b_trim)
  local d = describe(p or {}, ctx)
  print("[Symptom:ElephantsFoot] " .. (d.badge or ""))
end
