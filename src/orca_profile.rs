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
            from: Some("system".to_string()),
            values: serde_json::Map::new(),
        }
    }

    pub fn from_json_str(json_str: &str) -> Result<Self, String> {
        let val: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse Orca profile JSON: {}", e))?;
        
        if let serde_json::Value::Object(map) = val {
            let name = map.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Custom Profile")
                .to_string();
            let profile_type = map.get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("machine")
                .to_string();
            let inherits = map.get("inherits")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let from = map.get("from")
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
                arr.get(0).and_then(|first| first.as_str()).map(|s| s.to_string())
            } else {
                Some(v.to_string())
            }
        })
    }

    pub fn set_str(&mut self, key: &str, val: &str) {
        self.values.insert(key.to_string(), serde_json::Value::String(val.to_string()));
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
            "from": "system",
            "printer_model": "Anycubic Kobra S1",
            "printer_variant": "0.4",
            "nozzle_diameter": ["0.4"],
            "nozzle_type": "brass",
            "printable_area": ["0x0", "256x0", "256x256", "0x256"],
            "printable_height": "250",
            "filament_type": ["PETG"],
            "nozzle_temperature": ["240"],
            "nozzle_temperature_initial_layer": ["245"],
            "hot_plate_temp": ["75"],
            "hot_plate_temp_initial_layer": ["80"],
            "filament_flow_ratio": ["0.96"],
            "filament_max_volumetric_speed": ["12.0"],
            "filament_retraction_length": ["0.8"],
            "filament_retraction_speed": ["35"],
            "pressure_advance": ["0.038"],
            "layer_height": "0.20",
            "initial_layer_print_height": "0.24",
            "initial_layer_speed": "35",
            "initial_layer_line_width": "0.50",
            "outer_wall_line_width": "0.42",
            "sparse_infill_pattern": "grid",
            "wipe_on_loops": "0",
            "z_hop": ["0.0"],
            "z_hop_types": ["Normal"],
            "fan_cooling_layer_time": ["30"],
            "fan_min_speed": ["40"],
            "machine_max_speed_x": ["500"],
            "machine_max_acceleration_x": ["10000"],
            "flush_multiplier": "1.0"
        });

        Self {
            name: "Anycubic Kobra S1 - Stock Volcano V6 Nozzle".to_string(),
            profile_type: "machine".to_string(),
            inherits: Some("fdm_kobra_common".to_string()),
            from: Some("system".to_string()),
            values: json_data.as_object().unwrap().clone(),
        }
    }

    pub fn get_stock_voron_24() -> Self {
        let json_data = serde_json::json!({
            "name": "Voron 2.4 350mm - Revo Voron Edition",
            "type": "machine",
            "from": "system",
            "printer_model": "Voron 2.4 350mm",
            "printer_variant": "0.4",
            "nozzle_diameter": ["0.4"],
            "nozzle_type": "revo_brass",
            "printable_area": ["0x0", "350x0", "350x350", "0x350"],
            "printable_height": "330",
            "filament_type": ["ABS"],
            "nozzle_temperature": ["250"],
            "nozzle_temperature_initial_layer": ["255"],
            "hot_plate_temp": ["105"],
            "hot_plate_temp_initial_layer": ["110"],
            "filament_flow_ratio": ["0.95"],
            "filament_max_volumetric_speed": ["15.0"],
            "filament_retraction_length": ["0.6"],
            "filament_retraction_speed": ["40"],
            "pressure_advance": ["0.022"],
            "layer_height": "0.20",
            "initial_layer_speed": "40",
            "outer_wall_line_width": "0.42",
            "sparse_infill_pattern": "grid",
            "wipe_on_loops": "0",
            "z_hop": ["0.2"],
            "machine_max_speed_x": ["500"],
            "machine_max_acceleration_x": ["12000"]
        });

        Self {
            name: "Voron 2.4 350mm - Revo Voron Edition".to_string(),
            profile_type: "machine".to_string(),
            inherits: Some("fdm_voron_common".to_string()),
            from: Some("system".to_string()),
            values: json_data.as_object().unwrap().clone(),
        }
    }

    pub fn get_parameter_catalog() -> HashMap<String, ParameterMeta> {
        let mut map = HashMap::new();
        let items = vec![
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
                description: "Extrusion multiplier compensating for material swell".to_string(),
            },
            ParameterMeta {
                key: "filament_max_volumetric_speed".to_string(),
                label: "Max Volumetric Speed".to_string(),
                category: "Extrusion".to_string(),
                unit: "mm³/s".to_string(),
                description: "Hotend melting throughput limit".to_string(),
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
                key: "z_hop".to_string(),
                label: "Z-Hop when Retracting".to_string(),
                category: "Travel".to_string(),
                unit: "mm".to_string(),
                description: "Vertical lift on retract to clear printed contours".to_string(),
            },
            ParameterMeta {
                key: "flush_multiplier".to_string(),
                label: "Purge Multiplier".to_string(),
                category: "Multi-Material".to_string(),
                unit: "x".to_string(),
                description: "Purge volume scaling factor on spool swap".to_string(),
            },
        ];

        for item in items {
            map.insert(item.key.clone(), item);
        }
        map
    }
}
