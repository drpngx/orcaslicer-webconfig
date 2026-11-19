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
  pub anycubic_machine_bases: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
  #[serde(default)]
  pub anycubic_process_bases: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
  #[serde(default)]
  pub default_voron_base: BTreeMap<String, serde_json::Value>,
  #[serde(default)]
  pub instantiated_system_filaments: Vec<String>,
  #[serde(default)]
  pub instantiated_filament_plates: BTreeMap<String, [i16; 6]>,
}

impl CombinedCatalog {
  /// Finds the `CatalogMachine` matching `printer_model` (exact match first, then case-insensitive
  /// substring match).
  pub fn find_machine(&self, printer_model: &str) -> Option<&CatalogMachine> {
    let m = printer_model.trim();
    if m.is_empty() {
      return None;
    }
    self
      .machines
      .iter()
      .find(|cm| cm.name.eq_ignore_ascii_case(m))
      .or_else(|| {
        let low = m.to_lowercase();
        self
          .machines
          .iter()
          .filter(|cm| !cm.name.is_empty() && low.contains(&cm.name.to_lowercase()))
          .max_by_key(|cm| cm.name.len())
      })
  }

  /// Resolves the sanitized base machine profile map for `printer_model` (using the
  /// `AnycubicSlicerNext` system override for any Anycubic printer, `default_voron_base`
  /// for Voron rigs, or falling back to `default_machine_base`).
  pub fn machine_base_for_model(
    &self,
    printer_model: &str,
  ) -> &BTreeMap<String, serde_json::Value> {
    let m = printer_model.trim();
    if m.to_lowercase().contains("voron") && !self.default_voron_base.is_empty() {
      return &self.default_voron_base;
    }
    self
      .anycubic_machine_bases
      .get(m)
      .or_else(|| {
        self
          .anycubic_machine_bases
          .iter()
          .find(|(k, _)| k.eq_ignore_ascii_case(m))
          .map(|(_, v)| v)
      })
      .unwrap_or(&self.default_machine_base)
  }

  /// Resolves the sanitized `0.20mm Standard` base process profile map for `printer_model`
  /// (using the `AnycubicSlicerNext` system override for any Anycubic printer, or falling
  /// back to `default_process_base`).
  pub fn process_base_for_model(
    &self,
    printer_model: &str,
  ) -> &BTreeMap<String, serde_json::Value> {
    let m = printer_model.trim();
    self
      .anycubic_process_bases
      .get(m)
      .or_else(|| {
        self
          .anycubic_process_bases
          .iter()
          .find(|(k, _)| k.eq_ignore_ascii_case(m))
          .map(|(_, v)| v)
      })
      .unwrap_or(&self.default_process_base)
  }

  /// Resolves an instantiated system machine preset name for `(printer_model, nozzle_diameter)`
  /// strictly from `CatalogMachine.machine_presets` (never fabricating non-existent names).
  pub fn resolve_machine_inherits(&self, printer_model: &str, nozzle_diameter: &str) -> String {
    let mc = machine_constants(printer_model);
    let noz = if nozzle_diameter.trim().is_empty() {
      mc.default_nozzle_diameter.as_str()
    } else {
      nozzle_diameter.trim()
    };
    if let Some(cm) = self.find_machine(printer_model) {
      let exact = format!("{} {} nozzle", cm.name, noz);
      if cm.machine_presets.contains(&exact) {
        return exact;
      }
      if let Some(m) = cm
        .machine_presets
        .iter()
        .find(|p| p.contains(&format!("{} nozzle", noz)) || p.ends_with(noz))
        .or_else(|| {
          cm.machine_presets
            .iter()
            .find(|p| p.contains("0.4 nozzle"))
        })
        .or_else(|| cm.machine_presets.first())
      {
        return m.clone();
      }
      return exact;
    }
    format!("{} {} nozzle", mc.printer_model, noz)
  }

