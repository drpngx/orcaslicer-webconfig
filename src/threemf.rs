use crate::models::{FilamentSpool, MachineRig, ProcessProfile, WEBCONFIG_ORCA_PRESET_VERSION};
use serde::{Deserialize, Serialize};

/// Compute standard IEEE 802.3 CRC32 for ZIP archive entries
pub fn crc32_ieee(data: &[u8]) -> u32 {
  let mut crc: u32 = 0xFFFF_FFFF;
  for &byte in data {
    crc ^= byte as u32;
    for _ in 0..8 {
      if (crc & 1) != 0 {
        crc = (crc >> 1) ^ 0xEDB8_8320;
      } else {
        crc >>= 1;
      }
    }
  }
  !crc
}

/// Build an uncompressed (STORE method = 0) ZIP archive in memory from `(path, bytes)` entries.
pub fn build_zip_archive(files: &[(&str, &[u8])]) -> Vec<u8> {
  let mut out = Vec::new();
  let mut central_dir = Vec::new();

  for &(name, data) in files {
    let name_bytes = name.as_bytes();
    let crc = crc32_ieee(data);
    let size = data.len() as u32;
    let local_header_offset = out.len() as u32;

    // Local File Header (30 bytes + filename)
    // Bit 11 (0x0800) = Language encoding flag (EFS): filename and comment are UTF-8 encoded
    let utf8_flag: u16 = 0x0800;
    out.extend_from_slice(&0x0403_4B50u32.to_le_bytes()); // signature
    out.extend_from_slice(&20u16.to_le_bytes()); // version needed (2.0)
    out.extend_from_slice(&utf8_flag.to_le_bytes()); // general purpose bit flag (UTF-8)
    out.extend_from_slice(&0u16.to_le_bytes()); // compression method: 0 = STORE
    out.extend_from_slice(&0u16.to_le_bytes()); // last mod file time
    out.extend_from_slice(&0u16.to_le_bytes()); // last mod file date
    out.extend_from_slice(&crc.to_le_bytes()); // crc-32
    out.extend_from_slice(&size.to_le_bytes()); // compressed size
    out.extend_from_slice(&size.to_le_bytes()); // uncompressed size
    out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes()); // file name length
    out.extend_from_slice(&0u16.to_le_bytes()); // extra field length
    out.extend_from_slice(name_bytes);
    out.extend_from_slice(data);

    // Central Directory File Header (46 bytes + filename)
    central_dir.extend_from_slice(&0x0201_4B50u32.to_le_bytes()); // signature
    central_dir.extend_from_slice(&20u16.to_le_bytes()); // version made by
    central_dir.extend_from_slice(&20u16.to_le_bytes()); // version needed
    central_dir.extend_from_slice(&utf8_flag.to_le_bytes()); // flags (UTF-8)
    central_dir.extend_from_slice(&0u16.to_le_bytes()); // compression method: 0 = STORE
    central_dir.extend_from_slice(&0u16.to_le_bytes()); // mod time
    central_dir.extend_from_slice(&0u16.to_le_bytes()); // mod date
    central_dir.extend_from_slice(&crc.to_le_bytes()); // crc-32
    central_dir.extend_from_slice(&size.to_le_bytes()); // compressed size
    central_dir.extend_from_slice(&size.to_le_bytes()); // uncompressed size
    central_dir.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes()); // filename len
    central_dir.extend_from_slice(&0u16.to_le_bytes()); // extra len
    central_dir.extend_from_slice(&0u16.to_le_bytes()); // comment len
    central_dir.extend_from_slice(&0u16.to_le_bytes()); // disk number start
    central_dir.extend_from_slice(&0u16.to_le_bytes()); // internal file attrs
    central_dir.extend_from_slice(&0u32.to_le_bytes()); // external file attrs
    central_dir.extend_from_slice(&local_header_offset.to_le_bytes()); // local header offset
    central_dir.extend_from_slice(name_bytes);
  }

  let cd_offset = out.len() as u32;
  let cd_size = central_dir.len() as u32;
  let num_entries = files.len() as u16;

  out.extend_from_slice(&central_dir);

  // End of Central Directory Record (22 bytes)
  out.extend_from_slice(&0x0605_4B50u32.to_le_bytes()); // EOCD signature
  out.extend_from_slice(&0u16.to_le_bytes()); // disk number
  out.extend_from_slice(&0u16.to_le_bytes()); // disk with central dir
  out.extend_from_slice(&num_entries.to_le_bytes()); // entries on this disk
  out.extend_from_slice(&num_entries.to_le_bytes()); // total entries
  out.extend_from_slice(&cd_size.to_le_bytes()); // central dir size
  out.extend_from_slice(&cd_offset.to_le_bytes()); // central dir offset
  out.extend_from_slice(&0u16.to_le_bytes()); // comment length

  out
}

/// Extract a file by path from an uncompressed (STORE) ZIP archive in memory.
pub fn extract_file_from_zip(zip_bytes: &[u8], target_name: &str) -> Option<String> {
  let mut pos = 0usize;
  while pos + 30 <= zip_bytes.len() {
    let sig = u32::from_le_bytes([
      zip_bytes[pos],
      zip_bytes[pos + 1],
      zip_bytes[pos + 2],
      zip_bytes[pos + 3],
    ]);
    if sig != 0x0403_4B50 {
      break;
    }
    let method = u16::from_le_bytes([zip_bytes[pos + 8], zip_bytes[pos + 9]]);
    let comp_size = u32::from_le_bytes([
      zip_bytes[pos + 18],
      zip_bytes[pos + 19],
      zip_bytes[pos + 20],
      zip_bytes[pos + 21],
    ]) as usize;
    let name_len = u16::from_le_bytes([zip_bytes[pos + 26], zip_bytes[pos + 27]]) as usize;
    let extra_len = u16::from_le_bytes([zip_bytes[pos + 28], zip_bytes[pos + 29]]) as usize;

    let name_start = pos + 30;
    let data_start = name_start + name_len + extra_len;
    let data_end = data_start + comp_size;

    if data_end > zip_bytes.len() {
      break;
    }

    if let Ok(entry_name) = std::str::from_utf8(&zip_bytes[name_start..name_start + name_len]) {
      if entry_name == target_name && method == 0 {
        if let Ok(text) = std::str::from_utf8(&zip_bytes[data_start..data_end]) {
          return Some(text.to_string());
        }
      }
    }

    pos = data_end;
  }
  None
}

/// Resolve a guaranteed-valid, actually-instantiated system filament preset name for the active
/// `MachineRig` (or default machine), delegating strictly to `CombinedCatalog::resolve_filament_inherits`
/// (and preserving external user-disk `spool.inherits` only when verified in `instantiated_system_filaments`).
pub fn resolve_filament_inherits_for_rig(
  spool: &FilamentSpool,
  rig_opt: Option<&MachineRig>,
) -> String {
  let cat = crate::catalog::load_merged_catalog();
  let is_oc = spool.name.starts_with("OC -") || spool.name.starts_with("OC:");
  if spool.id.starts_with("disk_")
    && !is_oc
    && !spool.inherits.trim().is_empty()
    && cat.instantiated_system_filaments.contains(&spool.inherits)
    && !spool.inherits.contains("TPU for ACE @")
    && !spool.inherits.contains("PCTG @")
    && !spool.inherits.contains("PC-CF @")
    && !spool.inherits.contains("PC-GF @")
  {
    return spool.inherits.clone();
  }
  let (model, noz) = if let Some(r) = rig_opt {
    (r.printer_model.as_str(), r.formatted_nozzle_diameter())
  } else {
    let mc = &crate::constants::constants().machines[0];
    (mc.printer_model.as_str(), mc.default_nozzle_diameter.clone())
  };
  cat.resolve_filament_inherits(model, &noz, &spool.material_type, &spool.finish_type)
}

pub fn resolve_kobra_s1_filament_inherits(spool: &FilamentSpool) -> String {
  resolve_filament_inherits_for_rig(spool, None)
}

/// Map internal material codes to OrcaSlicer / AnycubicSlicerNext valid `filament_type` enum strings
pub fn resolve_orca_filament_type(material_type: &str) -> String {
  crate::constants::constants()
    .lookup_polymer(material_type)
    .slicer_filament_type
    .clone()
}

/// Side-channel key set by the `anycubic_pctg_as_petg` rule (Anycubic rigs only by default).
pub const PCTG_AS_PETG_SIDE_CHANNEL: &str = "_anycubic_pctg_as_petg";

/// Marker appended to `filament_notes` when a PCTG-family spool is declared as `PETG`, so a later
/// directory import restores `material_type = PCTG` (and its 240–275°C brackets) from disk.
pub const PCTG_AS_PETG_NOTE_MARKER: &str = "[PCTG declared as PETG for AnycubicSlicerNext]";

/// AnycubicSlicerNext's hard-coded `filament_type` enum lacks `PCTG`: maps a PCTG-family slicer
/// type (`PCTG`, `PCTG-CF`, ...) to its PETG equivalent. Any other type is returned unchanged.
pub fn declare_pctg_family_as_petg(slicer_filament_type: &str) -> String {
  let trimmed = slicer_filament_type.trim();
  match trimmed.to_uppercase().strip_prefix("PCTG") {
    Some(rest) => format!("PETG{}", rest),
    None => trimmed.to_string(),
  }
}

/// Side-channel key set by the `anycubic_pctg_as_petg` rule (Anycubic rigs only by default)
/// so PC-CF / PC-GF spools record a round-trip marker when declared as `PC`.
pub const PCCF_AS_PC_SIDE_CHANNEL: &str = "_anycubic_pccf_as_pc";

/// Marker appended to `filament_notes` when a PC-CF / PC-GF spool is declared as `PC`, so a later
/// directory import restores `material_type = PC-CF` (and its 255–295°C / hardened-nozzle limits)
/// even if the preset name was customized.
pub const PCCF_AS_PC_NOTE_MARKER: &str = "[PC-CF declared as PC for AnycubicSlicerNext]";

/// Returns true if the material code is a reinforced PC variant (`PC-CF`, `PC-GF`, `PC-RCF`, `PC-RGF`).
pub fn is_reinforced_pc_material(material_type: &str) -> bool {
  let up = material_type.trim().to_uppercase();
  matches!(up.as_str(), "PC-CF" | "PC-GF" | "PC-RCF" | "PC-RGF")
    || up.starts_with("PC-CF")
    || up.starts_with("PC-GF")
    || up.starts_with("PC-RCF")
    || up.starts_with("PC-RGF")
}

/// Anycubic ACE Pro 2 / AnycubicSlicerNext (Kobra S1) lacks `PC-CF` in its filament list:
/// maps a reinforced PC-family material (`PC-CF`, `PC-GF`, `PC-RCF`, `PC-RGF`) to `PC`.
pub fn declare_pccf_family_as_pc(slicer_filament_type: &str) -> String {
  let trimmed = slicer_filament_type.trim();
  if is_reinforced_pc_material(trimmed) {
    "PC".to_string()
  } else {
    trimmed.to_string()
  }
}

pub const FILAMENT_SETTING_KEYS: &[&str] = &[
  "close_fan_the_first_x_layers",
  "cool_plate_temp",
  "cool_plate_temp_initial_layer",
  "default_filament_colour",
  "eng_plate_temp",
  "eng_plate_temp_initial_layer",
  "fan_max_speed",
  "fan_max_speed_BRASS",
  "fan_max_speed_HS",
  "fan_min_speed",
  "fan_min_speed_BRASS",
  "fan_min_speed_HS",
  "filament_deretraction_speed",
  "filament_diameter",
  "filament_flow_ratio",
  "filament_max_volumetric_speed",
  "filament_notes",
  "filament_retract_before_wipe",
  "filament_retraction_length",
  "filament_retraction_speed",
  "filament_type",
  "filament_wipe",
  "filament_wipe_distance",
  "filament_z_hop",
  "filament_z_hop_types",
  "hot_plate_temp",
  "hot_plate_temp_initial_layer",
  "nozzle_temperature",
  "nozzle_temperature_BRASS",
  "nozzle_temperature_HS",
  "nozzle_temperature_initial_layer",
  "nozzle_temperature_initial_layer_BRASS",
  "nozzle_temperature_initial_layer_HS",
  "nozzle_temperature_range_high",
  "nozzle_temperature_range_low",
  "pressure_advance",
  "required_nozzle_HRC",
  "supertack_plate_temp",
  "supertack_plate_temp_initial_layer",
  "textured_cool_plate_temp",
  "textured_cool_plate_temp_initial_layer",
  "textured_plate_temp",
  "textured_plate_temp_initial_layer",
];

/// Returns true when `rig` is an Anycubic printer (whose AnycubicSlicerNext fork supports
/// `*_BRASS` and `*_HS` sibling filament keys). Non-Anycubic upstream OrcaSlicer presets
/// (`Bambu Lab`, `Creality`, `Voron`, `Prusa`, `Sovol`) must NOT emit `*_BRASS` / `*_HS` keys.
pub fn is_anycubic_printer_rig(rig: &MachineRig) -> bool {
  rig.printer_model.to_uppercase().contains("ANYCUBIC")
}

/// Returns true if `key` is an AnycubicSlicerNext fork-only filament key (`*_BRASS` / `*_HS`).
pub fn is_anycubic_fork_filament_key(key: &str) -> bool {
  key.ends_with("_BRASS") || key.ends_with("_HS")
}

/// Returns the canonical filament preset keys to export for `rig`.
pub fn filament_setting_keys_for_rig(rig: &MachineRig) -> Vec<&'static str> {
  let is_ac = is_anycubic_printer_rig(rig);
  let mut keys: Vec<&'static str> = FILAMENT_SETTING_KEYS
    .iter()
    .copied()
    .filter(|k| is_ac || !is_anycubic_fork_filament_key(k))
    .collect();
  keys.push("enable_pressure_advance");
  if is_ac {
    keys.push("nozzle_temperature_range_high_HS");
    keys.push("nozzle_temperature_range_low_HS");
  }
  keys.sort_unstable();
  keys.dedup();
  keys
}

/// Optional OrcaSlicer Filament-scoped keys that are seeded with base values before running the
/// Lua stack and exported into both `filament/*.json` and `project_settings.config` whenever a
/// Lua rule touches them.
pub const OPTIONAL_FILAMENT_SETTING_KEYS: &[&str] = &[
  "activate_air_filtration",
  "adaptive_pressure_advance",
  "additional_cooling_fan_speed",
  "chamber_temperature",
  "complete_print_exhaust_fan_speed",
  "dont_slow_down_outer_wall",
  "during_print_exhaust_fan_speed",
  "enable_overhang_bridge_fan",
  "enable_pressure_advance",
  "fan_cooling_layer_time",
  "filament_retract_lift_above",
  "filament_retract_lift_below",
  "filament_retract_lift_enforce",
  "filament_retract_when_changing_layer",
  "filament_retraction_minimum_travel",
  "nozzle_temperature_range_high_HS",
  "nozzle_temperature_range_low_HS",
  "overhang_fan_speed",
  "overhang_fan_threshold",
  "reduce_fan_stop_start_freq",
  "slow_down_for_layer_cooling",
  "slow_down_layer_time",
  "slow_down_min_speed",
  "temperature_vitrification",
];

/// Canonical Machine-scoped keys in OrcaSlicer's schema (`machine/*.json` and
/// `different_settings_to_system[N+1]`).
pub const MACHINE_SCHEMA_KEYS: &[&str] = &[
  "before_layer_change_gcode",
  "change_filament_gcode",
  "layer_change_gcode",
  "machine_end_gcode",
  "machine_pause_gcode",
  "machine_start_gcode",
  "nozzle_diameter",
  "nozzle_hrc",
  "nozzle_type",
  "printable_area",
  "printable_height",
  "printer_notes",
  "retract_restart_extra",
  "retraction_distances_when_cut",
  "time_lapse_gcode",
];

/// Canonical Shadow-Aware Routing for Dual-Scope Keys:
/// In OrcaSlicer / AnycubicSlicerNext ($\text{Machine} \prec \text{Filament} \prec \text{Process}$),
/// dual-scope extruder/retraction keys use "non-nil wins" precedence where the Filament preset's
/// `filament_*` key completely shadows the Machine/Process key unless `filament_* == "nil"`.
/// All Lua rules must target the canonical `filament_*` key (or `textured_plate_temp*` for
/// Textured PEI bed keys). Returns `Some(canonical_key)` when `key` is a shadowed dual-scope alias.
pub fn canonicalize_dual_scope_key(key: &str) -> Option<&'static str> {
  match key {
    "z_hop" => Some("filament_z_hop"),
    "z_hop_types" => Some("filament_z_hop_types"),
    "retract_lift_enforce" => Some("filament_retract_lift_enforce"),
    "retract_lift_above" => Some("filament_retract_lift_above"),
    "retract_lift_below" => Some("filament_retract_lift_below"),
    "retraction_length" => Some("filament_retraction_length"),
    "retraction_speed" => Some("filament_retraction_speed"),
    "deretraction_speed" => Some("filament_deretraction_speed"),
    "retract_before_wipe" => Some("filament_retract_before_wipe"),
    "wipe" => Some("filament_wipe"),
    "wipe_distance" => Some("filament_wipe_distance"),
    "retraction_minimum_travel" => Some("filament_retraction_minimum_travel"),
    "retract_when_changing_layer" => Some("filament_retract_when_changing_layer"),
    "max_volumetric_speed" => Some("filament_max_volumetric_speed"),
    "textured_pei_plate_temp" => Some("textured_plate_temp"),
    "textured_pei_plate_temp_initial_layer" => Some("textured_plate_temp_initial_layer"),
    _ => None,
  }
}

/// Exhaustive Key Routing Registry destination for every valid parameter in the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyDestination {
  /// Exported to standalone `machine/*.json` and the Machine slot of `project_settings.config`
  Machine,
  /// Exported to standalone `process/*.json` and `different_settings_to_system[0]` of `project_settings.config`
  Process,
  /// Exported to standalone `filament/*.json` and per-slot vectors (`different_settings_to_system[1..=N]`) of `project_settings.config`
  Filament,
  /// Exported to project-level multi-material purge settings in `project_settings.config`
  ProjectFlush,
  /// Internal engine / cross-rule metadata (`_`-prefixed keys and `computed_safe_speed_ceiling`)
  InternalMeta,
}

/// Classifies any parameter key into its exact exported artifact destination (`KeyDestination`).
/// Returns `None` if `key` is an unrouted phantom key or an un-canonicalized shadowed dual-scope key.
pub fn classify_key_destination(key: &str) -> Option<KeyDestination> {
  if key.starts_with('_') || key == "computed_safe_speed_ceiling" {
    return Some(KeyDestination::InternalMeta);
  }
  if matches!(
    key,
    "flush_multiplier" | "flush_volumes_matrix" | "flush_volumes_vector"
  ) {
    return Some(KeyDestination::ProjectFlush);
  }
  // Reject non-canonical shadowed dual-scope keys so rules cannot ambiguously write keys that
  // would be shadowed by `filament_*` at slice time.
  if canonicalize_dual_scope_key(key).is_some() {
    return None;
  }
  if FILAMENT_SETTING_KEYS.contains(&key) || OPTIONAL_FILAMENT_SETTING_KEYS.contains(&key) {
    return Some(KeyDestination::Filament);
  }
  if MACHINE_SCHEMA_KEYS.contains(&key) {
    return Some(KeyDestination::Machine);
  }
  if EXTRA_PROCESS_OVERRIDE_KEYS.contains(&key)
    || matches!(
      key,
      "layer_height"
        | "line_width"
        | "outer_wall_line_width"
        | "outer_wall_speed"
        | "inner_wall_speed"
        | "sparse_infill_speed"
        | "internal_solid_infill_speed"
        | "top_surface_speed"
        | "gap_infill_speed"
        | "support_speed"
        | "initial_layer_speed"
        | "initial_layer_infill_speed"
        | "default_acceleration"
        | "print_sequence"
    )
  {
    return Some(KeyDestination::Process);
  }
  let cat = crate::catalog::load_merged_catalog();
  if cat.default_process_base.contains_key(key)
    && !is_filament_scoped_key(key)
    && !is_machine_scoped_key(key)
    && !matches!(
      key,
      "type"
        | "from"
        | "is_custom_defined"
        | "inherits"
        | "compatible_printers"
        | "name"
        | "print_settings_id"
        | "notes"
        | "version"
        | "filename_format"
    )
  {
    return Some(KeyDestination::Process);
  }
  None
}

/// Returns `true` if `key` belongs to OrcaSlicer's **Filament** scope (per-extruder / per-slot
/// vector in `project_settings.config` and standalone `filament/*.json` presets).
/// Filament-scoped keys must NEVER be emitted into Process (`process/*.json`,
/// `different_settings_to_system[0]`) or Machine (`machine/*.json`,
/// `different_settings_to_system[N+1]`) presets.
pub fn is_filament_scoped_key(key: &str) -> bool {
  if FILAMENT_SETTING_KEYS.contains(&key) || OPTIONAL_FILAMENT_SETTING_KEYS.contains(&key) {
    return true;
  }
  key.starts_with("filament_")
    || key.starts_with("nozzle_temperature")
    || key.ends_with("_plate_temp")
    || key.ends_with("_plate_temp_initial_layer")
    || key.starts_with("fan_min_speed")
    || key.starts_with("fan_max_speed")
    || matches!(
      key,
      "overhang_fan_speed"
        | "overhang_fan_threshold"
        | "enable_overhang_bridge_fan"
        | "close_fan_the_first_x_layers"
        | "slow_down_layer_time"
        | "slow_down_min_speed"
        | "slow_down_for_layer_cooling"
        | "dont_slow_down_outer_wall"
        | "additional_cooling_fan_speed"
        | "during_print_exhaust_fan_speed"
        | "complete_print_exhaust_fan_speed"
        | "chamber_temperature"
        | "reduce_fan_stop_start_freq"
        | "fan_cooling_layer_time"
        | "pressure_advance"
        | "enable_pressure_advance"
        | "adaptive_pressure_advance"
        | "temperature_vitrification"
        | "required_nozzle_HRC"
        | "default_filament_colour"
    )
}

/// Returns `true` if `key` belongs to OrcaSlicer's **Machine** scope (`machine/*.json` and
/// `different_settings_to_system[N+1]`). Machine-scoped keys must NEVER be emitted into Process
/// (`process/*.json`, `different_settings_to_system[0]`) or Filament (`filament/*.json`) presets.
pub fn is_machine_scoped_key(key: &str) -> bool {
  matches!(
    key,
    "nozzle_type"
      | "nozzle_hrc"
      | "nozzle_diameter"
      | "retraction_length"
      | "retraction_speed"
      | "retraction_distances_when_cut"
      | "deretraction_speed"
      | "retract_before_wipe"
      | "retract_restart_extra"
      | "retraction_minimum_travel"
      | "z_hop"
      | "z_hop_types"
      | "wipe"
      | "wipe_distance"
      | "printer_notes"
      | "printable_area"
      | "printable_height"
      | "before_layer_change_gcode"
      | "change_filament_gcode"
      | "machine_end_gcode"
      | "machine_start_gcode"
      | "layer_change_gcode"
      | "time_lapse_gcode"
      | "machine_pause_gcode"
  ) || key.starts_with("machine_")
    || key.starts_with("extruder_")
}

fn first_str_from_map(
  map: &serde_json::Map<String, serde_json::Value>,
  key: &str,
) -> Option<String> {
  match map.get(key)? {
    serde_json::Value::String(s) => Some(s.clone()),
    serde_json::Value::Array(arr) => arr.first()?.as_str().map(|s| s.to_string()),
    serde_json::Value::Number(n) => Some(n.to_string()),
    _ => None,
  }
}

/// Resolves a filament spool's OrcaSlicer JSON parameter map by importing the resolved OrcaSlicer
/// base filament profile (`default_filament_bases` from `@orcaslicer_repo`), merging any imported
/// `spool.raw_disk_json` values, and applying the spool's active parameters (plus any active
/// weather/climate and anti-stringing offsets when `rig` is provided).
pub fn resolve_spool_orca_filament_map(
  spool: &FilamentSpool,
) -> serde_json::Map<String, serde_json::Value> {
  resolve_spool_orca_filament_map_with_rig_and_overrides(spool, None, None)
}

pub fn resolve_spool_orca_filament_map_with_rig(
  spool: &FilamentSpool,
  rig_opt: Option<&MachineRig>,
) -> serde_json::Map<String, serde_json::Value> {
  resolve_spool_orca_filament_map_with_rig_and_overrides(spool, rig_opt, None)
}

pub fn resolve_spool_orca_filament_map_with_rig_and_overrides(
  spool: &FilamentSpool,
  rig_opt: Option<&MachineRig>,
  derived_overrides: Option<&serde_json::Map<String, serde_json::Value>>,
) -> serde_json::Map<String, serde_json::Value> {
  resolve_spool_orca_filament_map_and_provenance(spool, rig_opt, derived_overrides).0
}

