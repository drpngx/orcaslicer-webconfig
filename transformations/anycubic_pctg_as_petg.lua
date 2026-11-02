-- Machine Compatibility Rule: Anycubic PCTG -> PETG `filament_type` Declaration
--
-- AnycubicSlicerNext ships an older, hard-coded `filament_type` enum that does NOT contain
-- `PCTG` (OrcaSlicer added it later). A filament preset declaring `"filament_type": ["PCTG"]`
-- is rejected / silently reset by AnycubicSlicerNext, so PCTG spools vanish from the dropdown.
--
-- This rule declares every PCTG-family spool (`PCTG`, `PCTG-CF`, `PCTG-GF`, ...) as `PETG` in the
-- exported filament presets / `.3mf` project_settings.config when the active rig is an Anycubic
-- machine. ONLY the declared `filament_type` string changes. Everything else stays PCTG:
--   * The internal spool keeps `material_type = PCTG`, so PCTG-scoped rules (e.g. PCTG Stringing
--     & Ooze Mitigation) and declarative `PCTG` scope filters still apply.
--   * Temperature brackets stay on the PCTG polymer limits (240–275°C nozzle, 70–90°C bed): the
--     effective nozzle temp, `nozzle_temperature_range_low/high`, and bed temps are clamped and
--     computed from PCTG, never from PETG's narrower 225–260°C window.
--   * PETG-only machine tweaks (Kobra un-bottleneck MVS 19 mm³/s / 0.70mm retraction) are NOT
--     applied to the PCTG spool just because it is declared as PETG.
--   * A marker is appended to `filament_notes` so re-importing the preset from disk restores PCTG
--     (and its brackets) even if the preset name does not contain "PCTG".
--
-- Implementation: sets the `_anycubic_pctg_as_petg` side-channel consumed by the per-spool
-- filament exporter (`resolve_spool_orca_filament_map_with_rig_and_overrides`).

params = {
  {
    id = "target_printers",
    label = "Apply To",
    description = "Anycubic Only (AnycubicSlicerNext hard-coded filament list lacks PCTG) or All Printers (force PETG declaration everywhere).",
    type = "enum",
    default = "anycubic",
    options = {
      { value = "anycubic", label = "Anycubic Rigs Only (Recommended)" },
      { value = "all", label = "All Printers" },
    },
  },
}

local function is_anycubic_rig(ctx)
  local model = string.upper(tostring((ctx and ctx.printer_model) or ""))
  if model == "" then
    -- No rig in context: the app's default machine is the Anycubic Kobra S1.
    return true
  end
  return string.find(model, "ANYCUBIC", 1, true) ~= nil or string.find(model, "KOBRA", 1, true) ~= nil
end

local function collect_pctg_materials(ctx)
  local seen = {}
  local out = {}
  local function add(m)
    local s = tostring(m or "")
    if string.find(string.upper(s), "PCTG", 1, true) and not seen[s] then
      seen[s] = true
      table.insert(out, s)
    end
  end
  if ctx then
    add(ctx.filament_type)
    if type(ctx.slots) == "table" then
      for _, s in ipairs(ctx.slots) do
        if type(s) == "table" then
          add(s.material_type)
        end
      end
    end
    if type(ctx.all_filament_types) == "table" then
      for _, m in ipairs(ctx.all_filament_types) do
        add(m)
      end
    end
  end
  return out
end

function describe(p, ctx)
  local target = (p and p.target_printers) or "anycubic"
  local scope_txt = (target == "all") and "on every printer" or "on Anycubic rigs"
  return {
    title = "Anycubic PCTG → PETG Filament Type Declaration",
    description = string.format(
      "Declares PCTG-family spools as filament_type = PETG %s (AnycubicSlicerNext's hard-coded list lacks PCTG). Temperatures, ranges, and PCTG-scoped rules stay on PCTG limits (240–275°C).",
      scope_txt
    ),
    badge = (target == "all") and "All Printers" or "Anycubic",
  }
end

function apply(config, ctx, p)
  local target = (p and p.target_printers) or "anycubic"
  if target ~= "all" and not is_anycubic_rig(ctx) then
    print(string.format(
      "[Anycubic:PCTG->PETG] Rig '%s' is not Anycubic; PCTG filament_type left unchanged",
      tostring((ctx and ctx.printer_model) or "")
    ))
    return
  end

  config["_anycubic_pctg_as_petg"] = "1"

  local pctg = collect_pctg_materials(ctx)
  if #pctg > 0 then
    print(string.format(
      "[Anycubic:PCTG->PETG] Declaring %s as filament_type=PETG; temps & brackets stay on PCTG limits",
      table.concat(pctg, ", ")
    ))
  else
    print("[Anycubic:PCTG->PETG] Armed: no PCTG spools loaded; any PCTG spool will export as filament_type=PETG")
  end
end
