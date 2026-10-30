use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneratedCatalog {
  pub machines: Vec<CatalogMachine>,
  pub nozzles: Vec<CatalogNozzle>,
  pub beds: Vec<CatalogBed>,
  pub ams_options: Vec<CatalogAmsOption>,
  pub filaments: Vec<CatalogFilament>,
  pub standard_colors: Vec<CatalogColorSwatch>,
  #[serde(default)]
  pub default_machine_base: BTreeMap<String, Value>,
  #[serde(default)]
  pub default_process_base: BTreeMap<String, Value>,
  #[serde(default)]
  pub default_filament_bases: BTreeMap<String, BTreeMap<String, Value>>,
  #[serde(default)]
  pub default_voron_base: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogMachine {
  pub id: String,
  pub vendor: String,
  pub name: String,
  pub kinematics: String,
  pub build_volume: String,
  pub default_nozzle_diameter: f32,
  pub available_nozzle_diameters: Vec<f32>,
  pub default_nozzle_material: String,
  pub default_bed_id: String,
  pub supported_bed_ids: Vec<String>,
  pub default_ams_id: String,
  pub supported_ams_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogNozzle {
  pub id: String,
  pub brand: String,
  pub name: String,
  pub material: String,
  pub is_high_flow: bool,
  pub base_flow_mm3s: f32,
  pub retraction_mm: f32,
  pub melt_zone_mult: f32,
  pub temp_offset_c: i32,
  pub description: String,
  pub available_diameters: Vec<f32>,
  pub compatible_vendors: Vec<String>,
  pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogBed {
  pub id: String,
  pub name: String,
  pub surface_code: String,
  pub temp_offset_c: i32,
  pub description: String,
  pub compatible_vendors: Vec<String>,
  pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogAmsOption {
  pub id: String,
  pub name: String,
  pub vendor: String,
  pub units: usize,
  pub total_slots: usize,
  pub description: String,
  #[serde(default)]
  pub compatible_vendors: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogFilament {
  pub brand: String,
  pub code: String,
  pub label: String,
  pub category: String,
  pub default_finish: String,
  pub default_color: String,
  pub base_nozzle_c: u16,
  pub base_bed_c: u16,
  pub max_vol_mm3s: f32,
  pub fan_pct: u8,
  pub default_flow_ratio: f32,
  pub default_retraction_mm: f32,
  pub dry_temp_c: u16,
  pub dry_hr: u8,
  pub requires_hardened: bool,
  pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogColorSwatch {
  pub name: String,
  pub hex: String,
}

fn collect_json_files(dir: &Path, out: &mut Vec<PathBuf>) {
  if let Ok(entries) = fs::read_dir(dir) {
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
      if path.is_dir() {
        collect_json_files(&path, out);
      } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
        out.push(path);
      }
    }
  }
}

fn first_str(val: Option<&Value>) -> Option<String> {
  match val? {
    Value::String(s) => Some(s.clone()),
    Value::Array(arr) => arr.first()?.as_str().map(|s| s.to_string()),
    Value::Number(n) => Some(n.to_string()),
    _ => None,
  }
}

fn normalize_vendor_name(raw_dir_name: &str) -> String {
  match raw_dir_name {
    "BBL" => "Bambu Lab".to_string(),
    "Custom" => "Custom / Generic".to_string(),
    other => other.to_string(),
  }
}

fn vendor_priority(vendor: &str) -> usize {
  match vendor {
    "Anycubic" => 0,
    "Bambu Lab" => 1,
    "Prusa" => 2,
    "Creality" => 3,
    "Voron" => 4,
    "Qidi" => 5,
    "Sovol" => 6,
    "RatRig" => 7,
    "Elegoo" => 8,
    "Flashforge" => 9,
    "FLSun" => 10,
    "Anker" => 11,
    "BIQU" => 12,
    _ => 50,
  }
}

fn resolve_profile<'a>(name: &str, by_name: &'a HashMap<String, Value>, depth: usize) -> MapView {
  if depth > 16 {
    return BTreeMap::new();
  }
  let Some(Value::Object(obj)) = by_name.get(name) else {
    return BTreeMap::new();
  };
  let mut merged = if let Some(Value::String(parent)) = obj.get("inherits") {
    resolve_profile(parent, by_name, depth + 1)
  } else {
    BTreeMap::new()
  };
  for (k, v) in obj {
    merged.insert(k.clone(), v.clone());
  }
  merged
}

type MapView = BTreeMap<String, Value>;

fn load_dir_profile_map(dir: &Path, mut base: HashMap<String, Value>) -> HashMap<String, Value> {
  if dir.is_dir() {
    let mut files = Vec::new();
    collect_json_files(dir, &mut files);
    for f in &files {
      if let Ok(txt) = fs::read_to_string(f) {
        if let Ok(val) = serde_json::from_str::<Value>(&txt) {
          if let Some(name) = val.get("name").and_then(|v| v.as_str()) {
            base.insert(name.to_string(), val);
          }
        }
      }
    }
  }
  base
}

fn is_nil_or_empty_orca_value(val: &Value) -> bool {
  match val {
    Value::Null => true,
    Value::String(s) => s.trim().eq_ignore_ascii_case("nil"),
    Value::Array(arr) => {
      arr.is_empty()
        || arr.iter().all(|x| {
          x.as_str()
            .map(|s| s.trim().eq_ignore_ascii_case("nil"))
            .unwrap_or(true)
        })
    }
    _ => false,
  }
}

fn sanitize_orca_profile_map(raw: MapView, fallback_machine: Option<&MapView>) -> MapView {
  let mut out = BTreeMap::new();
  for (k, v) in raw {
    if k.ends_with("_gcode")
      || matches!(
        k.as_str(),
        "setting_id"
          | "filament_id"
          | "instantiation"
          | "compatible_printers"
          | "compatible_printers_condition"
          | "compatible_prints_condition"
          | "compatible_prints"
          | "filament_ramming_parameters"
          | "small_area_infill_flow_compensation_model"
          | "adaptive_pressure_advance_model"
          | "head_wrap_detect_zone"
          | "bed_exclude_area"
          | "post_process"
          | "upward_compatible_machine"
          | "wiping_volumes_extruders"
          | "filament_notes"
      )
    {
      continue;
    }
    if is_nil_or_empty_orca_value(&v) {
      continue;
    }
    out.insert(k, v);
  }
  if let Some(mach) = fallback_machine {
    for (fil_key, mach_key) in [
      ("filament_retraction_length", "retraction_length"),
      ("filament_retraction_speed", "retraction_speed"),
      ("filament_deretraction_speed", "deretraction_speed"),
      ("filament_retract_before_wipe", "retract_before_wipe"),
      ("filament_wipe", "wipe"),
      ("filament_wipe_distance", "wipe_distance"),
      ("filament_z_hop", "z_hop"),
      ("filament_z_hop_types", "z_hop_types"),
    ] {
      if !out.contains_key(fil_key) {
        if let Some(mv) = mach.get(mach_key) {
          out.insert(fil_key.to_string(), mv.clone());
        }
      }
    }
  }
  out
}

fn parse_build_volume(resolved: &MapView) -> String {
  let mut max_x: i32 = 250;
  let mut max_y: i32 = 250;
  if let Some(Value::Array(pts)) = resolved.get("printable_area") {
    let mut fx = 0i32;
    let mut fy = 0i32;
    for pt in pts {
      if let Some(s) = pt.as_str() {
        let parts: Vec<&str> = s.split('x').collect();
        if parts.len() == 2 {
          if let (Ok(x), Ok(y)) = (
            parts[0].trim().parse::<f32>(),
            parts[1].trim().parse::<f32>(),
          ) {
            fx = fx.max(x.round() as i32);
            fy = fy.max(y.round() as i32);
          }
        }
      }
    }
    if fx > 0 && fy > 0 {
      max_x = fx;
      max_y = fy;
    }
  }
  let max_z = first_str(resolved.get("printable_height"))
    .and_then(|s| s.trim().parse::<f32>().ok())
    .map(|v| v.round() as i32)
    .unwrap_or(250);
  format!("{} × {} × {} mm", max_x, max_y, max_z)
}

fn parse_diameters(raw: &str) -> Vec<f32> {
  let mut diams: Vec<f32> = raw
    .split(|c| c == ';' || c == ',')
    .filter_map(|s| s.trim().parse::<f32>().ok())
    .filter(|&d| d >= 0.1 && d <= 2.0)
    .collect();
  diams.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
  diams.dedup_by(|a, b| (*a - *b).abs() < 0.005);
  if diams.is_empty() {
    vec![0.4]
  } else {
    diams
  }
}

fn detect_finish_from_code(u: &str) -> &'static str {
  if u.contains("-CF") || u.ends_with("CF") {
    "Carbon Fiber"
  } else if u.contains("-GF") || u.ends_with("GF") {
    "Glass Fiber"
  } else if u.contains("WOOD") {
    "Wood-Filled"
  } else if u.contains("METAL")
    || u.contains("BRONZE")
    || u.contains("COPPER")
    || u.contains("STEEL")
  {
    "Metal-Filled"
  } else if u.contains("GLOW") || u.contains("LUMINOUS") {
    "Glow-in-the-Dark"
  } else if u.contains("AERO") || u.contains("FOAM") || u.contains("-LW") {
    "Foaming"
  } else if u.contains("CONDUCTIVE") || u.contains("ESD") {
    "Conductive"
  } else if u.contains("MARBLE") || u.contains("STONE") || u.contains("TERRACOTTA") {
    "Marble/Stone/Terracotta"
  } else if u.contains("PHOTOCHROMIC") || u.contains("THERMOCHROMIC") {
    "Photochromic"
  } else if u.contains("MATTE") {
    "Matte"
  } else if u.contains("SILK") {
    "Silk"
  } else if u.contains("TRANSLUCENT") {
    "Translucent"
  } else {
    "Basic"
  }
}

fn categorize_filament(ft: &str) -> (&'static str, &'static str, &'static str) {
  let u = ft.to_uppercase();
  let finish = detect_finish_from_code(&u);
  if u.contains("PVA")
    || u.contains("BVOH")
    || u.contains("HIPS")
    || u.contains("SUPPORT")
    || u.starts_with("S-")
  {
    let sup_finish = if u.contains("PVA") || u.contains("BVOH") {
      "Translucent"
    } else {
      finish
    };
    ("PVA / Support", sup_finish, "#8E9299")
  } else if u.contains("PEBA")
    || u.contains("TPU")
    || u.contains("TPE")
    || u.contains("FLEX")
    || u.contains("EVA")
  {
    ("TPU / TPE / PEBA Flex", finish, "#8E9299")
  } else if u.contains("PA") || u.contains("PPA") || u.contains("PHTA") {
    ("Nylon (PA / PAHT / PPA)", finish, "#8E9299")
  } else if u.contains("PETG") || u.contains("PCTG") || u.starts_with("PET") || u.contains("COPE") {
    ("PETG / PCTG / PET", finish, "#8E9299")
  } else if u.contains("PC")
    || u.contains("PPS")
    || u.contains("PEEK")
    || u.starts_with("PP")
    || u == "PE"
    || u.starts_with("PE-")
    || u.contains("PVB")
  {
    ("PC / Engineering", finish, "#8E9299")
  } else if u.contains("ABS") || u.contains("ASA") || u.contains("SBS") {
    ("ABS / ASA", finish, "#8E9299")
  } else {
    ("PLA Family", finish, "#8E9299")
  }
}

fn main() {
  let args: Vec<String> = env::args().collect();
  if args.len() < 3 {
    eprintln!("Usage: generate_orca_catalog <output.json> <profiles_root_dir>");
    std::process::exit(1);
  }
  let out_path = PathBuf::from(&args[1]);
  let profiles_dir = PathBuf::from(&args[2]);

  // 1. Scan vendor directories for machines
  let mut machines: Vec<CatalogMachine> = Vec::new();
  if let Ok(vendor_entries) = fs::read_dir(&profiles_dir) {
    let mut vendor_dirs: Vec<PathBuf> = vendor_entries
      .filter_map(|e| e.ok().map(|e| e.path()))
      .filter(|p| {
        p.is_dir() && p.file_name().and_then(|s| s.to_str()) != Some("OrcaFilamentLibrary")
      })
      .collect();
    vendor_dirs.sort();

    for vdir in vendor_dirs {
      let raw_vendor = vdir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Generic");
      let vendor = normalize_vendor_name(raw_vendor);
      let machine_dir = vdir.join("machine");
      if !machine_dir.is_dir() {
        continue;
      }

      let mut machine_files = Vec::new();
      collect_json_files(&machine_dir, &mut machine_files);

      let mut by_name: HashMap<String, Value> = HashMap::new();
      let mut models: Vec<Value> = Vec::new();

      for mf in &machine_files {
        if let Ok(txt) = fs::read_to_string(mf) {
          if let Ok(val) = serde_json::from_str::<Value>(&txt) {
            if val.get("type").and_then(|v| v.as_str()) == Some("machine_model") {
              let tech = val
                .get("machine_tech")
                .and_then(|v| v.as_str())
                .unwrap_or("FFF");
              if tech == "FFF" {
                models.push(val.clone());
              }
            }
            if let Some(name) = val.get("name").and_then(|v| v.as_str()) {
              by_name.insert(name.to_string(), val);
            }
          }
        }
      }

      for model in models {
        let Some(model_name) = model.get("name").and_then(|v| v.as_str()) else {
          continue;
        };
        let diams_raw = model
          .get("nozzle_diameter")
          .and_then(|v| v.as_str())
          .unwrap_or("0.4");
        let available_nozzle_diameters = parse_diameters(diams_raw);
        let default_nozzle_diameter = if available_nozzle_diameters.contains(&0.4) {
          0.4
        } else {
          available_nozzle_diameters[0]
        };

        // Resolve default variant machine profile to inspect printable_area, gcode_flavor, nozzle_type
        let variant_candidates = [
          format!("{} 0.4 nozzle", model_name),
          format!("{} {} nozzle", model_name, default_nozzle_diameter),
          model_name.to_string(),
        ];
        let mut resolved = BTreeMap::new();
        for cand in &variant_candidates {
          if by_name.contains_key(cand) {
            resolved = resolve_profile(cand, &by_name, 0);
            if resolved.contains_key("printable_area") {
              break;
            }
          }
        }
        // Fallback: search any instantiated machine referencing printer_model == model_name
        if !resolved.contains_key("printable_area") {
          for (k, v) in &by_name {
            if v.get("printer_model").and_then(|s| s.as_str()) == Some(model_name) {
              resolved = resolve_profile(k, &by_name, 0);
              if resolved.contains_key("printable_area") {
                break;
              }
            }
          }
        }

        let build_volume = parse_build_volume(&resolved);
        let gcode_flavor =
          first_str(resolved.get("gcode_flavor")).unwrap_or_else(|| "klipper".to_string());
        let aux_fan = first_str(resolved.get("auxiliary_fan")).unwrap_or_default() == "1";
        let kinematics = format!(
          "{}{}",
          if gcode_flavor.eq_ignore_ascii_case("marlin") {
            "Marlin FFF"
          } else if gcode_flavor.eq_ignore_ascii_case("bambu") {
            "CoreXY High-Speed"
          } else {
            "Klipper High-Speed"
          },
          if aux_fan { " • Aux Cooling" } else { "" }
        );

        let raw_nozzle_type = first_str(resolved.get("nozzle_type"))
          .unwrap_or_else(|| "brass".to_string())
          .to_lowercase();
        let default_nozzle_material = if raw_nozzle_type.contains("hardened") {
          "Hardened Steel".to_string()
        } else if raw_nozzle_type.contains("stainless") {
          "Stainless Steel".to_string()
        } else {
          "Brass".to_string()
        };

        let id = format!(
          "{}_{}",
          vendor
            .to_lowercase()
            .replace(|c: char| !c.is_ascii_alphanumeric(), "_"),
          model_name
            .to_lowercase()
            .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        );

        let supported_bed_ids = match vendor.as_str() {
          "Bambu Lab" => vec![
            "textured_pei".to_string(),
            "smooth_pei".to_string(),
            "bambu_cool_plate".to_string(),
            "bambu_eng_plate".to_string(),
            "orca_supertack".to_string(),
            "cryogrip_supertack".to_string(),
            "g10_garolite".to_string(),
            "peo_pey_effect".to_string(),
          ],
          "Prusa" => vec![
            "textured_pei".to_string(),
            "smooth_pei".to_string(),
            "prusa_satin".to_string(),
            "prusa_pp".to_string(),
            "g10_garolite".to_string(),
            "peo_pey_effect".to_string(),
          ],
          _ => vec![
            "textured_pei".to_string(),
            "smooth_pei".to_string(),
            "cryogrip_supertack".to_string(),
            "g10_garolite".to_string(),
            "peo_pey_effect".to_string(),
          ],
        };

        let (default_ams_id, supported_ams_ids) = match vendor.as_str() {
          "Anycubic" => {
            if model_name.contains("Kobra S1")
              || model_name.contains("Kobra 3")
              || model_name.contains("Kobra X")
            {
              (
                "ace_pro_single".to_string(),
                vec![
                  "ace_pro_single".to_string(),
                  "ace_pro_double".to_string(),
                  "none_single_spool".to_string(),
                ],
              )
            } else {
              (
                "none_single_spool".to_string(),
                vec!["none_single_spool".to_string()],
              )
            }
          }
          "Bambu Lab" => {
            if model_name.contains("A1") {
              (
                "bambu_ams_lite".to_string(),
                vec![
                  "bambu_ams_lite".to_string(),
                  "none_single_spool".to_string(),
                ],
              )
            } else {
              (
                "bambu_ams_1".to_string(),
                vec![
                  "bambu_ams_1".to_string(),
                  "bambu_ams_2".to_string(),
                  "none_single_spool".to_string(),
                ],
              )
            }
          }
          "Prusa" => (
            "prusa_mmu3".to_string(),
            vec!["prusa_mmu3".to_string(), "none_single_spool".to_string()],
          ),
          "Creality" => {
            if model_name.contains("K2") || model_name.contains("Hi") {
              (
                "creality_cfs".to_string(),
                vec!["creality_cfs".to_string(), "none_single_spool".to_string()],
              )
            } else if model_name.contains("K1") {
              (
                "none_single_spool".to_string(),
                vec!["creality_cfs".to_string(), "none_single_spool".to_string()],
              )
            } else {
              (
                "none_single_spool".to_string(),
                vec!["none_single_spool".to_string()],
              )
            }
          }
          "Voron" | "RatRig" | "Sovol" | "Custom / Generic" => (
            "none_single_spool".to_string(),
            vec![
              "klipper_boxturtle".to_string(),
              "none_single_spool".to_string(),
            ],
          ),
          _ => (
            "none_single_spool".to_string(),
            vec!["none_single_spool".to_string()],
          ),
        };

        machines.push(CatalogMachine {
          id,
          vendor: vendor.clone(),
          name: model_name.to_string(),
          kinematics,
          build_volume,
          default_nozzle_diameter,
          available_nozzle_diameters,
          default_nozzle_material,
          default_bed_id: "textured_pei".to_string(),
          supported_bed_ids,
          default_ams_id,
          supported_ams_ids,
        });
      }
    }
  }

  // Sort machines so Anycubic Kobra S1 is first, followed by Kobra 3, Bambu Lab, etc.
  machines.sort_by(|a, b| {
    let vp_a = vendor_priority(&a.vendor);
    let vp_b = vendor_priority(&b.vendor);
    if vp_a != vp_b {
      return vp_a.cmp(&vp_b);
    }
    let kobra_s1_a = if a.name == "Anycubic Kobra S1" {
      0
    } else if a.name.contains("Kobra S1") {
      1
    } else if a.name.contains("Kobra 3") {
      2
    } else {
      10
    };
    let kobra_s1_b = if b.name == "Anycubic Kobra S1" {
      0
    } else if b.name.contains("Kobra S1") {
      1
    } else if b.name.contains("Kobra 3") {
      2
    } else {
      10
    };
    kobra_s1_a
      .cmp(&kobra_s1_b)
      .then_with(|| a.name.cmp(&b.name))
  });

  // 2. Stock OrcaSlicer Nozzles (Standard Brass is first as the OrcaSlicer default)
  let nozzles = vec![
    CatalogNozzle {
      id: "orca_brass".to_string(),
      brand: "OrcaSlicer Standard".to_string(),
      name: "Standard Brass Nozzle (OrcaSlicer Default)".to_string(),
      material: "Brass".to_string(),
      is_high_flow: false,
      base_flow_mm3s: 21.0,
      retraction_mm: 0.8,
      melt_zone_mult: 1.0,
      temp_offset_c: 0,
      description:
        "Baseline 0°C thermal reference (OrcaSlicer default nozzle) • Not recommended for abrasive -CF / -GF filaments"
          .to_string(),
      available_diameters: vec![0.2, 0.25, 0.4, 0.6, 0.8],
      compatible_vendors: vec![],
      source: "OrcaSlicer GitHub".to_string(),
    },
    CatalogNozzle {
      id: "orca_hardened_steel".to_string(),
      brand: "OrcaSlicer Standard".to_string(),
      name: "Hardened Steel Nozzle (OrcaSlicer Standard)".to_string(),
      material: "Hardened Steel".to_string(),
      is_high_flow: false,
      base_flow_mm3s: 20.0,
      retraction_mm: 0.8,
      melt_zone_mult: 1.0,
      temp_offset_c: 5,
      description: "+5°C Lua Thermal Rule • Abrasion-proof for -CF / -GF / Glow filaments"
        .to_string(),
      available_diameters: vec![0.25, 0.4, 0.6, 0.8],
      compatible_vendors: vec![],
      source: "OrcaSlicer GitHub".to_string(),
    },
    CatalogNozzle {
      id: "orca_stainless_steel".to_string(),
      brand: "OrcaSlicer Standard".to_string(),
      name: "Stainless Steel Nozzle (0.2mm / 0.4mm Fine Detail & Food-Safe)".to_string(),
      material: "Stainless Steel".to_string(),
      is_high_flow: false,
      base_flow_mm3s: 15.0,
      retraction_mm: 0.8,
      melt_zone_mult: 0.95,
      temp_offset_c: 5,
      description:
        "Lead-free stainless steel • Standard for 0.2mm Bambu Lab & fine miniature printing"
          .to_string(),
      available_diameters: vec![0.2, 0.4],
      compatible_vendors: vec![],
      source: "OrcaSlicer GitHub".to_string(),
    },
  ];

  // 3. Stock OrcaSlicer Build Plates (from fdm_filament_common.json plate keys)
  let beds = vec![
    CatalogBed {
      id: "textured_pei".to_string(),
      name: "Textured PEI Spring Steel Plate (OrcaSlicer textured_plate_temp)".to_string(),
      surface_code: "Textured PEI".to_string(),
      temp_offset_c: 0,
      description: "Baseline 0°C offset • Standard powder-coated PEI for PLA, PETG, ABS, ASA, TPU, PA".to_string(),
      compatible_vendors: vec![],
      source: "OrcaSlicer GitHub".to_string(),
    },
    CatalogBed {
      id: "smooth_pei".to_string(),
      name: "Smooth PEI / High-Temp Plate (OrcaSlicer hot_plate_temp)".to_string(),
      surface_code: "Smooth PEI".to_string(),
      temp_offset_c: 0,
      description: "Smooth specular bottom surface • Use release agent for PETG / PC / TPU".to_string(),
      compatible_vendors: vec![],
      source: "OrcaSlicer GitHub".to_string(),
    },
    CatalogBed {
      id: "orca_supertack".to_string(),
      name: "Bambu Cool Plate SuperTack (OrcaSlicer supertack_plate_temp)".to_string(),
      surface_code: "Cool Plate SuperTack".to_string(),
      temp_offset_c: -15,
      description: "OrcaSlicer supertack_plate_temp (-15°C offset: 45°C PLA, energy-saving zero-elephant-foot adhesion)".to_string(),
      compatible_vendors: vec!["Bambu Lab".to_string()],
      source: "OrcaSlicer GitHub".to_string(),
    },
    CatalogBed {
      id: "bambu_cool_plate".to_string(),
      name: "Cool Plate (OrcaSlicer cool_plate_temp — PLA Low-Temp)".to_string(),
      surface_code: "Cool Plate".to_string(),
      temp_offset_c: -20,
      description: "OrcaSlicer cool_plate_temp (-20°C offset: 35–40°C PLA printing to prevent heat creep)".to_string(),
      compatible_vendors: vec!["Bambu Lab".to_string()],
      source: "OrcaSlicer GitHub".to_string(),
    },
    CatalogBed {
      id: "bambu_eng_plate".to_string(),
      name: "Engineering Plate (OrcaSlicer eng_plate_temp — PC / PA / ABS / TPU)".to_string(),
      surface_code: "Engineering Plate".to_string(),
      temp_offset_c: 5,
      description: "OrcaSlicer eng_plate_temp (+5°C offset: high-temperature coated surface for PC, PAHT-CF, PPA-CF)".to_string(),
      compatible_vendors: vec!["Bambu Lab".to_string()],
      source: "OrcaSlicer GitHub".to_string(),
    },
  ];

  // 4. Scan filament profiles per vendor directory (including OrcaFilamentLibrary) so `inherits` chains
  // resolve within their own vendor tree without cross-vendor name collisions.
  let mut best_by_code: BTreeMap<String, (usize, CatalogFilament)> = BTreeMap::new();

  let mut lib_fil_by_name: HashMap<String, Value> = HashMap::new();
  let lib_dir = profiles_dir.join("OrcaFilamentLibrary");
  if lib_dir.is_dir() {
    let mut lib_files = Vec::new();
    collect_json_files(&lib_dir, &mut lib_files);
    for lf in &lib_files {
      if let Ok(txt) = fs::read_to_string(lf) {
        if let Ok(val) = serde_json::from_str::<Value>(&txt) {
          if val.get("type").and_then(|v| v.as_str()) == Some("filament") {
            if let Some(name) = val.get("name").and_then(|v| v.as_str()) {
              lib_fil_by_name.insert(name.to_string(), val);
            }
          }
        }
      }
    }
  }

  let mut all_vendor_fil_maps: Vec<HashMap<String, Value>> = Vec::new();
  if !lib_fil_by_name.is_empty() {
    all_vendor_fil_maps.push(lib_fil_by_name.clone());
  }
  if let Ok(vendor_entries) = fs::read_dir(&profiles_dir) {
    let mut vdirs: Vec<PathBuf> = vendor_entries
      .filter_map(|e| e.ok().map(|e| e.path()))
      .filter(|p| {
        p.is_dir() && p.file_name().and_then(|s| s.to_str()) != Some("OrcaFilamentLibrary")
      })
      .collect();
    vdirs.sort();
    for vd in vdirs {
      let fdir = vd.join("filament");
      if !fdir.is_dir() {
        continue;
      }
      let mut ffiles = Vec::new();
      collect_json_files(&fdir, &mut ffiles);
      let mut vmap = lib_fil_by_name.clone();
      for ff in &ffiles {
        if let Ok(txt) = fs::read_to_string(ff) {
          if let Ok(val) = serde_json::from_str::<Value>(&txt) {
            if val.get("type").and_then(|v| v.as_str()) == Some("filament") {
              if let Some(name) = val.get("name").and_then(|v| v.as_str()) {
                vmap.insert(name.to_string(), val);
              }
            }
          }
        }
      }
      all_vendor_fil_maps.push(vmap);
    }
  }

  for fil_by_name in &all_vendor_fil_maps {
    for (name, _) in fil_by_name {
      let resolved = resolve_profile(name, fil_by_name, 0);
      let Some(ft_raw) = first_str(resolved.get("filament_type")) else {
        continue;
      };
      let ft = match ft_raw.trim() {
        "TPU-95A" => "TPU 95A",
        "TPU-90A" => "TPU 90A",
        "TPU-64D" => "TPU 64D",
        "TPU-AMS" => "TPU for AMS",
        "SILK" => "PLA Silk",
        "PLA Basic" => "PLA",
        "PLA Basic+" => "PLA+",
        "PETG Basic" | "PETG Polymax" => "PETG",
        "PETG-CF10" => "PETG-CF",
        "S-PAHT" | "PAHT-S" => "Support for PAHT",
        "PA-S" => "Support for PA/PET",
        "S-Multi" => "Breakaway Support",
        other => other,
      };
      if ft.is_empty()
        || ft == "UNIV"
        || ft.contains("Material4Print")
        || ft.contains("HPP4GF25")
        || ft.contains("VXL90")
        || ft.contains("PACF Pro")
      {
        continue;
      }

      let explicit_nozzle = first_str(resolved.get("nozzle_temperature_BRASS"))
        .or_else(|| first_str(resolved.get("nozzle_temperature")))
        .and_then(|s| s.parse::<f32>().ok())
        .map(|v| v.round() as u16);

      // Score higher for profiles that actually resolve a temperature + Anycubic Kobra S1 / Generic @System / Bambu @System
      let base_score = if name.contains("@Anycubic Kobra S1") {
        110
      } else if name.starts_with("Generic ") && name.contains("@System") {
        100
      } else if name.starts_with("Bambu ") && name.contains("@System") {
        90
      } else if name.contains("@System") {
        80
      } else if name.contains("@base") {
        70
      } else if name.starts_with("fdm_filament_") {
        60
      } else {
        20
      };
      let score = if explicit_nozzle.is_some() {
        base_score + 200
      } else {
        base_score
      };

      // Also detect specialized variants from profile name (e.g. PLA+, PLA High Speed, PLA Matte, PLA Silk, PLA Glow, PETG HF, PETG Translucent, TPU 95A HF)
      let mut codes = vec![ft.to_string()];
      let name_up = name.to_uppercase();
      if ft == "PLA" {
        if name_up.contains("PLA+") || name_up.contains("PLA PLUS") {
          codes.push("PLA+".to_string());
        } else if name_up.contains("HIGH SPEED") || name_up.contains("PLA-HS") {
          codes.push("PLA High Speed".to_string());
        } else if name_up.contains("MATTE") {
          codes.push("PLA Matte".to_string());
        } else if name_up.contains("SILK") {
          codes.push("PLA Silk".to_string());
        } else if name_up.contains("GLOW") || name_up.contains("LUMINOUS") {
          codes.push("PLA Glow".to_string());
        }
      } else if ft == "PETG" {
        if name_up.contains("PETG HF") || name_up.contains("HIGH SPEED") {
          codes.push("PETG HF".to_string());
        } else if name_up.contains("TRANSLUCENT") {
          codes.push("PETG Translucent".to_string());
        }
      } else if ft == "TPU" {
        if name_up.contains("95A HF") {
          codes.push("TPU 95A HF".to_string());
        } else if name_up.contains("95A") {
          codes.push("TPU 95A".to_string());
        }
      }

      let hrc = first_str(resolved.get("required_nozzle_HRC"))
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(3);

      for code in codes {
        let (category, default_finish, default_color) = categorize_filament(&code);
        let (fallback_nozzle_c, fallback_bed_c, fallback_fan_pct): (u16, u16, u8) = match category {
          "PC / Engineering" => {
            if code.starts_with("PP") && !code.starts_with("PPS") {
              (230, 85, 70)
            } else if code.starts_with("PPS") {
              (315, 105, 20)
            } else if code.contains("PEEK") {
              (340, 120, 15)
            } else {
              (270, 100, 20)
            }
          }
          "Nylon (PA / PAHT / PPA)" => (280, 90, 15),
          "ABS / ASA" => (255, 100, 25),
          "PETG / PCTG / PET" => {
            if code.contains("PCTG") {
              (255, 80, 40)
            } else if code.starts_with("PET-") {
              (280, 90, 25)
            } else {
              (235, 70, 40)
            }
          }
          "TPU / TPE / PEBA Flex" => (225, 50, 80),
          _ => (215, 55, 100),
        };

        let raw_nozzle_c = explicit_nozzle.unwrap_or(fallback_nozzle_c).clamp(170, 350);
        // Guard against broken vendor profiles or overly hot Generic @System defaults so Brass + 5°C (HS) stays in safe polymer range
        let nozzle_c =
          if category == "PC / Engineering" && code.contains("PEEK") && raw_nozzle_c < 325 {
            fallback_nozzle_c
          } else if category == "PC / Engineering" && code.starts_with("PPS") && raw_nozzle_c < 300
          {
            fallback_nozzle_c
          } else if (category == "PC / Engineering"
            && code.starts_with("PC")
            && !(260..=285).contains(&raw_nozzle_c))
            || (category == "Nylon (PA / PAHT / PPA)" && !(260..=285).contains(&raw_nozzle_c))
            || (category == "ABS / ASA" && !(240..=265).contains(&raw_nozzle_c))
            || (category == "PETG / PCTG / PET"
              && code.starts_with("PETG")
              && !code.contains("ESD")
              && !(225..=245).contains(&raw_nozzle_c))
            || (category == "TPU / TPE / PEBA Flex" && !(205..=228).contains(&raw_nozzle_c))
            || (category == "PLA Family" && !(195..=220).contains(&raw_nozzle_c))
          {
            fallback_nozzle_c
          } else {
            raw_nozzle_c
          };

        let raw_bed_c = first_str(resolved.get("textured_plate_temp"))
          .or_else(|| first_str(resolved.get("hot_plate_temp")))
          .and_then(|s| s.parse::<f32>().ok())
          .map(|v| v.round() as u16)
          .unwrap_or(fallback_bed_c)
          .clamp(30, 130);
        let bed_c = if (category == "PC / Engineering"
          && code.starts_with("PC")
          && !(90..=115).contains(&raw_bed_c))
          || (category == "Nylon (PA / PAHT / PPA)" && !(75..=110).contains(&raw_bed_c))
          || (category == "ABS / ASA" && !(85..=110).contains(&raw_bed_c))
          || (category == "PETG / PCTG / PET" && !(65..=85).contains(&raw_bed_c))
          || (category == "TPU / TPE / PEBA Flex" && raw_bed_c > 60)
          || (category == "PLA Family" && raw_bed_c > 65)
        {
          fallback_bed_c
        } else {
          raw_bed_c
        };

        let raw_max_vol = first_str(resolved.get("filament_max_volumetric_speed"))
          .and_then(|s| s.parse::<f32>().ok())
          .unwrap_or(12.0);
        let max_vol = if category != "TPU / TPE / PEBA Flex"
          && category != "PVA / Support"
          && raw_max_vol < 6.0
        {
          12.0
        } else {
          raw_max_vol.clamp(1.5, 45.0)
        };
        let raw_fan_pct = first_str(resolved.get("fan_max_speed"))
          .and_then(|s| s.parse::<f32>().ok())
          .map(|v| v.round() as u8)
          .unwrap_or(fallback_fan_pct)
          .min(100);
        let fan_pct =
          if (category == "PC / Engineering" && !code.starts_with("PP") && raw_fan_pct > 50)
            || (category == "Nylon (PA / PAHT / PPA)" && raw_fan_pct > 50)
            || (category == "ABS / ASA" && raw_fan_pct > 50)
            || (category == "PETG / PCTG / PET" && raw_fan_pct > 70)
          {
            fallback_fan_pct
          } else {
            raw_fan_pct
          };

        let requires_hardened =
          hrc > 10 || code.contains("-CF") || code.contains("-GF") || code.contains("Glow");
        let (dry_temp_c, dry_hr) = match category {
          "PLA Family" => (45, 4),
          "PETG / PCTG / PET" => (60, 6),
          "ABS / ASA" => (65, 6),
          "PC / Engineering" => (80, 8),
          "Nylon (PA / PAHT / PPA)" => (80, 12),
          "TPU / TPE / PEBA Flex" => (55, 6),
          _ => (50, 6),
        };

        let default_flow_ratio = first_str(resolved.get("filament_flow_ratio"))
          .and_then(|s| s.parse::<f32>().ok())
          .unwrap_or(0.98)
          .clamp(0.50, 1.05);
        let default_retraction_mm = first_str(resolved.get("filament_retraction_length"))
          .or_else(|| first_str(resolved.get("retraction_length")))
          .and_then(|s| s.parse::<f32>().ok())
          .unwrap_or(0.8)
          .clamp(0.0, 3.0);

        let entry = CatalogFilament {
          brand: "Generic".to_string(),
          code: code.clone(),
          label: format!("{} (OrcaSlicer {})", code, ft),
          category: category.to_string(),
          default_finish: default_finish.to_string(),
          default_color: default_color.to_string(),
          base_nozzle_c: nozzle_c,
          base_bed_c: bed_c,
          max_vol_mm3s: max_vol,
          fan_pct,
          default_flow_ratio,
          default_retraction_mm,
          dry_temp_c,
          dry_hr,
          requires_hardened,
          source: "OrcaSlicer GitHub".to_string(),
        };

        let replace = match best_by_code.get(&code) {
          None => true,
          Some((prev_score, _)) => score > *prev_score,
        };
        if replace {
          best_by_code.insert(code.clone(), (score, entry.clone()));
        }

        // Also capture vendor-branded profiles from @orcaslicer_repo (Anycubic, Bambu Lab, eSUN, Overture)
        let explicit_vendor = first_str(resolved.get("filament_vendor"))
          .unwrap_or_default()
          .trim()
          .to_string();
        let detected_brand = if explicit_vendor.eq_ignore_ascii_case("Anycubic")
          || name.starts_with("Anycubic ")
        {
          Some("Anycubic")
        } else if explicit_vendor.eq_ignore_ascii_case("Bambu")
          || explicit_vendor.eq_ignore_ascii_case("Bambu Lab")
          || name.starts_with("Bambu ")
        {
          Some("Bambu Lab")
        } else if explicit_vendor.eq_ignore_ascii_case("eSUN") || name.starts_with("eSUN ") {
          Some("eSUN")
        } else if explicit_vendor.eq_ignore_ascii_case("Overture") || name.starts_with("Overture ")
        {
          Some("Overture")
        } else {
          None
        };
        if let Some(vb) = detected_brand {
          if explicit_nozzle.is_some() {
            let vkey = format!("{}::{}", vb, code);
            let mut v_entry = entry.clone();
            v_entry.brand = vb.to_string();
            v_entry.label = format!("{} {} (OrcaSlicer {})", vb, code, vb);
            v_entry.source = format!("OrcaSlicer GitHub ({})", vb);
            let v_replace = match best_by_code.get(&vkey) {
              None => true,
              Some((prev_s, _)) => score > *prev_s,
            };
            if v_replace {
              best_by_code.insert(vkey, (score, v_entry));
            }
          }
        }
      }
    }
  }

  // Ensure "PETG" uses Anycubic Kobra S1's 230°C Brass base (235°C HS), 70°C bed, 12.0 mm³/s, 40% fan,
  // and "PETG Translucent" uses tuned low-stringing / easy-PEI-release values (230°C Brass / 235°C HS,
  // 65°C Textured PEI bed, 9.0 mm³/s volumetric speed, 30% fan, 65°C drying).
  if let Some((_, petg)) = best_by_code.get_mut("PETG") {
    petg.base_nozzle_c = 230;
    petg.base_bed_c = 70;
    petg.max_vol_mm3s = 12.0;
    petg.fan_pct = 40;
    petg.dry_temp_c = 60;
    petg.dry_hr = 6;
  }
  for (child_code, parent_code, child_finish) in [
    ("PETG Translucent", "PETG", "Translucent"),
    ("PLA Matte", "PLA", "Matte"),
    ("PLA Silk", "PLA", "Silk"),
  ] {
    if let Some((_, parent)) = best_by_code.get(parent_code).cloned() {
      if let Some((_, child)) = best_by_code.get_mut(child_code) {
        child.base_nozzle_c = parent.base_nozzle_c;
        child.base_bed_c = if child_code == "PETG Translucent" {
          65
        } else {
          parent.base_bed_c
        };
        child.max_vol_mm3s = if child_code == "PETG Translucent" {
          9.0
        } else {
          parent.max_vol_mm3s
        };
        child.fan_pct = if child_code == "PETG Translucent" {
          30
        } else {
          parent.fan_pct
        };
        child.dry_temp_c = if child_code == "PETG Translucent" {
          65
        } else {
          parent.dry_temp_c
        };
        child.dry_hr = parent.dry_hr;
        child.requires_hardened = parent.requires_hardened;
        child.default_finish = child_finish.to_string();
      }
    }
  }

  let filaments: Vec<CatalogFilament> = best_by_code.into_values().map(|(_, v)| v).collect();

  // 5. Resolve full OrcaSlicer base profiles (Machine, Process, Filament, Voron) from @orcaslicer_repo
  let anycubic_dir = profiles_dir.join("Anycubic");
  let anycubic_mach_map = load_dir_profile_map(&anycubic_dir.join("machine"), HashMap::new());
  let anycubic_proc_map = load_dir_profile_map(&anycubic_dir.join("process"), HashMap::new());
  let anycubic_fil_map =
    load_dir_profile_map(&anycubic_dir.join("filament"), lib_fil_by_name.clone());

  let default_machine_base = sanitize_orca_profile_map(
    resolve_profile("Anycubic Kobra S1 0.4 nozzle", &anycubic_mach_map, 0),
    None,
  );
  let default_process_base = sanitize_orca_profile_map(
    resolve_profile(
      "0.20mm Standard @Anycubic Kobra S1 0.4 nozzle",
      &anycubic_proc_map,
      0,
    ),
    None,
  );

  let mut default_filament_bases: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
  for (code, profile_name) in [
    ("PLA", "Anycubic PLA @Anycubic Kobra S1 0.4 nozzle"),
    ("PETG", "Anycubic PETG @Anycubic Kobra S1 0.4 nozzle"),
    ("ABS", "Anycubic ABS @Anycubic Kobra S1 0.4 nozzle"),
    ("ASA", "Anycubic ASA @Anycubic Kobra S1 0.4 nozzle"),
    ("TPU", "Anycubic TPU @Anycubic Kobra S1 0.4 nozzle"),
    ("PA", "Anycubic Generic PA"),
    ("PA-CF", "Anycubic Generic PA-CF"),
    ("PC", "Anycubic Generic PC"),
    ("PLA-CF", "Anycubic Generic PLA-CF"),
    ("PVA", "Anycubic Generic PVA"),
  ] {
    let mut resolved_fil = sanitize_orca_profile_map(
      resolve_profile(profile_name, &anycubic_fil_map, 0),
      Some(&default_machine_base),
    );
    if let Some(mach_ret_len) = default_machine_base.get("retraction_length") {
      resolved_fil.insert(
        "filament_retraction_length".to_string(),
        mach_ret_len.clone(),
      );
    }
    if let Some(mach_ret_spd) = default_machine_base.get("retraction_speed") {
      resolved_fil.insert(
        "filament_retraction_speed".to_string(),
        mach_ret_spd.clone(),
      );
    }
    if let Some(cat_fil) = filaments.iter().find(|f| f.code == code) {
      let noz_str = cat_fil.base_nozzle_c.to_string();
      let bed_str = cat_fil.base_bed_c.to_string();
      let mvs_str = format!("{:.1}", cat_fil.max_vol_mm3s);
      let fan_str = cat_fil.fan_pct.to_string();
      let hrc_str = if cat_fil.requires_hardened { "55" } else { "3" };
      resolved_fil.insert(
        "nozzle_temperature".to_string(),
        serde_json::json!([noz_str]),
      );
      resolved_fil.insert(
        "nozzle_temperature_initial_layer".to_string(),
        serde_json::json!([noz_str]),
      );
      resolved_fil.insert("hot_plate_temp".to_string(), serde_json::json!([bed_str]));
      resolved_fil.insert(
        "hot_plate_temp_initial_layer".to_string(),
        serde_json::json!([bed_str]),
      );
      resolved_fil.insert(
        "textured_plate_temp".to_string(),
        serde_json::json!([bed_str]),
      );
      resolved_fil.insert(
        "textured_plate_temp_initial_layer".to_string(),
        serde_json::json!([bed_str]),
      );
      resolved_fil.insert(
        "filament_max_volumetric_speed".to_string(),
        serde_json::json!([mvs_str]),
      );
      resolved_fil.insert("fan_max_speed".to_string(), serde_json::json!([fan_str]));
      resolved_fil.insert(
        "required_nozzle_HRC".to_string(),
        serde_json::json!([hrc_str]),
      );
    }
    default_filament_bases.insert(code.to_string(), resolved_fil);
  }

  let voron_dir = profiles_dir.join("Voron");
  let voron_mach_map = load_dir_profile_map(&voron_dir.join("machine"), HashMap::new());
  let voron_proc_map = load_dir_profile_map(&voron_dir.join("process"), HashMap::new());
  let mut default_voron_base = sanitize_orca_profile_map(
    resolve_profile("Voron 2.4 350 0.4 nozzle", &voron_mach_map, 0),
    None,
  );
  for (k, v) in sanitize_orca_profile_map(
    resolve_profile("0.20mm Standard @Voron", &voron_proc_map, 0),
    None,
  ) {
    default_voron_base.entry(k).or_insert(v);
  }
  if let Some(abs_fil) = default_filament_bases.get("ABS") {
    for (k, v) in abs_fil {
      default_voron_base
        .entry(k.clone())
        .or_insert_with(|| v.clone());
    }
  }

  let catalog = GeneratedCatalog {
    machines,
    nozzles,
    beds,
    ams_options: Vec::new(),
    filaments,
    standard_colors: Vec::new(),
    default_machine_base,
    default_process_base,
    default_filament_bases,
    default_voron_base,
  };

  let json_out = serde_json::to_string_pretty(&catalog).expect("Failed to serialize catalog");
  fs::write(&out_path, json_out).expect("Failed to write output catalog JSON");
}