/// Evaluates the per-spool Filament transformation stack and returns the full typed
/// [`StackEvaluationResult`](crate::transformations::StackEvaluationResult) (including
/// `provenance_items` ledger chains $x_0 \to \dots \to x_{\text{shipped}}$) from the exact same
/// single `evaluate_stack` pass that produces standalone `.json` and `.3mf` filament maps.
pub fn evaluate_spool_filament_provenance(
  spool: &FilamentSpool,
  rig: &MachineRig,
  derived_overrides: Option<&serde_json::Map<String, serde_json::Value>>,
) -> crate::transformations::StackEvaluationResult {
  resolve_spool_orca_filament_map_and_provenance(spool, Some(rig), derived_overrides).1
}

fn resolve_spool_orca_filament_map_and_provenance(
  spool: &FilamentSpool,
  rig_opt: Option<&MachineRig>,
  derived_overrides: Option<&serde_json::Map<String, serde_json::Value>>,
) -> (
  serde_json::Map<String, serde_json::Value>,
  crate::transformations::StackEvaluationResult,
) {
  let c = crate::constants::constants();
  let cat = crate::catalog::load_merged_catalog();
  let orca_fil_type = resolve_orca_filament_type(&spool.material_type);
  let poly_cfg = c.lookup_polymer(&spool.material_type);
  let is_foaming_tpu = spool.is_foaming_tpu();
  let mut base_map = serde_json::Map::new();
  let base_lookup_key = match orca_fil_type.as_str() {
    "PA6-CF" | "PA-CF" | "PPA-CF" => "PA-CF",
    "PETG-CF" | "PET-CF" | "PCTG" | "PCTG-CF" => "PETG",
    "PEBA" | "TPU for AMS" | "TPU 95A" => "TPU",
    "ASA-CF" | "HIPS" | "PP" => "ASA",
    "ABS-CF" => "ABS",
    "PC-CF" | "PEEK" | "PPS" | "PPS-CF" => "PC",
    "PLA+" | "PHA" | "PCL" | "BVOH" => "PLA",
    other => other,
  };
  if let Some(fil_base) = cat
    .default_filament_bases
    .get(&orca_fil_type)
    .or_else(|| cat.default_filament_bases.get(base_lookup_key))
    .or_else(|| cat.default_filament_bases.get(&poly_cfg.family))
    .or_else(|| cat.default_filament_bases.get("PLA"))
  {
    for (k, v) in fil_base {
      base_map.insert(k.clone(), v.clone());
    }
  }
  let is_external_disk_spool = spool.id.starts_with("disk_")
    && !spool.name.starts_with("OC -")
    && !spool.name.starts_with("OC:");
  if !is_external_disk_spool {
    let norm_brand = FilamentSpool::normalized_brand(&spool.brand);
    let vkey = format!("{}::{}", norm_brand, spool.material_type);
    if let Some(vb) = cat.vendor_filament_bases.get(&vkey) {
      for (k, v) in vb {
        if k == "compatible_printers" || k == "filament_vendor" {
          continue;
        }
        if v.is_string()
          || (v.is_array()
            && v
              .as_array()
              .map(|a| !a.is_empty() && a.iter().all(|x| x.is_string()))
              .unwrap_or(false))
        {
          base_map.insert(k.clone(), v.clone());
        }
      }
    }
  } else if let Some(ref raw_txt) = spool.raw_disk_json {
    if let Ok(serde_json::Value::Object(disk_obj)) =
      serde_json::from_str::<serde_json::Value>(raw_txt)
    {
      for (k, v) in disk_obj {
        if k == "compatible_printers" || k == "filament_vendor" {
          continue;
        }
        if v.is_string()
          || (v.is_array()
            && v
              .as_array()
              .map(|a| !a.is_empty() && a.iter().all(|x| x.is_string()))
              .unwrap_or(false))
        {
          base_map.insert(k, v);
        }
      }
    }
  }

  // Evaluate per-spool weather/climate adjustments in Lua (`sym_weather_climate.lua`)
  // so 100% of weather/climate math lives in Lua (zero climate math in Rust).
  let weather_disabled = derived_overrides
    .and_then(|m| m.get("_weather_rule_disabled"))
    .and_then(|v| v.as_str())
    == Some("1");
  let (
    climate_noz_delta,
    climate_bed_delta,
    climate_init_bed_extra,
    climate_retract_delta,
    lua_climate_min_bed,
    lua_climate_zhop_enforce,
    lua_climate_exhaust_target,
    lua_climate_aux_fan_target,
  ) = if let Some(rig) = rig_opt.filter(|_| !weather_disabled) {
    let mut params = std::collections::HashMap::new();
    params.insert("climate_preset".to_string(), rig.climate_preset.clone());
    params.insert("ambient_temp_c".to_string(), rig.ambient_temp_c.to_string());
    params.insert("humidity_pct".to_string(), rig.humidity_pct.to_string());
    let weather_lua = crate::lua_engine::embed_param_defaults_in_lua(
      include_str!("../transformations/parametric/sym_weather_climate.lua"),
      &params,
    );
    let weather_ctx = serde_json::json!({
      "filament_type": spool.material_type,
      "finish_type": spool.finish_type,
      "nozzle_type": rig.nozzle_type,
      "nozzle_material": rig.nozzle_material,
      "hotend": rig.hotend,
      "addons": rig.addons,
      "climate_preset": rig.climate_preset,
      "ambient_temp_c": rig.ambient_temp_c,
      "humidity_pct": rig.humidity_pct,
    });
    let weather_in = serde_json::json!({ "name": spool.name });
    if let Ok((serde_json::Value::Object(wmap), _, _)) =
      crate::lua_engine::execute_transformation_lua(&weather_lua, &weather_in, &weather_ctx)
    {
      let gs = |k: &str| wmap.get(k).and_then(|v| v.as_str()).map(|s| s.to_string());
      (
        gs("_climate_noz_delta_c")
          .and_then(|s| s.parse::<i32>().ok())
          .unwrap_or(0),
        gs("_climate_bed_delta_c")
          .and_then(|s| s.parse::<i32>().ok())
          .unwrap_or(0),
        gs("_climate_init_bed_extra_c")
          .and_then(|s| s.parse::<i32>().ok())
          .unwrap_or(0),
        gs("_climate_retract_delta_mm")
          .and_then(|s| s.parse::<f64>().ok())
          .unwrap_or(0.0),
        gs("_climate_min_bed_c").and_then(|s| s.parse::<i32>().ok()),
        gs("_climate_zhop_enforce"),
        gs("_climate_exhaust_target").and_then(|s| s.parse::<i32>().ok()),
        gs("_climate_aux_fan_target").and_then(|s| s.parse::<i32>().ok()),
      )
    } else {
      (0, 0, 0, 0.0, None, None, None, None)
    }
  } else {
    (0, 0, 0, 0.0, None, None, None, None)
  };

  let extra_filament_rules: Vec<crate::transformations::TransformationRule> = derived_overrides
    .and_then(|m| m.get("_active_filament_rules_json"))
    .and_then(|v| v.as_str())
    .and_then(|s| serde_json::from_str(s).ok())
    .unwrap_or_default();
  let has_extra_rules = !extra_filament_rules.is_empty();
  let run_nozzle_curves_early =
    rig_opt.is_some() && !spool.is_siraya_foaming_air() && !is_foaming_tpu;
  let has_wet_lua_rule = run_nozzle_curves_early
    && extra_filament_rules.iter().any(|r| {
      r.enabled
        && (r.id.contains("wet_filament") || r.lua_code.contains("_wet_filament_rh"))
    });
  let has_structural_lua_rule = run_nozzle_curves_early
    && extra_filament_rules.iter().any(|r| {
      r.enabled
        && (r.id.contains("structural_strength")
          || r.lua_code.contains("_structural_temp_boost_c"))
    });
  let is_elastomer_early = c.is_elastomer_family_material(&spool.material_type);

  let wet_rh = derived_overrides
    .and_then(|m| m.get("_wet_filament_rh"))
    .and_then(|v| v.as_str())
    .and_then(|s| s.parse::<i32>().ok())
    .unwrap_or(0);
  let wet_active = wet_rh > 20 && !is_foaming_tpu && !has_wet_lua_rule;
  let wet_temp_drop = if wet_active {
    derived_overrides
      .and_then(|m| m.get("_wet_filament_temp_drop_c"))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<i32>().ok())
      .unwrap_or(0)
  } else {
    0
  };
  let wet_init_bed_boost = if wet_active {
    derived_overrides
      .and_then(|m| m.get("_wet_filament_init_bed_boost_c"))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<i32>().ok())
      .unwrap_or(0)
      .clamp(0, 4)
  } else {
    0
  };
  let wet_flow_trim = if wet_active && !is_elastomer_early {
    derived_overrides
      .and_then(|m| m.get("_wet_filament_flow_trim"))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<f64>().ok())
      .unwrap_or(0.0)
      .clamp(-0.02, 0.02)
  } else {
    0.0
  };
  let wet_retract_add = if wet_active && !is_elastomer_early {
    derived_overrides
      .and_then(|m| m.get("_wet_filament_retract_add_mm"))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<f64>().ok())
      .unwrap_or(0.0)
      .clamp(0.0, 0.15)
  } else {
    0.0
  };
  let wet_retract_before_wipe = if wet_active {
    derived_overrides
      .and_then(|m| m.get("_wet_filament_retract_before_wipe_pct"))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<i32>().ok())
  } else {
    None
  };

  let base_noz = first_str_from_map(&base_map, "nozzle_temperature")
    .and_then(|s| s.parse::<i32>().ok())
    .unwrap_or(spool.print_temp);
  let base_init = first_str_from_map(&base_map, "nozzle_temperature_initial_layer")
    .and_then(|s| s.parse::<i32>().ok())
    .unwrap_or(base_noz);
  let init_delta = if poly_cfg.initial_layer_temp_boost_c(is_foaming_tpu) == 0 {
    0
  } else if is_external_disk_spool {
    (base_init - base_noz).clamp(0, 10)
  } else {
    poly_cfg.initial_layer_temp_boost_c(is_foaming_tpu)
  };
  let mut min_safe_noz;
  let mut max_safe_noz;
  {
    let (mn, mx) = FilamentSpool::polymer_safe_nozzle_bounds(&spool.material_type);
    min_safe_noz = mn;
    max_safe_noz = mx;
  }
  if is_foaming_tpu {
    max_safe_noz = max_safe_noz.max(280);
  }
  let mut machine_max_noz = c.hardware_limits.max_hotend_temp_c;
  let mut machine_max_bed = c.hardware_limits.max_bed_temp_c;
  // The active machine's hotend and bed ratings cap the polymer bands from above (curated
  // `machines[]` override, else the global limit via `catalog::machine_constants`): a high-temp
  // spool must never export above a 260 °C PTFE hotend or 100 °C bed heater.
  if let Some(rig) = rig_opt {
    let mc = crate::catalog::machine_constants(&rig.printer_model);
    machine_max_noz = mc.max_hotend_temp_c.min(c.hardware_limits.max_hotend_temp_c);
    machine_max_bed = mc.max_bed_temp_c.min(c.hardware_limits.max_bed_temp_c);
    max_safe_noz = max_safe_noz.min(machine_max_noz);
    if min_safe_noz > max_safe_noz {
      min_safe_noz = max_safe_noz;
    }
  }
  let (raw_min_bed, raw_max_bed) = FilamentSpool::polymer_safe_bed_bounds(&spool.material_type);
  let max_safe_bed = raw_max_bed.min(machine_max_bed);
  let min_safe_bed = raw_min_bed.min(max_safe_bed);
  let climate_min_bed = lua_climate_min_bed
    .map(|mb| min_safe_bed.max(mb))
    .unwrap_or(min_safe_bed)
    .min(max_safe_bed);
  let mat_up_for_warp_early = spool.material_type.to_uppercase();
  let _is_warp_prone_early = poly_cfg.is_warp_prone()
    || orca_fil_type == "PET-CF"
    || orca_fil_type == "PET-GF"
    || mat_up_for_warp_early.contains("HIPS")
    || mat_up_for_warp_early.contains("PET-CF")
    || mat_up_for_warp_early.contains("PET-GF");
  let pre_wet_temp = (spool.print_temp + climate_noz_delta).clamp(min_safe_noz, max_safe_noz);
  // Structural Strength pack side-channel: per-spool nozzle boost (clamped to THIS polymer's safe max)
  // and max-fan reduction. Skipped for active-foaming TPU / elastomers, and skipped here when
  // `extra_filament_rules` already executes `structural_strength.lua` in `evaluate_stack`.
  let structural_boost = if is_foaming_tpu || is_elastomer_early || has_structural_lua_rule {
    0
  } else {
    derived_overrides
      .and_then(|m| m.get("_structural_temp_boost_c"))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<i32>().ok())
      .unwrap_or(0)
      .clamp(0, 10)
  };
  let structural_fan_cut_pct = if is_elastomer_early || has_structural_lua_rule {
    0
  } else {
    derived_overrides
      .and_then(|m| m.get("_structural_fan_cut_pct"))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<i32>().ok())
      .unwrap_or(0)
      .clamp(0, 60)
  };
  let pre_wet_temp = (pre_wet_temp + structural_boost).clamp(min_safe_noz, max_safe_noz);
  let effective_temp = (pre_wet_temp + wet_temp_drop).clamp(min_safe_noz, max_safe_noz);
  let initial_temp = (pre_wet_temp + init_delta).clamp(min_safe_noz, max_safe_noz);
  let lua_bed_delta = if has_extra_rules && rig_opt.is_some() {
    None
  } else {
    derived_overrides
      .and_then(|m| m.get("_lua_bed_delta_c"))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<f64>().ok())
      .map(|v| v.round() as i32)
  };
  let lua_init_bed_delta = if has_extra_rules && rig_opt.is_some() {
    None
  } else {
    derived_overrides
      .and_then(|m| m.get("_lua_init_bed_delta_c"))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<f64>().ok())
      .map(|v| v.round() as i32)
  };
  let explicit_lua_bed_override = if !has_extra_rules
    && lua_bed_delta.is_none()
    && lua_init_bed_delta.is_none()
    && derived_overrides.and_then(|m| m.get("_climate_bed_delta_c")).is_none()
    && !wet_active
  {
    derived_overrides.and_then(|ov| {
      let touched = ov.get("_lua_touched_keys")?.as_str()?;
      let has_plate = touched
        .split(';')
        .any(|k| matches!(k, "textured_plate_temp" | "hot_plate_temp"));
      if has_plate {
        first_str_from_map(ov, "textured_plate_temp")
          .or_else(|| first_str_from_map(ov, "hot_plate_temp"))
          .and_then(|s| s.parse::<f64>().ok())
          .map(|v| v.round() as i32)
      } else {
        None
      }
    })
  } else {
    None
  };
  let base_climate_bed = spool.bed_temp + climate_bed_delta;
  let effective_bed = if let Some(abs_b) = explicit_lua_bed_override {
    abs_b.clamp(climate_min_bed, max_safe_bed)
  } else {
    (base_climate_bed + lua_bed_delta.unwrap_or(0)).clamp(climate_min_bed, max_safe_bed)
  };
  let mut initial_bed = if let Some(abs_b) = explicit_lua_bed_override {
    let abs_bi = derived_overrides
      .and_then(|ov| {
        first_str_from_map(ov, "textured_plate_temp_initial_layer")
          .or_else(|| first_str_from_map(ov, "hot_plate_temp_initial_layer"))
      })
      .and_then(|s| s.parse::<f64>().ok())
      .map(|v| v.round() as i32)
      .unwrap_or(abs_b);
    abs_bi.clamp(climate_min_bed, max_safe_bed)
  } else {
    (base_climate_bed
      + lua_init_bed_delta.unwrap_or(0)
      + climate_init_bed_extra
      + wet_init_bed_boost)
      .clamp(climate_min_bed, max_safe_bed)
  };
  initial_bed = initial_bed.max(effective_bed);
  let max_bed_init_step = if is_elastomer_early { 3 } else { 5 };
  if initial_bed - effective_bed > max_bed_init_step {
    initial_bed = (effective_bed + max_bed_init_step).min(max_safe_bed);
  }
  let effective_flow_ratio = if wet_flow_trim.abs() > 1e-6 {
    (spool.flow_ratio as f64 + wet_flow_trim).clamp(0.91, 1.05) as f32
  } else {
    spool.flow_ratio
  };
  let export_name = match rig_opt {
    Some(r) => spool.format_concise_title_for_rig(r),
    None => spool.format_concise_title(),
  };
  let inherits = resolve_filament_inherits_for_rig(spool, rig_opt);

  let kobra_unbottleneck_petg_abs = derived_overrides
    .and_then(|m| m.get("_kobra_unbottleneck_petg_abs"))
    .and_then(|v| v.as_str())
    == Some("1");
  // `anycubic_pctg_as_petg`: only the *declared* `filament_type` string changes. `orca_fil_type`
  // (base preset lookup, PETG-only Kobra tweaks) and `spool.material_type` (safe temperature
  // bounds, `nozzle_temperature_range_low/high`) intentionally stay on PCTG / PC-CF.
  let declare_pctg_as_petg = derived_overrides
    .and_then(|m| m.get(PCTG_AS_PETG_SIDE_CHANNEL))
    .and_then(|v| v.as_str())
    == Some("1");
  let declare_pccf_as_pc = derived_overrides
    .and_then(|m| m.get(PCCF_AS_PC_SIDE_CHANNEL))
    .and_then(|v| v.as_str())
    == Some("1");
  let mut declared_fil_type = if declare_pctg_as_petg {
    declare_pctg_family_as_petg(&orca_fil_type)
  } else {
    orca_fil_type.clone()
  };
  let pctg_declared_as_petg = declared_fil_type != orca_fil_type;
  let pccf_declared_as_pc = declare_pccf_as_pc && is_reinforced_pc_material(&spool.material_type);
  if pccf_declared_as_pc {
    declared_fil_type = declare_pccf_family_as_pc(&declared_fil_type);
  }
  let is_translucent_finish = spool.finish_type.eq_ignore_ascii_case("Translucent")
    || spool.material_type.to_lowercase().contains("translucent");

  // Ensure retraction, wipe, z-hop, and fan_min_speed / fan_max_speed are always populated cleanly
  let rig_base_ret = rig_opt.map(|r| r.base_retraction_length).unwrap_or(0.8);
  let is_bowden_rig = rig_base_ret >= 2.0;
  let default_ret = if is_bowden_rig {
    if is_elastomer_early {
      (rig_base_ret * 0.65).clamp(2.0, 2.8)
    } else {
      rig_base_ret
    }
  } else {
    poly_cfg.default_retraction_mm as f64
  };
  let mut base_ret_len = if is_external_disk_spool {
    first_str_from_map(&base_map, "filament_retraction_length")
      .and_then(|s| s.parse::<f64>().ok())
      .unwrap_or(default_ret)
      .max(default_ret)
  } else if spool.is_siraya_foaming_air() {
    0.0
  } else if is_foaming_tpu {
    if is_bowden_rig {
      (rig_base_ret * 0.65).clamp(2.0, 2.8)
    } else {
      0.35
    }
  } else {
    default_ret
  };
  if kobra_unbottleneck_petg_abs
    && !is_bowden_rig
    && matches!(orca_fil_type.as_str(), "PETG" | "ABS" | "ASA")
    && base_ret_len > 0.70
  {
    base_ret_len = 0.70;
  }
  let max_ret_clamp = if is_bowden_rig {
    if is_elastomer_early {
      (rig_base_ret * 0.75).clamp(2.2, 3.0)
    } else {
      (rig_base_ret + 0.5).max(1.45)
    }
  } else if is_elastomer_early {
    1.00
  } else {
    1.25
  };
  let effective_ret_len = if spool.is_siraya_foaming_air() {
    0.0
  } else {
    (base_ret_len + climate_retract_delta + wet_retract_add).clamp(0.30, max_ret_clamp)
  };
  let ret_len_str = crate::constants::PolymerConstants::format_retraction_mm(effective_ret_len);
  let mut ret_spd_str = if is_external_disk_spool {
    first_str_from_map(&base_map, "filament_retraction_speed")
      .unwrap_or_else(|| poly_cfg.default_retraction_speed_mms.to_string())
  } else if is_foaming_tpu {
    "25".to_string()
  } else {
    poly_cfg.default_retraction_speed_mms.to_string()
  };
  // App-wide elastomer policy (constants.json `hardware_limits.elastomer_*`, mirrored by the
  // random-config audit): flexible families never retract faster than the configured cap,
  // regardless of vendor disk defaults — faster pulls slip at the direct-drive gears and
  // cavitate the melt.
  if c.is_elastomer_family_material(&spool.material_type) {
    if let Ok(s) = ret_spd_str.trim().parse::<f64>() {
      let cap = if is_bowden_rig {
        (c.hardware_limits.elastomer_max_retraction_speed_mms as f64).min(25.0)
      } else {
        c.hardware_limits.elastomer_max_retraction_speed_mms as f64
      };
      if s > cap {
        ret_spd_str = format!("{}", cap as i32);
      }
    }
  }
  let (fan_min_pct, fan_max_pct) =
    poly_cfg.resolve_fan_min_max_pct(spool.fan_speed_percent);
  let parent_pa_enabled = is_external_disk_spool
    && first_str_from_map(&base_map, "enable_pressure_advance")
      .map(|s| s.trim() == "1")
      .unwrap_or(false);
  let enable_pa_for_rig = !is_bowden_rig || parent_pa_enabled;
  let effective_pa = if enable_pa_for_rig {
    spool.pressure_advance
  } else {
    first_str_from_map(&base_map, "pressure_advance")
      .and_then(|s| s.parse::<f32>().ok())
      .unwrap_or(0.0)
  };
  let mut effective_mvs = spool.max_volumetric_speed;
  if is_bowden_rig && is_elastomer_early && effective_mvs > 3.6 {
    effective_mvs = 3.6;
  }
  if kobra_unbottleneck_petg_abs && !is_translucent_finish {
    if orca_fil_type == "PETG" && effective_mvs < 19.0 {
      effective_mvs = 19.0;
    } else if (orca_fil_type == "ABS" || orca_fil_type == "ASA") && effective_mvs < 23.0 {
      effective_mvs = 23.0;
    }
  }
  let (z_hop_str, z_hop_type_str, mut retract_before_wipe_str) =
    poly_cfg.resolve_z_hop_and_retract_before_wipe(&spool.finish_type, is_foaming_tpu);
  if let Some(rbw_pct) = wet_retract_before_wipe {
    retract_before_wipe_str = format!("{}%", rbw_pct);
  }
  let z_hop_type_str = z_hop_type_str.to_string();

  // Ensure all canonical dual-scope `filament_*` keys and optional filament keys have their
  // base values populated in `base_map` before running the per-spool Lua stack.
  for (fil_key, mach_key, default_val) in [
    ("filament_retraction_length", "retraction_length", ret_len_str.as_str()),
    ("filament_retraction_speed", "retraction_speed", ret_spd_str.as_str()),
    ("filament_deretraction_speed", "deretraction_speed", "0"),
    (
      "filament_retract_before_wipe",
      "retract_before_wipe",
      retract_before_wipe_str.as_str(),
    ),
    ("filament_wipe", "wipe", "1"),
    ("filament_wipe_distance", "wipe_distance", "2"),
    ("filament_z_hop", "z_hop", z_hop_str.as_str()),
    ("filament_z_hop_types", "z_hop_types", z_hop_type_str.as_str()),
    (
      "filament_retraction_minimum_travel",
      "retraction_minimum_travel",
      "1.5",
    ),
    (
      "filament_retract_lift_enforce",
      "retract_lift_enforce",
      "All Surfaces",
    ),
    ("filament_retract_lift_above", "retract_lift_above", "0"),
    ("filament_retract_lift_below", "retract_lift_below", "0"),
    (
      "filament_retract_when_changing_layer",
      "retract_when_changing_layer",
      "1",
    ),
  ] {
    if !base_map.contains_key(fil_key) {
      let fallback = cat
        .default_machine_base
        .get(mach_key)
        .cloned()
        .unwrap_or_else(|| serde_json::json!([default_val]));
      base_map.insert(fil_key.to_string(), fallback);
    }
  }
  if let Some(existing_lift_enforce) = first_str_from_map(&base_map, "filament_retract_lift_enforce")
  {
    let normalized = match existing_lift_enforce.trim() {
      "Top Surfaces Only" | "top_only" | "top" => "Top Only",
      "Bottom Surfaces Only" | "bottom_only" | "bottom" => "Bottom Only",
      "Top and Bottom Surfaces" | "top_and_bottom" => "Top and Bottom",
      "All" | "all" | "all_surfaces" => "All Surfaces",
      other => other,
    };
    if normalized != existing_lift_enforce {
      base_map.insert(
        "filament_retract_lift_enforce".to_string(),
        serde_json::json!([normalized]),
      );
    }
  }
  let mat_up_for_warp = spool.material_type.to_uppercase();
  let is_warp_prone_spool = poly_cfg.is_warp_prone()
    || orca_fil_type == "PET-CF"
    || orca_fil_type == "PET-GF"
    || mat_up_for_warp.contains("HIPS")
    || mat_up_for_warp.contains("PET-CF")
    || mat_up_for_warp.contains("PET-GF");
  let max_ovh_fan_pct: i32 = if is_warp_prone_spool {
    let boost = if fan_max_pct == 0 { 10 } else { 15 };
    (fan_max_pct + boost).clamp(10, (poly_cfg.catalog_fan_bounds_pct[1] as i32 + 10).min(35))
  } else {
    100
  };
  let default_ovh_fan_str = max_ovh_fan_pct.to_string();
  if is_warp_prone_spool && !is_external_disk_spool {
    base_map.insert(
      "overhang_fan_speed".to_string(),
      serde_json::json!([default_ovh_fan_str.clone()]),
    );
  }
  let fin_up_for_fiber = spool.finish_type.to_uppercase();
  let is_fiber_pla_spool = mat_up_for_warp.contains("PLA-CF")
    || mat_up_for_warp.contains("PLA-GF")
    || mat_up_for_warp.contains("PLA CARBON")
    || (mat_up_for_warp.contains("PLA")
      && (fin_up_for_fiber.contains("CARBON") || fin_up_for_fiber.contains("GLASS")));
  let min_cf = if is_warp_prone_spool || poly_cfg.is_copolyester() {
    3
  } else if is_fiber_pla_spool {
    2
  } else {
    1
  };
  let default_close_fan_layers = match min_cf {
    3.. => "3",
    2 => "2",
    _ => "1",
  };
  {
    let cur_cf = first_str_from_map(&base_map, "close_fan_the_first_x_layers")
      .and_then(|s| s.trim().parse::<i32>().ok())
      .unwrap_or(0);
    if cur_cf < min_cf {
      base_map.insert(
        "close_fan_the_first_x_layers".to_string(),
        serde_json::json!([default_close_fan_layers]),
      );
    }
  }
  let is_open_bedslinger = rig_opt.map(|r| r.is_bedslinger()).unwrap_or(false);
  let rig_has_aux_fan = rig_opt
    .map(|r| r.kinematics.contains("Aux Cooling"))
    .unwrap_or(true);
  let is_anycubic_rig = rig_opt.map(is_anycubic_printer_rig).unwrap_or(true);
  let parent_had_aux_fan = base_map.contains_key("additional_cooling_fan_speed");
  let spool_base_aux_pct = first_str_from_map(&base_map, "additional_cooling_fan_speed")
    .and_then(|s| s.trim().trim_end_matches('%').parse::<f64>().ok())
    .map(|n| n.round() as i32)
    .unwrap_or(0);
  let spool_base_exhaust_pct = first_str_from_map(&base_map, "during_print_exhaust_fan_speed")
    .and_then(|s| s.trim().trim_end_matches('%').parse::<f64>().ok())
    .map(|n| n.round() as i32)
    .unwrap_or(0);
  let default_deretract_spd = if is_bowden_rig && is_elastomer_early {
    "20"
  } else {
    "0"
  };
  for (opt_key, opt_default) in [
    ("overhang_fan_speed", default_ovh_fan_str.as_str()),
    ("overhang_fan_threshold", "25%"),
    ("enable_overhang_bridge_fan", "1"),
    ("close_fan_the_first_x_layers", default_close_fan_layers),
    ("dont_slow_down_outer_wall", "0"),
    ("slow_down_for_layer_cooling", "1"),
    ("slow_down_layer_time", "8"),
    ("slow_down_min_speed", "20"),
    ("additional_cooling_fan_speed", "0"),
    ("activate_air_filtration", if is_open_bedslinger { "0" } else { "1" }),
    (
      "during_print_exhaust_fan_speed",
      if is_warp_prone_spool || is_open_bedslinger {
        "0"
      } else {
        "60"
      },
    ),
    (
      "complete_print_exhaust_fan_speed",
      if is_open_bedslinger { "0" } else { "80" },
    ),
    ("chamber_temperature", "0"),
    ("reduce_fan_stop_start_freq", "1"),
    ("fan_cooling_layer_time", "30"),
    ("adaptive_pressure_advance", "0"),
    ("enable_pressure_advance", if enable_pa_for_rig { "1" } else { "0" }),
  ] {
    if !base_map.contains_key(opt_key) {
      base_map.insert(opt_key.to_string(), serde_json::json!([opt_default]));
    }
  }
  let seed_ovh_fan = first_str_from_map(&base_map, "overhang_fan_speed")
    .and_then(|s| s.trim().trim_end_matches('%').parse::<i32>().ok())
    .unwrap_or(max_ovh_fan_pct)
    .clamp(0, max_ovh_fan_pct);

  // ===== Single Canonical Per-Spool Evaluation (`TransformationEngine::evaluate_stack`) =====
  // Seed `seed_map` (`x0`) with the resolved catalog/vendor/disk base + spool baseline + domain
  // `_bounds`, execute the active Lua transformation stack ONCE, and project `eval.resulting_profile.values`
  // directly into the exported preset map with ZERO post-Lua physics overwrites.
  let mut seed_map = base_map;
  if !is_external_disk_spool {
    seed_map.insert(
      "filament_retract_before_wipe".to_string(),
      serde_json::json!([retract_before_wipe_str.clone()]),
    );
    seed_map.insert(
      "filament_deretraction_speed".to_string(),
      serde_json::json!([default_deretract_spd]),
    );
    seed_map.insert("filament_wipe".to_string(), serde_json::json!(["1"]));
    seed_map.insert(
      "filament_wipe_distance".to_string(),
      serde_json::json!(["2"]),
    );
  } else if wet_retract_before_wipe.is_some() {
    seed_map.insert(
      "filament_retract_before_wipe".to_string(),
      serde_json::json!([retract_before_wipe_str.clone()]),
    );
  }
  if !seed_map.contains_key("filament_diameter") {
    seed_map.insert("filament_diameter".to_string(), serde_json::json!(["1.75"]));
  }

  // Seed any caller-supplied direct filament overrides (e.g. climate side-channels or test overrides
  // passed via `derived_overrides` without `_active_filament_rules_json`) into `seed_map` BEFORE
  // `evaluate_stack` so `evaluate_stack` owns the final map 100%.
  let mut pre_touched_opt_keys: Vec<String> = Vec::new();
  if let Some(ov) = derived_overrides {
    let lua_touched_set: Option<std::collections::HashSet<&str>> = ov
      .get("_lua_touched_keys")
      .and_then(|v| v.as_str())
      .map(|s| s.split(';').filter(|k| !k.is_empty()).collect());
    if let Some(ref touched) = lua_touched_set {
      for &opt_key in OPTIONAL_FILAMENT_SETTING_KEYS {
        if touched.contains(opt_key) {
          if let Some(s) = first_str_from_map(ov, opt_key) {
            let final_s = if opt_key == "filament_retract_lift_enforce" {
              let normalized = match s.trim() {
                "Top Surfaces Only" | "top_only" | "top" => "Top Only",
                "Bottom Surfaces Only" | "bottom_only" | "bottom" => "Bottom Only",
                "Top and Bottom Surfaces" | "top_and_bottom" => "Top and Bottom",
                "All" | "all" | "all_surfaces" => "All Surfaces",
                other => other,
              };
              if ov.get("_climate_zhop_enforce").and_then(|v| v.as_str()) == Some(normalized) {
                lua_climate_zhop_enforce
                  .clone()
                  .unwrap_or_else(|| normalized.to_string())
              } else {
                normalized.to_string()
              }
            } else if opt_key == "during_print_exhaust_fan_speed" {
              let parsed_pct = s
                .trim()
                .trim_end_matches('%')
                .parse::<f64>()
                .map(|n| n.round() as i32)
                .unwrap_or(0);
              if is_warp_prone_spool {
                "0".to_string()
              } else if let Some(lua_ex) = lua_climate_exhaust_target {
                if lua_ex >= 0 && matches!(parsed_pct, 0 | 10 | 40 | 60 | 80 | 90 | 100) {
                  lua_ex.to_string()
                } else {
                  s
                }
              } else {
                s
              }
            } else if opt_key == "additional_cooling_fan_speed" {
              let parsed_pct = s
                .trim()
                .trim_end_matches('%')
                .parse::<f64>()
                .map(|n| n.round() as i32)
                .unwrap_or(0);
              if !rig_has_aux_fan || is_warp_prone_spool || is_fiber_pla_spool {
                "0".to_string()
              } else if let Some(lua_aux) = lua_climate_aux_fan_target {
                if lua_aux == 0 {
                  "0".to_string()
                } else if lua_aux > 0 {
                  let ref_aux = if ov.contains_key("_climate_aux_fan_target") {
                    if spool_base_aux_pct > 0 {
                      spool_base_aux_pct
                    } else {
                      lua_aux
                    }
                  } else if parsed_pct > 0 {
                    parsed_pct
                  } else {
                    lua_aux
                  };
                  ref_aux.min(lua_aux).to_string()
                } else if ov.contains_key("_climate_aux_fan_target") {
                  spool_base_aux_pct.to_string()
                } else {
                  s
                }
              } else {
                s
              }
            } else if opt_key == "overhang_fan_speed" {
              s.trim()
                .trim_end_matches('%')
                .parse::<f64>()
                .map(|n| (n.round() as i32).clamp(0, max_ovh_fan_pct).to_string())
                .unwrap_or(s)
            } else {
              s
            };
            seed_map.insert(opt_key.to_string(), serde_json::json!([final_s]));
            pre_touched_opt_keys.push(opt_key.to_string());
          }
        }
      }
      if !has_extra_rules {
        for k in [
          "filament_deretraction_speed",
          "filament_wipe",
          "filament_wipe_distance",
        ] {
          if touched.contains(k) {
            if let Some(s) = first_str_from_map(ov, k) {
              seed_map.insert(k.to_string(), serde_json::json!([s]));
            }
          }
        }
      }
    }
  }
  // Apply per-spool climate exhaust / aux-blower targets directly from THIS spool's own
  // `sym_weather_climate.lua` evaluation so Slot 0's polymer family never masks or leaks onto
  // other slots (e.g. PLA/PHA/TPU in Tropical get 60%/40% aux fan even when Slot 0 is PLA-CF/ABS).
  let ov_explicit_exhaust = derived_overrides
    .map(|ov| !ov.contains_key("_climate_exhaust_target") && ov.contains_key("during_print_exhaust_fan_speed"))
    .unwrap_or(false);
  let ov_explicit_aux = derived_overrides
    .map(|ov| !ov.contains_key("_climate_aux_fan_target") && ov.contains_key("additional_cooling_fan_speed"))
    .unwrap_or(false);
  if !is_open_bedslinger && !ov_explicit_exhaust {
    if is_warp_prone_spool {
      seed_map.insert(
        "during_print_exhaust_fan_speed".to_string(),
        serde_json::json!(["0"]),
      );
      pre_touched_opt_keys.push("during_print_exhaust_fan_speed".to_string());
    } else if let Some(lua_ex) = lua_climate_exhaust_target {
      if lua_ex >= 0 {
        seed_map.insert(
          "during_print_exhaust_fan_speed".to_string(),
          serde_json::json!([lua_ex.to_string()]),
        );
        pre_touched_opt_keys.push("during_print_exhaust_fan_speed".to_string());
      } else if spool_base_exhaust_pct > 0 {
        pre_touched_opt_keys.push("activate_air_filtration".to_string());
      }
    }
  }
  let is_foaming_spool = is_foaming_tpu || spool.finish_type.eq_ignore_ascii_case("Foaming");
  if !ov_explicit_aux {
    if !rig_has_aux_fan || is_warp_prone_spool || is_fiber_pla_spool || is_foaming_spool {
      seed_map.insert(
        "additional_cooling_fan_speed".to_string(),
        serde_json::json!(["0"]),
      );
      if parent_had_aux_fan || is_warp_prone_spool || is_fiber_pla_spool || is_foaming_spool {
        pre_touched_opt_keys.push("additional_cooling_fan_speed".to_string());
      }
    } else if let Some(lua_aux) = lua_climate_aux_fan_target {
      if lua_aux == 0 {
        seed_map.insert(
          "additional_cooling_fan_speed".to_string(),
          serde_json::json!(["0"]),
        );
        pre_touched_opt_keys.push("additional_cooling_fan_speed".to_string());
      } else if lua_aux > 0 {
        let target_aux = if spool_base_aux_pct > 0 {
          spool_base_aux_pct.min(lua_aux)
        } else {
          lua_aux
        };
        seed_map.insert(
          "additional_cooling_fan_speed".to_string(),
          serde_json::json!([target_aux.to_string()]),
        );
        pre_touched_opt_keys.push("additional_cooling_fan_speed".to_string());
      }
    }
  }

  let run_nozzle_curves =
    rig_opt.is_some() && !spool.is_siraya_foaming_air() && !is_foaming_tpu;
  let (brass_base, seed_temp, seed_init, pre_temp_delta_c, trim_floor, init_floor) =
    if let Some(rig) = rig_opt.filter(|_| run_nozzle_curves) {
      let noz_cfg = c.lookup_nozzle(&rig.nozzle_type);
      let noz_delta = if rig.nozzle_type.to_lowercase().contains("cm2") {
        0
      } else if noz_cfg.uses_hs_base_temp {
        c.hardware_limits.hs_base_temp_delta_c
      } else {
        noz_cfg.temp_offset_from_brass_c
      };
      let b_base = if spool.brass_base_temp > 0 {
        if (spool.print_temp - (spool.brass_base_temp + noz_delta)).abs() <= 1 {
          spool.brass_base_temp
        } else {
          (spool.print_temp - noz_delta).max(190)
        }
      } else {
        (spool.print_temp - noz_delta).max(190)
      };
      let pre_wet_seed =
        (b_base + climate_noz_delta + structural_boost).clamp(min_safe_noz, max_safe_noz);
      let s_temp = (pre_wet_seed + wet_temp_drop).clamp(min_safe_noz, max_safe_noz);
      let s_init = (pre_wet_seed + init_delta).clamp(min_safe_noz, max_safe_noz);
      let is_elastomer_spool = c.is_elastomer_family_material(&spool.material_type);
      let is_fiber_spool = spool.material_type.contains("-CF")
        || spool.material_type.contains("-GF")
        || spool.finish_type.contains("Carbon")
        || spool.finish_type.contains("Glass");
      let floor_relax = if !is_elastomer_spool && !is_fiber_spool && min_safe_noz <= 230 {
        c.hardware_limits.nozzle_temp_floor_relax_c
      } else {
        0
      };
      let mut t_floor = (min_safe_noz - floor_relax)
        .max(c.hardware_limits.min_extrusion_temp_c)
        .min(max_safe_noz);
      t_floor = t_floor.max((b_base - 16).min(max_safe_noz));
      let i_floor = min_safe_noz.max(b_base + init_delta - 8).min(max_safe_noz);
      (b_base, s_temp, s_init, s_temp - b_base, t_floor, i_floor)
    } else {
      let s_temp = effective_temp;
      let s_init = initial_temp.max(s_temp);
      (spool.brass_base_temp.max(s_temp), s_temp, s_init, 0, min_safe_noz, min_safe_noz)
    };

  let mvs_ceil: f32 = if is_bowden_rig && is_elastomer_early {
    3.8
  } else if orca_fil_type == "ABS" || orca_fil_type == "ASA" {
    24.0
  } else {
    22.0
  };
  let base_hop_positive = z_hop_str
    .trim()
    .parse::<f64>()
    .map(|v| v > 0.0)
    .unwrap_or(false);
  let base_ret_len_str = crate::constants::PolymerConstants::format_retraction_mm(base_ret_len);
  let seed_ovh_for_stack = first_str_from_map(&seed_map, "overhang_fan_speed")
    .and_then(|s| s.trim().trim_end_matches('%').parse::<i32>().ok())
    .unwrap_or(seed_ovh_fan)
    .clamp(0, max_ovh_fan_pct);

  let parent_plates = cat
    .instantiated_filament_plates
    .get(&inherits)
    .copied()
    .unwrap_or([-1; 6]);
  let seed_plate = |idx: usize, fallback_key: &str, fallback_val: i32| -> String {
    if parent_plates[idx] >= 0 {
      parent_plates[idx].to_string()
    } else {
      first_str_from_map(&seed_map, fallback_key).unwrap_or_else(|| fallback_val.to_string())
    }
  };
  let seed_cool = seed_plate(0, "cool_plate_temp", 35);
  let seed_tex_cool = seed_plate(1, "textured_cool_plate_temp", 40);
  let seed_supertack = seed_plate(2, "supertack_plate_temp", 35);
  let seed_eng = seed_plate(3, "eng_plate_temp", 0);
  let seed_hot = seed_plate(4, "hot_plate_temp", effective_bed);

  for (key, val) in [
    ("nozzle_temperature", seed_temp.to_string()),
    ("nozzle_temperature_initial_layer", seed_init.to_string()),
    ("nozzle_temperature_BRASS", seed_temp.to_string()),
    ("nozzle_temperature_initial_layer_BRASS", seed_init.to_string()),
    ("nozzle_temperature_HS", seed_temp.to_string()),
    ("nozzle_temperature_initial_layer_HS", seed_init.to_string()),
    ("hot_plate_temp", seed_hot.clone()),
    ("hot_plate_temp_initial_layer", seed_hot),
    ("textured_plate_temp", effective_bed.to_string()),
    ("textured_plate_temp_initial_layer", initial_bed.to_string()),
    ("cool_plate_temp", seed_cool.clone()),
    ("cool_plate_temp_initial_layer", seed_cool),
    ("eng_plate_temp", seed_eng.clone()),
    ("eng_plate_temp_initial_layer", seed_eng),
    ("textured_cool_plate_temp", seed_tex_cool.clone()),
    ("textured_cool_plate_temp_initial_layer", seed_tex_cool),
    ("supertack_plate_temp", seed_supertack.clone()),
    ("supertack_plate_temp_initial_layer", seed_supertack),
    ("filament_flow_ratio", format!("{:.2}", effective_flow_ratio)),
    ("filament_retraction_length", ret_len_str.clone()),
    ("filament_retraction_speed", ret_spd_str.clone()),
    ("filament_z_hop", z_hop_str.clone()),
    ("filament_z_hop_types", z_hop_type_str.clone()),
    ("pressure_advance", format!("{:.3}", effective_pa)),
    ("filament_max_volumetric_speed", format!("{:.1}", effective_mvs)),
    ("fan_min_speed", fan_min_pct.to_string()),
    ("fan_min_speed_BRASS", fan_min_pct.to_string()),
    ("fan_min_speed_HS", fan_min_pct.to_string()),
    ("fan_max_speed", fan_max_pct.to_string()),
    ("fan_max_speed_BRASS", fan_max_pct.to_string()),
    ("fan_max_speed_HS", fan_max_pct.to_string()),
    ("overhang_fan_speed", seed_ovh_for_stack.to_string()),
  ] {
    seed_map.insert(key.to_string(), serde_json::json!([val]));
  }
  seed_map.insert(
    "enable_pressure_advance".to_string(),
    serde_json::json!([if enable_pa_for_rig { "1" } else { "0" }]),
  );
  if !is_open_bedslinger {
    seed_map.insert(
      "activate_air_filtration".to_string(),
      serde_json::json!(["1"]),
    );
  }
  if structural_fan_cut_pct > 0 {
    seed_map.insert(
      "_structural_fan_cut_pct".to_string(),
      serde_json::Value::String(structural_fan_cut_pct.to_string()),
    );
  }
  seed_map.insert(
    "_enforce_init_ge_cont_nozzle".to_string(),
    serde_json::Value::String("1".to_string()),
  );

  let mut bounds_obj = serde_json::Map::new();
  let is_elastomer_spool = c.is_elastomer_family_material(&spool.material_type);
  let max_ret_spd = if is_elastomer_spool {
    c.hardware_limits.elastomer_max_retraction_speed_mms as f64
  } else {
    55.0
  };
  bounds_obj.insert(
    "filament_retraction_speed".to_string(),
    serde_json::json!({
      "min": 15.0,
      "max": max_ret_spd,
      "min_by": "Retraction Speed Physical Floor",
      "max_by": if is_elastomer_spool {
        "Elastomer Max Retraction Speed Ceiling"
      } else {
        "Direct-Drive / Bowden Retraction Speed Ceiling"
      },
    }),
  );
  for k in [
    "nozzle_temperature",
    "nozzle_temperature_BRASS",
    "nozzle_temperature_HS",
  ] {
    bounds_obj.insert(
      k.to_string(),
      serde_json::json!({
        "min": trim_floor as f64,
        "max": max_safe_noz as f64,
        "min_by": "Polymer + Machine Safe Extrusion Floor",
        "max_by": "Polymer + Machine Hotend Thermal Ceiling",
      }),
    );
  }
  for k in [
    "nozzle_temperature_initial_layer",
    "nozzle_temperature_initial_layer_BRASS",
    "nozzle_temperature_initial_layer_HS",
  ] {
    bounds_obj.insert(
      k.to_string(),
      serde_json::json!({
        "min": init_floor as f64,
        "max": max_safe_noz as f64,
        "min_by": "Polymer Safe First-Layer Extrusion Floor",
        "max_by": "Polymer + Machine Hotend Thermal Ceiling",
      }),
    );
  }
  bounds_obj.insert(
    "pressure_advance".to_string(),
    serde_json::json!({
      "min": if enable_pa_for_rig { 0.005 } else { 0.0 },
      "max": 0.120,
      "min_by": "Pressure Advance Physical Floor",
      "max_by": "Pressure Advance Physical Ceiling",
    }),
  );
  bounds_obj.insert(
    "filament_max_volumetric_speed".to_string(),
    serde_json::json!({
      "min": 1.5,
      "max": mvs_ceil as f64,
      "min_by": "Volumetric Flow Floor",
      "max_by": "Polymer Volumetric Flow Ceiling",
    }),
  );
  for k in [
    "fan_min_speed",
    "fan_min_speed_BRASS",
    "fan_min_speed_HS",
    "fan_max_speed",
    "fan_max_speed_BRASS",
    "fan_max_speed_HS",
  ] {
    bounds_obj.insert(
      k.to_string(),
      serde_json::json!({
        "min": 0.0,
        "max": 100.0,
        "min_by": "PWM Fan Speed Floor",
        "max_by": "PWM Fan Speed Ceiling",
      }),
    );
  }
  bounds_obj.insert(
    "overhang_fan_speed".to_string(),
    serde_json::json!({
      "min": 0.0,
      "max": max_ovh_fan_pct as f64,
      "min_by": "PWM Fan Speed Floor",
      "max_by": if max_ovh_fan_pct < 100 {
        "Warp-Prone Engineering Polymer Overhang Fan Ceiling"
      } else {
        "PWM Fan Speed Ceiling"
      },
    }),
  );
  for k in ["textured_plate_temp", "textured_plate_temp_initial_layer"] {
    bounds_obj.insert(
      k.to_string(),
      serde_json::json!({
        "min": climate_min_bed as f64,
        "max": max_safe_bed as f64,
        "min_by": "Polymer Safe Heated Bed Floor",
        "max_by": "Polymer + Machine Heated Bed Ceiling",
      }),
    );
  }
  if !base_hop_positive {
    bounds_obj.insert(
      "filament_z_hop".to_string(),
      serde_json::json!({
        "min": 0.0,
        "max": 0.0,
        "min_by": "Flexible / Foaming Polymer Zero Z-Hop Baseline",
        "max_by": "Flexible / Foaming Polymer Zero Z-Hop Baseline",
      }),
    );
  }
  seed_map.insert(
    "_bounds".to_string(),
    serde_json::Value::Object(bounds_obj),
  );

  let ctx = if let Some(rig) = rig_opt {
    let mc = crate::catalog::machine_constants(&rig.printer_model);
    serde_json::json!({
      "printer_model": rig.printer_model,
      "hotend": rig.hotend,
      "nozzle_type": rig.nozzle_type,
      "nozzle_material": rig.nozzle_material,
      "nozzle_diameter": rig.nozzle_diameter,
      "hotend_max_flow": rig.max_volumetric_flow_limit,
      "hotend_max_temp_c": mc.max_hotend_temp_c,
      "max_bed_temp_c": mc.max_bed_temp_c,
      "filament_bed_min_c": climate_min_bed,
      "filament_bed_max_c": max_safe_bed,
      "melt_zone_multiplier": rig.melt_zone_multiplier,
      "printer_kinematics_class": rig.kinematics_class(),
      "travel_accel_target_mm2": rig.travel_accel_policy().travel_accel_target_mm2,
      "travel_accel_floor_mm2": rig.travel_accel_policy().travel_accel_floor_mm2,
      "filament_type": spool.material_type,
      "filament_finish": spool.finish_type,
      "filament_nozzle_max_c": FilamentSpool::polymer_safe_nozzle_bounds(&spool.material_type).1,
      "filament_is_elastomer": is_elastomer_spool,
      "filament_is_warp_prone": is_warp_prone_spool,
      "addons": rig.addons,
      "climate_preset": rig.climate_preset,
      "ambient_temp_c": rig.ambient_temp_c,
      "humidity_pct": rig.humidity_pct,
      "pre_temp_delta_c": pre_temp_delta_c,
      "brass_base_temp_c": brass_base,
      "brass_base_init_temp_c": brass_base + init_delta,
      "brass_base_retract_mm": base_ret_len_str,
      "spool_base_bed_c": spool.base_bed_temp.max(35),
      "_is_anycubic_printer": is_anycubic_rig,
      "_parent_plate_cool": parent_plates[0],
      "_parent_plate_tex_cool": parent_plates[1],
      "_parent_plate_supertack": parent_plates[2],
      "_parent_plate_eng": parent_plates[3],
      "_parent_plate_hot": parent_plates[4],
      "_parent_plate_tex": parent_plates[5],
    })
  } else {
    serde_json::json!({
      "filament_type": spool.material_type,
      "filament_finish": spool.finish_type,
      "filament_nozzle_max_c": FilamentSpool::polymer_safe_nozzle_bounds(&spool.material_type).1,
      "filament_is_elastomer": is_elastomer_spool,
      "filament_is_warp_prone": is_warp_prone_spool,
      "max_bed_temp_c": machine_max_bed,
      "filament_bed_min_c": climate_min_bed,
      "filament_bed_max_c": max_safe_bed,
      "spool_base_bed_c": spool.base_bed_temp.max(35),
      "_parent_plate_cool": parent_plates[0],
      "_parent_plate_tex_cool": parent_plates[1],
      "_parent_plate_supertack": parent_plates[2],
      "_parent_plate_eng": parent_plates[3],
      "_parent_plate_hot": parent_plates[4],
      "_parent_plate_tex": parent_plates[5],
    })
  };

  let mut rules: Vec<crate::transformations::TransformationRule> = Vec::new();
  if let Some(rig) = rig_opt.filter(|_| run_nozzle_curves) {
    let mut def_rules = crate::transformations::TransformationEngine::get_default_rules();
    crate::transformations::TransformationEngine::sync_rules_for_rig(
      &mut def_rules,
      &rig.nozzle_type,
      &rig.nozzle_material,
      rig.ace_pro_units,
    );
    def_rules.retain(|r| {
      matches!(
        r.id.as_str(),
        "thermal_conductivity"
          | "rig_addons_cooling"
          | "symptom_stringing_diamondback"
          | "filament_pctg_antistringing"
          | "pctg_antistringing"
      )
    });
    rules.extend(def_rules);
  }
  for er in &extra_filament_rules {
    if er.enabled && !rules.iter().any(|existing| existing.id == er.id) {
      rules.push(er.clone());
    }
  }
  rules.push(crate::transformations::TransformationRule {
    id: "plate_surface_temperatures".to_string(),
    name: "Build Plate Surface Chemistry & Temperature Schedule".to_string(),
    description: "Computes per-plate temperatures (Textured PEI, Smooth PEI, Engineering, Cool Plate, Textured Cool, SuperTack) and syncs _BRASS/_HS siblings.".to_string(),
    category: "Build Plate Physics".to_string(),
    icon: "🌡️".to_string(),
    enabled: true,
    condition_expr: "all".to_string(),
    lua_code: include_str!("../transformations/plate_surface_temperatures.lua").to_string(),
    priority: 95,
    scope: crate::transformations::RuleScopeFilter::default(),
  });

  let seed_profile = crate::orca_profile::OrcaProfile {
    name: spool.name.clone(),
    profile_type: "filament".to_string(),
    inherits: None,
    from: None,
    values: seed_map,
  };
  let eval =
    crate::transformations::TransformationEngine::evaluate_stack(&seed_profile, &rules, &ctx);

  // Project `eval.resulting_profile.values` DIRECTLY as the canonical exported map!
  let mut out_map = eval.resulting_profile.values.clone();
  let mut opt_touched_merged: std::collections::BTreeSet<String> = pre_touched_opt_keys
    .into_iter()
    .collect();
  if let Some(t_str) = out_map.get("_lua_touched_keys").and_then(|v| v.as_str()) {
    for k in t_str.split(';').filter(|k| !k.is_empty()) {
      if is_filament_scoped_key(k) {
        opt_touched_merged.insert(k.to_string());
      }
    }
  }
  if is_warp_prone_spool {
    for warp_guard_key in [
      "overhang_fan_speed",
      "during_print_exhaust_fan_speed",
      "additional_cooling_fan_speed",
    ] {
      opt_touched_merged.insert(warp_guard_key.to_string());
    }
  }
  if is_fiber_pla_spool {
    for fiber_guard_key in [
      "additional_cooling_fan_speed",
      "close_fan_the_first_x_layers",
    ] {
      opt_touched_merged.insert(fiber_guard_key.to_string());
    }
  }
  if is_open_bedslinger {
    for enc_key in [
      "during_print_exhaust_fan_speed",
      "complete_print_exhaust_fan_speed",
      "activate_air_filtration",
      "chamber_temperature",
    ] {
      opt_touched_merged.remove(enc_key);
    }
  } else if opt_touched_merged.contains("during_print_exhaust_fan_speed")
    || opt_touched_merged.contains("complete_print_exhaust_fan_speed")
  {
    opt_touched_merged.insert("activate_air_filtration".to_string());
  }
  if !is_anycubic_rig {
    opt_touched_merged.retain(|k| !is_anycubic_fork_filament_key(k));
    for ac_k in [
      "nozzle_temperature_BRASS",
      "nozzle_temperature_HS",
      "nozzle_temperature_initial_layer_BRASS",
      "nozzle_temperature_initial_layer_HS",
      "fan_min_speed_BRASS",
      "fan_min_speed_HS",
      "fan_max_speed_BRASS",
      "fan_max_speed_HS",
      "nozzle_temperature_range_low_HS",
      "nozzle_temperature_range_high_HS",
    ] {
      out_map.remove(ac_k);
    }
  }

  // Ensure OrcaSlicer filament array shape `[scalar]` on all filament-scoped parameters
  let keys_snapshot: Vec<String> = out_map.keys().cloned().collect();
  for k in keys_snapshot {
    if k.starts_with('_') {
      continue;
    }
    if is_filament_scoped_key(&k) {
      if let Some(serde_json::Value::String(s)) = out_map.get(&k).cloned() {
        out_map.insert(k, serde_json::json!([s]));
      }
    }
  }
  if !opt_touched_merged.is_empty() {
    out_map.insert(
      "_spool_opt_touched".to_string(),
      serde_json::Value::String(opt_touched_merged.into_iter().collect::<Vec<_>>().join(";")),
    );
  }

  // Read final shipped scalars directly from `out_map` (zero physics overwrites!)
  let shipped_temp = first_str_from_map(&out_map, "nozzle_temperature")
    .and_then(|s| s.parse::<i32>().ok())
    .unwrap_or(seed_temp);
  let shipped_init_temp = first_str_from_map(&out_map, "nozzle_temperature_initial_layer")
    .and_then(|s| s.parse::<i32>().ok())
    .unwrap_or(seed_init);
  let shipped_bed = first_str_from_map(&out_map, "textured_plate_temp")
    .and_then(|s| s.parse::<i32>().ok())
    .unwrap_or(effective_bed);
  let shipped_flow = first_str_from_map(&out_map, "filament_flow_ratio")
    .and_then(|s| s.parse::<f32>().ok())
    .unwrap_or(effective_flow_ratio);
  let shipped_pa = first_str_from_map(&out_map, "pressure_advance")
    .and_then(|s| s.parse::<f32>().ok())
    .unwrap_or(effective_pa);
  let shipped_mvs = first_str_from_map(&out_map, "filament_max_volumetric_speed")
    .and_then(|s| s.parse::<f32>().ok())
    .unwrap_or(effective_mvs);
  let shipped_fan_min = first_str_from_map(&out_map, "fan_min_speed")
    .and_then(|s| s.parse::<i32>().ok())
    .unwrap_or(fan_min_pct);

  // Check if `anycubic_pctg_as_petg` declared PCTG->PETG or PC-CF->PC in `rig.transformation_rules`
  // when `derived_overrides` was not explicitly supplied by the caller.
  let eval_pctg_as_petg = derived_overrides.is_none()
    && first_str_from_map(&out_map, PCTG_AS_PETG_SIDE_CHANNEL).as_deref() == Some("1");
  let eval_pccf_as_pc = derived_overrides.is_none()
    && first_str_from_map(&out_map, PCCF_AS_PC_SIDE_CHANNEL).as_deref() == Some("1");
  if eval_pctg_as_petg && !pctg_declared_as_petg {
    declared_fil_type = declare_pctg_family_as_petg(&orca_fil_type);
  }
  let final_pctg_declared = declared_fil_type != orca_fil_type;
  let final_pccf_declared =
    (pccf_declared_as_pc || eval_pccf_as_pc) && is_reinforced_pc_material(&spool.material_type);
  if final_pccf_declared {
    declared_fil_type = declare_pccf_family_as_pc(&declared_fil_type);
  }

  let compat_printer = if let Some(r) = rig_opt {
    cat.resolve_machine_inherits(&r.printer_model, &r.formatted_nozzle_diameter())
  } else {
    let mc = &c.machines[0];
    cat.resolve_machine_inherits(&mc.printer_model, &mc.default_nozzle_diameter)
  };
  out_map.remove("filament_vendor");
  out_map.insert(
    "compatible_printers".to_string(),
    serde_json::json!([compat_printer]),
  );
  out_map.insert("inherits".to_string(), serde_json::json!(inherits));
  out_map.insert("name".to_string(), serde_json::json!(export_name));
  out_map.insert(
    "filament_settings_id".to_string(),
    serde_json::json!([export_name]),
  );
  out_map.insert(
    "filament_type".to_string(),
    serde_json::json!([declared_fil_type]),
  );
  out_map.insert(
    "required_nozzle_HRC".to_string(),
    serde_json::json!([spool.required_nozzle_hrc.to_string()]),
  );
  out_map.insert(
    "default_filament_colour".to_string(),
    serde_json::json!([spool.color_hex]),
  );
  let (range_low, range_high) = FilamentSpool::compute_nozzle_temp_range_for_machine(
    &spool.material_type,
    shipped_temp,
    shipped_init_temp,
    machine_max_noz,
  );
  out_map.insert(
    "nozzle_temperature_range_low".to_string(),
    serde_json::json!([range_low.to_string()]),
  );
  out_map.insert(
    "nozzle_temperature_range_high".to_string(),
    serde_json::json!([range_high.to_string()]),
  );
  if is_anycubic_rig {
    out_map.insert(
      "nozzle_temperature_range_low_HS".to_string(),
      serde_json::json!([range_low.to_string()]),
    );
    out_map.insert(
      "nozzle_temperature_range_high_HS".to_string(),
      serde_json::json!([range_high.to_string()]),
    );
  }
  out_map.insert(
    "enable_pressure_advance".to_string(),
    first_str_from_map(&out_map, "enable_pressure_advance")
      .map(|s| serde_json::json!([s]))
      .unwrap_or_else(|| serde_json::json!([if enable_pa_for_rig { "1" } else { "0" }])),
  );

  let mut notes = spool.generate_descriptive_notes_with_effective_values_and_pa(
    shipped_temp,
    shipped_bed,
    shipped_flow,
    shipped_pa,
    shipped_mvs,
    shipped_fan_min,
  );
  if final_pctg_declared {
    notes.push_str(&format!(
      " {} filament_type={} (slicer lacks {}); temps follow {} limits {}–{}°C nozzle, {}–{}°C bed.",
      PCTG_AS_PETG_NOTE_MARKER,
      declared_fil_type,
      orca_fil_type,
      spool.material_type,
      min_safe_noz,
      max_safe_noz,
      min_safe_bed,
      max_safe_bed
    ));
  }
  if final_pccf_declared {
    notes.push_str(&format!(
      " {} filament_type={} (ACE Pro 2 / slicer lacks {}); temps follow {} limits {}–{}°C nozzle, {}–{}°C bed.",
      PCCF_AS_PC_NOTE_MARKER,
      declared_fil_type,
      spool.material_type,
      spool.material_type,
      min_safe_noz,
      max_safe_noz,
      min_safe_bed,
      max_safe_bed
    ));
  }
  out_map.insert("filament_notes".to_string(), serde_json::json!([notes]));
  out_map.insert(
    "version".to_string(),
    serde_json::json!(WEBCONFIG_ORCA_PRESET_VERSION),
  );
  (out_map, eval)
}

