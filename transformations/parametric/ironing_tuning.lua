-- Core Slicer Config Pack #2: Top Surface Ironing & Gloss
-- Polymer-aware `ironing_flow` (`12%` PLA, `18%` PETG/PCTG, `22%` ASA/ABS/PC),
-- `ironing_type` (`topmost` vs. `top` / `solid`), `ironing_speed`, `ironing_spacing`,
-- and `top_surface_pattern = "monotonicline"`.

meta = {
  id = "ironing_tuning",
  name = "Top Surface Ironing & Gloss Pack",
  icon = "✨",
  category = "Core Slicer Config Pack (Top Surface & Ironing)",
  tree_category = "5. Top/Bottom Surfaces & Infill",
  tree_subcategory = "Top Surface Ironing & Gloss",
  slicer_menu_path = "Process → Quality → Ironing (`topmost` / `top` / `solid`) + Polymer-Aware Ironing Flow (`12%` PLA / `18%` PETG / `22%` ASA·ABS) + Speed + Spacing + `monotonicline`",
  when_to_run = "Enable when printing flat badges, enclosures, nameplates, or mechanical faces that need an injection-molded, line-free top finish.",
  formula_summary = "ironing_flow = Polymer_Flow (12% PLA / 18% PETG·PCTG / 22% ASA·ABS)  •  ironing_speed = 30–40 mm/s  •  ironing_spacing = 0.15 mm  •  top_surface_pattern = monotonicline",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Different polymers require different ironing flow rates to fill inter-line valleys without plowing up a ridge at the edges: PLA swells slightly and irons best at `12%` flow; PETG/PCTG prefers `18%` at `30 mm/s`; higher-shrinkage styrenics (ASA/ABS/PC) need `22%` at `40 mm/s`.",
    "2. Pairing `top_surface_pattern = monotonicline` with `only_one_wall_top = 1` ensures all underlying top-shell lines are laid down in a single uniform direction before the hot nozzle irons perpendicular to them.",
  },
  hardware_checklist = {
    "💎 Nozzle Tip Geometry: Flat-tip high-conductivity nozzles (Diamondback PCD / MicroSwiss CM2) produce the highest-gloss ironed finish; avoid ironing flexible TPU.",
  },
}

