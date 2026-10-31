use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

const CONSTANTS_JSON: &str = include_str!("../constants.json");

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebConfigConstants {
  pub hardware_limits: HardwareLimits,
  #[serde(default)]
  pub finish_variant_mappings: BTreeMap<String, String>,
  #[serde(default)]
  pub direct_inherits_preset_codes: Vec<String>,
  #[serde(default)]
  pub anycubic_system_filament_codes: Vec<String>,
  pub machines: Vec<MachineConstants>,
  pub brands: Vec<BrandConstants>,
  pub categories: Vec<CategoryConstants>,
  pub nozzles: Vec<NozzleConstants>,
  pub finishes: Vec<FinishConstants>,
  pub climates: Vec<ClimateConstants>,
  pub polymers: Vec<PolymerConstants>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HardwareLimits {
  pub min_extrusion_temp_c: i32,
  pub max_hotend_temp_c: i32,
  pub max_continuous_nozzle_temp_c: i32,
  pub max_foaming_tpu_nozzle_temp_c: i32,
  pub min_bed_temp_c: i32,
  pub max_bed_temp_c: i32,
  pub min_volumetric_speed_mm3s: f32,
  pub max_volumetric_speed_mm3s: f32,
  pub min_flow_ratio: f32,
  pub max_flow_ratio: f32,
  pub min_non_foaming_flow_ratio: f32,
  pub max_non_foaming_flow_ratio: f32,
  pub min_retraction_speed_mms: f32,
  pub max_retraction_speed_mms: f32,
  pub min_flush_volume_mm3: f64,
  pub max_flush_volume_mm3: f64,
  pub abrasive_required_hrc: u32,
  pub non_abrasive_required_hrc: u32,
  pub standard_ams_range_low_c: i32,
  pub standard_ams_range_high_c: i32,
  pub high_temp_range_low_c: i32,
  pub high_temp_range_high_c: i32,
  pub esd_safe_nozzle_min_c: i32,
  pub esd_safe_nozzle_max_c: i32,
  pub pcl_temp_range_low_c: i32,
  pub pcl_temp_range_high_c: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MachineConstants {
  pub printer_model: String,
  pub vendor_prefix: String,
  pub default_nozzle_diameter: String,
  pub max_hotend_temp_c: i32,
  pub max_continuous_nozzle_temp_c: i32,
  pub min_bed_temp_c: i32,
  pub max_bed_temp_c: i32,
  pub max_volumetric_speed_mm3s: f32,
  pub inherits_template: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrandConstants {
  pub name: String,
  pub order: usize,
  pub aliases: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CategoryConstants {
  pub name: String,
  pub order: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NozzleConstants {
  pub id: String,
  pub short_tag: String,
  pub orca_nozzle_type: String,
  pub hrc: u32,
  pub temp_offset_from_brass_c: i32,
  pub uses_hs_base_temp: bool,
  pub material_label: String,
  pub default_name: String,
  pub pa_multiplier: f32,
  pub match_tokens: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FinishConstants {
  pub code: String,
  pub label: String,
  pub title_suffix: Option<String>,
  pub requires_hardened: bool,
  pub nozzle_temp_offset_c: i32,
  pub default_opacity: f32,
  pub default_pigment_stickiness: f32,
  #[serde(default)]
  pub aliases: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClimateConstants {
  pub id: String,
  pub ambient_temp_c: i32,
  pub humidity_pct: i32,
}

fn default_z_hop_mm() -> f32 {
  0.2
}

fn default_z_hop_type() -> String {
  "Normal Lift".to_string()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PolymerConstants {
  pub canonical_key: String,
  pub family: String,
  pub match_tokens: Vec<String>,
  pub safe_nozzle_min_c: i32,
  pub safe_nozzle_max_c: i32,
  pub safe_bed_min_c: i32,
  pub safe_bed_max_c: i32,
  pub default_brass_nozzle_c: i32,
  pub default_bed_c: i32,
  pub default_max_volumetric_speed: f32,
  pub default_fan_pct: i32,
  pub default_flow_ratio: f32,
  pub default_pressure_advance: f32,
  pub default_retraction_mm: f32,
  pub default_retraction_speed_mms: i32,
  #[serde(default)]
  pub default_retract_before_wipe_pct: u8,
  #[serde(default = "default_z_hop_mm")]
  pub default_z_hop_mm: f32,
  #[serde(default = "default_z_hop_type")]
  pub default_z_hop_type: String,
  #[serde(default)]
  pub translucent_z_hop_mm: Option<f32>,
  #[serde(default)]
  pub translucent_pressure_advance: Option<f32>,
  pub default_dry_temp_c: i32,
  pub default_dry_time_hr: i32,
  pub slicer_filament_type: String,
  pub base_preset_code: String,
  pub requires_hardened: bool,
  pub standard_ams_compatible: bool,
  pub sort_priority: usize,
  pub catalog_nozzle_bounds_c: [u16; 2],
  pub catalog_bed_bounds_c: [u16; 2],
  pub catalog_fan_bounds_pct: [u8; 2],
  pub catalog_mvs_bounds_mm3s: [f32; 2],
  #[serde(default)]
  pub cf_upgrade_code: Option<String>,
  #[serde(default)]
  pub gf_upgrade_code: Option<String>,
  #[serde(default)]
  pub compatible_finishes: Vec<String>,
  #[serde(default)]
  pub gf_tokens: Vec<String>,
  #[serde(default)]
  pub compatible_finishes_gf: Vec<String>,
}

impl PolymerConstants {
  pub fn is_copolyester(&self) -> bool {
    self.family == "PETG"
  }

  pub fn is_flexible(&self) -> bool {
    self.family == "TPU"
  }

  pub fn is_warp_prone(&self) -> bool {
    matches!(
      self.family.as_str(),
      "ABS" | "ASA" | "PA" | "PC" | "HIGH_TEMP" | "PP"
    )
  }

  pub fn initial_layer_temp_boost_c(&self, is_foaming_flex: bool) -> i32 {
    if self.is_copolyester() || self.is_flexible() || is_foaming_flex {
      0
    } else {
      5
    }
  }

  pub fn resolve_fan_max_pct(&self, fan_min_pct: i32) -> i32 {
    if self.is_copolyester() && fan_min_pct >= 30 {
      90.max(fan_min_pct)
    } else {
      fan_min_pct
    }
  }

  pub fn resolve_pressure_advance(&self, finish_code: &str) -> f32 {
    if finish_code.eq_ignore_ascii_case("Translucent") {
      self
        .translucent_pressure_advance
        .unwrap_or(self.default_pressure_advance)
    } else {
      self.default_pressure_advance
    }
  }

  pub fn format_retraction_mm(val: f64) -> String {
    if val == 0.0 {
      "0".to_string()
    } else if (val - val.round()).abs() < 1e-6 {
      format!("{:.1}", val)
    } else {
      format!("{:.2}", val)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
    }
  }

  pub fn resolve_z_hop_and_retract_before_wipe<'a>(
    &'a self,
    finish_code: &str,
    is_foaming_flex: bool,
  ) -> (String, &'a str, String) {
    let z_hop_mm = if is_foaming_flex {
      0.0
    } else if finish_code.eq_ignore_ascii_case("Translucent") {
      self.translucent_z_hop_mm.unwrap_or(self.default_z_hop_mm)
    } else {
      self.default_z_hop_mm
    };
    let z_hop_str = if z_hop_mm == 0.0 {
      "0".to_string()
    } else {
      format!("{:.2}", z_hop_mm)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
    };
    let retract_before_wipe_str = format!("{}%", self.default_retract_before_wipe_pct);
    (
      z_hop_str,
      self.default_z_hop_type.as_str(),
      retract_before_wipe_str,
    )
  }

  /// Computes `(climate_noz_delta, climate_bed_delta, climate_init_bed_extra, climate_retract_delta)`
  /// for the active ambient weather preset using polymer family properties from `constants.json`.
  pub fn compute_climate_deltas(
    &self,
    temp_c: i32,
    rh_pct: i32,
    climate_preset: &str,
    is_heat_creep_resistant_nozzle: bool,
  ) -> (i32, i32, i32, f64) {
    let is_hot = temp_c >= 26 || climate_preset == "tropical" || climate_preset == "hot_dry";
    let is_cold = temp_c <= 18 || climate_preset == "cold_dry" || climate_preset == "cold_humid";
    let is_humid = rh_pct >= 60 || climate_preset == "tropical" || climate_preset == "cold_humid";

    let climate_retract_delta = if is_humid {
      if self.is_copolyester() || self.is_flexible() {
        0.20
      } else {
        0.15
      }
    } else {
      0.0
    };

    let mut climate_noz_delta = 0;
    let mut climate_bed_delta = 0;
    let mut climate_init_bed_extra = 0;
    if is_hot {
      climate_noz_delta = if is_heat_creep_resistant_nozzle {
        -5
      } else if self.is_copolyester() || self.family == "PLA" {
        -10
      } else if self.is_flexible() {
        -6
      } else {
        -4
      };
      climate_bed_delta = if self.is_copolyester() {
        -3
      } else if self.family == "PLA" || self.is_flexible() {
        -5
      } else {
        0
      };
    } else if is_cold {
      climate_noz_delta =
        if is_heat_creep_resistant_nozzle { 4 } else { 6 } - if is_humid { 3 } else { 0 };
      climate_bed_delta = 5;
      climate_init_bed_extra = 5;
    } else if is_humid {
      climate_noz_delta = -3;
    }
    (
      climate_noz_delta,
      climate_bed_delta,
      climate_init_bed_extra,
      climate_retract_delta,
    )
  }
}

pub fn constants() -> &'static WebConfigConstants {
  static INSTANCE: OnceLock<WebConfigConstants> = OnceLock::new();
  INSTANCE.get_or_init(|| {
    serde_json::from_str(CONSTANTS_JSON).expect("Failed to parse centralized constants.json")
  })
}

impl WebConfigConstants {
  pub fn lookup_machine<'a>(&'a self, printer_model: &str) -> &'a MachineConstants {
    self
      .machines
      .iter()
      .find(|m| m.printer_model.eq_ignore_ascii_case(printer_model.trim()))
      .or_else(|| {
        let low = printer_model.to_lowercase();
        self
          .machines
          .iter()
          .find(|m| low.contains(&m.printer_model.to_lowercase()))
      })
      .unwrap_or_else(|| &self.machines[0])
  }

  /// Resolves the system filament parent preset string for a given printer model, nozzle diameter, and material type
  /// using the per-machine `inherits_template` and the polymer's printer-agnostic `base_preset_code`.
  pub fn resolve_filament_inherits(
    &self,
    printer_model: &str,
    nozzle_diameter: &str,
    material_type: &str,
  ) -> String {
    let mc = self.lookup_machine(printer_model);
    let trimmed = material_type.trim();
    let base_code = if self
      .direct_inherits_preset_codes
      .iter()
      .any(|c| c.eq_ignore_ascii_case(trimmed))
    {
      trimmed
    } else {
      &self.lookup_polymer(material_type).base_preset_code
    };
    let noz = if nozzle_diameter.trim().is_empty() {
      mc.default_nozzle_diameter.as_str()
    } else {
      nozzle_diameter.trim()
    };
    let model = if printer_model.trim().is_empty() {
      mc.printer_model.as_str()
    } else {
      printer_model.trim()
    };
    mc.inherits_template
      .replace("{vendor_prefix}", &mc.vendor_prefix)
      .replace("{base_preset_code}", base_code)
      .replace("{printer_model}", model)
      .replace("{nozzle_diameter}", noz)
  }

  /// Normalize a brand name (or return `"Generic"` if empty/All/unknown).
  pub fn normalize_brand<'a>(&'a self, brand: &str) -> &'a str {
    let b = brand.trim();
    if b.is_empty() || b.eq_ignore_ascii_case("All") || b.eq_ignore_ascii_case("Generic") {
      return "Generic";
    }
    let up = b.to_uppercase();
    for bc in &self.brands {
      if bc.name.eq_ignore_ascii_case(b) || bc.aliases.iter().any(|a| up == *a) {
        return &bc.name;
      }
    }
    for bc in &self.brands {
      if bc.name != "Generic" && bc.aliases.iter().any(|a| up.contains(a)) {
        return &bc.name;
      }
    }
    "Generic"
  }

  /// Detect a brand name from a profile title or vendor string (e.g. `"OC - Polymaker PolyLite PETG • 0.4 Hardened"` -> `"Polymaker"`).
  pub fn detect_brand_from_text<'a>(&'a self, text: &str) -> &'a str {
    let up = text.to_uppercase();
    for bc in &self.brands {
      if bc.name != "Generic" && bc.aliases.iter().any(|a| up.contains(a)) {
        return &bc.name;
      }
    }
    "Generic"
  }

  pub fn brand_order(&self, brand: &str) -> usize {
    self
      .brands
      .iter()
      .find(|b| b.name.eq_ignore_ascii_case(brand))
      .map(|b| b.order)
      .unwrap_or(20)
  }

  pub fn category_order(&self, cat: &str) -> usize {
    self
      .categories
      .iter()
      .find(|c| c.name == cat)
      .map(|c| c.order)
      .unwrap_or(10)
  }

  pub fn lookup_nozzle<'a>(&'a self, nozzle_type_or_name: &str) -> &'a NozzleConstants {
    let low = nozzle_type_or_name.to_lowercase();
    self
      .nozzles
      .iter()
      .find(|n| n.id.eq_ignore_ascii_case(nozzle_type_or_name))
      .or_else(|| {
        self
          .nozzles
          .iter()
          .find(|n| n.match_tokens.iter().any(|t| low.contains(t)))
      })
      .unwrap_or_else(|| &self.nozzles[0])
  }

  pub fn lookup_finish<'a>(&'a self, finish_code: &str) -> &'a FinishConstants {
    let f = finish_code.trim();
    self
      .finishes
      .iter()
      .find(|fc| {
        fc.code.eq_ignore_ascii_case(f)
          || fc.aliases.iter().any(|a| a.eq_ignore_ascii_case(f))
      })
      .unwrap_or_else(|| &self.finishes[0])
  }

  pub fn lookup_climate<'a>(&'a self, preset_id: &str) -> Option<&'a ClimateConstants> {
    self
      .climates
      .iter()
      .find(|c| c.id.eq_ignore_ascii_case(preset_id.trim()))
  }

  /// Resolves a finish-variant catalog code (e.g. `"PLA Wood"`, `"PETG Translucent"`, `"PLA-LW Foaming"`)
  /// to its base polymer code (`"PLA"`, `"PETG"`, etc.) via `finish_variant_mappings` in `constants.json`.
  pub fn base_material_code<'a>(&'a self, code: &'a str) -> &'a str {
    if let Some(mapped) = self.finish_variant_mappings.get(code.trim()) {
      mapped.as_str()
    } else {
      code
    }
  }

  /// Returns the sort priority for a filament code within its `(brand, category)` bucket from `constants.json`.
  pub fn polymer_sort_priority(&self, code: &str) -> usize {
    let poly = self.lookup_polymer(code);
    if code.trim().eq_ignore_ascii_case(&poly.canonical_key) {
      poly.sort_priority
    } else {
      poly.sort_priority.saturating_add(1)
    }
  }

  /// Evaluates whether a finish is compatible with a polymer strictly from `constants.json`
  /// (`compatible_finishes`, `compatible_finishes_gf`, `cf_upgrade_code`, `gf_upgrade_code`, and `aliases`).
  /// Canonical base polymers can use any finish in `compatible_finishes`; named vendor SKU lines
  /// (where `material_type` is a specific SKU rather than the canonical polymer key) only allow
  /// `"Basic"` or a finish whose alias is already part of that SKU's name.
  pub fn is_finish_compatible_with_polymer(&self, finish_code: &str, material_type: &str) -> bool {
    let fc = self.lookup_finish(finish_code);
    let mat_trimmed = material_type.trim();
    let mat_up = mat_trimmed.to_uppercase();
    let poly = self.lookup_polymer(mat_trimmed);
    let is_canonical_polymer = mat_trimmed.eq_ignore_ascii_case(&poly.canonical_key)
      || self
        .anycubic_system_filament_codes
        .iter()
        .any(|c| c.eq_ignore_ascii_case(mat_trimmed))
      || poly
        .gf_tokens
        .iter()
        .any(|t| t.eq_ignore_ascii_case(mat_trimmed));
    if !is_canonical_polymer {
      return fc.code == "Basic"
        || fc
          .aliases
          .iter()
          .any(|al| !al.is_empty() && mat_up.contains(&al.to_uppercase()));
    }
    if fc.code == "Carbon Fiber" && poly.cf_upgrade_code.is_some() {
      return mat_trimmed.eq_ignore_ascii_case(&poly.canonical_key);
    }
    if fc.code == "Glass Fiber" && poly.gf_upgrade_code.is_some() {
      return mat_trimmed.eq_ignore_ascii_case(&poly.canonical_key);
    }
    let allowed = if !poly.gf_tokens.is_empty()
      && poly.gf_tokens.iter().any(|tok| mat_up.contains(tok))
    {
      &poly.compatible_finishes_gf
    } else {
      &poly.compatible_finishes
    };
    allowed.iter().any(|f| f.eq_ignore_ascii_case(&fc.code))
  }

  /// Evaluates whether a material code or surface finish requires a hardened nozzle (`requires_hardened`)
  /// using the polymer and finish definitions in `constants.json`.
  pub fn is_abrasive_material_or_finish(&self, material_type: &str, finish_type: &str) -> bool {
    if self.lookup_polymer(material_type).requires_hardened
      || self.lookup_finish(finish_type).requires_hardened
    {
      return true;
    }
    let mt_up = material_type.to_uppercase();
    let fin_up = finish_type.to_uppercase();
    self.finishes.iter().any(|fc| {
      fc.requires_hardened
        && (mt_up.contains(&fc.code.to_uppercase())
          || fin_up.contains(&fc.code.to_uppercase())
          || fc
            .aliases
            .iter()
            .any(|a| mt_up.contains(&a.to_uppercase()) || fin_up.contains(&a.to_uppercase())))
    })
  }

  /// Returns true if a filament code or finish is an ESD / Conductive or Foaming specialty variant.
  pub fn is_esd_or_foaming(&self, material_type: &str, finish_type: &str) -> bool {
    let fc = self.lookup_finish(finish_type);
    fc.code == "Foaming"
      || fc.code == "Conductive"
      || material_type.to_uppercase().contains("ESD")
  }

  pub fn lookup_polymer<'a>(&'a self, material_type: &str) -> &'a PolymerConstants {
    let up = material_type.trim().to_uppercase();
    // Exact canonical key match first
    if let Some(exact) = self
      .polymers
      .iter()
      .find(|p| p.canonical_key.eq_ignore_ascii_case(material_type.trim()))
    {
      return exact;
    }
    for p in &self.polymers {
      for tok in &p.match_tokens {
        if tok == "PC" {
          if up == "PC" || up.contains(" PC") || up.starts_with("PC ") {
            return p;
          }
        } else if tok == "PA" {
          if up == "PA" || up.starts_with("PA ") || up.ends_with(" PA") {
            return p;
          }
        } else if tok == "PHA" {
          if up == "PHA" || up.starts_with("PHA ") || up.ends_with(" PHA") || up.contains("PHA-")
          {
            return p;
          }
        } else if tok == "PCL" {
          if up == "PCL" || up.starts_with("PCL ") {
            return p;
          }
        } else if up.contains(tok) {
          return p;
        }
      }
    }
    self.polymers.last().expect("polymers list cannot be empty")
  }

  /// Detects an intrinsic fiber/specialty finish (e.g. `"Carbon Fiber"`, `"Glass Fiber"`, `"Translucent"`)
  /// from a material code string using `finishes` in `constants.json`.
  pub fn detect_intrinsic_finish<'a>(&'a self, material_type: &str) -> Option<&'a str> {
    let up = material_type.trim().to_uppercase();
    for code in ["Carbon Fiber", "Glass Fiber", "Translucent"] {
      if let Some(fc) = self.finishes.iter().find(|f| f.code == code) {
        if up.contains(&fc.code.to_uppercase())
          || fc.aliases.iter().any(|a| up.contains(&a.to_uppercase()))
        {
          return Some(&fc.code);
        }
      }
    }
    None
  }

  /// Detects the finish (`Translucent`, `Matte`, `Silk`, `Carbon Fiber`, `Glass Fiber`, `Foaming`, etc.,
  /// defaulting to `Basic`) from profile name, inherited preset name, and material type strings.
  pub fn detect_finish_from_texts<'a>(
    &'a self,
    name: &str,
    inherits: &str,
    raw_mat: &str,
    resolved_mat: &str,
  ) -> &'a FinishConstants {
    let combined_up = format!("{} {} {} {}", name, inherits, raw_mat, resolved_mat).to_uppercase();
    let name_up = format!("{} {}", name, resolved_mat).to_uppercase();
    for code in [
      "Translucent",
      "Matte",
      "Silk",
      "Carbon Fiber",
      "Glass Fiber",
      "Foaming",
      "Wood-Filled",
      "Metal-Filled",
      "Glow-in-the-Dark",
      "Conductive",
      "Marble/Stone/Terracotta",
      "Photochromic",
    ] {
      if let Some(fc) = self.finishes.iter().find(|f| f.code == code) {
        let search_target = if matches!(code, "Translucent" | "Matte" | "Silk") {
          &combined_up
        } else {
          &name_up
        };
        if fc.aliases.iter().any(|al| {
          let al_up = al.to_uppercase();
          if al_up == "CF" || al_up == "GF" || al_up == "LW" || al_up == "UV" {
            false
          } else if al_up == "AIR" {
            search_target.contains(" AIR ")
          } else {
            search_target.contains(&al_up)
          }
        }) {
          return fc;
        }
      }
    }
    &self.finishes[0]
  }

  /// Resolves the canonical or catalog material code from a profile name, optional `filament_type` field,
  /// and optional inherited `filament_type` field using `polymers` and `finish_variant_mappings` in `constants.json`.
  pub fn detect_polymer_from_profile_texts(
    &self,
    name: &str,
    disk_filament_type: Option<&str>,
    inherited_filament_type: Option<&str>,
  ) -> String {
    let upper_name = name.to_uppercase();
    // 1. Check composite / specialty polymer entries in `self.polymers` before generic base families
    for p in &self.polymers {
      let is_generic_family_root = matches!(
        p.canonical_key.as_str(),
        "PETG" | "TPU" | "ABS" | "ASA" | "PA" | "PC" | "PLA"
      );
      if !is_generic_family_root {
        if p.match_tokens.iter().any(|tok| {
          if tok == "PHA" {
            upper_name == "PHA"
              || upper_name.starts_with("PHA ")
              || upper_name.ends_with(" PHA")
              || upper_name.contains(" PHA ")
              || upper_name.contains("PHA-")
          } else if tok == "PCL" {
            upper_name == "PCL" || upper_name.starts_with("PCL ") || upper_name.contains(" PCL ")
          } else if tok == "64D" {
            upper_name.contains("TPU 64D")
          } else {
            upper_name.contains(tok)
          }
        }) {
          return self.base_material_code(&p.canonical_key).to_string();
        }
      }
    }
    // 2. Otherwise use explicit `filament_type` from disk if present, else match remaining polymers on `name`, else inherited
    let raw = disk_filament_type
      .map(|s| s.to_string())
      .or_else(|| {
        for p in &self.polymers {
          if p.match_tokens.iter().any(|tok| {
            if tok == "PA" {
              upper_name.contains(" PA ")
            } else if tok == "PC" {
              upper_name == "PC" || upper_name.contains(" PC ") || upper_name.starts_with("PC ")
            } else {
              upper_name.contains(tok)
            }
          }) {
            return Some(p.canonical_key.clone());
          }
        }
        None
      })
      .or_else(|| inherited_filament_type.map(|s| s.to_string()))
      .unwrap_or_else(|| self.polymers.last().unwrap().canonical_key.clone());
    self.base_material_code(&raw).to_string()
  }

  /// Parses the structured WebConfig `filament_notes` header:
  /// `[OrcaSlicer WebConfig v... — Filament] <Brand> <Material> (<Finish>). Nozzle: ...`
  pub fn parse_oc_filament_notes_header(&self, notes: &str) -> Option<(String, String, String)> {
    let marker = "— Filament] ";
    let idx = notes.find(marker)?;
    let after = &notes[idx + marker.len()..];
    let header_end = after.find("). ")?;
    let header = &after[..header_end];
    let open_paren = header.rfind(" (")?;
    let finish_str = header[open_paren + 2..].trim();
    let brand_and_mat = header[..open_paren].trim();

    for bc in &self.brands {
      let prefix = format!("{} ", bc.name);
      if let Some(mat_rest) = brand_and_mat.strip_prefix(&prefix) {
        let mat_clean = mat_rest.trim();
        if !mat_clean.is_empty() {
          return Some((
            bc.name.clone(),
            mat_clean.to_string(),
            finish_str.to_string(),
          ));
        }
      }
    }
    None
  }
}