fn build_filament_preset_json_with_source(
  spool: &FilamentSpool,
  rig: &MachineRig,
  derived_overrides: Option<&serde_json::Map<String, serde_json::Value>>,
  from_src: &str,
) -> String {
  let resolved =
    resolve_spool_orca_filament_map_with_rig_and_overrides(spool, Some(rig), derived_overrides);
  let mut out = serde_json::Map::new();
  out.insert("type".to_string(), serde_json::json!("filament"));
  out.insert("from".to_string(), serde_json::json!(from_src));
  out.insert("is_custom_defined".to_string(), serde_json::json!("0"));

  for meta_key in [
    "inherits",
    "name",
    "filament_settings_id",
    "compatible_printers",
  ] {
    if let Some(v) = resolved.get(meta_key) {
      out.insert(meta_key.to_string(), v.clone());
    }
  }
  let rig_fil_keys = filament_setting_keys_for_rig(rig);
  let is_ac_rig = is_anycubic_printer_rig(rig);
  let is_bedslinger_rig = rig.is_bedslinger();
  for key in &rig_fil_keys {
    if let Some(v) = resolved.get(*key) {
      out.insert((*key).to_string(), v.clone());
    }
  }
  let mut merged_opt_touched: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
  if let Some(ov) = derived_overrides {
    let is_process_eval_bundle = ov.contains_key("_evaluated_for_layer_height")
      || ov.contains_key("_active_filament_rules_json");
    if !is_process_eval_bundle {
      if let Some(touched_str) = ov.get("_lua_touched_keys").and_then(|v| v.as_str()) {
        for k in touched_str.split(';').filter(|k| !k.is_empty()) {
          if is_filament_scoped_key(k)
            && (is_ac_rig || !is_anycubic_fork_filament_key(k))
            && !(is_bedslinger_rig
              && matches!(
                k,
                "during_print_exhaust_fan_speed"
                  | "complete_print_exhaust_fan_speed"
                  | "activate_air_filtration"
                  | "chamber_temperature"
              ))
          {
            merged_opt_touched.insert(k.to_string());
          }
        }
      }
    }
  }
  if let Some(sp_str) = resolved.get("_spool_opt_touched").and_then(|v| v.as_str()) {
    for k in sp_str.split(';').filter(|k| !k.is_empty()) {
      if is_filament_scoped_key(k) && (is_ac_rig || !is_anycubic_fork_filament_key(k)) {
        merged_opt_touched.insert(k.to_string());
      }
    }
  }
  if !is_bedslinger_rig
    && (merged_opt_touched.contains("during_print_exhaust_fan_speed")
      || merged_opt_touched.contains("complete_print_exhaust_fan_speed"))
  {
    merged_opt_touched.insert("activate_air_filtration".to_string());
  }
  for opt_key in merged_opt_touched {
    if !out.contains_key(&opt_key) {
      if let Some(v) = resolved.get(&opt_key) {
        out.insert(opt_key, v.clone());
      }
    }
  }
  if let Some(v) = resolved.get("version") {
    out.insert("version".to_string(), v.clone());
  }

  serde_json::to_string_pretty(&serde_json::Value::Object(out)).unwrap_or_else(|_| "{}".to_string())
}