params = {
  {
    id = "ironing_preset",
    label = "1. Polymer Ironing & Surface Preset (Snaps Dials Below)",
    description = "Select Polymer-Aware Auto-Tuned (12% PLA / 18% PETG·PCTG / 22% ASA·ABS) or a specific polymer preset.",
    type = "enum",
    default = "auto_polymer",
    options = {
      {
        value = "auto_polymer",
        label = "🧬 Polymer-Aware Auto-Tuned (12% PLA • 18% PETG/PCTG • 22% ASA/ABS • Topmost)",
        sets = {
          ironing_type = "topmost",
          ironing_flow_pct = "15",
          ironing_speed_mms = "35",
          ironing_spacing_mm = "0.15",
          top_surface_pattern = "monotonicline",
        },
      },
      {
        value = "pla_matte_satin",
        label = "🍃 PLA / Matte / Silk Satin Finish (12% Flow @ 35 mm/s • 0.15mm Spacing)",
        sets = {
          ironing_type = "topmost",
          ironing_flow_pct = "12",
          ironing_speed_mms = "35",
          ironing_spacing_mm = "0.15",
          top_surface_pattern = "monotonicline",
        },
      },
      {
        value = "petg_gloss_seal",
        label = "🧴 PETG / PCTG Mirror-Gloss Seal (18% Flow @ 30 mm/s • 0.15mm Spacing)",
        sets = {
          ironing_type = "topmost",
          ironing_flow_pct = "18",
          ironing_speed_mms = "30",
          ironing_spacing_mm = "0.15",
          top_surface_pattern = "monotonicline",
        },
      },
      {
        value = "asa_abs_dense",
        label = "🛡️ ASA / ABS / PC Dense Top Weld (22% Flow @ 40 mm/s • 0.12mm Spacing)",
        sets = {
          ironing_type = "top",
          ironing_flow_pct = "22",
          ironing_speed_mms = "40",
          ironing_spacing_mm = "0.12",
          top_surface_pattern = "monotonicline",
        },
      },
      {
        value = "custom",
        label = "🎛️ Custom Ironing Flow, Speed & Spacing Dials",
      },
    },
  },
  {
    id = "ironing_type",
    label = "2. Ironing Scope (ironing_type)",
    description = "topmost irons only the highest Z horizontal face; top irons every stepped horizontal top surface; solid irons every solid internal layer for maximum density.",
    type = "enum",
    default = "topmost",
    options = {
      { value = "topmost", label = "topmost — Topmost Horizontal Surface Only (Fast)" },
      { value = "top", label = "top — All Stepped Top Surfaces (Recommended for Multi-Tier Parts)" },
      { value = "solid", label = "solid — Every Solid Layer (Structural / Optical Density)" },
    },
  },
  {
    id = "ironing_flow_pct",
    label = "3. Ironing Extrusion Flow (ironing_flow, %)",
    description = "Percentage of normal extrusion flow pushed while wiping the hot nozzle across the top layer. In 'auto_polymer' mode, resolves to 12% PLA, 18% PETG/PCTG, or 22% ASA/ABS.",
    type = "range",
    min = 5,
    max = 35,
    step = 1,
    default = "15",
    unit = "%",
    options = {
      { value = "10", label = "10% (Light Skim)" },
      { value = "12", label = "12% (PLA / Silk Optimal)" },
      { value = "15", label = "15% (Universal Baseline)" },
      { value = "18", label = "18% (PETG / PCTG Optimal)" },
      { value = "22", label = "22% (ASA / ABS / PC Optimal)" },
    },
  },
  {
    id = "ironing_speed_mms",
    label = "4. Ironing Travel Speed (ironing_speed, mm/s)",
    description = "Velocity of the hot nozzle during the ironing pass (30–40 mm/s gives smooth heat transfer without scorching).",
    type = "range",
    min = 15,
    max = 80,
    step = 5,
    default = "35",
    unit = "mm/s",
    options = {
      { value = "25", label = "25 mm/s (Slow High-Gloss)" },
      { value = "30", label = "30 mm/s (PETG / PCTG Recommended)" },
      { value = "35", label = "35 mm/s (PLA Recommended)" },
      { value = "40", label = "40 mm/s (ASA / ABS Recommended)" },
      { value = "60", label = "60 mm/s (High-Speed Skim)" },
    },
  },
  {
    id = "ironing_spacing_mm",
    label = "5. Ironing Line Spacing (ironing_spacing, mm)",
    description = "Lateral step-over distance between adjacent ironing passes (0.12–0.15 mm on a 0.4mm nozzle means each pass overlaps 3×).",
    type = "range",
    min = 0.08,
    max = 0.30,
    step = 0.01,
    default = "0.15",
    unit = "mm",
    options = {
      { value = "0.10", label = "0.10 mm (Ultra-Fine 4× Overlap)" },
      { value = "0.12", label = "0.12 mm (Dense Overlap)" },
      { value = "0.15", label = "0.15 mm (Recommended Standard)" },
      { value = "0.20", label = "0.20 mm (Fast 2× Overlap)" },
    },
  },
  {
    id = "top_surface_pattern",
    label = "6. Underlying Top Surface Pattern (top_surface_pattern)",
    description = "monotonicline lays down continuous unidirectional top lines without lift scars so the ironing pass has a uniform substrate.",
    type = "enum",
    default = "monotonicline",
    options = {
      { value = "monotonicline", label = "monotonicline — Continuous Unidirectional Lines (Recommended)" },
      { value = "monotonic", label = "monotonic — Monotonic Rectilinear" },
      { value = "concentric", label = "concentric — Concentric Rings" },
      { value = "archimedeanchords", label = "archimedeanchords — Spiral Chords" },
    },
  },
}

