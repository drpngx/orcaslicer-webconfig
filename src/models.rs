use serde::{Deserialize, Serialize};

/// Canonical version of the OrcaSlicer WebConfig web/desktop application,
/// sourced at compile time from `CARGO_PKG_VERSION` (`version.bzl` in Bazel / `Cargo.toml` in Cargo).
pub const WEBCONFIG_APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 4-part Semver representation of `WEBCONFIG_APP_VERSION` (`"<major>.<minor>.<patch>.0"`) required by OrcaSlicer's
/// `Semver::parse(key_values["version"])` when importing `.json` preset files from `.zip` bundles.
pub const WEBCONFIG_ORCA_PRESET_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), ".0");

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RigAddOn {
  pub id: String,
  pub name: String,
  pub category: String,
  pub effect_summary: String,
}

impl RigAddOn {
  pub fn get_catalog() -> Vec<Self> {
    vec![
      RigAddOn {
        id: "hollow_front_back_covers".to_string(),
        name: "Hollow Front/Back Toolhead Covers".to_string(),
        category: "Cooling & Mass".to_string(),
        effect_summary: "Increases ambient convective airflow across heatsink; lowers fan_min_speed -15% and exhaust fan to 40%.".to_string(),
      },
      RigAddOn {
        id: "microswiss_duct".to_string(),
        name: "MicroSwiss High-Velocity Dual-Fang Duct".to_string(),
        category: "Cooling Schedule".to_string(),
        effect_summary: "Concentrates static air pressure at nozzle tip; scales fan_max_speed 0.85x to prevent warping while keeping 100% overhang bridge fan.".to_string(),
      },
      RigAddOn {
        id: "flowtech_hotend".to_string(),
        name: "MicroSwiss FlowTech All-Metal Hotend".to_string(),
        category: "Melt Zone & Retraction".to_string(),
        effect_summary: "Leak-proof ceramic cylindrical heater; tightens retraction length to 0.65mm @ 50mm/s and boosts melt zone multiplier 1.15x.".to_string(),
      },
    ]
  }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MachineRig {
  pub id: String,
  pub name: String,
  pub printer_model: String, // "Anycubic Kobra S1", "Anycubic Kobra 3", "Voron 2.4 350mm"
  pub bed_size: String,
  pub kinematics: String,
  pub firmware: String,
  pub hotend: String,
  pub nozzle_type: String, // "hardened_steel", "diamondback", "brass", "ruby_brass", "cht"
  pub nozzle_material: String, // "Hardened Steel", "Diamondback PCD", "Brass", "Ruby Tip (Brass Body)", "Bi-Metal CHT"
  pub nozzle_name: String,
  pub nozzle_diameter: String, // "0.4", "0.6", "0.8"
  #[serde(default = "default_nozzle_hrc")]
  pub nozzle_hrc: u32, // 0 for standard Brass, 55 for Hardened Steel, 60 for Ruby-Brass, 68 for Diamondback PCD
  pub max_volumetric_flow_limit: f64, // mm³/s
  pub max_volumetric_flow: String,
  pub base_retraction_length: f64, // mm
  pub melt_zone_multiplier: f64,   // 1.0 standard, 1.15-1.25 high-flow
  pub ace_pro_units: usize,        // 1 = 4 slots, 2 = 8 slots Double ACE Pro 2
  pub addons: Vec<String>,         // e.g. ["hollow_front_back_covers", "microswiss_duct"]
  pub max_speed: String,
  pub max_accel: String,
  pub is_active: bool,
  pub applied_transformations: Vec<String>,
  pub source_disk_profile: Option<String>,
  #[serde(default)]
  pub notes: String,
  #[serde(default = "default_climate_preset")]
  pub climate_preset: String, // "temperate", "tropical", "hot_dry", "cold_dry", "cold_humid", "custom"
  #[serde(default = "default_ambient_temp_c")]
  pub ambient_temp_c: i32, // e.g. 22°C temperate, 30°C tropical, 33°C hot_dry, 14°C cold_dry, 15°C cold_humid
  #[serde(default = "default_humidity_pct")]
  pub humidity_pct: i32, // e.g. 45% RH temperate, 80% RH tropical/cold_humid, 20-25% RH dry
}

fn default_nozzle_hrc() -> u32 {
  55
}

fn default_climate_preset() -> String {
  "temperate".to_string()
}

fn default_ambient_temp_c() -> i32 {
  22
}

fn default_humidity_pct() -> i32 {
  45
}

fn default_spool_nozzle_type() -> String {
  "hardened_steel".to_string()
}

fn default_required_nozzle_hrc() -> u32 {
  3
}

/// Represents a parameter difference between an imported disk filament JSON and its inherited system base
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FilamentDeltaEntry {
  pub param_key: String,
  pub base_value: String,
  pub disk_value: String,
  pub reconstructed_by_lua: bool,
  pub rule_explanation: String,
}

impl FilamentDeltaEntry {
  /// Returns the specific Lua rule name (or override status) that produced or explains this delta,
  /// instead of a generic "MATCHES LUA RULE" label.
  pub fn rule_badge_label(&self) -> &'static str {
    if !self.reconstructed_by_lua {
      return "One-Off User Override (No Rule)";
    }
    let key = self.param_key.to_lowercase();
    let exp = self.rule_explanation.to_lowercase();
    if exp.contains("standalone") && !exp.contains("rule") {
      "Standalone Base Profile"
    } else if exp.contains("matches system hardened steel baseline") {
      "Rule: Hardened Steel Thermal (+5°C)"
    } else if key.contains("nozzle_temperature") {
      "Rule: Thermal Conductivity & Nozzle"
    } else if key.contains("plate_temp") || key.contains("bed") {
      "Rule: First-Layer & Bed Adhesion (+5°C)"
    } else if key.contains("retraction") {
      "Rule: Nozzle Retraction Schedule"
    } else if key.contains("fan") || key.contains("cooling") {
      "Rule: Rig Add-Ons & Cooling Schedule"
    } else if key.contains("volumetric") || key.contains("speed") {
      "Rule: Volumetric Speed Ceiling"
    } else {
      "Rule: Lua Transformation Stack"
    }
  }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FilamentSpool {
  pub id: String,
  pub name: String,
  pub brand: String,
  pub inherits: String,
  pub material_type: String, // "PLA", "PETG", "PCTG", "PLA-CF", "TPU", "ABS", "ASA", "PA"
  #[serde(default = "default_spool_nozzle_type")]
  pub nozzle_type: String, // e.g. "brass", "hardened_steel", "ruby_brass", "diamondback"
  #[serde(default = "default_spool_nozzle_type")]
  pub tuned_for_nozzle_type: String, // Original nozzle type this filament override was tuned for ("hardened_steel", "brass", or "any")
  #[serde(default = "default_required_nozzle_hrc")]
  pub required_nozzle_hrc: u32, // 3 for non-abrasive, 55 for CF/GF abrasive composites
  pub color_hex: String,
  pub color_name: String,
  #[serde(default)]
  pub brass_base_temp: i32, // Uncompensated Brass baseline temperature (e.g. 205°C PLA+, 230°C PETG)
  #[serde(default)]
  pub hs_base_temp: i32, // Hardened Steel baseline temperature (e.g. 220°C PLA+, 235°C PETG, 260°C PCTG)
  pub base_print_temp: i32, // Base system temperature for the active nozzle type
  pub print_temp: i32,      // Effective temperature for the active nozzle type
  pub base_bed_temp: i32,   // Base system bed temp
  pub bed_temp: i32,        // Disk/effective bed temp
  pub flow_ratio: f32,
  pub pressure_advance: f32,
  pub max_volumetric_speed: f32,
  pub fan_speed_percent: i32,
  pub remaining_grams: i32,
  pub is_loaded: bool,
  // Extended Web App Metadata
  pub finish_type: String, // "Basic", "Matte", "Silk", "Translucent", "Carbon"
  pub opacity: f32,        // 0.1 to 1.0
  pub pigment_stickiness: f32, // 0.5 to 1.8
  pub drying_temp_c: i32,
  pub drying_time_hr: i32,
  pub is_rfid_synced: bool,
  // Transformation deltas from base (`inherits`)
  pub deltas_from_base: Vec<FilamentDeltaEntry>,
  pub raw_disk_json: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProcessProfile {
  pub id: String,
  pub name: String,
  pub inherits: String,
  pub layer_height: f64,
  pub line_width: f64,
  pub outer_wall_speed: f64,
  pub inner_wall_speed: f64,
  pub sparse_infill_speed: f64,
  pub initial_layer_speed: f64,
  pub default_acceleration: f64,
  pub print_sequence: String,
  pub wall_generator: String,
  #[serde(default)]
  pub notes: String,
}

impl MachineRig {
  /// Short, scannable printer model tag for titles (e.g. "Kobra S1", "Kobra 3", "X1C", "Voron 2.4")
  pub fn short_model_tag(&self) -> String {
    let trimmed = self
      .printer_model
      .trim_start_matches("Anycubic ")
      .trim_start_matches("Bambu Lab ")
      .trim_start_matches("Creality ")
      .trim_start_matches("Prusa ")
      .trim();
    if trimmed == "X1 Carbon" {
      "X1C".to_string()
    } else if let Some((base, _)) = trimmed.split_once(" (") {
      base.trim().to_string()
    } else {
      trimmed.to_string()
    }
  }

  /// Short nozzle material tag for titles (e.g. "HS", "Diamondback", "Ruby-Brass", "MS-Brass", "CHT", "Brass")
  pub fn short_nozzle_tag(&self) -> &'static str {
    let nt = self.nozzle_type.to_lowercase();
    let nm = self.nozzle_material.to_lowercase();
    if nt.contains("diamondback") || nm.contains("diamondback") {
      "Diamondback"
    } else if nt.contains("ruby") || nm.contains("ruby") {
      "Ruby-Brass"
    } else if nt.contains("microswiss_brass") || nm.contains("twinclad") {
      "MS-Brass"
    } else if nt.contains("cht_cm2") || (nt.contains("cht") && nt.contains("cm2")) {
      "MS-CHT-CM2"
    } else if nt.contains("cm2") || nm.contains("cm2") {
      "MS-CM2"
    } else if nt.contains("cht") || nm.contains("cht") || nt.contains("obxidian") {
      "CHT-HiFlow"
    } else if nt.contains("hardened") || nm.contains("hardened") {
      "Hardened"
    } else if nt.contains("stainless") || nm.contains("stainless") {
      "Stainless"
    } else {
      "Brass"
    }
  }

  /// Returns all candidate orifice diameters `(value_str, label_str)` across the catalog (`0.2` through `1.0 mm`).
  pub fn all_candidate_nozzle_diameters() -> &'static [(&'static str, &'static str)] {
    &[
      ("0.2", "0.20 mm"),
      ("0.25", "0.25 mm"),
      ("0.3", "0.30 mm"),
      ("0.4", "0.40 mm"),
      ("0.6", "0.60 mm"),
      ("0.8", "0.80 mm"),
      ("1.0", "1.00 mm"),
    ]
  }

