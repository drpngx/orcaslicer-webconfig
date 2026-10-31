use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const ORCA_GENERATED_JSON: &str = include_str!(env!("ORCASLICER_GENERATED_CATALOG"));
const ADDITIONAL_CATALOG_JSON: &str = include_str!("../additional_catalog.json");
const IMPORTED_VENDOR_CATALOG_JSON: &str = include_str!(env!("IMPORTED_VENDOR_CATALOG"));

fn default_filament_brand() -> String {
  "Generic".to_string()
}

fn default_filament_flow_ratio() -> f32 {
  0.98
}

fn default_filament_retraction_mm() -> f32 {
  0.8
}

pub const FILAMENT_BRAND_OPTIONS: &[&str] = &[
  "All",
  "Generic",
  "Siraya Tech",
  "SUNLU",
  "Polymaker",
  "Anycubic",
  "Bambu Lab",
  "eSUN",
  "Overture",
];

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
  pub vendor_filament_bases: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
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
  #[serde(default = "default_filament_brand")]
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
  #[serde(default = "default_filament_flow_ratio")]
  pub default_flow_ratio: f32,
  #[serde(default = "default_filament_retraction_mm")]
  pub default_retraction_mm: f32,
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

pub const FINISH_OPTIONS: &[(&str, &str)] = &[
  ("Basic", "Basic"),
  ("Matte", "Matte"),
  ("Silk", "Silk"),
  ("Translucent", "Translucent"),
  ("Carbon Fiber", "Carbon Fiber (-CF)"),
  ("Glass Fiber", "Glass Fiber (-GF)"),
  ("Wood-Filled", "Wood-Filled"),
  ("Metal-Filled", "Metal-Filled"),
  ("Glow-in-the-Dark", "Glow-in-the-Dark"),
  ("Foaming", "Foaming (LW / Varioshore)"),
  ("Conductive", "Conductive / ESD"),
  ("Marble/Stone/Terracotta", "Marble / Stone / Terracotta"),
  ("Photochromic", "Photochromic (UV Color-Change)"),
];

impl CatalogFilament {
  /// Returns the base polymer code without finish suffixes (e.g., `"PETG Translucent"` -> `"PETG"`,
  /// `"PLA Matte"` -> `"PLA"`, `"PLA Silk"` -> `"PLA"`, `"PLA Wood"` -> `"PLA"`, etc.), so finish variants
  /// are modeled as `base_material_code() + default_finish` (e.g., `PLA` / `Wood-Filled`).
  pub fn base_material_code(&self) -> &str {
    crate::constants::constants().base_material_code(&self.code)
  }
  /// Returns a unique key `"<brand>::<code>"` for catalog lookup.
  pub fn catalog_key(&self) -> String {
    format!("{}::{}", self.brand, self.code)
  }
}

fn brand_order(brand: &str) -> usize {
  crate::constants::constants().brand_order(brand)
}

fn category_order(cat: &str) -> usize {
  crate::constants::constants().category_order(cat)
}

fn polymer_priority(code: &str) -> usize {
  crate::constants::constants().polymer_sort_priority(code)
}

pub fn load_merged_catalog() -> CombinedCatalog {
  static MERGED_CATALOG: std::sync::OnceLock<CombinedCatalog> = std::sync::OnceLock::new();
  MERGED_CATALOG.get_or_init(build_merged_catalog).clone()
}

