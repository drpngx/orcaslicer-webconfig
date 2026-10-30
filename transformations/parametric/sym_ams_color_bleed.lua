-- Symptom Fix: ACE Pro / AMS Color Bleeding, Purge Blobs & Prime Tower Collapse
meta = {
  id = "sym_ams_color_bleed",
  name = "ACE Pro / AMS Color Bleeding, Purge Blobs & Prime Tower Collapse",
  icon = "🎨",
  category = "7. Structural, Mechanical & Multi-Color (Multi-Material & ACE Pro)",
  tree_category = "7. Structural, Mechanical & Multi-Color",
  tree_subcategory = "ACE Pro Purge & Prime Tower",
  slicer_menu_path = "Relative Prime Tower Width Boost (+25%) & Prime Volume Boost (+35%) + Cut Retraction",
  when_to_run = "Apply when dark colors (Black/Red/CF) bleed into light colors (White/Yellow) after an ACE Pro swap, or the Prime Tower detaches/collapses.",
  formula_summary = "Tower_Width_new = Tower_Width_old × (1 + Width_Boost%)  •  Prime_Vol_new = Prime_Vol_old × (1 + Vol_Boost%)",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Color bleeding after a filament change means residual pigment in the nozzle melt-zone boundary layer wasn't fully swept out before returning to the model.",
    "2. Scaling `prime_tower_width` up by +25% (1.25×) and `prime_volume` / `flush_multiplier` up by +35% (1.35×) stabilizes tall prime towers and sweeps dark pigments cleanly on both standard and high-flow melt zones.",
  },
  hardware_checklist = {
    "💩 Inspect the Rear Purge Chute ('Poop Chute') Flap: Ensure purged filament coils drop freely into the chute and aren't getting dragged back onto the build plate by the nozzle wiper.",
    "🔢 Review the N×N Flushing Matrix in the Filament Tab: Increase the Flushing Multiplier to 1.15×–1.25× when swapping from Black/Red/CF to White/Clear.",
  },
}

params = {
  {
    id = "tower_width",
    label = "1. By what percentage should we widen Prime Tower Width (prime_tower_width) relative to current for stability?",
    description = "Scaling prime_tower_width by +25% (1.25×) prevents tall multi-color prime towers from wobbling or getting knocked over.",
    type = "range",
    min = 0,
    max = 80,
    step = 5,
    default = "+25%",
    unit = "%",
    options = {
      { value = "+15%", label = "+15% (1.15× Moderate Tower Widening)" },
      { value = "+25%", label = "+25% (1.25× Recommended Stable Tower)" },
      { value = "+45%", label = "+45% (1.45× Tall Multi-Color Prints)" },
    },
  },
  {
    id = "prime_vol",
    label = "2. By what percentage should we boost Prime Tower Volume (prime_volume) per swap relative to current?",
    description = "Scaling prime_volume by +35% (1.35×) stabilizes nozzle pressure and purges residual boundary-layer pigment after the poop-chute flush.",
    type = "range",
    min = 0,
    max = 120,
    step = 5,
    default = "+35%",
    unit = "%",
    options = {
      { value = "+20%", label = "+20% (1.20× Mild Purge Boost)" },
      { value = "+35%", label = "+35% (1.35× Recommended Anti-Bleed)" },
      { value = "+60%", label = "+60% (1.60× High-Contrast Dark→White / CHT Melt Zone)" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local tw_pct = math.abs(tonumber(p.tower_width) or 25.0)
  local tw_scale = math.max(1.0, math.min(2.0, (100.0 + tw_pct) / 100.0))
  local pv_pct = math.abs(tonumber(p.prime_vol) or 35.0)
  local pv_scale = math.max(1.0, math.min(2.5, (100.0 + pv_pct) / 100.0))
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Prime Tower Width +%.0f%% (%.2f×) • Prime Volume +%.0f%% (%.2f×)", tw_pct, tw_scale, pv_pct, pv_scale)
  return {
    title = string.format("Symptom Fix: ACE Pro Anti-Bleed & Tower (+%.0f%% Width, +%.0f%% Prime Vol)", tw_pct, pv_pct),
    description = string.format("Prevents multi-color bleeding and prime tower collapse for %s: scales prime_tower_width by %.2f× (+%.0f%%), scales prime_volume by %.2f× (+%.0f%%), and trims cut retraction.", scope, tw_scale, tw_pct, pv_scale, pv_pct),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local tw_pct = math.abs(tonumber(p and p.tower_width) or 25.0)
  local tw_scale = math.max(1.0, math.min(2.0, (100.0 + tw_pct) / 100.0))
  local pv_pct = math.abs(tonumber(p and p.prime_vol) or 35.0)
  local pv_scale = math.max(1.0, math.min(2.5, (100.0 + pv_pct) / 100.0))

  config["enable_prime_tower"] = "1"
  local cur_tw = tonumber(config["prime_tower_width"]) or 35
  config["prime_tower_width"] = tostring(math.floor(cur_tw * tw_scale + 0.5))
  local cur_pv = tonumber(config["prime_volume"]) or 35
  config["prime_volume"] = tostring(math.floor(cur_pv * pv_scale + 0.5))
  local cur_cut = tonumber(config["retraction_distances_when_cut"]) or 14.0
  config["retraction_distances_when_cut"] = string.format("%.1f", math.min(12.0, cur_cut * 0.85))
  local d = describe(p or {}, ctx)
  print("[Symptom:AMSColorBleed] " .. (d.badge or ""))
end