  /// Returns the manufactured orifice diameters `(value_str, label_str)` supported by a given `nozzle_type`:
  /// - MicroSwiss Diamondback PCD: `0.25, 0.4, 0.6, 0.8 mm`
  /// - MicroSwiss CM2 & CHT CM2: `0.4, 0.6, 0.8 mm`
  /// - MicroSwiss CHT High-Flow: `0.4, 0.6, 0.8, 1.0 mm`
  /// - MicroSwiss FlowTech Plated Brass: `0.2, 0.3, 0.4, 0.6, 0.8, 1.0 mm`
  /// - Ruby-Brass: `0.4, 0.6, 0.8 mm`
  /// - Standard Brass / Hardened Steel / MK8: `0.2, 0.25, 0.3, 0.4, 0.6, 0.8, 1.0 mm`
  pub fn supported_nozzle_diameters_for_type(
    nozzle_type: &str,
  ) -> &'static [(&'static str, &'static str)] {
    let nt = nozzle_type.to_lowercase();
    if nt.contains("diamondback") {
      &[
        ("0.25", "0.25 mm"),
        ("0.4", "0.40 mm"),
        ("0.6", "0.60 mm"),
        ("0.8", "0.80 mm"),
      ]
    } else if nt.contains("cm2") {
      &[("0.4", "0.40 mm"), ("0.6", "0.60 mm"), ("0.8", "0.80 mm")]
    } else if nt.contains("cht") {
      &[
        ("0.4", "0.40 mm"),
        ("0.6", "0.60 mm"),
        ("0.8", "0.80 mm"),
        ("1.0", "1.00 mm"),
      ]
    } else if nt == "microswiss_brass" {
      &[
        ("0.2", "0.20 mm"),
        ("0.3", "0.30 mm"),
        ("0.4", "0.40 mm"),
        ("0.6", "0.60 mm"),
        ("0.8", "0.80 mm"),
        ("1.0", "1.00 mm"),
      ]
    } else if nt.contains("ruby") {
      &[("0.4", "0.40 mm"), ("0.6", "0.60 mm"), ("0.8", "0.80 mm")]
    } else {
      Self::all_candidate_nozzle_diameters()
    }
  }

  /// Returns true if `diam` (e.g. `"0.25"`, `"0.4"`, `"1.0"`) is manufactured for `nozzle_type`.
  pub fn is_diameter_supported_for_nozzle(nozzle_type: &str, diam: &str) -> bool {
    let d_num = diam.trim().parse::<f64>().unwrap_or(0.4);
    Self::supported_nozzle_diameters_for_type(nozzle_type)
      .iter()
      .any(|(v, _)| {
        v.parse::<f64>()
          .map(|sv| (sv - d_num).abs() < 0.005)
          .unwrap_or(false)
      })
  }

  /// Normalized nozzle diameter string (e.g. `"0.4"`, `"0.6"`, `"0.8"`, `"0.2"`),
  /// repairing any accidentally truncated `"0"` back to `"0.4"`.
  pub fn formatted_nozzle_diameter(&self) -> String {
    self
      .nozzle_diameter
      .trim()
      .parse::<f64>()
      .ok()
      .filter(|&d| (0.1..=2.0).contains(&d))
      .map(|d| {
        let s = format!("{:.2}", d);
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        if trimmed.contains('.') {
          trimmed.to_string()
        } else {
          format!("{}.0", trimmed)
        }
      })
      .unwrap_or_else(|| "0.4".to_string())
  }

  /// Concise, easy-to-read title prefixed with `OC -` (no colon `:`, which is forbidden in Windows filenames!)
  /// highlighting the big hardware traits:
  /// e.g. `OC - Kobra S1 • 0.4 Hardened` or `OC - Kobra S1 • 0.4 Diamondback +FlowTech`
  pub fn format_concise_title(&self) -> String {
    let model = self.short_model_tag();
    let noz = self.short_nozzle_tag();
    let addon_tag = if self.addons.is_empty() {
      String::new()
    } else if self.addons.len() == 1 {
      let a = &self.addons[0];
      if a.contains("flowtech") || a.contains("FlowTech") {
        " +FlowTech".to_string()
      } else if a.contains("duct") || a.contains("Duct") {
        " +MS-Duct".to_string()
      } else if a.contains("hollow") || a.contains("Hollow") {
        " +Hollow".to_string()
      } else {
        " +1Mod".to_string()
      }
    } else {
      format!(" +{}Mods", self.addons.len())
    };
    format!(
      "OC - {} • {} {}{}",
      model,
      self.formatted_nozzle_diameter(),
      noz,
      addon_tag
    )
  }

  /// Returns `(orca_nozzle_type, nozzle_hrc)` for the Machine OrcaSlicer JSON profile:
  /// - Hardened Steel (`hardened_steel` / `cm2`) -> `("hardened_steel", 55)`
  /// - Ruby Tip (Brass Body) (`ruby_brass`) -> `("brass", 60)` (uses Brass thermal profile + HRC 60 so CF/GF composites are allowed!)
  /// - Diamondback PCD (`diamondback`) -> `("brass", 68)` (uses Brass/high-conductivity thermal profile + HRC 68 for extreme wear resistance!)
  /// - Plated Brass (`microswiss_brass`) -> `("brass", 35)`
  /// - Standard Brass (`brass`) -> `("brass", 3)` (matches OrcaSlicer standard Brass HRC 3 for non-abrasive filaments)
  pub fn orca_machine_nozzle_type_and_hrc(&self) -> (&'static str, u32) {
    let nt = self.nozzle_type.to_lowercase();
    let nm = self.nozzle_material.to_lowercase();
    if nt.contains("diamondback") || nm.contains("diamondback") {
      ("brass", 68)
    } else if nt.contains("ruby") || nm.contains("ruby") {
      ("brass", 60)
    } else if nt.contains("hardened")
      || nm.contains("hardened")
      || nt.contains("cm2")
      || nm.contains("cm2")
      || nt.contains("obxidian")
    {
      ("hardened_steel", 55)
    } else if nt.contains("stainless") || nm.contains("stainless") {
      ("stainless_steel", 50)
    } else if nt.contains("microswiss_brass") || nm.contains("twinclad") {
      ("brass", 35)
    } else if nt.contains("cht") || nm.contains("cht") {
      ("brass", 55)
    } else {
      ("brass", 3)
    }
  }

  /// Generate a full descriptive note (`printer_notes`) summarizing the machine rig, nozzle, add-ons, and active Lua rules
  pub fn generate_descriptive_notes(&self) -> String {
    let (orca_nt, hrc) = self.orca_machine_nozzle_type_and_hrc();
    let thermal_note = match self.short_nozzle_tag() {
      "Diamondback" => "-10°C PCD diamond conductivity offset, nozzle_type=brass + nozzle_hrc=68, 0.94 flow, 0.60mm retract @ 45mm/s",
      "Ruby-Brass" => "0°C Brass thermal conductivity, nozzle_type=brass + nozzle_hrc=60 (Ruby gemstone tip enabled for -CF/-GF composites)",
      "MS-Brass" => "0°C TwinClad XT low-friction plated brass (nozzle_type=brass, nozzle_hrc=35), 0.99 flow, 0.70mm retract @ 45mm/s",
      "MS-CM2" => "0°C Copper-core + M2 HSS tip (nozzle_type=hardened_steel, nozzle_hrc=55), 0.60mm retract @ 45mm/s",
      "CHT-HiFlow" => "+25% split-core high-flow volumetric capacity, 0.60mm retract",
      "Hardened" | "Stainless" => {
        "+5°C thermal lag compensation (nozzle_type=hardened_steel, nozzle_hrc=55), direct-drive stock retraction preserved"
      }
      _ => "0°C factory Brass thermal baseline (nozzle_type=brass, nozzle_hrc=0)",
    };
    let addons_note = if self.addons.is_empty() {
      "Stock toolhead covers & cooling duct".to_string()
    } else {
      self.addons.join(", ")
    };
    let climate_note = self.climate_summary_badge();
    format!(
      "[OrcaSlicer WebConfig v{} — Machine] {} ({}, {}). Nozzle: {}mm {} [Orca nozzle_type={}, nozzle_hrc={}] ({}; Max Flow: {}, Retract: {:.2}mm, Melt-Zone Mult: {:.2}x). Add-Ons: {}. Weather: {}.",
      WEBCONFIG_APP_VERSION,
      self.printer_model,
      self.kinematics,
      self.bed_size,
      self.nozzle_diameter,
      self.nozzle_material,
      orca_nt,
      hrc,
      thermal_note,
      self.max_volumetric_flow,
      self.base_retraction_length,
      self.melt_zone_multiplier,
      addons_note,
      climate_note
    )
  }

  /// Returns true if the active nozzle / hotend / add-ons use a MicroSwiss, FlowTech, Diamondback PCD,
  /// TwinClad XT, CM2, or CHT high-conductivity / low-friction heat-creep-resistant architecture.
  pub fn is_heat_creep_resistant_nozzle(&self) -> bool {
    let nt = self.nozzle_type.to_lowercase();
    let nm = self.nozzle_material.to_lowercase();
    let he = self.hotend.to_lowercase();
    let addons = self.addons.join(" ").to_lowercase();
    nt.contains("microswiss")
      || nm.contains("microswiss")
      || nm.contains("twinclad")
      || nt.contains("cm2")
      || nm.contains("cm2")
      || he.contains("microswiss")
      || he.contains("flowtech")
      || addons.contains("flowtech")
      || addons.contains("microswiss")
      || nt.contains("diamondback")
      || nm.contains("diamondback")
      || nt.contains("cht")
      || nm.contains("cht")
  }

  /// Apply a named weather preset ("temperate", "tropical", "hot_dry", "cold_dry", "cold_humid")
  /// and snap `ambient_temp_c` and `humidity_pct` accordingly.
  pub fn apply_weather_preset(&mut self, preset: &str) {
    self.climate_preset = preset.to_string();
    match preset {
      "tropical" => {
        self.ambient_temp_c = 30;
        self.humidity_pct = 80;
      }
      "hot_dry" => {
        self.ambient_temp_c = 33;
        self.humidity_pct = 20;
      }
      "cold_dry" => {
        self.ambient_temp_c = 14;
        self.humidity_pct = 25;
      }
      "cold_humid" => {
        self.ambient_temp_c = 15;
        self.humidity_pct = 80;
      }
      "temperate" => {
        self.ambient_temp_c = 22;
        self.humidity_pct = 45;
      }
      _ => {}
    }
  }

  /// Update ambient temperature and relative humidity dials directly and set preset to `"custom"`
  /// unless the values match a standard preset.
  pub fn apply_weather_dials(&mut self, temp_c: i32, rh_pct: i32) {
    self.ambient_temp_c = temp_c.clamp(8, 40);
    self.humidity_pct = rh_pct.clamp(10, 95);
    self.climate_preset = match (self.ambient_temp_c, self.humidity_pct) {
      (22, 45) => "temperate".to_string(),
      (30, 80) => "tropical".to_string(),
      (33, 20) => "hot_dry".to_string(),
      (14, 25) => "cold_dry".to_string(),
      (15, 80) => "cold_humid".to_string(),
      _ => "custom".to_string(),
    };
  }

  /// Human-readable badge summarizing the rig's active ambient weather & heat-creep posture.
  pub fn climate_summary_badge(&self) -> String {
    let label = match self.climate_preset.as_str() {
      "tropical" => "🌴 Tropical (Hot & Humid)",
      "hot_dry" => "☀️ Hot & Dry",
      "cold_dry" => "❄️ Cold & Dry",
      "cold_humid" => "🌧️ Cold & Humid",
      "custom" => "🎛️ Custom Dials",
      _ => "🌤️ Temperate Baseline",
    };
    let temp_f = ((self.ambient_temp_c as f64) * 9.0 / 5.0 + 32.0).round() as i32;
    format!(
      "{} ({}°C / {}°F, {}% RH)",
      label, self.ambient_temp_c, temp_f, self.humidity_pct
    )
  }

  /// Refresh `notes` and `nozzle_hrc` (and update `name` to the concise `OC:` title if it is a created/duplicated rig not bound to a legacy disk filename)
  pub fn refresh_title_and_notes(&mut self, update_title: bool) {
    let (_, hrc) = self.orca_machine_nozzle_type_and_hrc();
    self.nozzle_hrc = hrc;
    if update_title {
      self.name = self.format_concise_title();
    }
    self.notes = self.generate_descriptive_notes();
  }

  /// Construct the detected Anycubic Kobra S1 with Hardened Steel 0.4mm nozzle (`Anycubic Kobra S1 0.4 nozzle - png`)
  pub fn kobra_s1_hardened_steel_png() -> Self {
    let mut rig = Self {
      id: "kobra_s1_hs_png".to_string(),
      name: "OC - Kobra S1 • 0.4 Hardened".to_string(),
      printer_model: "Anycubic Kobra S1".to_string(),
      bed_size: "250 × 250 × 250 mm".to_string(),
      kinematics: "CoreXY Enclosed S-Series".to_string(),
      firmware: "Klipper (AnycubicSlicerNext 2.0.0.3)".to_string(),
      hotend: "Kobra S1 All-Metal Quick-Swap".to_string(),
      nozzle_type: "hardened_steel".to_string(),
      nozzle_material: "Hardened Steel".to_string(),
      nozzle_name: "Hardened Steel 0.40mm (nozzle_type = hardened_steel, HRC 55)".to_string(),
      nozzle_diameter: "0.4".to_string(),
      nozzle_hrc: 55,
      max_volumetric_flow_limit: 16.0,
      max_volumetric_flow: "16.0 mm³/s".to_string(),
      base_retraction_length: 0.80,
      melt_zone_multiplier: 1.07,
      ace_pro_units: 2, // 6+ filaments in AnycubicSlicerNext.conf -> Double ACE Pro 2 (8-Slot)
      addons: vec![],
      max_speed: "600 mm/s".to_string(),
      max_accel: "20,000 mm/s²".to_string(),
      is_active: true,
      applied_transformations: vec![
        "Thermal Conductivity & Nozzle Material Rule".to_string(),
        "Volumetric Speed Ceiling Rule".to_string(),
        "Upgraded Flushing Volume Matrix (Lua)".to_string(),
      ],
      source_disk_profile: Some(
        "user/434139/machine/Anycubic Kobra S1 0.4 nozzle - png.json".to_string(),
      ),
      notes: String::new(),
      climate_preset: "temperate".to_string(),
      ambient_temp_c: 22,
      humidity_pct: 45,
    };
    rig.notes = rig.generate_descriptive_notes();
    rig
  }

  /// Construct the second machine profile found in the user's directory: `Anycubic Kobra S1 -NewStartcode V4 -0.4n -Next`
  pub fn kobra_s1_new_startcode_v4() -> Self {
    let mut rig = Self {
      id: "kobra_s1_v4_next".to_string(),
      name: "Anycubic Kobra S1 -NewStartcode V4 -0.4n -Next".to_string(),
      printer_model: "Anycubic Kobra S1".to_string(),
      bed_size: "250 × 250 × 250 mm".to_string(),
      kinematics: "CoreXY Enclosed S-Series".to_string(),
      firmware: "Klipper (NewStartcode V4 Z-Probe Clean)".to_string(),
      hotend: "Kobra S1 All-Metal (71.6 mm³ Melt Zone)".to_string(),
      nozzle_type: "hardened_steel".to_string(),
      nozzle_material: "Hardened Steel".to_string(),
      nozzle_name: "Hardened Steel 0.40mm (V4 Fast Retract 70mm/s)".to_string(),
      nozzle_diameter: "0.4".to_string(),
      nozzle_hrc: 55,
      max_volumetric_flow_limit: 18.0,
      max_volumetric_flow: "18.0 mm³/s".to_string(),
      base_retraction_length: 0.65,
      melt_zone_multiplier: 1.00,
      ace_pro_units: 2,
      addons: vec!["hollow_front_back_covers".to_string()],
      max_speed: "600 mm/s".to_string(),
      max_accel: "20,000 mm/s²".to_string(),
      is_active: false,
      applied_transformations: vec![
        "Thermal Conductivity & Nozzle Material Rule".to_string(),
        "Physical Rig Add-Ons & Cooling Schedule Rule".to_string(),
      ],
      source_disk_profile: Some(
        "user/434139/machine/Anycubic Kobra S1 -NewStartcode V4 -0.4n -Next.json".to_string(),
      ),
      notes: String::new(),
      climate_preset: "temperate".to_string(),
      ambient_temp_c: 22,
      humidity_pct: 45,
    };
    rig.notes = rig.generate_descriptive_notes();
    rig
  }

  /// Template catalog when creating a brand new rig from scratch
  pub fn get_template_catalog() -> Vec<Self> {
    let mut tpl_diamond = Self {
      id: "template_kobra_s1_diamondback".to_string(),
      name: String::new(),
      printer_model: "Anycubic Kobra S1".to_string(),
      bed_size: "250 × 250 × 250 mm".to_string(),
      kinematics: "CoreXY Enclosed S-Series".to_string(),
      firmware: "Klipper KobraOS".to_string(),
      hotend: "MicroSwiss FlowTech All-Metal".to_string(),
      nozzle_type: "diamondback".to_string(),
      nozzle_material: "Diamondback PCD".to_string(),
      nozzle_name: "Diamondback PCD Diamond 0.40mm".to_string(),
      nozzle_diameter: "0.4".to_string(),
      nozzle_hrc: 68,
      max_volumetric_flow_limit: 24.0,
      max_volumetric_flow: "24.0 mm³/s".to_string(),
      base_retraction_length: 0.65,
      melt_zone_multiplier: 1.15,
      ace_pro_units: 2,
      addons: vec![
        "flowtech_hotend".to_string(),
        "microswiss_duct".to_string(),
        "hollow_front_back_covers".to_string(),
      ],
      max_speed: "600 mm/s".to_string(),
      max_accel: "20,000 mm/s²".to_string(),
      is_active: false,
      applied_transformations: vec![
        "Thermal Conductivity & Nozzle Material Rule".to_string(),
        "Physical Rig Add-Ons & Cooling Schedule Rule".to_string(),
        "Volumetric Speed Ceiling Rule".to_string(),
      ],
      source_disk_profile: None,
      notes: String::new(),
      climate_preset: "temperate".to_string(),
      ambient_temp_c: 22,
      humidity_pct: 45,
    };
    tpl_diamond.refresh_title_and_notes(true);

    let mut tpl_k3 = Self {
      id: "template_kobra_3_combo".to_string(),
      name: String::new(),
      printer_model: "Anycubic Kobra 3".to_string(),
      bed_size: "250 × 250 × 260 mm".to_string(),
      kinematics: "Bed-Slinger High-Speed".to_string(),
      firmware: "KobraOS v2.3".to_string(),
      hotend: "Kobra 3 Quick-Swap".to_string(),
      nozzle_type: "brass".to_string(),
      nozzle_material: "Brass".to_string(),
      nozzle_name: "Standard Brass 0.40mm".to_string(),
      nozzle_diameter: "0.4".to_string(),
      nozzle_hrc: 0,
      max_volumetric_flow_limit: 16.0,
      max_volumetric_flow: "16.0 mm³/s".to_string(),
      base_retraction_length: 0.80,
      melt_zone_multiplier: 1.00,
      ace_pro_units: 1,
      addons: vec![],
      max_speed: "600 mm/s".to_string(),
      max_accel: "15,000 mm/s²".to_string(),
      is_active: false,
      applied_transformations: vec!["Volumetric Speed Ceiling Rule".to_string()],
      source_disk_profile: None,
      notes: String::new(),
      climate_preset: "temperate".to_string(),
      ambient_temp_c: 22,
      humidity_pct: 45,
    };
    tpl_k3.refresh_title_and_notes(true);

    vec![Self::kobra_s1_hardened_steel_png(), tpl_diamond, tpl_k3]
  }
}

