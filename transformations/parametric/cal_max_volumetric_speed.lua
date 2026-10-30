-- Calibration Print #5: Max Volumetric Speed (MVS) Race Track Calibration
meta = {
  id = "cal_max_volumetric_speed",
  name = "Max Volumetric Speed (MVS) Race Track Calibration",
  icon = "🏎️",
  category = "Calibration Print #5 (Volumetric Flow Ceiling)",
  tree_category = "Post-Print Calibration",
  tree_subcategory = "Calibration Test",
  slicer_menu_path = "Calibration → More… → Max Volumetric Speed",
  when_to_run = "Run 5th to measure the true melting limit (mm³/s) of your hotend + nozzle + filament combination.",
  formula_summary = "Safe_MVS = (Start_Flow + Measured_Height_Z × Step_Per_mm) × (1 - Safety_Margin_Pct / 100)",
  default_condition_expr = "filament:PCTG",
  how_to_read_print = {
    "1. Print the Max Volumetric Speed test (default: Start = 8.0 mm³/s, Step = 0.5 mm³/s per 1 mm of Z height).",
    "2. Use calipers to measure the height Z (in mm) from the bottom build plate right where the glossy wall turns matte, develops wavy under-extrusion ripples, or starts skipping.",
    "3. Enter Start Flow, Step per mm, and Measured Height Z below: we compute Peak_Flow = Start + (Z × Step) and apply your safety margin!",
  },
  hardware_checklist = {
    "🖨️ Slice and print the built-in test model from the AnycubicSlicerNext / OrcaSlicer top 'Calibration' menu before entering your measurements below.",
  },
}

params = {
  {
    id = "start_flow",
    label = "1. What Starting Volumetric Flow (mm³/s) did you set in the MVS dialog?",
    description = "Default in OrcaSlicer / AnycubicSlicerNext is 8.0 mm³/s (or 5.0 mm³/s for TPU).",
    type = "range",
    min = 1.0,
    max = 30.0,
    step = 0.5,
    default = "8.0",
    unit = "mm³/s",
    options = {
      { value = "5.0", label = "5.0 mm³/s" },
      { value = "8.0", label = "8.0 mm³/s (Default)" },
      { value = "10.0", label = "10.0 mm³/s" },
      { value = "12.0", label = "12.0 mm³/s" },
    },
  },
  {
    id = "step_per_mm",
    label = "2. What Flow Step (mm³/s per 1 mm of Z height) did you set?",
    description = "Default in OrcaSlicer / AnycubicSlicerNext is 0.50 mm³/s per mm.",
    type = "range",
    min = 0.05,
    max = 5.0,
    step = 0.05,
    default = "0.50",
    unit = "mm³/s/mm",
    options = {
      { value = "0.25", label = "0.25 mm³/s per mm" },
      { value = "0.50", label = "0.50 mm³/s per mm (Default)" },
      { value = "1.00", label = "1.00 mm³/s per mm" },
    },
  },
  {
    id = "measured_z",
    label = "3. At what height Z (mm) from the bottom did the print start turning matte or wavy?",
    description = "Example: Start 8.0 + (14 mm × 0.5) = 15.0 mm³/s peak → 13.5 mm³/s with 10% safety margin.",
    type = "range",
    min = 1.0,
    max = 60.0,
    step = 0.5,
    default = "14.0",
    unit = "mm",
    options = {
      { value = "10.0", label = "10 mm" },
      { value = "12.0", label = "12 mm" },
      { value = "14.0", label = "14 mm" },
      { value = "16.0", label = "16 mm" },
      { value = "20.0", label = "20 mm" },
      { value = "24.0", label = "24 mm" },
    },
  },
  {
    id = "margin_pct",
    label = "4. What safety margin below the failure point should be kept?",
    description = "A 10% safety margin prevents under-extrusion during long high-speed infill runs.",
    type = "range",
    min = 0,
    max = 40,
    step = 5,
    default = "10",
    unit = "%",
    options = {
      { value = "10", label = "10% Safety Margin (Recommended)" },
      { value = "15", label = "15% Conservative Margin" },
      { value = "5", label = "5% Aggressive" },
    },
  },
}

function validate(p, ctx)
  local start = tonumber(p.start_flow) or 8.0
  local step = tonumber(p.step_per_mm) or 0.50
  local z_mm = tonumber(p.measured_z) or 14.0
  local margin = tonumber(p.margin_pct) or 10.0
  local safe_mvs = (start + (z_mm * step)) * (1.0 - margin / 100.0)
  if safe_mvs < 1.5 or safe_mvs > 50.0 then
    return false, string.format("Computed Safe MVS (%.1f mm³/s) is outside the valid range [1.5, 50.0] mm³/s.", safe_mvs)
  end
  return true
end

function describe(p, ctx)
  local start = tonumber(p.start_flow) or 8.0
  local step = tonumber(p.step_per_mm) or 0.50
  local z_mm = tonumber(p.measured_z) or 14.0
  local margin = tonumber(p.margin_pct) or 10.0
  local peak_flow = start + (z_mm * step)
  local safe_mvs = math.max(2.0, math.min(50.0, peak_flow * (1.0 - margin / 100.0)))
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Peak = %.1f + (%.1fmm × %.2f) = %.1f mm³/s → Safe MVS (-%.0f%%) = %.1f mm³/s", start, z_mm, step, peak_flow, margin, safe_mvs)
  return {
    title = string.format("Calibration: Max Volumetric Speed = %.1f mm³/s", safe_mvs),
    description = string.format("Post-print MVS Race Track calibration for %s: failure at Z=%.1fmm (%.1f mm³/s peak) with %.0f%% margin → %.1f mm³/s.", scope, z_mm, peak_flow, margin, safe_mvs),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local start = tonumber(p and p.start_flow) or 8.0
  local step = tonumber(p and p.step_per_mm) or 0.50
  local z_mm = tonumber(p and p.measured_z) or 14.0
  local margin = tonumber(p and p.margin_pct) or 10.0
  local peak_flow = start + (z_mm * step)
  local safe_mvs = math.max(2.0, math.min(50.0, peak_flow * (1.0 - margin / 100.0)))
  config["filament_max_volumetric_speed"] = string.format("%.1f", safe_mvs)
  local d = describe(p or {}, ctx)
  print("[Calibration:MVS] " .. (d.badge or ""))
end