/// Export a standalone OrcaSlicer Filament `.json` delta preset (Section 4.A)
pub fn export_standalone_filament_json(spool: &FilamentSpool, rig: &MachineRig) -> String {
  build_filament_preset_json_with_source(spool, rig, None, "User")
}

/// Export a standalone OrcaSlicer Filament `.json` delta preset with active Lua `derived_overrides`
/// (such as `_wet_filament_*` humidity adjustments), guaranteeing 100% key-for-key parity with
/// `.3mf` `Metadata/project_settings.config`.
pub fn export_standalone_filament_json_with_overrides(
  spool: &FilamentSpool,
  rig: &MachineRig,
  derived_overrides: &serde_json::Map<String, serde_json::Value>,
) -> String {
  build_filament_preset_json_with_source(spool, rig, Some(derived_overrides), "User")
}

/// Evaluates the effective OrcaSlicer profile parameters by starting from the imported OrcaSlicer
/// base profile (`@orcaslicer_repo` `default_machine_base` + `default_process_base`), applying the
/// active `ProcessProfile` speeds/heights, running the default Lua transformation stack if
/// `derived_overrides` has not yet been evaluated, and merging `derived_overrides` on top.
pub fn resolve_effective_orca_profile_values(
  rig: &MachineRig,
  process: &ProcessProfile,
  derived_overrides: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
  let mut base = crate::orca_profile::OrcaProfile::get_stock_for_printer_model(&rig.printer_model);
  base.set_str("layer_height", &format!("{:.2}", process.layer_height));
  base.set_str("line_width", &format!("{:.2}", process.line_width));
  base.set_str(
    "outer_wall_line_width",
    &format!("{:.2}", process.line_width),
  );
  base.set_str(
    "outer_wall_speed",
    &format!("{:.0}", process.outer_wall_speed),
  );
  base.set_str(
    "inner_wall_speed",
    &format!("{:.0}", process.inner_wall_speed),
  );
  base.set_str(
    "sparse_infill_speed",
    &format!("{:.0}", process.sparse_infill_speed),
  );
  base.set_str(
    "initial_layer_speed",
    &format!("{:.0}", process.initial_layer_speed),
  );
  base.set_str(
    "default_acceleration",
    &format!("{:.0}", process.default_acceleration),
  );
  base.set_str("wall_generator", &process.wall_generator);
  base.set_str("print_sequence", &process.print_sequence);
  base.set_str("notes", &process.notes);

  let mut effective = if derived_overrides.contains_key("seam_slope_type") {
    base.values
  } else {
    let mut rules = crate::transformations::TransformationEngine::get_default_rules();
    crate::transformations::TransformationEngine::sync_rules_for_rig(
      &mut rules,
      &rig.nozzle_type,
      &rig.nozzle_material,
      rig.ace_pro_units,
    );
    let ctx = serde_json::json!({
      "printer_model": rig.printer_model,
      "nozzle_type": rig.nozzle_type,
      "nozzle_material": rig.nozzle_material,
      "nozzle_diameter": rig.nozzle_diameter,
      "hotend_max_flow": rig.max_volumetric_flow_limit,
      "hotend_max_temp_c": crate::catalog::machine_constants(&rig.printer_model).max_hotend_temp_c,
      "melt_zone_multiplier": rig.melt_zone_multiplier,
      "printer_kinematics_class": rig.kinematics_class(),
      "travel_accel_target_mm2": rig.travel_accel_policy().travel_accel_target_mm2,
      "travel_accel_floor_mm2": rig.travel_accel_policy().travel_accel_floor_mm2,
      "layer_height": process.layer_height,
      "line_width": process.line_width,
      "process_tier": crate::transformations::classify_process_scope_tier(process),
      "addons": rig.addons,
    });
    crate::transformations::TransformationEngine::evaluate_stack(&base, &rules, &ctx)
      .resulting_profile
      .values
  };

  let is_optical_glass_mode = derived_overrides
    .get("sparse_infill_pattern")
    .and_then(|v| v.as_str())
    == Some("alignedrectilinear")
    && derived_overrides
      .get("sparse_infill_density")
      .and_then(|v| v.as_str())
      == Some("100%");
  let has_wet_filament_first_layer = derived_overrides
    .get("_wet_filament_init_bed_boost_c")
    .and_then(|v| v.as_str())
    .and_then(|s| s.parse::<i32>().ok())
    .unwrap_or(0)
    > 0;
  let has_cold_climate_first_layer = derived_overrides
    .get("_climate_init_bed_extra_c")
    .and_then(|v| v.as_str())
    .and_then(|s| s.parse::<f64>().ok())
    .unwrap_or(0.0)
    > 0.0;
  let has_first_layer_slowdown = has_wet_filament_first_layer || has_cold_climate_first_layer;

  let eval_lh = derived_overrides
    .get("_evaluated_for_layer_height")
    .and_then(|v| v.as_str())
    .and_then(|s| s.parse::<f64>().ok())
    .unwrap_or(process.layer_height);
  let get_user_scale = |key: &str| -> f64 {
    derived_overrides
      .get(&format!("_user_rule_scale_{}", key))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<f64>().ok())
      .unwrap_or(1.0)
  };

  for (k, v) in derived_overrides {
    if k.starts_with('_') || is_filament_scoped_key(k) {
      continue;
    }
    if k == "layer_height" {
      continue;
    }
    // Do not let single-process/single-filament speed caps from `derived_overrides` overwrite
    // each distinct ProcessProfile's own speeds (`outer_wall_speed`, `inner_wall_speed`, `sparse_infill_speed`,
    // `internal_solid_infill_speed`, `initial_layer_speed`, `default_acceleration`), unless Optical Glass mode
    // or Wet Filament / Cold Climate first-layer recovery is active.
    if !is_optical_glass_mode
      && !(has_first_layer_slowdown
        && matches!(
          k.as_str(),
          "initial_layer_speed" | "initial_layer_infill_speed"
        ))
      && !(matches!(k.as_str(), "line_width" | "outer_wall_line_width")
        && v.as_str() != Some(&format!("{:.2}", process.line_width)))
      && matches!(
        k.as_str(),
        "line_width"
          | "outer_wall_line_width"
          | "outer_wall_speed"
          | "inner_wall_speed"
          | "sparse_infill_speed"
          | "internal_solid_infill_speed"
          | "top_surface_speed"
          | "gap_infill_speed"
          | "support_speed"
          | "initial_layer_speed"
          | "initial_layer_infill_speed"
          | "default_acceleration"
      )
    {
      continue;
    }
    if k == "travel_acceleration"
      && !derived_overrides.contains_key("_bounds")
      && !derived_overrides.contains_key("_lua_touched_keys")
    {
      // Fallback for synthetic override maps that bypassed `TransformationEngine::evaluate_stack`
      // (stacks evaluated through `evaluate_stack` enforce `travel_acceleration` bounds via
      // `constrain("travel_acceleration", ...)` and the Priority = ∞ Epilogue Projection pass).
      let accel_policy = rig.travel_accel_policy();
      if let Some(ta) = v.as_str().and_then(|s| s.parse::<f64>().ok()) {
        if ta < accel_policy.travel_accel_floor_mm2 as f64 {
          effective.insert(
            k.clone(),
            serde_json::json!(accel_policy.travel_accel_stock_mm2.to_string()),
          );
          continue;
        }
      }
    }
    // Scale layer-height-dependent overrides (`support_top_z_distance`, `support_bottom_z_distance`,
    // `top_shell_thickness`, `bottom_shell_thickness`) accurately for this specific `process.layer_height`.
    if (process.layer_height - eval_lh).abs() > 1e-4 && eval_lh > 0.01 {
      if matches!(
        k.as_str(),
        "support_top_z_distance" | "support_bottom_z_distance"
      ) {
        if let Some(zd_val) = v.as_str().and_then(|s| s.parse::<f64>().ok()) {
          if zd_val > 0.01 {
            let zd_offset = zd_val - eval_lh;
            let adjusted = (process.layer_height + zd_offset).max(0.04);
            effective.insert(k.clone(), serde_json::json!(format!("{:.2}", adjusted)));
            continue;
          }
        }
      } else if matches!(k.as_str(), "top_shell_thickness" | "bottom_shell_thickness") {
        if let Some(th_val) = v.as_str().and_then(|s| s.parse::<f64>().ok()) {
          if th_val > 0.01 {
            let adjusted = th_val * (process.layer_height / eval_lh);
            effective.insert(k.clone(), serde_json::json!(format!("{:.2}", adjusted)));
            continue;
          }
        }
      }
    }
    effective.insert(k.clone(), v.clone());
  }
  if !is_optical_glass_mode {
    // Preserve each ProcessProfile's exact configured speeds and acceleration without artificial hotend clamping,
    // while applying any explicit user-enabled speed/acceleration/line-width Lua rule scale factors!
    for (key, raw_spd) in [
      ("outer_wall_speed", process.outer_wall_speed),
      ("inner_wall_speed", process.inner_wall_speed),
      ("sparse_infill_speed", process.sparse_infill_speed),
      ("default_acceleration", process.default_acceleration),
    ] {
      let scale = get_user_scale(key);
      let mut scaled_spd = (raw_spd * scale).round().max(15.0);
      if key == "outer_wall_speed" {
        if let Some(vfa_mode) = derived_overrides
          .get("_outer_wall_vfa_mode")
          .and_then(|v| v.as_str())
        {
          let sub_spd = derived_overrides
            .get("_outer_wall_vfa_sub_speed")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(55.0);
          let clear_spd = derived_overrides
            .get("_outer_wall_vfa_clear_speed")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(145.0);
          let eff_mvs = derived_overrides
            .get("_outer_wall_vfa_effective_mvs")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(18.0);
          let is_silk = derived_overrides
            .get("_outer_wall_vfa_is_silk")
            .and_then(|v| v.as_str())
            == Some("1");
          let lock_pref = derived_overrides
            .get("_outer_wall_vfa_lock_pref")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");
          let ow_width = effective
            .get("outer_wall_line_width")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(process.line_width);
          let cross_sec = (process.layer_height * ow_width).max(0.01);
          let mvs_ceiling = (eff_mvs / cross_sec).floor();
          let actual_outer_spd = scaled_spd.min(mvs_ceiling);
          let is_speed_or_draft_tier = process.quality_tier() == "high_speed"
            || process.layer_height >= 0.24
            || process.outer_wall_speed >= 245.0;
          let is_corexy_vfa = derived_overrides
            .get("_outer_wall_vfa_is_corexy")
            .and_then(|v| v.as_str())
            != Some("0");
          if vfa_mode == "sub_resonance_gloss"
            || (is_silk && scaled_spd > sub_spd + 5.0)
            || (is_corexy_vfa
              && !is_speed_or_draft_tier
              && actual_outer_spd > sub_spd + 5.0
              && actual_outer_spd < clear_spd)
          {
            scaled_spd = (sub_spd * scale).round().max(15.0);
          }
          let _ = lock_pref;
        }
      }
      effective.insert(
        key.to_string(),
        serde_json::json!(format!("{:.0}", scaled_spd)),
      );
    }
    // `inner_wall_line_width` / `sparse_infill_line_width` now flow straight through from the
    // Lua stack (nozzle-proportional Line Width Geometry pack + relative symptom wizards).
    let il_scale = get_user_scale("initial_layer_speed");
    if !has_first_layer_slowdown {
      let scaled_il = (process.initial_layer_speed * il_scale).round().max(10.0);
      effective.insert(
        "initial_layer_speed".to_string(),
        serde_json::json!(format!("{:.0}", scaled_il)),
      );
    }
  }
  if effective.get("seam_slope_type").and_then(|v| v.as_str()) == Some("contour") {
    effective.insert(
      "seam_slope_type".to_string(),
      serde_json::Value::String("external".to_string()),
    );
  }
  effective
}