/// Converts a profile display name into a clean Windows-NTFS-safe `.json` filename,
/// preserving `•` (since ZIP archives use the `0x0800` UTF-8 flag) while replacing Windows-forbidden
/// path/filename characters (`:`, `/`, `\`, `*`, `?`, `"`, `<`, `>`, `|`) with `-`.
/// For example: `"OC: PETG-CF • 0.4 Hardened"` -> `"OC - PETG-CF • 0.4 Hardened.json"`.
pub fn sanitize_profile_filename(name: &str) -> String {
  let base = name.trim().trim_end_matches(".json");
  let replaced = base.replace(": ", " - ").replace(':', "-");
  let mut out = String::with_capacity(replaced.len() + 5);
  for ch in replaced.chars() {
    if ch.is_ascii_alphanumeric()
      || matches!(
        ch,
        ' ' | '-' | '_' | '.' | '+' | '@' | '(' | ')' | '#' | '•'
      )
    {
      out.push(ch);
    } else {
      out.push('-');
    }
  }
  let collapsed = out.split_whitespace().collect::<Vec<_>>().join(" ");
  format!("{}.json", collapsed.trim_matches([' ', '-']))
}

/// Sanitizes the filename portion of a `user/<folder>/<category>/<filename>` relative path,
/// splitting after the 3rd `/` (`user/<folder>/<category>/`) so that any `/` inside a profile name
/// (such as `200/300` in process speeds) is converted to `-` in the filename rather than creating a subdirectory.
pub fn sanitize_user_relative_path(rel_path: &str) -> String {
  let norm = rel_path.replace('\\', "/");
  if norm.starts_with("user/") && norm.ends_with(".json") {
    let mut parts = norm.splitn(4, '/');
    if let (Some(u), Some(folder), Some(cat), Some(rest)) =
      (parts.next(), parts.next(), parts.next(), parts.next())
    {
      if let Some(base_rest) = rest.strip_prefix("base/") {
        return format!(
          "{}/{}/{}/base/{}",
          u,
          folder,
          cat,
          sanitize_profile_filename(base_rest)
        );
      }
      return format!(
        "{}/{}/{}/{}",
        u,
        folder,
        cat,
        sanitize_profile_filename(rest)
      );
    }
  }
  norm
}

