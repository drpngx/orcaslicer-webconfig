-- Dynamic Combination Rule 3: Upgraded Flushing Volume Matrix Algorithm
-- Evaluates RGB luminance delta, polymer family cross-contamination (e.g. PLA <-> PETG = 300 mm3 base -> 450 mm3 at 1.5x),
-- pigment stickiness (1.15x for Carbon Fiber -CF, 1.25x for heavy red dyes), and optical translucency
-- across N active ACE Pro slots (4 or 8 slots).

local function hex_to_luminance(hex)
  if not hex or #hex < 7 then
    return 0.5
  end
  local r = tonumber(string.sub(hex, 2, 3), 16) or 128
  local g = tonumber(string.sub(hex, 4, 5), 16) or 128
  local b = tonumber(string.sub(hex, 6, 7), 16) or 128
  return (0.299 * r + 0.587 * g + 0.114 * b) / 255.0
end

local function polymer_family(mat)
  if not mat then
    return "PLA"
  end
  if mat == "PETG" or mat == "PCTG" or mat == "PETG-CF" or mat == "PETG-GF" or mat == "PCTG-CF" or mat == "PCTG-GF" or mat == "PET-CF" then
    return "PETG"
  elseif mat == "PLA" or mat == "PLA+" or mat == "PLA-CF" or mat == "PLA-GF" or mat == "PLA Matte" or mat == "PLA Silk" or mat == "PLA High Speed" then
    return "PLA"
  elseif mat == "ABS" or mat == "ASA" or mat == "ABS-GF" or mat == "ASA-CF" then
    return "STYRENIC"
  elseif mat == "TPU" or mat == "TPU 95A" or mat == "PEBA" or mat == "PEBA 90A" or mat == "PEBA-CF" or mat == "Rebound TPU" then
    return "ELASTOMER"
  elseif mat == "PA" or mat == "PA6-CF" or mat == "PAHT" or mat == "PAHT-CF" or mat == "PAHT-GF" or mat == "PPA" or mat == "PPA-CF" or mat == "PPA-GF" or mat == "PC" or mat == "PC-CF" or mat == "PC-GF" then
    return "ENGINEERING"
  end
  return mat
end

function apply(config, ctx)
  local slots = ctx.slots
  if not slots then
    return
  end

  local n = #slots
  if n == 0 then
    return
  end

  local melt_mult = tonumber(ctx.melt_zone_multiplier) or 1.0
  local flush_mult = tonumber(ctx.flush_multiplier) or tonumber(config["flush_multiplier"]) or 1.0
  local matrix = {}
  local idx = 1

  for i = 1, n do
    local s_out = slots[i]
    local lum_out = hex_to_luminance(s_out.color_hex)
    local stickiness = tonumber(s_out.pigment_stickiness) or 1.0
    local fam_out = polymer_family(s_out.material_type)

    for j = 1, n do
      if i == j then
        matrix[idx] = "0"
      else
        local s_in = slots[j]
        local lum_in = hex_to_luminance(s_in.color_hex)
        local fam_in = polymer_family(s_in.material_type)
        local opacity_in = tonumber(s_in.opacity) or 1.0
        if opacity_in < 0.25 then
          opacity_in = 0.25
        end

        local vol = 0
        -- 1. Incompatible Polymer Family Cross-Contamination (e.g. PLA <-> PETG):
        -- Base 300 mm3 at 1.0x -> 450 mm3 at 1.5x multiplier (preventing weak layer interfaces without wasting 886+ mm3)
        if fam_out ~= fam_in then
          local cross_base = 300.0
          if stickiness > 1.2 then
            cross_base = 300.0 * math.min(1.25, stickiness)
          end
          vol = cross_base * flush_mult
        else
          -- 2. Same Polymer Family Luminance Delta (Dark -> Light scales 180..300 at 1.0x; Light -> Dark scales 95..150 at 1.0x)
          local delta = lum_in - lum_out
          local base_vol = 160.0
          if delta > 0 then
            base_vol = 180.0 + (delta * 120.0)
          else
            base_vol = 95.0 + (math.abs(delta) * 55.0)
          end

          -- 3. Physical Multipliers: Pigment Stickiness & Receiving Translucency
          local translucency_factor = 1.0
          if opacity_in < 0.85 then
            translucency_factor = math.min(1.30, 1.0 / opacity_in)
          end
          vol = base_vol * stickiness * translucency_factor * flush_mult
        end

        -- 4. Clamp to [70, 900] mm3
        local clamped = math.max(70, math.min(900, math.floor(vol + 0.5)))
        matrix[idx] = tostring(clamped)
      end
      idx = idx + 1
    end
  end

  config["flush_multiplier"] = string.format("%.2f", flush_mult)
  config["flush_volumes_matrix"] = matrix
  print("[FlushMatrix] Computed " .. tostring(n) .. "x" .. tostring(n) .. " (" .. tostring(n * n) .. " cells) flushing matrix (flush_multiplier=" .. string.format("%.2f", flush_mult) .. "x, cross-polymer base=300 mm3 -> " .. tostring(math.floor(300.0 * flush_mult + 0.5)) .. " mm3)")
end
