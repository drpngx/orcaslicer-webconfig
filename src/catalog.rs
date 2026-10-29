use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const ORCA_GENERATED_JSON: &str = include_str!(env!("ORCASLICER_GENERATED_CATALOG"));
const ADDITIONAL_CATALOG_JSON: &str = include_str!("../additional_catalog.json");

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CombinedCatalog {
  #[serde(default)]
  pub machines: Vec<CatalogMachine>,
  #[serde(default)]
  pub nozzles: Vec<CatalogNozzle>,
  #[serde(default)]
  pub beds: Vec<CatalogBed>,
  #[serde(default)]
  pub ams_options: Vec<CatalogAmsOption>,
  #[serde(default)]
  pub filaments: Vec<CatalogFilament>,
  #[serde(default)]
  pub standard_colors: Vec<CatalogColorSwatch>,
  #[serde(default)]
  pub default_machine_base: BTreeMap<String, serde_json::Value>,
  #[serde(default)]
  pub default_process_base: BTreeMap<String, serde_json::Value>,
  #[serde(default)]
  pub default_filament_bases: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
  #[serde(default)]
  pub default_voron_base: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
  #[serde(default)]
  pub supported_ams_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
  #[serde(default)]
  pub compatible_vendors: Vec<String>,
  #[serde(default)]
  pub source: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogBed {
  pub id: String,
  pub name: String,
  pub surface_code: String,
  pub temp_offset_c: i32,
  pub description: String,
  #[serde(default)]
  pub compatible_vendors: Vec<String>,
  #[serde(default)]
  pub source: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogFilament {
  pub code: String,
  pub label: String,
  pub category: String,
  pub default_finish: String,
  pub default_color: String,
  pub base_nozzle_c: u16,
  pub base_bed_c: u16,
  pub max_vol_mm3s: f32,
  pub fan_pct: u8,
  pub dry_temp_c: u16,
  pub dry_hr: u8,
  pub requires_hardened: bool,
  #[serde(default)]
  pub source: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogColorSwatch {
  pub name: String,
  pub hex: String,
}

impl CatalogMachine {
  pub fn is_bed_compatible(&self, bed: &CatalogBed) -> bool {
    if !self.supported_bed_ids.is_empty() {
      self.supported_bed_ids.iter().any(|id| id == &bed.id)
    } else {
      bed.compatible_vendors.is_empty() || bed.compatible_vendors.iter().any(|v| v == &self.vendor)
    }
  }

  pub fn is_ams_compatible(&self, ams: &CatalogAmsOption) -> bool {
    if !self.supported_ams_ids.is_empty() {
      self.supported_ams_ids.iter().any(|id| id == &ams.id)
    } else {
      ams.compatible_vendors.is_empty() || ams.compatible_vendors.iter().any(|v| v == &self.vendor)
    }
  }
}

impl CatalogNozzle {
  pub fn supports_diameter(&self, d: f32) -> bool {
    self
      .available_diameters
      .iter()
      .any(|&x| (x - d).abs() < 0.005)
  }
}

impl CatalogFilament {
  /// Returns the base polymer code without finish suffixes (e.g., `"PETG Translucent"` -> `"PETG"`,
  /// `"PLA Matte"` -> `"PLA"`, `"PLA Silk"` -> `"PLA"`), so finish variants are modeled as
  /// `base_material_code() + default_finish` (e.g., `PETG` / `Translucent`) rather than a special material.
  pub fn base_material_code(&self) -> &str {
    match self.code.as_str() {
      "PETG Translucent" => "PETG",
      "PLA Matte" | "PLA Silk" => "PLA",
      other => other,
    }
  }
}

fn category_order(cat: &str) -> usize {
  match cat {
    "PLA Family" => 0,
    "PETG / PCTG / PET" => 1,
    "PC / Engineering" => 2,
    "ABS / ASA" => 3,
    "Nylon (PA / PAHT / PPA)" => 4,
    "TPU / TPE / PEBA Flex" => 5,
    "PVA / Support" => 6,
    _ => 10,
  }
}

pub fn load_merged_catalog() -> CombinedCatalog {
  let generated: CombinedCatalog =
    serde_json::from_str(ORCA_GENERATED_JSON).expect("Invalid generated OrcaSlicer catalog JSON");
  let additional: CombinedCatalog =
    serde_json::from_str(ADDITIONAL_CATALOG_JSON).expect("Invalid additional_catalog.json");

  // 1. Merge machines (generated from @orcaslicer_repo + any additional)
  let mut machines = generated.machines;
  for m in additional.machines {
    if let Some(existing) = machines
      .iter_mut()
      .find(|x| x.id == m.id || x.name == m.name)
    {
      *existing = m;
    } else {
      machines.push(m);
    }
  }

  // 2. Merge nozzles: put OrcaSlicer Standard Brass first (as the default in OrcaSlicer configs),
  // followed by MicroSwiss (including MicroSwiss Plated Brass, Diamondback, CM2, CHT), Hardened Steel, Stainless, & Aftermarket
  let mut nozzles = Vec::new();
  if let Some(br) = generated.nozzles.iter().find(|n| n.id == "orca_brass") {
    nozzles.push(br.clone());
  }
  for n in additional.nozzles {
    if !nozzles.iter().any(|x| x.id == n.id) {
      nozzles.push(n);
    }
  }
  for n in generated.nozzles {
    if !nozzles.iter().any(|x| x.id == n.id) {
      nozzles.push(n);
    }
  }

  // 3. Merge beds
  let mut beds = Vec::new();
  for b in generated.beds {
    beds.push(b);
  }
  for b in additional.beds {
    if let Some(existing) = beds.iter_mut().find(|x| x.id == b.id) {
      *existing = b;
    } else {
      beds.push(b);
    }
  }

  // 4. Merge AMS options
  let ams_options = if !additional.ams_options.is_empty() {
    additional.ams_options
  } else {
    generated.ams_options
  };

  // 5. Merge Filaments (OrcaSlicer generated base + Additional Catalog overrides/extras)
  let mut fil_map: BTreeMap<String, CatalogFilament> = BTreeMap::new();
  for f in generated.filaments {
    fil_map.insert(f.code.clone(), f);
  }
  for f in additional.filaments {
    fil_map.insert(f.code.clone(), f);
  }
  let mut filaments: Vec<CatalogFilament> = fil_map.into_values().collect();
  filaments.sort_by(|a, b| {
    category_order(&a.category)
      .cmp(&category_order(&b.category))
      .then_with(|| a.code.cmp(&b.code))
  });

  CombinedCatalog {
    machines,
    nozzles,
    beds,
    ams_options,
    filaments,
    standard_colors: additional.standard_colors,
    default_machine_base: generated.default_machine_base,
    default_process_base: generated.default_process_base,
    default_filament_bases: generated.default_filament_bases,
    default_voron_base: generated.default_voron_base,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_merged_catalog_has_orcaslicer_and_microswiss_and_materials() {
    let cat = load_merged_catalog();
    assert!(
      cat.machines.len() > 300,
      "Expected >300 machines from OrcaSlicer GitHub repo, got {}",
      cat.machines.len()
    );
    assert!(cat.machines.iter().any(|m| m.name == "Anycubic Kobra S1"));
    assert!(cat.machines.iter().any(|m| m.vendor == "Bambu Lab"));

    // Verify MicroSwiss nozzle diameter rules:
    // MicroSwiss Brass has 0.2, 0.4, 0.6; MicroSwiss Diamondback, CM2, CHT only have 0.4!
    let ms_brass = cat
      .nozzles
      .iter()
      .find(|n| n.id == "microswiss_brass")
      .expect("Missing microswiss_brass");
    assert!(ms_brass.supports_diameter(0.2));
    assert!(ms_brass.supports_diameter(0.4));
    assert!(ms_brass.supports_diameter(0.6));

    for only_04_id in ["microswiss_diamondback", "microswiss_cm2", "microswiss_cht"] {
      let n = cat
        .nozzles
        .iter()
        .find(|n| n.id == only_04_id)
        .unwrap_or_else(|| panic!("Missing {}", only_04_id));
      assert_eq!(n.available_diameters, vec![0.4]);
      assert!(!n.supports_diameter(0.2));
      assert!(n.supports_diameter(0.4));
      assert!(!n.supports_diameter(0.6));
    }

    // Verify no "PLA Composite (-CF/-GF)" category exists
    assert!(
      !cat
        .filaments
        .iter()
        .any(|f| f.category.contains("PLA Composite")),
      "PLA Composite category should not exist; PLA-CF and PLA-GF belong in PLA Family"
    );

    // Verify required materials exist and CF/GF have distinct options/parameters
    for required_code in [
      "PLA",
      "PLA-CF",
      "PLA-GF",
      "PETG",
      "PETG-CF",
      "PETG-GF",
      "PCTG",
      "PCTG-CF",
      "PCTG-GF",
      "PC",
      "PC-CF",
      "PC-GF",
      "PAHT",
      "PAHT-CF",
      "PAHT-GF",
      "PPA",
      "PPA-CF",
      "PPA-GF",
      "PEBA",
      "PEBA 90A",
      "PEBA-CF",
      "PVA",
      "BVOH",
      "Rebound TPU",
      "TPU 95A",
    ] {
      assert!(
        cat.filaments.iter().any(|f| f.code == required_code),
        "Missing filament code: {}",
        required_code
      );
    }

    let pla_cf = cat.filaments.iter().find(|f| f.code == "PLA-CF").unwrap();
    let pla_gf = cat.filaments.iter().find(|f| f.code == "PLA-GF").unwrap();
    assert_eq!(pla_cf.category, "PLA Family");
    assert_eq!(pla_gf.category, "PLA Family");
    assert_ne!(pla_cf.default_finish, pla_gf.default_finish);
    assert_ne!(pla_cf.base_nozzle_c, pla_gf.base_nozzle_c);

    // Verify PC defaults to Basic finish (not Translucent) and has safe engineering temperatures (≥255°C nozzle, ≥90°C bed)
    let pc = cat.filaments.iter().find(|f| f.code == "PC").unwrap();
    assert_eq!(pc.default_finish, "Basic");
    assert!(
      pc.base_nozzle_c >= 255 && pc.base_nozzle_c <= 310,
      "Expected PC base_nozzle_c in 255..=310, got {}",
      pc.base_nozzle_c
    );
    assert!(
      pc.base_bed_c >= 90,
      "Expected PC base_bed_c >= 90, got {}",
      pc.base_bed_c
    );

    // Verify PETG Translucent maps to base material "PETG" with "Translucent" finish (not Basic)
    if let Some(petg_trans) = cat.filaments.iter().find(|f| f.code == "PETG Translucent") {
      assert_eq!(petg_trans.base_material_code(), "PETG");
      assert_eq!(petg_trans.default_finish, "Translucent");
    }

    for f in &cat.filaments {
      assert_eq!(
        f.default_color, "#8E9299",
        "Expected all filaments to default to gray (#8E9299), got {} for {}",
        f.default_color, f.code
      );
    }

    assert!(!cat.standard_colors.is_empty());

    // Verify Machine <-> Bed and Machine <-> AMS Compatibility Matrix!
    let kobra_s1 = cat
      .machines
      .iter()
      .find(|m| m.name == "Anycubic Kobra S1")
      .expect("Missing Anycubic Kobra S1");
    let bambu_x1c = cat
      .machines
      .iter()
      .find(|m| m.name.contains("Bambu Lab X1 Carbon"))
      .expect("Missing Bambu Lab X1 Carbon");
    let bambu_a1 = cat
      .machines
      .iter()
      .find(|m| m.name == "Bambu Lab A1")
      .expect("Missing Bambu Lab A1");

    let bambu_ams_1 = cat
      .ams_options
      .iter()
      .find(|a| a.id == "bambu_ams_1")
      .unwrap();
    let bambu_ams_lite = cat
      .ams_options
      .iter()
      .find(|a| a.id == "bambu_ams_lite")
      .unwrap();
    let ace_pro = cat
      .ams_options
      .iter()
      .find(|a| a.id == "ace_pro_single")
      .unwrap();
    let bambu_supertack = cat.beds.iter().find(|b| b.id == "orca_supertack").unwrap();
    let prusa_satin = cat.beds.iter().find(|b| b.id == "prusa_satin").unwrap();

    // Anycubic Kobra S1 can use ACE Pro, CANNOT use Bambu Lab AMS or Prusa sheets or Bambu SuperTack
    assert!(kobra_s1.is_ams_compatible(ace_pro));
    assert!(!kobra_s1.is_ams_compatible(bambu_ams_1));
    assert!(!kobra_s1.is_ams_compatible(bambu_ams_lite));
    assert!(!kobra_s1.is_bed_compatible(bambu_supertack));
    assert!(!kobra_s1.is_bed_compatible(prusa_satin));

    // Bambu X1C can use Bambu AMS 1/2, CANNOT use Anycubic ACE Pro or AMS lite
    assert!(bambu_x1c.is_ams_compatible(bambu_ams_1));
    assert!(!bambu_x1c.is_ams_compatible(bambu_ams_lite));
    assert!(!bambu_x1c.is_ams_compatible(ace_pro));
    assert!(bambu_x1c.is_bed_compatible(bambu_supertack));
    assert!(!bambu_x1c.is_bed_compatible(prusa_satin));

    // Verify default nozzle is OrcaSlicer Standard Brass first, and MicroSwiss Brass nozzles exist
    assert_eq!(cat.nozzles[0].id, "orca_brass");
    assert_eq!(cat.nozzles[0].material, "Brass");
    assert!(cat.nozzles.iter().any(|n| n.id == "microswiss_brass"));
    assert!(cat.nozzles.iter().any(|n| n.id == "microswiss_mk8_brass"));

    // Verify Kobra S1 default nozzle material from OrcaSlicer config is Brass
    assert_eq!(kobra_s1.default_nozzle_material, "Brass");

    // Bambu A1 can use AMS lite, CANNOT use Bambu X1/P1 AMS or Anycubic ACE Pro
    assert!(bambu_a1.is_ams_compatible(bambu_ams_lite));
    assert!(!bambu_a1.is_ams_compatible(bambu_ams_1));
    assert!(!bambu_a1.is_ams_compatible(ace_pro));

    // Verify PendingDiskUpdate::has_value_change() filters out identical values
    let same_num = crate::transformations::PendingDiskUpdate {
      param_key: "machine.nozzle_diameter".to_string(),
      param_label: "Nozzle Diameter".to_string(),
      source_relative_path: "test.json".to_string(),
      json_field_key: "nozzle_diameter".to_string(),
      old_disk_value: "0.40".to_string(),
      new_clean_value: "0.4".to_string(),
    };
    assert!(!same_num.has_value_change());

    let diff_temp = crate::transformations::PendingDiskUpdate {
      param_key: "filament.1.nozzle_temperature".to_string(),
      param_label: "Nozzle Temp".to_string(),
      source_relative_path: "test.json".to_string(),
      json_field_key: "nozzle_temperature".to_string(),
      old_disk_value: "230°C".to_string(),
      new_clean_value: "235".to_string(),
    };
    assert!(diff_temp.has_value_change());
  }
}
