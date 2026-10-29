use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrcaProfile {
  pub name: String,
  pub profile_type: String, // "machine", "filament", "process", "combined"
  #[serde(skip_serializing_if = "Option::is_none")]
  pub inherits: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub from: Option<String>,
  pub values: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterMeta {
  pub key: String,
  pub label: String,
  pub category: String,
  pub unit: String,
  pub description: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WizardActionOp {
  AddOffset,     // e.g. +5 or -5 (adds delta to current numeric value)
  MultiplyScale, // e.g. * 0.95 or * 1.15 (scales current numeric value)
  SetValue,      // e.g. = "arachne" or = "245" or = "1" or = "10%"
}

impl WizardActionOp {
  pub fn label(&self) -> &'static str {
    match self {
      WizardActionOp::AddOffset => "Add / Offset (+ / -)",
      WizardActionOp::MultiplyScale => "Scale / Multiply (×)",
      WizardActionOp::SetValue => "Set Value (=)",
    }
  }

  pub fn short_symbol(&self) -> &'static str {
    match self {
      WizardActionOp::AddOffset => "+/-",
      WizardActionOp::MultiplyScale => "×",
      WizardActionOp::SetValue => "=",
    }
  }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SettingValueKind {
  Numeric {
    default_op: WizardActionOp,
    presets: Vec<String>,
    suggested: String,
  },
  EnumChoices {
    options: Vec<String>,
    suggested: String,
  },
  BooleanFlag {
    suggested_enabled: bool,
  },
  Percentage {
    presets: Vec<String>,
    suggested: String,
  },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WizardSettingDef {
  pub key: String,
  pub label: String,
  pub section: String,
  pub unit: String,
  pub description: String,
  pub default_base_value: String,
  pub is_array_in_orca: bool,
  pub decimals: usize,
  pub kind: SettingValueKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WizardRuleAction {
  pub key: String,
  pub label: String,
  pub section: String,
  pub unit: String,
  pub is_array_in_orca: bool,
  pub decimals: usize,
  pub default_base_value: String,
  pub op: WizardActionOp,
  pub operand: String,
}

impl WizardRuleAction {
  pub fn human_summary(&self) -> String {
    let unit_str = if self.unit.is_empty() || self.unit == "flag" {
      String::new()
    } else {
      format!(" {}", self.unit)
    };
    match self.op {
      WizardActionOp::AddOffset => {
        let raw = self.operand.trim().trim_start_matches('+');
        let delta = raw.parse::<f64>().unwrap_or(0.0);
        let sign = if delta >= 0.0 { "+" } else { "" };
        format!(
          "{} ({}): {}{:.prec$}{}",
          self.label,
          self.key,
          sign,
          delta,
          unit_str,
          prec = self.decimals
        )
      }
      WizardActionOp::MultiplyScale => {
        let factor = self
          .operand
          .trim()
          .trim_start_matches(['x', '×', '*'])
          .trim()
          .parse::<f64>()
          .unwrap_or(1.0);
        format!("{} ({}): × {:.2}", self.label, self.key, factor)
      }
      WizardActionOp::SetValue => {
        if self.unit == "flag" {
          let state_lbl = if self.operand == "1" || self.operand.eq_ignore_ascii_case("true") {
            "Enabled (1)"
          } else {
            "Disabled (0)"
          };
          format!("{} ({}): = {}", self.label, self.key, state_lbl)
        } else {
          format!(
            "{} ({}): = {}{}",
            self.label, self.key, self.operand, unit_str
          )
        }
      }
    }
  }

  pub fn to_lua_lines(&self) -> String {
    let fmt_spec = if self.decimals == 0 {
      "%d".to_string()
    } else {
      format!("%.{}f", self.decimals)
    };
    let fallback_num = self
      .default_base_value
      .trim_end_matches('%')
      .parse::<f64>()
      .unwrap_or(0.0);

    match self.op {
      WizardActionOp::SetValue => {
        let escaped = self.operand.replace('"', "\\\"");
        if self.is_array_in_orca {
          format!("  config[\"{}\"] = {{ \"{}\" }}", self.key, escaped)
        } else {
          format!("  config[\"{}\"] = \"{}\"", self.key, escaped)
        }
      }
      WizardActionOp::AddOffset => {
        let delta = self
          .operand
          .trim()
          .trim_start_matches('+')
          .parse::<f64>()
          .unwrap_or(0.0);
        if self.is_array_in_orca {
          format!(
            "  if config[\"{k}\"] and config[\"{k}\"][1] then\n    local cur = tonumber(config[\"{k}\"][1]) or {fb}\n    config[\"{k}\"] = {{ string.format(\"{fmt}\", cur + ({d})) }}\n  else\n    config[\"{k}\"] = {{ string.format(\"{fmt}\", {fb} + ({d})) }}\n  end",
            k = self.key,
            fb = fallback_num,
            fmt = fmt_spec,
            d = delta
          )
        } else {
          format!(
            "  local cur_{k} = tonumber(config[\"{k}\"]) or {fb}\n  config[\"{k}\"] = string.format(\"{fmt}\", cur_{k} + ({d}))",
            k = self.key,
            fb = fallback_num,
            fmt = fmt_spec,
            d = delta
          )
        }
      }
      WizardActionOp::MultiplyScale => {
        let factor = self
          .operand
          .trim()
          .trim_start_matches(['x', '×', '*'])
          .trim()
          .parse::<f64>()
          .unwrap_or(1.0);
        if self.is_array_in_orca {
          format!(
            "  if config[\"{k}\"] and config[\"{k}\"][1] then\n    local cur = tonumber(config[\"{k}\"][1]) or {fb}\n    config[\"{k}\"] = {{ string.format(\"{fmt}\", cur * {f}) }}\n  else\n    config[\"{k}\"] = {{ string.format(\"{fmt}\", {fb} * {f}) }}\n  end",
            k = self.key,
            fb = fallback_num,
            fmt = fmt_spec,
            f = factor
          )
        } else {
          format!(
            "  local cur_{k} = tonumber(config[\"{k}\"]) or {fb}\n  config[\"{k}\"] = string.format(\"{fmt}\", cur_{k} * {f})",
            k = self.key,
            fb = fallback_num,
            fmt = fmt_spec,
            f = factor
          )
        }
      }
    }
  }

  pub fn generate_lua_script(actions: &[WizardRuleAction], rule_title: &str) -> String {
    if actions.is_empty() {
      return "-- Add one or more setting actions in the Simple Rule Wizard above\nfunction apply(config, ctx)\nend\n".to_string();
    }
    let mut lines = Vec::new();
    lines.push(format!(
      "-- Visual Rule Wizard: {}",
      rule_title.replace('\n', " ")
    ));
    for a in actions {
      lines.push(format!("--   • {}", a.human_summary()));
    }
    lines.push("function apply(config, ctx)".to_string());
    for a in actions {
      lines.push(a.to_lua_lines());
    }
    let log_summary: Vec<String> = actions.iter().map(|a| a.human_summary()).collect();
    lines.push(format!(
      "  print(\"[WizardRule] {}\")",
      log_summary.join("; ").replace('"', "'")
    ));
    lines.push("end\n".to_string());
    lines.join("\n")
  }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiffItem {
  pub key: String,
  pub label: String,
  pub category: String,
  pub unit: String,
  pub base_value: String,
  pub override_value: String,
  pub source_rule: String,
  pub is_modified: bool,
}

impl OrcaProfile {
  pub fn new(name: &str, profile_type: &str) -> Self {
    Self {
      name: name.to_string(),
      profile_type: profile_type.to_string(),
      inherits: None,
      from: Some("User".to_string()),
      values: serde_json::Map::new(),
    }
  }

  pub fn from_json_str(json_str: &str) -> Result<Self, String> {
    let val: serde_json::Value = serde_json::from_str(json_str)
      .map_err(|e| format!("Failed to parse Orca profile JSON: {}", e))?;

    if let serde_json::Value::Object(map) = val {
      let name = map
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Custom Profile")
        .to_string();
      let profile_type = map
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("machine")
        .to_string();
      let inherits = map
        .get("inherits")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
      let from = map
        .get("from")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

      Ok(Self {
        name,
        profile_type,
        inherits,
        from,
        values: map,
      })
    } else {
      Err("Expected JSON Object for profile".to_string())
    }
  }

  pub fn to_json_pretty(&self) -> String {
    serde_json::to_string_pretty(&self.values).unwrap_or_else(|_| "{}".to_string())
  }

  pub fn get_str(&self, key: &str) -> Option<String> {
    self.values.get(key).and_then(|v| {
      if let Some(s) = v.as_str() {
        Some(s.to_string())
      } else if let Some(arr) = v.as_array() {
        arr
          .first()
          .and_then(|first| first.as_str())
          .map(|s| s.to_string())
      } else {
        Some(v.to_string())
      }
    })
  }

  pub fn set_str(&mut self, key: &str, val: &str) {
    self
      .values
      .insert(key.to_string(), serde_json::Value::String(val.to_string()));
  }

  pub fn set_array_str(&mut self, key: &str, val: &str) {
    self.values.insert(
      key.to_string(),
      serde_json::Value::Array(vec![serde_json::Value::String(val.to_string())]),
    );
  }

  pub fn get_stock_kobra_s1() -> Self {
    let json_data = serde_json::json!({
      "name": "Anycubic Kobra S1 - Stock Volcano V6 Nozzle",
      "type": "machine",
      "from": "User",
      "inherits": "Anycubic Kobra S1 0.4 nozzle",
      "printer_model": "Anycubic Kobra S1",
      "printer_variant": "0.4",
      "nozzle_diameter": ["0.4"],
      "nozzle_type": "brass",
      "nozzle_hrc": "0",
      "printable_area": ["0x0", "250x0", "250x250", "0x250"],
      "printable_height": "260",
      "filament_type": ["PETG"],
      "nozzle_temperature": ["240"],
      "nozzle_temperature_initial_layer": ["245"],
      "hot_plate_temp": ["75"],
      "hot_plate_temp_initial_layer": ["80"],
      "filament_flow_ratio": ["1.00"],
      "filament_max_volumetric_speed": ["15.0"],
      "filament_retraction_length": ["2.00"],
      "filament_retraction_speed": ["35"],
      "filament_deretraction_speed": ["30"],
      "filament_retract_before_wipe": ["0%"],
      "filament_wipe_distance": ["1.0"],
      "pressure_advance": ["0.040"],
      "layer_height": "0.20",
      "initial_layer_print_height": "0.24",
      "initial_layer_speed": "35",
      "initial_layer_line_width": "0.50",
      "outer_wall_line_width": "0.42",
      "outer_wall_speed": "200",
      "inner_wall_speed": "300",
      "sparse_infill_speed": "350",
      "sparse_infill_pattern": "grid",
      "sparse_infill_density": "15%",
      "wall_loops": "2",
      "wall_generator": "classic",
      "wall_transition_angle": "10",
      "wall_transition_length": "100%",
      "min_feature_size": "25%",
      "min_bead_width": "85%",
      "only_one_wall_top": "0",
      "seam_position": "aligned",
      "seam_slope_type": "none",
      "seam_slope_conditional": "0",
      "seam_slope_start_height": "0%",
      "seam_slope_entire_loop": "0",
      "seam_slope_min_length": "20",
      "seam_slope_steps": "10",
      "seam_gap": "15%",
      "scarf_angle_threshold": "155",
      "ironing_type": "no ironing",
      "ironing_flow": "10%",
      "ironing_speed": "30",
      "print_sequence": "by layer",
      "brim_type": "auto_brim",
      "brim_width": "5",
      "enable_support": "0",
      "support_type": "normal(auto)",
      "wipe_on_loops": "0",
      "reduce_crossing_wall": "0",
      "z_hop": ["0.0"],
      "z_hop_types": ["Normal Lift"],
      "travel_acceleration": "4000",
      "cut_retraction_length": "28.0",
      "transition_flush_temp_hold": "205",
      "fan_cooling_layer_time": ["30"],
      "fan_min_speed": ["40"],
      "machine_max_speed_x": ["500"],
      "machine_max_acceleration_x": ["10000"],
      "flush_multiplier": "1.0",
      "computed_safe_speed_ceiling": "250",
      "version": "2.0.0.0"
    });

    Self {
      name: "Anycubic Kobra S1 - Stock Volcano V6 Nozzle".to_string(),
      profile_type: "machine".to_string(),
      inherits: Some("Anycubic Kobra S1 0.4 nozzle".to_string()),
      from: Some("User".to_string()),
      values: json_data.as_object().unwrap().clone(),
    }
  }

  pub fn get_stock_voron_24() -> Self {
    let json_data = serde_json::json!({
      "name": "Voron 2.4 350mm - Revo Voron Edition",
      "type": "machine",
      "from": "User",
      "inherits": "Voron_v2_350_afterburner 0.4 nozzle",
      "printer_model": "Voron 2.4 350mm",
      "printer_variant": "0.4",
      "nozzle_diameter": ["0.4"],
      "nozzle_type": "hardened_steel",
      "printable_area": ["0x0", "350x0", "350x350", "0x350"],
      "printable_height": "340",
      "filament_type": ["ABS"],
      "nozzle_temperature": ["250"],
      "nozzle_temperature_initial_layer": ["255"],
      "hot_plate_temp": ["105"],
      "hot_plate_temp_initial_layer": ["110"],
      "filament_flow_ratio": ["0.95"],
      "filament_max_volumetric_speed": ["18.0"],
      "filament_retraction_length": ["0.5"],
      "filament_retraction_speed": ["40"],
      "pressure_advance": ["0.022"],
      "layer_height": "0.20",
      "initial_layer_speed": "40",
      "outer_wall_line_width": "0.42",
      "outer_wall_speed": "200",
      "inner_wall_speed": "300",
      "sparse_infill_speed": "350",
      "sparse_infill_pattern": "grid",
      "wipe_on_loops": "0",
      "z_hop": ["0.2"],
      "machine_max_speed_x": ["500"],
      "machine_max_acceleration_x": ["12000"],
      "version": "2.0.0.0"
    });

    Self {
      name: "Voron 2.4 350mm - Revo Voron Edition".to_string(),
      profile_type: "machine".to_string(),
      inherits: Some("Voron_v2_350_afterburner 0.4 nozzle".to_string()),
      from: Some("User".to_string()),
      values: json_data.as_object().unwrap().clone(),
    }
  }

  pub fn get_parameter_catalog() -> HashMap<String, ParameterMeta> {
    let mut map = HashMap::new();
    let items = vec![
      ParameterMeta {
        key: "nozzle_type".to_string(),
        label: "Orca Machine Nozzle Type".to_string(),
        category: "Machine".to_string(),
        unit: "".to_string(),
        description: "Machine OrcaSlicer nozzle_type (brass or hardened_steel)".to_string(),
      },
      ParameterMeta {
        key: "nozzle_hrc".to_string(),
        label: "Nozzle Hardness (HRC)".to_string(),
        category: "Machine".to_string(),
        unit: "HRC".to_string(),
        description:
          "Rockwell C hardness (0 Brass, 55 Hardened Steel, 60 Ruby-Brass, 68 Diamondback PCD)"
            .to_string(),
      },
      ParameterMeta {
        key: "nozzle_temperature".to_string(),
        label: "Print Temperature".to_string(),
        category: "Thermal".to_string(),
        unit: "°C".to_string(),
        description: "Extruder temperature during active printing".to_string(),
      },
      ParameterMeta {
        key: "nozzle_temperature_initial_layer".to_string(),
        label: "Initial Layer Temp".to_string(),
        category: "Thermal".to_string(),
        unit: "°C".to_string(),
        description: "Hotend temperature for bed-contact first layer".to_string(),
      },
      ParameterMeta {
        key: "hot_plate_temp".to_string(),
        label: "Bed Temperature".to_string(),
        category: "Thermal".to_string(),
        unit: "°C".to_string(),
        description: "Heated build plate temperature".to_string(),
      },
      ParameterMeta {
        key: "hot_plate_temp_initial_layer".to_string(),
        label: "Bed Temp Initial Layer".to_string(),
        category: "Thermal".to_string(),
        unit: "°C".to_string(),
        description: "Heated build plate first layer adhesion temperature".to_string(),
      },
      ParameterMeta {
        key: "filament_flow_ratio".to_string(),
        label: "Flow Ratio".to_string(),
        category: "Extrusion".to_string(),
        unit: "".to_string(),
        description: "Extrusion multiplier compensating for material swell & nozzle conductivity"
          .to_string(),
      },
      ParameterMeta {
        key: "filament_max_volumetric_speed".to_string(),
        label: "Max Volumetric Speed".to_string(),
        category: "Extrusion".to_string(),
        unit: "mm³/s".to_string(),
        description: "Hotend melting throughput limit".to_string(),
      },
      ParameterMeta {
        key: "computed_safe_speed_ceiling".to_string(),
        label: "Safe Speed Ceiling".to_string(),
        category: "Speed".to_string(),
        unit: "mm/s".to_string(),
        description: "min(Hotend_flow, Filament_flow) / (Layer_Height * Line_Width)".to_string(),
      },
      ParameterMeta {
        key: "outer_wall_speed".to_string(),
        label: "Outer Wall Speed".to_string(),
        category: "Speed".to_string(),
        unit: "mm/s".to_string(),
        description: "Perimeter speed dynamically capped by volumetric ceiling".to_string(),
      },
      ParameterMeta {
        key: "inner_wall_speed".to_string(),
        label: "Inner Wall Speed".to_string(),
        category: "Speed".to_string(),
        unit: "mm/s".to_string(),
        description: "Inner perimeter speed dynamically capped by volumetric ceiling".to_string(),
      },
      ParameterMeta {
        key: "sparse_infill_speed".to_string(),
        label: "Sparse Infill Speed".to_string(),
        category: "Speed".to_string(),
        unit: "mm/s".to_string(),
        description: "Internal infill speed dynamically capped by volumetric ceiling".to_string(),
      },
      ParameterMeta {
        key: "filament_retraction_length".to_string(),
        label: "Retraction Length".to_string(),
        category: "Extrusion".to_string(),
        unit: "mm".to_string(),
        description: "Distance filament is pulled back during travels".to_string(),
      },
      ParameterMeta {
        key: "filament_retraction_speed".to_string(),
        label: "Retraction Speed".to_string(),
        category: "Extrusion".to_string(),
        unit: "mm/s".to_string(),
        description: "Velocity of retract movement".to_string(),
      },
      ParameterMeta {
        key: "filament_deretraction_speed".to_string(),
        label: "Deretraction Speed".to_string(),
        category: "Extrusion".to_string(),
        unit: "mm/s".to_string(),
        description: "Velocity of filament prime after retraction".to_string(),
      },
      ParameterMeta {
        key: "filament_retract_before_wipe".to_string(),
        label: "Retract Before Wipe".to_string(),
        category: "Extrusion".to_string(),
        unit: "".to_string(),
        description: "Fraction of retraction performed before wiping nozzle".to_string(),
      },
      ParameterMeta {
        key: "filament_wipe_distance".to_string(),
        label: "Wipe Distance".to_string(),
        category: "Extrusion".to_string(),
        unit: "mm".to_string(),
        description: "Distance nozzle wipes along inner wall before travel".to_string(),
      },
      ParameterMeta {
        key: "wall_generator".to_string(),
        label: "Wall Generator".to_string(),
        category: "Walls & Seams".to_string(),
        unit: "".to_string(),
        description: "Perimeter engine (classic vs arachne variable-width walls)".to_string(),
      },
      ParameterMeta {
        key: "seam_slope_type".to_string(),
        label: "Scarf Joint Seam Mode".to_string(),
        category: "Walls & Seams".to_string(),
        unit: "".to_string(),
        description: "OrcaSlicer scarf seam ramp mode (none, contour, all)".to_string(),
      },
      ParameterMeta {
        key: "seam_slope_conditional".to_string(),
        label: "Conditional Scarf Seam".to_string(),
        category: "Walls & Seams".to_string(),
        unit: "flag".to_string(),
        description: "Apply scarf joint only on smooth curves above angle threshold".to_string(),
      },
      ParameterMeta {
        key: "seam_slope_start_height".to_string(),
        label: "Scarf Start Height".to_string(),
        category: "Walls & Seams".to_string(),
        unit: "".to_string(),
        description: "Initial layer height ratio for scarf ramp".to_string(),
      },
      ParameterMeta {
        key: "seam_slope_min_length".to_string(),
        label: "Scarf Slope Length".to_string(),
        category: "Walls & Seams".to_string(),
        unit: "mm".to_string(),
        description: "Horizontal ramp length for scarf seam overlap".to_string(),
      },
      ParameterMeta {
        key: "seam_slope_steps".to_string(),
        label: "Scarf Seam Steps".to_string(),
        category: "Walls & Seams".to_string(),
        unit: "steps".to_string(),
        description: "Number of discrete Z-ramp steps in scarf joint".to_string(),
      },
      ParameterMeta {
        key: "seam_gap".to_string(),
        label: "Seam Gap".to_string(),
        category: "Walls & Seams".to_string(),
        unit: "".to_string(),
        description: "Loop closure gap percentage to prevent seam bulge".to_string(),
      },
      ParameterMeta {
        key: "scarf_angle_threshold".to_string(),
        label: "Scarf Angle Threshold".to_string(),
        category: "Walls & Seams".to_string(),
        unit: "°".to_string(),
        description: "Corner angle above which scarf seam is activated".to_string(),
      },
      ParameterMeta {
        key: "pressure_advance".to_string(),
        label: "Pressure Advance".to_string(),
        category: "Extrusion".to_string(),
        unit: "s".to_string(),
        description: "Nozzle pressure compensation coefficient".to_string(),
      },
      ParameterMeta {
        key: "initial_layer_speed".to_string(),
        label: "Initial Layer Speed".to_string(),
        category: "Speed".to_string(),
        unit: "mm/s".to_string(),
        description: "Print speed for the crucial first layer".to_string(),
      },
      ParameterMeta {
        key: "initial_layer_line_width".to_string(),
        label: "First Layer Line Width".to_string(),
        category: "Extrusion".to_string(),
        unit: "mm".to_string(),
        description: "Width of bead pressed onto the bed".to_string(),
      },
      ParameterMeta {
        key: "sparse_infill_pattern".to_string(),
        label: "Infill Pattern".to_string(),
        category: "Structure".to_string(),
        unit: "".to_string(),
        description: "Geometry used for internal solid support".to_string(),
      },
      ParameterMeta {
        key: "wipe_on_loops".to_string(),
        label: "Wipe on Loops".to_string(),
        category: "Travel".to_string(),
        unit: "flag".to_string(),
        description: "Wipe nozzle inside perimeter before travel move".to_string(),
      },
      ParameterMeta {
        key: "reduce_crossing_wall".to_string(),
        label: "Avoid Crossing Walls".to_string(),
        category: "Travel".to_string(),
        unit: "flag".to_string(),
        description: "Detour travel moves around perimeters to prevent nozzle knock".to_string(),
      },
      ParameterMeta {
        key: "z_hop".to_string(),
        label: "Z-Hop Height".to_string(),
        category: "Travel".to_string(),
        unit: "mm".to_string(),
        description: "Vertical lift on retract to clear printed contours".to_string(),
      },
      ParameterMeta {
        key: "z_hop_types".to_string(),
        label: "Z-Hop Strategy".to_string(),
        category: "Travel".to_string(),
        unit: "".to_string(),
        description: "Normal, Spiral, or Slope Z-hop trajectory".to_string(),
      },
      ParameterMeta {
        key: "travel_acceleration".to_string(),
        label: "Travel Acceleration".to_string(),
        category: "Kinematics".to_string(),
        unit: "mm/s²".to_string(),
        description: "Non-extruding travel move acceleration to snap whiskers".to_string(),
      },
      ParameterMeta {
        key: "cut_retraction_length".to_string(),
        label: "Cut Retraction Limit".to_string(),
        category: "Multi-Material".to_string(),
        unit: "mm".to_string(),
        description: "Pre-cut pullback distance in ACE Pro toolhead".to_string(),
      },
      ParameterMeta {
        key: "transition_flush_temp_hold".to_string(),
        label: "Transition Flush Temp".to_string(),
        category: "Multi-Material".to_string(),
        unit: "°C".to_string(),
        description: "Hotend hold temp when purging high-temp to low-temp polymers".to_string(),
      },
      ParameterMeta {
        key: "flush_multiplier".to_string(),
        label: "Purge Multiplier".to_string(),
        category: "Multi-Material".to_string(),
        unit: "x".to_string(),
        description: "Purge volume scaling factor on spool swap".to_string(),
      },
      ParameterMeta {
        key: "wall_loops".to_string(),
        label: "Wall Loops (Perimeters)".to_string(),
        category: "Walls & Seams".to_string(),
        unit: "loops".to_string(),
        description: "Number of perimeter wall loops".to_string(),
      },
      ParameterMeta {
        key: "seam_position".to_string(),
        label: "Seam Position".to_string(),
        category: "Walls & Seams".to_string(),
        unit: "".to_string(),
        description: "Z-seam placement strategy (aligned, nearest, back, random)".to_string(),
      },
      ParameterMeta {
        key: "only_one_wall_top".to_string(),
        label: "Only One Wall on Top".to_string(),
        category: "Walls & Seams".to_string(),
        unit: "flag".to_string(),
        description: "Use single perimeter on top solid surfaces for cleaner finish".to_string(),
      },
      ParameterMeta {
        key: "sparse_infill_density".to_string(),
        label: "Sparse Infill Density".to_string(),
        category: "Structure".to_string(),
        unit: "".to_string(),
        description: "Internal infill percentage".to_string(),
      },
      ParameterMeta {
        key: "ironing_type".to_string(),
        label: "Top Surface Ironing".to_string(),
        category: "Structure".to_string(),
        unit: "".to_string(),
        description: "Hot nozzle pass over top solid layers (no ironing, top, topmost, all_solid)"
          .to_string(),
      },
      ParameterMeta {
        key: "ironing_flow".to_string(),
        label: "Ironing Flow Rate".to_string(),
        category: "Structure".to_string(),
        unit: "".to_string(),
        description: "Extrusion percentage during top-surface ironing pass".to_string(),
      },
      ParameterMeta {
        key: "ironing_speed".to_string(),
        label: "Ironing Speed".to_string(),
        category: "Structure".to_string(),
        unit: "mm/s".to_string(),
        description: "Travel velocity during ironing pass".to_string(),
      },
      ParameterMeta {
        key: "print_sequence".to_string(),
        label: "Print Sequence".to_string(),
        category: "Structure".to_string(),
        unit: "".to_string(),
        description: "Layer-by-layer vs Object-by-object print order".to_string(),
      },
      ParameterMeta {
        key: "fan_min_speed".to_string(),
        label: "Min Part Cooling Fan".to_string(),
        category: "Cooling & Adhesion".to_string(),
        unit: "%".to_string(),
        description: "Minimum part cooling fan duty cycle".to_string(),
      },
      ParameterMeta {
        key: "fan_cooling_layer_time".to_string(),
        label: "Fan Cooling Layer Time".to_string(),
        category: "Cooling & Adhesion".to_string(),
        unit: "s".to_string(),
        description: "Layer time threshold below which fan ramps up".to_string(),
      },
      ParameterMeta {
        key: "brim_type".to_string(),
        label: "Brim Type".to_string(),
        category: "Cooling & Adhesion".to_string(),
        unit: "".to_string(),
        description: "Bed adhesion brim mode (auto_brim, no_brim, outer_only, inner_only)"
          .to_string(),
      },
      ParameterMeta {
        key: "brim_width".to_string(),
        label: "Brim Width".to_string(),
        category: "Cooling & Adhesion".to_string(),
        unit: "mm".to_string(),
        description: "Width of first-layer adhesion brim".to_string(),
      },
      ParameterMeta {
        key: "enable_support".to_string(),
        label: "Enable Support".to_string(),
        category: "Cooling & Adhesion".to_string(),
        unit: "flag".to_string(),
        description: "Generate overhang support structures".to_string(),
      },
      ParameterMeta {
        key: "support_type".to_string(),
        label: "Support Type".to_string(),
        category: "Cooling & Adhesion".to_string(),
        unit: "".to_string(),
        description: "Support geometry (normal(auto), tree(auto), normal(manual), tree(manual))"
          .to_string(),
      },
    ];

    for item in items {
      map.insert(item.key.clone(), item);
    }
    map
  }

  /// Rich, searchable catalog of OrcaSlicer settings for the Simple Visual Rule Wizard,
  /// grouped by section and typed by operation (Add/Offset, Multiply, Set Choice, Toggle Flag, Set Percentage).
  pub fn get_wizard_setting_catalog() -> Vec<WizardSettingDef> {
    vec![
      // --- Section: Thermal ---
      WizardSettingDef {
        key: "nozzle_temperature".to_string(),
        label: "Print Temperature".to_string(),
        section: "Thermal".to_string(),
        unit: "°C".to_string(),
        description: "Active hotend extrusion temperature (add/subtract °C offset or set exact temp)".to_string(),
        default_base_value: "240".to_string(),
        is_array_in_orca: true,
        decimals: 0,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::AddOffset,
          presets: vec!["-10".to_string(), "-5".to_string(), "-3".to_string(), "+5".to_string(), "+10".to_string(), "+15".to_string()],
          suggested: "+5".to_string(),
        },
      },
      WizardSettingDef {
        key: "nozzle_temperature_initial_layer".to_string(),
        label: "Initial Layer Nozzle Temp".to_string(),
        section: "Thermal".to_string(),
        unit: "°C".to_string(),
        description: "First-layer hotend temperature for bed bonding".to_string(),
        default_base_value: "245".to_string(),
        is_array_in_orca: true,
        decimals: 0,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::AddOffset,
          presets: vec!["-5".to_string(), "+5".to_string(), "+10".to_string()],
          suggested: "+5".to_string(),
        },
      },
      WizardSettingDef {
        key: "hot_plate_temp".to_string(),
        label: "Bed Temperature".to_string(),
        section: "Thermal".to_string(),
        unit: "°C".to_string(),
        description: "Heated build plate temperature during printing".to_string(),
        default_base_value: "75".to_string(),
        is_array_in_orca: true,
        decimals: 0,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::AddOffset,
          presets: vec!["-5".to_string(), "+5".to_string(), "+7".to_string(), "+10".to_string()],
          suggested: "+5".to_string(),
        },
      },
      WizardSettingDef {
        key: "hot_plate_temp_initial_layer".to_string(),
        label: "Initial Layer Bed Temp".to_string(),
        section: "Thermal".to_string(),
        unit: "°C".to_string(),
        description: "First-layer heated bed temperature to prevent corner lift".to_string(),
        default_base_value: "80".to_string(),
        is_array_in_orca: true,
        decimals: 0,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::AddOffset,
          presets: vec!["-5".to_string(), "+5".to_string(), "+10".to_string()],
          suggested: "+5".to_string(),
        },
      },
      // --- Section: Walls & Seams ---
      WizardSettingDef {
        key: "wall_generator".to_string(),
        label: "Wall Generator Engine".to_string(),
        section: "Walls & Seams".to_string(),
        unit: "".to_string(),
        description: "Switch between Classic fixed-width perimeters and Arachne variable-width walls".to_string(),
        default_base_value: "classic".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::EnumChoices {
          options: vec!["classic".to_string(), "arachne".to_string()],
          suggested: "arachne".to_string(),
        },
      },
      WizardSettingDef {
        key: "seam_slope_type".to_string(),
        label: "Scarf Joint Seam Mode".to_string(),
        section: "Walls & Seams".to_string(),
        unit: "".to_string(),
        description: "OrcaSlicer scarf seam ramp mode (none = disabled, contour = outer wall, all = outer + inner)".to_string(),
        default_base_value: "none".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::EnumChoices {
          options: vec!["none".to_string(), "contour".to_string(), "all".to_string()],
          suggested: "all".to_string(),
        },
      },
      WizardSettingDef {
        key: "seam_position".to_string(),
        label: "Seam Position".to_string(),
        section: "Walls & Seams".to_string(),
        unit: "".to_string(),
        description: "Z-seam placement along each layer contour".to_string(),
        default_base_value: "aligned".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::EnumChoices {
          options: vec!["aligned".to_string(), "nearest".to_string(), "back".to_string(), "random".to_string()],
          suggested: "aligned".to_string(),
        },
      },
      WizardSettingDef {
        key: "seam_slope_conditional".to_string(),
        label: "Conditional Scarf Seam".to_string(),
        section: "Walls & Seams".to_string(),
        unit: "flag".to_string(),
        description: "Only apply scarf ramp on smooth curves above angle threshold".to_string(),
        default_base_value: "0".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::BooleanFlag {
          suggested_enabled: true,
        },
      },
      WizardSettingDef {
        key: "seam_slope_steps".to_string(),
        label: "Scarf Seam Steps".to_string(),
        section: "Walls & Seams".to_string(),
        unit: "steps".to_string(),
        description: "Number of Z-ramp steps in scarf joint (higher = smoother ramp)".to_string(),
        default_base_value: "10".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["10".to_string(), "15".to_string(), "20".to_string(), "+5".to_string()],
          suggested: "15".to_string(),
        },
      },
      WizardSettingDef {
        key: "seam_slope_min_length".to_string(),
        label: "Scarf Slope Length".to_string(),
        section: "Walls & Seams".to_string(),
        unit: "mm".to_string(),
        description: "Horizontal ramp length for scarf seam overlap".to_string(),
        default_base_value: "20".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["10".to_string(), "15".to_string(), "20".to_string(), "25".to_string()],
          suggested: "15".to_string(),
        },
      },
      WizardSettingDef {
        key: "seam_slope_start_height".to_string(),
        label: "Scarf Start Height".to_string(),
        section: "Walls & Seams".to_string(),
        unit: "".to_string(),
        description: "Starting height percentage for scarf ramp".to_string(),
        default_base_value: "0%".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::Percentage {
          presets: vec!["0%".to_string(), "10%".to_string(), "15%".to_string(), "20%".to_string()],
          suggested: "10%".to_string(),
        },
      },
      WizardSettingDef {
        key: "seam_gap".to_string(),
        label: "Seam Gap".to_string(),
        section: "Walls & Seams".to_string(),
        unit: "".to_string(),
        description: "Perimeter loop closure gap percentage to prevent seam bulge".to_string(),
        default_base_value: "15%".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::Percentage {
          presets: vec!["5%".to_string(), "10%".to_string(), "15%".to_string()],
          suggested: "10%".to_string(),
        },
      },
      WizardSettingDef {
        key: "scarf_angle_threshold".to_string(),
        label: "Scarf Angle Threshold".to_string(),
        section: "Walls & Seams".to_string(),
        unit: "°".to_string(),
        description: "Corner angle above which scarf seam activates on smooth contours".to_string(),
        default_base_value: "155".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["135".to_string(), "145".to_string(), "155".to_string(), "165".to_string()],
          suggested: "155".to_string(),
        },
      },
      WizardSettingDef {
        key: "wall_loops".to_string(),
        label: "Wall Loops (Perimeters)".to_string(),
        section: "Walls & Seams".to_string(),
        unit: "loops".to_string(),
        description: "Number of perimeter wall loops for part strength".to_string(),
        default_base_value: "2".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["2".to_string(), "3".to_string(), "4".to_string(), "5".to_string(), "+1".to_string()],
          suggested: "3".to_string(),
        },
      },
      WizardSettingDef {
        key: "only_one_wall_top".to_string(),
        label: "Only One Wall on Top Surfaces".to_string(),
        section: "Walls & Seams".to_string(),
        unit: "flag".to_string(),
        description: "Use a single perimeter on top solid surfaces for cleaner top finish".to_string(),
        default_base_value: "0".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::BooleanFlag {
          suggested_enabled: true,
        },
      },
      // --- Section: Extrusion & Retraction ---
      WizardSettingDef {
        key: "filament_retraction_length".to_string(),
        label: "Retraction Length".to_string(),
        section: "Extrusion & Retraction".to_string(),
        unit: "mm".to_string(),
        description: "Pullback distance on travel moves (0.60-0.95mm direct drive)".to_string(),
        default_base_value: "2.00".to_string(),
        is_array_in_orca: true,
        decimals: 2,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["0.60".to_string(), "0.80".to_string(), "0.85".to_string(), "0.95".to_string(), "-0.20".to_string(), "+0.15".to_string()],
          suggested: "0.85".to_string(),
        },
      },
      WizardSettingDef {
        key: "filament_retraction_speed".to_string(),
        label: "Retraction Speed".to_string(),
        section: "Extrusion & Retraction".to_string(),
        unit: "mm/s".to_string(),
        description: "Retract pullback speed (35-50 mm/s for direct drive)".to_string(),
        default_base_value: "35".to_string(),
        is_array_in_orca: true,
        decimals: 0,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["35".to_string(), "40".to_string(), "45".to_string(), "50".to_string(), "+5".to_string(), "+10".to_string()],
          suggested: "45".to_string(),
        },
      },
      WizardSettingDef {
        key: "filament_deretraction_speed".to_string(),
        label: "Deretraction Speed".to_string(),
        section: "Extrusion & Retraction".to_string(),
        unit: "mm/s".to_string(),
        description: "Filament prime velocity after travel move".to_string(),
        default_base_value: "30".to_string(),
        is_array_in_orca: true,
        decimals: 0,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["25".to_string(), "30".to_string(), "35".to_string(), "40".to_string()],
          suggested: "35".to_string(),
        },
      },
      WizardSettingDef {
        key: "filament_retract_before_wipe".to_string(),
        label: "Retract Before Wipe".to_string(),
        section: "Extrusion & Retraction".to_string(),
        unit: "".to_string(),
        description: "Percentage of retraction completed before nozzle wipe (70% ideal for PCTG/PETG)".to_string(),
        default_base_value: "0%".to_string(),
        is_array_in_orca: true,
        decimals: 0,
        kind: SettingValueKind::Percentage {
          presets: vec!["0%".to_string(), "50%".to_string(), "70%".to_string(), "100%".to_string()],
          suggested: "70%".to_string(),
        },
      },
      WizardSettingDef {
        key: "filament_wipe_distance".to_string(),
        label: "Wipe Distance".to_string(),
        section: "Extrusion & Retraction".to_string(),
        unit: "mm".to_string(),
        description: "Nozzle wipe length inside perimeter before travel".to_string(),
        default_base_value: "1.0".to_string(),
        is_array_in_orca: true,
        decimals: 1,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["1.0".to_string(), "1.5".to_string(), "2.0".to_string(), "2.5".to_string()],
          suggested: "2.0".to_string(),
        },
      },
      WizardSettingDef {
        key: "filament_flow_ratio".to_string(),
        label: "Flow Ratio (Extrusion Multiplier)".to_string(),
        section: "Extrusion & Retraction".to_string(),
        unit: "".to_string(),
        description: "Extrusion flow multiplier (set exact or scale)".to_string(),
        default_base_value: "1.00".to_string(),
        is_array_in_orca: true,
        decimals: 2,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["0.94".to_string(), "0.96".to_string(), "0.98".to_string(), "1.00".to_string(), "-0.02".to_string(), "+0.02".to_string()],
          suggested: "0.96".to_string(),
        },
      },
      WizardSettingDef {
        key: "pressure_advance".to_string(),
        label: "Pressure Advance".to_string(),
        section: "Extrusion & Retraction".to_string(),
        unit: "s".to_string(),
        description: "Klipper linear pressure advance factor".to_string(),
        default_base_value: "0.040".to_string(),
        is_array_in_orca: true,
        decimals: 3,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["0.025".to_string(), "0.035".to_string(), "0.040".to_string(), "0.060".to_string(), "0.080".to_string()],
          suggested: "0.040".to_string(),
        },
      },
      WizardSettingDef {
        key: "initial_layer_line_width".to_string(),
        label: "First Layer Line Width (Squish)".to_string(),
        section: "Extrusion & Retraction".to_string(),
        unit: "mm".to_string(),
        description: "Wider first-layer bead for stronger bed adhesion".to_string(),
        default_base_value: "0.50".to_string(),
        is_array_in_orca: false,
        decimals: 2,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["0.48".to_string(), "0.50".to_string(), "0.55".to_string(), "0.60".to_string()],
          suggested: "0.55".to_string(),
        },
      },
      // --- Section: Speed & Kinematics ---
      WizardSettingDef {
        key: "outer_wall_speed".to_string(),
        label: "Outer Wall Speed".to_string(),
        section: "Speed & Kinematics".to_string(),
        unit: "mm/s".to_string(),
        description: "Visible outer perimeter speed (lower for gloss consistency & zero ringing)".to_string(),
        default_base_value: "200".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["60".to_string(), "100".to_string(), "120".to_string(), "150".to_string(), "-30".to_string(), "-50".to_string()],
          suggested: "120".to_string(),
        },
      },
      WizardSettingDef {
        key: "inner_wall_speed".to_string(),
        label: "Inner Wall Speed".to_string(),
        section: "Speed & Kinematics".to_string(),
        unit: "mm/s".to_string(),
        description: "Internal perimeter speed".to_string(),
        default_base_value: "300".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["150".to_string(), "200".to_string(), "250".to_string(), "-50".to_string()],
          suggested: "200".to_string(),
        },
      },
      WizardSettingDef {
        key: "sparse_infill_speed".to_string(),
        label: "Sparse Infill Speed".to_string(),
        section: "Speed & Kinematics".to_string(),
        unit: "mm/s".to_string(),
        description: "Internal infill printing velocity".to_string(),
        default_base_value: "350".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["200".to_string(), "250".to_string(), "270".to_string(), "-50".to_string()],
          suggested: "250".to_string(),
        },
      },
      WizardSettingDef {
        key: "initial_layer_speed".to_string(),
        label: "Initial Layer Speed".to_string(),
        section: "Speed & Kinematics".to_string(),
        unit: "mm/s".to_string(),
        description: "First-layer print speed for reliable bed adhesion".to_string(),
        default_base_value: "35".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["20".to_string(), "25".to_string(), "35".to_string(), "50".to_string(), "-10".to_string()],
          suggested: "25".to_string(),
        },
      },
      WizardSettingDef {
        key: "travel_acceleration".to_string(),
        label: "Travel Acceleration".to_string(),
        section: "Speed & Kinematics".to_string(),
        unit: "mm/s²".to_string(),
        description: "Non-extruding travel acceleration to snap fine stringing whiskers".to_string(),
        default_base_value: "4000".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["5000".to_string(), "7000".to_string(), "8000".to_string(), "10000".to_string(), "+2000".to_string()],
          suggested: "8000".to_string(),
        },
      },
      // --- Section: Structure & Infill ---
      WizardSettingDef {
        key: "sparse_infill_pattern".to_string(),
        label: "Sparse Infill Pattern".to_string(),
        section: "Structure & Infill".to_string(),
        unit: "".to_string(),
        description: "Internal infill geometry (gyroid & crosshatch avoid nozzle-crossing collisions)".to_string(),
        default_base_value: "grid".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::EnumChoices {
          options: vec![
            "grid".to_string(),
            "gyroid".to_string(),
            "crosshatch".to_string(),
            "adaptivecubic".to_string(),
            "honeycomb".to_string(),
            "rectilinear".to_string(),
            "lightning".to_string(),
          ],
          suggested: "gyroid".to_string(),
        },
      },
      WizardSettingDef {
        key: "sparse_infill_density".to_string(),
        label: "Sparse Infill Density".to_string(),
        section: "Structure & Infill".to_string(),
        unit: "".to_string(),
        description: "Internal infill percentage".to_string(),
        default_base_value: "15%".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::Percentage {
          presets: vec!["10%".to_string(), "15%".to_string(), "20%".to_string(), "25%".to_string(), "40%".to_string()],
          suggested: "20%".to_string(),
        },
      },
      WizardSettingDef {
        key: "ironing_type".to_string(),
        label: "Top Surface Ironing Mode".to_string(),
        section: "Structure & Infill".to_string(),
        unit: "".to_string(),
        description: "Smooth flat top surfaces with a heated nozzle pass".to_string(),
        default_base_value: "no ironing".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::EnumChoices {
          options: vec![
            "no ironing".to_string(),
            "top".to_string(),
            "topmost".to_string(),
            "all_solid".to_string(),
          ],
          suggested: "top".to_string(),
        },
      },
      WizardSettingDef {
        key: "ironing_flow".to_string(),
        label: "Ironing Flow Rate".to_string(),
        section: "Structure & Infill".to_string(),
        unit: "".to_string(),
        description: "Extrusion flow percentage during ironing pass".to_string(),
        default_base_value: "10%".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::Percentage {
          presets: vec!["10%".to_string(), "15%".to_string(), "20%".to_string()],
          suggested: "15%".to_string(),
        },
      },
      WizardSettingDef {
        key: "print_sequence".to_string(),
        label: "Print Sequence".to_string(),
        section: "Structure & Infill".to_string(),
        unit: "".to_string(),
        description: "Print by layer vs by object (by object eliminates inter-part stringing)".to_string(),
        default_base_value: "by layer".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::EnumChoices {
          options: vec!["by layer".to_string(), "by object".to_string()],
          suggested: "by object".to_string(),
        },
      },
      // --- Section: Travel & Z-Hop ---
      WizardSettingDef {
        key: "z_hop".to_string(),
        label: "Z-Hop Height".to_string(),
        section: "Travel & Z-Hop".to_string(),
        unit: "mm".to_string(),
        description: "Vertical nozzle lift during travel moves".to_string(),
        default_base_value: "0.0".to_string(),
        is_array_in_orca: true,
        decimals: 2,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["0.00".to_string(), "0.20".to_string(), "0.40".to_string(), "+0.20".to_string()],
          suggested: "0.20".to_string(),
        },
      },
      WizardSettingDef {
        key: "z_hop_types".to_string(),
        label: "Z-Hop Strategy".to_string(),
        section: "Travel & Z-Hop".to_string(),
        unit: "".to_string(),
        description: "Trajectory of Z-lift (Spiral shears fine strings; Slope avoids overhang knocks)".to_string(),
        default_base_value: "Normal Lift".to_string(),
        is_array_in_orca: true,
        decimals: 0,
        kind: SettingValueKind::EnumChoices {
          options: vec![
            "Normal Lift".to_string(),
            "Spiral Lift".to_string(),
            "Slope Lift".to_string(),
            "Auto Lift".to_string(),
          ],
          suggested: "Spiral Lift".to_string(),
        },
      },
      WizardSettingDef {
        key: "wipe_on_loops".to_string(),
        label: "Wipe on Loops".to_string(),
        section: "Travel & Z-Hop".to_string(),
        unit: "flag".to_string(),
        description: "Wipe nozzle inward on perimeter loop closure".to_string(),
        default_base_value: "0".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::BooleanFlag {
          suggested_enabled: true,
        },
      },
      WizardSettingDef {
        key: "reduce_crossing_wall".to_string(),
        label: "Avoid Crossing Walls".to_string(),
        section: "Travel & Z-Hop".to_string(),
        unit: "flag".to_string(),
        description: "Route travel moves inside parts to avoid outer wall blemishes and knocks".to_string(),
        default_base_value: "0".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::BooleanFlag {
          suggested_enabled: true,
        },
      },
      // --- Section: Cooling & Adhesion ---
      WizardSettingDef {
        key: "fan_min_speed".to_string(),
        label: "Min Part Cooling Fan Speed".to_string(),
        section: "Cooling & Adhesion".to_string(),
        unit: "%".to_string(),
        description: "Baseline part cooling fan speed (add/subtract % or set exact duty cycle)".to_string(),
        default_base_value: "40".to_string(),
        is_array_in_orca: true,
        decimals: 0,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::AddOffset,
          presets: vec!["-15".to_string(), "-10".to_string(), "+10".to_string(), "+20".to_string(), "25".to_string(), "60".to_string()],
          suggested: "-10".to_string(),
        },
      },
      WizardSettingDef {
        key: "brim_type".to_string(),
        label: "Brim Type".to_string(),
        section: "Cooling & Adhesion".to_string(),
        unit: "".to_string(),
        description: "First-layer bed adhesion brim mode".to_string(),
        default_base_value: "auto_brim".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::EnumChoices {
          options: vec![
            "auto_brim".to_string(),
            "no_brim".to_string(),
            "outer_only".to_string(),
            "inner_only".to_string(),
            "outer_and_inner".to_string(),
          ],
          suggested: "outer_only".to_string(),
        },
      },
      WizardSettingDef {
        key: "brim_width".to_string(),
        label: "Brim Width".to_string(),
        section: "Cooling & Adhesion".to_string(),
        unit: "mm".to_string(),
        description: "Width of first-layer brim in millimeters".to_string(),
        default_base_value: "5".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::Numeric {
          default_op: WizardActionOp::SetValue,
          presets: vec!["0".to_string(), "3".to_string(), "5".to_string(), "8".to_string(), "10".to_string()],
          suggested: "5".to_string(),
        },
      },
      WizardSettingDef {
        key: "enable_support".to_string(),
        label: "Enable Overhang Supports".to_string(),
        section: "Cooling & Adhesion".to_string(),
        unit: "flag".to_string(),
        description: "Enable support generation for steep overhangs".to_string(),
        default_base_value: "0".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::BooleanFlag {
          suggested_enabled: true,
        },
      },
      WizardSettingDef {
        key: "support_type".to_string(),
        label: "Support Structure Type".to_string(),
        section: "Cooling & Adhesion".to_string(),
        unit: "".to_string(),
        description: "Switch between Normal grid supports and organic Tree supports".to_string(),
        default_base_value: "normal(auto)".to_string(),
        is_array_in_orca: false,
        decimals: 0,
        kind: SettingValueKind::EnumChoices {
          options: vec![
            "normal(auto)".to_string(),
            "tree(auto)".to_string(),
            "normal(manual)".to_string(),
            "tree(manual)".to_string(),
          ],
          suggested: "tree(auto)".to_string(),
        },
      },
    ]
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::transformations::{TransformationEngine, TransformationRule};

  #[test]
  fn test_wizard_rule_action_generates_valid_sandboxed_lua() {
    let catalog = OrcaProfile::get_wizard_setting_catalog();
    assert!(catalog.len() >= 30);

    let temp_def = catalog
      .iter()
      .find(|s| s.key == "nozzle_temperature")
      .unwrap();
    let wall_def = catalog.iter().find(|s| s.key == "wall_generator").unwrap();

    let actions = vec![
      WizardRuleAction {
        key: temp_def.key.clone(),
        label: temp_def.label.clone(),
        section: temp_def.section.clone(),
        unit: temp_def.unit.clone(),
        is_array_in_orca: temp_def.is_array_in_orca,
        decimals: temp_def.decimals,
        default_base_value: temp_def.default_base_value.clone(),
        op: WizardActionOp::AddOffset,
        operand: "+8".to_string(),
      },
      WizardRuleAction {
        key: wall_def.key.clone(),
        label: wall_def.label.clone(),
        section: wall_def.section.clone(),
        unit: wall_def.unit.clone(),
        is_array_in_orca: wall_def.is_array_in_orca,
        decimals: wall_def.decimals,
        default_base_value: wall_def.default_base_value.clone(),
        op: WizardActionOp::SetValue,
        operand: "arachne".to_string(),
      },
    ];

    let lua_script = WizardRuleAction::generate_lua_script(&actions, "Wizard Test Rule");
    let rule = TransformationRule {
      id: "wizard_test".to_string(),
      name: "Wizard Test Rule".to_string(),
      description: "Test".to_string(),
      category: "Visual Wizard Rule".to_string(),
      icon: "🪄".to_string(),
      enabled: true,
      condition_expr: "all".to_string(),
      lua_code: lua_script,
      priority: 50,
    };

    let base = OrcaProfile::get_stock_kobra_s1();
    let ctx = serde_json::json!({ "printer_model": "Anycubic Kobra S1" });
    let eval = TransformationEngine::evaluate_stack(&base, &[rule], &ctx);
    assert_eq!(
      eval.resulting_profile.get_str("nozzle_temperature"),
      Some("248".to_string())
    );
    assert_eq!(
      eval.resulting_profile.get_str("wall_generator"),
      Some("arachne".to_string())
    );
  }
}
