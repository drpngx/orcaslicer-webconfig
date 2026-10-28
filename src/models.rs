use serde::{Deserialize, Serialize};

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
}

fn default_nozzle_hrc() -> u32 {
  55
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

  /// Concise, easy-to-read title prefixed with `OC:` highlighting the big hardware traits (without ACE8 since AMS/ACE is configured separately):
  /// e.g. `OC: Kobra S1 • 0.4 Hardened` or `OC: Kobra S1 • 0.4 Diamondback +FlowTech`
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
      "OC: {} • {} {}{}",
      model, self.nozzle_diameter, noz, addon_tag
    )
  }

  /// Returns `(orca_nozzle_type, nozzle_hrc)` for the Machine OrcaSlicer JSON profile:
  /// - Hardened Steel (`hardened_steel` / `cm2`) -> `("hardened_steel", 55)`
  /// - Ruby Tip (Brass Body) (`ruby_brass`) -> `("brass", 60)` (uses Brass thermal profile + HRC 60 so CF/GF composites are allowed!)
  /// - Diamondback PCD (`diamondback`) -> `("brass", 68)` (uses Brass/high-conductivity thermal profile + HRC 68 for extreme wear resistance!)
  /// - Plated Brass (`microswiss_brass`) -> `("brass", 35)`
  /// - Standard Brass (`brass`) -> `("brass", 0)`
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
      ("brass", 0)
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
    format!(
      "[OrcaSlicer WebConfig Machine] {} ({}, {}). Nozzle: {}mm {} [Orca nozzle_type={}, nozzle_hrc={}] ({}; Max Flow: {}, Retract: {:.2}mm, Melt-Zone Mult: {:.2}x). Add-Ons: {}.",
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
      addons_note
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
      name: "OC: Kobra S1 • 0.4 Hardened".to_string(),
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
    };
    tpl_k3.refresh_title_and_notes(true);

    vec![Self::kobra_s1_hardened_steel_png(), tpl_diamond, tpl_k3]
  }
}

impl FilamentSpool {
  /// Concise, scannable `OC:` display name for the filament spool including its nozzle type tag
  pub fn format_concise_title(&self) -> String {
    let base_name = if let Some(rest) = self.name.strip_prefix("OC: ") {
      rest
    } else if let Some(rest) = self.name.strip_prefix("OC:") {
      rest.trim()
    } else {
      self.name.as_str()
    };
    format!("OC: {}", base_name)
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
    self.nozzle_type = new_nozzle_type.to_string();

    if nt.contains("hardened") || nt.contains("stainless") || nt.contains("cm2") {
      self.base_print_temp = hs;
      // Only preserve a custom Hardened Steel disk override if we are on a Hardened Steel nozzle
      if self.tuned_for_nozzle_type.contains("hardened") && self.print_temp > hs {
        // Keep user's HS override
      } else {
        self.print_temp = hs;
      }
    } else if nt.contains("diamondback") {
      self.base_print_temp = brass;
      self.print_temp = (brass - 10).max(180);
    } else {
      // Standard Brass, Ruby Tip (Brass Body), MicroSwiss Plated Brass, or CHT:
      // Use the uncompensated Brass baseline (NEVER the Hardened Steel _HS temperature!)
      self.base_print_temp = brass;
      self.print_temp = brass;
    }
  }

