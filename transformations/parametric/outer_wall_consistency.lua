-- Parametric Slicer Config Pack: CoreXY Outer-Wall Speed Consistency & VFA / Gloss-Banding Guard
--
-- Why the Math & Physics Matter on CoreXY (Kobra S1, Bambu X1/P1, Voron):
-- 1. The CoreXY VFA Resonance Band (70 - 130 mm/s):
--    Vertical Fine Artifacts (VFAs) originate from 1.8 deg stepper motors and 2GT belt teeth meshing
--    with idlers at resonant frequencies between ~70 mm/s and 130 mm/s. To avoid VFA ripple on outer
--    walls, the toolhead must either crawl UNDER the band (<= 60 mm/s) or blast PAST it (>= 145 mm/s).
-- 2. The MVS Trap (Flow Rate = Layer_Height * Line_Width * Speed):
--    At 0.20mm layer height and 0.42mm outer wall width, 200 mm/s requires 16.8 mm3/s.
--    When printing PETG, Silk PLA, TPU, or ABS/ASA with MVS = 8 - 11 mm3/s, the slicer silently
--    clamps outer_wall_speed to 10 / (0.20 * 0.42) = 119 mm/s -- dead center in the VFA resonance
--    band! Stepping outer_wall_speed down to 55 - 60 mm/s (~4.6 - 5.0 mm3/s) guarantees uniform,
--    glossy walls and zero stepper ripple.
-- 3. Gloss Banding (dont_slow_down_outer_wall = "1") & The Spire Caveat:
--    When minimum layer time slows a short layer from 200 mm/s (matte) down to 80 mm/s (shiny),
--    horizontal gloss bands ("Benchy hull line") appear. Locking outer wall speed eliminates gloss
--    stripes while the slicer still slows inner walls and infill for cooling.
--    Caveat: On miniatures or thin, hollow vertical spires (1-2 walls, 0% infill) printed in high-MVS
--    PLA at 200 mm/s (especially 0.12mm Fine), keeping outer walls at 200 mm/s can overheat a tiny 3mm tip.
--    In Auto Guard mode, 0.12mm Fine with High-MVS PLA automatically leaves dont_slow_down_outer_wall = "0"
--    (or you can pick the "Miniatures & Delicate Spires" preset explicitly).

wizard = {
  id = "outer_wall_consistency",
  name = "CoreXY Outer-Wall Speed Consistency & VFA / Gloss-Banding Guard",
  category = "Core Slicer Config Pack",
  icon = "〰️",
  slicer_menu_path = "Speed → Outer wall / Cooling → Don't slow down outer wall (`dont_slow_down_outer_wall`, `outer_wall_speed`, `small_perimeter_speed`)",
  when_to_run = "Default-on for CoreXY rigs (Kobra S1, Bambu X1/P1, Voron): eliminates 70–130 mm/s VFA belt ripples and Benchy hull-line gloss banding for PETG, Silk PLA, ABS/ASA, TPU, and 0.20–0.28mm PLA parts, while automatically keeping layer-time slowdown enabled for 0.12mm Fine PLA miniatures & tiny spires.",
  default_scope = "all",
  tree_category = "walls_surfaces",
  tree_subcategory = "CoreXY VFA & Gloss-Banding Guard",
  defect_Preview = "Horizontal gloss/matte bands ('Benchy hull line') on layer-time transitions + 2mm pitch vertical ripples (VFAs) when MVS clamps outer walls into 70–130 mm/s",
  fix_summary = "Locks dont_slow_down_outer_wall = 1 for uniform sheen (auto-disabled on 0.12mm Fine high-MVS PLA for tiny spires) and steps outer_wall_speed down to 55–60 mm/s whenever MVS or Silk finish cannot sustain >= 145 mm/s.",
  hardware_checklist = {
    "Check X/Y CoreXY 2GT belt tension equalized on both motors (uneven tension amplifies 70–120 mm/s VFA ringing)",
    "For tiny hollow 3mm spires/miniatures at 200 mm/s in PLA, use 0.12mm Fine (auto Spire-Safe) or select 'Miniatures & Delicate Spires'",
  },
}