pub const EXTRA_PROCESS_OVERRIDE_KEYS: &[&str] = &[
  "initial_layer_line_width",
  "inner_wall_line_width",
  "sparse_infill_line_width",
  "top_surface_line_width",
  "internal_solid_infill_line_width",
  "support_line_width",
  "seam_slope_inner_walls",
  "enable_support",
  "support_type",
  "support_style",
  "support_top_z_distance",
  "support_bottom_z_distance",
  "support_interface_top_layers",
  "support_interface_bottom_layers",
  "support_interface_spacing",
  "support_interface_speed",
  "support_object_xy_distance",
  "ironing_type",
  "ironing_flow",
  "ironing_speed",
  "ironing_spacing",
  "top_surface_pattern",
  "bottom_surface_pattern",
  "internal_solid_infill_pattern",
  "infill_direction",
  "only_one_wall_top",
  "wall_sequence",
  "precise_outer_wall",
  "xy_hole_compensation",
  "xy_contour_compensation",
  "elefant_foot_compensation",
  "elefant_foot_compensation_layers",
  "bridge_flow",
  "bridge_speed",
  "enable_overhang_speed",
  "detect_overhang_wall",
  "overhang_1_4_speed",
  "overhang_2_4_speed",
  "overhang_3_4_speed",
  "overhang_4_4_speed",
  "wall_loops",
  "alternate_extra_wall",
  "top_shell_layers",
  "bottom_shell_layers",
  "top_shell_thickness",
  "bottom_shell_thickness",
  "infill_wall_overlap",
  "sparse_infill_pattern",
  "sparse_infill_density",
  "small_perimeter_speed",
  "small_perimeter_threshold",
  "bottom_solid_infill_flow_ratio",
  "top_surface_flow_ratio",
  "brim_type",
  "brim_width",
  "enable_prime_tower",
  "prime_tower_width",
  "prime_volume",
  "initial_layer_print_height",
  "max_travel_detour_distance",
  "min_bead_width",
  "min_feature_size",
  "outer_wall_acceleration",
  "outer_wall_jerk",
  "reduce_crossing_wall",
  "reduce_infill_retraction",
  "scarf_angle_threshold",
  "seam_gap",
  "seam_position",
  "seam_slope_conditional",
  "seam_slope_entire_loop",
  "seam_slope_min_length",
  "seam_slope_start_height",
  "seam_slope_steps",
  "seam_slope_type",
  "spiral_mode",
  "travel_acceleration",
  "travel_jerk",
  "wall_generator",
  "wall_transition_angle",
  "wall_transition_length",
  "wipe_on_loops",
];

/// Builds a user Process `.json` preset from the imported OrcaSlicer process base + Lua-evaluated overrides
pub fn build_process_preset_json(
  process: &ProcessProfile,
  rig: &MachineRig,
  derived_overrides: &serde_json::Map<String, serde_json::Value>,
) -> String {
  let cat = crate::catalog::load_merged_catalog();
  let effective = resolve_effective_orca_profile_values(rig, process, derived_overrides);
  let compat_printer =
    cat.resolve_machine_inherits(&rig.printer_model, &rig.formatted_nozzle_diameter());
  let proc_name = if process.name.trim().is_empty() {
    process.format_concise_title(&rig.short_model_tag())
  } else {
    process.name.clone()
  };

  let proc_inherits = process.resolve_inherits_for_rig(rig);
  let mut out = serde_json::Map::new();
  out.insert("type".to_string(), serde_json::json!("process"));
  out.insert("from".to_string(), serde_json::json!("User"));
  out.insert("is_custom_defined".to_string(), serde_json::json!("0"));
  out.insert("inherits".to_string(), serde_json::json!(proc_inherits));
  out.insert(
    "compatible_printers".to_string(),
    serde_json::json!([compat_printer]),
  );
  out.insert("name".to_string(), serde_json::json!(proc_name));
  out.insert(
    "print_settings_id".to_string(),
    serde_json::json!(proc_name),
  );

  for (k, base_val) in cat.process_base_for_model(&rig.printer_model) {
    if matches!(
      k.as_str(),
      "type"
        | "from"
        | "is_custom_defined"
        | "inherits"
        | "compatible_printers"
        | "name"
        | "print_settings_id"
        | "notes"
        | "version"
        | "filename_format"
    ) || is_filament_scoped_key(k)
      || is_machine_scoped_key(k)
    {
      continue;
    }
    if let Some(eff_val) = effective.get(k) {
      let is_core_or_changed = eff_val != base_val
        || matches!(
          k.as_str(),
          "layer_height"
            | "line_width"
            | "outer_wall_line_width"
            | "outer_wall_speed"
            | "inner_wall_speed"
            | "sparse_infill_speed"
            | "initial_layer_speed"
            | "default_acceleration"
            | "wall_generator"
            | "wall_transition_angle"
            | "wall_transition_length"
            | "min_feature_size"
            | "min_bead_width"
            | "seam_slope_type"
            | "seam_slope_conditional"
            | "seam_slope_start_height"
            | "seam_slope_entire_loop"
            | "seam_slope_inner_walls"
            | "seam_slope_min_length"
            | "seam_slope_steps"
            | "seam_gap"
            | "scarf_angle_threshold"
            | "wipe_on_loops"
            | "sparse_infill_pattern"
            | "reduce_crossing_wall"
            | "print_sequence"
        );
      if is_core_or_changed && eff_val.is_string() {
        out.insert(k.clone(), eff_val.clone());
      }
    }
  }
  let lua_touched_set: Option<std::collections::HashSet<&str>> = derived_overrides
    .get("_lua_touched_keys")
    .and_then(|v| v.as_str())
    .map(|s| s.split(';').filter(|k| !k.is_empty()).collect());
  for &extra_key in EXTRA_PROCESS_OVERRIDE_KEYS {
    if is_filament_scoped_key(extra_key) || is_machine_scoped_key(extra_key) {
      continue;
    }
    let is_touched = match &lua_touched_set {
      Some(set) => set.contains(extra_key),
      None => derived_overrides.contains_key(extra_key),
    };
    if !out.contains_key(extra_key)
      && (is_touched
        || extra_key == "seam_slope_inner_walls")
    {
      if let Some(v) = effective.get(extra_key).filter(|v| v.is_string()) {
        out.insert(extra_key.to_string(), v.clone());
      }
    }
  }
  let proc_notes = if process.has_auto_generated_notes() {
    process.generate_descriptive_notes_with_effective_values(&effective)
  } else {
    process.notes.clone()
  };
  out.insert("notes".to_string(), serde_json::json!(proc_notes));
  out.insert(
    "version".to_string(),
    serde_json::json!(WEBCONFIG_ORCA_PRESET_VERSION),
  );

  serde_json::to_string_pretty(&serde_json::Value::Object(out)).unwrap_or_else(|_| "{}".to_string())
}

fn format_retraction_length_str(ret_len: f64) -> String {
  if (ret_len - ret_len.round()).abs() < 1e-6 {
    format!("{:.1}", ret_len)
  } else {
    format!("{:.2}", ret_len)
      .trim_end_matches('0')
      .trim_end_matches('.')
      .to_string()
  }
}

/// Builds a user Machine `.json` preset from the imported OrcaSlicer machine base + rig settings
pub fn build_machine_preset_json(rig: &MachineRig) -> String {
  build_machine_preset_json_with_overrides(rig, None)
}