  /// Returns a compatibility warning if:
  /// 1. The filament is abrasive (`required_nozzle_hrc > rig_nozzle_hrc`, e.g. `-CF`/`-GF` on a Brass `HRC 0` nozzle), OR
  /// 2. The filament had a disk override tuned for Hardened Steel (`_HS`) and the active rig is using a different nozzle type.
  pub fn compatibility_warning(
    &self,
    rig_nozzle_type: &str,
    rig_nozzle_hrc: u32,
  ) -> Option<String> {
    if self.required_nozzle_hrc > rig_nozzle_hrc {
      return Some(format!(
        "Incompatible Nozzle Hardness: {} requires HRC ≥ {} (Active nozzle is HRC {}). Use Hardened Steel (HRC 55), Ruby-Brass (HRC 60), or Diamondback (HRC 68).",
        self.material_type, self.required_nozzle_hrc, rig_nozzle_hrc
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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebConfigStateSnapshot {
  pub schema_version: String,
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
    for sp in &mut self.filaments {
      sp.deltas_from_base.retain(|d| {
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

    let stock = crate::orca_profile::OrcaProfile::get_stock_kobra_s1();
    for (k, v) in stock.values {
      self.base_profile.values.entry(k).or_insert(v);
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
    if lower.contains("high quality") || lower.contains("hq") || lower.contains("fine") {
      "high_quality"
    } else if lower.contains("high speed")
      || lower.contains("_hs_")
      || lower.contains("speed")
      || lower.contains("draft")
      || lower.contains("fast")
      || lower.contains("sport")
    {
      "high_speed"
    } else {
      "standard"
    }
  }

  /// Concise, easy-to-read title prefixed with `OC:` highlighting the big process traits without being too long:
  /// e.g. `OC: 0.20mm HQ • 60/180 • Arachne • By-Obj @KS1`
  pub fn format_concise_title(&self, short_machine: &str) -> String {
    let tier_short = match self.quality_tier() {
      "high_quality" => "HQ",
      "high_speed" => "Speed",
      _ => "Std",
    };
    let wall_tag = if self.wall_generator.eq_ignore_ascii_case("arachne") {
      "Arachne"
    } else {
      "Classic"
    };
    let seq_tag = if self.print_sequence.eq_ignore_ascii_case("by object") {
      " • By-Obj"
    } else {
      ""
    };
    format!(
      "OC: {:.2}mm {} • {:.0}/{:.0} • {}{} @{}",
      self.layer_height,
      tier_short,
      self.outer_wall_speed,
      self.inner_wall_speed,
      wall_tag,
      seq_tag,
      short_machine
    )
  }

  /// Generate a full descriptive note (`notes`) summarizing the process tier, speeds, acceleration, and slicing strategy
  pub fn generate_descriptive_notes(&self) -> String {
    let tier_label = match self.quality_tier() {
      "high_quality" => "High Quality (Fine Surface Detail & Overhang Precision)",
      "high_speed" => "High Speed (Rapid Prototyping Capped by Volumetric Speed Ceiling)",
      _ => "Standard (Balanced Structural Strength & Throughput)",
    };
    format!(
      "[OrcaSlicer WebConfig Process — {}] Layer Height: {:.2}mm, Line Width: {:.2}mm. Speeds: {:.0} mm/s Outer Wall, {:.0} mm/s Inner Wall, {:.0} mm/s Sparse Infill, {:.0} mm/s Initial Layer. Default Accel: {:.0} mm/s². Wall Generator: '{}', Print Sequence: '{}'. Protected by Lua Volumetric Speed Ceiling & Non-Crossing Gyroid/Z-Hop rules.",
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
    // 1. Anycubic's 0.20mm High Quality (Best speed-vs-surface-finish tradeoff: 0.20mm layer, 60 mm/s outer wall, 150 mm/s inner wall, 250 mm/s infill)
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
    hq_020.name = hq_020.format_concise_title("KS1");
    hq_020.notes = hq_020.generate_descriptive_notes();

    // 2. Anycubic's 0.12mm Fine High Quality (Ultra-fine vertical detail)
    let mut hq_012 = ProcessProfile {
      id: "proc_012_hq_png".to_string(),
      name: String::new(),
      inherits: "0.12mm High Quality @Anycubic Kobra S1 0.4 nozzle".to_string(),
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
    hq_012.name = hq_012.format_concise_title("KS1");
    hq_012.notes = hq_012.generate_descriptive_notes();

    // 3. Anycubic's 0.20mm Standard
    let mut std_proc = ProcessProfile {
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
    std_proc.name = std_proc.format_concise_title("KS1");
    std_proc.notes = std_proc.generate_descriptive_notes();

    // 4. 0.20mm High Speed
    let mut hs = ProcessProfile {
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
    hs.name = hs.format_concise_title("KS1");
    hs.notes = hs.generate_descriptive_notes();

    vec![hq_020, hq_012, std_proc, hs]
  }
}
