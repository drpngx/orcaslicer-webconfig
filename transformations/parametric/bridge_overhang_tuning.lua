-- Core Slicer Config Pack #4: Bridges & Steep Overhangs
-- Anti-sag `bridge_flow` (`0.90–0.95`), `bridge_speed`, progressive 4-stage overhang speed curve
-- (`overhang_1_4_speed`..`overhang_4_4_speed`), `enable_overhang_speed = "1"`,
-- `detect_overhang_wall = "1"`, and `overhang_fan_speed = "100"`.

meta = {
  id = "bridge_overhang_tuning",
  name = "Bridges & Steep Overhangs Pack",
  icon = "🌉",
  category = "Core Slicer Config Pack (Bridges & Overhangs)",
  tree_category = "6. Overhangs, Bridging & Supports",
  tree_subcategory = "Progressive Overhang Speed Curve & Bridge Tension",
  slicer_menu_path = "Process → Speed → Overhang Speed (`overhang_1_4_speed`..`overhang_4_4_speed`) + Bridge Speed (`bridge_speed`) + Quality → Bridge Flow (`bridge_flow`) + Cooling → `overhang_fan_speed = 100%`",
  when_to_run = "Enable when printing models with horizontal unsupported spans, archways, fillets, chamfered undersides, or steep 50°–75° overhangs without supports.",
  formula_summary = "bridge_flow = 0.90–0.95  •  bridge_speed = 22–40 mm/s  •  Overhang Curve = 50 → 35 → 22 → 15 mm/s (10–25% .. 75–100%)  •  overhang_fan_speed = 100%",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Horizontal bridges sag when extruded at 1.00× flow because gravity pulls the molten strand downward before it freezes. Setting `bridge_flow = 0.92` (-8%) puts the strand under slight mechanical tension between anchor pillars.",
    "2. OrcaSlicer splits overhanging perimeters into 4 overlap tiers (`10–25%`, `25–50%`, `50–75%`, `75–100%`). A smooth, monotonically decreasing velocity curve (`50 → 35 → 22 → 15 mm/s`) prevents sudden extruder pressure spikes at overhang transitions while giving the part cooling fan (`100%`) time to freeze steep edges before they curl upward.",
  },
  hardware_checklist = {
    "🌀 Part Cooling Duct Alignment: Dual-sided cooling ducts (or orienting the steepest overhang toward the primary blower) dramatically improve >65° overhang quality.",
  },
}

