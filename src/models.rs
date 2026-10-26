use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MachineRig {
    pub id: String,
    pub name: String,
    pub printer_model: String,
    pub bed_size: String,
    pub kinematics: String,
    pub firmware: String,
    pub hotend: String,
    pub nozzle_type: String, // "diamondback", "brass", "hardened_steel"
    pub nozzle_name: String,
    pub nozzle_diameter: String,
    pub max_volumetric_flow: String,
    pub max_speed: String,
    pub max_accel: String,
    pub is_active: bool,
    pub applied_transformations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FilamentSpool {
    pub id: String,
    pub name: String,
    pub brand: String,
    pub material_type: String, // "PLA", "PETG", "TPU", "ABS", "ASA"
    pub color_hex: String,
    pub color_name: String,
    pub print_temp: i32,
    pub bed_temp: i32,
    pub flow_ratio: f32,
    pub pressure_advance: f32,
    pub max_volumetric_speed: f32,
    pub fan_speed_percent: i32,
    pub remaining_grams: i32,
    pub is_loaded: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AceSlot {
    pub slot_index: usize, // 1 to 4
    pub spool: FilamentSpool,
    pub is_active_feed: bool,
    pub feed_status: String,
}

impl MachineRig {
    pub fn default_kobra_diamondback() -> Self {
        Self {
            id: "kobra_s1_diamondback".to_string(),
            name: "Anycubic Kobra S1 — MicroSwiss + Diamondback".to_string(),
            printer_model: "Anycubic Kobra S1".to_string(),
            bed_size: "256 × 256 × 250 mm".to_string(),
            kinematics: "CoreXY Enclosed S-Series".to_string(),
            firmware: "Klipper-based KobraOS v2.4".to_string(),
            hotend: "MicroSwiss FlowTech All-Metal".to_string(),
            nozzle_type: "diamondback".to_string(),
            nozzle_name: "Diamondback PCD Diamond 0.40mm".to_string(),
            nozzle_diameter: "0.40mm".to_string(),
            max_volumetric_flow: "32.0 mm³/s".to_string(),
            max_speed: "600 mm/s".to_string(),
            max_accel: "20,000 mm/s²".to_string(),
            is_active: true,
            applied_transformations: vec![
                "Diamondback PCD Nozzle Thermal Tuning".to_string(),
                "Stringing & Oozing Mitigation (PETG)".to_string(),
            ],
        }
    }

    pub fn default_kobra_stock() -> Self {
        Self {
            id: "kobra_s1_stock".to_string(),
            name: "Anycubic Kobra S1 — Stock Volcano V6 Nozzle".to_string(),
            printer_model: "Anycubic Kobra S1".to_string(),
            bed_size: "256 × 256 × 250 mm".to_string(),
            kinematics: "CoreXY Enclosed S-Series".to_string(),
            firmware: "Klipper-based KobraOS v2.4".to_string(),
            hotend: "Stock Ceramic Heater Core".to_string(),
            nozzle_type: "brass".to_string(),
            nozzle_name: "Standard Volcano V6 Brass 0.40mm".to_string(),
            nozzle_diameter: "0.40mm".to_string(),
            max_volumetric_flow: "15.0 mm³/s".to_string(),
            max_speed: "500 mm/s".to_string(),
            max_accel: "10,000 mm/s²".to_string(),
            is_active: false,
            applied_transformations: vec![],
        }
    }

    pub fn default_voron_24() -> Self {
        Self {
            id: "voron_24_revo".to_string(),
            name: "Voron 2.4 350mm — Revo Voron Edition".to_string(),
            printer_model: "Voron 2.4 350mm".to_string(),
            bed_size: "350 × 350 × 330 mm".to_string(),
            kinematics: "Flying Gantry Quad-Gantry CoreXY".to_string(),
            firmware: "Klipper v0.12.0 (CanBoot SB2209)".to_string(),
            hotend: "E3D RapidChange Revo Voron 60W".to_string(),
            nozzle_type: "hardened_steel".to_string(),
            nozzle_name: "Revo ObXidian High Flow 0.40mm".to_string(),
            nozzle_diameter: "0.40mm".to_string(),
            max_volumetric_flow: "28.0 mm³/s".to_string(),
            max_speed: "500 mm/s".to_string(),
            max_accel: "12,000 mm/s²".to_string(),
            is_active: false,
            applied_transformations: vec![
                "Nozzle Knock & Spaghettization Guard".to_string(),
            ],
        }
    }
}

impl FilamentSpool {
    pub fn get_default_spools() -> Vec<Self> {
        vec![
            FilamentSpool {
                id: "spool_pla_hs".to_string(),
                name: "High Speed PLA".to_string(),
                brand: "Anycubic".to_string(),
                material_type: "PLA".to_string(),
                color_hex: "#06B6D4".to_string(),
                color_name: "Cyan (Anycubic)".to_string(),
                print_temp: 210,
                bed_temp: 60,
                flow_ratio: 0.98,
                pressure_advance: 0.024,
                max_volumetric_speed: 18.0,
                fan_speed_percent: 100,
                remaining_grams: 820,
                is_loaded: true,
            },
            FilamentSpool {
                id: "spool_petg_polylite".to_string(),
                name: "PolyLite PETG".to_string(),
                brand: "Polymaker".to_string(),
                material_type: "PETG".to_string(),
                color_hex: "#F97316".to_string(),
                color_name: "Signal Orange".to_string(),
                print_temp: 235,
                bed_temp: 75,
                flow_ratio: 0.96,
                pressure_advance: 0.038,
                max_volumetric_speed: 12.0,
                fan_speed_percent: 40,
                remaining_grams: 490,
                is_loaded: true,
            },
            FilamentSpool {
                id: "spool_pla_matte".to_string(),
                name: "PLA Matte Charcoal".to_string(),
                brand: "Polymaker".to_string(),
                material_type: "PLA".to_string(),
                color_hex: "#334155".to_string(),
                color_name: "Slate Dark".to_string(),
                print_temp: 205,
                bed_temp: 55,
                flow_ratio: 0.97,
                pressure_advance: 0.022,
                max_volumetric_speed: 15.0,
                fan_speed_percent: 100,
                remaining_grams: 950,
                is_loaded: true,
            },
            FilamentSpool {
                id: "spool_tpu_sunlu".to_string(),
                name: "Sunlu TPU 95A".to_string(),
                brand: "Sunlu".to_string(),
                material_type: "TPU".to_string(),
                color_hex: "#10B981".to_string(),
                color_name: "Emerald Green".to_string(),
                print_temp: 220,
                bed_temp: 45,
                flow_ratio: 1.02,
                pressure_advance: 0.045,
                max_volumetric_speed: 4.5,
                fan_speed_percent: 50,
                remaining_grams: 310,
                is_loaded: true,
            },
            FilamentSpool {
                id: "spool_abs_poly".to_string(),
                name: "Polymaker PolyLite ABS".to_string(),
                brand: "Polymaker".to_string(),
                material_type: "ABS".to_string(),
                color_hex: "#EC4899".to_string(),
                color_name: "Hot Pink".to_string(),
                print_temp: 250,
                bed_temp: 105,
                flow_ratio: 0.95,
                pressure_advance: 0.020,
                max_volumetric_speed: 15.0,
                fan_speed_percent: 20,
                remaining_grams: 740,
                is_loaded: false,
            },
        ]
    }
}