impl FilamentSpool {
  /// Returns the concise suffix used in profile titles when a specialty finish is active
  /// (e.g. `"Translucent"`, `"Wood"`, `"Metal"`, `"Glow"`, `"Foaming"`, `"Conductive"`, `"Marble"`, `"Photochromic"`).
  pub fn finish_title_suffix(finish_type: &str) -> Option<&'static str> {
    if finish_type.eq_ignore_ascii_case("Translucent") {
      Some("Translucent")
    } else if finish_type.eq_ignore_ascii_case("Wood-Filled")
      || finish_type.eq_ignore_ascii_case("Wood")
    {
      Some("Wood")
    } else if finish_type.eq_ignore_ascii_case("Metal-Filled")
      || finish_type.eq_ignore_ascii_case("Metal")
    {
      Some("Metal")
    } else if finish_type.eq_ignore_ascii_case("Glow-in-the-Dark")
      || finish_type.eq_ignore_ascii_case("Glow")
    {
      Some("Glow")
    } else if finish_type.eq_ignore_ascii_case("Foaming") {
      Some("Foaming")
    } else if finish_type.eq_ignore_ascii_case("Conductive") {
      Some("Conductive")
    } else if finish_type.eq_ignore_ascii_case("Marble/Stone/Terracotta")
      || finish_type.eq_ignore_ascii_case("Marble")
    {
      Some("Marble")
    } else if finish_type.eq_ignore_ascii_case("Photochromic") {
      Some("Photochromic")
    } else {
      None
    }
  }

  /// Normalizes a brand label (`"Custom"`, `""` -> `"Generic"`).
  pub fn normalized_brand(raw_brand: &str) -> String {
    let b = raw_brand.trim();
    if b.is_empty() || b.eq_ignore_ascii_case("Custom") || b.eq_ignore_ascii_case("Generic") {
      "Generic".to_string()
    } else if b.eq_ignore_ascii_case("Siraya") || b.eq_ignore_ascii_case("Siraya Tech") {
      "Siraya Tech".to_string()
    } else if b.eq_ignore_ascii_case("SUNLU") {
      "SUNLU".to_string()
    } else if b.eq_ignore_ascii_case("Polymaker") {
      "Polymaker".to_string()
    } else if b.eq_ignore_ascii_case("Anycubic") {
      "Anycubic".to_string()
    } else if b.eq_ignore_ascii_case("Bambu") || b.eq_ignore_ascii_case("Bambu Lab") {
      "Bambu Lab".to_string()
    } else if b.eq_ignore_ascii_case("eSUN") {
      "eSUN".to_string()
    } else if b.eq_ignore_ascii_case("Overture") {
      "Overture".to_string()
    } else {
      b.to_string()
    }
  }

  /// Builds the canonical `OC - [<Brand> ]<Material>[ <Finish>] • 0.4 <Nozzle>` preset name.
  pub fn default_oc_profile_name_with_brand(
    brand: &str,
    base_mat_code: &str,
    finish_type: &str,
    active_nozzle_type: &str,
  ) -> String {
    let nt_low = active_nozzle_type.to_lowercase();
    let short_noz = if nt_low.contains("diamondback") {
      "Diamond"
    } else if nt_low.contains("ruby") {
      "Ruby"
    } else if nt_low.contains("cht") {
      "CHT"
    } else if nt_low.contains("cm2") {
      "CM2"
    } else if nt_low.contains("hardened") {
      "Hardened"
    } else {
      "Brass"
    };
    let mat_with_finish = if let Some(suf) = Self::finish_title_suffix(finish_type) {
      if !base_mat_code.to_lowercase().contains(&suf.to_lowercase())
        && !(suf == "Foaming" && base_mat_code.to_lowercase().contains("air"))
      {
        format!("{} {}", base_mat_code, suf)
      } else {
        base_mat_code.to_string()
      }
    } else {
      base_mat_code.to_string()
    };
    let norm_b = Self::normalized_brand(brand);
    let mat_with_brand = if norm_b == "Generic"
      || mat_with_finish
        .to_lowercase()
        .starts_with(&norm_b.to_lowercase())
    {
      mat_with_finish
    } else {
      format!("{} {}", norm_b, mat_with_finish)
    };
    format!("OC - {} • 0.4 {}", mat_with_brand, short_noz)
  }

  /// Builds the canonical `OC - <Material>[ <Finish>] • 0.4 <Nozzle>` preset name.
  pub fn default_oc_profile_name(
    base_mat_code: &str,
    finish_type: &str,
    active_nozzle_type: &str,
  ) -> String {
    Self::default_oc_profile_name_with_brand(
      "Generic",
      base_mat_code,
      finish_type,
      active_nozzle_type,
    )
  }

  /// Concise, scannable `OC -` display name for the filament spool including its nozzle type tag
  /// (never uses `:`, which is forbidden in Windows filenames when OrcaSlicer saves imported presets!)
  pub fn format_concise_title(&self) -> String {
    if self.id.starts_with("disk_")
      && !self.name.starts_with("OC:")
      && !self.name.starts_with("OC -")
    {
      return self.name.replace(": ", " - ").replace(':', "-");
    }
    let cleaned = self.name.replace(" - 0.", " • 0.");
    let base_name = if let Some(rest) = cleaned.strip_prefix("OC: ") {
      rest
    } else if let Some(rest) = cleaned.strip_prefix("OC - ") {
      rest
    } else if let Some(rest) = cleaned.strip_prefix("OC:") {
      rest.trim()
    } else if let Some(rest) = cleaned.strip_prefix("OC -") {
      rest.trim()
    } else {
      cleaned.as_str()
    };
    let mut safe_base = base_name.replace(": ", " - ").replace(':', "-");
    if let Some(suf) = Self::finish_title_suffix(&self.finish_type) {
      if !safe_base.to_lowercase().contains(&suf.to_lowercase()) {
        let prefix = format!("{} •", self.material_type);
        if safe_base.starts_with(&prefix) {
          safe_base = safe_base.replacen(&prefix, &format!("{} {} •", self.material_type, suf), 1);
        }
      }
    }
    format!("OC - {}", safe_base)
  }

  /// Recomputes `base_print_temp` and `print_temp` when switching the Machine Rig to `new_nozzle_type`,
  /// ensuring that values tuned specifically for a Hardened Steel nozzle (`_HS`) are NEVER used on
  /// a Brass, Ruby-Brass, or Diamondback PCD nozzle!
  pub fn recompute_for_nozzle(&mut self, new_nozzle_type: &str) {
    let brass = if self.brass_base_temp > 0 {
      self.brass_base_temp
    } else {
      (self.base_print_temp - 5).max(190)
    };
    let hs = if self.hs_base_temp > 0 {
      self.hs_base_temp
    } else {
      brass + 5
    };
    let nt = new_nozzle_type.to_lowercase();
    let is_external_disk_spool = self.id.starts_with("disk_")
      && !self.name.starts_with("OC -")
      && !self.name.starts_with("OC:");
    self.nozzle_type = new_nozzle_type.to_string();
    if !is_external_disk_spool {
      self.tuned_for_nozzle_type = new_nozzle_type.to_string();
    }

    if nt.contains("hardened") || nt.contains("stainless") || nt.contains("cm2") {
      self.base_print_temp = hs;
      // Only preserve a custom Hardened Steel disk override on an external non-OC disk profile within +15°C of `hs`
      if is_external_disk_spool
        && self.tuned_for_nozzle_type.contains("hardened")
        && self.print_temp > hs
        && self.print_temp <= hs + 15
      {
        // Keep external user's HS override
      } else {
        self.print_temp = hs;
      }
    } else if nt.contains("diamondback") {
      self.base_print_temp = brass;
      self.print_temp = (brass - 10).max(80);
    } else {
      // Standard Brass, Ruby Tip (Brass Body), MicroSwiss Plated Brass, or CHT:
      // Use the uncompensated Brass baseline (NEVER the Hardened Steel _HS temperature!)
      self.base_print_temp = brass;
      self.print_temp = brass;
    }

    if !is_external_disk_spool
      && (self.name.starts_with("OC -") || self.name.starts_with("OC:"))
    {
      let new_short_noz = if nt.contains("diamondback") {
        "Diamond"
      } else if nt.contains("ruby") {
        "Ruby"
      } else if nt.contains("cht") {
        "CHT"
      } else if nt.contains("cm2") {
        "CM2"
      } else if nt.contains("hardened") {
        "Hardened"
      } else {
        "Brass"
      };
      if let Some(dot_idx) = self.name.rfind("• 0.") {
        let prefix = &self.name[..dot_idx];
        let after_dot = &self.name[dot_idx + "• ".len()..];
        let diam_token = after_dot.split_whitespace().next().unwrap_or("0.4");
        self.name = format!("{}• {} {}", prefix, diam_token, new_short_noz);
      } else {
        self.name = Self::default_oc_profile_name_with_brand(
          &self.brand,
          &self.material_type,
          &self.finish_type,
          new_nozzle_type,
        );
      }
    }
  }

  /// Returns a compatibility warning if:
  /// 1. The filament is abrasive (`required_nozzle_hrc > 3 && required_nozzle_hrc > rig_nozzle_hrc`, e.g. `-CF`/`-GF` on a Brass nozzle), OR
  /// 2. The filament had a disk override tuned for Hardened Steel (`_HS`) and the active rig is using a different nozzle type.
  pub fn compatibility_warning(
    &self,
    rig_nozzle_type: &str,
    rig_nozzle_hrc: u32,
  ) -> Option<String> {
    let eff_rig_hrc = rig_nozzle_hrc.max(3);
    if self.required_nozzle_hrc > 3 && self.required_nozzle_hrc > eff_rig_hrc {
      return Some(format!(
        "Incompatible Nozzle Hardness: {} requires HRC ≥ {} (Active nozzle is HRC {}). Use Hardened Steel (HRC 55), Ruby-Brass (HRC 60), or Diamondback (HRC 68).",
        self.material_type, self.required_nozzle_hrc, eff_rig_hrc
      ));
    }
    let rig_is_hs = rig_nozzle_type.contains("hardened")
      || rig_nozzle_type.contains("stainless")
      || rig_nozzle_type.contains("cm2");
    let tuned_is_hs = self.tuned_for_nozzle_type.contains("hardened");
    if tuned_is_hs && !rig_is_hs {
      return Some(format!(
        "Disk profile was tuned for Hardened Steel (_HS); switched to clean Brass baseline ({}°C → {}°C) so Hardened Steel values are not used on {}.",
        self.hs_base_temp, self.print_temp, rig_nozzle_type
      ));
    }
    None
  }

  /// Maps any material code (including vendor-specific codes like `"PolyLite PETG"`,
  /// `"Polymaker PETG"`, `"High Speed Matte PETG"`, `"Fibreheart PETG-CF"`, `"Flex TPU 95A HF"`,
  /// `"Panchroma PLA"`, etc.) to its canonical polymer key so switching brands preserves the selected polymer.
  pub fn canonical_polymer_key(mat: &str) -> &'static str {
    let up = mat.trim().to_uppercase();
    if up.contains("PCTG-CF") || up.contains("PCTG-GF") {
      "PCTG-CF"
    } else if up.contains("PCTG") {
      "PCTG"
    } else if up.contains("PETG-CF") || up.contains("PETG-GF") {
      "PETG-CF"
    } else if up.contains("PETG") {
      "PETG"
    } else if up.contains("PET-CF") || up.contains("PET-GF") {
      "PET-CF"
    } else if up.contains("TPU-CF") || up.contains("TPU-GF") {
      "TPU-GF"
    } else if up.contains("TPU FOR AMS") || up.contains("TPU FOR ACE") || up.contains("TPU 64D") {
      "TPU for AMS"
    } else if up.contains("TPU 85A") {
      "TPU 85A"
    } else if up.contains("TPU 90A") {
      "TPU 90A"
    } else if up.contains("TPU 95A") {
      "TPU 95A"
    } else if up.contains("TPU") || up.contains("TPE") {
      "TPU"
    } else if up.contains("PEBA") {
      "PEBA"
    } else if up.contains("PC-CF") || up.contains("PC-GF") {
      "PC-CF"
    } else if up.contains("PC-ABS")
      || up.contains("PC/ABS")
      || up == "PC"
      || up.contains(" PC")
      || up.starts_with("PC ")
    {
      "PC"
    } else if up.contains("ASA-CF") || up.contains("ASA-GF") {
      "ASA-CF"
    } else if up.contains("ASA") {
      "ASA"
    } else if up.contains("ABS-CF") || up.contains("ABS-GF") {
      "ABS-CF"
    } else if up.contains("ABS") {
      "ABS"
    } else if up.contains("PEEK") {
      "PEEK"
    } else if up.contains("PPS") {
      "PPS"
    } else if up.contains("PPA-CF") || up.contains("PPA-GF") || up.contains("PPA") {
      "PPA-CF"
    } else if up.contains("PAHT-CF") || up.contains("PAHT-GF") || up.contains("PAHT") {
      "PAHT-CF"
    } else if up.contains("PA6-CF") || up.contains("PA6-GF") || up.contains("PA-CF") {
      "PA6-CF"
    } else if up.contains("PVA") {
      "PVA"
    } else if up.contains("BVOH") {
      "BVOH"
    } else if up.contains("HIPS") {
      "HIPS"
    } else if up.contains("PA12-CF") || up.contains("PA12-GF") {
      "PA12-CF"
    } else if up.contains("PA12") || up.contains("PA6") || up == "PA" || up.contains("NYLON") {
      "PA"
    } else if up == "PHA" || up.starts_with("PHA ") || up.ends_with(" PHA") {
      "PHA"
    } else if up == "PCL" || up.starts_with("PCL ") {
      "PCL"
    } else if up.contains("PLA-CF") || up.contains("PLA-GF") || up.contains("PLA CARBON") {
      "PLA-CF"
    } else if up.contains("PLA+") || up.contains("PLA PLUS") {
      "PLA+"
    } else {
      "PLA"
    }
  }

  /// Broad polymer family key for inheritance compatibility & fallback matching across brands.
  pub fn polymer_family_key(mat: &str) -> &'static str {
    match Self::canonical_polymer_key(mat) {
      "PCTG" | "PCTG-CF" | "PETG" | "PETG-CF" | "PET-CF" => "PETG",
      "TPU for AMS" | "TPU 85A" | "TPU 90A" | "TPU 95A" | "TPU" | "TPU-GF" | "PEBA" => "TPU",
      "ASA" | "ASA-CF" => "ASA",
      "ABS" | "ABS-CF" => "ABS",
      "PC" | "PC-CF" | "PPS" | "PEEK" => "PC",
      "PPA-CF" | "PAHT-CF" | "PA6-CF" | "PA12-CF" | "PA" => "PA",
      "PVA" | "BVOH" | "HIPS" => "SUPPORT",
      "PCL" => "PCL",
      _ => "PLA",
    }
  }

  /// Computes a multi-material-compatible `[nozzle_temperature_range_low, nozzle_temperature_range_high]`
  /// window for OrcaSlicer / AnycubicSlicerNext so single-extruder multi-material prints (ACE Pro / AMS)
  /// mixing compatible filaments (e.g. `TPU for AMS` at 220–230°C + `PETG` at 230–255°C, or `PLA` + `TPU` / `PETG` supports)
  /// do not trigger false "Selected Nozzle Temperature are incompatible, which may result in clogging" errors.
  pub fn compute_nozzle_temp_range(
    material_type: &str,
    effective_temp: i32,
    initial_temp: i32,
  ) -> (i32, i32) {
    let canon = Self::canonical_polymer_key(material_type);
    if canon == "PCL" {
      return (70, 180);
    }
    let max_t = effective_temp.max(initial_temp);
    let min_t = effective_temp.min(initial_temp);
    if max_t > 265
      || matches!(
        canon,
        "ABS" | "ABS-CF" | "ASA" | "ASA-CF" | "PC" | "PC-CF" | "PA" | "PA6-CF" | "PA12-CF"
          | "PAHT-CF" | "PPA-CF" | "PET-CF"
      )
    {
      let low = 215.min(min_t - 35).max(185);
      let high = 305.max(max_t + 20).min(350);
      (low, high)
    } else {
      // Standard/medium-temp ACE Pro / AMS filaments (PLA, PLA+, PHA, PVA, TPU, TPU for AMS, PEBA, PETG, PETG-CF, PCTG):
      // Unified 185°C..270°C safe extrusion window so TPU for AMS (220–230°C), PETG (230–255°C), and PLA (200–225°C)
      // can be loaded together on the same single-extruder toolhead without clogging range errors.
      let low = 185.min(min_t - 15).max(175);
      let high = 270.max(max_t + 15).min(320);
      (low, high)
    }
  }

  /// Updates the spool's brand (`"Generic"`, `"Siraya Tech"`, `"SUNLU"`, `"Polymaker"`, `"Anycubic"`, `"Bambu Lab"`, `"eSUN"`, `"Overture"`)
  /// and resolves the best matching material preset for that brand while **strictly preserving** the selected polymer
  /// (e.g. if `PETG` is selected and the user switches brand, it keeps `PETG` and never resets to the first filament of the brand).
  pub fn apply_brand_change(&mut self, new_brand: &str, active_nozzle_type: &str) {
    let norm_b = Self::normalized_brand(new_brand);
    self.brand = norm_b.clone();
    let catalog = crate::catalog::load_merged_catalog();
    let target_canon = Self::canonical_polymer_key(&self.material_type);
    let cur_finish = self.finish_type.clone();

    let is_specialty_line = |code: &str| -> bool {
      let up = code.to_uppercase();
      up.contains("ESD") || up.contains("CONDUCTIVE") || up.contains("GLOW") || up.contains("WOOD")
    };

    // 1. Exact `(brand, material_type)` or `base_material_code()` match
    let matched = catalog
      .filaments
      .iter()
      .find(|f| {
        f.brand.eq_ignore_ascii_case(&norm_b)
          && (f.code.eq_ignore_ascii_case(&self.material_type)
            || f
              .base_material_code()
              .eq_ignore_ascii_case(&self.material_type))
      })
      // 2. Same canonical polymer within `new_brand` with matching finish (preferring standard lines over ESD/Glow)
      .or_else(|| {
        catalog.filaments.iter().find(|f| {
          f.brand.eq_ignore_ascii_case(&norm_b)
            && Self::canonical_polymer_key(&f.code) == target_canon
            && f.default_finish.eq_ignore_ascii_case(&cur_finish)
            && !is_specialty_line(&f.code)
        })
      })
      // 3. Same canonical polymer within `new_brand` (preferring non-specialty line)
      .or_else(|| {
        catalog.filaments.iter().find(|f| {
          f.brand.eq_ignore_ascii_case(&norm_b)
            && Self::canonical_polymer_key(&f.code) == target_canon
            && !is_specialty_line(&f.code)
        })
      })
      .or_else(|| {
        catalog.filaments.iter().find(|f| {
          f.brand.eq_ignore_ascii_case(&norm_b)
            && Self::canonical_polymer_key(&f.code) == target_canon
        })
      })
      .cloned();

    if let Some(m) = matched {
      let target_code = m.code.clone();
      self.apply_material_change(&target_code, active_nozzle_type);
      self.brand = norm_b;
      if !cur_finish.eq_ignore_ascii_case(&self.finish_type) {
        self.apply_finish_change(&cur_finish, active_nozzle_type);
      }
      self.name = Self::default_oc_profile_name_with_brand(
        &self.brand,
        &self.material_type,
        &self.finish_type,
        active_nozzle_type,
      );
      self.rebuild_synthetic_disk_json(active_nozzle_type);
    } else {
      // Brand has no dedicated catalog entry for this polymer (e.g., Overture + PETG, or Siraya Tech + PETG):
      // Keep the user's selected polymer (`target_canon`) and finish (`cur_finish`) and apply the new brand!
      let preserved_mat = if Self::canonical_polymer_key(&self.material_type) == target_canon
        && matches!(
          self.material_type.as_str(),
          "PLA"
            | "PLA+"
            | "PLA-CF"
            | "PETG"
            | "PETG-CF"
            | "PCTG"
            | "TPU"
            | "TPU for AMS"
            | "TPU 95A"
            | "TPU 85A"
            | "ASA"
            | "ABS"
            | "ABS-CF"
            | "PC"
            | "PA"
            | "PA6-CF"
            | "PVA"
            | "PHA"
        )
      {
        self.material_type.clone()
      } else {
        target_canon.to_string()
      };
      self.apply_material_change(&preserved_mat, active_nozzle_type);
      self.brand = norm_b;
      if !cur_finish.eq_ignore_ascii_case(&self.finish_type) {
        self.apply_finish_change(&cur_finish, active_nozzle_type);
      }
      self.name = Self::default_oc_profile_name_with_brand(
        &self.brand,
        &self.material_type,
        &self.finish_type,
        active_nozzle_type,
      );
      self.rebuild_synthetic_disk_json(active_nozzle_type);
    }
  }

  /// Updates the spool's material type (`PLA`, `PLA+`, `PETG`, `PCTG`, `PLA-CF`, `TPU`, `ASA`, `ABS`, or vendor-specific code),
  /// recalculating its Brass/Hardened-Steel base temperatures, bed temperature, max volumetric speed,
  /// ACE Pro dryer preset, nozzle hardness requirement, and profile title/inheritance.
  pub fn apply_material_change(&mut self, new_material: &str, active_nozzle_type: &str) {
    let catalog = crate::catalog::load_merged_catalog();
    let norm_brand = Self::normalized_brand(&self.brand);
    let target_canon = Self::canonical_polymer_key(new_material);
    let mat_entry = catalog
      .filaments
      .iter()
      .find(|f| {
        f.brand.eq_ignore_ascii_case(&norm_brand)
          && (f.code.eq_ignore_ascii_case(new_material)
            || f.base_material_code().eq_ignore_ascii_case(new_material))
      })
      .or_else(|| {
        catalog.filaments.iter().find(|f| {
          f.brand.eq_ignore_ascii_case(&norm_brand)
            && Self::canonical_polymer_key(&f.code) == target_canon
            && f.default_finish.eq_ignore_ascii_case(&self.finish_type)
        })
      })
      .or_else(|| {
        catalog.filaments.iter().find(|f| {
          f.brand.eq_ignore_ascii_case(&norm_brand)
            && Self::canonical_polymer_key(&f.code) == target_canon
        })
      })
      .or_else(|| {
        catalog.filaments.iter().find(|f| {
          f.brand == "Generic"
            && (f.code.eq_ignore_ascii_case(new_material)
              || f.base_material_code().eq_ignore_ascii_case(new_material))
        })
      })
      .or_else(|| {
        catalog
          .filaments
          .iter()
          .find(|f| f.code.eq_ignore_ascii_case(new_material))
      })
      .or_else(|| {
        catalog
          .filaments
          .iter()
          .find(|f| Self::canonical_polymer_key(&f.code) == target_canon)
      });

    // Preserve the user's selected brand (do NOT reset `self.brand` to "Generic" when using a generic polymer baseline)
    self.brand = norm_brand;

    let (brass_temp, bed_temp, max_vol, fan_pct, dry_c, dry_hr) = if let Some(m) = mat_entry {
      (
        m.base_nozzle_c as i32,
        m.base_bed_c as i32,
        m.max_vol_mm3s,
        m.fan_pct as i32,
        m.dry_temp_c as i32,
        m.dry_hr as i32,
      )
    } else {
      match new_material.to_uppercase().as_str() {
        "PLA+" => (210, 60, 18.0, 100, 45, 4),
        "PLA-CF" | "PLA-GF" => (220, 60, 14.0, 80, 50, 4),
        "PETG" | "PETG-CF" => (235, 70, 13.0, 40, 55, 6),
        "PCTG" => (255, 75, 16.0, 35, 60, 6),
        "TPU" => (225, 50, 5.0, 80, 55, 6),
        "ASA" | "ABS" => (255, 100, 16.0, 20, 65, 6),
        "PA-CF" | "PA" => (280, 90, 12.0, 15, 70, 8),
        _ => (215, 60, 16.0, 100, 45, 4),
      }
    };

    let hs_temp = if new_material.eq_ignore_ascii_case("PLA+") && self.brand == "Generic" {
      220
    } else {
      (brass_temp + 5).clamp(80, 420)
    };

    let base_mat_code = mat_entry
      .map(|m| m.base_material_code().to_string())
      .unwrap_or_else(|| match new_material {
        "PETG Translucent" => "PETG".to_string(),
        "PLA Matte" | "PLA Silk" => "PLA".to_string(),
        other => other.to_string(),
      });

    self.material_type = base_mat_code.clone();
    self.brass_base_temp = brass_temp;
    self.hs_base_temp = hs_temp;
    self.base_print_temp = brass_temp;
    self.print_temp = brass_temp;
    self.base_bed_temp = bed_temp;
    self.bed_temp = bed_temp;
    self.max_volumetric_speed = max_vol;
    self.fan_speed_percent = fan_pct;
    self.drying_temp_c = dry_c;
    self.drying_time_hr = dry_hr;

    let is_abrasive = mat_entry.map(|m| m.requires_hardened).unwrap_or(false)
      || new_material.contains("-CF")
      || new_material.contains("-GF")
      || new_material.contains("Glow");
    self.required_nozzle_hrc = if is_abrasive { 55 } else { 3 };
    if let Some(m) = mat_entry {
      self.finish_type = m.default_finish.clone();
    } else if new_material.contains("-CF") {
      self.finish_type = "Carbon Fiber".to_string();
    } else if new_material.contains("-GF") {
      self.finish_type = "Glass Fiber".to_string();
    } else if new_material.to_lowercase().contains("translucent") {
      self.finish_type = "Translucent".to_string();
    } else if self.finish_type == "Carbon Fiber" || self.finish_type == "Glass Fiber" {
      self.finish_type = "Basic".to_string();
    }

    if let Some(m) = mat_entry {
      if m.default_flow_ratio > 0.45 && m.default_flow_ratio <= 1.05 {
        self.flow_ratio = m.default_flow_ratio;
      } else if base_mat_code.contains("PCTG") || base_mat_code.contains("PETG") {
        self.flow_ratio = 0.95;
      } else {
        self.flow_ratio = 0.98;
      }
    } else if base_mat_code.contains("PCTG") {
      self.flow_ratio = 0.95;
      self.pressure_advance = 0.048;
    } else if base_mat_code.contains("PETG") {
      self.flow_ratio = 0.95;
      self.pressure_advance = 0.045;
    } else {
      self.flow_ratio = 0.98;
      self.pressure_advance = 0.035;
    }

    self.inherits = crate::threemf::resolve_kobra_s1_filament_inherits(self);
    self.name = Self::default_oc_profile_name_with_brand(
      &self.brand,
      &base_mat_code,
      &self.finish_type,
      active_nozzle_type,
    );
    self.deltas_from_base.clear();
    self.recompute_for_nozzle(active_nozzle_type);
    let fin = self.finish_type.clone();
    self.apply_finish_change(&fin, active_nozzle_type);
  }

  /// Updates the spool's finish (`Basic`, `Matte`, `Silk`, `Translucent`, `Carbon Fiber`, `Glass Fiber`,
  /// `Wood-Filled`, `Metal-Filled`, `Glow-in-the-Dark`, `Foaming`, `Conductive`, `Marble/Stone/Terracotta`, `Photochromic`)
  /// and synchronizes its optical opacity, pigment stickiness, nozzle hardness requirement, flow ratio, volumetric ceiling,
  /// and synthetic JSON so picking a base polymer (e.g. `PLA`) and flipping its finish (e.g. to `Wood-Filled` or `Foaming`)
  /// produces the exact same spool as clicking `PLA Wood` or `PLA-LW Foaming` directly in the catalog.
  pub fn apply_finish_change(&mut self, new_finish: &str, active_nozzle_type: &str) {
    self.finish_type = new_finish.to_string();

    // Look up whether the catalog has a dedicated entry for `(self.brand, self.material_type, new_finish)`
    let catalog = crate::catalog::load_merged_catalog();
    let norm_brand = Self::normalized_brand(&self.brand);
    let variant_entry = if new_finish.eq_ignore_ascii_case("Basic") {
      None
    } else {
      catalog
        .filaments
        .iter()
        .find(|f| {
          f.brand.eq_ignore_ascii_case(&norm_brand)
            && f
              .base_material_code()
              .eq_ignore_ascii_case(&self.material_type)
            && f.default_finish.eq_ignore_ascii_case(new_finish)
            && f.code != self.material_type
        })
        .or_else(|| {
          catalog.filaments.iter().find(|f| {
            f.base_material_code()
              .eq_ignore_ascii_case(&self.material_type)
              && f.default_finish.eq_ignore_ascii_case(new_finish)
              && f.code != self.material_type
          })
        })
    };

    let resolved_entry = variant_entry.or_else(|| {
      catalog
        .filaments
        .iter()
        .find(|f| {
          f.brand.eq_ignore_ascii_case(&norm_brand)
            && (f.code.eq_ignore_ascii_case(&self.material_type)
              || f
                .base_material_code()
                .eq_ignore_ascii_case(&self.material_type))
        })
        .or_else(|| {
          let canon = Self::canonical_polymer_key(&self.material_type);
          catalog
            .filaments
            .iter()
            .find(|f| {
              f.brand.eq_ignore_ascii_case(&norm_brand)
                && Self::canonical_polymer_key(&f.code) == canon
            })
            .or_else(|| {
              catalog
                .filaments
                .iter()
                .find(|f| Self::canonical_polymer_key(&f.code) == canon)
            })
        })
    });

    if let Some(m) = resolved_entry {
      self.brass_base_temp = m.base_nozzle_c as i32;
      self.hs_base_temp = if self.material_type.eq_ignore_ascii_case("PLA+")
        && self.brand == "Generic"
        && variant_entry.is_none()
      {
        220
      } else {
        (self.brass_base_temp + 5).clamp(80, 420)
      };
      self.base_print_temp = self.brass_base_temp;
      self.print_temp = self.brass_base_temp;
      self.base_bed_temp = m.base_bed_c as i32;
      self.bed_temp = m.base_bed_c as i32;
      self.max_volumetric_speed = m.max_vol_mm3s;
      self.fan_speed_percent = m.fan_pct as i32;
      self.drying_temp_c = m.dry_temp_c as i32;
      self.drying_time_hr = m.dry_hr as i32;
      if m.default_flow_ratio > 0.45 && (m.default_flow_ratio - 0.98).abs() > 0.005 {
        self.flow_ratio = m.default_flow_ratio;
      }
      self.recompute_for_nozzle(active_nozzle_type);
    }

    if new_finish == "Translucent" {
      if self.opacity > 0.6 {
        self.opacity = 0.35;
      }
      self.pigment_stickiness = 1.0;
      if self.material_type == "PETG" {
        self.base_bed_temp = 65;
        self.bed_temp = 65;
        self.max_volumetric_speed = 9.0;
        self.fan_speed_percent = 30;
        self.pressure_advance = 0.050;
        self.drying_temp_c = 65;
        self.drying_time_hr = 6;
      }
    } else if new_finish == "Glow-in-the-Dark" || new_finish == "Photochromic" {
      if self.opacity > 0.75 {
        self.opacity = 0.65;
      }
      self.pigment_stickiness = if new_finish == "Glow-in-the-Dark" {
        1.25
      } else {
        1.05
      };
    } else {
      if self.opacity < 0.7 {
        self.opacity = 1.0;
      }
      self.pigment_stickiness = if new_finish == "Conductive" {
        1.45
      } else if new_finish == "Metal-Filled" {
        1.30
      } else if new_finish == "Wood-Filled" || new_finish == "Silk" {
        1.20
      } else if new_finish == "Carbon Fiber" || self.material_type.contains("-CF") {
        1.15
      } else if new_finish == "Marble/Stone/Terracotta" {
        1.15
      } else if new_finish == "Glass Fiber" || self.material_type.contains("-GF") {
        1.10
      } else {
        1.0
      };
      if self.material_type == "PETG" && variant_entry.is_none() {
        self.base_bed_temp = 70;
        self.bed_temp = 70;
        self.max_volumetric_speed = 12.0;
        self.fan_speed_percent = 40;
        self.pressure_advance = 0.045;
        self.drying_temp_c = 60;
        self.drying_time_hr = 6;
      }
    }

    // Flow ratio & MVS adjustments for Foaming, Wood, Metal, and Conductive finishes
    if new_finish == "Foaming" {
      if self.is_foaming_tpu() {
        let shore = self.effective_foaming_shore_a();
        self.apply_foaming_shore_hardness(shore, active_nozzle_type);
        return;
      }
      // Active foaming PLA/ASA filaments (LW-PLA / LW-ASA) expand 1.5x–1.8x in the nozzle
      self.flow_ratio = 0.58;
      if self.max_volumetric_speed > 6.5 {
        self.max_volumetric_speed = 6.0;
      }
    } else if self.flow_ratio < 0.80 {
      // Restore normal non-foaming flow ratio if switching away from Foaming
      self.flow_ratio =
        if self.material_type.contains("PCTG") || self.material_type.contains("PETG") {
          0.95
        } else {
          0.98
        };
    }

    // Update `OC - <Material>[ <Finish>] • ...` title suffix cleanly
    if let Some((left, right)) = self.name.split_once(" • ") {
      let base_prefix = format!("OC - {}", self.material_type);
      if left.starts_with(&base_prefix) {
        let new_left = if let Some(suf) = Self::finish_title_suffix(new_finish) {
          format!("{} {}", base_prefix, suf)
        } else {
          base_prefix
        };
        self.name = format!("{} • {}", new_left, right);
      }
    }

    let is_abrasive = new_finish.contains("Carbon")
      || new_finish.contains("Glass")
      || new_finish.contains("Wood")
      || new_finish.contains("Metal")
      || new_finish.contains("Glow")
      || new_finish.contains("Conductive")
      || new_finish.contains("Marble")
      || self.material_type.contains("-CF")
      || self.material_type.contains("-GF")
      || self.material_type.contains("Glow")
      || self.material_type.starts_with("PA");
    self.required_nozzle_hrc = if is_abrasive { 55 } else { 3 };
    self.rebuild_synthetic_disk_json(active_nozzle_type);
  }

  /// Returns true if this spool is an active-foaming flexible filament (`TPU`, `Rebound TPU`, `PEBA`, `TPE` with `Foaming` finish).
  pub fn is_foaming_tpu(&self) -> bool {
    let mt = self.material_type.to_uppercase();
    (mt.contains("TPU") || mt.contains("PEBA") || mt.contains("TPE"))
      && self.finish_type.eq_ignore_ascii_case("Foaming")
  }

  /// Returns true if this spool uses Siraya Tech's high-temperature foaming curve (`250°C–270°C` TPU Air / PEBA Air / Roamr Air).
  pub fn is_siraya_foaming_air(&self) -> bool {
    let b = self.brand.to_lowercase();
    let mt = self.material_type.to_lowercase();
    let nm = self.name.to_lowercase();
    (b.contains("siraya") || mt.contains("siraya") || nm.contains("siraya") || mt.contains("air"))
      && self.is_foaming_tpu()
  }

  /// Computes `(brass_base_temp_c, flow_ratio, max_volumetric_speed_mm3s, fan_speed_pct)` for a target Shore A hardness (`55..=92`)
  /// on Siraya Tech Flex TPU Air / PEBA Air (`240–270°C` foaming window, `9.0 mm³/s` MVS, `0mm` retraction).
  pub fn compute_siraya_foaming_tpu_curve(shore_a: u32) -> (i32, f32, f32, i32) {
    let s = shore_a.clamp(55, 92) as f32;
    if s >= 85.0 {
      (235, 0.96, 9.0, 100)
    } else if s >= 75.0 {
      let t = (85.0 - s) / 10.0;
      (
        (235.0 + t * 15.0).round() as i32,
        ((0.96 - t * 0.29) * 100.0).round() / 100.0,
        9.0,
        80,
      )
    } else if s >= 70.0 {
      let t = (75.0 - s) / 5.0;
      (
        (250.0 + t * 10.0).round() as i32,
        ((0.67 - t * 0.11) * 100.0).round() / 100.0,
        9.0,
        70,
      )
    } else if s >= 65.0 {
      let t = (70.0 - s) / 5.0;
      (
        (260.0 + t * 10.0).round() as i32,
        ((0.56 - t * 0.04) * 100.0).round() / 100.0,
        9.0,
        60,
      )
    } else {
      (275, 0.50, 7.5, 50)
    }
  }

  /// Computes `(brass_base_temp_c, flow_ratio, max_volumetric_speed_mm3s, fan_speed_pct)` for a target Shore A hardness (`55..=92`)
  /// on active-foaming / varioShore TPU.
  pub fn compute_foaming_tpu_curve(shore_a: u32) -> (i32, f32, f32, i32) {
    let s = shore_a.clamp(55, 92) as f32;
    if s >= 85.0 {
      let t = (92.0 - s) / 7.0;
      (
        (195.0 + t * 15.0).round() as i32,
        ((0.98 - t * 0.13) * 100.0).round() / 100.0,
        ((4.5 - t * 0.5) * 10.0).round() / 10.0,
        (70.0 - t * 20.0).round() as i32,
      )
    } else if s >= 75.0 {
      let t = (85.0 - s) / 10.0;
      (
        (210.0 + t * 10.0).round() as i32,
        ((0.85 - t * 0.10) * 100.0).round() / 100.0,
        ((4.0 - t * 0.5) * 10.0).round() / 10.0,
        (50.0 - t * 10.0).round() as i32,
      )
    } else if s >= 65.0 {
      let t = (75.0 - s) / 10.0;
      (
        (220.0 + t * 15.0).round() as i32,
        ((0.75 - t * 0.10) * 100.0).round() / 100.0,
        ((3.5 - t * 0.5) * 10.0).round() / 10.0,
        (40.0 - t * 15.0).round() as i32,
      )
    } else if s >= 60.0 {
      let t = (65.0 - s) / 5.0;
      (
        (235.0 + t * 10.0).round() as i32,
        ((0.65 - t * 0.05) * 100.0).round() / 100.0,
        ((3.0 - t * 0.5) * 10.0).round() / 10.0,
        (25.0 - t * 10.0).round() as i32,
      )
    } else {
      let t = (60.0 - s) / 5.0;
      (
        (245.0 + t * 5.0).round() as i32,
        ((0.60 - t * 0.04) * 100.0).round() / 100.0,
        2.5,
        (15.0 - t * 5.0).round() as i32,
      )
    }
  }

  /// Infers the active Shore A hardness (`60`, `65`, `70`, `75`, `80`, `85`, `92`) of a Foaming TPU spool
  /// from its title tag (e.g. `OC - TPU Foaming 65A • 0.4 Hardened`) or its `brass_base_temp`.
  pub fn effective_foaming_shore_a(&self) -> u32 {
    // Check explicit target Shore A token in title or material_type (e.g. "-> 75A" or "Foaming 65A")
    if let Some((_, after_arrow)) = self.material_type.split_once("->") {
      for token in after_arrow.split(|c: char| !c.is_alphanumeric()) {
        if let Some(num_str) = token.strip_suffix('A') {
          if let Ok(val) = num_str.parse::<u32>() {
            if (55..=92).contains(&val) {
              return val;
            }
          }
        }
      }
    }
    for token in self.name.split_whitespace() {
      if let Some(num_str) = token.strip_suffix('A') {
        if let Ok(val) = num_str.parse::<u32>() {
          // Skip "85A" / "95A" base polymer label in "TPU Air 85A" if there is a foaming temp
          if (val == 85 || val == 95) && self.is_siraya_foaming_air() && self.brass_base_temp >= 245
          {
            continue;
          }
          if (55..=92).contains(&val) {
            return val;
          }
        }
      }
    }
    if self.is_siraya_foaming_air() {
      if self.brass_base_temp <= 238 {
        85
      } else if self.brass_base_temp <= 254 {
        75
      } else if self.brass_base_temp <= 264 {
        70
      } else {
        65
      }
    } else if self.brass_base_temp <= 200 {
      92
    } else if self.brass_base_temp <= 214 {
      85
    } else if self.brass_base_temp <= 226 {
      75
    } else if self.brass_base_temp <= 240 {
      65
    } else {
      60
    }
  }

  /// Updates a Foaming TPU spool to target a specific Shore A hardness (`55..=92`), recalculating
  /// its Brass/Hardened-Steel/Diamondback temperatures, expansion-compensated `flow_ratio`,
  /// `max_volumetric_speed`, `fan_speed_percent`, profile title (`OC - TPU Foaming <Shore>A • ...`),
  /// and synthetic OrcaSlicer JSON.
  pub fn apply_foaming_shore_hardness(&mut self, shore_a: u32, active_nozzle_type: &str) {
    let clamped_shore = shore_a.clamp(55, 92);
    self.finish_type = "Foaming".to_string();
    let (brass_c, flow, mvs, fan_pct) = if self.is_siraya_foaming_air() {
      Self::compute_siraya_foaming_tpu_curve(clamped_shore)
    } else {
      Self::compute_foaming_tpu_curve(clamped_shore)
    };
    self.brass_base_temp = brass_c;
    self.hs_base_temp = (brass_c + 5).clamp(185, 350);
    self.flow_ratio = flow;
    self.max_volumetric_speed = mvs;
    self.fan_speed_percent = fan_pct;
    self.required_nozzle_hrc = 3;
    self.recompute_for_nozzle(active_nozzle_type);

    let right_suffix = self
      .name
      .split_once(" • ")
      .map(|(_, r)| r.to_string())
      .unwrap_or_else(|| {
        let def = Self::default_oc_profile_name_with_brand(
          &self.brand,
          &self.material_type,
          "Foaming",
          active_nozzle_type,
        );
        def
          .split_once(" • ")
          .map(|(_, r)| r.to_string())
          .unwrap_or_else(|| "0.4 Hardened".to_string())
      });
    let norm_b = Self::normalized_brand(&self.brand);
    let prefix_b = if norm_b == "Generic"
      || self
        .material_type
        .to_lowercase()
        .starts_with(&norm_b.to_lowercase())
    {
      String::new()
    } else {
      format!("{} ", norm_b)
    };
    let clean_mat = self
      .material_type
      .split_once(" (")
      .map(|(head, _)| head)
      .unwrap_or(&self.material_type);
    if clean_mat.to_lowercase().contains("air") {
      self.name = format!(
        "OC - {}{} {}A • {}",
        prefix_b, clean_mat, clamped_shore, right_suffix
      );
    } else {
      self.name = format!(
        "OC - {}{} Foaming {}A • {}",
        prefix_b, clean_mat, clamped_shore, right_suffix
      );
    }
    self.rebuild_synthetic_disk_json(active_nozzle_type);
  }

  pub fn rebuild_synthetic_disk_json(&mut self, active_nozzle_type: &str) {
    let nt_low = active_nozzle_type.to_lowercase();
    let short_noz = if nt_low.contains("diamondback") {
      "Diamond"
    } else if nt_low.contains("ruby") {
      "Ruby"
    } else if nt_low.contains("cht") {
      "CHT"
    } else if nt_low.contains("cm2") {
      "CM2"
    } else if nt_low.contains("hardened") {
      "Hardened"
    } else {
      "Brass"
    };
    let _orca_noz = if nt_low.contains("hardened")
      || nt_low.contains("diamondback")
      || nt_low.contains("ruby")
      || nt_low.contains("cm2")
    {
      "hardened_steel"
    } else {
      "brass"
    };
    let is_pctg = self.material_type.contains("PCTG");
    let is_pctg_or_petg = is_pctg || self.material_type.contains("PETG");
    let is_translucent_petg =
      is_pctg_or_petg && self.finish_type.eq_ignore_ascii_case("Translucent");
    let is_foaming_tpu = self.is_foaming_tpu();
    let init_boost = if is_pctg_or_petg || is_foaming_tpu {
      0
    } else {
      5
    };

    self.deltas_from_base.clear();
    if self.print_temp != self.brass_base_temp {
      self.deltas_from_base.push(FilamentDeltaEntry {
        param_key: "nozzle_temperature".to_string(),
        base_value: format!("{}°C", self.brass_base_temp),
        disk_value: format!("{}°C", self.print_temp),
        reconstructed_by_lua: true,
        rule_explanation: format!(
          "{} {:+}°C thermal rule",
          short_noz,
          self.print_temp - self.brass_base_temp
        ),
      });
    }

    let norm_brand = Self::normalized_brand(&self.brand);
    let catalog = crate::catalog::load_merged_catalog();
    let vkey = format!("{}::{}", norm_brand, self.material_type);
    let vendor_base = catalog.vendor_filament_bases.get(&vkey);
    let ret_len_str = if self.is_siraya_foaming_air() {
      "0".to_string()
    } else if is_foaming_tpu {
      "0.35".to_string()
    } else if let Some(vb) = vendor_base {
      vb.get("filament_retraction_length")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
          if is_pctg {
            "1.0"
          } else if is_pctg_or_petg {
            "0.8"
          } else {
            "0.6"
          }
          .to_string()
        })
    } else if is_pctg {
      "1.0".to_string()
    } else if is_pctg_or_petg {
      "0.8".to_string()
    } else {
      "0.6".to_string()
    };
    let fan_max_str = if is_pctg_or_petg && self.fan_speed_percent >= 30 {
      "90".to_string()
    } else {
      self.fan_speed_percent.to_string()
    };

    let mut json_map = serde_json::Map::new();
    if let Some(vb) = vendor_base {
      for (k, v) in vb {
        if k != "compatible_printers" {
          json_map.insert(k.clone(), v.clone());
        }
      }
    }
    let (range_low, range_high) = Self::compute_nozzle_temp_range(
      &self.material_type,
      self.print_temp,
      (self.print_temp + init_boost).min(420),
    );
    let overrides = serde_json::json!({
      "from": "User",
      "inherits": self.inherits,
      "name": self.name,
      "filament_vendor": [norm_brand],
      "filament_settings_id": [self.name],
      "filament_type": [self.material_type],
      "required_nozzle_HRC": [self.required_nozzle_hrc.to_string()],
      "filament_colour": [self.color_hex],
      "default_filament_colour": [self.color_hex],
      "nozzle_temperature": [self.print_temp.to_string()],
      "nozzle_temperature_BRASS": [self.brass_base_temp.to_string()],
      "nozzle_temperature_HS": [self.hs_base_temp.to_string()],
      "nozzle_temperature_initial_layer": [(self.print_temp + init_boost).min(420).to_string()],
      "nozzle_temperature_initial_layer_BRASS": [(self.brass_base_temp + init_boost).min(420).to_string()],
      "nozzle_temperature_initial_layer_HS": [(self.hs_base_temp + init_boost).min(420).to_string()],
      "nozzle_temperature_range_low": [range_low.to_string()],
      "nozzle_temperature_range_high": [range_high.to_string()],
      "hot_plate_temp": [self.bed_temp.to_string()],
      "hot_plate_temp_initial_layer": [self.bed_temp.to_string()],
      "textured_plate_temp": [self.bed_temp.to_string()],
      "textured_plate_temp_initial_layer": [self.bed_temp.to_string()],
      "filament_flow_ratio": [format!("{:.2}", self.flow_ratio)],
      "pressure_advance": [format!("{:.3}", self.pressure_advance)],
      "filament_max_volumetric_speed": [format!("{:.1}", self.max_volumetric_speed)],
      "fan_min_speed": [self.fan_speed_percent.to_string()],
      "fan_min_speed_BRASS": [self.fan_speed_percent.to_string()],
      "fan_min_speed_HS": [self.fan_speed_percent.to_string()],
      "fan_max_speed": [fan_max_str],
      "fan_max_speed_BRASS": [fan_max_str],
      "fan_max_speed_HS": [fan_max_str],
      "filament_retraction_length": [ret_len_str],
      "filament_retraction_speed": [if is_foaming_tpu { "25" } else if is_pctg { "45" } else if is_pctg_or_petg { "40" } else { "40" }],
      "filament_z_hop": [if is_foaming_tpu { "0" } else if is_translucent_petg { "0.1" } else if is_pctg_or_petg { "0.15" } else { "0.2" }],
      "filament_z_hop_types": [if is_pctg_or_petg { "Spiral Lift" } else { "Normal Lift" }],
      "filament_retract_before_wipe": ["0%"],
      "filament_deretraction_speed": ["0"],
      "filament_wipe": ["1"],
      "filament_wipe_distance": ["2"],
      "version": WEBCONFIG_ORCA_PRESET_VERSION
    });
    if let serde_json::Value::Object(ov_obj) = overrides {
      for (k, v) in ov_obj {
        json_map.insert(k, v);
      }
    }

    self.raw_disk_json =
      Some(serde_json::to_string_pretty(&serde_json::Value::Object(json_map)).unwrap_or_default());
  }

  /// Creates a clean `FilamentSpool` from a `CatalogFilament` entry (using the active rig's nozzle type),
  /// matching the Start-From-Scratch Setup Wizard's spool construction.
  pub fn from_catalog_entry(
    idx: usize,
    mat: &crate::catalog::CatalogFilament,
    active_nozzle_type: &str,
  ) -> Self {
    let base_mat_code = mat.base_material_code().to_string();
    let finish_type = mat.default_finish.clone();
    let base_nozzle = mat.base_nozzle_c as i32;
    let base_bed = mat.base_bed_c as i32;
    let hs_nozzle = (base_nozzle + 5).clamp(80, 420);
    let nt_low = active_nozzle_type.to_lowercase();
    let effective_nozzle =
      if nt_low.contains("hardened") || nt_low.contains("stainless") || nt_low.contains("cm2") {
        hs_nozzle
      } else if nt_low.contains("diamondback") {
        (base_nozzle - 10).max(80)
      } else {
        base_nozzle
      };
    let is_pctg = base_mat_code.contains("PCTG");
    let is_pctg_or_petg = is_pctg || base_mat_code.contains("PETG");
    let default_pa = if is_pctg {
      0.048f32
    } else if base_mat_code.contains("PETG") {
      0.045f32
    } else {
      0.035f32
    };
    let default_flow =
      if mat.default_flow_ratio > 0.45 && (mat.default_flow_ratio - 0.98).abs() > 0.005 {
        mat.default_flow_ratio
      } else if is_pctg_or_petg {
        0.95f32
      } else {
        0.98f32
      };
    let base_mat_tag = match base_mat_code.as_str() {
      "PCTG" => "PETG",
      "PCTG-CF" | "PCTG-GF" | "PETG-GF" => "PETG-CF",
      "PLA-GF" | "PLA Glow" => "PLA",
      other => other,
    };
    let inherits = format!("Anycubic {} @Anycubic Kobra S1 0.4 nozzle", base_mat_tag);
    let short_noz = if nt_low.contains("diamondback") {
      "Diamond"
    } else if nt_low.contains("ruby") {
      "Ruby"
    } else if nt_low.contains("cht") {
      "CHT"
    } else if nt_low.contains("cm2") {
      "CM2"
    } else if nt_low.contains("hardened") {
      "Hardened"
    } else {
      "Brass"
    };
    let norm_brand = Self::normalized_brand(&mat.brand);
    let profile_name = Self::default_oc_profile_name_with_brand(
      &norm_brand,
      &base_mat_code,
      &finish_type,
      active_nozzle_type,
    );
    let is_abrasive = mat.requires_hardened
      || finish_type.contains("Carbon")
      || finish_type.contains("Glass")
      || base_mat_code.contains("-CF")
      || base_mat_code.contains("-GF");
    let req_hrc: u32 = if is_abrasive { 55 } else { 3 };
    let _orca_noz = if nt_low.contains("hardened")
      || nt_low.contains("diamondback")
      || nt_low.contains("ruby")
      || nt_low.contains("cm2")
    {
      "hardened_steel"
    } else {
      "brass"
    };
    let fan_pct = mat.fan_pct as i32;
    let fan_max_pct = if is_pctg_or_petg && fan_pct >= 30 {
      90
    } else {
      fan_pct
    };
    let init_boost = if is_pctg_or_petg { 0 } else { 5 };
    let ret_len_str = if is_pctg {
      "1.0"
    } else if is_pctg_or_petg {
      "0.8"
    } else {
      "0.6"
    };
    let ret_spd_str = if is_pctg { "45" } else { "40" };
    let max_vol = mat.max_vol_mm3s;
    let color_hex = "#8E9299".to_string();

    let mut deltas_from_base = Vec::new();
    if effective_nozzle != base_nozzle {
      deltas_from_base.push(FilamentDeltaEntry {
        param_key: "nozzle_temperature".to_string(),
        base_value: format!("{}°C", base_nozzle),
        disk_value: format!("{}°C", effective_nozzle),
        reconstructed_by_lua: true,
        rule_explanation: format!(
          "{} {:+}°C thermal rule",
          short_noz,
          effective_nozzle - base_nozzle
        ),
      });
    }

    let (range_low, range_high) = Self::compute_nozzle_temp_range(
      &base_mat_code,
      effective_nozzle,
      (effective_nozzle + init_boost).min(350),
    );
    let raw_disk_json = serde_json::to_string_pretty(&serde_json::json!({
      "from": "User",
      "inherits": inherits,
      "name": profile_name,
      "filament_settings_id": [profile_name],
      "filament_type": [base_mat_code],
      "required_nozzle_HRC": [req_hrc.to_string()],
      "filament_colour": [color_hex],
      "default_filament_colour": [color_hex],
      "nozzle_temperature": [effective_nozzle.to_string()],
      "nozzle_temperature_BRASS": [base_nozzle.to_string()],
      "nozzle_temperature_HS": [hs_nozzle.to_string()],
      "nozzle_temperature_initial_layer": [(effective_nozzle + init_boost).min(350).to_string()],
      "nozzle_temperature_initial_layer_BRASS": [(base_nozzle + init_boost).min(350).to_string()],
      "nozzle_temperature_initial_layer_HS": [(hs_nozzle + init_boost).min(350).to_string()],
      "nozzle_temperature_range_low": [range_low.to_string()],
      "nozzle_temperature_range_high": [range_high.to_string()],
      "hot_plate_temp": [base_bed.to_string()],
      "hot_plate_temp_initial_layer": [base_bed.to_string()],
      "textured_plate_temp": [base_bed.to_string()],
      "textured_plate_temp_initial_layer": [base_bed.to_string()],
      "filament_flow_ratio": [format!("{:.2}", default_flow)],
      "pressure_advance": [format!("{:.3}", default_pa)],
      "filament_max_volumetric_speed": [format!("{:.1}", max_vol)],
      "fan_min_speed": [fan_pct.to_string()],
      "fan_min_speed_BRASS": [fan_pct.to_string()],
      "fan_min_speed_HS": [fan_pct.to_string()],
      "fan_max_speed": [fan_max_pct.to_string()],
      "fan_max_speed_BRASS": [fan_max_pct.to_string()],
      "fan_max_speed_HS": [fan_max_pct.to_string()],
      "filament_retraction_length": [ret_len_str],
      "filament_retraction_speed": [ret_spd_str],
      "filament_z_hop": [if is_pctg_or_petg { "0.15" } else { "0.2" }],
      "filament_z_hop_types": [if is_pctg_or_petg { "Spiral Lift" } else { "Normal Lift" }],
      "filament_retract_before_wipe": ["0%"],
      "filament_deretraction_speed": ["0"],
      "filament_wipe": ["1"],
      "filament_wipe_distance": ["2"],
      "version": WEBCONFIG_ORCA_PRESET_VERSION
    }))
    .unwrap_or_default();

    let mut spool = FilamentSpool {
      id: format!(
        "spool_catalog_{}_{}_{}",
        idx,
        norm_brand.to_lowercase().replace(' ', "_"),
        mat.code.to_lowercase().replace(' ', "_")
      ),
      name: profile_name,
      brand: norm_brand,
      inherits,
      material_type: base_mat_code.clone(),
      nozzle_type: active_nozzle_type.to_string(),
      tuned_for_nozzle_type: active_nozzle_type.to_string(),
      required_nozzle_hrc: req_hrc,
      color_hex: color_hex.clone(),
      color_name: "Cool Gray".to_string(),
      brass_base_temp: base_nozzle,
      hs_base_temp: hs_nozzle,
      base_print_temp: base_nozzle,
      print_temp: effective_nozzle,
      base_bed_temp: base_bed,
      bed_temp: base_bed,
      flow_ratio: default_flow,
      pressure_advance: default_pa,
      max_volumetric_speed: max_vol,
      fan_speed_percent: fan_pct,
      remaining_grams: 1000,
      is_loaded: true,
      finish_type: finish_type.clone(),
      opacity: if finish_type == "Translucent" {
        0.35
      } else {
        1.0
      },
      pigment_stickiness: if finish_type == "Carbon Fiber" || base_mat_code.contains("-CF") {
        1.15
      } else if finish_type == "Silk" {
        1.20
      } else if finish_type == "Glass Fiber" {
        1.10
      } else {
        1.0
      },
      drying_temp_c: mat.dry_temp_c as i32,
      drying_time_hr: mat.dry_hr as i32,
      is_rfid_synced: false,
      deltas_from_base,
      raw_disk_json: Some(raw_disk_json),
    };
    spool.apply_finish_change(&finish_type, active_nozzle_type);
    spool
  }

  /// Generate a full descriptive note (`filament_notes`) including the WebConfig app version and tuned parameters
  pub fn generate_descriptive_notes(&self) -> String {
    format!(
      "[OrcaSlicer WebConfig v{} — Filament] {} {} ({}). Nozzle: {}°C (Brass Base: {}°C, HS Base: {}°C), Bed: {}°C, Flow: {:.2}, PA: {:.3}, Max Vol: {:.1} mm³/s, Fan: {}%, Required HRC: {}, Dry: {}°C / {}h.",
      WEBCONFIG_APP_VERSION,
      Self::normalized_brand(&self.brand),
      self.material_type,
      self.finish_type,
      self.print_temp,
      self.brass_base_temp,
      self.hs_base_temp,
      self.bed_temp,
      self.flow_ratio,
      self.pressure_advance,
      self.max_volumetric_speed,
      self.fan_speed_percent,
      self.required_nozzle_hrc,
      self.drying_temp_c,
      self.drying_time_hr
    )
  }
}