params = {
  {
    id = "consistency_mode",
    label = "1. Outer-Wall Consistency & VFA Guard Preset",
    description = "Auto VFA + Gloss Guard (context-aware for CoreXY, polymer MVS, Silk finish, and 0.12mm Fine spires), Spire-Safe VFA Guard (always allow cooling slowdown on tips), or Sub-Resonance Mirror Gloss (force 55 mm/s).",
    type = "enum",
    default = "auto_guard",
    options = {
      {
        value = "auto_guard",
        label = "Auto VFA & Gloss-Banding Guard — Context-Aware by Machine, Polymer & Layer Tier (Recommended)",
        sets = {
          dont_slow_down_outer_wall = "auto",
          sub_vfa_speed = "55",
          vfa_clear_speed = "145",
          small_perimeter_speed = "50",
        },
      },
      {
        value = "spire_safe",
        label = "Miniatures & Delicate Vertical Spires — VFA Guard Only (Allow Cooling Slowdown on Tips)",
        sets = {
          dont_slow_down_outer_wall = "0",
          sub_vfa_speed = "55",
          vfa_clear_speed = "145",
          small_perimeter_speed = "50",
        },
      },
      {
        value = "sub_resonance_gloss",
        label = "Sub-Resonance Mirror Gloss — Silk PLA / PETG / Aesthetic (Force 55 mm/s Outer Wall)",
        sets = {
          dont_slow_down_outer_wall = "1",
          sub_vfa_speed = "55",
          vfa_clear_speed = "145",
          small_perimeter_speed = "45",
        },
      },
    },
  },
  {
    id = "dont_slow_down_outer_wall",
    label = "2. Lock Outer Wall Speed Against Layer-Time Cooling (`dont_slow_down_outer_wall`)",
    description = "Auto (1 on PETG/Silk/ABS/ASA/TPU & 0.20-0.28mm PLA; 0 on 0.12mm Fine high-MVS PLA for delicate spires), Always Lock (1), or Always Allow Slowdown (0).",
    type = "enum",
    default = "auto",
    options = {
      { value = "auto", label = "Auto — Lock (1) Except 0.12mm Fine High-MVS PLA Miniatures/Spires (0)" },
      { value = "1", label = "1 — Always Lock Outer Wall Speed (Eliminates Gloss Banding & VFA Drift)" },
      { value = "0", label = "0 — Always Allow Layer-Time Slowdown (Safe for Tiny Hollow Spires at 200 mm/s)" },
    },
  },
  {
    id = "sub_vfa_speed",
    label = "3. Sub-Resonance Outer Wall Speed (When MVS Cannot Clear VFA Band)",
    description = "Target outer wall speed (40 - 65 mm/s) below the 70 - 130 mm/s CoreXY stepper/belt VFA resonance band. 55 - 60 mm/s (~4.6 - 5.0 mm3/s) gives uniform glossy walls and zero VFA ripple.",
    type = "range",
    min = 40,
    max = 65,
    step = 5,
    default = "55",
    unit = "mm/s",
    options = {
      { value = "50", label = "50 mm/s (Ultra-Gloss)" },
      { value = "55", label = "55 mm/s (Recommended Sub-VFA)" },
      { value = "60", label = "60 mm/s (HQ Standard)" },
    },
  },
  {
    id = "vfa_clear_speed",
    label = "4. Minimum High-Speed Threshold to Clear CoreXY VFA Band",
    description = "Minimum MVS-sustainable speed (135 - 180 mm/s) required to stay above the 70 - 130 mm/s CoreXY VFA resonance zone. If MVS / (layer_height * line_width) is below this threshold, outer_wall_speed steps down to Sub-Resonance Speed.",
    type = "range",
    min = 135,
    max = 180,
    step = 5,
    default = "145",
    unit = "mm/s",
    options = {
      { value = "140", label = "140 mm/s" },
      { value = "145", label = "145 mm/s (Recommended)" },
      { value = "150", label = "150 mm/s (Strict Clear)" },
    },
  },
  {
    id = "small_perimeter_speed",
    label = "5. Small Perimeter Speed (`small_perimeter_speed`)",
    description = "Speed for small holes, bosses, and pins (35 - 60 mm/s) kept cleanly below the 70 mm/s VFA resonance onset.",
    type = "range",
    min = 35,
    max = 60,
    step = 5,
    default = "50",
    unit = "mm/s",
    options = {
      { value = "45", label = "45 mm/s" },
      { value = "50", label = "50 mm/s (Recommended)" },
      { value = "55", label = "55 mm/s" },
    },
  },
}

