use crate::models::{FilamentSpool, MachineRig, ProcessProfile};
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

/// Resolve a guaranteed-valid system filament preset name for Anycubic Kobra S1 0.4 nozzle
pub fn resolve_kobra_s1_filament_inherits(spool: &FilamentSpool) -> String {
  if spool.id.starts_with("disk_")
    && !spool.inherits.trim().is_empty()
    && !spool.inherits.contains("TPU for ACE @Anycubic Kobra S1 ")
    && !spool.inherits.contains("TPU 95A @Anycubic Kobra S1 ")
    && !spool.inherits.contains("PCTG @Anycubic Kobra S1 ")
  {
    return spool.inherits.clone();
  }
  match spool.material_type.trim() {
    "PLA+" => "Anycubic PLA+ @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PLA-CF" | "PLA-GF" => "Anycubic PLA-CF @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PLA Matte" => "Anycubic PLA Matte @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PLA Silk" => "Anycubic PLA Silk @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PLA Glow" => "Anycubic PLA Glow @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PLA High Speed" => "Anycubic PLA High Speed @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PETG-CF" | "PETG-GF" | "PCTG-CF" | "PCTG-GF" => {
      "Anycubic PETG-CF @Anycubic Kobra S1 0.4 nozzle".to_string()
    }
    "PET-CF" | "PET-GF" => "Anycubic PET-CF @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PETG" | "PCTG" | "PETG Translucent" | "PETG HF" => {
      "Anycubic PETG @Anycubic Kobra S1 0.4 nozzle".to_string()
    }
    "TPU" | "TPU for ACE" | "TPU for AMS" | "TPU 95A" | "TPU 95A HF" | "TPU 90A" | "TPU 85A"
    | "TPU 64D" | "Rebound TPU" | "TPE" => "Anycubic TPU @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PEBA" | "PEBA 90A" | "PEBA-CF" => "Anycubic PEBA @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "ASA" | "ASA-CF" | "ASA-GF" => "Anycubic ASA @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "ABS" | "ABS-CF" | "ABS-GF" => "Anycubic ABS @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PC" | "PC-CF" | "PC-GF" => "Anycubic PC @Anycubic Kobra S1 0.4 nozzle".to_string(),
    "PA" | "PA-CF" | "PA6-CF" | "PA6-GF" | "PA12" | "PA12-CF" | "PAHT" | "PAHT-CF" | "PAHT-GF"
    | "PPA" | "PPA-CF" | "PPA-GF" => "Anycubic PA6-CF @Anycubic Kobra S1 0.4 nozzle".to_string(),
    _ => "Anycubic PLA @Anycubic Kobra S1 0.4 nozzle".to_string(),
  }
}

/// Map internal material codes to OrcaSlicer / AnycubicSlicerNext valid `filament_type` enum strings
pub fn resolve_orca_filament_type(material_type: &str) -> String {
  match material_type.trim() {
    "PLA+" | "PLA Matte" | "PLA Silk" | "PLA Glow" | "PLA High Speed" => "PLA".to_string(),
    "PLA-GF" => "PLA-CF".to_string(),
    "PCTG" | "PETG Translucent" | "PETG HF" => "PETG".to_string(),
    "PCTG-CF" | "PCTG-GF" | "PETG-GF" => "PETG-CF".to_string(),
    "PET-GF" => "PET-CF".to_string(),
    "TPU 95A" | "TPU 95A HF" | "TPU 90A" | "TPU 85A" | "TPU 64D" | "TPU for AMS"
    | "Rebound TPU" | "TPE" => "TPU".to_string(),
    "PEBA 90A" | "PEBA-CF" => "PEBA".to_string(),
    "ASA-CF" | "ASA-GF" => "ASA".to_string(),
    "ABS-CF" | "ABS-GF" => "ABS".to_string(),
    "PA-CF" | "PA6-GF" | "PA12-CF" => "PA6-CF".to_string(),
    "PAHT" | "PAHT-GF" | "PPA" | "PPA-CF" | "PPA-GF" => "PAHT-CF".to_string(),
    "PA12" => "PA".to_string(),
    other => other.to_string(),
  }
}