/// Builds a user Machine `.json` preset from the imported OrcaSlicer machine base + rig settings
/// and any machine-level Lua overrides (such as `machine_start_gcode` and `machine_end_gcode`).
pub fn build_machine_preset_json_with_overrides(
  rig: &MachineRig,
  derived_overrides: Option<&serde_json::Map<String, serde_json::Value>>,
) -> String {
  let cat = crate::catalog::load_merged_catalog();
  let (orca_noz, hrc) = rig.orca_machine_nozzle_type_and_hrc();
  let noz_diam_str = rig.formatted_nozzle_diameter();
  let notes_str = if rig.notes.trim().is_empty() {
    rig.generate_descriptive_notes()
  } else {
    rig.notes.clone()
  };
  let mach_name = if rig.name.trim().is_empty() {
    rig.format_concise_title()
  } else {
    rig.name.clone()
  };
  let mach_inherits = cat.resolve_machine_inherits(&rig.printer_model, &noz_diam_str);
  let mut out = serde_json::Map::new();
  out.insert("type".to_string(), serde_json::json!("machine"));
  out.insert("from".to_string(), serde_json::json!("User"));
  out.insert("is_custom_defined".to_string(), serde_json::json!("0"));
  out.insert("inherits".to_string(), serde_json::json!(mach_inherits));
  out.insert("name".to_string(), serde_json::json!(mach_name));
  out.insert(
    "printer_settings_id".to_string(),
    serde_json::json!(mach_name),
  );
  // Stamp every save (matching AnycubicSlicerNext's convention) so a later directory import can
  // order `OC -` incarnations of the same rig (nozzle swaps rename the preset) newest-first.
  out.insert(
    "printer_timestamp".to_string(),
    serde_json::json!(crate::models::now_printer_timestamp()),
  );
  out.insert(
    "printer_model".to_string(),
    serde_json::json!(rig.printer_model),
  );
  out.insert(
    "printer_variant".to_string(),
    serde_json::json!(noz_diam_str),
  );
  out.insert("nozzle_type".to_string(), serde_json::json!(orca_noz));
  out.insert("nozzle_hrc".to_string(), serde_json::json!(hrc.to_string()));
  out.insert(
    "nozzle_diameter".to_string(),
    serde_json::json!([noz_diam_str]),
  );
  let ret_len_str = format_retraction_length_str(rig.base_retraction_length);
  out.insert(
    "retraction_length".to_string(),
    serde_json::json!([ret_len_str]),
  );
  let cut_retract_override = derived_overrides.and_then(|ov| {
    let is_touched = ov
      .get("_lua_touched_keys")
      .and_then(|v| v.as_str())
      .map(|s| s.split(';').any(|k| k == "retraction_distances_when_cut"))
      .unwrap_or_else(|| ov.contains_key("retraction_distances_when_cut"));
    if is_touched {
      first_str_from_map(ov, "retraction_distances_when_cut")
    } else {
      None
    }
  });
  if let Some(cut_str) = cut_retract_override.filter(|_| is_anycubic_printer_rig(rig)) {
    out.insert(
      "retraction_distances_when_cut".to_string(),
      serde_json::json!([cut_str]),
    );
  } else if rig.ace_pro_units > 0 && is_anycubic_printer_rig(rig) {
    out.insert(
      "retraction_distances_when_cut".to_string(),
      serde_json::json!(["18"]),
    );
  }
  let cm_opt = cat.find_machine(&rig.printer_model);
  let has_exact_anycubic_base = cat
    .anycubic_machine_bases
    .contains_key(rig.printer_model.trim());
  if let Some(cm) = cm_opt.filter(|cm| !has_exact_anycubic_base && cm.default_retraction_speed_mms > 0.0) {
    out.insert(
      "retraction_speed".to_string(),
      serde_json::json!([format!("{:.0}", cm.default_retraction_speed_mms)]),
    );
  } else if let Some(ret_spd) = cat
    .machine_base_for_model(&rig.printer_model)
    .get("retraction_speed")
  {
    out.insert("retraction_speed".to_string(), ret_spd.clone());
  }
  if let Some(ov) = derived_overrides {
    for gcode_key in [
      "before_layer_change_gcode",
      "change_filament_gcode",
      "layer_change_gcode",
      "machine_end_gcode",
      "machine_pause_gcode",
      "machine_start_gcode",
      "time_lapse_gcode",
    ] {
      if let Some(s) = ov
        .get(gcode_key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
      {
        out.insert(gcode_key.to_string(), serde_json::json!(s));
      }
    }
  }
  out.insert("printer_notes".to_string(), serde_json::json!(notes_str));
  out.insert(
    "version".to_string(),
    serde_json::json!(WEBCONFIG_ORCA_PRESET_VERSION),
  );
  serde_json::to_string_pretty(&serde_json::Value::Object(out)).unwrap_or_else(|_| "{}".to_string())
}

/// Build the `Metadata/project_settings.config` JSON string for 4-slot or 8-slot ACE Pro loadouts (Section 4.B)
pub fn build_project_settings_config_json(
  rig: &MachineRig,
  process: &ProcessProfile,
  active_slots: &[FilamentSpool],
  flush_matrix: &[String],
  derived_overrides: &serde_json::Map<String, serde_json::Value>,
) -> String {
  let cat = crate::catalog::load_merged_catalog();
  let effective = resolve_effective_orca_profile_values(rig, process, derived_overrides);
  let (orca_nozzle_type, nozzle_hrc) = rig.orca_machine_nozzle_type_and_hrc();
  let noz_diam_str = rig.formatted_nozzle_diameter();
  let rig_notes = if rig.notes.trim().is_empty() {
    rig.generate_descriptive_notes()
  } else {
    rig.notes.clone()
  };
  let proc_notes = if process.has_auto_generated_notes() {
    process.generate_descriptive_notes_with_effective_values(&effective)
  } else {
    process.notes.clone()
  };
  let mach_name = if rig.name.trim().is_empty() {
    rig.format_concise_title()
  } else {
    rig.name.clone()
  };
  let proc_name = if process.name.trim().is_empty() {
    process.format_concise_title(&rig.short_model_tag())
  } else {
    process.name.clone()
  };

  let raw_slot_maps: Vec<serde_json::Map<String, serde_json::Value>> = active_slots
    .iter()
    .map(|sp| {
      resolve_spool_orca_filament_map_with_rig_and_overrides(sp, Some(rig), Some(derived_overrides))
    })
    .collect();
  // When multiple slots share the same `filament_settings_id` (`OC - *.json`), the file on disk
  // corresponds to the last slot with that `filament_settings_id`. Ensure all preset-level keys
  // (`FILAMENT_SETTING_KEYS`, including `default_filament_colour`) match that canonical disk file
  // so OrcaSlicer's `profile_print_params_same` returns true for every slot, while `filament_colour`
  // preserves each slot's individual spool color.
  let mut canonical_by_id: std::collections::HashMap<
    String,
    serde_json::Map<String, serde_json::Value>,
  > = std::collections::HashMap::new();
  for m in &raw_slot_maps {
    if let Some(fid) = first_str_from_map(m, "filament_settings_id") {
      canonical_by_id.insert(fid, m.clone());
    }
  }
  let slot_maps: Vec<serde_json::Map<String, serde_json::Value>> = raw_slot_maps
    .iter()
    .map(|m| {
      if let Some(fid) = first_str_from_map(m, "filament_settings_id") {
        if let Some(canon) = canonical_by_id.get(&fid) {
          return canon.clone();
        }
      }
      m.clone()
    })
    .collect();
  let slot_extruder_colours: Vec<String> = raw_slot_maps
    .iter()
    .map(|m| first_str_from_map(m, "default_filament_colour").unwrap_or_default())
    .collect();

  let collect_slot_vec = |key: &str| -> Vec<String> {
    slot_maps
      .iter()
      .map(|m| first_str_from_map(m, key).unwrap_or_default())
      .collect()
  };

  let rig_mach_base = cat.machine_base_for_model(&rig.printer_model);
  let rig_proc_base = cat.process_base_for_model(&rig.printer_model);
  let cm_opt = cat.find_machine(&rig.printer_model);
  let has_exact_anycubic_base = cat
    .anycubic_machine_bases
    .contains_key(rig.printer_model.trim());
  let mach_inherits = cat.resolve_machine_inherits(&rig.printer_model, &noz_diam_str);
  let compat_printer = mach_inherits.clone();
  let proc_inherits = process.resolve_inherits_for_rig(rig);
  let mut inherits_group: Vec<String> = Vec::with_capacity(active_slots.len() + 2);
  inherits_group.push(proc_inherits);
  for m in &slot_maps {
    inherits_group.push(first_str_from_map(m, "inherits").unwrap_or_default());
  }
  inherits_group.push(mach_inherits);

  let mut out = serde_json::Map::new();
  out.insert("from".to_string(), serde_json::json!("project"));
  out.insert(
    "curr_bed_type".to_string(),
    serde_json::json!("Textured PEI Plate"),
  );
  out.insert(
    "printer_model".to_string(),
    serde_json::json!(rig.printer_model),
  );
  out.insert(
    "printer_variant".to_string(),
    serde_json::json!(noz_diam_str),
  );
  out.insert(
    "printer_settings_id".to_string(),
    serde_json::json!(mach_name),
  );
  out.insert("printer_notes".to_string(), serde_json::json!(rig_notes));
  out.insert(
    "print_settings_id".to_string(),
    serde_json::json!(proc_name),
  );
  out.insert("notes".to_string(), serde_json::json!(proc_notes));
  out.insert(
    "nozzle_diameter".to_string(),
    serde_json::json!([noz_diam_str]),
  );
  out.insert(
    "nozzle_hrc".to_string(),
    serde_json::json!(nozzle_hrc.to_string()),
  );
  out.insert(
    "nozzle_type".to_string(),
    serde_json::json!(orca_nozzle_type),
  );
  out.insert(
    "retraction_length".to_string(),
    serde_json::json!([format_retraction_length_str(rig.base_retraction_length)]),
  );
  let is_ac_rig_proj = is_anycubic_printer_rig(rig);
  let is_bedslinger_proj = rig.is_bedslinger();
  let cut_retract_override = {
    let is_touched = derived_overrides
      .get("_lua_touched_keys")
      .and_then(|v| v.as_str())
      .map(|s| s.split(';').any(|k| k == "retraction_distances_when_cut"))
      .unwrap_or_else(|| derived_overrides.contains_key("retraction_distances_when_cut"));
    if is_touched && is_ac_rig_proj {
      first_str_from_map(derived_overrides, "retraction_distances_when_cut")
    } else {
      None
    }
  };
  let has_cut_retract_in_machine =
    cut_retract_override.is_some() || (rig.ace_pro_units > 0 && is_ac_rig_proj);
  if let Some(cut_str) = cut_retract_override {
    out.insert(
      "retraction_distances_when_cut".to_string(),
      serde_json::json!([cut_str]),
    );
  } else if rig.ace_pro_units > 0 && is_ac_rig_proj {
    out.insert(
      "retraction_distances_when_cut".to_string(),
      serde_json::json!(["18"]),
    );
  }
  if let Some(cm) = cm_opt.filter(|_| !has_exact_anycubic_base) {
    if cm.default_retraction_speed_mms > 0.0 {
      out.insert(
        "retraction_speed".to_string(),
        serde_json::json!([format!("{:.0}", cm.default_retraction_speed_mms)]),
      );
    }
    if !cm.printable_area.is_empty() {
      out.insert(
        "printable_area".to_string(),
        serde_json::json!(cm.printable_area),
      );
    }
    if !cm.printable_height.is_empty() {
      out.insert(
        "printable_height".to_string(),
        serde_json::json!(cm.printable_height),
      );
    }
  } else {
    for mach_key in ["retraction_speed", "printable_area", "printable_height"] {
      if let Some(v) = rig_mach_base
        .get(mach_key)
        .or_else(|| effective.get(mach_key))
      {
        out.insert(mach_key.to_string(), v.clone());
      }
    }
  }
  out.insert(
    "compatible_printers".to_string(),
    serde_json::json!([compat_printer]),
  );

  let slot_ids = collect_slot_vec("filament_settings_id");
  out.insert(
    "filament_settings_id".to_string(),
    serde_json::json!(slot_ids),
  );
  let rig_fil_keys = filament_setting_keys_for_rig(rig);
  for &fk in &rig_fil_keys {
    let vals = collect_slot_vec(fk);
    if fk == "default_filament_colour" {
      out.insert(
        "filament_colour".to_string(),
        serde_json::json!(slot_extruder_colours.clone()),
      );
    }
    out.insert(fk.to_string(), serde_json::json!(vals));
  }
  let lua_touched_set: Option<std::collections::HashSet<&str>> = derived_overrides
    .get("_lua_touched_keys")
    .and_then(|v| v.as_str())
    .map(|s| s.split(';').filter(|k| !k.is_empty()).collect());
  let mut any_slot_opt_touched: std::collections::BTreeSet<String> =
    std::collections::BTreeSet::new();
  if let Some(ref touched) = lua_touched_set {
    for &k in touched {
      if is_filament_scoped_key(k)
        && (is_ac_rig_proj || !is_anycubic_fork_filament_key(k))
        && !(is_bedslinger_proj
          && matches!(
            k,
            "during_print_exhaust_fan_speed"
              | "complete_print_exhaust_fan_speed"
              | "activate_air_filtration"
              | "chamber_temperature"
          ))
      {
        any_slot_opt_touched.insert(k.to_string());
      }
    }
  }
  for m in &slot_maps {
    if let Some(sp_str) = m.get("_spool_opt_touched").and_then(|v| v.as_str()) {
      for k in sp_str.split(';').filter(|k| !k.is_empty()) {
        if is_filament_scoped_key(k) && (is_ac_rig_proj || !is_anycubic_fork_filament_key(k)) {
          any_slot_opt_touched.insert(k.to_string());
        }
      }
    }
  }
  if !is_bedslinger_proj
    && (any_slot_opt_touched.contains("during_print_exhaust_fan_speed")
      || any_slot_opt_touched.contains("complete_print_exhaust_fan_speed"))
  {
    any_slot_opt_touched.insert("activate_air_filtration".to_string());
  }
  for opt_key in &any_slot_opt_touched {
    if !out.contains_key(opt_key) {
      let vals = collect_slot_vec(opt_key);
      out.insert(opt_key.clone(), serde_json::json!(vals));
    }
  }
  let per_slot_diff_strs: Vec<String> = slot_maps
    .iter()
    .map(|m| {
      let mut keys: Vec<String> = rig_fil_keys.iter().map(|s| s.to_string()).collect();
      let slot_sp_set: std::collections::HashSet<&str> = m
        .get("_spool_opt_touched")
        .and_then(|v| v.as_str())
        .map(|s| s.split(';').filter(|k| !k.is_empty()).collect())
        .unwrap_or_default();
      for opt_key in &any_slot_opt_touched {
        let in_global = lua_touched_set
          .as_ref()
          .map(|t| t.contains(opt_key.as_str()))
          .unwrap_or(false);
        if (in_global
          || slot_sp_set.contains(opt_key.as_str())
          || (!is_bedslinger_proj
            && opt_key == "activate_air_filtration"
            && (slot_sp_set.contains("during_print_exhaust_fan_speed")
              || slot_sp_set.contains("complete_print_exhaust_fan_speed"))))
          && !keys.contains(opt_key)
        {
          keys.push(opt_key.clone());
        }
      }
      keys.join(";")
    })
    .collect();

  let mut proc_diff_keys = std::collections::BTreeSet::new();
  proc_diff_keys.insert("notes".to_string());
  for (k, base_val) in cat.process_base_for_model(&rig.printer_model) {
    if matches!(
      k.as_str(),
      "type"
        | "from"
        | "is_custom_defined"
        | "inherits"
        | "compatible_printers"
        | "name"
        | "print_settings_id"
        | "notes"
        | "version"
        | "filename_format"
    ) || is_filament_scoped_key(k)
      || is_machine_scoped_key(k)
    {
      continue;
    }
    if let Some(eff_val) = effective.get(k) {
      let is_core_or_changed = eff_val != base_val
        || matches!(
          k.as_str(),
          "layer_height"
            | "line_width"
            | "outer_wall_line_width"
            | "outer_wall_speed"
            | "inner_wall_speed"
            | "sparse_infill_speed"
            | "initial_layer_speed"
            | "default_acceleration"
            | "wall_generator"
            | "wall_transition_angle"
            | "wall_transition_length"
            | "min_feature_size"
            | "min_bead_width"
            | "seam_slope_type"
            | "seam_slope_conditional"
            | "seam_slope_start_height"
            | "seam_slope_entire_loop"
            | "seam_slope_inner_walls"
            | "seam_slope_min_length"
            | "seam_slope_steps"
            | "seam_gap"
            | "scarf_angle_threshold"
            | "wipe_on_loops"
            | "sparse_infill_pattern"
            | "reduce_crossing_wall"
            | "print_sequence"
        );
      if is_core_or_changed && eff_val.is_string() {
        out.insert(k.clone(), eff_val.clone());
        proc_diff_keys.insert(k.clone());
      }
    }
  }
  for &extra_key in EXTRA_PROCESS_OVERRIDE_KEYS {
    if is_filament_scoped_key(extra_key) || is_machine_scoped_key(extra_key) {
      continue;
    }
    let is_touched = match &lua_touched_set {
      Some(set) => set.contains(extra_key),
      None => derived_overrides.contains_key(extra_key),
    };
    if !out.contains_key(extra_key)
      && (is_touched
        || extra_key == "seam_slope_inner_walls")
    {
      if let Some(v) = effective.get(extra_key).filter(|v| v.is_string()) {
        out.insert(extra_key.to_string(), v.clone());
        proc_diff_keys.insert(extra_key.to_string());
      }
    }
  }

  let mut machine_diff_keys: std::collections::BTreeSet<String> = [
    "nozzle_diameter",
    "nozzle_hrc",
    "nozzle_type",
    "printer_notes",
    "retraction_length",
    "retraction_speed",
  ]
  .into_iter()
  .map(|s| s.to_string())
  .collect();
  if has_cut_retract_in_machine {
    machine_diff_keys.insert("retraction_distances_when_cut".to_string());
  }
  for gcode_key in [
    "before_layer_change_gcode",
    "change_filament_gcode",
    "layer_change_gcode",
    "machine_end_gcode",
    "machine_pause_gcode",
    "machine_start_gcode",
    "time_lapse_gcode",
  ] {
    if let Some(s) = derived_overrides
      .get(gcode_key)
      .and_then(|v| v.as_str())
      .filter(|s| !s.trim().is_empty())
    {
      out.insert(gcode_key.to_string(), serde_json::json!(s));
      machine_diff_keys.insert(gcode_key.to_string());
    }
  }
  let machine_diff_str = machine_diff_keys.into_iter().collect::<Vec<_>>().join(";");

  let mut different_settings_to_system: Vec<String> = Vec::with_capacity(active_slots.len() + 2);
  different_settings_to_system.push(proc_diff_keys.into_iter().collect::<Vec<_>>().join(";"));
  for sdiff in per_slot_diff_strs {
    different_settings_to_system.push(sdiff);
  }
  different_settings_to_system.push(machine_diff_str);

  out.insert(
    "inherits_group".to_string(),
    serde_json::json!(inherits_group),
  );
  out.insert(
    "different_settings_to_system".to_string(),
    serde_json::json!(different_settings_to_system),
  );

  if let Some(fm) = effective
    .get("flush_multiplier")
    .or_else(|| rig_mach_base.get("printer_flush_multiplier"))
  {
    out.insert("flush_multiplier".to_string(), fm.clone());
  }
  let base_wipe_vol = rig_proc_base
    .get("prime_volume")
    .and_then(|v| v.as_str())
    .and_then(|s| s.parse::<i32>().ok())
    .map(|p| (p * 7).to_string())
    .unwrap_or_default();
  out.insert(
    "flush_volumes_vector".to_string(),
    serde_json::json!(vec![base_wipe_vol; active_slots.len() * 2]),
  );
  out.insert(
    "flush_volumes_matrix".to_string(),
    serde_json::json!(flush_matrix),
  );
  if has_exact_anycubic_base {
    if let Some(ver) = rig_mach_base
      .get("version")
      .or_else(|| effective.get("version"))
    {
      out.insert("version".to_string(), ver.clone());
    } else {
      out.insert("version".to_string(), serde_json::json!("02.05.01.00"));
    }
  } else {
    out.insert("version".to_string(), serde_json::json!("02.05.01.00"));
  }

  let rendered = serde_json::to_string_pretty(&serde_json::Value::Object(out))
    .unwrap_or_else(|_| "{}".to_string());

  #[cfg(debug_assertions)]
  {
    let mach_json = build_machine_preset_json_with_overrides(rig, Some(derived_overrides));
    let proc_json = build_process_preset_json(process, rig, derived_overrides);
    let mut disk_filaments_by_id: std::collections::HashMap<String, String> =
      std::collections::HashMap::new();
    for sp in active_slots {
      let fjson = export_standalone_filament_json_with_overrides(sp, rig, derived_overrides);
      let fid = sp.format_concise_title_for_rig(rig);
      disk_filaments_by_id.insert(fid, fjson.clone());
      if !sp.name.trim().is_empty() {
        disk_filaments_by_id.insert(sp.name.clone(), fjson);
      }
    }
    if let Err(errs) = verify_project_settings_matches_standalone_presets(
      &rendered,
      &mach_json,
      &proc_json,
      &disk_filaments_by_id,
    ) {
      panic!(
        "3MF project_settings.config diverged from standalone .json presets (would cause AnycubicSlicerNext profile_print_params_same == false and reset slots on New Project!):\n  - {}",
        errs.join("\n  - ")
      );
    }
  }

  rendered
}

/// Verifies that `Metadata/project_settings.config` matches the standalone Machine, Process, and
/// Filament `.json` files key-for-key under OrcaSlicer / AnycubicSlicerNext's
/// `PresetCollection::load_external_preset` (`update_non_diff_values_to_base_config` +
/// `profile_print_params_same`).
///
/// If even a single parameter differs between `project_settings.config` and the disk `.json` files:
/// - Slot 1 (`i == 0`, `LoadAndSelect::Always`) gets marked dirty (`is_dirty = true`) and triggers
///   the "Save / Discard modifications" dialog.
/// - Slots 2..N (`i = 1..N-1`, `LoadAndSelect::OnlyIfModified`) get loaded as temporary
///   `is_project_embedded = true` presets (`OC - ... (Loadout.3mf)`), which are erased by
///   `reset_project_embedded_presets()` when clicking "New Project" -> "Discard", resetting Slots 2..N
///   to `Anycubic PLA @Anycubic Kobra S1 0.4 nozzle`!
pub fn verify_project_settings_matches_standalone_presets(
  project_settings_json: &str,
  machine_json: &str,
  process_json: &str,
  filament_jsons_by_id: &std::collections::HashMap<String, String>,
) -> Result<(), Vec<String>> {
  let mut errors = Vec::new();

  let proj_obj = match serde_json::from_str::<serde_json::Value>(project_settings_json)
    .ok()
    .and_then(|v| v.as_object().cloned())
  {
    Some(o) => o,
    None => {
      errors.push("project_settings_json is not a valid JSON object".to_string());
      return Err(errors);
    }
  };
  let mach_obj = match serde_json::from_str::<serde_json::Value>(machine_json)
    .ok()
    .and_then(|v| v.as_object().cloned())
  {
    Some(o) => o,
    None => {
      errors.push("machine_json is not a valid JSON object".to_string());
      return Err(errors);
    }
  };
  let proc_obj = match serde_json::from_str::<serde_json::Value>(process_json)
    .ok()
    .and_then(|v| v.as_object().cloned())
  {
    Some(o) => o,
    None => {
      errors.push("process_json is not a valid JSON object".to_string());
      return Err(errors);
    }
  };

  let slot_ids: Vec<String> = proj_obj
    .get("filament_settings_id")
    .and_then(|v| v.as_array())
    .map(|arr| {
      arr
        .iter()
        .filter_map(|x| x.as_str().map(|s| s.to_string()))
        .collect()
    })
    .unwrap_or_default();
  if slot_ids.is_empty() {
    errors.push("project_settings.config has empty filament_settings_id".to_string());
    return Err(errors);
  }

  let diff_groups: Vec<std::collections::BTreeSet<String>> = proj_obj
    .get("different_settings_to_system")
    .and_then(|v| v.as_array())
    .map(|arr| {
      arr
        .iter()
        .map(|x| {
          x.as_str()
            .unwrap_or("")
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
        })
        .collect()
    })
    .unwrap_or_default();

  if diff_groups.len() != slot_ids.len() + 2 {
    errors.push(format!(
      "different_settings_to_system length ({}) != slot_ids.len() + 2 ({})",
      diff_groups.len(),
      slot_ids.len() + 2
    ));
    return Err(errors);
  }

  let is_ignored_meta_key = |k: &str| -> bool {
    matches!(
      k,
      "type"
        | "from"
        | "is_custom_defined"
        | "name"
        | "version"
        | "inherits"
        | "compatible_prints"
        | "compatible_prints_condition"
        | "compatible_printers"
        | "compatible_printers_condition"
        | "print_settings_id"
        | "filament_settings_id"
        | "sla_print_settings_id"
        | "sla_material_settings_id"
        | "printer_settings_id"
        | "printer_model"
        | "printer_variant"
        | "default_print_profile"
        | "default_filament_profile"
        | "default_sla_print_profile"
        | "default_sla_material_profile"
        | "printer_timestamp"
        | "print_timestamp"
        | "filament_timestamp"
    )
  };

  // 1. Verify Process preset (diff_groups[0])
  let proc_diff = &diff_groups[0];
  for (k, v) in &proc_obj {
    if is_ignored_meta_key(k) {
      continue;
    }
    if is_filament_scoped_key(k) {
      errors.push(format!(
        "Process preset leaked filament-scoped key '{}' ({:?})",
        k, v
      ));
    }
    if is_machine_scoped_key(k) {
      errors.push(format!(
        "Process preset leaked machine-scoped key '{}' ({:?})",
        k, v
      ));
    }
    if !proc_diff.contains(k) {
      errors.push(format!(
        "Process key '{}' is in standalone process .json ({:?}) but missing from different_settings_to_system[0]",
        k, v
      ));
    }
    match proj_obj.get(k) {
      Some(pv) if pv == v => {}
      other => errors.push(format!(
        "Process key '{}' mismatch: standalone .json={:?} vs 3mf project_settings={:?}",
        k, v, other
      )),
    }
  }
  for k in proc_diff {
    if is_ignored_meta_key(k) {
      continue;
    }
    if is_filament_scoped_key(k) {
      errors.push(format!(
        "different_settings_to_system[0] (Process) leaked filament-scoped key '{}'",
        k
      ));
    }
    if is_machine_scoped_key(k) {
      errors.push(format!(
        "different_settings_to_system[0] (Process) leaked machine-scoped key '{}'",
        k
      ));
    }
    match (proc_obj.get(k), proj_obj.get(k)) {
      (Some(dv), Some(pv)) if dv == pv => {}
      (dv, pv) => errors.push(format!(
        "Process diff key '{}' mismatch: standalone .json={:?} vs 3mf project_settings={:?}",
        k, dv, pv
      )),
    }
  }

  // 2. Verify Machine preset (diff_groups[slot_ids.len() + 1])
  let mach_diff = &diff_groups[slot_ids.len() + 1];
  for (k, v) in &mach_obj {
    if is_ignored_meta_key(k) {
      continue;
    }
    if is_filament_scoped_key(k) {
      errors.push(format!(
        "Machine preset leaked filament-scoped key '{}' ({:?})",
        k, v
      ));
    }
    if !mach_diff.contains(k) {
      errors.push(format!(
        "Machine key '{}' is in standalone machine .json ({:?}) but missing from different_settings_to_system[{}]",
        k,
        v,
        slot_ids.len() + 1
      ));
    }
    match proj_obj.get(k) {
      Some(pv) if pv == v => {}
      other => errors.push(format!(
        "Machine key '{}' mismatch: standalone .json={:?} vs 3mf project_settings={:?}",
        k, v, other
      )),
    }
  }
  for k in mach_diff {
    if is_ignored_meta_key(k) {
      continue;
    }
    if is_filament_scoped_key(k) {
      errors.push(format!(
        "different_settings_to_system[{}] (Machine) leaked filament-scoped key '{}'",
        slot_ids.len() + 1,
        k
      ));
    }
    match (mach_obj.get(k), proj_obj.get(k)) {
      (Some(dv), Some(pv)) if dv == pv => {}
      (dv, pv) => errors.push(format!(
        "Machine diff key '{}' mismatch: standalone .json={:?} vs 3mf project_settings={:?}",
        k, dv, pv
      )),
    }
  }

  // 3. Verify every Filament slot (diff_groups[1..=slot_ids.len()])
  for (slot_idx, fid) in slot_ids.iter().enumerate() {
    let fil_str = match filament_jsons_by_id.get(fid) {
      Some(s) => s,
      None => {
        errors.push(format!(
          "Slot {} filament '{}' missing from filament_jsons_by_id",
          slot_idx + 1,
          fid
        ));
        continue;
      }
    };
    let fil_obj = match serde_json::from_str::<serde_json::Value>(fil_str)
      .ok()
      .and_then(|v| v.as_object().cloned())
    {
      Some(o) => o,
      None => {
        errors.push(format!(
          "Slot {} filament '{}' is not a valid JSON object",
          slot_idx + 1,
          fid
        ));
        continue;
      }
    };
    let fil_diff = &diff_groups[slot_idx + 1];
    for (k, v) in &fil_obj {
      if is_ignored_meta_key(k) {
        continue;
      }
      if !is_filament_scoped_key(k) {
        errors.push(format!(
          "Slot {} ({}) filament preset leaked non-filament key '{}' ({:?})",
          slot_idx + 1,
          fid,
          k,
          v
        ));
      }
      if !fil_diff.contains(k) {
        errors.push(format!(
          "Slot {} ({}) filament key '{}' is in standalone .json ({:?}) but missing from different_settings_to_system[{}]",
          slot_idx + 1,
          fid,
          k,
          v,
          slot_idx + 1
        ));
      }
      let disk_scalar = v
        .as_array()
        .and_then(|a| a.first())
        .and_then(|x| x.as_str())
        .or_else(|| v.as_str());
      let proj_scalar = proj_obj
        .get(k)
        .and_then(|pv| pv.as_array())
        .and_then(|a| a.get(slot_idx))
        .and_then(|x| x.as_str());
      if disk_scalar != proj_scalar || disk_scalar.is_none() {
        errors.push(format!(
          "Slot {} ({}) filament key '{}' mismatch: standalone .json={:?} vs 3mf project_settings[{}]={:?}",
          slot_idx + 1,
          fid,
          k,
          disk_scalar,
          slot_idx,
          proj_scalar
        ));
      }
    }
    for k in fil_diff {
      if is_ignored_meta_key(k) {
        continue;
      }
      let disk_scalar = fil_obj
        .get(k)
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|x| x.as_str())
        .or_else(|| fil_obj.get(k).and_then(|v| v.as_str()));
      let proj_scalar = proj_obj
        .get(k)
        .and_then(|pv| pv.as_array())
        .and_then(|a| a.get(slot_idx))
        .and_then(|x| x.as_str());
      if disk_scalar != proj_scalar || disk_scalar.is_none() {
        errors.push(format!(
          "Slot {} ({}) filament diff key '{}' mismatch: standalone .json={:?} vs 3mf project_settings[{}]={:?}",
          slot_idx + 1,
          fid,
          k,
          disk_scalar,
          slot_idx,
          proj_scalar
        ));
      }
    }
  }

  if errors.is_empty() {
    Ok(())
  } else {
    Err(errors)
  }
}

/// Verifies the full 3-way Provenance-to-Export Parity Invariant:
/// $$x_{\text{provenance\_shipped}} == x_{\text{JSON}} == x_{\text{3MF}}$$
/// together with continuous ledger chains ($x_0 \to \text{step}_1 \to \dots \to \text{Epilogue} = x_{\text{shipped}}$)
/// across both Process provenance and per-slot Filament provenance.
pub fn verify_provenance_matches_standalone_and_3mf_exports(
  process_provenance: &[crate::transformations::ParameterProvenance],
  slot_filament_provenance: &[Vec<crate::transformations::ParameterProvenance>],
  project_settings_json: &str,
  machine_json: &str,
  process_json: &str,
  filament_jsons_by_id: &std::collections::HashMap<String, String>,
) -> Result<(), Vec<String>> {
  let mut errors = Vec::new();
  if let Err(mut preset_errs) = verify_project_settings_matches_standalone_presets(
    project_settings_json,
    machine_json,
    process_json,
    filament_jsons_by_id,
  ) {
    errors.append(&mut preset_errs);
  }

  let Some(proj_obj) = serde_json::from_str::<serde_json::Value>(project_settings_json)
    .ok()
    .and_then(|v| v.as_object().cloned())
  else {
    errors.push("project_settings_json is not a valid JSON object".to_string());
    return Err(errors);
  };
  let Some(proc_obj) = serde_json::from_str::<serde_json::Value>(process_json)
    .ok()
    .and_then(|v| v.as_object().cloned())
  else {
    errors.push("process_json is not a valid JSON object".to_string());
    return Err(errors);
  };
  let slot_ids: Vec<String> = proj_obj
    .get("filament_settings_id")
    .and_then(|v| v.as_array())
    .map(|arr| {
      arr
        .iter()
        .filter_map(|x| x.as_str().map(|s| s.to_string()))
        .collect()
    })
    .unwrap_or_default();

  let scalars_equiv = |a: &str, b: &str| -> bool {
    let at = a.trim();
    let bt = b.trim();
    if at == bt {
      return true;
    }
    if let (Ok(an), Ok(bn)) = (
      at.trim_end_matches('%').parse::<f64>(),
      bt.trim_end_matches('%').parse::<f64>(),
    ) {
      if at.ends_with('%') == bt.ends_with('%') && (an - bn).abs() < 1e-4 {
        return true;
      }
    }
    false
  };

  // 1. Verify Process Provenance ledger chains and x_provenance == x_JSON == x_3MF
  for prov in process_provenance {
    if let Err(e) = prov.verify_ledger_chain() {
      errors.push(format!("Process provenance {}", e));
    }
    if prov.is_modified
      && !prov.steps.is_empty()
      && classify_key_destination(&prov.param_key) == Some(KeyDestination::Process)
      && !is_filament_scoped_key(&prov.param_key)
      && !is_machine_scoped_key(&prov.param_key)
    {
      if let Some(json_val) = proc_obj.get(&prov.param_key).and_then(|v| v.as_str()) {
        let shipped = prov.raw_final_scalar();
        if !scalars_equiv(&shipped, json_val) {
          errors.push(format!(
            "Process provenance parity mismatch on '{}': x_provenance='{}' vs x_JSON='{}'",
            prov.param_key, shipped, json_val
          ));
        }
      }
      if let Some(threemf_val) = proj_obj.get(&prov.param_key).and_then(|v| v.as_str()) {
        let shipped = prov.raw_final_scalar();
        if !scalars_equiv(&shipped, threemf_val) {
          errors.push(format!(
            "Process provenance parity mismatch on '{}': x_provenance='{}' vs x_3MF='{}'",
            prov.param_key, shipped, threemf_val
          ));
        }
      }
    }
  }

  // 2. Verify Per-Slot Filament Provenance ledger chains and x_provenance == x_JSON == x_3MF
  const CANONICAL_FILAMENT_PROVENANCE_KEYS: &[&str] = &[
    "nozzle_temperature",
    "nozzle_temperature_initial_layer",
    "filament_flow_ratio",
    "filament_retraction_length",
    "filament_retraction_speed",
    "filament_z_hop",
    "filament_z_hop_types",
    "pressure_advance",
    "filament_max_volumetric_speed",
    "fan_min_speed",
    "fan_max_speed",
    "hot_plate_temp",
    "hot_plate_temp_initial_layer",
    "textured_plate_temp",
    "textured_plate_temp_initial_layer",
    "cool_plate_temp",
    "cool_plate_temp_initial_layer",
    "eng_plate_temp",
    "eng_plate_temp_initial_layer",
    "textured_cool_plate_temp",
    "textured_cool_plate_temp_initial_layer",
    "supertack_plate_temp",
    "supertack_plate_temp_initial_layer",
  ];

  for (slot_idx, slot_provs) in slot_filament_provenance.iter().enumerate() {
    let Some(fid) = slot_ids.get(slot_idx) else {
      continue;
    };
    let Some(fil_str) = filament_jsons_by_id.get(fid) else {
      continue;
    };
    let Some(fil_obj) = serde_json::from_str::<serde_json::Value>(fil_str)
      .ok()
      .and_then(|v| v.as_object().cloned())
    else {
      continue;
    };
    for prov in slot_provs {
      if let Err(e) = prov.verify_ledger_chain() {
        errors.push(format!("Slot {} ({}) filament provenance {}", slot_idx + 1, fid, e));
      }
      if CANONICAL_FILAMENT_PROVENANCE_KEYS.contains(&prov.param_key.as_str()) {
        let shipped = prov.raw_final_scalar();
        if let Some(disk_scalar) = fil_obj
          .get(&prov.param_key)
          .and_then(|v| v.as_array())
          .and_then(|a| a.first())
          .and_then(|x| x.as_str())
          .or_else(|| fil_obj.get(&prov.param_key).and_then(|v| v.as_str()))
        {
          if !scalars_equiv(&shipped, disk_scalar) {
            errors.push(format!(
              "Slot {} ({}) filament provenance parity mismatch on '{}': x_provenance='{}' vs x_JSON='{}'",
              slot_idx + 1,
              fid,
              prov.param_key,
              shipped,
              disk_scalar
            ));
          }
        }
        if let Some(proj_scalar) = proj_obj
          .get(&prov.param_key)
          .and_then(|pv| pv.as_array())
          .and_then(|a| a.get(slot_idx))
          .and_then(|x| x.as_str())
        {
          if !scalars_equiv(&shipped, proj_scalar) {
            errors.push(format!(
              "Slot {} ({}) filament provenance parity mismatch on '{}': x_provenance='{}' vs x_3MF[{}]='{}'",
              slot_idx + 1,
              fid,
              prov.param_key,
              shipped,
              slot_idx,
              proj_scalar
            ));
          }
        }
      }
    }
  }

  if errors.is_empty() {
    Ok(())
  } else {
    Err(errors)
  }
}