fn default_config_version() -> u32 {
  1
}

fn default_orca_slicer_version() -> String {
  "2.2".to_string()
}

fn default_webconfig_app_version() -> String {
  WEBCONFIG_APP_VERSION.to_string()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebConfigStateSnapshot {
  pub schema_version: String,
  #[serde(default = "default_config_version")]
  pub config_version: u32,
  #[serde(default = "default_orca_slicer_version")]
  pub orca_slicer_version: String,
  #[serde(default = "default_webconfig_app_version")]
  pub webconfig_app_version: String,
  pub machines: Vec<MachineRig>,
  pub active_machine_id: String,
  pub filaments: Vec<FilamentSpool>,
  pub active_filament_type: String,
  pub processes: Vec<ProcessProfile>,
  pub active_process_id: String,
  pub base_profile: crate::orca_profile::OrcaProfile,
  pub rules: Vec<crate::transformations::TransformationRule>,
  pub conflicts: Vec<crate::transformations::ExternalConflict>,
  pub pending_updates: Vec<crate::transformations::PendingDiskUpdate>,
  pub bound_filename: String,
  pub bound_path_hint: String,
  pub detected_hardware_summary: String,
  pub flush_multiplier: f64,
  pub disk_flush_multiplier: f64,
  pub disk_flush_matrix: Vec<String>,
  pub disk_flush_slot_count: usize,
  pub conf_relative_path: String,
}

impl WebConfigStateSnapshot {
  /// Sanitizes a loaded or imported snapshot so that:
  /// 1. Any no-op filament delta where `base_value` and `disk_value` have identical numeric values
  ///    (e.g. `260°C (Standalone Base) -> 260°C` on PCTG) is stripped out.
  /// 2. Stock baseline keys for Scarf Seams, Arachne Walls, and Wipe/Deretraction are populated on `base_profile`.
  /// 3. Any newly introduced built-in transformation rules (such as Scarf Seams & Arachne or PCTG Anti-Stringing)
  ///    are merged into `rules` if not already present.
  pub fn sanitize(&mut self) {
    if self.config_version == 0 {
      self.config_version = 1;
    }
    if self.orca_slicer_version.trim().is_empty() {
      self.orca_slicer_version = default_orca_slicer_version();
    }
    if self.webconfig_app_version.trim().is_empty() || self.webconfig_app_version == "2.2.0" {
      self.webconfig_app_version = default_webconfig_app_version();
    }
    if self.schema_version.trim().is_empty()
      || self.schema_version == "1.0"
      || self.schema_version == "2.2.0"
    {
      self.schema_version = WEBCONFIG_APP_VERSION.to_string();
    }
    if self.bound_filename.contains("From Scratch")
      || self.bound_path_hint.contains("Start-From-Scratch")
    {
      self.disk_flush_matrix.clear();
      self.disk_flush_slot_count = 0;
    }

    let catalog = crate::catalog::load_merged_catalog();
    let has_oc_spools = self
      .filaments
      .iter()
      .any(|f| f.name.starts_with("OC -") || f.name.starts_with("OC:"));
    let active_noz_for_sanitize = self
      .machines
      .iter()
      .find(|m| m.id == self.active_machine_id)
      .or_else(|| self.machines.first())
      .map(|m| m.nozzle_type.clone())
      .unwrap_or_else(|| "hardened_steel".to_string());
    for sp in &mut self.filaments {
      if has_oc_spools
        && !sp.name.starts_with("OC -")
        && !sp.name.starts_with("OC:")
        && sp.name.contains("@Anycubic")
        && sp.id.starts_with("disk_sys_spool_")
      {
        let col_hex = sp.color_hex.clone();
        let col_name = sp.color_name.clone();
        let is_loaded = sp.is_loaded;
        let mat_code = sp.material_type.clone();
        if let Some(cat_entry) = catalog
          .filaments
          .iter()
          .find(|f| {
            f.brand.eq_ignore_ascii_case("Anycubic") && f.code.eq_ignore_ascii_case(&mat_code)
          })
          .or_else(|| {
            catalog
              .filaments
              .iter()
              .find(|f| f.code.eq_ignore_ascii_case(&mat_code))
          })
        {
          let mut upgraded =
            FilamentSpool::from_catalog_entry(1, cat_entry, &active_noz_for_sanitize);
          upgraded.id = sp.id.clone();
          upgraded.color_hex = col_hex;
          upgraded.color_name = col_name;
          upgraded.is_loaded = is_loaded;
          upgraded.rebuild_synthetic_disk_json(&active_noz_for_sanitize);
          *sp = upgraded;
        }
      }
      if sp.name.contains(':') {
        sp.name = sp.name.replace(": ", " - ").replace(':', "-");
      }
      if (sp.name.starts_with("OC:") || sp.name.starts_with("OC -")) && sp.name.contains(" - 0.") {
        sp.name = sp.name.replace(" - 0.", " • 0.");
      }
      if sp.material_type.eq_ignore_ascii_case("PETG Translucent") {
        sp.material_type = "PETG".to_string();
        sp.finish_type = "Translucent".to_string();
        if sp.opacity > 0.6 {
          sp.opacity = 0.35;
        }
      } else if sp.material_type.eq_ignore_ascii_case("PLA Matte") {
        sp.material_type = "PLA".to_string();
        sp.finish_type = "Matte".to_string();
      } else if sp.material_type.eq_ignore_ascii_case("PLA Silk") {
        sp.material_type = "PLA".to_string();
        sp.finish_type = "Silk".to_string();
      } else if sp.material_type.eq_ignore_ascii_case("PC") {
        let lower_name = sp.name.to_lowercase();
        if sp.finish_type == "Translucent"
          && !lower_name.contains("translucent")
          && !lower_name.contains("clear")
          && !lower_name.contains("transparent")
        {
          sp.finish_type = "Basic".to_string();
          sp.opacity = 1.0;
        }
      }
      if sp.finish_type.eq_ignore_ascii_case("Translucent")
        && !sp.name.to_lowercase().contains("translucent")
      {
        let prefix = format!("OC - {} •", sp.material_type);
        if sp.name.starts_with(&prefix) {
          sp.name = sp.name.replacen(
            &prefix,
            &format!("OC - {} Translucent •", sp.material_type),
            1,
          );
        }
      }

      sp.brand = FilamentSpool::normalized_brand(&sp.brand);
      if sp.brand == "Generic" {
        let nm_up = sp.name.to_uppercase();
        if nm_up.contains("SIRAYA") {
          sp.brand = "Siraya Tech".to_string();
        } else if nm_up.contains("SUNLU") {
          sp.brand = "SUNLU".to_string();
        } else if nm_up.contains("POLYMAKER")
          || nm_up.contains("POLYLITE")
          || nm_up.contains("POLYMAX")
          || nm_up.contains("POLYTERRA")
          || nm_up.contains("FIBERON")
          || nm_up.contains("PANCHROMA")
        {
          sp.brand = "Polymaker".to_string();
        } else if nm_up.contains("ANYCUBIC") {
          sp.brand = "Anycubic".to_string();
        } else if nm_up.contains("BAMBU") {
          sp.brand = "Bambu Lab".to_string();
        } else if nm_up.contains("ESUN") {
          sp.brand = "eSUN".to_string();
        } else if nm_up.contains("OVERTURE") {
          sp.brand = "Overture".to_string();
        }
      }

      // Heal any scratch/catalog/OC spool in localStorage whose base temperature, print_temp, or translucent PETG tuning was corrupted or came from an older snapshot
      let is_oc_managed = sp.name.starts_with("OC -") || sp.name.starts_with("OC:");
      let is_foaming_or_esd = sp.finish_type.eq_ignore_ascii_case("Foaming")
        || sp.finish_type.eq_ignore_ascii_case("Conductive")
        || sp.material_type.to_uppercase().contains("ESD");
      if (!sp.id.starts_with("disk_") || is_oc_managed) && !is_foaming_or_esd {
        if let Some(cat_entry) = catalog
          .filaments
          .iter()
          .find(|f| {
            f.brand.eq_ignore_ascii_case(&sp.brand)
              && f.code.eq_ignore_ascii_case(&sp.material_type)
          })
          .or_else(|| {
            catalog
              .filaments
              .iter()
              .find(|f| f.code.eq_ignore_ascii_case(&sp.material_type))
          })
          .or_else(|| {
            let canon = FilamentSpool::canonical_polymer_key(&sp.material_type);
            catalog
              .filaments
              .iter()
              .find(|f| f.brand.eq_ignore_ascii_case(&sp.brand) && FilamentSpool::canonical_polymer_key(&f.code) == canon)
              .or_else(|| {
                catalog
                  .filaments
                  .iter()
                  .find(|f| FilamentSpool::canonical_polymer_key(&f.code) == canon)
              })
          })
        {
          let nt_low = sp.nozzle_type.to_lowercase();
          let exp_brass = cat_entry.base_nozzle_c as i32;
          let exp_hs = if sp.material_type.eq_ignore_ascii_case("PLA+") && sp.brand == "Generic" {
            220
          } else {
            (exp_brass + 5).clamp(80, 420)
          };
          let exp_print_temp = if nt_low.contains("hardened")
            || nt_low.contains("stainless")
            || nt_low.contains("cm2")
          {
            exp_hs
          } else if nt_low.contains("diamondback") {
            (exp_brass - 10).max(80)
          } else {
            exp_brass
          };
          let needs_translucent_petg_heal = sp.material_type == "PETG"
            && sp.finish_type == "Translucent"
            && (sp.bed_temp > 65 || sp.max_volumetric_speed > 10.0 || sp.fan_speed_percent > 35);
          let canon_mat = FilamentSpool::canonical_polymer_key(&sp.material_type);
          let needs_warp_fan_heal = matches!(
            canon_mat,
            "ABS" | "ABS-CF" | "ASA" | "ASA-CF" | "PC" | "PC-CF" | "PA" | "PA6-CF" | "PA12-CF"
              | "PAHT-CF" | "PPA-CF"
          ) && sp.fan_speed_percent > 35;
          let needs_temp_heal = !sp.is_foaming_tpu()
            && (sp.brass_base_temp != exp_brass || (sp.print_temp - exp_print_temp).abs() > 8);
          if needs_temp_heal || needs_translucent_petg_heal || needs_warp_fan_heal {
            let noz = sp.nozzle_type.clone();
            let fin = sp.finish_type.clone();
            let saved_name = sp.name.clone();
            let col_hex = sp.color_hex.clone();
            let col_name = sp.color_name.clone();
            let mat = sp.material_type.clone();
            sp.apply_material_change(&mat, &noz);
            sp.name = saved_name;
            sp.color_hex = col_hex;
            sp.color_name = col_name;
            sp.apply_finish_change(&fin, &noz);
          }
          let mat_up = sp.material_type.to_uppercase();
          if !mat_up.contains("TPU")
            && !mat_up.contains("TPE")
            && !mat_up.contains("PEBA")
            && !mat_up.contains("PVA")
            && !mat_up.contains("BVOH")
            && sp.max_volumetric_speed <= 2.5
          {
            sp.max_volumetric_speed = cat_entry.max_vol_mm3s.max(12.0);
          }
        }
      }

      if is_oc_managed {
        if !sp.is_foaming_tpu() {
          let noz = sp.nozzle_type.clone();
          sp.recompute_for_nozzle(&noz);
          sp.rebuild_synthetic_disk_json(&noz);
        } else if let Some(ref raw) = sp.raw_disk_json {
          if let Ok(serde_json::Value::Object(mut rmap)) = serde_json::from_str::<serde_json::Value>(raw) {
            if rmap.remove("compatible_printers").is_some() {
              sp.raw_disk_json = serde_json::to_string_pretty(&serde_json::Value::Object(rmap)).ok();
            }
          }
        }
        sp.inherits = crate::threemf::resolve_kobra_s1_filament_inherits(sp);
        sp.deltas_from_base.retain(|d| d.param_key != "inherits");
        for d in &mut sp.deltas_from_base {
          d.reconstructed_by_lua = true;
        }
      }

      sp.deltas_from_base.retain(|d| {
        if d.param_key == "inherits" {
          let fam_a = FilamentSpool::polymer_family_key(&d.base_value);
          let fam_b = FilamentSpool::polymer_family_key(&d.disk_value);
          if fam_a == fam_b {
            return false;
          }
        }
        let base_head = d
          .base_value
          .split('(')
          .next()
          .unwrap_or(&d.base_value)
          .trim();
        let disk_head = d
          .disk_value
          .split('(')
          .next()
          .unwrap_or(&d.disk_value)
          .trim();
        !crate::transformations::values_are_equivalent(base_head, disk_head)
      });
    }

    // Never retain external drift conflicts against our own generated `OC -` profiles or compatible polymer families (e.g. PHA -> Anycubic PLA)
    self.conflicts.retain(|c| {
      if c.source_relative_path.contains("/OC -")
        || c.source_relative_path.contains("/OC:")
        || c.param_key.contains(":OC -")
        || c.param_key.contains(":OC:")
      {
        return false;
      }
      if c.json_field_key == "inherits" {
        let fam_expected = FilamentSpool::polymer_family_key(&c.expected_derived_value);
        let fam_actual = FilamentSpool::polymer_family_key(&c.actual_external_value);
        if fam_expected == fam_actual {
          return false;
        }
      }
      true
    });

    if self.machines.is_empty() {
      self
        .machines
        .push(MachineRig::kobra_s1_hardened_steel_png());
    }
    for mach in &mut self.machines {
      mach.nozzle_diameter = mach.formatted_nozzle_diameter();
      let (_, hrc) = mach.orca_machine_nozzle_type_and_hrc();
      mach.nozzle_hrc = hrc;
      if mach.name.contains(':') {
        mach.name = mach.name.replace(": ", " - ").replace(':', "-");
      }
      if mach.notes.trim().is_empty()
        || (mach.name.starts_with("OC -") && !mach.notes.contains(WEBCONFIG_APP_VERSION))
      {
        mach.notes = mach.generate_descriptive_notes();
      }
    }
    let short_mach = self
      .machines
      .iter()
      .find(|m| m.id == self.active_machine_id)
      .or_else(|| self.machines.first())
      .map(|m| m.short_model_tag())
      .unwrap_or_else(|| "Kobra S1".to_string());
    let canonical_procs = ProcessProfile::get_default_processes();
    for proc in &mut self.processes {
      if proc.name.contains(':') {
        proc.name = proc.name.replace(": ", " - ").replace(':', "-");
      }
      if proc.name.starts_with("OC:")
        || proc.name.starts_with("OC -")
        || proc.id.starts_with("proc_")
      {
        if proc.print_sequence.eq_ignore_ascii_case("by layer") {
          proc.print_sequence = "by object".to_string();
        }
        if let Some(canon) = canonical_procs.iter().find(|dp| {
          dp.id == proc.id
            || dp.name.split(" @").next() == proc.name.split(" @").next()
        }) {
          let was_corrupted = matches!(
            proc.outer_wall_speed.round() as i32,
            51 | 151 | 162 | 238
          ) || matches!(
            proc.inner_wall_speed.round() as i32,
            128 | 151 | 162 | 238
          ) || matches!(
            proc.sparse_infill_speed.round() as i32,
            151 | 162 | 238
          ) || (proc.initial_layer_speed - 18.0).abs() < 1.0
            || (proc.layer_height - canon.layer_height).abs() > 0.01;
          if was_corrupted {
            proc.layer_height = canon.layer_height;
            proc.line_width = canon.line_width;
            proc.outer_wall_speed = canon.outer_wall_speed;
            proc.inner_wall_speed = canon.inner_wall_speed;
            proc.sparse_infill_speed = canon.sparse_infill_speed;
            proc.initial_layer_speed = canon.initial_layer_speed;
            proc.default_acceleration = canon.default_acceleration;
            proc.inherits = canon.inherits.clone();
          }
        }
        if proc.inherits == "fdm_process_common" || proc.inherits.trim().is_empty() {
          proc.inherits = if (proc.layer_height - 0.12).abs() < 0.015 {
            "0.12mm Standard @Anycubic Kobra S1 0.4 nozzle".to_string()
          } else if (proc.layer_height - 0.28).abs() < 0.015 {
            "0.28mm Standard @Anycubic Kobra S1 0.4 nozzle".to_string()
          } else {
            "0.20mm Standard @Anycubic Kobra S1 0.4 nozzle".to_string()
          };
        } else if proc.inherits == "0.12mm High Quality @Anycubic Kobra S1 0.4 nozzle" {
          proc.inherits = "0.12mm Standard @Anycubic Kobra S1 0.4 nozzle".to_string();
        }
        proc.name = proc.format_concise_title(&short_mach);
        proc.notes = proc.generate_descriptive_notes();
      }
    }
    for mut def_proc in canonical_procs {
      def_proc.name = def_proc.format_concise_title(&short_mach);
      def_proc.notes = def_proc.generate_descriptive_notes();
      if !self.processes.iter().any(|p| p.name == def_proc.name) {
        self.processes.push(def_proc);
      }
    }
    // Deduplicate process profiles by name so we never have duplicate profiles
    let mut seen_proc_names = std::collections::HashSet::new();
    self
      .processes
      .retain(|p| seen_proc_names.insert(p.name.clone()));

    self.pending_updates.retain_mut(|upd| {
      if upd.json_field_key == "nozzle_diameter" && upd.new_clean_value == "0" {
        upd.new_clean_value = "0.4".to_string();
      }
      if upd.source_relative_path.starts_with("user/")
        && upd.source_relative_path.ends_with(".json")
      {
        upd.source_relative_path = sanitize_user_relative_path(&upd.source_relative_path);
        if upd.source_relative_path.contains("/machine/OC -")
          || upd.source_relative_path.contains("/process/OC -")
          || upd.source_relative_path.contains("/filament/OC -")
        {
          return false;
        }
      }
      true
    });

    let stock = crate::orca_profile::OrcaProfile::get_stock_kobra_s1();
    for (k, v) in stock.values {
      self.base_profile.values.entry(k).or_insert(v);
    }

    for key in ["z_hop_types", "filament_z_hop_types"] {
      if let Some(val) = self.base_profile.values.get_mut(key) {
        let normalize = |s: &str| -> String {
          match s.trim() {
            "Normal" | "normal" => "Normal Lift".to_string(),
            "Spiral" | "spiral" => "Spiral Lift".to_string(),
            "Slope" | "slope" | "z-hop-slope" => "Slope Lift".to_string(),
            "Auto" | "auto" => "Auto Lift".to_string(),
            other => other.to_string(),
          }
        };
        match val {
          serde_json::Value::String(s) => *s = normalize(s),
          serde_json::Value::Array(arr) => {
            for elem in arr.iter_mut() {
              if let serde_json::Value::String(s) = elem {
                *s = normalize(s);
              }
            }
          }
          _ => {}
        }
      }
    }

    for rule in &mut self.rules {
      rule.lua_code = rule
        .lua_code
        .replace("{ \"Slope\" }", "{ \"Slope Lift\" }")
        .replace("{ \"Spiral\" }", "{ \"Spiral Lift\" }")
        .replace("{ \"Normal\" }", "{ \"Normal Lift\" }")
        .replace("= \"Slope\"", "= \"Slope Lift\"")
        .replace("= \"Spiral\"", "= \"Spiral Lift\"")
        .replace("= \"Normal\"", "= \"Normal Lift\"");
    }

    for default_rule in crate::transformations::TransformationEngine::get_default_rules() {
      if !self.rules.iter().any(|r| r.id == default_rule.id) {
        self.rules.push(default_rule);
      }
    }
  }
}

impl ProcessProfile {
  pub fn quality_tier(&self) -> &'static str {
    let lower = format!("{} {}", self.id.to_lowercase(), self.name.to_lowercase());
    if lower.contains("quality") || lower.contains("hq") || lower.contains("fine") {
      "high_quality"
    } else if lower.contains("speed")
      || lower.contains("_hs")
      || lower.contains("draft")
      || lower.contains("fast")
      || lower.contains("sport")
    {
      "high_speed"
    } else {
      "standard"
    }
  }

  /// Concise, easy-to-read title prefixed with `OC -` (`Standard`, `Quality`, `Speed` without redundant `High` or Windows-forbidden `:`):
  /// e.g. `OC - 0.20mm Standard @Kobra S1`, `OC - 0.20mm Quality @Kobra S1`, `OC - 0.20mm Speed @Kobra S1`,
  /// `OC - 0.12mm Quality @Kobra S1`, `OC - 0.28mm Speed @Kobra S1`
  pub fn format_concise_title(&self, short_machine: &str) -> String {
    let tier_label = match self.quality_tier() {
      "high_quality" => "Quality",
      "high_speed" => "Speed",
      _ => "Standard",
    };
    format!(
      "OC - {:.2}mm {} @{}",
      self.layer_height, tier_label, short_machine
    )
  }

  /// Generate a full descriptive note (`notes`) summarizing the process tier, speeds, acceleration, and slicing strategy
  pub fn generate_descriptive_notes(&self) -> String {
    let tier_label = match self.quality_tier() {
      "high_quality" => "Quality (Fine Surface Detail & Overhang Precision)",
      "high_speed" => "Speed (Rapid Prototyping Capped by Volumetric Speed Ceiling)",
      _ => "Standard (Balanced Structural Strength & Throughput)",
    };
    format!(
      "[OrcaSlicer WebConfig v{} — Process: {}] Layer Height: {:.2}mm, Line Width: {:.2}mm. Speeds: {:.0} mm/s Outer Wall, {:.0} mm/s Inner Wall, {:.0} mm/s Sparse Infill, {:.0} mm/s Initial Layer. Default Accel: {:.0} mm/s². Wall Generator: '{}', Print Sequence: '{}'. Protected by Lua Volumetric Speed Ceiling & Non-Crossing Gyroid/Z-Hop rules.",
      WEBCONFIG_APP_VERSION,
      tier_label,
      self.layer_height,
      self.line_width,
      self.outer_wall_speed,
      self.inner_wall_speed,
      self.sparse_infill_speed,
      self.initial_layer_speed,
      self.default_acceleration,
      self.wall_generator,
      self.print_sequence
    )
  }

  pub fn get_default_processes() -> Vec<Self> {
    // 1. 0.20mm Standard (Balanced structural strength & everyday speed)
    let mut std_020 = ProcessProfile {
      id: "proc_020_std_png".to_string(),
      name: String::new(),
      inherits: "0.20mm Standard @Anycubic Kobra S1 0.4 nozzle".to_string(),
      layer_height: 0.20,
      line_width: 0.42,
      outer_wall_speed: 200.0,
      inner_wall_speed: 300.0,
      sparse_infill_speed: 270.0,
      initial_layer_speed: 50.0,
      default_acceleration: 10000.0,
      print_sequence: "by object".to_string(),
      wall_generator: "arachne".to_string(),
      notes: String::new(),
    };
    std_020.name = std_020.format_concise_title("Kobra S1");
    std_020.notes = std_020.generate_descriptive_notes();

    // 2. 0.20mm Quality (Best speed-vs-surface-finish tradeoff: 0.20mm layer, 60 mm/s outer wall, 150 mm/s inner wall, 250 mm/s infill)
    let mut hq_020 = ProcessProfile {
      id: "proc_020_hq_png".to_string(),
      name: String::new(),
      inherits: "0.20mm High Quality @Anycubic Kobra S1 0.4 nozzle".to_string(),
      layer_height: 0.20,
      line_width: 0.42,
      outer_wall_speed: 60.0,
      inner_wall_speed: 150.0,
      sparse_infill_speed: 250.0,
      initial_layer_speed: 50.0,
      default_acceleration: 5000.0,
      print_sequence: "by object".to_string(),
      wall_generator: "arachne".to_string(),
      notes: String::new(),
    };
    hq_020.name = hq_020.format_concise_title("Kobra S1");
    hq_020.notes = hq_020.generate_descriptive_notes();

    // 3. 0.20mm Speed (Rapid 0.20mm prototyping capped by Volumetric Speed Ceiling)
    let mut hs_020 = ProcessProfile {
      id: "proc_020_hs_png".to_string(),
      name: String::new(),
      inherits: "0.20mm Standard @Anycubic Kobra S1 0.4 nozzle".to_string(),
      layer_height: 0.20,
      line_width: 0.42,
      outer_wall_speed: 300.0,
      inner_wall_speed: 350.0,
      sparse_infill_speed: 350.0,
      initial_layer_speed: 60.0,
      default_acceleration: 15000.0,
      print_sequence: "by object".to_string(),
      wall_generator: "arachne".to_string(),
      notes: String::new(),
    };
    hs_020.name = hs_020.format_concise_title("Kobra S1");
    hs_020.notes = hs_020.generate_descriptive_notes();

    // 4. 0.12mm Quality (Ultra-fine vertical detail)
    let mut hq_012 = ProcessProfile {
      id: "proc_012_hq_png".to_string(),
      name: String::new(),
      inherits: "0.12mm Standard @Anycubic Kobra S1 0.4 nozzle".to_string(),
      layer_height: 0.12,
      line_width: 0.42,
      outer_wall_speed: 60.0,
      inner_wall_speed: 150.0,
      sparse_infill_speed: 200.0,
      initial_layer_speed: 50.0,
      default_acceleration: 5000.0,
      print_sequence: "by object".to_string(),
      wall_generator: "arachne".to_string(),
      notes: String::new(),
    };
    hq_012.name = hq_012.format_concise_title("Kobra S1");
    hq_012.notes = hq_012.generate_descriptive_notes();

    // 5. 0.28mm Speed (Thick-layer high-throughput functional draft)
    let mut hs_028 = ProcessProfile {
      id: "proc_028_hs_png".to_string(),
      name: String::new(),
      inherits: "0.28mm Standard @Anycubic Kobra S1 0.4 nozzle".to_string(),
      layer_height: 0.28,
      line_width: 0.44,
      outer_wall_speed: 250.0,
      inner_wall_speed: 320.0,
      sparse_infill_speed: 320.0,
      initial_layer_speed: 60.0,
      default_acceleration: 12000.0,
      print_sequence: "by object".to_string(),
      wall_generator: "arachne".to_string(),
      notes: String::new(),
    };
    hs_028.name = hs_028.format_concise_title("Kobra S1");
    hs_028.notes = hs_028.generate_descriptive_notes();

    vec![std_020, hq_020, hs_020, hq_012, hs_028]
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_version_single_source_of_truth() {
    assert!(!WEBCONFIG_APP_VERSION.is_empty());
    assert_ne!(WEBCONFIG_APP_VERSION, "0.0.0");
    assert_eq!(
      WEBCONFIG_ORCA_PRESET_VERSION,
      format!("{}.0", WEBCONFIG_APP_VERSION)
    );

    let expected_toml_line = format!("version = \"{}\"", WEBCONFIG_APP_VERSION);
    let root_cargo = include_str!("../Cargo.toml");
    assert!(
      root_cargo.lines().any(|l| l.trim() == expected_toml_line),
      "Cargo.toml version must match version.bzl ({})",
      WEBCONFIG_APP_VERSION
    );

    let server_cargo = include_str!("../server/Cargo.toml");
    assert!(
      server_cargo.lines().any(|l| l.trim() == expected_toml_line),
      "server/Cargo.toml version must match version.bzl ({})",
      WEBCONFIG_APP_VERSION
    );

    let tauri_cargo = include_str!("../src-tauri/Cargo.toml");
    assert!(
      tauri_cargo.lines().any(|l| l.trim() == expected_toml_line),
      "src-tauri/Cargo.toml version must match version.bzl ({})",
      WEBCONFIG_APP_VERSION
    );

    let tauri_conf: serde_json::Value =
      serde_json::from_str(include_str!("../src-tauri/tauri.conf.json")).unwrap();
    assert_eq!(
      tauri_conf["version"].as_str(),
      Some(WEBCONFIG_APP_VERSION),
      "src-tauri/tauri.conf.json version must match version.bzl ({})",
      WEBCONFIG_APP_VERSION
    );
  }
}
