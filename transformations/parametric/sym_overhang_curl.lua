-- Symptom Fix: Overhang Curling, Drooping Bridges & Corner Pillowing
meta = {
  id = "sym_overhang_curl",
  name = "Fix Overhang Curling, Drooping Bridges & Corner Pillowing",
  icon = "🌀",
  category = "Filament Symptom Fix (Overhangs & Cooling)",
  tree_category = "6. Overhangs, Bridging & Supports",
  tree_subcategory = "Steep Overhang Cooling",
  slicer_menu_path = "Overhang Fan Speed Boost + Relative Minimum Layer Time Boost + Inner/Outer Wall Order",
  when_to_run = "Apply when steep overhangs curl upward (causing the nozzle to knock into them), bridges sag, or small spires overheat.",
  formula_summary = "Overhang_Fan_new = min(100%, Fan_old + Fan_Boost%)  •  Layer_Time_new = Layer_Time_old + Extra_Seconds  •  Wall Sequence = inner/outer",
  default_condition_expr = "all",
  how_to_read_print = {
    "1. Overhang curling happens when a new layer is deposited before the underlying overhang perimeter has solidified.",
    "2. Adding +3s to +5s to minimum layer time (`slow_down_layer_time`), boosting overhang fan by +15% to +25% (up to 100%), and printing Inner-then-Outer walls anchors overhangs cleanly.",
  },
  hardware_checklist = {
    "🌀 Orient Steep Overhangs Toward the Part Cooling Fan: Rotate the model on the build plate so the steepest overhang faces the strongest side of the toolhead cooling duct.",
  },
}

params = {
  {
    id = "overhang_fan",
    label = "1. How much should we boost Fan Speed on steep overhangs & bridges (overhang_fan_speed)?",
    description = "Adding +20% to +30% (capped at 100%) freezes unsupported overhang edges even on low-fan filaments like PETG/PCTG.",
    type = "range",
    min = 0,
    max = 60,
    step = 5,
    default = "+20%",
    unit = "%",
    options = {
      { value = "+10%", label = "+10% Overhang Fan Boost (Mild)" },
      { value = "+20%", label = "+20% Overhang Fan Boost (Recommended)" },
      { value = "+35%", label = "+35% / 100% Max Overhang Freeze" },
    },
  },
  {
    id = "layer_time",
    label = "2. How many extra seconds should we add to Minimum Layer Time (slow_down_layer_time)?",
    description = "Adding +4s relative to your profile's current layer time ensures small features and overhang tips cool below glass-transition before the next layer arrives.",
    type = "range",
    min = 0,
    max = 15,
    step = 1,
    default = "+4",
    unit = "s",
    options = {
      { value = "+2", label = "+2s Extra Layer Cooling Time (Fast / High-Flow)" },
      { value = "+4", label = "+4s Extra Layer Cooling Time (Recommended Anti-Curl)" },
      { value = "+6", label = "+6s Extra Layer Cooling Time (Small Spires / Heavy Overhangs)" },
    },
  },
  {
    id = "wall_seq",
    label = "3. Which Wall Printing Sequence should be used?",
    description = "Inner/Outer prints the inner wall first so the outer overhang perimeter has an adjacent wall to bond against.",
    type = "enum",
    default = "inner wall/outer wall",
    unit = "",
    options = {
      { value = "inner wall/outer wall", label = "inner wall/outer wall (Best Overhang Support)" },
      { value = "inner-outer-inner wall", label = "inner-outer-inner wall (Best Dimensional Accuracy)" },
    },
  },
}

function validate(p, ctx)
  return true
end

local function is_warp_prone_polymer(config, ctx)
  if ctx and ctx.filament_is_warp_prone == true then
    return true
  end
  local ft = ""
  if ctx and ctx.filament_type then
    ft = tostring(ctx.filament_type)
  elseif config and config["filament_type"] then
    local raw = config["filament_type"]
    ft = type(raw) == "table" and tostring(raw[1] or "") or tostring(raw)
  end
  local up = string.upper(ft)
  if string.find(up, "PCTG", 1, true) ~= nil then
    return false
  end
  return string.find(up, "ABS", 1, true) ~= nil
    or string.find(up, "ASA", 1, true) ~= nil
    or string.find(up, "HIPS", 1, true) ~= nil
    or string.find(up, "PC", 1, true) ~= nil
    or string.find(up, "PPA", 1, true) ~= nil
    or string.find(up, "PAHT", 1, true) ~= nil
    or string.find(up, "PA6", 1, true) ~= nil
    or string.find(up, "PA12", 1, true) ~= nil
    or up == "PA"
    or string.find(up, "PA%-") ~= nil
    or string.find(up, "NYLON", 1, true) ~= nil
end

function describe(p, ctx)
  local ov_boost = math.floor((tonumber(p.overhang_fan) or 20) + 0.5)
  local l_time_boost = math.floor((tonumber(p.layer_time) or 4) + 0.5)
  local w_seq = p.wall_seq or "inner wall/outer wall"
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local cap_pct = is_warp_prone_polymer(nil, ctx) and 35 or 100
  local badge = string.format("Overhang Fan %+d%% (≤%d%%) • Min Layer Time %+ds • Order = %s", ov_boost, cap_pct, l_time_boost, w_seq)
  return {
    title = string.format("Symptom Fix: Overhang Curling (%+d%% Fan, %+ds Layer Time)", ov_boost, l_time_boost),
    description = string.format("Stops steep overhang curling and bridge sag for %s: boosts overhang fan by %+d%% (up to %d%%), adds %+ds to minimum layer time, and sets %s wall sequence.", scope, ov_boost, cap_pct, l_time_boost, w_seq),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local ov_boost = tonumber(p and p.overhang_fan) or 20
  local l_time_boost = tonumber(p and p.layer_time) or 4
  local w_seq = (p and p.wall_seq) or "inner wall/outer wall"

  local function set_arr(key, val)
    if type(config[key]) == "string" then
      config[key] = val
    else
      config[key] = { val }
    end
  end

  local warp_prone = is_warp_prone_polymer(config, ctx)
  local fan_cap = warp_prone and 35 or 100
  local default_of = warp_prone and 20 or 80
  local cur_of = scalar_num(config["overhang_fan_speed"], default_of)
  set_arr("overhang_fan_speed", fmt_num(math.min(fan_cap, cur_of + ov_boost), "%.0f"))
  local cur_lt = scalar_num(config["slow_down_layer_time"], 6)
  set_arr("slow_down_layer_time", fmt_num(math.max(4, cur_lt + l_time_boost), "%.0f"))
  set_arr("slow_down_for_layer_cooling", "1")
  config["wall_sequence"] = w_seq
  local d = describe(p or {}, ctx)
  print("[Symptom:Overhangs] " .. (d.badge or ""))
end