const FILAMENT_DIFF_KEYS: &str = "default_filament_colour;fan_max_speed;fan_max_speed_BRASS;fan_max_speed_HS;filament_deretraction_speed;filament_diameter;filament_flow_ratio;filament_max_volumetric_speed;filament_retract_before_wipe;filament_retraction_length;filament_retraction_speed;filament_type;filament_wipe;filament_wipe_distance;filament_z_hop;filament_z_hop_types;hot_plate_temp;hot_plate_temp_initial_layer;nozzle_temperature;nozzle_temperature_BRASS;nozzle_temperature_HS;nozzle_temperature_initial_layer;nozzle_temperature_initial_layer_BRASS;nozzle_temperature_initial_layer_HS;pressure_advance;required_nozzle_HRC;textured_plate_temp;textured_plate_temp_initial_layer";

fn build_filament_preset_json_with_source(
  spool: &FilamentSpool,
  _rig: &MachineRig,
  from_src: &str,
) -> String {
  let effective_temp = spool.print_temp;
  let is_pctg_or_petg =
    spool.material_type.contains("PCTG") || spool.material_type.contains("PETG");
  let is_translucent_petg =
    is_pctg_or_petg && spool.finish_type.eq_ignore_ascii_case("Translucent");
  let initial_temp = if is_pctg_or_petg {
    effective_temp
  } else {
    (effective_temp + 5).min(350)
  };
  let export_name = spool.format_concise_title();
  let inherits = resolve_kobra_s1_filament_inherits(spool);
  let orca_fil_type = resolve_orca_filament_type(&spool.material_type);

  let json_val = serde_json::json!({
    "type": "filament",
    "from": from_src,
    "is_custom_defined": "0",
    "inherits": inherits,
    "name": export_name,
    "filament_settings_id": [export_name],
    "compatible_printers": ["Anycubic Kobra S1 0.4 nozzle"],
    "filament_type": [orca_fil_type],
    "filament_diameter": ["1.75"],
    "required_nozzle_HRC": [spool.required_nozzle_hrc.to_string()],
    "default_filament_colour": [spool.color_hex],
    "nozzle_temperature": [effective_temp.to_string()],
    "nozzle_temperature_BRASS": [effective_temp.to_string()],
    "nozzle_temperature_HS": [effective_temp.to_string()],
    "nozzle_temperature_initial_layer": [initial_temp.to_string()],
    "nozzle_temperature_initial_layer_BRASS": [initial_temp.to_string()],
    "nozzle_temperature_initial_layer_HS": [initial_temp.to_string()],
    "hot_plate_temp": [spool.bed_temp.to_string()],
    "hot_plate_temp_initial_layer": [spool.bed_temp.to_string()],
    "textured_plate_temp": [spool.bed_temp.to_string()],
    "textured_plate_temp_initial_layer": [spool.bed_temp.to_string()],
    "filament_flow_ratio": [format!("{:.2}", spool.flow_ratio)],
    "pressure_advance": [format!("{:.3}", spool.pressure_advance)],
    "filament_max_volumetric_speed": [format!("{:.1}", spool.max_volumetric_speed)],
    "fan_max_speed": [spool.fan_speed_percent.to_string()],
    "fan_max_speed_BRASS": [spool.fan_speed_percent.to_string()],
    "fan_max_speed_HS": [spool.fan_speed_percent.to_string()],
    "filament_retraction_length": [if is_pctg_or_petg { "0.8" } else { "0.6" }],
    "filament_retraction_speed": [if is_pctg_or_petg { "35" } else { "40" }],
    "filament_z_hop": [if is_translucent_petg { "0.1" } else if is_pctg_or_petg { "0.15" } else { "0.2" }],
    "filament_z_hop_types": [if is_pctg_or_petg { "Spiral Lift" } else { "Normal Lift" }],
    "filament_retract_before_wipe": [if is_pctg_or_petg { "70%" } else { "0%" }],
    "filament_deretraction_speed": [if is_pctg_or_petg { "30" } else { "0" }],
    "filament_wipe": ["1"],
    "filament_wipe_distance": ["2"],
    "version": "2.0.0.0"
  });

  serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| "{}".to_string())
}

/// Export a standalone OrcaSlicer Filament `.json` delta preset (Section 4.A)
pub fn export_standalone_filament_json(spool: &FilamentSpool, rig: &MachineRig) -> String {
  build_filament_preset_json_with_source(spool, rig, "User")
}

