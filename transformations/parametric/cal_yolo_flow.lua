-- Calibration Print #2: YOLO Flow Rate Calibration (Single-Pass Chip Test)
meta = {
  id = "cal_yolo_flow",
  name = "YOLO Flow Rate Calibration (Single-Pass Chip Test)",
  icon = "🎯",
  category = "Calibration Print #2 (Flow Rate)",
  tree_category = "Post-Print Calibration",
  tree_subcategory = "Calibration Test",
  slicer_menu_path = "Calibration → Flow Rate → YOLO (Recommended) or Pass 1 / Pass 2",
  when_to_run = "Run 2nd (right after Temperature Tower) whenever dialing in a new filament spool or nozzle.",
  formula_summary = "YOLO Mode: Flow_new = Flow_old + Chip_Value  •  Classic Pass 1/2: Flow_new = Flow_old × (100 + Pass_Pct) / 100",
  default_condition_expr = "filament:PCTG",
  how_to_read_print = {
    "1. In AnycubicSlicerNext / OrcaSlicer top menu, click Calibration → Flow Rate → YOLO and print the numbered modifier chips (-0.05 to +0.05).",
    "2. Rub your fingernail across the CENTER of each top surface (ignore the outer perimeter seam where the nozzle turns around).",
    "3. Pick the chip whose center feels smoothest — no tiny gaps between extrusion lines (under-extrusion) and no rough ridges plowed up between lines (over-extrusion).",
    "4. Answer the questions below: in YOLO mode, the chip number is added directly to your starting flow (Flow_new = Flow_old + Chip_Modifier).",
  },
  hardware_checklist = {
    "🖨️ Slice and print the built-in test model from the AnycubicSlicerNext / OrcaSlicer top 'Calibration' menu before entering your measurements below.",
  },
}

params = {
  {
    id = "mode",
    label = "1. Which Flow Rate calibration test did you print?",
    description = "YOLO mode prints chips labeled -0.05 to +0.05 (direct addition). Classic Pass 1/2 prints chips labeled -10 to +10 (%).",
    type = "enum",
    default = "YOLO",
    unit = "",
    options = {
      { value = "YOLO", label = "YOLO Mode (-0.05 to +0.05 — Direct Add)" },
      { value = "Classic", label = "Classic Pass 1/2 (-20% to +20% — Percentage)" },
    },
  },
  {
    id = "old_flow",
    label = "2. What was your starting Flow Ratio (filament_flow_ratio) when you sliced the test?",
    description = "Check the Filament tab for your spool's current flow ratio (e.g. 0.96 for Anycubic PETG/PLA+, 0.95 for PCTG, 0.98 for PLA).",
    type = "range",
    min = 0.70,
    max = 1.25,
    step = 0.01,
    default = "0.96",
    unit = "ratio",
    options = {
      { value = "0.98", label = "0.98 (Standard PLA)" },
      { value = "0.96", label = "0.96 (Anycubic PLA+ / PETG)" },
      { value = "0.95", label = "0.95 (PCTG / Silk)" },
      { value = "0.93", label = "0.93 (ABS / ASA / PA)" },
      { value = "1.00", label = "1.00 (Unscaled Base)" },
    },
  },
  {
    id = "chip_val",
    label = "3. What number is printed on the smoothest chip?",
    description = "If two adjacent chips (e.g. -0.01 and -0.02) look equally smooth, you can type the midpoint (-0.015) in the input box.",
    type = "range",
    min = -25.0,
    max = 25.0,
    step = 0.005,
    default = "-0.02",
    unit = "modifier",
    options = {
      { value = "-0.04", label = "-0.04" },
      { value = "-0.03", label = "-0.03" },
      { value = "-0.02", label = "-0.02" },
      { value = "-0.015", label = "-0.015" },
      { value = "-0.01", label = "-0.01" },
      { value = "0.00", label = "0.00 (Already Spot-On)" },
      { value = "+0.01", label = "+0.01" },
      { value = "+0.015", label = "+0.015" },
      { value = "+0.02", label = "+0.02" },
      { value = "+0.03", label = "+0.03" },
    },
  },
}

function validate(p, ctx)
  local mode = p.mode or "YOLO"
  local old_flow = tonumber(p.old_flow) or 0.96
  local chip_val = tonumber(p.chip_val) or -0.02
  if mode == "YOLO" and (chip_val < -0.15 or chip_val > 0.15) then
    return false, "In YOLO mode, chip modifier must be between -0.15 and +0.15 (switch to Classic mode for percentage values)."
  end
  local new_flow = old_flow + chip_val
  if mode == "Classic" then
    new_flow = old_flow * (100.0 + chip_val) / 100.0
  end
  if new_flow < 0.75 or new_flow > 1.25 then
    return false, string.format("Computed flow ratio %.3f is outside the safe extruder range [0.750, 1.250].", new_flow)
  end
  return true
end

function describe(p, ctx)
  local mode = p.mode or "YOLO"
  local old_flow = tonumber(p.old_flow) or 0.96
  local chip_str = p.chip_val or "-0.02"
  local chip_val = tonumber(chip_str) or -0.02
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local new_flow = old_flow + chip_val
  if mode == "Classic" then
    new_flow = old_flow * (100.0 + chip_val) / 100.0
  end
  new_flow = math.max(0.75, math.min(1.25, new_flow))
  local badge = ""
  if mode == "Classic" then
    badge = string.format("Flow_new = %.3f × (100 %+.1f%%) / 100 = %.3f", old_flow, chip_val, new_flow)
  else
    badge = string.format("Flow_new = %.3f (Old) %+.3f (YOLO Chip) = %.3f", old_flow, chip_val, new_flow)
  end
  return {
    title = string.format("Calibration: %s Flow Rate (%.3f → %.3f)", mode, old_flow, new_flow),
    description = string.format("%s Flow Rate Calibration for %s: starting flow %.3f with smoothest chip %s → new filament_flow_ratio = %.3f.", mode, scope, old_flow, chip_str, new_flow),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local mode = (p and p.mode) or "YOLO"
  local old_flow = tonumber(p and p.old_flow) or 0.96
  local chip_val = tonumber(p and p.chip_val) or -0.02
  local new_flow = old_flow + chip_val
  if mode == "Classic" then
    new_flow = old_flow * (100.0 + chip_val) / 100.0
  end
  new_flow = math.max(0.75, math.min(1.25, new_flow))
  config["filament_flow_ratio"] = string.format("%.3f", new_flow)
  local d = describe(p or {}, ctx)
  print("[Calibration:FlowRate] " .. (d.badge or ""))
end
