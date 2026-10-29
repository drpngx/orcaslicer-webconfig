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
    out.extend_from_slice(&0x0403_4B50u32.to_le_bytes()); // signature
    out.extend_from_slice(&20u16.to_le_bytes()); // version needed (2.0)
    out.extend_from_slice(&0u16.to_le_bytes()); // general purpose bit flag
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
    central_dir.extend_from_slice(&0u16.to_le_bytes()); // flags
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

/// Export a standalone OrcaSlicer Filament `.json` delta preset (Section 4.A)
pub fn export_standalone_filament_json(spool: &FilamentSpool, rig: &MachineRig) -> String {
  // Thermal Conductivity Rule (Section 3 Dynamic Combination Logic)
  let temp_offset = match rig.short_nozzle_tag() {
    "Hardened" | "Stainless" => 5,
    "Diamondback" => -10,
    _ => 0,
  };
  let brass_temp = if spool.brass_base_temp > 0 {
    spool.brass_base_temp
  } else {
    spool.base_print_temp
  };
  let hs_temp = if spool.hs_base_temp > 0 {
    spool.hs_base_temp
  } else {
    brass_temp + 5
  };
  let effective_temp = spool.base_print_temp + temp_offset;
  let initial_temp = effective_temp + 5;
  let (orca_nozzle_type, _) = rig.orca_machine_nozzle_type_and_hrc();
  let export_name = spool.format_concise_title();
  let is_pctg_or_petg =
    spool.material_type.contains("PCTG") || spool.material_type.contains("PETG");

  let json_val = serde_json::json!({
    "from": "User",
    "inherits": spool.inherits,
    "name": export_name,
    "filament_settings_id": [export_name],
    "filament_type": [spool.material_type],
    "nozzle_type": [orca_nozzle_type],
    "required_nozzle_HRC": [spool.required_nozzle_hrc.to_string()],
    "filament_colour": [spool.color_hex],
    "nozzle_temperature": [effective_temp.to_string()],
    "nozzle_temperature_BRASS": [brass_temp.to_string()],
    "nozzle_temperature_HS": [hs_temp.to_string()],
    "nozzle_temperature_initial_layer": [initial_temp.to_string()],
    "nozzle_temperature_initial_layer_BRASS": [(brass_temp + 5).to_string()],
    "nozzle_temperature_initial_layer_HS": [(hs_temp + 5).to_string()],
    "hot_plate_temp": [spool.bed_temp.to_string()],
    "textured_plate_temp": [spool.bed_temp.to_string()],
    "hot_plate_temp_initial_layer": [spool.bed_temp.to_string()],
    "textured_plate_temp_initial_layer": [spool.bed_temp.to_string()],
    "filament_flow_ratio": [format!("{:.2}", spool.flow_ratio)],
    "pressure_advance": [format!("{:.3}", spool.pressure_advance)],
    "filament_max_volumetric_speed": [format!("{:.1}", spool.max_volumetric_speed)],
    "fan_max_speed": [spool.fan_speed_percent.to_string()],
    "fan_max_speed_BRASS": [spool.fan_speed_percent.to_string()],
    "fan_max_speed_HS": [spool.fan_speed_percent.to_string()],
    "filament_z_hop": [if is_pctg_or_petg { "0.15" } else { "0.2" }],
    "filament_z_hop_types": [if is_pctg_or_petg { "Spiral" } else { "Normal Lift" }],
    "filament_retract_before_wipe": [if is_pctg_or_petg { "70%" } else { "0%" }],
    "filament_deretraction_speed": [if is_pctg_or_petg { "30" } else { "0" }],
    "orcaslicer_webconfig_metadata": {
      "finish_type": spool.finish_type,
      "nozzle_type": spool.nozzle_type,
      "required_nozzle_hrc": spool.required_nozzle_hrc,
      "opacity": spool.opacity,
      "pigment_stickiness": spool.pigment_stickiness,
      "drying_temp_c": spool.drying_temp_c,
      "drying_time_hr": spool.drying_time_hr,
      "is_rfid_synced": spool.is_rfid_synced
    },
    "version": "2.0.0.0"
  });

  serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| "{}".to_string())
}