local function resolve_effective_ironing(p, ctx)
  local preset = (p and p.ironing_preset) or "auto_polymer"
  local flow = math.floor((tonumber(p and p.ironing_flow_pct) or 15) + 0.5)
  local spd = math.floor((tonumber(p and p.ironing_speed_mms) or 35) + 0.5)
  local spacing = tonumber(p and p.ironing_spacing_mm) or 0.15

  if preset == "auto_polymer" then
    local fil = string.upper(tostring((ctx and ctx.filament_type) or "PETG"))
    local nd = tonumber(ctx and ctx.nozzle_diameter) or 0.40
    local scale_sp = function(base_sp)
      local raw = base_sp * ((nd / 0.40) ^ 0.75)
      local clamped = math.max(0.08, math.min(0.30, raw))
      return math.floor(clamped * 100.0 + 0.5) / 100.0
    end
    if string.find(fil, "PLA") then
      return { flow = 12, spd = 35, spacing = scale_sp(0.15), mode_tag = "Auto PLA (12%)" }
    elseif string.find(fil, "ASA") or string.find(fil, "ABS") or string.find(fil, "PC") or string.find(fil, "PA") then
      return { flow = 22, spd = 40, spacing = scale_sp(0.12), mode_tag = string.format("Auto %s (22%%)", fil) }
    elseif string.find(fil, "PETG") or string.find(fil, "PCTG") or string.find(fil, "CPE") then
      return { flow = 18, spd = 30, spacing = scale_sp(0.15), mode_tag = string.format("Auto %s (18%%)", fil) }
    else
      return { flow = flow, spd = spd, spacing = scale_sp(spacing), mode_tag = string.format("Auto %s (%d%%)", fil, flow) }
    end
  end
  return { flow = flow, spd = spd, spacing = spacing, mode_tag = string.format("%d%% Flow", flow) }
end

function validate(p, ctx)
  local flow = tonumber(p and p.ironing_flow_pct) or 15
  local spd = tonumber(p and p.ironing_speed_mms) or 35
  local spacing = tonumber(p and p.ironing_spacing_mm) or 0.15

  if flow > 25 and spd < 25 then
    return false, "Ironing flow > 25% combined with slow speed (< 25 mm/s) causes severe over-extrusion plowing and burnt plastic blobs on top edges."
  end
  if spacing < 0.10 and flow > 22 then
    return false, "Ultra-tight ironing spacing (< 0.10mm) with high ironing flow (> 22%) floods the top surface. Reduce flow <= 20% or increase spacing >= 0.12mm."
  end
  return true
end

function describe(p, ctx)
  local itype = (p and p.ironing_type) or "topmost"
  local pat = (p and p.top_surface_pattern) or "monotonicline"
  local eff = resolve_effective_ironing(p, ctx)
  local flow = eff.flow
  local spd = eff.spd
  local spacing = eff.spacing
  local mode_tag = eff.mode_tag
  local badge = string.format("Ironing = %s • Flow = %d%% (%s) @ %d mm/s • Spacing = %.2fmm • Top = %s", itype, flow, mode_tag, spd, spacing, pat)
  return {
    title = string.format("Ironing Pack: %s (%d%% Flow [%s], %d mm/s, %.2fmm)", itype, flow, mode_tag, spd, spacing),
    description = string.format("Polymer-aware top surface ironing: sets ironing_type = %s, ironing_flow = %d%% (%s), ironing_speed = %d mm/s, ironing_spacing = %.2fmm, top_surface_pattern = %s, and only_one_wall_top = 1.", itype, flow, mode_tag, spd, spacing, pat),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local itype = (p and p.ironing_type) or "topmost"
  local pat = (p and p.top_surface_pattern) or "monotonicline"
  local eff = resolve_effective_ironing(p, ctx)
  local flow = eff.flow
  local spd = eff.spd
  local spacing = eff.spacing

  config["ironing_type"] = itype
  config["ironing_flow"] = string.format("%d%%", flow)
  config["ironing_speed"] = tostring(spd)
  config["ironing_spacing"] = fmt_num(spacing, "%.2f")
  config["top_surface_pattern"] = pat
  config["only_one_wall_top"] = "1"

  local d = describe(p or {}, ctx)
  print("[Pack:IroningTuning] " .. (d.badge or ""))
end