params = {
  {
    id = "bridge_preset",
    label = "1. Bridge Tension & Overhang Curve Preset (Snaps Dials Below)",
    description = "Choose Tensioned Anti-Sag (0.92 flow, 28 mm/s bridge, 50/35/22/15 mm/s curve), Extreme 60°–75° Overhangs, High-Speed Functional, or Custom Curve.",
    type = "enum",
    default = "tensioned_anti_sag",
    options = {
      {
        value = "tensioned_anti_sag",
        label = "🌉 Tensioned Anti-Sag & Balanced Overhangs (0.92 Flow @ 28 mm/s • 50/35/22/15 mm/s Curve • 100% Fan)",
        sets = {
          bridge_flow = "0.92",
          bridge_speed = "28",
          overhang_1_4_speed = "50",
          overhang_2_4_speed = "35",
          overhang_3_4_speed = "22",
          overhang_4_4_speed = "15",
          overhang_fan_speed = "100",
        },
      },
      {
        value = "steep_overhang_max",
        label = "🏔️ Extreme 60°–75° Overhangs & Micro-Features (0.90 Flow @ 22 mm/s • 40/25/15/10 mm/s Curve • 100% Fan)",
        sets = {
          bridge_flow = "0.90",
          bridge_speed = "22",
          overhang_1_4_speed = "40",
          overhang_2_4_speed = "25",
          overhang_3_4_speed = "15",
          overhang_4_4_speed = "10",
          overhang_fan_speed = "100",
        },
      },
      {
        value = "high_speed_functional",
        label = "🚀 High-Speed Functional Bridges (0.95 Flow @ 40 mm/s • 60/45/30/20 mm/s Curve • 100% Fan)",
        sets = {
          bridge_flow = "0.95",
          bridge_speed = "40",
          overhang_1_4_speed = "60",
          overhang_2_4_speed = "45",
          overhang_3_4_speed = "30",
          overhang_4_4_speed = "20",
          overhang_fan_speed = "100",
        },
      },
      {
        value = "custom",
        label = "🎛️ Custom Bridge Flow & 4-Stage Overhang Speed Dials",
      },
    },
  },
  {
    id = "bridge_flow",
    label = "2. Anti-Sag Bridge Flow Ratio (bridge_flow)",
    description = "0.90–0.95 stretches unsupported bridge strands slightly so they stay taut across long spans instead of drooping.",
    type = "range",
    min = 0.80,
    max = 1.05,
    step = 0.01,
    default = "0.92",
    unit = "×",
    options = {
      { value = "0.88", label = "0.88× (-12% High Tension)" },
      { value = "0.90", label = "0.90× (-10% Long Spans / PETG)" },
      { value = "0.92", label = "0.92× (-8% Recommended Anti-Sag)" },
      { value = "0.95", label = "0.95× (-5% Light Tension)" },
      { value = "1.00", label = "1.00× (Unmodified)" },
    },
  },
  {
    id = "bridge_speed",
    label = "3. External Bridge Print Speed (bridge_speed, mm/s)",
    description = "Velocity of unsupported bridge extrusions (22–35 mm/s lets the cooling fan solidify the strand in mid-air).",
    type = "range",
    min = 15,
    max = 60,
    step = 2,
    default = "28",
    unit = "mm/s",
    options = {
      { value = "22", label = "22 mm/s (Slow Freeze)" },
      { value = "28", label = "28 mm/s (Recommended)" },
      { value = "35", label = "35 mm/s (Standard)" },
      { value = "45", label = "45 mm/s (Fast)" },
    },
  },
  {
    id = "overhang_1_4_speed",
    label = "4. Overhang Speed Tier 1 — Mild 10–25% Overhang (overhang_1_4_speed, mm/s)",
    description = "Print speed when 10–25% of the perimeter bead width overhangs free air.",
    type = "range",
    min = 20,
    max = 80,
    step = 5,
    default = "50",
    unit = "mm/s",
    options = {
      { value = "40", label = "40 mm/s" },
      { value = "50", label = "50 mm/s (Recommended)" },
      { value = "60", label = "60 mm/s" },
    },
  },
  {
    id = "overhang_2_4_speed",
    label = "5. Overhang Speed Tier 2 — Moderate 25–50% Overhang (overhang_2_4_speed, mm/s)",
    description = "Print speed when 25–50% of the perimeter bead width overhangs free air (~45°–55° slope).",
    type = "range",
    min = 15,
    max = 60,
    step = 5,
    default = "35",
    unit = "mm/s",
    options = {
      { value = "25", label = "25 mm/s" },
      { value = "35", label = "35 mm/s (Recommended)" },
      { value = "45", label = "45 mm/s" },
    },
  },
  {
    id = "overhang_3_4_speed",
    label = "6. Overhang Speed Tier 3 — Steep 50–75% Overhang (overhang_3_4_speed, mm/s)",
    description = "Print speed when 50–75% of the perimeter bead width overhangs free air (~60°–70° slope).",
    type = "range",
    min = 10,
    max = 45,
    step = 2,
    default = "22",
    unit = "mm/s",
    options = {
      { value = "15", label = "15 mm/s" },
      { value = "22", label = "22 mm/s (Recommended)" },
      { value = "30", label = "30 mm/s" },
    },
  },
  {
    id = "overhang_4_4_speed",
    label = "7. Overhang Speed Tier 4 — Extreme 75–100% Overhang / Perimeter Bridge (overhang_4_4_speed, mm/s)",
    description = "Print speed when 75–100% of the perimeter bead overhangs free air (>70° slope or curved arch).",
    type = "range",
    min = 8,
    max = 35,
    step = 2,
    default = "15",
    unit = "mm/s",
    options = {
      { value = "10", label = "10 mm/s (Max Freeze)" },
      { value = "15", label = "15 mm/s (Recommended)" },
      { value = "20", label = "20 mm/s" },
    },
  },
  {
    id = "overhang_fan_speed",
    label = "8. Overhang & Bridge Cooling Fan Speed (overhang_fan_speed, %)",
    description = "Part cooling fan duty cycle forced during bridges and steep overhangs.",
    type = "range",
    min = 50,
    max = 100,
    step = 5,
    default = "100",
    unit = "%",
    options = {
      { value = "80", label = "80% (Enclosed ABS / ASA)" },
      { value = "90", label = "90%" },
      { value = "100", label = "100% (Recommended Max Freeze)" },
    },
  },
}