local function is_corexy_printer(ctx)
  local model = string.upper(tostring((ctx and ctx.printer_model) or ""))
  if model == "" then
    return true
  end
  if string.find(model, "A1", 1, true)
    or string.find(model, "KOBRA 2", 1, true)
    or string.find(model, "KOBRA 3", 1, true)
    or string.find(model, "ENDER 3", 1, true)
    or string.find(model, "PRUSA MK", 1, true)
  then
    return false
  end
  return true
end

local function infer_polymer_mvs(config, ctx)
  local cfg_v = nil
  if config["filament_max_volumetric_speed"] and config["filament_max_volumetric_speed"][1] then
    cfg_v = tonumber(config["filament_max_volumetric_speed"][1])
  end
  local ctx_v = tonumber(ctx and ctx.filament_max_flow)
  if cfg_v and cfg_v > 0 and (not ctx_v or math.abs(cfg_v - 12.0) > 0.01) then
    return cfg_v
  end
  if ctx_v and ctx_v > 0 then
    return ctx_v
  end
  if cfg_v and cfg_v > 0 then
    return cfg_v
  end
  local ftype = string.upper(tostring((ctx and ctx.filament_type) or "PLA"))
  local finish = string.upper(tostring((ctx and ctx.filament_finish) or ""))
  if string.find(ftype, "SILK", 1, true) or string.find(finish, "SILK", 1, true) then
    return 9.0
  elseif string.find(ftype, "TPU", 1, true) or string.find(ftype, "PEBA", 1, true) then
    return 6.0
  elseif string.find(ftype, "PETG", 1, true) or string.find(ftype, "PCTG", 1, true) then
    return 10.5
  elseif string.find(ftype, "ABS", 1, true)
    or string.find(ftype, "ASA", 1, true)
    or string.find(ftype, "PC", 1, true)
    or string.find(ftype, "PA", 1, true)
  then
    return 11.5
  end
  return 18.0
end

function validate(p, ctx)
  local sub_spd = tonumber(p.sub_vfa_speed) or 55
  local clear_spd = tonumber(p.vfa_clear_speed) or 145
  if sub_spd > 68 then
    return false, "Sub-resonance outer wall speed must be <= 65 mm/s to stay below the 70 - 130 mm/s CoreXY VFA resonance band."
  end
  if clear_spd < 130 then
    return false, "VFA clear speed threshold must be >= 135 mm/s to stay above the 70 - 130 mm/s CoreXY VFA resonance band."
  end
  return true
end

