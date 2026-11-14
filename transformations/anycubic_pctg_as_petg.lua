-- Machine Compatibility Rule: Anycubic PCTG -> PETG & PC-CF -> PC `filament_type` Declaration
--
-- AnycubicSlicerNext / ACE Pro 2 ships an older, hard-coded `filament_type` enum that does NOT
-- contain `PCTG` (OrcaSlicer added it later) and ACE Pro 2 lacks `PC-CF` (only `PC` is available).
-- A filament preset declaring `"filament_type": ["PCTG"]` is rejected / silently reset by
-- AnycubicSlicerNext, and `"PC-CF"` cannot be matched to a `PC`-loaded ACE Pro 2 slot.
--
-- This rule declares:
--   * Every PCTG-family spool (`PCTG`, `PCTG-CF`, `PCTG-GF`, ...) as `PETG`
--   * Every reinforced PC-family spool (`PC-CF`, `PC-GF`, `PC-RCF`, `PC-RGF`) as `PC`
-- in the exported filament presets / `.3mf` project_settings.config when the active rig is an
-- Anycubic machine. ONLY the declared `filament_type` string changes. Everything else stays true
-- to the underlying polymer:
--   * The internal spool keeps `material_type = PCTG` or `PC-CF`, so polymer-scoped rules and
--     declarative scope filters still apply.
--   * Temperature brackets stay on the true polymer limits (PCTG: 240–275°C nozzle, 70–90°C bed;
--     PC-CF: 255–295°C nozzle, 80–110°C bed, hardened nozzle required).
--   * A marker is appended to `filament_notes` so re-importing the preset from disk restores PCTG
--     or PC-CF (and its brackets) even if the preset name does not contain "PCTG" or "PC-CF".
--
-- Implementation: sets the `_anycubic_pctg_as_petg` and `_anycubic_pccf_as_pc` side-channels
-- consumed by the per-spool filament exporter (`resolve_spool_orca_filament_map_with_rig_and_overrides`).

params = {
  {
    id = "target_printers",
    label = "Apply To",
    description = "Anycubic Only (AnycubicSlicerNext / ACE Pro 2 lacks PCTG and PC-CF) or All Printers (force PETG/PC declaration everywhere).",
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

local function is_pctg_material(m)
  return string.find(string.upper(tostring(m or "")), "PCTG", 1, true) ~= nil
end

local function is_pccf_material(m)
  local u = string.upper(tostring(m or ""))
  return string.find(u, "PC-CF", 1, true) ~= nil
    or string.find(u, "PC-GF", 1, true) ~= nil
    or string.find(u, "PC-RCF", 1, true) ~= nil
    or string.find(u, "PC-RGF", 1, true) ~= nil
end

local function collect_matching_materials(ctx, pred)
  local seen = {}
  local out = {}
  local function add(m)
    local s = tostring(m or "")
    if pred(s) and not seen[s] then
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
    title = "Anycubic PCTG → PETG & PC-CF → PC Filament Type Declaration",
    description = string.format(
      "Declares PCTG-family spools as filament_type = PETG and PC-CF/PC-GF spools as filament_type = PC %s (AnycubicSlicerNext / ACE Pro 2 lacks PCTG and PC-CF). Temperatures, ranges, and polymer-scoped rules stay on true PCTG (240–275°C) and PC-CF (255–295°C) limits.",
      scope_txt
    ),
    badge = (target == "all") and "All Printers" or "Anycubic",
  }
end

function apply(config, ctx, p)
  local target = (p and p.target_printers) or "anycubic"
  if target ~= "all" and not is_anycubic_rig(ctx) then
    print(string.format(
      "[Anycubic:Compat] Rig '%s' is not Anycubic; PCTG and PC-CF filament_type declarations left unchanged",
      tostring((ctx and ctx.printer_model) or "")
    ))
    return
  end

  config["_anycubic_pctg_as_petg"] = "1"
  config["_anycubic_pccf_as_pc"] = "1"

  local pctg = collect_matching_materials(ctx, is_pctg_material)
  if #pctg > 0 then
    print(string.format(
      "[Anycubic:PCTG->PETG] Declaring %s as filament_type=PETG; temps & brackets stay on PCTG limits",
      table.concat(pctg, ", ")
    ))
  else
    print("[Anycubic:PCTG->PETG] Armed: no PCTG spools loaded; any PCTG spool will export as filament_type=PETG")
  end

  local pccf = collect_matching_materials(ctx, is_pccf_material)
  if #pccf > 0 then
    print(string.format(
      "[Anycubic:PC-CF->PC] Declaring %s as filament_type=PC; temps & brackets stay on PC-CF limits",
      table.concat(pccf, ", ")
    ))
  else
    print("[Anycubic:PC-CF->PC] Armed: no PC-CF spools loaded; any PC-CF spool will export as filament_type=PC")
  end
end
