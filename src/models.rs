use serde::{Deserialize, Serialize};

/// Canonical version of the OrcaSlicer WebConfig web/desktop application,
/// sourced at compile time from `CARGO_PKG_VERSION` (`version.bzl` in Bazel / `Cargo.toml` in Cargo).
pub const WEBCONFIG_APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Current wall-clock time in AnycubicSlicerNext's `printer_timestamp` format
/// (`"YYYY-MM-DD HH:MM:SS"`, lexicographically sortable). Used to stamp machine presets so a
/// later directory import can tell which `OC -` incarnation (e.g. `• 0.4 Hardened` vs
/// `• 0.4 Diamondback`) was saved most recently.
pub fn now_printer_timestamp() -> String {
  let secs = current_unix_secs();
  let days = secs.div_euclid(86_400);
  let secs_of_day = secs.rem_euclid(86_400);
  let (y, m, d) = civil_from_days(days);
  format!(
    "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
    y,
    m,
    d,
    secs_of_day / 3600,
    (secs_of_day % 3600) / 60,
    secs_of_day % 60
  )
}

fn current_unix_secs() -> i64 {
  #[cfg(target_arch = "wasm32")]
  {
    (js_sys::Date::now() / 1000.0) as i64
  }
  #[cfg(not(target_arch = "wasm32"))]
  {
    std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map(|d| d.as_secs() as i64)
      .unwrap_or(0)
  }
}

/// Converts days since the Unix epoch to a civil calendar date (Howard Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
  let z = z + 719_468;
  let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
  let doe = z - era * 146_097;
  let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
  let y = yoe + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
  let mp = (5 * doy + 2) / 153;
  let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
  let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
  (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 4-part Semver representation of `WEBCONFIG_APP_VERSION` (`"<major>.<minor>.<patch>.0"`) required by OrcaSlicer's
/// `Semver::parse(key_values["version"])` when importing `.json` preset files from `.zip` bundles.
pub const WEBCONFIG_ORCA_PRESET_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), ".0");