/// Build a complete `.3mf` Project Container (ZIP archive) with `Metadata/project_settings.config`
/// and `Metadata/orcaslicer_webconfig_loadout.json`.
/// Note: Do NOT embed `Metadata/filament_settings_<k>.config`, `Metadata/process_settings_1.config`,
/// or `Metadata/machine_settings_1.config` inside `Metadata/`, because `BBS_3MF` extracts any extra
/// `Metadata/*.config` files and calls `PresetBundle::load_config_file_config`, which sets
/// `is_external = true` on existing user presets of the same name and causes `remove_project_presets()`
/// to purge them when the user clicks "New Project"!
pub fn build_3mf_project_container(
  rig: &MachineRig,
  process: &ProcessProfile,
  active_slots: &[FilamentSpool],
  flush_matrix: &[String],
  derived_overrides: &serde_json::Map<String, serde_json::Value>,
) -> Vec<u8> {
  let content_types_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/>
  <Default Extension="config" ContentType="application/json"/>
  <Default Extension="json" ContentType="application/json"/>
</Types>"#;

  let rels_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Target="/3D/3dmodel.model" Id="rel-1" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
</Relationships>"#;

  let model_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <metadata name="Application">BambuStudio-02.03.00.03</metadata>
  <resources/>
  <build/>
</model>"#;

  let project_settings_json =
    build_project_settings_config_json(rig, process, active_slots, flush_matrix, derived_overrides);

  let webconfig_loadout_json = serde_json::to_string_pretty(&serde_json::json!({
    "ace_pro_units": rig.ace_pro_units,
    "filament_material_type": active_slots.iter().map(|s| s.material_type.clone()).collect::<Vec<_>>(),
    "filament_finish_type": active_slots.iter().map(|s| s.finish_type.clone()).collect::<Vec<_>>(),
    "filament_opacity": active_slots.iter().map(|s| s.opacity).collect::<Vec<_>>(),
    "filament_pigment_stickiness": active_slots.iter().map(|s| s.pigment_stickiness).collect::<Vec<_>>(),
    "filament_drying_temp_c": active_slots.iter().map(|s| s.drying_temp_c).collect::<Vec<_>>(),
    "filament_drying_time_hr": active_slots.iter().map(|s| s.drying_time_hr).collect::<Vec<_>>()
  }))
  .unwrap_or_else(|_| "{}".to_string());

  let files: Vec<(&str, &[u8])> = vec![
    ("[Content_Types].xml", content_types_xml.as_bytes()),
    ("_rels/.rels", rels_xml.as_bytes()),
    ("3D/3dmodel.model", model_xml.as_bytes()),
    (
      "Metadata/project_settings.config",
      project_settings_json.as_bytes(),
    ),
    (
      "Metadata/orcaslicer_webconfig_loadout.json",
      webconfig_loadout_json.as_bytes(),
    ),
  ];

  build_zip_archive(&files)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportedAceLoadout {
  pub ace_pro_units: usize,
  pub spools: Vec<FilamentSpool>,
  pub flush_matrix: Option<Vec<String>>,
}

/// Parse an imported `.3mf` archive, `Metadata/project_settings.config` JSON, or `OrcaSlicer.conf`
/// to populate either a 4-slot Single ACE Pro or 8-slot Double ACE Pro 2 loadout!
pub fn parse_ace_loadout_from_bytes_or_json(data: &[u8]) -> Result<ImportedAceLoadout, String> {
  let (json_text, webconfig_meta_val) = if data.len() >= 4 && &data[0..4] == b"PK\x03\x04" {
    let ps = extract_file_from_zip(data, "Metadata/project_settings.config").ok_or_else(|| {
      "ZIP/.3mf archive does not contain Metadata/project_settings.config".to_string()
    })?;
    let meta = extract_file_from_zip(data, "Metadata/orcaslicer_webconfig_loadout.json")
      .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
    (ps, meta)
  } else {
    (
      std::str::from_utf8(data)
        .map_err(|e| format!("Invalid UTF-8 in config file: {}", e))?
        .to_string(),
      None,
    )
  };

  let val: serde_json::Value =
    serde_json::from_str(&json_text).map_err(|e| format!("Failed to parse JSON loadout: {}", e))?;

  // Check if this is `OrcaSlicer.conf` with a `"presets"` block or a `project_settings.config`
  let ids_arr = val
    .get("filament_settings_id")
    .and_then(|v| v.as_array())
    .or_else(|| {
      val
        .get("presets")
        .and_then(|p| p.get("filaments"))
        .and_then(|v| v.as_array())
    });

  let Some(ids) = ids_arr else {
    return Err("No 'filament_settings_id' or 'presets.filaments' array found in file".to_string());
  };

  let count = ids.len();
  if count == 0 {
    return Err("Filament array in config is empty".to_string());
  }

  let types = webconfig_meta_val
    .as_ref()
    .and_then(|m| m.get("filament_material_type"))
    .and_then(|v| v.as_array())
    .or_else(|| val.get("filament_type").and_then(|v| v.as_array()));
  // Machine identity for per-slot `inherits`: `printer_settings_id` carries the machine
  // preset title ("OC - Bambu Lab X1 Carbon • 0.4 Ruby-Brass" or a stock Orca name); match
  // a known printer model inside it (longest match wins so "Kobra S1 Max" beats "Kobra S1")
  // so imported spools stop claiming Anycubic/Kobra inherits on other vendors' files.
  let printer_settings_id = val
    .get("printer_settings_id")
    .and_then(|v| v.as_str())
    .map(|s| s.to_string())
    .or_else(|| {
      val
        .get("presets")
        .and_then(|p| p.get("machine"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
    })
    .unwrap_or_default();
  let imported_printer_model = {
    let id_lower = printer_settings_id.to_lowercase();
    // Machine titles use the rig's SHORT model tag ("OC - X1C • 0.4 Ruby-Brass"), stock Orca
    // names use the full model — match either, longest needle wins (so "Kobra S1 Max"
    // beats "Kobra S1").
    let short_tag_of = |model: &str| -> String {
      let trimmed = model
        .trim_start_matches("Anycubic ")
        .trim_start_matches("Bambu Lab ")
        .trim_start_matches("Creality ")
        .trim_start_matches("Prusa ")
        .trim();
      if trimmed == "X1 Carbon" {
        "X1C".to_string()
      } else {
        trimmed.to_string()
      }
    };
    let mut needles: Vec<(String, String)> = Vec::new();
    for m in &crate::constants::constants().machines {
      needles.push((m.printer_model.clone(), m.printer_model.clone()));
      needles.push((short_tag_of(&m.printer_model), m.printer_model.clone()));
    }
    for m in &crate::catalog::load_merged_catalog().machines {
      needles.push((m.name.clone(), m.name.clone()));
      needles.push((short_tag_of(&m.name), m.name.clone()));
    }
    needles
      .into_iter()
      .filter(|(needle, _)| !needle.is_empty() && id_lower.contains(&needle.to_lowercase()))
      .max_by_key(|(needle, _)| needle.len())
      .map(|(_, model)| model)
  };
  let imported_nozzle_diam = printer_settings_id
    .rfind("• ")
    .and_then(|idx| {
      printer_settings_id[idx + "• ".len()..]
        .split_whitespace()
        .next()
        .map(|s| s.to_string())
    })
    .unwrap_or_else(|| "0.4".to_string());
  let colors = val.get("filament_colour").and_then(|v| v.as_array());
  let temps = val.get("nozzle_temperature").and_then(|v| v.as_array());
  let bed_temps = val
    .get("textured_plate_temp")
    .or_else(|| val.get("hot_plate_temp"))
    .and_then(|v| v.as_array());
  let inherits_group = val.get("inherits_group").and_then(|v| v.as_array());
  let flows = val.get("filament_flow_ratio").and_then(|v| v.as_array());
  let pas = val.get("pressure_advance").and_then(|v| v.as_array());
  let mvs = val
    .get("filament_max_volumetric_speed")
    .and_then(|v| v.as_array());
  let finishes = webconfig_meta_val
    .as_ref()
    .and_then(|m| m.get("filament_finish_type"))
    .and_then(|v| v.as_array())
    .or_else(|| val.get("filament_finish_type").and_then(|v| v.as_array()));
  let opacities = webconfig_meta_val
    .as_ref()
    .and_then(|m| m.get("filament_opacity"))
    .and_then(|v| v.as_array())
    .or_else(|| val.get("filament_opacity").and_then(|v| v.as_array()));
  let stickiness = webconfig_meta_val
    .as_ref()
    .and_then(|m| m.get("filament_pigment_stickiness"))
    .and_then(|v| v.as_array())
    .or_else(|| {
      val
        .get("filament_pigment_stickiness")
        .and_then(|v| v.as_array())
    });

  let mut spools = Vec::new();
  for idx in 0..count {
    let name = ids
      .get(idx)
      .and_then(|v| v.as_str())
      .unwrap_or("Imported Spool")
      .to_string();
    let mat = types
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .unwrap_or("PLA")
      .to_string();
    let color_hex = colors
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .unwrap_or("#06B6D4")
      .to_string();
    let print_temp = temps
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<i32>().ok())
      .unwrap_or(215);
    let bed_temp = bed_temps
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<i32>().ok())
      .unwrap_or(60);
    let flow_ratio = flows
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<f32>().ok())
      .unwrap_or(0.98);
    let pressure_advance = pas
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<f32>().ok())
      .unwrap_or(0.032);
    let max_vol = mvs
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<f32>().ok())
      .unwrap_or(16.0);
    let finish_type = finishes
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .unwrap_or("Basic")
      .to_string();
    let opacity = opacities
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_f64())
      .map(|f| f as f32)
      .unwrap_or(1.0);
    let pigment_stickiness = stickiness
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_f64())
      .map(|f| f as f32)
      .unwrap_or(1.0);

    let c = crate::constants::constants();
    let detected_b = c.detect_brand_from_text(&name);
    let brand = if detected_b != "Generic" {
      detected_b.to_string()
    } else if name.starts_with("OC -") || name.starts_with("OC:") {
      "Generic".to_string()
    } else {
      "Anycubic".to_string()
    };

    let noz_cfg = c.lookup_nozzle(&name);
    let detected_noz = noz_cfg.id.clone();
    let poly_cfg = c.lookup_polymer(&mat);

    let hrcs = val.get("required_nozzle_HRC").and_then(|v| v.as_array());
    let req_hrc = hrcs
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<u32>().ok())
      .unwrap_or_else(|| {
        if FilamentSpool::is_abrasive_material_or_finish(&mat, &finish_type) {
          c.hardware_limits.abrasive_required_hrc
        } else {
          c.hardware_limits.non_abrasive_required_hrc
        }
      });

    let fan_mins = val.get("fan_min_speed").and_then(|v| v.as_array());
    let fan_maxs = val.get("fan_max_speed").and_then(|v| v.as_array());
    let primary_fans = if poly_cfg.is_copolyester() {
      fan_mins.or(fan_maxs)
    } else {
      fan_maxs.or(fan_mins)
    };
    let fan_pct = primary_fans
      .and_then(|a| a.get(idx))
      .and_then(|v| v.as_str())
      .and_then(|s| s.parse::<i32>().ok())
      .unwrap_or(poly_cfg.default_fan_pct);

    let brass_base = if noz_cfg.uses_hs_base_temp {
      (print_temp - 5).max(190)
    } else {
      print_temp - noz_cfg.temp_offset_from_brass_c
    };
    let hs_base = brass_base + 5;
    let mut sp = FilamentSpool {
      id: format!("imported_slot_{}", idx + 1),
      name,
      brand,
      inherits: String::new(),
      material_type: mat.clone(),
      nozzle_type: detected_noz.clone(),
      tuned_for_nozzle_type: detected_noz,
      required_nozzle_hrc: req_hrc,
      color_hex: color_hex.clone(),
      color_name: format!("Slot #{} ({})", idx + 1, color_hex),
      brass_base_temp: brass_base,
      hs_base_temp: hs_base,
      base_print_temp: brass_base,
      print_temp,
      base_bed_temp: bed_temp,
      bed_temp,
      flow_ratio,
      pressure_advance,
      max_volumetric_speed: max_vol,
      fan_speed_percent: fan_pct,
      remaining_grams: 850,
      is_loaded: idx < 8,
      finish_type,
      opacity,
      pigment_stickiness,
      drying_temp_c: poly_cfg.default_dry_temp_c,
      drying_time_hr: poly_cfg.default_dry_time_hr,
      is_rfid_synced: true,
      deltas_from_base: Vec::new(),
      raw_disk_json: None,
    };
    let grp_inh = inherits_group
      .filter(|a| 1 + idx + 1 < a.len())
      .and_then(|a| a.get(1 + idx))
      .and_then(|v| v.as_str())
      .map(|s| s.trim().to_string())
      .filter(|s| !s.is_empty());
    sp.inherits = if let Some(gi) = grp_inh {
      gi
    } else if let Some(ref model) = imported_printer_model {
      let mut stub_rig = MachineRig::kobra_s1_hardened_steel_png();
      stub_rig.printer_model = model.clone();
      stub_rig.nozzle_diameter = imported_nozzle_diam.clone();
      resolve_filament_inherits_for_rig(&sp, Some(&stub_rig))
    } else {
      resolve_kobra_s1_filament_inherits(&sp)
    };
    spools.push(sp);
  }

  let flush_matrix = val
    .get("flush_volumes_matrix")
    .and_then(|v| v.as_array())
    .map(|arr| {
      arr
        .iter()
        .map(|x| x.as_str().unwrap_or("0").to_string())
        .collect()
    });

  let ace_pro_units = if count > 4 { 2 } else { 1 };

  Ok(ImportedAceLoadout {
    ace_pro_units,
    spools,
    flush_matrix,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::dir_import::{build_reference_anycubic_entries, import_scanned_directory};

  #[test]
  fn test_3mf_roundtrip_and_8slot_double_ace_pro2_import() {
    let rig = MachineRig::kobra_s1_hardened_steel_png();
    let proc = ProcessProfile::get_default_processes().remove(0);
    let entries = build_reference_anycubic_entries();
    let mut spools = import_scanned_directory(&entries).unwrap().filaments;
    while spools.len() < 8 {
      let mut dup = spools[0].clone();
      dup.id = format!("extra_slot_{}", spools.len() + 1);
      spools.push(dup);
    }
    let spools: Vec<FilamentSpool> = spools.into_iter().take(8).collect();
    let dummy_matrix: Vec<String> = (0..64)
      .map(|i| {
        if i % 9 == 0 {
          "0".to_string()
        } else {
          "320".to_string()
        }
      })
      .collect();
    let overrides = serde_json::Map::new();

    let zip_bytes = build_3mf_project_container(&rig, &proc, &spools, &dummy_matrix, &overrides);
    assert_eq!(&zip_bytes[0..4], b"PK\x03\x04");

    let imported = parse_ace_loadout_from_bytes_or_json(&zip_bytes).unwrap();
    assert_eq!(imported.ace_pro_units, 2);
    assert_eq!(imported.spools.len(), 8);
    assert!(!imported.spools[0].name.is_empty());
    assert_eq!(imported.flush_matrix.unwrap().len(), 64);

    // Verify that NO extra Metadata/*_settings_*.config files are embedded (which would mark user presets as is_external=true)
    for slot_idx in 1..=8 {
      assert!(
        extract_file_from_zip(
          &zip_bytes,
          &format!("Metadata/filament_settings_{}.config", slot_idx)
        )
        .is_none(),
        "Metadata/filament_settings_{}.config must not be embedded in .3mf",
        slot_idx
      );
      let fil_cfg = export_standalone_filament_json(&spools[slot_idx - 1], &rig);
      let fil_val: serde_json::Value = serde_json::from_str(&fil_cfg).unwrap();
      let fil_obj = fil_val.as_object().unwrap();
      assert!(
        !fil_obj.contains_key("filament_colour"),
        "Standalone filament preset must not contain filament_colour"
      );
      assert!(fil_obj.contains_key("nozzle_temperature_BRASS"));
      assert!(fil_obj.contains_key("nozzle_temperature_HS"));
      assert!(fil_obj.contains_key("fan_max_speed_BRASS"));
      assert!(fil_obj.contains_key("fan_max_speed_HS"));
      for (k, v) in fil_obj {
        assert!(
          v.is_string() || (v.is_array() && v.as_array().unwrap().iter().all(|x| x.is_string())),
          "Non-string/string-array key {} in standalone filament json: {:?}",
          k,
          v
        );
      }
    }
    assert!(extract_file_from_zip(&zip_bytes, "Metadata/machine_settings_1.config").is_none());
    assert!(extract_file_from_zip(&zip_bytes, "Metadata/process_settings_1.config").is_none());

    let mut noisy_overrides = serde_json::Map::new();
    noisy_overrides.insert(
      "fan_cooling_layer_time".to_string(),
      serde_json::json!(["30"]),
    );
    noisy_overrides.insert("z_hop_types".to_string(), serde_json::json!(["Slope"]));
    let ps_json =
      build_project_settings_config_json(&rig, &proc, &spools, &dummy_matrix, &noisy_overrides);
    let ps_val: serde_json::Value = serde_json::from_str(&ps_json).unwrap();
    let ps_map = ps_val.as_object().unwrap();
    assert!(
      !ps_map.contains_key("fan_cooling_layer_time"),
      "1-element filament array from base_profile must never leak into project_settings.config"
    );
    assert_eq!(ps_map["nozzle_diameter"], serde_json::json!(["0.4"]));
    assert_eq!(ps_map["printer_variant"], serde_json::json!("0.4"));
    assert_eq!(ps_map["outer_wall_line_width"], serde_json::json!("0.42"));
    assert_eq!(ps_map["wall_generator"], serde_json::json!("arachne"));
    assert_eq!(ps_map["seam_slope_type"], serde_json::json!("external"));
    assert_eq!(ps_map["seam_slope_steps"], serde_json::json!("15"));
    assert_eq!(ps_map["seam_slope_min_length"], serde_json::json!("15"));
    assert_eq!(ps_map["sparse_infill_pattern"], serde_json::json!("gyroid"));
    assert_eq!(ps_map["filament_diameter"].as_array().unwrap().len(), 8);
    assert_eq!(ps_map["default_filament_colour"], ps_map["filament_colour"]);
    assert_eq!(
      ps_map["nozzle_temperature_BRASS"],
      ps_map["nozzle_temperature"]
    );
    assert_eq!(
      ps_map["nozzle_temperature_HS"],
      ps_map["nozzle_temperature"]
    );
    assert_eq!(ps_map["fan_max_speed_BRASS"], ps_map["fan_max_speed"]);
    assert_eq!(ps_map["fan_max_speed_HS"], ps_map["fan_max_speed"]);
    assert_eq!(ps_map["inherits_group"].as_array().unwrap().len(), 10);

    let model_xml = extract_file_from_zip(&zip_bytes, "3D/3dmodel.model").unwrap();
    assert!(model_xml.contains("BambuStudio-02.03.00.03"));

    assert_eq!(
      ps_map["different_settings_to_system"]
        .as_array()
        .unwrap()
        .len(),
      10
    );
    for (k, v) in ps_map {
      assert!(
        v.is_string() || (v.is_array() && v.as_array().unwrap().iter().all(|x| x.is_string())),
        "Key {} in project_settings.config is not string or Vec<String>: {:?}",
        k,
        v
      );
    }

    let mach_json = build_machine_preset_json(&rig);
    let mach_val: serde_json::Value = serde_json::from_str(&mach_json).unwrap();
    assert_eq!(mach_val["version"], serde_json::json!("0.1.0.0"));
    assert_eq!(mach_val["nozzle_hrc"], serde_json::json!("55"));
    assert!(mach_val["printer_notes"]
      .as_str()
      .unwrap()
      .contains("[OrcaSlicer WebConfig v0.1.0 — Machine]"));

    // Verify all 5 default OC process presets export their exact configured speeds without spurious clamping or leaked internal_solid_infill_speed
    let mut tropical_overrides = serde_json::Map::new();
    tropical_overrides.insert("seam_slope_type".to_string(), serde_json::json!("all"));
    tropical_overrides.insert("outer_wall_speed".to_string(), serde_json::json!("151"));
    tropical_overrides.insert("inner_wall_speed".to_string(), serde_json::json!("151"));
    tropical_overrides.insert("sparse_infill_speed".to_string(), serde_json::json!("151"));
    tropical_overrides.insert(
      "internal_solid_infill_speed".to_string(),
      serde_json::json!("213"),
    );
    tropical_overrides.insert("initial_layer_speed".to_string(), serde_json::json!("18"));
    let expected_procs = [
      (
        "OC - 0.20mm Standard @Kobra S1",
        "0.20",
        "200",
        "300",
        "270",
        "50",
        "10000",
      ),
      (
        "OC - 0.20mm Quality @Kobra S1",
        "0.20",
        "60",
        "150",
        "250",
        "50",
        "5000",
      ),
      (
        "OC - 0.20mm Speed @Kobra S1",
        "0.20",
        "300",
        "350",
        "350",
        "60",
        "15000",
      ),
      (
        "OC - 0.12mm Quality @Kobra S1",
        "0.12",
        "60",
        "150",
        "200",
        "50",
        "5000",
      ),
      (
        "OC - 0.28mm Speed @Kobra S1",
        "0.28",
        "250",
        "320",
        "320",
        "60",
        "12000",
      ),
    ];
    for (p, (exp_name, exp_lh, exp_ow, exp_iw, exp_si, exp_il, exp_acc)) in
      ProcessProfile::get_default_processes()
        .iter()
        .zip(expected_procs.iter())
    {
      assert_eq!(&p.name, exp_name);
      let pj = build_process_preset_json(&p, &rig, &tropical_overrides);
      let pv: serde_json::Value = serde_json::from_str(&pj).unwrap();
      assert_eq!(pv["layer_height"], serde_json::json!(*exp_lh));
      assert_eq!(pv["outer_wall_speed"], serde_json::json!(*exp_ow));
      assert_eq!(pv["inner_wall_speed"], serde_json::json!(*exp_iw));
      assert_eq!(pv["sparse_infill_speed"], serde_json::json!(*exp_si));
      assert_eq!(pv["initial_layer_speed"], serde_json::json!(*exp_il));
      assert_eq!(pv["default_acceleration"], serde_json::json!(*exp_acc));
      assert!(
        pv.get("internal_solid_infill_speed").is_none(),
        "Process {} must not leak internal_solid_infill_speed from derived_overrides",
        exp_name
      );
    }
  }

  #[test]
  fn test_tpu_for_ams_and_petg_temp_range_and_brand_preserving_and_pha_export() {
    let catalog = crate::catalog::load_merged_catalog();
    // 1. Verify PLA is the very first filament in Generic (never PHA)
    let first_generic = catalog
      .filaments
      .iter()
      .find(|f| f.brand == "Generic")
      .expect("Generic brand in catalog");
    assert_eq!(first_generic.code, "PLA");

    // 2. Verify selecting PETG and switching across all brands keeps PETG (never resets to first filament of brand)
    let petg_entry = catalog
      .filaments
      .iter()
      .find(|f| f.brand == "Generic" && f.code == "PETG")
      .expect("Generic PETG in catalog");
    let mut spool = FilamentSpool::from_catalog_entry(1, petg_entry, "hardened_steel");
    assert_eq!(
      FilamentSpool::canonical_polymer_key(&spool.material_type),
      "PETG"
    );

    for brand in [
      "SUNLU",
      "Polymaker",
      "Anycubic",
      "Bambu Lab",
      "eSUN",
      "Overture",
      "Siraya Tech",
      "Generic",
    ] {
      spool.apply_brand_change(brand, "hardened_steel");
      assert_eq!(
        spool.brand, brand,
        "Brand should update to '{}' when switching brand on PETG",
        brand
      );
      assert_eq!(
        FilamentSpool::canonical_polymer_key(&spool.material_type),
        "PETG",
        "Switching brand to '{}' must preserve PETG polymer (got '{}')",
        brand,
        spool.material_type
      );
    }

    // 3. Verify TPU for AMS + PETG exported presets & .3mf project_settings.config have mutually compatible nozzle_temperature_range_low/high
    let tpu_ams_entry = catalog
      .filaments
      .iter()
      .find(|f| f.code == "TPU for AMS")
      .expect("TPU for AMS in catalog");
    let tpu_ams_spool = FilamentSpool::from_catalog_entry(1, tpu_ams_entry, "hardened_steel");
    let petg_spool = FilamentSpool::from_catalog_entry(2, petg_entry, "hardened_steel");
    let rig = MachineRig::kobra_s1_hardened_steel_png();

    let tpu_json: serde_json::Value =
      serde_json::from_str(&export_standalone_filament_json(&tpu_ams_spool, &rig)).unwrap();
    let petg_json: serde_json::Value =
      serde_json::from_str(&export_standalone_filament_json(&petg_spool, &rig)).unwrap();

    let tpu_temp: i32 = tpu_json["nozzle_temperature"][0]
      .as_str()
      .unwrap()
      .parse()
      .unwrap();
    let petg_temp: i32 = petg_json["nozzle_temperature"][0]
      .as_str()
      .unwrap()
      .parse()
      .unwrap();
    let tpu_low: i32 = tpu_json["nozzle_temperature_range_low"][0]
      .as_str()
      .unwrap()
      .parse()
      .unwrap();
    let tpu_high: i32 = tpu_json["nozzle_temperature_range_high"][0]
      .as_str()
      .unwrap()
      .parse()
      .unwrap();
    let petg_low: i32 = petg_json["nozzle_temperature_range_low"][0]
      .as_str()
      .unwrap()
      .parse()
      .unwrap();
    let petg_high: i32 = petg_json["nozzle_temperature_range_high"][0]
      .as_str()
      .unwrap()
      .parse()
      .unwrap();

    assert!(
      tpu_temp >= petg_low && tpu_temp <= petg_high,
      "TPU for AMS temp ({}°C) must be inside PETG range [{}..{}°C]",
      tpu_temp,
      petg_low,
      petg_high
    );
    assert!(
      petg_temp >= tpu_low && petg_temp <= tpu_high,
      "PETG temp ({}°C) must be inside TPU for AMS range [{}..{}°C]",
      petg_temp,
      tpu_low,
      tpu_high
    );

    // 4. Verify importing OC - PHA • 0.4 Hardened.json produces zero conflicts and zero safety warnings
    let pha_entry = catalog
      .filaments
      .iter()
      .find(|f| f.code == "PHA")
      .expect("PHA in catalog");
    let pha_spool = FilamentSpool::from_catalog_entry(1, pha_entry, "hardened_steel");
    let pha_disk_json = export_standalone_filament_json(&pha_spool, &rig);
    let scanned = vec![crate::fs_access::ScannedFileEntry {
      relative_path: "user/434139/filament/OC - PHA • 0.4 Hardened.json".to_string(),
      filename: "OC - PHA • 0.4 Hardened.json".to_string(),
      bytes: pha_disk_json.into_bytes(),
    }];
    let imported = crate::dir_import::import_scanned_directory(&scanned).unwrap();
    assert!(
      imported.conflicts.is_empty(),
      "OC - PHA • 0.4 Hardened must not trigger any external conflicts: {:?}",
      imported.conflicts
    );
    let warnings = crate::transformations::TransformationEngine::evaluate_safety_warnings(
      &rig,
      &imported.filaments,
      &imported.base_profile,
      &imported.conflicts,
      &imported.disk_flush_matrix,
    );
    assert!(
      warnings.is_empty(),
      "OC - PHA • 0.4 Hardened must not trigger any safety warnings at export: {:?}",
      warnings
    );
  }

  #[test]
  fn test_per_process_lua_layer_height_and_speed_scaling_across_all_5_presets() {
    let rig = MachineRig::kobra_s1_hardened_steel_png();
    let procs = ProcessProfile::get_default_processes();
    let active_proc = &procs[0]; // 0.20mm Standard @Kobra S1 (layer_height=0.20, outer_wall_speed=200, sparse_infill_speed=270)

    let mut base = crate::orca_profile::OrcaProfile::get_stock_kobra_s1();
    base.set_str("layer_height", &format!("{:.2}", active_proc.layer_height));
    base.set_str(
      "outer_wall_speed",
      &format!("{:.0}", active_proc.outer_wall_speed),
    );
    base.set_str(
      "inner_wall_speed",
      &format!("{:.0}", active_proc.inner_wall_speed),
    );
    base.set_str(
      "sparse_infill_speed",
      &format!("{:.0}", active_proc.sparse_infill_speed),
    );

    let mut rules = crate::transformations::TransformationEngine::get_default_rules();
    rules.push(crate::transformations::TransformationRule {
      id: "rule_test_support_and_speed".to_string(),
      name: "Test Support Z-Gap & Speed Scaling".to_string(),
      description: "Scales outer_wall_speed by 0.75x, sparse_infill_speed by 0.80x, and sets support_top_z_distance = layer_height + 0.04".to_string(),
      category: "Process".to_string(),
      icon: "tune".to_string(),
      enabled: true,
      priority: 95,
      scope: crate::transformations::RuleScopeFilter::default(),
      condition_expr: "true".to_string(),
      lua_code: r#"
function apply(config, ctx)
  local lh = tonumber(config["layer_height"]) or 0.20
  config["support_top_z_distance"] = string.format("%.2f", lh + 0.04)
  config["top_shell_thickness"] = string.format("%.2f", lh * 6.0)
  local ow = tonumber(config["outer_wall_speed"]) or 200
  config["outer_wall_speed"] = tostring(math.floor(ow * 0.75 + 0.5))
  local si = tonumber(config["sparse_infill_speed"]) or 270
  config["sparse_infill_speed"] = tostring(math.floor(si * 0.80 + 0.5))
end
"#.to_string(),
    });

    let ctx = serde_json::json!({
      "printer_model": rig.printer_model,
      "nozzle_type": rig.nozzle_type,
      "nozzle_material": rig.nozzle_material,
      "nozzle_diameter": rig.nozzle_diameter,
      "hotend_max_flow": rig.max_volumetric_flow_limit,
      "filament_max_flow": 18.0,
      "layer_height": active_proc.layer_height,
      "line_width": active_proc.line_width,
    });

    let eval_res =
      crate::transformations::TransformationEngine::evaluate_stack(&base, &rules, &ctx);
    let derived_overrides = &eval_res.resulting_profile.values;

    // 1. 0.20mm Standard (base outer=200 -> 150, base infill=270 -> 216, lh=0.20 -> zd=0.24, top_thick=1.20)
    let p_020_std: serde_json::Value = serde_json::from_str(&build_process_preset_json(
      &procs[0],
      &rig,
      derived_overrides,
    ))
    .unwrap();
    assert_eq!(
      p_020_std["support_top_z_distance"],
      serde_json::json!("0.24")
    );
    assert_eq!(p_020_std["top_shell_thickness"], serde_json::json!("1.20"));
    assert_eq!(p_020_std["outer_wall_speed"], serde_json::json!("150"));
    assert_eq!(p_020_std["sparse_infill_speed"], serde_json::json!("216"));

    // 2. 0.12mm Quality (base outer=60 -> 45, base infill=200 -> 160, lh=0.12 -> zd=0.16, top_thick=0.72)
    let p_012_hq: serde_json::Value = serde_json::from_str(&build_process_preset_json(
      &procs[3],
      &rig,
      derived_overrides,
    ))
    .unwrap();
    assert_eq!(
      p_012_hq["support_top_z_distance"],
      serde_json::json!("0.16")
    );
    assert_eq!(p_012_hq["top_shell_thickness"], serde_json::json!("0.72"));
    assert_eq!(p_012_hq["outer_wall_speed"], serde_json::json!("45"));
    assert_eq!(p_012_hq["sparse_infill_speed"], serde_json::json!("160"));

    // 3. 0.28mm Speed (base outer=250 -> 188, base infill=320 -> 256, lh=0.28 -> zd=0.32, top_thick=1.68)
    let p_028_spd: serde_json::Value = serde_json::from_str(&build_process_preset_json(
      &procs[4],
      &rig,
      derived_overrides,
    ))
    .unwrap();
    assert_eq!(
      p_028_spd["support_top_z_distance"],
      serde_json::json!("0.32")
    );
    assert_eq!(p_028_spd["top_shell_thickness"], serde_json::json!("1.68"));
    assert_eq!(p_028_spd["outer_wall_speed"], serde_json::json!("188"));
    assert_eq!(p_028_spd["sparse_infill_speed"], serde_json::json!("256"));
  }

  #[test]
  fn test_filament_process_machine_scope_isolation_and_heterogeneous_slots() {
    let catalog = crate::catalog::load_merged_catalog();
    let mut rig = MachineRig::kobra_s1_hardened_steel_png();
    rig.nozzle_type = "diamondback".to_string();
    rig.nozzle_material = "diamond".to_string();
    rig.hotend = "FlowTech (All-Metal)".to_string();
    rig.melt_zone_multiplier = 1.35;

    let proc = ProcessProfile::get_default_processes().remove(0);

    // Build a heterogeneous 4-slot ACE Pro loadout: PLA, PETG, ASA, TPU for AMS
    let mut spools = Vec::new();
    for (idx, code) in ["PLA", "PETG", "ASA", "TPU for AMS"].iter().enumerate() {
      let entry = catalog
        .filaments
        .iter()
        .find(|f| f.code == *code)
        .unwrap_or_else(|| panic!("Missing {} in catalog", code));
      let mut sp = FilamentSpool::from_catalog_entry(idx + 1, entry, "diamondback");
      sp.id = format!("slot_{}", idx + 1);
      spools.push(sp);
    }

    // Simulate a Lua evaluation output containing mixed Process, Machine, and Filament keys
    // (including `overhang_fan_speed` from `bridge_overhang_tuning` and `nozzle_temperature` /
    // `filament_retraction_length` from `thermal_conductivity` + `symptom_stringing_diamondback`).
    let mut mixed_overrides = serde_json::Map::new();
    mixed_overrides.insert("nozzle_temperature".to_string(), serde_json::json!(["202"]));
    mixed_overrides.insert(
      "nozzle_temperature_initial_layer".to_string(),
      serde_json::json!(["212"]),
    );
    mixed_overrides.insert("hot_plate_temp".to_string(), serde_json::json!(["60"]));
    mixed_overrides.insert("chamber_temperature".to_string(), serde_json::json!(["45"]));
    mixed_overrides.insert(
      "filament_retraction_length".to_string(),
      serde_json::json!(["0.50"]),
    );
    mixed_overrides.insert(
      "filament_retraction_speed".to_string(),
      serde_json::json!(["44"]),
    );
    mixed_overrides.insert("filament_z_hop".to_string(), serde_json::json!(["0.15"]));
    mixed_overrides.insert("overhang_fan_speed".to_string(), serde_json::json!("100"));
    mixed_overrides.insert("bridge_flow".to_string(), serde_json::json!("0.92"));
    mixed_overrides.insert("bridge_speed".to_string(), serde_json::json!("28"));
    mixed_overrides.insert("overhang_4_4_speed".to_string(), serde_json::json!("15"));
    mixed_overrides.insert(
      "_lua_touched_keys".to_string(),
      serde_json::json!(
        "nozzle_temperature;filament_retraction_length;overhang_fan_speed;bridge_flow;bridge_speed;overhang_4_4_speed"
      ),
    );

    let proc_json = build_process_preset_json(&proc, &rig, &mixed_overrides);
    let proc_val: serde_json::Value = serde_json::from_str(&proc_json).unwrap();
    let proc_obj = proc_val.as_object().unwrap();

    // 1. Process preset keeps true Process keys (`bridge_flow`, `bridge_speed`, `overhang_4_4_speed`)
    //    and rejects ALL Filament-scoped and Machine-scoped keys (`overhang_fan_speed`, `nozzle_temperature`, etc.)
    assert_eq!(proc_obj.get("bridge_flow"), Some(&serde_json::json!("0.92")));
    assert_eq!(proc_obj.get("bridge_speed"), Some(&serde_json::json!("28")));
    assert_eq!(
      proc_obj.get("overhang_4_4_speed"),
      Some(&serde_json::json!("15"))
    );
    for k in proc_obj.keys() {
      assert!(
        !is_filament_scoped_key(k),
        "Process preset must NEVER contain filament-scoped key '{}'",
        k
      );
      assert!(
        !is_machine_scoped_key(k),
        "Process preset must NEVER contain machine-scoped key '{}'",
        k
      );
    }

    // 2. Machine preset contains zero Filament-scoped keys
    let mach_json = build_machine_preset_json_with_overrides(&rig, Some(&mixed_overrides));
    let mach_val: serde_json::Value = serde_json::from_str(&mach_json).unwrap();
    let mach_obj = mach_val.as_object().unwrap();
    for k in mach_obj.keys() {
      assert!(
        !is_filament_scoped_key(k),
        "Machine preset must NEVER contain filament-scoped key '{}'",
        k
      );
    }

    // 3. 3MF `project_settings.config` evaluates each slot independently via the per-spool Lua pipeline
    //    without Slot 1's PLA `nozzle_temperature` (202°C) overwriting Slot 2 (PETG) or Slot 3 (ASA)!
    let flush_matrix = vec!["0".to_string(); 16];
    let proj_json =
      build_project_settings_config_json(&rig, &proc, &spools, &flush_matrix, &mixed_overrides);
    let proj_val: serde_json::Value = serde_json::from_str(&proj_json).unwrap();
    let noz_temps = proj_val["nozzle_temperature"].as_array().unwrap();
    assert_eq!(noz_temps.len(), 4);
    let pla_t: i32 = noz_temps[0].as_str().unwrap().parse().unwrap();
    let petg_t: i32 = noz_temps[1].as_str().unwrap().parse().unwrap();
    let asa_t: i32 = noz_temps[2].as_str().unwrap().parse().unwrap();
    assert!(
      pla_t < petg_t && petg_t < asa_t,
      "Expected independent per-slot DiamondBack temperatures PLA ({}°C) < PETG ({}°C) < ASA ({}°C)",
      pla_t,
      petg_t,
      asa_t
    );
  }

  #[test]
  fn test_lua_bed_adhesion_transformations_are_not_overwritten_by_spool_default() {
    let cat = crate::catalog::load_merged_catalog();
    let petg_entry = cat
      .filaments
      .iter()
      .find(|f| f.brand == "Anycubic" && f.code == "PETG")
      .unwrap();
    let spool = FilamentSpool::from_catalog_entry(1, petg_entry, "microswiss_diamondback");
    let mut rig = MachineRig::kobra_s1_hardened_steel_png();
    rig.nozzle_type = "microswiss_diamondback".to_string();
    rig.climate_preset = "temperate".to_string();
    rig.ambient_temp_c = 22;
    rig.humidity_pct = 45;

    let base_profile = crate::orca_profile::OrcaProfile::get_stock_kobra_s1();
    let mut params = std::collections::HashMap::new();
    params.insert("bed_boost".to_string(), "+5".to_string());
    params.insert("first_layer_spd".to_string(), "-30%".to_string());
    params.insert("fan_off_layers".to_string(), "+2".to_string());
    params.insert("brim_type".to_string(), "outer_only".to_string());
    let rule = crate::transformations::TransformationRule {
      id: "sym_warping_adhesion".to_string(),
      name: "Bed Adhesion".to_string(),
      description: String::new(),
      category: "Symptom Fix".to_string(),
      icon: "🧲".to_string(),
      enabled: true,
      condition_expr: "all".to_string(),
      lua_code: crate::lua_engine::embed_param_defaults_in_lua(
        include_str!("../transformations/parametric/sym_warping_adhesion.lua"),
        &params,
      ),
      priority: 50,
      scope: crate::transformations::RuleScopeFilter::default(),
    };
    let eval = crate::transformations::TransformationEngine::evaluate_stack(
      &base_profile,
      &[rule],
      &serde_json::json!({"nozzle_diameter": 0.4}),
    );
    let exported: serde_json::Value = serde_json::from_str(
      &export_standalone_filament_json_with_overrides(
        &spool,
        &rig,
        &eval.resulting_profile.values,
      ),
    )
    .unwrap();
    let expected_bed = (spool.bed_temp + 5).to_string();
    assert_eq!(
      exported["textured_plate_temp"][0].as_str(),
      Some(expected_bed.as_str()),
      "Lua sym_warping_adhesion +5°C boost must stack onto spool.bed_temp instead of being overwritten by default"
    );
    assert_eq!(
      exported["textured_plate_temp_initial_layer"][0].as_str(),
      Some(expected_bed.as_str()),
    );
  }

  #[test]
  fn test_kobra_s1_diamondback_flowtech_anycubic_abs_exact_schedule() {
    let mut rig = MachineRig::get_template_catalog()
      .into_iter()
      .find(|r| r.nozzle_type == "diamondback")
      .expect("Kobra S1 Diamondback + FlowTech template rig");
    rig.climate_preset = "tropical".to_string();
    rig.ambient_temp_c = 30;
    rig.humidity_pct = 80;
    let catalog = crate::catalog::load_merged_catalog();
    let abs_entry = catalog
      .filaments
      .iter()
      .find(|f| f.brand == "Anycubic" && f.code == "ABS")
      .expect("Anycubic ABS catalog entry");
    let spool = FilamentSpool::from_catalog_entry(1, abs_entry, &rig.nozzle_type);

    // Case A: User's active Tropical (30°C / 80% RH) + Wet Filament stack
    let mut wet_overrides = serde_json::Map::new();
    wet_overrides.insert("_wet_filament_rh".to_string(), serde_json::json!("65"));
    wet_overrides.insert(
      "_wet_filament_temp_drop_c".to_string(),
      serde_json::json!("-3"),
    );
    wet_overrides.insert(
      "_wet_filament_init_bed_boost_c".to_string(),
      serde_json::json!("4"),
    );
    wet_overrides.insert(
      "_wet_filament_flow_trim".to_string(),
      serde_json::json!("-0.01"),
    );
    wet_overrides.insert(
      "_wet_filament_retract_add_mm".to_string(),
      serde_json::json!("0.10"),
    );
    wet_overrides.insert(
      "_wet_filament_retract_before_wipe_pct".to_string(),
      serde_json::json!("45"),
    );

    let exported: serde_json::Value = serde_json::from_str(
      &export_standalone_filament_json_with_overrides(&spool, &rig, &wet_overrides),
    )
    .unwrap();
    eprintln!(
      "EXACT KOBRA S1 + DIAMONDBACK + FLOWTECH + TROPICAL + WET ANYCUBIC ABS:\n{}",
      serde_json::to_string_pretty(&exported).unwrap()
    );

    // 1. Nozzle schedule: Anycubic ABS Brass base 260°C / 265°C initial -> Diamondback PCD (-8.0°C / -7.5°C) + Wet (-3°C cont) -> 249°C / 258°C
    assert_eq!(exported["nozzle_temperature"][0].as_str(), Some("249"));
    assert_eq!(
      exported["nozzle_temperature_initial_layer"][0].as_str(),
      Some("258")
    );
    assert_eq!(exported["nozzle_temperature_range_low"][0].as_str(), Some("230"));
    assert_eq!(exported["nozzle_temperature_range_high"][0].as_str(), Some("285"));

    // 2. Six-plate schedule: Textured PEI transformed to 115°C initial / 110°C continuous (NOT stuck at 110/110!),
    //    Smooth PEI 105°C initial / 100°C continuous, Cool Plate & Textured Cool Plate 0°C (unsupported on ABS so
    //    selecting a Cool Plate never prints ABS at 35°C!), Engineering Plate 0°C (matching Anycubic Kobra S1).
    assert_eq!(exported["textured_plate_temp"][0].as_str(), Some("110"));
    assert_eq!(
      exported["textured_plate_temp_initial_layer"][0].as_str(),
      Some("115")
    );
    assert_eq!(exported["hot_plate_temp"][0].as_str(), Some("100"));
    assert_eq!(
      exported["hot_plate_temp_initial_layer"][0].as_str(),
      Some("105")
    );
    assert_eq!(exported["eng_plate_temp"][0].as_str(), Some("0"));
    assert_eq!(exported["eng_plate_temp_initial_layer"][0].as_str(), Some("0"));
    assert_eq!(exported["cool_plate_temp"][0].as_str(), Some("0"));
    assert_eq!(
      exported["cool_plate_temp_initial_layer"][0].as_str(),
      Some("0")
    );
    assert_eq!(exported["textured_cool_plate_temp"][0].as_str(), Some("0"));
    assert_eq!(
      exported["textured_cool_plate_temp_initial_layer"][0].as_str(),
      Some("0")
    );

    // 3. Chamber heat retention & warp-safe overhang/part cooling
    assert_eq!(exported["during_print_exhaust_fan_speed"][0].as_str(), Some("0"));
    assert_eq!(exported["additional_cooling_fan_speed"][0].as_str(), Some("0"));
    assert_eq!(exported["fan_min_speed"][0].as_str(), Some("10"));
    assert_eq!(exported["fan_max_speed"][0].as_str(), Some("17"));
    assert_eq!(exported["overhang_fan_speed"][0].as_str(), Some("35"));
    assert_eq!(exported["close_fan_the_first_x_layers"][0].as_str(), Some("3"));
    assert_eq!(
      exported["inherits"].as_str(),
      Some("Anycubic ABS @Anycubic Kobra S1 0.4 nozzle")
    );
  }

  #[test]
  fn test_kobra_s1_diamondback_tropical_wet_anycubic_pla_cf_and_process_adhesion() {
    let mut rig = MachineRig::get_template_catalog()
      .into_iter()
      .find(|r| r.nozzle_type == "diamondback")
      .expect("Kobra S1 Diamondback + FlowTech template rig");
    rig.addons = vec!["flowtech_hotend".to_string()];
    rig.climate_preset = "tropical".to_string();
    rig.ambient_temp_c = 30;
    rig.humidity_pct = 80;
    let catalog = crate::catalog::load_merged_catalog();
    let pla_cf_entry = catalog
      .filaments
      .iter()
      .find(|f| f.brand == "Anycubic" && f.code == "PLA-CF")
      .expect("Anycubic PLA-CF catalog entry");
    let spool = FilamentSpool::from_catalog_entry(4, pla_cf_entry, &rig.nozzle_type);

    let mut wet_overrides = serde_json::Map::new();
    wet_overrides.insert("_wet_filament_rh".to_string(), serde_json::json!("65"));
    wet_overrides.insert(
      "_wet_filament_temp_drop_c".to_string(),
      serde_json::json!("-3"),
    );
    wet_overrides.insert(
      "_wet_filament_init_bed_boost_c".to_string(),
      serde_json::json!("4"),
    );
    wet_overrides.insert(
      "_wet_filament_flow_trim".to_string(),
      serde_json::json!("-0.01"),
    );
    wet_overrides.insert(
      "_wet_filament_retract_add_mm".to_string(),
      serde_json::json!("0.10"),
    );
    wet_overrides.insert(
      "_wet_filament_retract_before_wipe_pct".to_string(),
      serde_json::json!("45"),
    );
    wet_overrides.insert(
      "during_print_exhaust_fan_speed".to_string(),
      serde_json::json!("90"),
    );
    wet_overrides.insert(
      "additional_cooling_fan_speed".to_string(),
      serde_json::json!("90"),
    );
    wet_overrides.insert(
      "_lua_touched_keys".to_string(),
      serde_json::json!("during_print_exhaust_fan_speed;additional_cooling_fan_speed"),
    );

    let exported: serde_json::Value = serde_json::from_str(
      &export_standalone_filament_json_with_overrides(&spool, &rig, &wet_overrides),
    )
    .unwrap();
    eprintln!(
      "EXACT KOBRA S1 + DIAMONDBACK + FLOWTECH + TROPICAL + WET ANYCUBIC PLA-CF:\n{}",
      serde_json::to_string_pretty(&exported).unwrap()
    );

    // 1. Textured & Smooth PEI preserve Anycubic PLA-CF's 65°C continuous baseline + 69°C initial layer
    //    (NEVER cut down to 60°C by Tropical neat-PLA bed drop!).
    assert_eq!(exported["textured_plate_temp"][0].as_str(), Some("65"));
    assert_eq!(
      exported["textured_plate_temp_initial_layer"][0].as_str(),
      Some("69")
    );
    assert_eq!(exported["hot_plate_temp"][0].as_str(), Some("65"));
    assert_eq!(
      exported["hot_plate_temp_initial_layer"][0].as_str(),
      Some("69")
    );

    // 2. Nozzle temperature stays >= 208°C continuous / >= 218°C initial on Diamondback PCD
    //    (NEVER crushed down to 204°C by triple-stacked cooling!).
    let cont_noz: i32 = exported["nozzle_temperature"][0]
      .as_str()
      .unwrap()
      .parse()
      .unwrap();
    let init_noz: i32 = exported["nozzle_temperature_initial_layer"][0]
      .as_str()
      .unwrap()
      .parse()
      .unwrap();
    assert!(
      cont_noz >= 208,
      "Expected PLA-CF continuous nozzle >= 208°C on PCD, got {}°C",
      cont_noz
    );
    assert!(
      init_noz >= 217,
      "Expected PLA-CF initial nozzle >= 217°C on PCD, got {}°C",
      init_noz
    );

    // 3. Fan routing: M106 P2 horizontal side blower (`additional_cooling_fan_speed`) is 0% OFF on PLA-CF,
    //    M106 P3 chamber exhaust (`during_print_exhaust_fan_speed`) is 90% ON, and fans stay off >= 2 layers!
    assert_eq!(exported["additional_cooling_fan_speed"][0].as_str(), Some("0"));
    assert_eq!(
      exported["during_print_exhaust_fan_speed"][0].as_str(),
      Some("90")
    );
    let close_layers: i32 = exported["close_fan_the_first_x_layers"][0]
      .as_str()
      .unwrap()
      .parse()
      .unwrap();
    assert!(
      close_layers >= 2,
      "Expected PLA-CF close_fan_the_first_x_layers >= 2, got {}",
      close_layers
    );

    // 4. Process profile check: Scarf seam defaults to "external" (outer contour only, inner walls = 0),
    //    overhang slowdowns (50 / 30 / 10 mm/s) are NEVER wiped to "0" by overhang_1_4_speed == "0",
    //    and `dont_slow_down_outer_wall` is NEVER emitted in Process JSON!
    let base_proc = crate::orca_profile::OrcaProfile::get_stock_kobra_s1();
    let rules = crate::transformations::TransformationEngine::get_default_rules();
    let eval_proc = crate::transformations::TransformationEngine::evaluate_stack(
      &base_proc,
      &rules,
      &serde_json::json!({
        "nozzle_diameter": 0.4,
        "printer_kinematics_class": "corexy",
        "_process_seeded": true,
      }),
    );
    let procs = crate::models::ProcessProfile::get_default_processes();
    let proc_json: serde_json::Value = serde_json::from_str(&build_process_preset_json(
      &procs[0],
      &rig,
      &eval_proc.resulting_profile.values,
    ))
    .unwrap();
    assert_eq!(proc_json["seam_slope_type"].as_str(), Some("external"));
    assert_eq!(proc_json["seam_slope_inner_walls"].as_str(), Some("0"));
    assert!(proc_json.get("dont_slow_down_outer_wall").is_none());
    for ov_k in [
      "overhang_2_4_speed",
      "overhang_3_4_speed",
      "overhang_4_4_speed",
    ] {
      assert_ne!(
        proc_json.get(ov_k).and_then(|v| v.as_str()),
        Some("0"),
        "Process {} must never be wiped out to '0': {:?}",
        ov_k,
        proc_json
      );
    }
  }
}
