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

function describe(p, ctx)
  local ov_boost = math.floor(tonumber(p.overhang_fan) or 20)
  local l_time_boost = math.floor(tonumber(p.layer_time) or 4)
  local w_seq = p.wall_seq or "inner wall/outer wall"
  local scope = (ctx and ctx.scope) or "Selected Profile"
  local badge = string.format("Overhang Fan %+d%% (≤100%%) • Min Layer Time %+ds • Order = %s", ov_boost, l_time_boost, w_seq)
  return {
    title = string.format("Symptom Fix: Overhang Curling (%+d%% Fan, %+ds Layer Time)", ov_boost, l_time_boost),
    description = string.format("Stops steep overhang curling and bridge sag for %s: boosts overhang fan by %+d%% (up to 100%%), adds %+ds to minimum layer time, and sets %s wall sequence.", scope, ov_boost, l_time_boost, w_seq),
    badge = badge,
  }
end

function apply(config, ctx, p)
  local ov_boost = math.floor(tonumber(p and p.overhang_fan) or 20)
  local l_time_boost = math.floor(tonumber(p and p.layer_time) or 4)
  local w_seq = (p and p.wall_seq) or "inner wall/outer wall"

  local cur_of = tonumber(config["overhang_fan_speed"]) or 80
  config["overhang_fan_speed"] = tostring(math.min(100, math.floor(cur_of + ov_boost)))
  local cur_lt = tonumber(config["slow_down_layer_time"]) or 6
  config["slow_down_layer_time"] = tostring(math.max(4, math.floor(cur_lt + l_time_boost)))
  config["slow_down_for_layer_cooling"] = "1"
  config["wall_sequence"] = w_seq
  local d = describe(p or {}, ctx)
  print("[Symptom:Overhangs] " .. (d.badge or ""))
end