function describe(p, ctx)
  local mode = p.consistency_mode or "auto_guard"
  local lock_ow = p.dont_slow_down_outer_wall or "auto"
  local sub_spd = tonumber(p.sub_vfa_speed) or 55
  local clear_spd = tonumber(p.vfa_clear_speed) or 145
  local mode_lbl = "Auto VFA & Gloss Guard"
  if mode == "spire_safe" then
    mode_lbl = "Spire-Safe VFA Guard"
  elseif mode == "sub_resonance_gloss" then
    mode_lbl = "Sub-Resonance Mirror Gloss"
  end
  local badge = string.format(
    "%s: dont_slow_down_outer_wall=%s • Sub-VFA %d mm/s if MVS < %d mm/s",
    mode_lbl, lock_ow, sub_spd, clear_spd
  )
  return {
    title = string.format("Wizard: CoreXY Outer-Wall VFA & Gloss Guard (%s)", mode_lbl),
    description = string.format(
      "Eliminates CoreXY 70–130 mm/s VFA belt ripples and Benchy hull-line gloss banding: dont_slow_down_outer_wall = %s, steps outer_wall_speed down to %d mm/s whenever MVS cannot sustain >= %d mm/s.",
      lock_ow, sub_spd, clear_spd
    ),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local mode = (p and p.consistency_mode) or "auto_guard"
  local lock_pref = (p and p.dont_slow_down_outer_wall) or "auto"
  local sub_spd = math.floor((tonumber(p and p.sub_vfa_speed) or 55) + 0.5)
  local clear_spd = math.floor((tonumber(p and p.vfa_clear_speed) or 145) + 0.5)
  local small_spd = math.floor((tonumber(p and p.small_perimeter_speed) or 50) + 0.5)

  if not is_corexy_printer(ctx) and mode == "auto_guard" then
    print("[OuterWallConsistency] Non-CoreXY kinematics detected; skipping CoreXY 70-130 mm/s VFA guard")
    return
  end

  local hotend_flow = tonumber(ctx and ctx.hotend_max_flow) or 22.0
  local filament_flow = infer_polymer_mvs(config, ctx)
  local effective_flow = (hotend_flow < 14.0) and math.min(hotend_flow, filament_flow) or filament_flow

  local ftype = string.upper(tostring((ctx and ctx.filament_type) or "PLA"))
  local finish = string.upper(tostring((ctx and ctx.filament_finish) or ""))
  local is_silk = string.find(ftype, "SILK", 1, true) ~= nil or string.find(finish, "SILK", 1, true) ~= nil

  local layer_h = tonumber(config["layer_height"]) or tonumber(ctx and ctx.layer_height) or 0.20
  local outer_w = tonumber(config["outer_wall_line_width"]) or tonumber(ctx and ctx.line_width) or 0.42
  local cross_section = math.max(0.01, layer_h * outer_w)

  local mvs_ceiling = math.floor(effective_flow / cross_section)
  local cur_outer_spd = tonumber(config["outer_wall_speed"]) or 200
  local actual_outer_spd = math.min(cur_outer_spd, mvs_ceiling)

  local new_outer_spd = cur_outer_spd
  local stepped_down = false
  if mode == "sub_resonance_gloss" or (is_silk and cur_outer_spd > (sub_spd + 5)) then
    new_outer_spd = sub_spd
    stepped_down = true
  elseif actual_outer_spd > (sub_spd + 5) and actual_outer_spd < clear_spd then
    -- MVS or profile speed would trap the outer wall inside the 70 - 130 mm/s CoreXY VFA resonance band!
    new_outer_spd = sub_spd
    stepped_down = true
  end

  -- Resolve dont_slow_down_outer_wall:
  -- In "auto" mode:
  -- - If outer wall is stepped down to <= 65 mm/s (low-MVS PETG/TPU/ABS/ASA or Silk PLA), locking is 100% safe even on spires.
  -- - If printing 0.12mm Fine (layer_h <= 0.145) at high speed (>= 145 mm/s, e.g. High-MVS PLA miniatures & thin hollow spires),
  --   keep dont_slow_down_outer_wall = "0" so tiny 3mm tips can slow down to cool!
  -- - On 0.20mm / 0.28mm cosmetic & functional parts, lock dont_slow_down_outer_wall = "1" to prevent Benchy hull-line gloss banding.
  local lock_ow = lock_pref
  if lock_pref == "auto" then
    if mode == "spire_safe" then
      lock_ow = "0"
    elseif not stepped_down and not is_silk and new_outer_spd >= clear_spd and layer_h <= 0.145 then
      lock_ow = "0"
    else
      lock_ow = "1"
    end
  end

  config["dont_slow_down_outer_wall"] = lock_ow
  config["small_perimeter_speed"] = tostring(small_spd)
  if stepped_down then
    config["outer_wall_speed"] = tostring(new_outer_spd)
  end

  -- Side-channel metadata so per-process / standalone export can evaluate each process tier's cross-section
  config["_outer_wall_vfa_mode"] = mode
  config["_outer_wall_vfa_lock_pref"] = lock_pref
  config["_outer_wall_vfa_is_silk"] = is_silk and "1" or "0"
  config["_outer_wall_vfa_sub_speed"] = tostring(sub_spd)
  config["_outer_wall_vfa_clear_speed"] = tostring(clear_spd)
  config["_outer_wall_vfa_effective_mvs"] = fmt_num(effective_flow, "%.2f")

  print(string.format(
    "[OuterWallConsistency:%s] dont_slow_down_outer_wall=%s, MVS=%.1f mm3/s @ %.2fx%.2fmm (ceiling %d mm/s) -> outer_wall_speed=%d mm/s%s",
    mode,
    lock_ow,
    effective_flow,
    layer_h,
    outer_w,
    mvs_ceiling,
    new_outer_spd,
    stepped_down and " (stepped below 70-130 mm/s VFA band)" or (lock_ow == "0" and " (Spire-Safe cooling slowdown enabled)" or " (clears VFA band)")
  ))
end