function validate(p, ctx)
  local s1 = tonumber(p and p.overhang_1_4_speed) or 50
  local s2 = tonumber(p and p.overhang_2_4_speed) or 35
  local s3 = tonumber(p and p.overhang_3_4_speed) or 22
  local s4 = tonumber(p and p.overhang_4_4_speed) or 15

  if not (s1 >= s2 and s2 >= s3 and s3 >= s4) then
    return false, string.format(
      "Overhang speed curve must decrease monotonically as overhang steepness increases (got %d → %d → %d → %d mm/s).",
      s1, s2, s3, s4
    )
  end
  return true
end

function describe(p, ctx)
  local bf = tonumber(p and p.bridge_flow) or 0.92
  local bs = math.floor((tonumber(p and p.bridge_speed) or 28) + 0.5)
  local s1 = math.floor((tonumber(p and p.overhang_1_4_speed) or 50) + 0.5)
  local s2 = math.floor((tonumber(p and p.overhang_2_4_speed) or 35) + 0.5)
  local s3 = math.floor((tonumber(p and p.overhang_3_4_speed) or 22) + 0.5)
  local s4 = math.floor((tonumber(p and p.overhang_4_4_speed) or 15) + 0.5)
  local fan = math.floor((tonumber(p and p.overhang_fan_speed) or 100) + 0.5)

  local badge = string.format("Bridge = %.2f× @ %d mm/s • Overhang Curve = %d/%d/%d/%d mm/s • Fan = %d%%", bf, bs, s1, s2, s3, s4, fan)
  return {
    title = string.format("Bridges & Overhangs Pack: %.2f× Bridge @ %d mm/s (%d/%d/%d/%d Curve)", bf, bs, s1, s2, s3, s4),
    description = string.format("Anti-sag bridging & progressive overhang deceleration: sets bridge_flow = %.2f, bridge_speed = %d mm/s, overhang_1_4..4_4_speed = %d/%d/%d/%d mm/s, detect_overhang_wall = 1, and overhang_fan_speed = %d%%.", bf, bs, s1, s2, s3, s4, fan),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local bf = tonumber(p and p.bridge_flow) or 0.92
  local bs = math.floor((tonumber(p and p.bridge_speed) or 28) + 0.5)
  local s1 = math.floor((tonumber(p and p.overhang_1_4_speed) or 50) + 0.5)
  local s2 = math.floor((tonumber(p and p.overhang_2_4_speed) or 35) + 0.5)
  local s3 = math.floor((tonumber(p and p.overhang_3_4_speed) or 22) + 0.5)
  local s4 = math.floor((tonumber(p and p.overhang_4_4_speed) or 15) + 0.5)
  local fan = math.floor((tonumber(p and p.overhang_fan_speed) or 100) + 0.5)

  config["bridge_flow"] = fmt_num(bf, "%.2f")
  config["bridge_speed"] = tostring(bs)
  config["enable_overhang_speed"] = "1"
  config["detect_overhang_wall"] = "1"
  config["overhang_1_4_speed"] = tostring(s1)
  config["overhang_2_4_speed"] = tostring(s2)
  config["overhang_3_4_speed"] = tostring(s3)
  config["overhang_4_4_speed"] = tostring(s4)
  config["overhang_fan_speed"] = tostring(fan)

  local d = describe(p or {}, ctx)
  print("[Pack:BridgeOverhangTuning] " .. (d.badge or ""))
end