  /// Resolves an instantiated system filament preset name for `(printer_model, nozzle_diameter,
  /// material_type, finish_type)` strictly by foreign-key lookup against `CatalogMachine.filament_presets`
  /// and `instantiated_system_filaments` (never fabricating string-template preset names).
  pub fn resolve_filament_inherits(
    &self,
    printer_model: &str,
    nozzle_diameter: &str,
    material_type: &str,
    finish_type: &str,
  ) -> String {
    let c = crate::constants::constants();
    let model = if printer_model.trim().is_empty() {
      c.machines[0].printer_model.as_str()
    } else {
      printer_model.trim()
    };
    let mc = machine_constants(model);
    let noz = if nozzle_diameter.trim().is_empty() {
      mc.default_nozzle_diameter.clone()
    } else {
      nozzle_diameter.trim().to_string()
    };
    let poly = c.lookup_polymer(material_type);
    let base_code = c.resolve_base_preset_code(material_type, finish_type, &mc.vendor_prefix);
    let is_anycubic = mc.vendor_prefix.eq_ignore_ascii_case("Anycubic")
      || model.to_ascii_lowercase().contains("anycubic");
    if let Some(cm) = self.find_machine(model) {
      let finish_upgraded = base_code != poly.base_preset_code.as_str();
      let lookup_keys = if is_anycubic || finish_upgraded {
        [
          base_code,
          material_type,
          poly.canonical_key.as_str(),
          poly.slicer_filament_type.as_str(),
          poly.family.as_str(),
          "PLA",
        ]
      } else {
        [
          material_type,
          poly.canonical_key.as_str(),
          poly.slicer_filament_type.as_str(),
          base_code,
          poly.family.as_str(),
          "PLA",
        ]
      };
      for lk in lookup_keys {
        if let Some(found) = cm.filament_presets.get(lk) {
          if noz != "0.4" && found.contains("0.4 nozzle") {
            let swapped = found.replace("0.4 nozzle", &format!("{} nozzle", noz));
            if self.instantiated_system_filaments.contains(&swapped) {
              return swapped;
            }
            for sys_cand in [
              format!("Generic {} @System", poly.slicer_filament_type),
              format!("Generic {} @System", poly.family),
              "Generic PLA @System".to_string(),
            ] {
              if self.instantiated_system_filaments.contains(&sys_cand) {
                return sys_cand;
              }
            }
          }
          return found.clone();
        }
      }
      if let Some(first_p) = cm.filament_presets.values().next() {
        return first_p.clone();
      }
    }
    for sys_cand in [
      format!("Generic {} @System", poly.slicer_filament_type),
      format!("Generic {} @System", base_code),
      format!("Generic {} @System", poly.family),
      "Generic PLA @System".to_string(),
    ] {
      if self.instantiated_system_filaments.contains(&sys_cand) {
        return sys_cand;
      }
    }
    "Generic PLA @System".to_string()
  }
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
  #[serde(default)]
  pub printable_area: Vec<String>,
  #[serde(default)]
  pub printable_height: String,
  #[serde(default)]
  pub default_retraction_mm: f32,
  #[serde(default)]
  pub default_retraction_speed_mms: f32,
  #[serde(default)]
  pub default_print_profile: String,
  #[serde(default)]
  pub default_outer_wall_speed: f32,
  #[serde(default)]
  pub default_inner_wall_speed: f32,
  #[serde(default)]
  pub default_sparse_infill_speed: f32,
  #[serde(default)]
  pub default_initial_layer_speed: f32,
  #[serde(default)]
  pub default_acceleration: f32,
  #[serde(default)]
  pub machine_presets: Vec<String>,
  #[serde(default)]
  pub process_presets: Vec<String>,
  #[serde(default)]
  pub filament_presets: BTreeMap<String, String>,
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

pub fn load_merged_catalog() -> &'static CombinedCatalog {
  static MERGED_CATALOG: std::sync::OnceLock<CombinedCatalog> = std::sync::OnceLock::new();
  MERGED_CATALOG.get_or_init(build_merged_catalog)
}

