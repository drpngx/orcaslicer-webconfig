-- Symptom 2: Multi-Material Tip Forming & Anti-Clog Rule (ACE Pro / Double ACE Pro 2)
-- Prevents heat-creep softening & unpurged high-temp residue during multi-material swaps (e.g., PETG -> PLA):
-- 1. Holds transition flush temp at 240°C during active purge (matching Anycubic PLA's nozzle_temperature_range_high = 240°C
--    so residual PETG at 235°C is completely flushed before cooling down to PLA printing temp).
-- 2. Keeps slicer cut_retraction_length at "0" on Kobra S1 (since KobraOS firmware T[next_extruder] performs the 18mm cutter
--    retraction internally; adding another 18mm in slicer G-code pulls molten filament into the cold throat).
-- 3. Ensures flush_multiplier is at least 1.45x (or preserves the user's 1.50x multiplier).

function apply(config, ctx)
  config["cut_retraction_length"] = "0"
  config["transition_flush_temp_hold"] = "240"
  local cur_mult = tonumber(ctx.flush_multiplier) or tonumber(config["flush_multiplier"]) or 1.0
  if cur_mult < 1.45 then
    config["flush_multiplier"] = "1.45"
  else
    config["flush_multiplier"] = string.format("%.2f", cur_mult)
  end

  print("[Symptom:ACEProAntiClog] Transition flush hold -> 240C (PLA max safe range), Slicer cut retract -> 0mm (handled by KobraOS T[n] macro), Flush multiplier -> " .. config["flush_multiplier"] .. "x")
end