/// Build the `Metadata/project_settings.config` JSON string for 4-slot or 8-slot ACE Pro loadouts (Section 4.B)
pub fn build_project_settings_config_json(
  rig: &MachineRig,
  process: &ProcessProfile,
  active_slots: &[FilamentSpool],
  flush_matrix: &[String],
  derived_overrides: &serde_json::Map<String, serde_json::Value>,
) -> String {
  let temp_offset = match rig.short_nozzle_tag() {
    "Hardened" | "Stainless" => 5,
    "Diamondback" => -10,
    _ => 0,
  };
  let (orca_nozzle_type, orca_nozzle_hrc) = rig.orca_machine_nozzle_type_and_hrc();

  let ids: Vec<String> = active_slots
    .iter()
    .map(|s| s.format_concise_title())
    .collect();
  let types: Vec<String> = active_slots
    .iter()
    .map(|s| s.material_type.clone())
    .collect();
  let fil_nozzle_types: Vec<String> = active_slots
    .iter()
    .map(|_| orca_nozzle_type.to_string())
    .collect();
  let fil_required_hrcs: Vec<String> = active_slots
    .iter()
    .map(|s| s.required_nozzle_hrc.to_string())
    .collect();
  let colors: Vec<String> = active_slots.iter().map(|s| s.color_hex.clone()).collect();
  let temps: Vec<String> = active_slots
    .iter()
    .map(|s| (s.print_temp + temp_offset).to_string())
    .collect();
  let init_temps: Vec<String> = active_slots
    .iter()
    .map(|s| (s.print_temp + temp_offset + 5).to_string())
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
  let finishes: Vec<String> = active_slots.iter().map(|s| s.finish_type.clone()).collect();
  let opacities: Vec<f32> = active_slots.iter().map(|s| s.opacity).collect();
  let stickiness: Vec<f32> = active_slots.iter().map(|s| s.pigment_stickiness).collect();
  let drying_temps: Vec<i32> = active_slots.iter().map(|s| s.drying_temp_c).collect();
  let drying_hours: Vec<i32> = active_slots.iter().map(|s| s.drying_time_hr).collect();

  // Volumetric Speed Ceiling Rule (Section 3)
  let min_fil_flow = active_slots
    .iter()
    .map(|s| s.max_volumetric_speed as f64)
    .fold(rig.max_volumetric_flow_limit, f64::min);
  let effective_flow = rig.max_volumetric_flow_limit.min(min_fil_flow);
  let cross_section = (process.layer_height * process.line_width).max(0.01);
  let safe_speed_ceiling = (effective_flow / cross_section).floor();

  let flush_vec: Vec<String> = vec!["140".to_string(); active_slots.len() * 2];

  let mut config_obj = serde_json::json!({
    "printer_model": rig.printer_model,
    "printer_settings_id": rig.format_concise_title(),
    "printer_notes": rig.notes,
    "print_settings_id": process.format_concise_title(&rig.short_model_tag()),
    "notes": process.notes,
    "nozzle_diameter": [rig.nozzle_diameter],
    "nozzle_type": orca_nozzle_type,
    "nozzle_hrc": orca_nozzle_hrc.to_string(),
    "nozzle_material": rig.nozzle_material,
    "ace_pro_units": rig.ace_pro_units,
    "active_slot_count": active_slots.len(),
    "melt_zone_multiplier": rig.melt_zone_multiplier,
    "filament_settings_id": ids,
    "filament_type": types,
    "filament_nozzle_type": fil_nozzle_types,
    "required_nozzle_HRC": fil_required_hrcs,
    "filament_colour": colors,
    "nozzle_temperature": temps,
    "nozzle_temperature_initial_layer": init_temps,
    "hot_plate_temp": bed_temps,
    "filament_flow_ratio": flows,
    "pressure_advance": pas,
    "filament_max_volumetric_speed": mvs,
    "filament_finish_type": finishes,
    "filament_opacity": opacities,
    "filament_pigment_stickiness": stickiness,
    "filament_drying_temp_c": drying_temps,
    "filament_drying_time_hr": drying_hours,
    "layer_height": format!("{:.2}", process.layer_height),
    "outer_wall_line_width": format!("{:.2}", process.line_width),
    "outer_wall_speed": format!("{:.0}", process.outer_wall_speed.min(safe_speed_ceiling)),
    "inner_wall_speed": format!("{:.0}", process.inner_wall_speed.min(safe_speed_ceiling)),
    "sparse_infill_speed": format!("{:.0}", process.sparse_infill_speed.min(safe_speed_ceiling)),
    "initial_layer_speed": format!("{:.0}", process.initial_layer_speed),
    "default_acceleration": format!("{:.0}", process.default_acceleration),
    "computed_safe_speed_ceiling_mm_s": safe_speed_ceiling,
    "flush_multiplier": derived_overrides
      .get("flush_multiplier")
      .and_then(|v| v.as_str())
      .unwrap_or("1.0"),
    "flush_volumes_vector": flush_vec,
    "flush_volumes_matrix": flush_matrix,
    "version": "2.0.0.0"
  });

  if let Some(map) = config_obj.as_object_mut() {
    for (k, v) in derived_overrides {
      if !map.contains_key(k) {
        map.insert(k.clone(), v.clone());
      }
    }
  }

  serde_json::to_string_pretty(&config_obj).unwrap_or_else(|_| "{}".to_string())
}