/// Build the `Metadata/project_settings.config` JSON string for 4-slot or 8-slot ACE Pro loadouts (Section 4.B)
pub fn build_project_settings_config_json(
  rig: &MachineRig,
  process: &ProcessProfile,
  active_slots: &[FilamentSpool],
  flush_matrix: &[String],
  derived_overrides: &serde_json::Map<String, serde_json::Value>,
) -> String {
  let (orca_nozzle_type, _) = rig.orca_machine_nozzle_type_and_hrc();

  let ids: Vec<String> = active_slots
    .iter()
    .map(|s| s.format_concise_title())
    .collect();
  let types: Vec<String> = active_slots
    .iter()
    .map(|s| resolve_orca_filament_type(&s.material_type))
    .collect();
  let fil_required_hrcs: Vec<String> = active_slots
    .iter()
    .map(|s| s.required_nozzle_hrc.to_string())
    .collect();
  let colors: Vec<String> = active_slots.iter().map(|s| s.color_hex.clone()).collect();
  let temps: Vec<String> = active_slots
    .iter()
    .map(|s| s.print_temp.to_string())
    .collect();
  let init_temps: Vec<String> = active_slots
    .iter()
    .map(|s| {
      if s.material_type.contains("PCTG") || s.material_type.contains("PETG") {
        s.print_temp.to_string()
      } else {
        (s.print_temp + 5).min(350).to_string()
      }
    })
    .collect();
  let bed_temps: Vec<String> = active_slots
    .iter()
    .map(|s| s.bed_temp.to_string())
    .collect();
  let flows: Vec<String> = active_slots
    .iter()
    .map(|s| format!("{:.2}", s.flow_ratio))
    .collect();
  let pas: Vec<String> = active_slots
    .iter()
    .map(|s| format!("{:.3}", s.pressure_advance))
    .collect();
  let mvs: Vec<String> = active_slots
    .iter()
    .map(|s| format!("{:.1}", s.max_volumetric_speed))
    .collect();
  let fan_max: Vec<String> = active_slots
    .iter()
    .map(|s| s.fan_speed_percent.to_string())
    .collect();
  let fil_retract_len: Vec<String> = active_slots
    .iter()
    .map(|s| {
      if s.material_type.contains("PCTG") || s.material_type.contains("PETG") {
        "0.8".to_string()
      } else {
        "0.6".to_string()
      }
    })
    .collect();
  let fil_retract_spd: Vec<String> = active_slots
    .iter()
    .map(|s| {
      if s.material_type.contains("PCTG") || s.material_type.contains("PETG") {
        "35".to_string()
      } else {
        "40".to_string()
      }
    })
    .collect();
  let fil_wipe: Vec<String> = vec!["1".to_string(); active_slots.len()];
  let fil_wipe_dist: Vec<String> = vec!["2".to_string(); active_slots.len()];
  let fil_z_hop: Vec<String> = active_slots
    .iter()
    .map(|s| {
      let is_petg = s.material_type.contains("PCTG") || s.material_type.contains("PETG");
      if is_petg && s.finish_type.eq_ignore_ascii_case("Translucent") {
        "0.1".to_string()
      } else if is_petg {
        "0.15".to_string()
      } else {
        "0.2".to_string()
      }
    })
    .collect();
  let fil_z_hop_types: Vec<String> = active_slots
    .iter()
    .map(|s| {
      if s.material_type.contains("PCTG") || s.material_type.contains("PETG") {
        "Spiral Lift".to_string()
      } else {
        "Normal Lift".to_string()
      }
    })
    .collect();
  let fil_rbw: Vec<String> = active_slots
    .iter()
    .map(|s| {
      if s.material_type.contains("PCTG") || s.material_type.contains("PETG") {
        "70%".to_string()
      } else {
        "0%".to_string()
      }
    })
    .collect();
  let fil_deretract: Vec<String> = active_slots
    .iter()
    .map(|s| {
      if s.material_type.contains("PCTG") || s.material_type.contains("PETG") {
        "30".to_string()
      } else {
        "0".to_string()
      }
    })
    .collect();

  // Build inherits_group and different_settings_to_system (length = 1 process + N filaments + 1 printer)
  let proc_inherits = if process.inherits.trim().is_empty() {
    "0.20mm Standard @Anycubic Kobra S1 0.4 nozzle".to_string()
  } else {
    process.inherits.clone()
  };
  let mach_inherits = "Anycubic Kobra S1 0.4 nozzle".to_string();
  let mut inherits_group: Vec<String> = Vec::with_capacity(active_slots.len() + 2);
  inherits_group.push(proc_inherits);
  for s in active_slots {
    inherits_group.push(resolve_kobra_s1_filament_inherits(s));
  }
  inherits_group.push(mach_inherits);

  let mut different_settings_to_system: Vec<String> = Vec::with_capacity(active_slots.len() + 2);
  different_settings_to_system.push(
    "default_acceleration;initial_layer_speed;inner_wall_speed;layer_height;line_width;min_bead_width;min_feature_size;notes;outer_wall_line_width;outer_wall_speed;print_sequence;reduce_crossing_wall;scarf_angle_threshold;seam_gap;seam_slope_conditional;seam_slope_entire_loop;seam_slope_inner_walls;seam_slope_min_length;seam_slope_start_height;seam_slope_steps;seam_slope_type;sparse_infill_pattern;sparse_infill_speed;wall_generator;wall_transition_angle;wall_transition_length;wipe_on_loops".to_string(),
  );
  for _ in active_slots {
    different_settings_to_system.push(FILAMENT_DIFF_KEYS.to_string());
  }
  different_settings_to_system.push(
    "nozzle_diameter;nozzle_type;printer_notes;retraction_length;retraction_speed".to_string(),
  );

  let flush_vec: Vec<String> = vec!["140".to_string(); active_slots.len() * 2];
  let fil_diameters: Vec<String> = vec!["1.75".to_string(); active_slots.len()];

  let noz_diam_str = rig.formatted_nozzle_diameter();

  let get_proc_str = |key: &str, default_val: &str| -> String {
    let raw = derived_overrides
      .get(key)
      .and_then(|v| v.as_str())
      .unwrap_or(default_val);
    if key == "seam_slope_type" && raw == "contour" {
      "external".to_string()
    } else {
      raw.to_string()
    }
  };

  let config_obj = serde_json::json!({
    "from": "project",
    "printer_model": rig.printer_model,
    "printer_variant": noz_diam_str,
    "printer_settings_id": rig.format_concise_title(),
    "printer_notes": rig.notes,
    "print_settings_id": process.format_concise_title(&rig.short_model_tag()),
    "notes": process.notes,
    "nozzle_diameter": [noz_diam_str],
    "nozzle_type": orca_nozzle_type,
    "retraction_length": [format!("{:.2}", rig.base_retraction_length)],
    "retraction_speed": ["40"],
    "printable_area": ["0x0", "250x0", "250x250", "0x250"],
    "printable_height": "250",
    "compatible_printers": ["Anycubic Kobra S1 0.4 nozzle"],
    "inherits_group": inherits_group,
    "different_settings_to_system": different_settings_to_system,
    "filament_settings_id": ids,
    "filament_type": types,
    "filament_diameter": fil_diameters,
    "required_nozzle_HRC": fil_required_hrcs,
    "default_filament_colour": colors.clone(),
    "filament_colour": colors,
    "nozzle_temperature": temps.clone(),
    "nozzle_temperature_BRASS": temps.clone(),
    "nozzle_temperature_HS": temps,
    "nozzle_temperature_initial_layer": init_temps.clone(),
    "nozzle_temperature_initial_layer_BRASS": init_temps.clone(),
    "nozzle_temperature_initial_layer_HS": init_temps,
    "hot_plate_temp": bed_temps.clone(),
    "hot_plate_temp_initial_layer": bed_temps.clone(),
    "textured_plate_temp": bed_temps.clone(),
    "textured_plate_temp_initial_layer": bed_temps,
    "filament_flow_ratio": flows,
    "pressure_advance": pas,
    "filament_max_volumetric_speed": mvs,
    "fan_max_speed": fan_max.clone(),
    "fan_max_speed_BRASS": fan_max.clone(),
    "fan_max_speed_HS": fan_max,
    "filament_retraction_length": fil_retract_len,
    "filament_retraction_speed": fil_retract_spd,
    "filament_z_hop": fil_z_hop,
    "filament_z_hop_types": fil_z_hop_types,
    "filament_retract_before_wipe": fil_rbw,
    "filament_deretraction_speed": fil_deretract,
    "filament_wipe": fil_wipe,
    "filament_wipe_distance": fil_wipe_dist,
    "layer_height": format!("{:.2}", process.layer_height),
    "line_width": format!("{:.2}", process.line_width),
    "outer_wall_line_width": format!("{:.2}", process.line_width),
    "outer_wall_speed": format!("{:.0}", process.outer_wall_speed),
    "inner_wall_speed": format!("{:.0}", process.inner_wall_speed),
    "sparse_infill_speed": format!("{:.0}", process.sparse_infill_speed),
    "initial_layer_speed": format!("{:.0}", process.initial_layer_speed),
    "default_acceleration": format!("{:.0}", process.default_acceleration),
    "wall_generator": get_proc_str("wall_generator", &process.wall_generator),
    "wall_transition_angle": get_proc_str("wall_transition_angle", "10"),
    "wall_transition_length": get_proc_str("wall_transition_length", "100%"),
    "min_feature_size": get_proc_str("min_feature_size", "25%"),
    "min_bead_width": get_proc_str("min_bead_width", "85%"),
    "seam_slope_type": get_proc_str("seam_slope_type", "all"),
    "seam_slope_conditional": get_proc_str("seam_slope_conditional", "1"),
    "seam_slope_start_height": get_proc_str("seam_slope_start_height", "10%"),
    "seam_slope_entire_loop": get_proc_str("seam_slope_entire_loop", "0"),
    "seam_slope_inner_walls": get_proc_str("seam_slope_inner_walls", "1"),
    "seam_slope_min_length": get_proc_str("seam_slope_min_length", "15"),
    "seam_slope_steps": get_proc_str("seam_slope_steps", "15"),
    "seam_gap": get_proc_str("seam_gap", "10%"),
    "scarf_angle_threshold": get_proc_str("scarf_angle_threshold", "155"),
    "wipe_on_loops": get_proc_str("wipe_on_loops", "1"),
    "sparse_infill_pattern": get_proc_str("sparse_infill_pattern", "gyroid"),
    "reduce_crossing_wall": get_proc_str("reduce_crossing_wall", "1"),
    "print_sequence": process.print_sequence,
    "flush_multiplier": derived_overrides
      .get("flush_multiplier")
      .and_then(|v| v.as_str())
      .unwrap_or("1.0"),
    "flush_volumes_vector": flush_vec,
    "flush_volumes_matrix": flush_matrix,
    "version": "02.03.00.03"
  });

  serde_json::to_string_pretty(&config_obj).unwrap_or_else(|_| "{}".to_string())
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
  let colors = val.get("filament_colour").and_then(|v| v.as_array());
  let temps = val.get("nozzle_temperature").and_then(|v| v.as_array());
  let bed_temps = val.get("hot_plate_temp").and_then(|v| v.as_array());
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

    let brand = name
      .split(&['-', ' '][..])
      .find(|part| !part.is_empty() && *part != "Kobra")
      .unwrap_or("Anycubic")
      .to_string();

    let brass_base = (print_temp - 5).max(190);
    spools.push(FilamentSpool {
      id: format!("imported_slot_{}", idx + 1),
      name,
      brand,
      inherits: format!("Anycubic {} @Anycubic Kobra S1 0.4 nozzle", mat),
      material_type: mat.clone(),
      nozzle_type: "hardened_steel".to_string(),
      tuned_for_nozzle_type: "hardened_steel".to_string(),
      required_nozzle_hrc: 3,
      color_hex: color_hex.clone(),
      color_name: format!("Slot #{} ({})", idx + 1, color_hex),
      brass_base_temp: brass_base,
      hs_base_temp: print_temp,
      base_print_temp: brass_base,
      print_temp,
      base_bed_temp: (bed_temp - 5).max(45),
      bed_temp,
      flow_ratio,
      pressure_advance,
      max_volumetric_speed: max_vol,
      fan_speed_percent: if mat == "PETG" { 40 } else { 100 },
      remaining_grams: 850,
      is_loaded: true,
      finish_type,
      opacity,
      pigment_stickiness,
      drying_temp_c: if mat == "PETG" { 55 } else { 45 },
      drying_time_hr: if mat == "PETG" { 6 } else { 4 },
      is_rfid_synced: true,
      deltas_from_base: Vec::new(),
      raw_disk_json: None,
    });
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
    assert_eq!(ps_map["seam_slope_type"], serde_json::json!("all"));
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
  }
}