/// Machine hardware constants for `printer_model`. A curated `constants.json machines[]`
/// entry wins when present — those are DELIBERATE overrides for facts the OrcaSlicer repo
/// does not publish or overstates (machine profiles carry no max hotend/bed temps or
/// volumetric limits; e.g. Creality's Ender-3 filament profiles reach 280 °C but its
/// PTFE-lined hotend is rated 260 °C sustained, so the curated entry says 260). Otherwise
/// the entry is DERIVED from the generated Orca catalog machine (vendor, name, default
/// nozzle) plus the app-global `hardware_limits` — any of the catalog's 380 printers gets
/// sane machine constants without a hand-maintained table, instead of silently inheriting
/// the Kobra S1's vendor prefix and limits.
pub fn machine_constants(printer_model: &str) -> crate::constants::MachineConstants {
  let c = crate::constants::constants();
  if let Some(curated) = c.find_curated_machine(printer_model) {
    return curated.clone();
  }
  let cat = load_merged_catalog();
  let model = printer_model.trim();
  let entry = cat
    .machines
    .iter()
    .find(|m| m.name.eq_ignore_ascii_case(model))
    .or_else(|| {
      let low = model.to_lowercase();
      cat
        .machines
        .iter()
        .find(|m| !m.name.is_empty() && low.contains(&m.name.to_lowercase()))
    });
  let (name, vendor, noz) = match entry {
    Some(m) => (
      m.name.clone(),
      m.vendor.clone(),
      format!("{:.1}", m.default_nozzle_diameter),
    ),
    None => {
      // Unknown even to the catalog: keep the caller's model string, take the leading word
      // as the vendor prefix (covers "Sovol SV06" when a catalog entry is absent).
      let fallback_name = if model.is_empty() {
        c.machines[0].printer_model.clone()
      } else {
        model.to_string()
      };
      let fallback_vendor = fallback_name
        .split_whitespace()
        .next()
        .unwrap_or("Generic")
        .to_string();
      (fallback_name, fallback_vendor, "0.4".to_string())
    }
  };
  crate::constants::MachineConstants {
    printer_model: name,
    vendor_prefix: vendor,
    default_nozzle_diameter: noz,
    max_hotend_temp_c: c.hardware_limits.max_hotend_temp_c,
    max_continuous_nozzle_temp_c: c.hardware_limits.max_continuous_nozzle_temp_c,
    min_bed_temp_c: c.hardware_limits.min_bed_temp_c,
    max_bed_temp_c: c.hardware_limits.max_bed_temp_c,
    max_volumetric_speed_mm3s: c.hardware_limits.max_volumetric_speed_mm3s,
    inherits_template: c.machines[0].inherits_template.clone(),
  }
}