fn build_merged_catalog() -> CombinedCatalog {
  let generated: CombinedCatalog =
    serde_json::from_str(ORCA_GENERATED_JSON).expect("Invalid generated OrcaSlicer catalog JSON");
  let additional: CombinedCatalog =
    serde_json::from_str(ADDITIONAL_CATALOG_JSON).expect("Invalid additional_catalog.json");
  let imported_vendor: CombinedCatalog = serde_json::from_str(IMPORTED_VENDOR_CATALOG_JSON)
    .expect("Invalid vendor_profiles/imported_vendor_catalog.json");

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

  // 5. Merge Filaments (OrcaSlicer generated base + Additional Catalog + Imported Vendor .3mf / Preset Manager Catalog)
  let mut fil_map: BTreeMap<String, CatalogFilament> = BTreeMap::new();
  for f in generated.filaments {
    fil_map.insert(f.catalog_key(), f);
  }
  for f in additional.filaments {
    fil_map.insert(f.catalog_key(), f);
  }
  for f in imported_vendor.filaments {
    fil_map.insert(f.catalog_key(), f);
  }
  let c = crate::constants::constants();
  for ac_code in &c.anycubic_system_filament_codes {
    let ac_key = format!("Anycubic::{}", ac_code);
    if !fil_map.contains_key(&ac_key) {
      let gen_key = format!("Generic::{}", ac_code);
      if let Some(mut base_f) = fil_map.get(&gen_key).cloned() {
        let poly = c.lookup_polymer(ac_code);
        base_f.brand = "Anycubic".to_string();
        base_f.label = format!("Anycubic {} (@Anycubic Kobra S1 0.4 nozzle)", ac_code);
        base_f.source = "AnycubicSlicerNext System".to_string();
        base_f.requires_hardened =
          c.is_abrasive_material_or_finish(ac_code, &base_f.default_finish);
        if poly.requires_hardened && base_f.default_finish == "Basic" {
          base_f.default_finish = "Carbon Fiber".to_string();
        }
        fil_map.insert(ac_key, base_f);
      }
    }
  }
  let mut filaments: Vec<CatalogFilament> = fil_map
    .into_values()
    .map(|mut f| {
      let poly = c.lookup_polymer(&f.code);
      f.requires_hardened =
        crate::models::FilamentSpool::is_abrasive_material_or_finish(&f.code, &f.default_finish);
      if !c.is_esd_or_foaming(&f.code, &f.default_finish) {
        f.base_nozzle_c = f
          .base_nozzle_c
          .clamp(poly.catalog_nozzle_bounds_c[0], poly.catalog_nozzle_bounds_c[1]);
        f.base_bed_c = f
          .base_bed_c
          .clamp(poly.catalog_bed_bounds_c[0], poly.catalog_bed_bounds_c[1]);
        f.fan_pct = f
          .fan_pct
          .clamp(poly.catalog_fan_bounds_pct[0], poly.catalog_fan_bounds_pct[1]);
        f.max_vol_mm3s = f
          .max_vol_mm3s
          .clamp(poly.catalog_mvs_bounds_mm3s[0], poly.catalog_mvs_bounds_mm3s[1]);
      }
      f.base_nozzle_c = f.base_nozzle_c.max(poly.safe_nozzle_min_c as u16);
      f.base_bed_c = f
        .base_bed_c
        .clamp(poly.safe_bed_min_c as u16, poly.safe_bed_max_c as u16);
      f
    })
    .collect();
  filaments.sort_by(|a, b| {
    brand_order(&a.brand)
      .cmp(&brand_order(&b.brand))
      .then_with(|| category_order(&a.category).cmp(&category_order(&b.category)))
      .then_with(|| polymer_priority(&a.code).cmp(&polymer_priority(&b.code)))
      .then_with(|| a.code.cmp(&b.code))
  });

  let mut default_filament_bases = generated.default_filament_bases;
  for base_map in default_filament_bases.values_mut() {
    base_map.insert(
      "compatible_printers".to_string(),
      serde_json::json!(["Anycubic Kobra S1 0.4 nozzle"]),
    );
  }

  let mut vendor_filament_bases = generated.vendor_filament_bases;
  for (k, v) in imported_vendor.vendor_filament_bases {
    vendor_filament_bases.insert(k, v);
  }
  for base_map in vendor_filament_bases.values_mut() {
    base_map.insert(
      "compatible_printers".to_string(),
      serde_json::json!(["Anycubic Kobra S1 0.4 nozzle"]),
    );
  }

  CombinedCatalog {
    machines,
    nozzles,
    beds,
    ams_options,
    filaments,
    standard_colors: additional.standard_colors,
    default_machine_base: generated.default_machine_base,
    default_process_base: generated.default_process_base,
    default_filament_bases,
    vendor_filament_bases,
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
    // - MicroSwiss Brass: 0.2, 0.3, 0.4, 0.6, 0.8, 1.0
    // - MicroSwiss Diamondback: 0.25, 0.4, 0.6, 0.8
    // - MicroSwiss CM2 & CHT CM2: 0.4, 0.6, 0.8
    // - MicroSwiss CHT: 0.4, 0.6, 0.8, 1.0
    let ms_brass = cat
      .nozzles
      .iter()
      .find(|n| n.id == "microswiss_brass")
      .expect("Missing microswiss_brass");
    assert_eq!(
      ms_brass.available_diameters,
      vec![0.2, 0.3, 0.4, 0.6, 0.8, 1.0]
    );

    let ms_db = cat
      .nozzles
      .iter()
      .find(|n| n.id == "microswiss_diamondback")
      .expect("Missing microswiss_diamondback");
    assert_eq!(ms_db.available_diameters, vec![0.25, 0.4, 0.6, 0.8]);
    assert!(!ms_db.supports_diameter(0.2));
    assert!(ms_db.supports_diameter(0.25));
    assert!(ms_db.supports_diameter(0.6));
    assert!(ms_db.supports_diameter(0.8));

    for cm2_id in ["microswiss_cm2", "microswiss_cht_cm2"] {
      let n = cat
        .nozzles
        .iter()
        .find(|n| n.id == cm2_id)
        .unwrap_or_else(|| panic!("Missing {}", cm2_id));
      assert_eq!(n.available_diameters, vec![0.4, 0.6, 0.8]);
      assert!(!n.supports_diameter(0.25));
      assert!(n.supports_diameter(0.4));
      assert!(n.supports_diameter(0.8));
    }

    let ms_cht = cat
      .nozzles
      .iter()
      .find(|n| n.id == "microswiss_cht")
      .expect("Missing microswiss_cht");
    assert_eq!(ms_cht.available_diameters, vec![0.4, 0.6, 0.8, 1.0]);
    assert!(ms_cht.supports_diameter(1.0));

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

    // Verify all 7 new specialty finishes exist in FINISH_OPTIONS, have catalog entries, and tune FilamentSpool properly
    for (finish_val, _) in [
      ("Wood-Filled", "Wood"),
      ("Metal-Filled", "Metal"),
      ("Glow-in-the-Dark", "Glow"),
      ("Foaming", "Foaming"),
      ("Conductive", "Conductive"),
      ("Marble/Stone/Terracotta", "Marble"),
      ("Photochromic", "Photochromic"),
    ] {
      assert!(
        FINISH_OPTIONS.iter().any(|(v, _)| *v == finish_val),
        "Missing {} in FINISH_OPTIONS",
        finish_val
      );
      assert!(
        cat.filaments.iter().any(|f| f.default_finish == finish_val),
        "Expected at least one catalog filament with default_finish = {}",
        finish_val
      );
    }

    let pla_entry = cat.filaments.iter().find(|f| f.code == "PLA").unwrap();
    let mut spool =
      crate::models::FilamentSpool::from_catalog_entry(1, pla_entry, "e3d_obxidian_04");
    spool.apply_finish_change("Wood-Filled", "e3d_obxidian_04");
    assert_eq!(spool.finish_type, "Wood-Filled");
    assert_eq!(spool.required_nozzle_hrc, 55);
    assert!(
      spool.name.contains("PLA Wood"),
      "Expected 'PLA Wood' in '{}'",
      spool.name
    );

    spool.apply_finish_change("Foaming", "e3d_obxidian_04");
    assert_eq!(spool.finish_type, "Foaming");
    assert!((spool.flow_ratio - 0.58).abs() < 0.01);
    assert!(
      spool.name.contains("PLA Foaming"),
      "Expected 'PLA Foaming' in '{}'",
      spool.name
    );

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

    // Verify checked-in Vendor Profiles (Siraya Tech .3mf, SUNLU Preset Manager, Polymaker Preset Manager, Anycubic)
    let siraya_count = cat
      .filaments
      .iter()
      .filter(|f| f.brand == "Siraya Tech")
      .count();
    assert!(
      siraya_count >= 20,
      "Expected >= 20 Siraya Tech .3mf profiles, got {}",
      siraya_count
    );
    let sunlu_count = cat.filaments.iter().filter(|f| f.brand == "SUNLU").count();
    assert!(
      sunlu_count >= 25,
      "Expected >= 25 SUNLU Preset Manager profiles, got {}",
      sunlu_count
    );
    let polymaker_count = cat
      .filaments
      .iter()
      .filter(|f| f.brand == "Polymaker")
      .count();
    assert!(
      polymaker_count >= 50,
      "Expected >= 50 Polymaker Preset Manager profiles, got {}",
      polymaker_count
    );

    // Verify Siraya Tech TPU Air 85A .3mf base profile and Shore hardness curve
    let siraya_air = cat
      .filaments
      .iter()
      .find(|f| f.brand == "Siraya Tech" && f.code == "TPU Air 85A")
      .expect("Missing Siraya Tech TPU Air 85A from checked-in .3mf");
    assert_eq!(siraya_air.base_nozzle_c, 250);
    assert!((siraya_air.default_flow_ratio - 0.67).abs() < 0.02);
    assert!(cat
      .vendor_filament_bases
      .contains_key("Siraya Tech::TPU Air 85A"));

    let mut air_spool =
      crate::models::FilamentSpool::from_catalog_entry(1, siraya_air, "orca_brass");
    assert_eq!(air_spool.brand, "Siraya Tech");
    assert!(air_spool.is_siraya_foaming_air());
    air_spool.apply_foaming_shore_hardness(70, "orca_brass");
    assert_eq!(air_spool.print_temp, 260);
    assert!((air_spool.flow_ratio - 0.56).abs() < 0.01);

    // Verify apply_brand_change switches to vendor base temperatures and vendor_filament_bases
    let mut pet_cf =
      crate::models::FilamentSpool::from_catalog_entry(2, siraya_air, "e3d_obxidian_04");
    pet_cf.apply_brand_change("SUNLU", "e3d_obxidian_04");
    assert_eq!(pet_cf.brand, "SUNLU");
    pet_cf.apply_material_change("PETG-CF", "e3d_obxidian_04");
    assert!(pet_cf.name.contains("SUNLU"));
  }

  #[test]
  fn test_every_catalog_filament_exports_valid_kobra_s1_dropdown_json_and_roundtrips() {
    let cat = load_merged_catalog();
    let rig = crate::models::MachineRig::kobra_s1_hardened_steel_png();
    let procs = crate::models::ProcessProfile::get_default_processes();
    let valid_kobra_s1_inherits = [
      "Anycubic PLA @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic PLA+ @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic PLA-CF @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic PLA Matte @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic PLA Silk @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic PLA Glow @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic PLA High Speed @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic PETG @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic PETG-CF @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic PET-CF @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic TPU @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic PEBA @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic ASA @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic ABS @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic PC @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic PA6-CF @Anycubic Kobra S1 0.4 nozzle",
    ];

    // 1. Verify default_filament_bases and vendor_filament_bases always carry Kobra S1 compatible_printers
    for (k, base_map) in &cat.default_filament_bases {
      assert_eq!(
        base_map.get("compatible_printers"),
        Some(&serde_json::json!(["Anycubic Kobra S1 0.4 nozzle"])),
        "default_filament_bases['{}'] must have compatible_printers = ['Anycubic Kobra S1 0.4 nozzle']",
        k
      );
    }
    for (k, base_map) in &cat.vendor_filament_bases {
      assert_eq!(
        base_map.get("compatible_printers"),
        Some(&serde_json::json!(["Anycubic Kobra S1 0.4 nozzle"])),
        "vendor_filament_bases['{}'] must have compatible_printers = ['Anycubic Kobra S1 0.4 nozzle']",
        k
      );
    }

    // 2. Verify every single CatalogFilament entry exports with compatible_printers = ["Anycubic Kobra S1 0.4 nozzle"], no filament_vendor, inherits a valid Kobra S1 base, and round-trips through AnycubicSlicerNext.conf
    for (idx, fil) in cat.filaments.iter().enumerate() {
      let spool = crate::models::FilamentSpool::from_catalog_entry(1, fil, "hardened_steel");
      assert!(
        spool.name.starts_with("OC - "),
        "Catalog filament {:?} produced non-OC name '{}'",
        fil,
        spool.name
      );
      let json_str = crate::threemf::export_standalone_filament_json(&spool, &rig);
      let val: serde_json::Value =
        serde_json::from_str(&json_str).expect("Valid exported filament JSON");
      assert_eq!(
        val.get("compatible_printers"),
        Some(&serde_json::json!(["Anycubic Kobra S1 0.4 nozzle"])),
        "Exported filament '{}' ({} / {}) must have compatible_printers = ['Anycubic Kobra S1 0.4 nozzle']!",
        spool.name,
        fil.brand,
        fil.code
      );
      assert!(
        val.get("filament_vendor").is_none(),
        "Exported filament '{}' ({} / {}) must NOT contain filament_vendor (causes 3MF slot mismatch)!",
        spool.name,
        fil.brand,
        fil.code
      );
      let inherits = val["inherits"].as_str().unwrap_or("");
      assert!(
        valid_kobra_s1_inherits.contains(&inherits),
        "Exported filament '{}' has invalid Kobra S1 inherits '{}'",
        spool.name,
        inherits
      );

      // Test round-trip save & re-import for all Anycubic filaments and every 15th vendor filament
      if fil.brand == "Anycubic" || idx % 15 == 0 {
        let mut entries = vec![crate::fs_access::ScannedFileEntry {
          relative_path: "AnycubicSlicerNext.conf".to_string(),
          filename: "AnycubicSlicerNext.conf".to_string(),
          bytes: br#"{
            "app": { "preset_folder": "434139", "version": "2.0.0.3", "flush_multiplier": "1.000000" },
            "models": [{ "model": "Anycubic Kobra S1", "nozzle_diameter": "0.4", "vendor": "Anycubic" }],
            "presets": { "machine": "Anycubic Kobra S1 0.4 nozzle" },
            "anycubic_presets": [{
              "machine": "Anycubic Kobra S1 0.4 nozzle",
              "process": "0.20mm Standard @Anycubic Kobra S1 0.4 nozzle",
              "filament": "Anycubic PLA @Anycubic Kobra S1 0.4 nozzle"
            }]
          }"#
            .to_vec(),
        }];
        let _ = crate::dir_import::sync_live_state_to_entries(
          &mut entries,
          std::slice::from_ref(&rig),
          &rig.id,
          std::slice::from_ref(&spool),
          &procs,
          &procs[0].id,
          &["0".to_string()],
          1.0,
          "AnycubicSlicerNext.conf",
          &[],
          &serde_json::Map::new(),
        );
        let reimported = crate::dir_import::import_scanned_directory(&entries).unwrap();
        assert_eq!(
          reimported.filaments[0].name, spool.name,
          "Round-trip failed for {} ({}): expected '{}', got '{}'",
          fil.brand, fil.code, spool.name, reimported.filaments[0].name
        );
      }
    }
  }
}
