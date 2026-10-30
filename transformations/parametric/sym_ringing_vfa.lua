-- Symptom Fix: Ghosting / Ringing (Wavy Echoes) & VFA Vertical Fine Artifacts
meta = {
  id = "sym_ringing_vfa",
  name = "Ghosting / Ringing (Wavy Echoes) & VFA Vertical Fine Artifacts",
  icon = "〰️",
  category = "4. Walls, Seams & Dimensional (Resonance & Belts)",
  tree_category = "4. Walls, Seams & Dimensional",
  tree_subcategory = "Input Shaping & Belt Resonance",
  slicer_menu_path = "Relative Outer Wall Speed Adjustment + Relative Outer Wall Acceleration Scaling",
  when_to_run = "Apply when faint repeating ripples ('ghost echoes') appear after sharp corners/lettering, or uniform vertical belt-pitch lines (VFAs) show on glossy walls.",
  formula_summary = "Outer_Accel_new = Outer_Accel_old × (1 - Accel_Drop%)  •  Outer_Speed_new = Outer_Speed_old × (1 ± Speed_Shift%)",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Ghosting/Ringing (ripples that fade out after a sharp corner or text) comes from high Outer Wall Acceleration shaking the gantry before Input Shaping can damp it.",
    "2. VFAs (uniform vertical micro-stripes across the entire wall) occur when outer wall speed sits inside a stepper/belt resonance band — scale outer wall acceleration down by -40% (0.60×) and shift outer wall speed by +20% (1.20×) or -30% (0.70×) out of resonance.",
  },
  hardware_checklist = {
    "🎸 Tension Kobra S1 CoreXY Belts & Run Input Shaper: Check the two belt tensioners on the rear of the Kobra S1 gantry so both X/Y belts have equal tension, then run 'Vibration Compensation (Input Shaping)' from the printer setup menu.",
    "🪨 Place Printer on a Stable Surface: A wobbly table amplifies gantry jerk into ringing echoes.",
  },
}

params = {
  {
    id = "outer_accel",
    label = "1. By what percentage should we reduce Outer Wall Acceleration & Jerk (outer_wall_acceleration) to eliminate corner ringing?",
    description = "Scaling outer wall acceleration by -40% (0.60× of your current outer wall acceleration) eliminates ghosting with almost zero impact on total print time.",
    type = "range",
    min = -70,
    max = 0,
    step = 5,
    default = "-40%",
    unit = "%",
    options = {
      { value = "-25%", label = "-25% (0.75× Balanced CoreXY Damping)" },
      { value = "-40%", label = "-40% (0.60× Recommended Anti-Ringing)" },
      { value = "-55%", label = "-55% (0.45× Ultra-Clean Show Quality)" },
    },
  },
  {
    id = "outer_spd",
    label = "2. How should we shift Outer Wall Speed (outer_wall_speed) relative to current to move out of the VFA belt-resonance band?",
    description = "+20% (1.20×) cruises above the 2GT belt-tooth resonance band on high-flow hotends; -30% (0.70×) slows below resonance for silk/detail parts.",
    type = "range",
    min = -50,
    max = 50,
    step = 5,
    default = "+20%",
    unit = "%",
    options = {
      { value = "+20%", label = "+20% (1.20× Cruise Above VFA Resonance — Recommended)" },
      { value = "-30%", label = "-30% (0.70× Slow Below VFA Resonance — Silk/Detail)" },
      { value = "0%", label = "0% (1.00× Keep Current Outer Wall Speed)" },
    },
  },
}

function validate(p, ctx)
  return true
end

function describe(p, ctx)
  local acc_pct = tonumber(p.outer_accel) or -40.0
  local acc_scale = 0.60
  if acc_pct < 0 then
    acc_scale = math.max(0.30, math.min(1.0, (100.0 + acc_pct) / 100.0))
  else
    acc_scale = math.max(0.30, math.min(1.0, (100.0 - acc_pct) / 100.0))
  end
  local acc_drop_label = math.floor((1.0 - acc_scale) * 100.0 + 0.5)
  local spd_pct = tonumber(p.outer_spd) or 20.0
  local spd_scale = math.max(0.40, math.min(1.60, (100.0 + spd_pct) / 100.0))
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Outer Wall Accel -%.0f%% (%.2f×) • Outer Wall Speed %+.0f%% (%.2f× VFA Shift)", acc_drop_label, acc_scale, spd_pct, spd_scale)
  return {
    title = string.format("Symptom Fix: Ringing & VFA (-%.0f%% Accel, %+.0f%% Outer Wall Speed)", acc_drop_label, spd_pct),
    description = string.format("Eliminates ghosting echoes and VFA belt artifacts for %s: scales outer_wall_acceleration by %.2f× (-%.0f%%) and outer_wall_speed by %.2f× (%+.0f%%).", scope, acc_scale, acc_drop_label, spd_scale, spd_pct),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local acc_pct = tonumber(p and p.outer_accel) or -40.0
  local acc_scale = 0.60
  if acc_pct < 0 then
    acc_scale = math.max(0.30, math.min(1.0, (100.0 + acc_pct) / 100.0))
  else
    acc_scale = math.max(0.30, math.min(1.0, (100.0 - acc_pct) / 100.0))
  end
  local spd_pct = tonumber(p and p.outer_spd) or 20.0
  local spd_scale = math.max(0.40, math.min(1.60, (100.0 + spd_pct) / 100.0))

  local cur_acc = tonumber(config["outer_wall_acceleration"]) or 5000
  config["outer_wall_acceleration"] = tostring(math.max(1000, math.floor(cur_acc * acc_scale + 0.5)))
  local cur_spd = tonumber(config["outer_wall_speed"]) or 140
  config["outer_wall_speed"] = tostring(math.max(30, math.floor(cur_spd * spd_scale + 0.5)))
  local cur_j = tonumber(config["outer_wall_jerk"]) or 9
  config["outer_wall_jerk"] = tostring(math.max(4, math.floor(cur_j * acc_scale + 0.5)))
  local d = describe(p or {}, ctx)
  print("[Symptom:RingingVFA] " .. (d.badge or ""))
end