/// Splits an auto-generated `OC -` preset title into `(title_base, nozzle_diameter, nozzle_tag)`:
/// `"OC - Kobra S1 • 0.4 Diamondback +FlowTech"` -> `("Kobra S1", "0.4", "Diamondback")` and
/// `"OC - PETG • 0.4 Hardened"` -> `("PETG", "0.4", "Hardened")`.
/// Returns `None` when the title does not follow the `• <diameter> <tag>` convention, in which case
/// callers must treat the preset as a regular hand-named profile (never prune / never retag it).
pub fn split_oc_nozzle_title(title: &str) -> Option<(String, String, String)> {
  let cleaned = title.trim();
  let stripped = cleaned
    .strip_prefix("OC - ")
    .or_else(|| cleaned.strip_prefix("OC: "))
    .or_else(|| cleaned.strip_prefix("OC -"))
    .or_else(|| cleaned.strip_prefix("OC:"))
    .unwrap_or(cleaned)
    .trim();
  let idx = stripped.rfind("• ")?;
  let base = stripped[..idx].trim().to_string();
  if base.is_empty() {
    return None;
  }
  // `•` is 3 bytes of UTF-8, so the bullet + space separator is 4 bytes wide.
  let tail = stripped[idx + "• ".len()..].trim();
  let mut tokens = tail.split_whitespace();
  let diam = tokens.next()?;
  diam.parse::<f64>().ok()?;
  let tag = tokens.next()?.to_string();
  Some((base, diam.to_string(), tag))
}

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
        name: "MicroSwiss FlowTech™ All-Metal Hotend (Required for MicroSwiss Nozzles)".to_string(),
        category: "Hotend (Auto-Installed with MicroSwiss Nozzles)".to_string(),
        effect_summary: "The hotend BODY that ALL MicroSwiss nozzles (Diamondback PCD / CM2 / CHT / Plated Brass) thread into — they do NOT fit the stock hotend. Selecting any MicroSwiss nozzle installs this hotend automatically. Leak-proof ceramic cylindrical heater; compresses retraction through a viscoelastic curve (rigid filaments ~0.88–0.95×, TPU travel preserved, scaled by nozzle orifice) and boosts melt-zone multiplier 1.15x.".to_string(),
      },
    ]
  }

  /// Friendly display name for an add-on `id` (e.g. `"flowtech_hotend"` -> `"MicroSwiss FlowTech™ All-Metal Hotend"`),
  /// falling back to the raw id for unknown/custom add-ons.
  pub fn friendly_name(addon_id: &str) -> String {
    Self::get_catalog()
      .into_iter()
      .find(|a| a.id == addon_id)
      .map(|a| a.name.split(" (").next().unwrap_or(&a.name).to_string())
      .unwrap_or_else(|| addon_id.to_string())
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

  /// Short nozzle material tag for titles (e.g. "Diamondback", "Ruby-Brass", "MS-Brass", "MS-CM2", "CHT-HiFlow", "Hardened", "Brass").
  /// Material synonyms (e.g. "TwinClad") are only consulted on the rig itself; filament spools carry just `nozzle_type`.
  pub fn short_nozzle_tag(&self) -> &'static str {
    let nt = self.nozzle_type.to_lowercase();
    let nm = self.nozzle_material.to_lowercase();
    if nt.contains("diamondback") || nm.contains("diamondback") || nm.contains("pcd") {
      "Diamondback"
    } else {
      Self::short_nozzle_tag_for_type(&self.nozzle_type)
    }
  }

  /// True for MicroSwiss-manufactured nozzle types (Diamondback PCD, CM2, CHT CM2, FlowTech CHT,
  /// Plated/MK8 Brass). These nozzles ONLY fit the MicroSwiss FlowTech all-metal hotend — they do
  /// NOT thread into the stock hotend — so selecting one auto-installs the `flowtech_hotend` add-on.
  pub fn is_microswiss_nozzle_type(nozzle_type: &str) -> bool {
    let nt = nozzle_type.to_lowercase();
    nt.contains("microswiss")
      || nt.contains("diamondback")
      || nt.contains("cm2")
      || nt.contains("cht")
      || nt.contains("mk8_brass")
  }

  /// Motion-system class of this rig ("corexy" | "bedslinger"), derived from its kinematics
  /// description via constants.json `printer_classes` (tokens + default). Exposed to the Lua
  /// rules as `ctx.printer_kinematics_class`.
  pub fn kinematics_class(&self) -> String {
    crate::constants::constants().kinematics_class(&self.kinematics)
  }

  /// Travel-acceleration breakaway policy for this rig's motion-system class
  /// (`printer_classes.policies`): stock/target/floor in mm/s². Printer-class physics —
  /// a CoreXY gantry and a bed-slinger need different numbers for the same rule.
  pub fn travel_accel_policy(&self) -> crate::constants::PrinterClassPolicy {
    crate::constants::constants().printer_class_policy(&self.kinematics_class())
  }

  /// Canonical nozzle tag from `nozzle_type` alone (`"diamondback"` -> `"Diamondback"`), so machine
  /// titles and filament preset names (`OC - PETG • 0.4 Diamondback`) always agree on the same tag.
  pub fn short_nozzle_tag_for_type(nozzle_type: &str) -> &'static str {
    let nt = nozzle_type.to_lowercase();
    if nt.contains("diamondback") {
      "Diamondback"
    } else if nt.contains("ruby") {
      "Ruby-Brass"
    } else if nt.contains("microswiss_brass") || nt.contains("mk8_brass") {
      "MS-Brass"
    } else if nt.contains("cht_cm2") || (nt.contains("cht") && nt.contains("cm2")) {
      "MS-CHT-CM2"
    } else if nt.contains("cm2") {
      "MS-CM2"
    } else if nt.contains("cht") || nt.contains("obxidian") {
      "CHT-HiFlow"
    } else if nt.contains("hardened") {
      "Hardened"
    } else if nt.contains("stainless") {
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
  /// - MicroSwiss Diamondback PCD: `0.25, 0.4, 0.6, 0.8, 1.0 mm` (official FlowTech DiamondBack lineup)
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
        ("1.0", "1.00 mm"),
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
      "Diamondback" => "PCD 543 W/m·K thermal curve (−5…−11°C by polymer band; see transformation stack), quadratic die-swell flow trim ≤1.2%, nozzle_type=brass + nozzle_hrc=68 (PCD hardness 49.8 GPa)",
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
      self
        .addons
        .iter()
        .map(|a| RigAddOn::friendly_name(a))
        .collect::<Vec<_>>()
        .join(", ")
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
    let fc = crate::constants::constants().lookup_finish(finish_type);
    if fc.code == "Carbon Fiber" || fc.code == "Glass Fiber" {
      return None;
    }
    fc.title_suffix.as_deref()
  }

  /// Normalizes a brand label (`"Custom"`, `""` -> `"Generic"`).
  pub fn normalized_brand(raw_brand: &str) -> String {
    crate::constants::constants()
      .normalize_brand(raw_brand)
      .to_string()
  }

  /// Builds the canonical `OC - [<Brand> ]<Material>[ <Finish>] • 0.4 <Nozzle>` preset name.
  pub fn default_oc_profile_name_with_brand(
    brand: &str,
    base_mat_code: &str,
    finish_type: &str,
    active_nozzle_type: &str,
  ) -> String {
    // Canonical tag (matches `MachineRig::short_nozzle_tag` in machine titles, e.g. "Diamondback")
    let short_noz = MachineRig::short_nozzle_tag_for_type(active_nozzle_type);
    let mat_with_finish = if finish_type.eq_ignore_ascii_case("Carbon Fiber") {
      let up = base_mat_code.to_uppercase();
      if up.contains("-CF")
        || up.contains("-GF")
        || up.contains("-RCF")
        || up.contains("-RGF")
        || up.contains("CARBON")
        || up.contains("GLASS")
      {
        base_mat_code.to_string()
      } else {
        format!("{}-CF", base_mat_code)
      }
    } else if finish_type.eq_ignore_ascii_case("Glass Fiber") {
      let up = base_mat_code.to_uppercase();
      if up.contains("-GF")
        || up.contains("-CF")
        || up.contains("-RCF")
        || up.contains("-RGF")
        || up.contains("GLASS")
        || up.contains("CARBON")
      {
        base_mat_code.to_string()
      } else {
        format!("{}-GF", base_mat_code)
      }
    } else if let Some(suf) = Self::finish_title_suffix(finish_type) {
      let fc = crate::constants::constants().lookup_finish(finish_type);
      let has_alias_in_name = fc
        .aliases
        .iter()
        .any(|al| !al.is_empty() && base_mat_code.to_lowercase().contains(&al.to_lowercase()));
      if !base_mat_code.to_lowercase().contains(&suf.to_lowercase())
        && !has_alias_in_name
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
    let c = crate::constants::constants();
    let hs_delta = c.hardware_limits.hs_base_temp_delta_c;
    let brass = if self.brass_base_temp > 0 {
      self.brass_base_temp
    } else {
      (self.base_print_temp - hs_delta).max(190)
    };
    let hs = if self.hs_base_temp > 0 {
      self.hs_base_temp
    } else {
      brass + hs_delta
    };
    let is_external_disk_spool = self.id.starts_with("disk_")
      && !self.name.starts_with("OC -")
      && !self.name.starts_with("OC:");
    self.nozzle_type = new_nozzle_type.to_string();
    if !is_external_disk_spool {
      self.tuned_for_nozzle_type = new_nozzle_type.to_string();
    }

    let (min_safe, mut max_safe) = Self::polymer_safe_nozzle_bounds(&self.material_type);
    if self.is_foaming_tpu() {
      max_safe = max_safe.max(c.hardware_limits.max_foaming_tpu_nozzle_temp_c);
    }
    let noz_cfg = c.lookup_nozzle(new_nozzle_type);
    if noz_cfg.uses_hs_base_temp {
      self.base_print_temp = hs;
      // Only preserve a custom Hardened Steel disk override on an external non-OC disk profile within +15°C of `hs`
      if is_external_disk_spool
        && self.tuned_for_nozzle_type.contains("hardened")
        && self.print_temp > hs
        && self.print_temp <= hs + 15
      {
        // Keep external user's HS override
      } else {
        self.print_temp = hs.clamp(min_safe, max_safe);
      }
    } else {
      self.base_print_temp = brass;
      self.print_temp = (brass + noz_cfg.temp_offset_from_brass_c).clamp(min_safe, max_safe);
    }

    if !is_external_disk_spool && (self.name.starts_with("OC -") || self.name.starts_with("OC:")) {
      // Canonical tag so filament presets rename in lockstep with the machine title
      // (e.g. `OC - PETG • 0.4 Hardened` -> `OC - PETG • 0.4 Diamondback`).
      let new_short_noz = MachineRig::short_nozzle_tag_for_type(new_nozzle_type);
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
    let rig_is_hs = crate::constants::constants()
      .lookup_nozzle(rig_nozzle_type)
      .uses_hs_base_temp;
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
    &crate::constants::constants()
      .lookup_polymer(mat)
      .canonical_key
  }

  /// Broad polymer family key for inheritance compatibility & fallback matching across brands.
  pub fn polymer_family_key(mat: &str) -> &'static str {
    &crate::constants::constants().lookup_polymer(mat).family
  }

  /// Returns `(min_safe_nozzle_c, max_safe_nozzle_c)` for a given polymer so that stacked
  /// nozzle-material offsets (e.g. Diamondback -10°C), surface-finish offsets (e.g. Wood -5°C),
  /// and climate offsets (e.g. Tropical/Hot -5..-10°C or Cold +6°C) can NEVER push the final
  /// nozzle temperature below the polymer's cold-extrusion limit or above its thermal-degradation limit.
  pub fn polymer_safe_nozzle_bounds(material_type: &str) -> (i32, i32) {
    let c = crate::constants::constants();
    if material_type.to_uppercase().contains("ESD") {
      return (
        c.hardware_limits.esd_safe_nozzle_min_c,
        c.hardware_limits.esd_safe_nozzle_max_c,
      );
    }
    let p = c.lookup_polymer(material_type);
    (p.safe_nozzle_min_c, p.safe_nozzle_max_c)
  }

  /// Returns `(min_safe_bed_c, max_safe_bed_c)` for a given polymer on the Kobra S1 (`<= 110°C` bed ceiling)
  /// so climate offsets never drop bed temperatures below adhesion floor (`>= 40°C`) or above `110°C`.
  pub fn polymer_safe_bed_bounds(material_type: &str) -> (i32, i32) {
    let p = crate::constants::constants().lookup_polymer(material_type);
    (p.safe_bed_min_c, p.safe_bed_max_c)
  }

  /// Computes a multi-material-compatible `[nozzle_temperature_range_low, nozzle_temperature_range_high]`
  /// window for OrcaSlicer / AnycubicSlicerNext so single-extruder multi-material prints (ACE Pro / AMS)
  /// mixing compatible filaments (e.g. `TPU for AMS` at 220–230°C + `PETG` at 230–255°C, or `PLA` + `TPU` / `PETG` supports)
  /// do not trigger false "Selected Nozzle Temperature are incompatible, which may result in clogging" errors,
  /// while never exceeding the Kobra S1's physical `300°C` hotend ceiling.
  pub fn compute_nozzle_temp_range(
    material_type: &str,
    effective_temp: i32,
    initial_temp: i32,
  ) -> (i32, i32) {
    let c = crate::constants::constants();
    let hw = &c.hardware_limits;
    let p = c.lookup_polymer(material_type);
    if p.canonical_key == "PCL" {
      return (hw.pcl_temp_range_low_c, hw.pcl_temp_range_high_c);
    }
    let max_t = effective_temp.max(initial_temp);
    let min_t = effective_temp.min(initial_temp);
    if max_t > 265 || !p.standard_ams_compatible {
      let low = hw
        .high_temp_range_low_c
        .min(min_t)
        .max(hw.standard_ams_range_low_c);
      let high = hw
        .high_temp_range_high_c
        .max(max_t)
        .min(hw.max_hotend_temp_c);
      (low, high)
    } else {
      let low = hw
        .standard_ams_range_low_c
        .min(min_t - 10)
        .max(hw.min_extrusion_temp_c);
      let high = hw
        .standard_ams_range_high_c
        .max(max_t)
        .min(hw.max_hotend_temp_c);
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
      let preserved_mat = if self.material_type.eq_ignore_ascii_case(target_canon) {
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

    let c = crate::constants::constants();
    let fallback_poly = c.lookup_polymer(new_material);
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
      (
        fallback_poly.default_brass_nozzle_c,
        fallback_poly.default_bed_c,
        fallback_poly.default_max_volumetric_speed,
        fallback_poly.default_fan_pct,
        fallback_poly.default_dry_temp_c,
        fallback_poly.default_dry_time_hr,
      )
    };

    let hs_temp = if new_material.eq_ignore_ascii_case("PLA+") && self.brand == "Generic" {
      220
    } else {
      (brass_temp + 5).clamp(80, 420)
    };

    let base_mat_code = mat_entry
      .map(|m| m.base_material_code().to_string())
      .unwrap_or_else(|| c.base_material_code(new_material).to_string());

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

    if let Some(m) = mat_entry {
      self.finish_type = m.default_finish.clone();
    } else if let Some(intrinsic_fin) = c.detect_intrinsic_finish(new_material) {
      self.finish_type = intrinsic_fin.to_string();
    } else if self.finish_type == "Carbon Fiber" || self.finish_type == "Glass Fiber" {
      self.finish_type = "Basic".to_string();
    }
    let is_abrasive = mat_entry.map(|m| m.requires_hardened).unwrap_or(false)
      || Self::is_abrasive_material_or_finish(new_material, &self.finish_type);
    self.required_nozzle_hrc = if is_abrasive {
      c.hardware_limits.abrasive_required_hrc
    } else {
      c.hardware_limits.non_abrasive_required_hrc
    };

    let poly_cfg = c.lookup_polymer(&base_mat_code);
    self.pressure_advance = poly_cfg.default_pressure_advance;
    if let Some(m) = mat_entry {
      let min_valid_flow = if m.default_finish.eq_ignore_ascii_case("Foaming") {
        c.hardware_limits.min_flow_ratio
      } else {
        c.hardware_limits.min_non_foaming_flow_ratio
      };
      if m.default_flow_ratio >= min_valid_flow
        && m.default_flow_ratio <= c.hardware_limits.max_non_foaming_flow_ratio
      {
        self.flow_ratio = m.default_flow_ratio;
      } else {
        self.flow_ratio = poly_cfg.default_flow_ratio;
      }
    } else {
      self.flow_ratio = poly_cfg.default_flow_ratio;
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
    let c = crate::constants::constants();
    let effective_finish = match c.detect_intrinsic_finish(&self.material_type) {
      Some(fin @ ("Carbon Fiber" | "Glass Fiber")) => fin,
      _ if c.is_finish_compatible_with_polymer(new_finish, &self.material_type) => new_finish,
      _ => "Basic",
    };
    self.finish_type = effective_finish.to_string();
    let new_finish = effective_finish;
    let poly_before_upgrade = c.lookup_polymer(&self.material_type);
    let fc = c.lookup_finish(new_finish);
    if fc.code == "Carbon Fiber" {
      if let Some(ref cf_code) = poly_before_upgrade.cf_upgrade_code {
        if self
          .material_type
          .trim()
          .eq_ignore_ascii_case(&poly_before_upgrade.canonical_key)
        {
          self.material_type = cf_code.clone();
          self.inherits = crate::threemf::resolve_kobra_s1_filament_inherits(self);
        }
      }
    } else if fc.code == "Glass Fiber" {
      if let Some(ref gf_code) = poly_before_upgrade.gf_upgrade_code {
        if self
          .material_type
          .trim()
          .eq_ignore_ascii_case(&poly_before_upgrade.canonical_key)
        {
          self.material_type = gf_code.clone();
          self.inherits = crate::threemf::resolve_kobra_s1_filament_inherits(self);
        }
      }
    }

    // Look up whether the catalog has a dedicated entry for `(self.brand, self.material_type, new_finish)`
    let catalog = crate::catalog::load_merged_catalog();
    let norm_brand = Self::normalized_brand(&self.brand);
    let variant_entry = if matches!(new_finish, "Basic" | "Matte" | "Silk")
      || (Self::finish_title_suffix(new_finish).is_none()
        && fc.code != "Carbon Fiber"
        && fc.code != "Glass Fiber")
    {
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
      let min_valid_flow = if fc.code == "Foaming" {
        c.hardware_limits.min_flow_ratio
      } else {
        c.hardware_limits.min_non_foaming_flow_ratio
      };
      if m.default_flow_ratio >= min_valid_flow
        && m.default_flow_ratio <= c.hardware_limits.max_non_foaming_flow_ratio
        && (m.default_flow_ratio - 0.98).abs() > 0.005
      {
        self.flow_ratio = m.default_flow_ratio;
      } else {
        let poly_cfg = c.lookup_polymer(&self.material_type);
        self.flow_ratio = poly_cfg.default_flow_ratio;
      }
      self.recompute_for_nozzle(active_nozzle_type);
    }

    self.opacity = fc.default_opacity;
    self.pigment_stickiness = fc.default_pigment_stickiness;
    let poly_cfg = c.lookup_polymer(&self.material_type);
    self.pressure_advance = poly_cfg.resolve_pressure_advance(&fc.code);
    if fc.code == "Translucent" && poly_cfg.translucent_z_hop_mm.is_some() {
      self.base_bed_temp = 65;
      self.bed_temp = 65;
      self.max_volumetric_speed = 9.0;
      self.fan_speed_percent = 30;
      self.drying_temp_c = 65;
      self.drying_time_hr = 6;
    } else if fc.code != "Translucent"
      && poly_cfg.translucent_z_hop_mm.is_some()
      && variant_entry.is_none()
    {
      self.base_bed_temp = poly_cfg.default_bed_c;
      self.bed_temp = poly_cfg.default_bed_c;
      self.max_volumetric_speed = poly_cfg.default_max_volumetric_speed;
      self.fan_speed_percent = poly_cfg.default_fan_pct;
      self.drying_temp_c = resolved_entry
        .map(|m| m.dry_temp_c as i32)
        .unwrap_or(poly_cfg.default_dry_temp_c);
      self.drying_time_hr = poly_cfg.default_dry_time_hr;
    }

    // Flow ratio & MVS adjustments for Foaming finishes
    if fc.code == "Foaming" {
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
      self.flow_ratio = c.lookup_polymer(&self.material_type).default_flow_ratio;
    }

    // Update `OC - [<Brand> ]<Material>[ <Finish>] • 0.4 <Nozzle>` title suffix cleanly
    // (including when `self.brand != "Generic"`, where `self.name` starts with `OC - <Brand> ...`)
    if self.name.starts_with("OC -") {
      self.name = Self::default_oc_profile_name_with_brand(
        &self.brand,
        &self.material_type,
        &self.finish_type,
        active_nozzle_type,
      );
    }

    let is_abrasive = Self::is_abrasive_material_or_finish(&self.material_type, &self.finish_type);
    self.required_nozzle_hrc = if is_abrasive {
      c.hardware_limits.abrasive_required_hrc
    } else {
      c.hardware_limits.non_abrasive_required_hrc
    };
    self.rebuild_synthetic_disk_json(active_nozzle_type);
  }

  /// Single source of truth for whether a material code or surface finish contains abrasive particles
  /// (`-CF`, `-GF`, `Carbon Fiber`, `Glass Fiber`, `Wood-Filled`, `Metal-Filled`, `Glow-in-the-Dark`,
  /// `Conductive`, `Marble/Stone/Terracotta`, `ESD`) requiring a hardened nozzle (`HRC >= 55`).
  /// Unfilled polymers (`PLA`, `PETG`, `PCTG`, `TPU`, `PA`, `PC`, `ABS`, `ASA`, `PVA`, `BVOH`, `HIPS`, `PHA`)
  /// with non-abrasive finishes (`Basic`, `Matte`, `Silk`, `Translucent`, `Foaming`, `Photochromic`) return `false`.
  pub fn is_abrasive_material_or_finish(material_type: &str, finish_type: &str) -> bool {
    crate::constants::constants().is_abrasive_material_or_finish(material_type, finish_type)
  }

  /// Returns true if this spool is an active-foaming flexible filament (`TPU`, `Rebound TPU`, `PEBA`, `TPE` with `Foaming` finish).
  pub fn is_foaming_tpu(&self) -> bool {
    let c = crate::constants::constants();
    c.lookup_polymer(&self.material_type).is_flexible()
      && c.lookup_finish(&self.finish_type).code == "Foaming"
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
    let c = crate::constants::constants();
    let short_noz = MachineRig::short_nozzle_tag_for_type(active_nozzle_type);
    let poly_cfg = c.lookup_polymer(&self.material_type);
    let is_foaming_tpu = self.is_foaming_tpu();
    let init_boost = poly_cfg.initial_layer_temp_boost_c(is_foaming_tpu);

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
    let ret_len_val = if self.is_siraya_foaming_air() {
      0.0
    } else if is_foaming_tpu {
      0.35
    } else {
      poly_cfg.default_retraction_mm as f64
    };
    let ret_len_str = crate::constants::PolymerConstants::format_retraction_mm(ret_len_val);
    let ret_spd_str = if is_foaming_tpu {
      "25".to_string()
    } else {
      poly_cfg.default_retraction_speed_mms.to_string()
    };
    let fan_max_str = poly_cfg
      .resolve_fan_max_pct(self.fan_speed_percent)
      .to_string();

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
      self.print_temp.min(self.brass_base_temp),
      (self.print_temp.max(self.hs_base_temp) + init_boost).min(420),
    );
    let (z_hop_str, z_hop_type_str, retract_before_wipe_str) =
      poly_cfg.resolve_z_hop_and_retract_before_wipe(&self.finish_type, is_foaming_tpu);
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
      "cool_plate_temp": [self.bed_temp.to_string()],
      "cool_plate_temp_initial_layer": [self.bed_temp.to_string()],
      "eng_plate_temp": [self.bed_temp.to_string()],
      "eng_plate_temp_initial_layer": [self.bed_temp.to_string()],
      "textured_cool_plate_temp": [self.bed_temp.to_string()],
      "textured_cool_plate_temp_initial_layer": [self.bed_temp.to_string()],
      "supertack_plate_temp": [self.bed_temp.to_string()],
      "supertack_plate_temp_initial_layer": [self.bed_temp.to_string()],
      "filament_flow_ratio": [format!("{:.2}", self.flow_ratio)],
      "enable_pressure_advance": ["1"],
      "pressure_advance": [format!("{:.3}", self.pressure_advance)],
      "temperature_vitrification": [(self.bed_temp + 10).max(55).to_string()],
      "filament_max_volumetric_speed": [format!("{:.1}", self.max_volumetric_speed)],
      "fan_min_speed": [self.fan_speed_percent.to_string()],
      "fan_min_speed_BRASS": [self.fan_speed_percent.to_string()],
      "fan_min_speed_HS": [self.fan_speed_percent.to_string()],
      "fan_max_speed": [fan_max_str],
      "fan_max_speed_BRASS": [fan_max_str],
      "fan_max_speed_HS": [fan_max_str],
      "filament_retraction_length": [ret_len_str],
      "filament_retraction_speed": [ret_spd_str],
      "filament_z_hop": [z_hop_str],
      "filament_z_hop_types": [z_hop_type_str],
      "filament_retract_before_wipe": [retract_before_wipe_str],
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
    let c = crate::constants::constants();
    let base_mat_code = mat.base_material_code().to_string();
    let finish_type = mat.default_finish.clone();
    let base_nozzle = mat.base_nozzle_c as i32;
    let base_bed = mat.base_bed_c as i32;
    let hs_nozzle = (base_nozzle + 5).clamp(80, 420);
    let poly_cfg = c.lookup_polymer(&base_mat_code);
    let fc = c.lookup_finish(&finish_type);
    let default_pa = poly_cfg.resolve_pressure_advance(&fc.code);
    let min_valid_flow = if fc.code == "Foaming" {
      c.hardware_limits.min_flow_ratio
    } else {
      c.hardware_limits.min_non_foaming_flow_ratio
    };
    let default_flow = if mat.default_flow_ratio >= min_valid_flow
      && mat.default_flow_ratio <= c.hardware_limits.max_non_foaming_flow_ratio
      && (mat.default_flow_ratio - 0.98).abs() > 0.005
    {
      mat.default_flow_ratio
    } else {
      poly_cfg.default_flow_ratio
    };
    let norm_brand = Self::normalized_brand(&mat.brand);
    let inherits = c.resolve_filament_inherits("Anycubic Kobra S1", "0.4", &base_mat_code);
    let profile_name = Self::default_oc_profile_name_with_brand(
      &norm_brand,
      &base_mat_code,
      &finish_type,
      active_nozzle_type,
    );
    let is_abrasive =
      mat.requires_hardened || Self::is_abrasive_material_or_finish(&base_mat_code, &finish_type);
    let req_hrc: u32 = if is_abrasive {
      c.hardware_limits.abrasive_required_hrc
    } else {
      c.hardware_limits.non_abrasive_required_hrc
    };
    let fan_pct = mat.fan_pct as i32;
    let max_vol = mat.max_vol_mm3s;
    let color_hex = "#8E9299".to_string();

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
      material_type: base_mat_code,
      nozzle_type: active_nozzle_type.to_string(),
      tuned_for_nozzle_type: active_nozzle_type.to_string(),
      required_nozzle_hrc: req_hrc,
      color_hex,
      color_name: "Cool Gray".to_string(),
      brass_base_temp: base_nozzle,
      hs_base_temp: hs_nozzle,
      base_print_temp: base_nozzle,
      print_temp: base_nozzle,
      base_bed_temp: base_bed,
      bed_temp: base_bed,
      flow_ratio: default_flow,
      pressure_advance: default_pa,
      max_volumetric_speed: max_vol,
      fan_speed_percent: fan_pct,
      remaining_grams: 1000,
      is_loaded: true,
      finish_type: finish_type.clone(),
      opacity: fc.default_opacity,
      pigment_stickiness: fc.default_pigment_stickiness,
      drying_temp_c: mat.dry_temp_c as i32,
      drying_time_hr: mat.dry_hr as i32,
      is_rfid_synced: false,
      deltas_from_base: Vec::new(),
      raw_disk_json: None,
    };
    spool.recompute_for_nozzle(active_nozzle_type);
    spool.apply_finish_change(&finish_type, active_nozzle_type);
    spool
  }

  /// Generate a full descriptive note (`filament_notes`) including the WebConfig app version and tuned parameters
  pub fn generate_descriptive_notes(&self) -> String {
    self.generate_descriptive_notes_with_effective_temps(self.print_temp, self.bed_temp)
  }

  /// Generate `filament_notes` reflecting the exact exported `effective_nozzle_c` and `effective_bed_c`
  /// (including any active climate/weather compensation applied during export).
  pub fn generate_descriptive_notes_with_effective_temps(
    &self,
    effective_nozzle_c: i32,
    effective_bed_c: i32,
  ) -> String {
    self.generate_descriptive_notes_with_effective_values(
      effective_nozzle_c,
      effective_bed_c,
      self.flow_ratio,
      self.max_volumetric_speed,
      self.fan_speed_percent,
    )
  }

  /// Generate `filament_notes` reflecting all effective exported values (`effective_nozzle_c`,
  /// `effective_bed_c`, `effective_flow_ratio`, `effective_mvs`, and `effective_fan_pct`).
  pub fn generate_descriptive_notes_with_effective_values(
    &self,
    effective_nozzle_c: i32,
    effective_bed_c: i32,
    effective_flow_ratio: f32,
    effective_mvs: f32,
    effective_fan_pct: i32,
  ) -> String {
    self.generate_descriptive_notes_with_effective_values_and_pa(
      effective_nozzle_c,
      effective_bed_c,
      effective_flow_ratio,
      self.pressure_advance,
      effective_mvs,
      effective_fan_pct,
    )
  }

  /// Generate `filament_notes` reflecting all effective exported values including coupled `effective_pa`.
  pub fn generate_descriptive_notes_with_effective_values_and_pa(
    &self,
    effective_nozzle_c: i32,
    effective_bed_c: i32,
    effective_flow_ratio: f32,
    effective_pa: f32,
    effective_mvs: f32,
    effective_fan_pct: i32,
  ) -> String {
    format!(
      "[OrcaSlicer WebConfig v{} — Filament] {} {} ({}). Nozzle: {}°C (Brass Base: {}°C, HS Base: {}°C), Bed: {}°C, Flow: {:.2}, PA: {:.3}, Max Vol: {:.1} mm³/s, Fan: {}%, Required HRC: {}, Dry: {}°C / {}h.",
      WEBCONFIG_APP_VERSION,
      Self::normalized_brand(&self.brand),
      self.material_type,
      self.finish_type,
      effective_nozzle_c,
      self.brass_base_temp,
      self.hs_base_temp,
      effective_bed_c,
      effective_flow_ratio,
      effective_pa,
      effective_mvs,
      effective_fan_pct,
      self.required_nozzle_hrc,
      self.drying_temp_c,
      self.drying_time_hr
    )
  }

  /// If this `FilamentSpool` was created from or linked to Spoolman (`id` starting with `"spoolman_"`),
  /// returns `Some((spool_id, filament_id))`.
  pub fn spoolman_ids(&self) -> Option<(u32, u32)> {
    let rest = self.id.strip_prefix("spoolman_")?;
    if let Some((s_str, f_str)) = rest.split_once('_') {
      let sid = s_str.parse::<u32>().ok()?;
      let fid = f_str
        .split('_')
        .next()
        .and_then(|x| x.parse::<u32>().ok())
        .unwrap_or(sid);
      Some((sid, fid))
    } else {
      let sid = rest.parse::<u32>().ok()?;
      Some((sid, sid))
    }
  }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpoolmanSpool {
  pub spool_id: u32,
  pub filament_id: u32,
  pub vendor_name: String,
  pub filament_name: String,
  pub material: String,
  pub color_hex: String,
  pub remaining_weight_g: i32,
  pub initial_weight_g: i32,
  pub extruder_temp_c: Option<i32>,
  pub bed_temp_c: Option<i32>,
  #[serde(default)]
  pub location: String,
  #[serde(default)]
  pub lot_nr: String,
  #[serde(default)]
  pub comment: String,
  #[serde(default)]
  pub extra_flow_ratio: Option<f32>,
  #[serde(default)]
  pub extra_pressure_advance: Option<f32>,
  #[serde(default)]
  pub extra_max_volumetric_speed: Option<f32>,
}

fn extract_extra_f32(extra: Option<&serde_json::Value>, keys: &[&str]) -> Option<f32> {
  let obj = extra?.as_object()?;
  for k in keys {
    if let Some(v) = obj.get(*k) {
      if let Some(f) = v.as_f64() {
        return Some(f as f32);
      }
      if let Some(s) = v.as_str() {
        let cleaned = s.trim().trim_matches('"').trim_matches('\'').trim();
        if let Ok(f) = cleaned.parse::<f32>() {
          return Some(f);
        }
      }
    }
  }
  None
}

impl SpoolmanSpool {
  /// Parses a Spoolman inventory JSON payload. Supports:
  /// 1. Raw Spoolman `GET /api/v1/spool` JSON array (`[ { "id": 1, "filament": { ... }, ... } ]`)
  /// 2. Wrapped object `{ "spools": [...] }` or `{ "items": [...] }`
  /// 3. Direct `Vec<SpoolmanSpool>` array
  pub fn parse_inventory_json(json_str: &str) -> Result<Vec<Self>, String> {
    let trimmed = json_str.trim();
    if trimmed.is_empty() {
      return Err("Empty Spoolman JSON payload".to_string());
    }
    if let Ok(direct) = serde_json::from_str::<Vec<SpoolmanSpool>>(trimmed) {
      if !direct.is_empty() {
        return Ok(direct);
      }
    }

    let root: serde_json::Value =
      serde_json::from_str(trimmed).map_err(|e| format!("Invalid JSON: {}", e))?;
    let arr = if let Some(a) = root.as_array() {
      a
    } else if let Some(a) = root.get("spools").and_then(|v| v.as_array()) {
      a
    } else if let Some(a) = root.get("items").and_then(|v| v.as_array()) {
      a
    } else if root.get("id").is_some() && root.get("filament").is_some() {
      return Self::from_api_spool_value(&root).map(|s| vec![s]);
    } else {
      return Err(
        "Expected a Spoolman `/api/v1/spool` JSON array or object containing `spools`".to_string(),
      );
    };

    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
      if let Ok(s) = serde_json::from_value::<SpoolmanSpool>(item.clone()) {
        out.push(s);
      } else if let Ok(s) = Self::from_api_spool_value(item) {
        out.push(s);
      }
    }
    Ok(out)
  }

  pub fn from_api_spool_value(v: &serde_json::Value) -> Result<Self, String> {
    let archived = v.get("archived").and_then(|b| b.as_bool()).unwrap_or(false);
    if archived {
      return Err("Archived spool".to_string());
    }
    let spool_id = v
      .get("id")
      .and_then(|x| x.as_u64())
      .ok_or_else(|| "Missing spool id".to_string())? as u32;
    let fil = v.get("filament").unwrap_or(v);
    let filament_id = fil
      .get("id")
      .and_then(|x| x.as_u64())
      .map(|x| x as u32)
      .unwrap_or(spool_id);
    let vendor_name = fil
      .get("vendor")
      .and_then(|vnd| vnd.get("name"))
      .and_then(|x| x.as_str())
      .or_else(|| v.get("vendor_name").and_then(|x| x.as_str()))
      .unwrap_or("Generic")
      .trim()
      .to_string();
    let filament_name = fil
      .get("name")
      .and_then(|x| x.as_str())
      .unwrap_or("Filament")
      .trim()
      .to_string();
    let material = fil
      .get("material")
      .and_then(|x| x.as_str())
      .unwrap_or("PLA")
      .trim()
      .to_string();
    let raw_hex = fil
      .get("color_hex")
      .and_then(|x| x.as_str())
      .or_else(|| {
        fil
          .get("multi_color_hexes")
          .and_then(|x| x.as_str())
          .and_then(|s| s.split(',').next())
      })
      .unwrap_or("06B6D4")
      .trim()
      .trim_start_matches('#');
    let clean_hex = if raw_hex.len() >= 6 {
      format!("#{}", &raw_hex[..6].to_uppercase())
    } else {
      "#06B6D4".to_string()
    };

    let initial_weight_g = v
      .get("initial_weight")
      .and_then(|x| x.as_f64())
      .or_else(|| fil.get("weight").and_then(|x| x.as_f64()))
      .unwrap_or(1000.0)
      .round() as i32;
    let remaining_weight_g = v
      .get("remaining_weight")
      .and_then(|x| x.as_f64())
      .map(|w| w.round() as i32)
      .unwrap_or(initial_weight_g)
      .max(0);

    let extruder_temp_c = fil
      .get("settings_extruder_temp")
      .and_then(|x| x.as_i64())
      .map(|x| x as i32)
      .filter(|&t| (140..=420).contains(&t));
    let bed_temp_c = fil
      .get("settings_bed_temp")
      .and_then(|x| x.as_i64())
      .map(|x| x as i32)
      .filter(|&t| (0..=150).contains(&t));

    let location = v
      .get("location")
      .and_then(|x| x.as_str())
      .unwrap_or("")
      .trim()
      .to_string();
    let lot_nr = v
      .get("lot_nr")
      .and_then(|x| x.as_str())
      .unwrap_or("")
      .trim()
      .to_string();
    let comment = v
      .get("comment")
      .and_then(|x| x.as_str())
      .unwrap_or("")
      .trim()
      .to_string();

    let spool_extra = v.get("extra");
    let fil_extra = fil.get("extra");
    let extra_flow_ratio =
      extract_extra_f32(spool_extra, &["flow_ratio", "filament_flow_ratio", "flow"])
        .or_else(|| extract_extra_f32(fil_extra, &["flow_ratio", "filament_flow_ratio", "flow"]));
    let extra_pressure_advance =
      extract_extra_f32(spool_extra, &["pressure_advance", "pa", "k_value"])
        .or_else(|| extract_extra_f32(fil_extra, &["pressure_advance", "pa", "k_value"]));
    let extra_max_volumetric_speed = extract_extra_f32(
      spool_extra,
      &[
        "max_volumetric_speed",
        "filament_max_volumetric_speed",
        "mvs",
      ],
    )
    .or_else(|| {
      extract_extra_f32(
        fil_extra,
        &[
          "max_volumetric_speed",
          "filament_max_volumetric_speed",
          "mvs",
        ],
      )
    });

    Ok(SpoolmanSpool {
      spool_id,
      filament_id,
      vendor_name,
      filament_name,
      material,
      color_hex: clean_hex,
      remaining_weight_g,
      initial_weight_g,
      extruder_temp_c,
      bed_temp_c,
      location,
      lot_nr,
      comment,
      extra_flow_ratio,
      extra_pressure_advance,
      extra_max_volumetric_speed,
    })
  }

  /// Infers a 0-based ACE Pro / AMS slot index (`0..max_slots`) from `self.location`,
  /// supporting e.g. `"ACE 1"`, `"ACE Slot 3"`, `"Slot 2"`, `"AMS 4"`, `"#1"`, or `"T0"`..`"T7"`.
  pub fn inferred_slot_index(&self, max_slots: usize) -> Option<usize> {
    let loc = self.location.trim().to_lowercase();
    if loc.is_empty() || max_slots == 0 {
      return None;
    }
    for tok in loc.split(|c: char| !c.is_alphanumeric()) {
      if let Some(t_idx) = tok.strip_prefix('t').and_then(|n| n.parse::<usize>().ok()) {
        if t_idx < max_slots {
          return Some(t_idx);
        }
      }
    }
    let has_slot_keyword = loc.contains("ace")
      || loc.contains("slot")
      || loc.contains("ams")
      || loc.contains("unit")
      || loc.starts_with('#');
    if has_slot_keyword {
      // Handle "ACE 2-1" or "AMS 2 Slot 1" -> unit 2, slot 1 -> index 4
      let nums: Vec<usize> = loc
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse::<usize>().ok())
        .collect();
      if nums.len() >= 2 && (1..=2).contains(&nums[0]) && (1..=4).contains(&nums[1]) {
        let idx = (nums[0] - 1) * 4 + (nums[1] - 1);
        if idx < max_slots {
          return Some(idx);
        }
      }
      if let Some(&n) = nums.last() {
        if (1..=max_slots).contains(&n) {
          return Some(n - 1);
        }
      }
    }
    None
  }

  /// Converts this `SpoolmanSpool` into a configured `FilamentSpool` for the active nozzle type.
  pub fn to_filament_spool(&self, active_nozzle_type: &str) -> FilamentSpool {
    let c = crate::constants::constants();
    let catalog = crate::catalog::load_merged_catalog();
    let combined_text = format!(
      "{} {} {}",
      self.vendor_name, self.material, self.filament_name
    );
    let raw_poly =
      c.detect_polymer_from_profile_texts(&combined_text, Some(self.material.as_str()), None);
    let base_mat = c.base_material_code(&raw_poly).to_string();
    let fc = c.detect_finish_from_texts(&self.filament_name, &self.material, &raw_poly, &base_mat);
    let finish_code = fc.code.clone();
    let norm_brand = FilamentSpool::normalized_brand(&self.vendor_name);

    let mut spool = if let Some(cat_entry) = catalog
      .filaments
      .iter()
      .find(|f| f.brand.eq_ignore_ascii_case(&norm_brand) && f.code.eq_ignore_ascii_case(&base_mat))
      .or_else(|| {
        catalog
          .filaments
          .iter()
          .find(|f| f.code.eq_ignore_ascii_case(&base_mat))
      })
      .or_else(|| catalog.filaments.first())
    {
      FilamentSpool::from_catalog_entry(self.spool_id as usize, cat_entry, active_nozzle_type)
    } else {
      let fallback = &catalog.filaments[0];
      FilamentSpool::from_catalog_entry(self.spool_id as usize, fallback, active_nozzle_type)
    };

    spool.id = format!("spoolman_{}_{}", self.spool_id, self.filament_id);
    spool.brand = norm_brand.clone();
    spool.apply_material_change(&base_mat, active_nozzle_type);
    spool.apply_finish_change(&finish_code, active_nozzle_type);
    spool.color_hex = self.color_hex.clone();
    spool.color_name = if !self.filament_name.is_empty() {
      format!("{} (#{})", self.filament_name, self.spool_id)
    } else {
      format!("Spool #{}", self.spool_id)
    };
    spool.remaining_grams = self.remaining_weight_g;
    spool.is_rfid_synced = true;

    if let Some(ext_c) = self.extruder_temp_c {
      spool.brass_base_temp = ext_c;
      spool.hs_base_temp = (ext_c + 5).clamp(80, 420);
      spool.recompute_for_nozzle(active_nozzle_type);
    }
    if let Some(bed_c) = self.bed_temp_c {
      let (min_b, max_b) = FilamentSpool::polymer_safe_bed_bounds(&spool.material_type);
      let clamped_b = bed_c.clamp(min_b, max_b);
      spool.base_bed_temp = clamped_b;
      spool.bed_temp = clamped_b;
    }
    if let Some(fr) = self.extra_flow_ratio {
      if (0.40..=1.25).contains(&fr) {
        spool.flow_ratio = fr;
      }
    }
    if let Some(pa) = self.extra_pressure_advance {
      if (0.0..=0.25).contains(&pa) {
        spool.pressure_advance = pa;
      }
    }
    if let Some(mvs) = self.extra_max_volumetric_speed {
      if (1.0..=50.0).contains(&mvs) {
        spool.max_volumetric_speed = mvs;
      }
    }

    spool.name = FilamentSpool::default_oc_profile_name_with_brand(
      &spool.brand,
      &spool.material_type,
      &spool.finish_type,
      active_nozzle_type,
    );
    spool.rebuild_synthetic_disk_json(active_nozzle_type);
    spool
  }

  /// Exports the active filament loadout and Spoolman inventory as a standard Spoolman `/api/v1/spool`-compatible JSON array
  /// so Static Website users can save, share, or re-import their Spoolman inventory + calibrated Flow/PA/MVS values.
  pub fn export_as_spoolman_api_json(
    active_spools: &[FilamentSpool],
    inventory: &[SpoolmanSpool],
  ) -> String {
    let mut items = Vec::new();
    for (idx, sp) in active_spools.iter().enumerate() {
      let (sid, fid) = sp
        .spoolman_ids()
        .unwrap_or(((idx + 1) as u32, (idx + 1) as u32));
      let inv_match = inventory.iter().find(|i| i.spool_id == sid);
      let loc = inv_match
        .map(|i| i.location.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("ACE Slot {}", idx + 1));
      let lot = inv_match.map(|i| i.lot_nr.clone()).unwrap_or_default();
      items.push(serde_json::json!({
        "id": sid,
        "remaining_weight": sp.remaining_grams,
        "initial_weight": 1000,
        "location": loc,
        "lot_nr": lot,
        "archived": false,
        "extra": {
          "flow_ratio": format!("\"{:.3}\"", sp.flow_ratio),
          "pressure_advance": format!("\"{:.3}\"", sp.pressure_advance),
          "max_volumetric_speed": format!("\"{:.1}\"", sp.max_volumetric_speed),
        },
        "filament": {
          "id": fid,
          "name": sp.color_name,
          "material": sp.material_type,
          "color_hex": sp.color_hex.trim_start_matches('#'),
          "settings_extruder_temp": sp.brass_base_temp,
          "settings_bed_temp": sp.bed_temp,
          "vendor": {
            "name": sp.brand
          },
          "extra": {
            "flow_ratio": format!("\"{:.3}\"", sp.flow_ratio),
            "pressure_advance": format!("\"{:.3}\"", sp.pressure_advance),
            "max_volumetric_speed": format!("\"{:.1}\"", sp.max_volumetric_speed),
          }
        }
      }));
    }
    for inv in inventory {
      if !active_spools
        .iter()
        .any(|sp| sp.spoolman_ids().map(|(s, _)| s) == Some(inv.spool_id))
      {
        items.push(serde_json::json!({
          "id": inv.spool_id,
          "remaining_weight": inv.remaining_weight_g,
          "initial_weight": inv.initial_weight_g,
          "location": inv.location,
          "lot_nr": inv.lot_nr,
          "comment": inv.comment,
          "archived": false,
          "extra": {
            "flow_ratio": inv.extra_flow_ratio.map(|v| format!("\"{:.3}\"", v)).unwrap_or_default(),
            "pressure_advance": inv.extra_pressure_advance.map(|v| format!("\"{:.3}\"", v)).unwrap_or_default(),
            "max_volumetric_speed": inv.extra_max_volumetric_speed.map(|v| format!("\"{:.1}\"", v)).unwrap_or_default(),
          },
          "filament": {
            "id": inv.filament_id,
            "name": inv.filament_name,
            "material": inv.material,
            "color_hex": inv.color_hex.trim_start_matches('#'),
            "settings_extruder_temp": inv.extruder_temp_c,
            "settings_bed_temp": inv.bed_temp_c,
            "vendor": {
              "name": inv.vendor_name
            }
          }
        }));
      }
    }
    serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_string())
  }
}

