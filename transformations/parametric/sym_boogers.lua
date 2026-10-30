-- Symptom Fix: Boogers, Nozzle Blobs, Zits & Burnt Drips
meta = {
  id = "sym_boogers",
  name = "Fix Boogers, Nozzle Blobs, Zits & Burnt Drips",
  icon = "🦠",
  category = "Filament Symptom Fix (Boogers & Blobs)",
  tree_category = "2. Oozing, Stringing & Blobs",
  tree_subcategory = "Nozzle Tip Accumulation",
  slicer_menu_path = "Filament Flow Ratio + Relative Deretraction Speed + Non-Crossing Infill + Wipe on Loops",
  when_to_run = "Apply when molten plastic snowballs on the nozzle tip and drops burnt boogers onto the print, or leaves zits/pimples along walls.",
  formula_summary = "Flow_new = Flow_old + Flow_Trim  •  Deretract_new = Deretract_old × (1 - Slowdown%)  •  Infill = Non-Crossing",
  default_condition_expr = "filament:PCTG",
  how_to_read_print = {
    "1. Boogers on PETG/PCTG happen when slight over-extrusion (1–3%) or a crossing infill pattern (Grid/Triangles) causes the nozzle tip to scrape up ridges of plastic until a blob falls off.",
    "2. Zits/pimples along outer walls happen when deretraction (reloading filament after travel) is too fast, causing a pressure burp at seam starts.",
    "3. Trimming flow by -0.02, scaling deretraction speed down by -25% (0.75×), and using Gyroid/Crosshatch infill eliminates nozzle buildup on any hotend.",
  },
  hardware_checklist = {
    "🧦 Check Silicone Sock & Nozzle Tip: A torn silicone sock or plastic crusted under the sock edge drips burnt brown boogers onto light-colored prints — pull off the sock when cool and peel away any crust.",
    "💎 Consider DiamondBack / Coated Nozzle for PETG/PCTG: Polycrystalline diamond (PCD) and WS2 non-stick nozzles have much lower surface energy than bare steel, preventing PETG/PCTG from climbing the nozzle cone.",
  },
}

params = {
  {
    id = "flow_trim",
    label = "1. How much should we trim Flow Ratio (filament_flow_ratio) relative to current to stop plastic snow-plowing on the nozzle tip?",
    description = "-0.02 (-2%) is the sweet spot when PETG/PCTG prints fine overall but accumulates small nozzle boogers over long prints.",
    type = "range",
    min = -0.10,
    max = 0.00,
    step = 0.01,
    default = "-0.02",
    unit = "ratio",
    options = {
      { value = "0.00", label = "0.00 (Keep Current Flow)" },
      { value = "-0.01", label = "-0.01 (-1% Subtle Trim)" },
      { value = "-0.02", label = "-0.02 (-2% Recommended Anti-Booger)" },
      { value = "-0.03", label = "-0.03 (-3% Heavy Buildup Fix)" },
    },
  },
  {
    id = "deretract_spd",
    label = "2. By what percentage should we slow Deretraction (Reload) Speed to prevent seam zits/blobs?",
    description = "Scaling deretraction speed by -25% (0.75× of your current reload speed) prevents the nozzle from 'burping' a blob when resuming extrusion.",
    type = "range",
    min = -60,
    max = 0,
    step = 5,
    default = "-25%",
    unit = "%",
    options = {
      { value = "-15%", label = "-15% (0.85× Mild Reload Slowdown)" },
      { value = "-25%", label = "-25% (0.75× Recommended Anti-Burp)" },
      { value = "-35%", label = "-35% (0.65× Ultra-Smooth Seam Restart)" },
    },
  },
  {
    id = "infill_pattern",
    label = "3. Switch sparse infill to a non-crossing pattern so the nozzle never scrapes over infill intersections?",
    description = "Grid infill crosses over itself on the same layer, scraping plastic onto the nozzle tip. Gyroid and Crosshatch never cross on the same layer.",
    type = "enum",
    default = "gyroid",
    unit = "",
    options = {
      { value = "gyroid", label = "gyroid (Zero Same-Layer Crossings — Best)" },
      { value = "crosshatch", label = "crosshatch (Fast Non-Crossing)" },
      { value = "keep", label = "Keep Current Infill Pattern" },
    },
  },
  {
    id = "seam_gap",
    label = "4. What Seam Gap (% of nozzle diameter) should be left at outer wall loop closure?",
    description = "10%–12% of nozzle diameter prevents the end of the outer wall loop from plowing into the start of the loop.",
    type = "enum",
    default = "10%",
    unit = "",
    options = {
      { value = "8%", label = "8% (Standard)" },
      { value = "10%", label = "10% (Recommended)" },
      { value = "12%", label = "12% (Extra Anti-Zit Clearance)" },
    },
  },
}

function validate(p, ctx)
  local ft = tonumber(p.flow_trim) or -0.02
  if ft > 0.0 then
    return false, "Anti-booger flow_trim must be <= 0.00 (positive flow increases nozzle buildup)."
  end
  return true
end

function describe(p, ctx)
  local flow_trim = tonumber(p.flow_trim) or -0.02
  local deret_pct = tonumber(p.deretract_spd) or -25.0
  local d_scale = 0.75
  if deret_pct < 0 then
    d_scale = math.max(0.40, math.min(1.0, (100.0 + deret_pct) / 100.0))
  else
    d_scale = math.max(0.40, math.min(1.0, (100.0 - deret_pct) / 100.0))
  end
  local drop_label = math.floor((1.0 - d_scale) * 100.0 + 0.5)
  local infill = p.infill_pattern or "gyroid"
  local seam_gap = p.seam_gap or "10%"
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Flow %+.2f • Deretract -%.0f%% (%.2f×) • Infill=%s • Seam Gap %s", flow_trim, drop_label, d_scale, infill, seam_gap)
  return {
    title = string.format("Symptom Fix: Boogers & Blobs (Flow %+.2f, Deretract -%.0f%%)", flow_trim, drop_label),
    description = string.format("Eliminates nozzle tip boogers and seam blobs for %s: trims flow by %+.2f, scales deretraction speed by %.2f× (-%.0f%%), sets %s seam gap, and uses non-crossing infill (%s).", scope, flow_trim, d_scale, drop_label, seam_gap, infill),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local flow_trim = tonumber(p and p.flow_trim) or -0.02
  local deret_pct = tonumber(p and p.deretract_spd) or -25.0
  local d_scale = 0.75
  if deret_pct < 0 then
    d_scale = math.max(0.40, math.min(1.0, (100.0 + deret_pct) / 100.0))
  else
    d_scale = math.max(0.40, math.min(1.0, (100.0 - deret_pct) / 100.0))
  end
  local infill = (p and p.infill_pattern) or "gyroid"
  local seam_gap = (p and p.seam_gap) or "10%"

  local cur_f = tonumber(config["filament_flow_ratio"]) or 0.96
  config["filament_flow_ratio"] = string.format("%.3f", cur_f + flow_trim)
  local cur_d = tonumber(config["filament_deretraction_speed"]) or 40
  config["filament_deretraction_speed"] = tostring(math.max(15, math.floor(cur_d * d_scale + 0.5)))
  config["filament_retract_before_wipe"] = "80%"
  config["seam_gap"] = seam_gap
  config["wipe_on_loops"] = "1"
  config["reduce_infill_retraction"] = "0"
  if infill ~= "keep" then
    config["sparse_infill_pattern"] = infill
  end
  local d = describe(p or {}, ctx)
  print("[Symptom:Boogers] " .. (d.badge or ""))
end