fn build_merged_catalog() -> CombinedCatalog {
  let generated: CombinedCatalog =
    serde_json::from_str(ORCA_GENERATED_JSON).expect("Invalid generated OrcaSlicer catalog JSON");
  let additional: CombinedCatalog =
    serde_json::from_str(ADDITIONAL_CATALOG_JSON).expect("Invalid additional_catalog.json");
  let imported_vendor: CombinedCatalog = serde_json::from_str(IMPORTED_VENDOR_CATALOG_JSON)
    .expect("Invalid vendor_profiles/imported_vendor_catalog.json");

  // 1. Merge machines (generated from @orcaslicer_repo + any additional), preserving generated
  // preset lists and hardware geometry when an `additional_catalog.json` entry omits them.
  let mut machines = generated.machines;
  for mut m in additional.machines {
    if let Some(existing) = machines
      .iter_mut()
      .find(|x| x.id == m.id || x.name == m.name)
    {
      if m.printable_area.is_empty() {
        m.printable_area = existing.printable_area.clone();
      }
      if m.printable_height.is_empty() {
        m.printable_height = existing.printable_height.clone();
      }
      if m.default_retraction_mm <= 0.0 {
        m.default_retraction_mm = existing.default_retraction_mm;
      }
      if m.default_retraction_speed_mms <= 0.0 {
        m.default_retraction_speed_mms = existing.default_retraction_speed_mms;
      }
      if m.default_print_profile.is_empty() {
        m.default_print_profile = existing.default_print_profile.clone();
      }
      if m.default_outer_wall_speed <= 0.0 {
        m.default_outer_wall_speed = existing.default_outer_wall_speed;
      }
      if m.default_inner_wall_speed <= 0.0 {
        m.default_inner_wall_speed = existing.default_inner_wall_speed;
      }
      if m.default_sparse_infill_speed <= 0.0 {
        m.default_sparse_infill_speed = existing.default_sparse_infill_speed;
      }
      if m.default_initial_layer_speed <= 0.0 {
        m.default_initial_layer_speed = existing.default_initial_layer_speed;
      }
      if m.default_acceleration <= 0.0 {
        m.default_acceleration = existing.default_acceleration;
      }
      if m.machine_presets.is_empty() {
        m.machine_presets = existing.machine_presets.clone();
      }
      if m.process_presets.is_empty() {
        m.process_presets = existing.process_presets.clone();
      }
      if m.filament_presets.is_empty() {
        m.filament_presets = existing.filament_presets.clone();
      }
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
        f.base_nozzle_c = f.base_nozzle_c.clamp(
          poly.catalog_nozzle_bounds_c[0],
          poly.catalog_nozzle_bounds_c[1],
        );
        f.base_bed_c = f
          .base_bed_c
          .clamp(poly.catalog_bed_bounds_c[0], poly.catalog_bed_bounds_c[1]);
        f.fan_pct = f.fan_pct.clamp(
          poly.catalog_fan_bounds_pct[0],
          poly.catalog_fan_bounds_pct[1],
        );
        f.max_vol_mm3s = f.max_vol_mm3s.clamp(
          poly.catalog_mvs_bounds_mm3s[0],
          poly.catalog_mvs_bounds_mm3s[1],
        );
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
    anycubic_machine_bases: generated.anycubic_machine_bases,
    anycubic_process_bases: generated.anycubic_process_bases,
    default_voron_base: generated.default_voron_base,
    instantiated_system_filaments: generated.instantiated_system_filaments,
    instantiated_filament_plates: generated.instantiated_filament_plates,
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
      "Anycubic PLA Wood @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic PLA Metal @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic PLA Marble @Anycubic Kobra S1 0.4 nozzle",
      "Anycubic PLA Translucent @Anycubic Kobra S1 0.4 nozzle",
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
      let canon_poly = crate::models::FilamentSpool::canonical_polymer_key(&spool.material_type);
      let expected_parent = match canon_poly {
        "PC" | "PC-CF" | "PEEK" | "PPS" => "Anycubic PC @Anycubic Kobra S1 0.4 nozzle",
        "PA" | "PA6-CF" | "PA12-CF" | "PAHT-CF" | "PPA-CF" => {
          "Anycubic PA6-CF @Anycubic Kobra S1 0.4 nozzle"
        }
        "PET-CF" => "Anycubic PET-CF @Anycubic Kobra S1 0.4 nozzle",
        "PEBA" => "Anycubic PEBA @Anycubic Kobra S1 0.4 nozzle",
        "PETG-CF" | "PCTG-CF" => "Anycubic PETG-CF @Anycubic Kobra S1 0.4 nozzle",
        "PETG" | "PCTG" => "Anycubic PETG @Anycubic Kobra S1 0.4 nozzle",
        "ASA" | "ASA-CF" | "PP" | "HIPS" => "Anycubic ASA @Anycubic Kobra S1 0.4 nozzle",
        "ABS" | "ABS-CF" => "Anycubic ABS @Anycubic Kobra S1 0.4 nozzle",
        "PLA-CF" => "Anycubic PLA-CF @Anycubic Kobra S1 0.4 nozzle",
        "PLA+" => {
          if spool.material_type.eq_ignore_ascii_case("PLA High Speed") {
            "Anycubic PLA High Speed @Anycubic Kobra S1 0.4 nozzle"
          } else {
            "Anycubic PLA+ @Anycubic Kobra S1 0.4 nozzle"
          }
        }
        _ => inherits,
      };
      assert_eq!(
        inherits, expected_parent,
        "Exported filament '{}' ({} / {}) inherited wrong parent '{}'",
        spool.name, fil.brand, fil.code, inherits
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

  #[test]
  fn test_anycubic_slicer_next_fork_overrides_all_anycubic_machines_and_filaments() {
    let cat = load_merged_catalog();
    let expected_anycubic_models = [
      "Anycubic Kobra 1",
      "Anycubic Kobra 1 Plus",
      "Anycubic Kobra 1 Max",
      "Anycubic Kobra 2",
      "Anycubic Kobra 2 Neo",
      "Anycubic Kobra 2 Pro",
      "Anycubic Kobra 2 Plus",
      "Anycubic Kobra 2 Max",
      "Anycubic Kobra 3",
      "Anycubic Kobra 3 V2",
      "Anycubic Kobra 3 Max",
      "Anycubic Kobra 3 Max V2",
      "Anycubic Kobra 4",
      "Anycubic Kobra X",
      "Anycubic Kobra S1",
      "Anycubic Kobra S1 Max",
    ];
    let actual_anycubic: Vec<&str> = cat
      .machines
      .iter()
      .filter(|m| m.vendor == "Anycubic")
      .map(|m| m.name.as_str())
      .collect();
    assert_eq!(
      actual_anycubic.len(),
      expected_anycubic_models.len(),
      "All Anycubic machines must come authoritatively from AnycubicSlicerNext: {:?}",
      actual_anycubic
    );
    for model in expected_anycubic_models {
      let m = cat
        .machines
        .iter()
        .find(|m| m.name == model)
        .unwrap_or_else(|| panic!("Missing AnycubicSlicerNext machine '{}'", model));
      assert!(
        cat.anycubic_machine_bases.contains_key(model),
        "Missing AnycubicSlicerNext machine base for '{}'",
        model
      );
      assert!(
        cat.anycubic_process_bases.contains_key(model),
        "Missing AnycubicSlicerNext process base for '{}'",
        model
      );
      let is_corexy = model.contains("Kobra S1");
      assert_eq!(
        crate::constants::constants().kinematics_class(&m.kinematics),
        if is_corexy { "corexy" } else { "bedslinger" },
        "Wrong kinematics class for '{}' ({})",
        model,
        m.kinematics
      );
    }

    let kobra_4 = cat
      .machines
      .iter()
      .find(|m| m.name == "Anycubic Kobra 4")
      .unwrap();
    assert!(kobra_4
      .supported_ams_ids
      .contains(&"ace_pro_single".to_string()));

    // Verify per-machine printable_area comes from AnycubicSlicerNext (e.g. Kobra S1 Max = 350x350)
    let s1_max_base = cat.machine_base_for_model("Anycubic Kobra S1 Max");
    assert_eq!(
      s1_max_base.get("printable_height"),
      Some(&serde_json::json!("350"))
    );

    let ac_abs = cat
      .filaments
      .iter()
      .find(|f| f.brand == "Anycubic" && f.code == "ABS")
      .expect("Anycubic ABS must exist in merged catalog");
    assert_eq!(ac_abs.base_bed_c, 110);
    assert_eq!(ac_abs.base_nozzle_c, 260);
    assert_eq!(ac_abs.fan_pct, 20);
    assert!((ac_abs.max_vol_mm3s - 15.0).abs() < 0.05);
    assert!(ac_abs.source.contains("AnycubicSlicerNext System Override"));

    let gen_abs = cat
      .filaments
      .iter()
      .find(|f| f.brand == "Generic" && f.code == "ABS")
      .expect("Generic ABS must exist in merged catalog");
    assert_eq!(gen_abs.base_bed_c, 110);
    assert_eq!(gen_abs.base_nozzle_c, 260);

    let ac_asa = cat
      .filaments
      .iter()
      .find(|f| f.brand == "Anycubic" && f.code == "ASA")
      .expect("Anycubic ASA must exist in merged catalog");
    assert_eq!(ac_asa.base_bed_c, 110);
    assert_eq!(ac_asa.base_nozzle_c, 270);
    assert_eq!(ac_asa.fan_pct, 20);
  }
}