fn base64_decode_bytes(input: &str) -> Option<Vec<u8>> {
  const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let mut lut = [255u8; 256];
  for (i, &b) in TABLE.iter().enumerate() {
    lut[b as usize] = i as u8;
  }
  let clean: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
  if clean.is_empty() || clean.len() % 4 != 0 {
    return None;
  }
  let mut out = Vec::with_capacity((clean.len() / 4) * 3);
  for chunk in clean.chunks(4) {
    let a = lut[chunk[0] as usize];
    let b = lut[chunk[1] as usize];
    if a == 255 || b == 255 {
      return None;
    }
    let c = if chunk[2] == b'=' {
      0
    } else {
      let v = lut[chunk[2] as usize];
      if v == 255 {
        return None;
      }
      v
    };
    let d = if chunk[3] == b'=' {
      0
    } else {
      let v = lut[chunk[3] as usize];
      if v == 255 {
        return None;
      }
      v
    };
    out.push((a << 2) | (b >> 4));
    if chunk[2] != b'=' {
      out.push((b << 4) | (c >> 2));
    }
    if chunk[3] != b'=' {
      out.push((c << 6) | d);
    }
  }
  Some(out)
}

fn base64_encode_bytes(input: &[u8]) -> String {
  const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let mut out = String::with_capacity(((input.len() + 2) / 3) * 4);
  for chunk in input.chunks(3) {
    let b0 = chunk[0];
    let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
    let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };
    out.push(TABLE[(b0 >> 2) as usize] as char);
    out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
    if chunk.len() > 1 {
      out.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
    } else {
      out.push('=');
    }
    if chunk.len() > 2 {
      out.push(TABLE[(b2 & 0x3f) as usize] as char);
    } else {
      out.push('=');
    }
  }
  out
}