/// Build a complete `.3mf` Project Container (ZIP archive) with `Metadata/project_settings.config`
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
</Types>"#;

  let rels_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Target="/3D/3dmodel.model" Id="rel-1" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
</Relationships>"#;

  let model_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <metadata name="Application">OrcaSlicer-WebConfig-2.1</metadata>
  <resources/>
  <build/>
</model>"#;

  let project_settings_json =
    build_project_settings_config_json(rig, process, active_slots, flush_matrix, derived_overrides);

  let files = [
    ("[Content_Types].xml", content_types_xml.as_bytes()),
    ("_rels/.rels", rels_xml.as_bytes()),
    ("3D/3dmodel.model", model_xml.as_bytes()),
    (
      "Metadata/project_settings.config",
      project_settings_json.as_bytes(),
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
  let json_text = if data.len() >= 4 && &data[0..4] == b"PK\x03\x04" {
    extract_file_from_zip(data, "Metadata/project_settings.config").ok_or_else(|| {
      "ZIP/.3mf archive does not contain Metadata/project_settings.config".to_string()
    })?
  } else {
    std::str::from_utf8(data)
      .map_err(|e| format!("Invalid UTF-8 in config file: {}", e))?
      .to_string()
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

  let types = val.get("filament_type").and_then(|v| v.as_array());
  let colors = val.get("filament_colour").and_then(|v| v.as_array());
  let temps = val.get("nozzle_temperature").and_then(|v| v.as_array());
  let bed_temps = val.get("hot_plate_temp").and_then(|v| v.as_array());
  let flows = val.get("filament_flow_ratio").and_then(|v| v.as_array());
  let pas = val.get("pressure_advance").and_then(|v| v.as_array());
  let mvs = val
    .get("filament_max_volumetric_speed")
    .and_then(|v| v.as_array());
  let finishes = val.get("filament_finish_type").and_then(|v| v.as_array());
  let opacities = val.get("filament_opacity").and_then(|v| v.as_array());
  let stickiness = val
    .get("filament_pigment_stickiness")
    .and_then(|v| v.as_array());

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
  }
}