/// Decodes AnycubicSlicerNext's 2-pass `(base64_decode -> byte - 5)` obfuscated string
/// used in `anycubic_remote_printing` (e.g. `current_device_id`).
pub fn decode_anycubic_obfuscated_str(encoded: &str) -> Option<String> {
  let pass1_raw = base64_decode_bytes(encoded.trim())?;
  let pass1_shifted: Vec<u8> = pass1_raw.into_iter().map(|b| b.wrapping_sub(5)).collect();
  let pass1_str = std::str::from_utf8(&pass1_shifted).ok()?;
  let pass2_raw = base64_decode_bytes(pass1_str)?;
  let pass2_shifted: Vec<u8> = pass2_raw.into_iter().map(|b| b.wrapping_sub(5)).collect();
  let plain = String::from_utf8(pass2_shifted).ok()?;
  Some(
    plain
      .trim()
      .trim_matches('"')
      .trim_matches('\'')
      .to_string(),
  )
}

/// Encodes a string using AnycubicSlicerNext's 2-pass `(byte + 5 -> base64_encode)` obfuscation.
pub fn encode_anycubic_obfuscated_str(plain: &str) -> String {
  let pass1: Vec<u8> = plain.bytes().map(|b| b.wrapping_add(5)).collect();
  let b64_1 = base64_encode_bytes(&pass1);
  let pass2: Vec<u8> = b64_1.bytes().map(|b| b.wrapping_add(5)).collect();
  base64_encode_bytes(&pass2)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnycubicAceSlotInfo {
  pub slot_index: usize,
  pub profile_name: String,
  pub material_type: String,
  pub color_hex: String,
  pub status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct AnycubicSniffedPrinterState {
  pub discovery_tier: String,
  pub device_id: String,
  pub printer_model: String,
  pub active_machine_preset: String,
  pub active_process_preset: String,
  pub ams_filament_ids: String,
  pub cloud_session_detected: bool,
  pub mqtt_broker_url: String,
  pub mqtt_connected_in_logs: bool,
  pub lan_rinkhals_reachable: bool,
  pub lan_endpoint: String,
  pub dryer_status: String,
  pub slots: Vec<AnycubicAceSlotInfo>,
}

impl AnycubicSniffedPrinterState {
  pub fn infer_material_from_profile_name(profile_name: &str) -> String {
    let up = profile_name.to_uppercase();
    for mat in [
      "PETG-CF", "PLA-CF", "ABS-CF", "PA-CF", "PET-CF", "PCTG", "PETG", "TPU", "ASA", "ABS", "PC",
      "PHA", "PVA", "PLA+", "PLA",
    ] {
      if up.contains(mat) {
        return mat.to_string();
      }
    }
    "PLA".to_string()
  }

  /// Tier 1 Auto-Sniff: Parses `AnycubicSlicerNext.conf` (stripping any trailing `# MD5 checksum` line)
  /// to extract the decoded hardware `current_device_id`, `ams_filament_ids`, `anycubic_cloud` session status,
  /// and active `anycubic_presets` ACE Pro slots & RGB hex colors.
  pub fn sniff_from_conf_str(conf_text: &str) -> Result<Self, String> {
    let cleaned: String = conf_text
      .lines()
      .filter(|l| !l.trim_start().starts_with('#'))
      .collect::<Vec<_>>()
      .join("\n");
    let val: serde_json::Value = serde_json::from_str(&cleaned)
      .map_err(|e| format!("Failed to parse AnycubicSlicerNext.conf JSON: {}", e))?;

    let encoded_dev_id = val
      .get("anycubic_remote_printing")
      .and_then(|rp| rp.get("current_device_id"))
      .and_then(|v| v.as_str())
      .unwrap_or("");
    let device_id = decode_anycubic_obfuscated_str(encoded_dev_id).unwrap_or_default();

    let mut ams_filament_ids = String::new();
    if let Some(ams_obj) = val.get("ams_filament_ids").and_then(|v| v.as_object()) {
      for (k, v) in ams_obj {
        let clean_k = k.trim_matches('"');
        if (!device_id.is_empty() && clean_k == device_id) || ams_filament_ids.is_empty() {
          if let Some(s) = v.as_str() {
            ams_filament_ids = s.to_string();
          }
        }
      }
    }

    let mut cloud_session_detected = false;
    if let Some(cloud_obj) = val.get("anycubic_cloud").and_then(|v| v.as_object()) {
      for (_k, v) in cloud_obj {
        if let Some(s) = v.as_str() {
          if s.len() > 64 {
            cloud_session_detected = true;
          }
        }
      }
    }

    let printer_model = val
      .get("firstguide")
      .and_then(|fg| fg.get("model"))
      .and_then(|v| v.as_str())
      .unwrap_or("Anycubic Kobra S1")
      .to_string();

    let mut active_machine_preset = val
      .get("presets")
      .and_then(|p| p.get("machine"))
      .and_then(|v| v.as_str())
      .unwrap_or("")
      .to_string();

    let mut active_process_preset = String::new();
    let mut slot_names: Vec<String> = Vec::new();
    let mut slot_colors: Vec<String> = Vec::new();

    if let Some(arr) = val.get("anycubic_presets").and_then(|a| a.as_array()) {
      let mut best_priority = 0usize;
      for preset in arr {
        let mach = preset.get("machine").and_then(|m| m.as_str()).unwrap_or("");
        if mach.contains(".3mf)") {
          continue;
        }
        let mut f_names = Vec::new();
        for key in [
          "filament",
          "filament_01",
          "filament_02",
          "filament_03",
          "filament_04",
          "filament_05",
          "filament_06",
          "filament_07",
        ] {
          if let Some(fnm) = preset.get(key).and_then(|v| v.as_str()) {
            let trimmed = fnm.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('(') {
              let clean_fnm = trimmed
                .split_once(".3mf)")
                .map(|(head, _)| head.rsplit_once('(').map(|(h, _)| h).unwrap_or(head))
                .unwrap_or(trimmed);
              f_names.push(clean_fnm.trim().to_string());
            }
          }
        }
        let oc_fil_count = f_names
          .iter()
          .filter(|n| n.starts_with("OC -") || n.starts_with("OC:"))
          .count();
        let is_exact_active = !active_machine_preset.is_empty() && mach == active_machine_preset;
        let priority = if oc_fil_count >= 2 && (mach.contains("Kobra S1") || mach.starts_with("OC"))
        {
          40 + oc_fil_count
        } else if is_exact_active && oc_fil_count > 0 {
          30 + oc_fil_count
        } else if is_exact_active {
          20
        } else if (mach.contains("Kobra S1") || mach.starts_with("OC")) && oc_fil_count > 0 {
          15 + oc_fil_count
        } else if mach.contains("Kobra S1") {
          10
        } else {
          0
        };
        if priority > best_priority && !f_names.is_empty() {
          best_priority = priority;
          if active_machine_preset.is_empty() {
            active_machine_preset = mach.to_string();
          }
          if let Some(proc_str) = preset.get("process").and_then(|p| p.as_str()) {
            active_process_preset = proc_str.to_string();
          }
          slot_names = f_names;
          if let Some(cols) = preset.get("filament_colors").and_then(|fc| fc.as_str()) {
            slot_colors = cols
              .split(',')
              .map(|s| s.trim().to_string())
              .filter(|s| !s.is_empty())
              .collect();
          }
        }
      }
    }

    let default_colors = [
      "#BC4033", "#658946", "#E10600", "#4B5563", "#8E9299", "#1A1A1A", "#8E9299", "#D4AF37",
    ];
    let mut slots = Vec::new();
    for (idx, prof_name) in slot_names.iter().enumerate() {
      let color_hex = slot_colors.get(idx).cloned().unwrap_or_else(|| {
        default_colors
          .get(idx)
          .copied()
          .unwrap_or("#3B82F6")
          .to_string()
      });
      slots.push(AnycubicAceSlotInfo {
        slot_index: idx + 1,
        profile_name: prof_name.clone(),
        material_type: Self::infer_material_from_profile_name(prof_name),
        color_hex,
        status: "Sniffed (.conf)".to_string(),
      });
    }

    Ok(Self {
      discovery_tier: "Tier 1: Local AnycubicSlicerNext.conf".to_string(),
      device_id,
      printer_model,
      active_machine_preset,
      active_process_preset,
      ams_filament_ids,
      cloud_session_detected,
      mqtt_broker_url: "mqtts://mqtt-universe.anycubic.com:8883".to_string(),
      mqtt_connected_in_logs: false,
      lan_rinkhals_reachable: false,
      lan_endpoint: String::new(),
      dryer_status: "Idle / Not Queried".to_string(),
      slots,
    })
  }

  /// Patches an `AnycubicSlicerNext.conf` string with updated slot filament names and RGB colors
  /// from the active WebConfig slots, preserving any trailing `# MD5 checksum` line.
  pub fn patch_conf_str(
    conf_text: &str,
    slot_profile_names: &[String],
    slot_colors_hex: &[String],
  ) -> Result<String, String> {
    let mut checksum_line = String::new();
    let mut body_lines = Vec::new();
    for line in conf_text.lines() {
      if line.trim_start().starts_with('#') {
        checksum_line = line.to_string();
      } else {
        body_lines.push(line);
      }
    }
    let cleaned = body_lines.join("\n");
    let mut val: serde_json::Value = serde_json::from_str(&cleaned)
      .map_err(|e| format!("Failed to parse AnycubicSlicerNext.conf JSON: {}", e))?;

    let colors_csv = slot_colors_hex
      .iter()
      .map(|c| {
        if c.starts_with('#') {
          c.to_uppercase()
        } else {
          format!("#{}", c.to_uppercase())
        }
      })
      .collect::<Vec<_>>()
      .join(",");

    let keys = [
      "filament",
      "filament_01",
      "filament_02",
      "filament_03",
      "filament_04",
      "filament_05",
      "filament_06",
      "filament_07",
    ];

    if let Some(arr) = val
      .get_mut("anycubic_presets")
      .and_then(|a| a.as_array_mut())
    {
      for preset in arr.iter_mut() {
        let mach = preset
          .get("machine")
          .and_then(|m| m.as_str())
          .unwrap_or("")
          .to_string();
        if mach.contains(".3mf)") {
          continue;
        }
        if mach.contains("Kobra S1") || mach.starts_with("OC") {
          if let Some(obj) = preset.as_object_mut() {
            for (i, key) in keys.iter().enumerate() {
              if let Some(p_name) = slot_profile_names.get(i) {
                obj.insert(
                  (*key).to_string(),
                  serde_json::Value::String(p_name.clone()),
                );
              } else {
                obj.remove(*key);
              }
            }
            if !colors_csv.is_empty() {
              obj.insert(
                "filament_colors".to_string(),
                serde_json::Value::String(colors_csv.clone()),
              );
            }
          }
        }
      }
    }

    let mut out = serde_json::to_string_pretty(&val)
      .map_err(|e| format!("Failed to serialize AnycubicSlicerNext.conf: {}", e))?;
    if !checksum_line.is_empty() {
      out.push('\n');
      out.push_str(&checksum_line);
    }
    out.push('\n');
    Ok(out)
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

fn default_spoolman_url() -> String {
  "http://127.0.0.1:7912".to_string()
}

fn default_moonraker_url() -> String {
  "http://192.168.1.100:7125".to_string()
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
  #[serde(default = "default_spoolman_url")]
  pub spoolman_url: String,
  #[serde(default)]
  pub spoolman_inventory: Vec<SpoolmanSpool>,
  #[serde(default)]
  pub moonraker_enabled: bool,
  #[serde(default = "default_moonraker_url")]
  pub moonraker_url: String,
  #[serde(default)]
  pub active_moonraker_spool_id: Option<u32>,
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
      let c = crate::constants::constants();
      let base_mat_for_sp = c.base_material_code(&sp.material_type).to_string();
      if base_mat_for_sp != sp.material_type {
        if let Some(inferred_fin) = c.detect_intrinsic_finish(&sp.material_type) {
          sp.finish_type = inferred_fin.to_string();
          sp.opacity = c.lookup_finish(inferred_fin).default_opacity;
        }
        sp.material_type = base_mat_for_sp;
      } else if !c.is_finish_compatible_with_polymer(&sp.finish_type, &sp.material_type)
        || (sp.material_type.eq_ignore_ascii_case("PC")
          && sp.finish_type == "Translucent"
          && c.detect_intrinsic_finish(&sp.name) != Some("Translucent"))
      {
        sp.finish_type = "Basic".to_string();
        sp.opacity = 1.0;
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
        sp.brand = c.detect_brand_from_text(&sp.name).to_string();
      }

      // Heal any scratch/catalog/OC spool in localStorage whose base temperature, print_temp, or translucent PETG tuning was corrupted or came from an older snapshot
      let is_oc_managed = sp.name.starts_with("OC -") || sp.name.starts_with("OC:");
      let is_foaming_or_esd = c.is_esd_or_foaming(&sp.material_type, &sp.finish_type);
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
              .find(|f| {
                f.brand.eq_ignore_ascii_case(&sp.brand)
                  && FilamentSpool::canonical_polymer_key(&f.code) == canon
              })
              .or_else(|| {
                catalog
                  .filaments
                  .iter()
                  .find(|f| FilamentSpool::canonical_polymer_key(&f.code) == canon)
              })
          })
        {
          let noz_cfg = c.lookup_nozzle(&sp.nozzle_type);
          let exp_brass = cat_entry.base_nozzle_c as i32;
          let exp_hs = if sp.material_type.eq_ignore_ascii_case("PLA+") && sp.brand == "Generic" {
            220
          } else {
            (exp_brass + 5).clamp(80, 420)
          };
          let exp_print_temp = if noz_cfg.uses_hs_base_temp {
            exp_hs
          } else {
            (exp_brass + noz_cfg.temp_offset_from_brass_c).max(80)
          };
          let poly_cfg = c.lookup_polymer(&sp.material_type);
          let needs_translucent_petg_heal = poly_cfg.translucent_z_hop_mm.is_some()
            && sp.finish_type == "Translucent"
            && (sp.bed_temp > 65 || sp.max_volumetric_speed > 10.0 || sp.fan_speed_percent > 35);
          let needs_warp_fan_heal = poly_cfg.is_warp_prone() && sp.fan_speed_percent > 35;
          let exp_flow = if cat_entry.default_flow_ratio
            >= c.hardware_limits.min_non_foaming_flow_ratio
            && cat_entry.default_flow_ratio <= c.hardware_limits.max_non_foaming_flow_ratio
            && (cat_entry.default_flow_ratio - 0.98).abs() > 0.005
          {
            cat_entry.default_flow_ratio
          } else {
            poly_cfg.default_flow_ratio
          };
          let exp_dry_c =
            if poly_cfg.translucent_z_hop_mm.is_some() && sp.finish_type == "Translucent" {
              65
            } else {
              cat_entry.dry_temp_c as i32
            };
          let needs_flow_or_dry_heal = !sp.is_foaming_tpu()
            && ((sp.flow_ratio - exp_flow).abs() > 0.005 || sp.drying_temp_c != exp_dry_c);
          let needs_temp_heal = !sp.is_foaming_tpu()
            && (sp.brass_base_temp != exp_brass || (sp.print_temp - exp_print_temp).abs() > 8);
          if needs_temp_heal
            || needs_flow_or_dry_heal
            || needs_translucent_petg_heal
            || needs_warp_fan_heal
          {
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
          if !poly_cfg.is_flexible()
            && poly_cfg.family != "SOLUBLE"
            && sp.max_volumetric_speed <= 2.5
          {
            sp.max_volumetric_speed = cat_entry.max_vol_mm3s.max(12.0);
          }
        }
      }

      if is_oc_managed {
        let (min_b, max_b) = FilamentSpool::polymer_safe_bed_bounds(&sp.material_type);
        sp.base_bed_temp = sp.base_bed_temp.clamp(min_b, max_b);
        sp.bed_temp = sp.bed_temp.clamp(min_b, max_b);
        if !sp.is_foaming_tpu() {
          let noz = sp.nozzle_type.clone();
          sp.recompute_for_nozzle(&noz);
          sp.rebuild_synthetic_disk_json(&noz);
        } else if let Some(ref raw) = sp.raw_disk_json {
          if let Ok(serde_json::Value::Object(mut rmap)) =
            serde_json::from_str::<serde_json::Value>(raw)
          {
            if rmap.remove("compatible_printers").is_some() {
              sp.raw_disk_json =
                serde_json::to_string_pretty(&serde_json::Value::Object(rmap)).ok();
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
        if let Some(canon) = canonical_procs
          .iter()
          .find(|dp| dp.id == proc.id || dp.name.split(" @").next() == proc.name.split(" @").next())
        {
          let was_corrupted = matches!(proc.outer_wall_speed.round() as i32, 51 | 151 | 162 | 238)
            || matches!(proc.inner_wall_speed.round() as i32, 128 | 151 | 162 | 238)
            || matches!(proc.sparse_infill_speed.round() as i32, 151 | 162 | 238)
            || (proc.initial_layer_speed - 18.0).abs() < 1.0
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

    for key in ["retract_lift_enforce", "filament_retract_lift_enforce"] {
      if let Some(val) = self.base_profile.values.get_mut(key) {
        let normalize = |s: &str| -> String {
          match s.trim() {
            "Top Surfaces Only" | "top_only" | "top" => "Top Only".to_string(),
            "Bottom Surfaces Only" | "bottom_only" | "bottom" => "Bottom Only".to_string(),
            "Top and Bottom Surfaces" | "top_and_bottom" => "Top and Bottom".to_string(),
            "All" | "all" | "all_surfaces" => "All Surfaces".to_string(),
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

    for fil in &mut self.filaments {
      if let Some(ref mut raw_json) = fil.raw_disk_json {
        if raw_json.contains("Top Surfaces Only") || raw_json.contains("Bottom Surfaces Only") {
          *raw_json = raw_json
            .replace("\"Top Surfaces Only\"", "\"Top Only\"")
            .replace("\"Bottom Surfaces Only\"", "\"Bottom Only\"");
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
        .replace("= \"Normal\"", "= \"Normal Lift\"")
        .replace("\"Top Surfaces Only\"", "\"Top Only\"")
        .replace("\"Bottom Surfaces Only\"", "\"Bottom Only\"");
    }

    for default_rule in crate::transformations::TransformationEngine::get_default_rules() {
      if let Some(existing_rule) = self.rules.iter_mut().find(|r| r.id == default_rule.id) {
        let should_refresh = match existing_rule.id.as_str() {
          "sym_wet_filament" => !existing_rule.lua_code.contains("wet_retract_add"),
          "weather_climate_adjustment"
          | "ironing_tuning"
          | "thermal_conductivity"
          | "rig_addons_cooling"
          | "anycubic_pctg_as_petg"
          | "volumetric_speed_ceiling"
          | "flush_volumes_matrix"
          | "symptom_stringing_diamondback"
          | "symptom_clog_ace_pro"
          | "symptom_bed_adhesion_petg"
          | "symptom_spaghetti_pla"
          | "process_scarf_seams_arachne"
          | "filament_pctg_antistringing" => true,
          _ => false,
        };
        if should_refresh {
          existing_rule.name = default_rule.name;
          existing_rule.description = default_rule.description;
          existing_rule.lua_code = default_rule.lua_code;
        }
      } else {
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
    self.generate_descriptive_notes_with_effective_values(&serde_json::Map::new())
  }

  /// Returns true when `notes` is empty or was auto-generated by WebConfig (not hand-written by the user).
  pub fn has_auto_generated_notes(&self) -> bool {
    let n = self.notes.trim();
    n.is_empty() || n.starts_with("[OrcaSlicer WebConfig v")
  }

  /// Generate `notes` reflecting the exact exported values in `effective` (after the Lua stack,
  /// VFA outer-wall step-down, user speed scales, etc.), falling back to this profile's own fields.
  pub fn generate_descriptive_notes_with_effective_values(
    &self,
    effective: &serde_json::Map<String, serde_json::Value>,
  ) -> String {
    let tier_label = match self.quality_tier() {
      "high_quality" => "Quality (Fine Surface Detail & Overhang Precision)",
      "high_speed" => "Speed (Rapid Prototyping Capped by Volumetric Speed Ceiling)",
      _ => "Standard (Balanced Structural Strength & Throughput)",
    };
    let num = |key: &str, fallback: f64| -> f64 {
      effective
        .get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| s.trim().trim_end_matches('%').parse::<f64>().ok())
        .unwrap_or(fallback)
    };
    let text = |key: &str, fallback: &str| -> String {
      effective
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
    };
    format!(
      "[OrcaSlicer WebConfig v{} — Process: {}] Layer Height: {:.2}mm, Line Width: {:.2}mm. Speeds: {:.0} mm/s Outer Wall, {:.0} mm/s Inner Wall, {:.0} mm/s Sparse Infill, {:.0} mm/s Initial Layer. Default Accel: {:.0} mm/s². Wall Generator: '{}', Print Sequence: '{}'. Protected by Lua Volumetric Speed Ceiling & Non-Crossing Gyroid/Z-Hop rules.",
      WEBCONFIG_APP_VERSION,
      tier_label,
      num("layer_height", self.layer_height),
      num("line_width", self.line_width),
      num("outer_wall_speed", self.outer_wall_speed),
      num("inner_wall_speed", self.inner_wall_speed),
      num("sparse_infill_speed", self.sparse_infill_speed),
      num("initial_layer_speed", self.initial_layer_speed),
      num("default_acceleration", self.default_acceleration),
      text("wall_generator", &self.wall_generator),
      text("print_sequence", &self.print_sequence)
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

  #[test]
  fn test_spoolman_inventory_parse_and_slot_mapping_roundtrip() {
    let raw_api_json = r##"[
      {
        "id": 14,
        "remaining_weight": 742.4,
        "initial_weight": 1000.0,
        "location": "ACE Slot 2",
        "lot_nr": "LOT-2026-09",
        "archived": false,
        "extra": {
          "flow_ratio": "\"0.945\"",
          "pressure_advance": "\"0.032\"",
          "max_volumetric_speed": "\"19.0\""
        },
        "filament": {
          "id": 105,
          "name": "Rapid PETG Cyan",
          "material": "PETG",
          "color_hex": "06b6d4",
          "settings_extruder_temp": 235,
          "settings_bed_temp": 75,
          "vendor": {
            "name": "eSUN"
          }
        }
      },
      {
        "id": 22,
        "remaining_weight": 85.0,
        "initial_weight": 1000.0,
        "location": "ACE 2-1",
        "archived": false,
        "filament": {
          "id": 208,
          "name": "Carbon PLA-CF",
          "material": "PLA-CF",
          "color_hex": "#1E293B",
          "settings_extruder_temp": 220,
          "settings_bed_temp": 60,
          "vendor": {
            "name": "Bambu Lab"
          }
        }
      }
    ]"##;

    let parsed = SpoolmanSpool::parse_inventory_json(raw_api_json).expect("parses Spoolman JSON");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].spool_id, 14);
    assert_eq!(parsed[0].filament_id, 105);
    assert_eq!(parsed[0].vendor_name, "eSUN");
    assert_eq!(parsed[0].color_hex, "#06B6D4");
    assert_eq!(parsed[0].remaining_weight_g, 742);
    assert_eq!(parsed[0].inferred_slot_index(8), Some(1)); // ACE Slot 2 -> 0-based index 1
    assert_eq!(parsed[1].inferred_slot_index(8), Some(4)); // ACE 2-1 -> unit 2, slot 1 -> index 4

    let spool = parsed[0].to_filament_spool("hardened_steel");
    assert_eq!(spool.spoolman_ids(), Some((14, 105)));
    assert_eq!(spool.material_type, "PETG");
    assert_eq!(spool.remaining_grams, 742);
    assert_eq!(spool.brass_base_temp, 235);
    assert_eq!(spool.print_temp, 240); // +5°C Hardened Steel offset
    assert_eq!(spool.bed_temp, 75);
    assert!((spool.flow_ratio - 0.945).abs() < 1e-4);
    assert!((spool.pressure_advance - 0.032).abs() < 1e-4);
    assert!((spool.max_volumetric_speed - 19.0).abs() < 1e-4);

    let exported = SpoolmanSpool::export_as_spoolman_api_json(&[spool], &parsed);
    let reparsed =
      SpoolmanSpool::parse_inventory_json(&exported).expect("reparses exported Spoolman JSON");
    assert_eq!(reparsed.len(), 2);
    assert_eq!(reparsed[0].spool_id, 14);
  }

  #[test]
  fn test_anycubic_obfuscation_and_conf_auto_sniff_roundtrip() {
    let encoded_dev_id = "T390PFNZXjdmb2lzZlx4MGZ/eXZTbzpzZjdeO2ZJOndVSV48X290PmZcXjdPfEJC";
    let decoded = decode_anycubic_obfuscated_str(encoded_dev_id).expect("decodes device_id");
    assert_eq!(decoded, "56011e2bdf9f6e19bfa5c9f716a58da1");

    let re_encoded = encode_anycubic_obfuscated_str("\"56011e2bdf9f6e19bfa5c9f716a58da1\"");
    assert_eq!(re_encoded, encoded_dev_id);

    let sample_conf = r##"{
  "ams_filament_ids": {
    "\"56011e2bdf9f6e19bfa5c9f716a58da1\"": "1,2,3,4,1,2,4"
  },
  "anycubic_cloud": {
    "l1qde7GcMLvhaKcXOcWRLA==": "QfEMBe5x6jnnsY9cu5cFGS3qcz8aOxKzqWqKJicpotZtWQu4wAz5NoXuUH/rkvf9fJvXIA7ABCDEFGHIJKLMNOPQRSTUVWXYZ"
  },
  "anycubic_presets": [
    {
      "filament": "OC - PETG-CF • 0.4 Hardened",
      "filament_01": "OC - PCTG • 0.4 Hardened",
      "filament_02": "OC - TPU for AMS • 0.4 Hardened",
      "filament_03": "Anycubic PLA-CF @Anycubic Kobra S1 0.4 nozzle",
      "filament_colors": "#BC4033,#658946,#E10600,#4B5563",
      "machine": "OC - Kobra S1 • 0.4 Hardened",
      "process": "OC - 0.20mm Standard @Kobra S1"
    }
  ],
  "anycubic_remote_printing": {
    "current_device_id": "T390PFNZXjdmb2lzZlx4MGZ/eXZTbzpzZjdeO2ZJOndVSV48X290PmZcXjdPfEJC"
  },
  "firstguide": {
    "model": "Anycubic Kobra S1"
  },
  "presets": {
    "machine": "OC - Kobra S1 • 0.4 Hardened"
  }
}
# MD5 checksum 78EEA34537C082A7933147457EA67CA5
"##;

    let sniffed = AnycubicSniffedPrinterState::sniff_from_conf_str(sample_conf)
      .expect("sniffs AnycubicSlicerNext.conf");
    assert_eq!(sniffed.device_id, "56011e2bdf9f6e19bfa5c9f716a58da1");
    assert_eq!(sniffed.ams_filament_ids, "1,2,3,4,1,2,4");
    assert!(sniffed.cloud_session_detected);
    assert_eq!(sniffed.printer_model, "Anycubic Kobra S1");
    assert_eq!(sniffed.slots.len(), 4);
    assert_eq!(sniffed.slots[0].material_type, "PETG-CF");
    assert_eq!(sniffed.slots[0].color_hex, "#BC4033");
    assert_eq!(sniffed.slots[1].material_type, "PCTG");

    let patched = AnycubicSniffedPrinterState::patch_conf_str(
      sample_conf,
      &[
        "OC - ASA • 0.4 Hardened".to_string(),
        "OC - PLA+ • 0.4 Hardened".to_string(),
      ],
      &["#112233".to_string(), "#445566".to_string()],
    )
    .expect("patches conf");
    assert!(patched.contains("# MD5 checksum 78EEA34537C082A7933147457EA67CA5"));
    let resniffed =
      AnycubicSniffedPrinterState::sniff_from_conf_str(&patched).expect("re-sniffs patched conf");
    assert_eq!(resniffed.slots.len(), 2);
    assert_eq!(resniffed.slots[0].material_type, "ASA");
    assert_eq!(resniffed.slots[0].color_hex, "#112233");
  }
}
