use crate::catalog::{load_merged_catalog, CatalogFilament};
use crate::components::color_picker::{ColorPicker, ColorSelection};
use crate::components::wizard_tab::render_issue_illustration;
use crate::models::FilamentSpool;
use crate::transformations::{
  ParameterizedCalibrationTemplate, TransformationEngine, TransformationRule,
};
use std::collections::HashMap;
use web_sys::HtmlInputElement;
use yew::prelude::*;

fn extract_event_value(e: &Event) -> Option<String> {
  let target = e.target()?;
  js_sys::Reflect::get(&target, &wasm_bindgen::JsValue::from_str("value"))
    .ok()
    .and_then(|v| v.as_string())
}

#[derive(Properties, PartialEq)]
pub struct FilamentTabProps {
  pub is_config_loaded: bool,
  pub filaments: Vec<FilamentSpool>,
  pub active_filament_type: String,
  pub active_nozzle_type: String,
  pub ace_pro_units: usize, // 1 = 4 slots, 2 = 8 slots Double ACE Pro 2
  pub flush_matrix: Vec<String>,
  pub disk_flush_matrix: Vec<String>,
  pub disk_flush_slot_count: usize,
  pub flush_multiplier: f64,
  pub disk_flush_multiplier: f64,
  pub melt_zone_multiplier: f64,
  pub on_change_flush_multiplier: Callback<f64>,
  pub on_stage_flush_to_disk: Callback<()>,
  pub on_select_active_filament: Callback<String>,
  pub on_change_ace_units: Callback<usize>,
  pub on_update_spool: Callback<FilamentSpool>,
  pub on_swap_slot_spool: Callback<(usize, usize)>,
  pub on_add_spool: Callback<FilamentSpool>,
  pub on_remove_spool: Callback<usize>,
  pub on_clear_spools: Callback<()>,
  pub on_create_rule: Callback<TransformationRule>,
  pub on_export_spool_json: Callback<FilamentSpool>,
  pub on_export_3mf: Callback<()>,
  pub on_open_bind_modal: Callback<()>,
}

#[function_component]
pub fn FilamentTab(props: &FilamentTabProps) -> Html {
  let selected_slot_idx = use_state(|| 0usize);
  let show_raw_disk_json = use_state(|| false);
  let added_spools_search = use_state(String::new);
  let filament_catalog_search = use_state(String::new);
  let filament_category_filter = use_state(|| "All".to_string());
  // "diff" = Before (Disk) -> After (Lua), "lua" = Lua Output Only, "disk" = Imported Disk Only
  let matrix_view_mode = use_state(|| "diff".to_string());
  let qa_templates = use_memo((), |_| {
    TransformationEngine::get_parameterized_calibration_templates()
  });
  let qa_tree_cat = use_state(|| "all".to_string());
  let active_qa_id = use_state(|| None::<String>);
  let qa_answers = use_state(HashMap::<String, String>::new);

  if !props.is_config_loaded {
    return html! {
      <div class="flex flex-col items-center justify-center bg-surface-container-low p-space-xl rounded-2xl border border-outline-variant gap-space-md text-center my-8">
        <span class="material-symbols-outlined text-primary text-5xl">{"swap_horiz"}</span>
        <h2 class="font-headline-lg text-headline-lg text-on-surface font-bold">
          {"No Configuration Loaded Yet"}
        </h2>
        <p class="text-body-md text-on-surface-variant max-w-xl">
          {"Nothing is loaded. Use the startup Import Modal (or click Reset All in the top right) to import your configuration directory or start from scratch."}
        </p>
      </div>
    };
  }

  let ams_capacity = (props.ace_pro_units * 4).max(1);
  let slot_count = ams_capacity.min(props.filaments.len());
  let active_slots: Vec<FilamentSpool> = props.filaments.iter().take(slot_count).cloned().collect();

  let on_select = props.on_select_active_filament.clone();
  let on_change_ace = props.on_change_ace_units.clone();
  let on_update_spool = props.on_update_spool.clone();
  let on_swap_slot = props.on_swap_slot_spool.clone();
  let on_add_spool = props.on_add_spool.clone();
  let on_remove_spool = props.on_remove_spool.clone();
  let on_clear_spools = props.on_clear_spools.clone();
  let on_export_spool = props.on_export_spool_json.clone();
  let active_noz = props.active_nozzle_type.clone();
  let catalog = use_memo((), |_| load_merged_catalog());
  let material_choices: Vec<String> = {
    let mut choices = Vec::new();
    for f in &catalog.filaments {
      let base = f.base_material_code().to_string();
      if !choices
        .iter()
        .any(|c: &String| c.eq_ignore_ascii_case(&base))
      {
        choices.push(base);
      }
    }
    choices
  };

  let editing_spool = props
    .filaments
    .get(*selected_slot_idx)
    .or_else(|| props.filaments.first())
    .cloned();

  let disk_n = props.disk_flush_slot_count;
  let has_disk_matrix = disk_n > 0 && props.disk_flush_matrix.len() >= disk_n * disk_n;
  let mut changed_cells = 0usize;
  let mut comparable_cells = 0usize;
  let mut cross_polymer_boosts = 0usize;
  let mut disk_sum = 0i64;
  let mut lua_sum = 0i64;

  for i in 0..slot_count {
    for j in 0..slot_count {
      if i == j {
        continue;
      }
      let lua_idx = i * slot_count + j;
      let lua_val: i32 = props
        .flush_matrix
        .get(lua_idx)
        .and_then(|s| s.parse().ok())
        .unwrap_or(240);
      if has_disk_matrix && i < disk_n && j < disk_n {
        let disk_idx = i * disk_n + j;
        let disk_val: i32 = props
          .disk_flush_matrix
          .get(disk_idx)
          .and_then(|s| s.parse().ok())
          .unwrap_or(0);
        comparable_cells += 1;
        disk_sum += disk_val as i64;
        lua_sum += lua_val as i64;
        if (lua_val - disk_val).abs() > 0 {
          changed_cells += 1;
        }
        if active_slots[i].material_type != active_slots[j].material_type && lua_val > disk_val {
          cross_polymer_boosts += 1;
        }
      }
    }
  }

  let avg_disk = if comparable_cells > 0 {
    disk_sum as f64 / comparable_cells as f64
  } else {
    0.0
  };
  let avg_lua = if comparable_cells > 0 {
    lua_sum as f64 / comparable_cells as f64
  } else {
    0.0
  };
  let on_change_mult = props.on_change_flush_multiplier.clone();
  let on_stage_flush = props.on_stage_flush_to_disk.clone();
  let view_mode = (*matrix_view_mode).clone();

  let fil_q = (*filament_catalog_search).to_lowercase();
  let cat_filter = (*filament_category_filter).clone();
  let filtered_catalog: Vec<CatalogFilament> = catalog
    .filaments
    .iter()
    .filter(|f| {
      let cat_ok = cat_filter == "All" || f.category == cat_filter;
      let q_ok = fil_q.is_empty()
        || f.code.to_lowercase().contains(&fil_q)
        || f.label.to_lowercase().contains(&fil_q)
        || f.category.to_lowercase().contains(&fil_q)
        || f.default_finish.to_lowercase().contains(&fil_q);
      cat_ok && q_ok
    })
    .cloned()
    .collect();
  let spool_search = (*added_spools_search).to_lowercase();
  let categories = [
    "All",
    "PLA Family",
    "PETG / PCTG / PET",
    "PC / Engineering",
    "ABS / ASA",
    "Nylon (PA / PAHT / PPA)",
    "TPU / TPE / PEBA Flex",
    "PVA / Support",
  ];
  let nt_low = active_noz.to_lowercase();
  let noz_temp_offset =
    if nt_low.contains("hardened") || nt_low.contains("stainless") || nt_low.contains("cm2") {
      5
    } else if nt_low.contains("diamondback") {
      -10
    } else {
      0
    };

  html! {
    <div class="flex flex-col w-full gap-space-lg">
      // Physical AMS / ACE Pro / Double ACE Pro 2 Bank Header
      <div class="flex flex-wrap items-center justify-between gap-space-md bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant">
        <div class="flex items-center gap-space-md flex-wrap">
          <span class="material-symbols-outlined text-primary text-2xl">{"swap_horiz"}</span>
          <div class="flex flex-col">
            <div class="flex items-center gap-space-sm flex-wrap">
              <span class="font-headline-sm text-headline-sm text-on-surface font-semibold">
                {if props.ace_pro_units == 2 {
                  "Anycubic Double ACE Pro 2 — Spool Loadout & Polymer Catalog (8 Active Slots + Unlimited Reserve)"
                } else {
                  "Anycubic ACE Pro — Spool Loadout & Polymer Catalog (4 Active Slots + Unlimited Reserve)"
                }}
              </span>
              <span class="bg-tertiary-container/30 text-tertiary text-xs px-2 py-0.5 rounded-full font-mono">
                {format!(
                  "{} Total Spools ({} in AMS Slots{}{})",
                  props.filaments.len(),
                  slot_count,
                  if props.filaments.len() > ams_capacity {
                    format!(" • {} Reserve", props.filaments.len() - ams_capacity)
                  } else {
                    String::new()
                  },
                  ""
                )}
              </span>
            </div>
            <span class="text-xs text-on-surface-variant">
              {"Add, color-pick, rename, reorder (↑/↓), and remove (✕) spools freely — even exceeding the AMS slot count. Click any spool row to inspect deltas or run Print Quality & Calibration Wizards."}
            </span>
          </div>
        </div>

        <div class="flex items-center gap-space-sm flex-wrap">
          // 4-Slot vs 8-Slot Double ACE Pro 2 Switcher
          <div class="flex items-center bg-surface-container-lowest p-0.5 rounded-lg border border-outline-variant text-xs font-mono">
            <button
              class={format!(
                "px-space-md py-1 rounded transition-colors {}",
                if props.ace_pro_units == 1 {
                  "bg-primary text-on-primary font-bold"
                } else {
                  "text-on-surface-variant hover:text-on-surface"
                }
              )}
              onclick={
                let cb = on_change_ace.clone();
                Callback::from(move |_| cb.emit(1))
              }
            >
              {"Single ACE (4 Slots)"}
            </button>
            <button
              class={format!(
                "px-space-md py-1 rounded transition-colors {}",
                if props.ace_pro_units == 2 {
                  "bg-primary text-on-primary font-bold"
                } else {
                  "text-on-surface-variant hover:text-on-surface"
                }
              )}
              onclick={
                let cb = on_change_ace.clone();
                Callback::from(move |_| cb.emit(2))
              }
            >
              {"Double ACE Pro 2 (8 Slots)"}
            </button>
          </div>

          <button
            class="flex items-center gap-space-xs bg-surface-container-highest hover:bg-surface-bright text-primary border border-primary/50 px-space-md py-space-xs rounded-lg text-xs font-semibold transition-colors cursor-pointer"
            title="Add a new filament spool to the loadout"
            onclick={{
              let cb = on_add_spool.clone();
              let sel_slot = selected_slot_idx.clone();
              let new_idx = props.filaments.len();
              let next_idx = new_idx + 1;
              let noz = active_noz.clone();
              let default_petg = catalog
                .filaments
                .iter()
                .find(|f| f.code == "PETG")
                .cloned();
              Callback::from(move |_| {
                let sp = if let Some(ref mat) = default_petg {
                  FilamentSpool::from_catalog_entry(next_idx, mat, &noz)
                } else {
                  let mut fallback = FilamentSpool {
                    id: format!("spool_custom_{}", next_idx),
                    name: format!("OC - Anycubic PETG #{}", next_idx),
                    brand: "Anycubic".to_string(),
                    inherits: "Anycubic PETG @Anycubic Kobra S1 0.4 nozzle".to_string(),
                    material_type: "PETG".to_string(),
                    nozzle_type: noz.clone(),
                    tuned_for_nozzle_type: noz.clone(),
                    required_nozzle_hrc: 3,
                    color_hex: "#8E9299".to_string(),
                    color_name: "Cool Gray".to_string(),
                    brass_base_temp: 230,
                    hs_base_temp: 235,
                    base_print_temp: 235,
                    print_temp: 235,
                    base_bed_temp: 70,
                    bed_temp: 70,
                    flow_ratio: 0.95,
                    pressure_advance: 0.045,
                    max_volumetric_speed: 13.0,
                    fan_speed_percent: 40,
                    remaining_grams: 1000,
                    is_loaded: true,
                    finish_type: "Basic".to_string(),
                    opacity: 1.0,
                    pigment_stickiness: 1.0,
                    drying_temp_c: 55,
                    drying_time_hr: 6,
                    is_rfid_synced: false,
                    deltas_from_base: Vec::new(),
                    raw_disk_json: None,
                  };
                  fallback.apply_material_change("PETG", &noz);
                  fallback
                };
                cb.emit(sp);
                sel_slot.set(new_idx);
              })
            }}
          >
            <span class="material-symbols-outlined text-[16px]">{"add_circle"}</span>
            {"+ Add Spool"}
          </button>

          <button
            class="flex items-center gap-space-xs bg-primary-container hover:bg-primary text-on-primary-container px-space-md py-space-xs rounded-lg text-xs font-semibold transition-colors cursor-pointer"
            onclick={
              let cb = props.on_export_3mf.clone();
              Callback::from(move |_| cb.emit(()))
            }
          >
            <span class="material-symbols-outlined text-[16px]">{"deployed_code"}</span>
            {format!("Export {}-Slot .3mf Container", slot_count)}
          </button>
        </div>
      </div>

      // UNIFIED SPOOL LOADOUT & POLYMER CATALOG (Consistent across Import and Start-From-Scratch!)
      <div class="flex flex-col gap-space-md bg-surface-container-lowest p-space-lg rounded-xl border border-outline-variant">
        // TOP SECTION (FULL WIDTH): Added Spools / Slots (Supports unlimited spools beyond AMS capacity)
        <div class="flex flex-col gap-space-sm bg-surface-container-low p-space-md rounded-xl border border-outline-variant">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <div class="flex items-center gap-2 flex-wrap">
              <span class="text-xs font-bold text-on-surface uppercase tracking-wider font-mono">
                {format!("1. Added Spools / Slots ({})", props.filaments.len())}
              </span>
              <span class="text-[11px] font-mono text-on-surface-variant">
                {format!(
                  "Slots #1–#{} feed the active AMS/ACE hub; spools #{} and above are reserve bank spools (use ↑/↓ to swap into AMS slots).",
                  ams_capacity,
                  ams_capacity + 1
                )}
              </span>
            </div>
            <div class="flex items-center gap-3">
              {if props.filaments.len() > 1 {
                html! {
                  <div class="relative w-48 sm:w-64">
                    <span class="material-symbols-outlined text-sm text-on-surface-variant absolute left-2.5 top-1/2 -translate-y-1/2">{"search"}</span>
                    <input
                      type="text"
                      value={(*added_spools_search).clone()}
                      placeholder="Filter spools..."
                      class="w-full bg-surface-container-lowest border border-outline-variant rounded-lg pl-8 pr-7 py-1 text-xs text-on-surface font-mono"
                      oninput={
                        let ass = added_spools_search.clone();
                        Callback::from(move |e: InputEvent| {
                          let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                          ass.set(input.value());
                        })
                      }
                    />
                  </div>
                }
              } else {
                html! {}
              }}
              {if !props.filaments.is_empty() {
                let cb_clear = on_clear_spools.clone();
                let sel_slot = selected_slot_idx.clone();
                html! {
                  <button
                    onclick={Callback::from(move |_| {
                      cb_clear.emit(());
                      sel_slot.set(0);
                    })}
                    class="text-[11px] font-mono text-error hover:underline cursor-pointer"
                  >
                    {"Clear All"}
                  </button>
                }
              } else {
                html! {}
              }}
            </div>
          </div>

          {if props.filaments.is_empty() {
            html! {
              <div class="flex items-center justify-center border border-dashed border-outline-variant rounded-xl py-4 px-space-md text-center gap-2">
                <span class="material-symbols-outlined text-xl text-on-surface-variant/60">{"inventory_2"}</span>
                <span class="text-xs text-on-surface-variant">
                  {"No filaments in loadout — click "}
                  <strong class="text-primary font-mono">{"+ Add"}</strong>
                  {" on any polymer in the Polymer Catalog below to add a spool, then click its color square to pick a color."}
                </span>
              </div>
            }
          } else {
            let total_len = props.filaments.len();
            html! {
              <div class="flex flex-col gap-1.5 max-h-80 overflow-y-auto pr-1">
                {for props.filaments.iter().enumerate().filter(|(_, sp)| {
                  spool_search.is_empty()
                    || sp.name.to_lowercase().contains(&spool_search)
                    || sp.material_type.to_lowercase().contains(&spool_search)
                    || sp.finish_type.to_lowercase().contains(&spool_search)
                }).map(|(idx, spool)| {
                  let is_selected_slot = *selected_slot_idx == idx;
                  let is_active_mat = spool.material_type.eq_ignore_ascii_case(&props.active_filament_type);
                  let in_ams = idx < ams_capacity;
                  let unit_num = if idx < 4 { 1 } else { 2 };
                  let sel_slot = selected_slot_idx.clone();
                  let sp_col = spool.clone();
                  let cb_col = on_update_spool.clone();
                  let sp_name = spool.clone();
                  let cb_name = on_update_spool.clone();
                  let sp_mat = spool.clone();
                  let cb_mat_upd = on_update_spool.clone();
                  let noz_for_mat = active_noz.clone();
                  let mat_options = material_choices.clone();
                  let sp_fin = spool.clone();
                  let cb_fin = on_update_spool.clone();
                  let noz_for_fin = active_noz.clone();
                  let cb_swap_up = on_swap_slot.clone();
                  let cb_swap_down = on_swap_slot.clone();
                  let cb_del = on_remove_spool.clone();
                  let sel_after_del = selected_slot_idx.clone();
                  let cur_sel = *selected_slot_idx;
                  let sp_export = spool.clone();
                  let cb_exp = on_export_spool.clone();
                  let mat_act = spool.material_type.clone();
                  let cb_set_act = on_select.clone();

                  html! {
                    <div
                      onclick={Callback::from(move |_| sel_slot.set(idx))}
                      class={format!(
                        "flex flex-wrap lg:flex-nowrap items-center justify-between px-3 py-2 rounded-lg border gap-2.5 font-mono text-xs transition-all cursor-pointer {}",
                        if is_selected_slot {
                          "bg-surface-container border-primary shadow-md ring-1 ring-primary/40"
                        } else {
                          "bg-surface-container-lowest border-outline-variant hover:border-surface-bright"
                        }
                      )}
                    >
                      <div class="flex items-center gap-2 min-w-0 flex-1">
                        <span
                          class={format!(
                            "px-2 py-0.5 rounded font-bold shrink-0 text-xs {}",
                            if in_ams {
                              "bg-primary/20 text-primary"
                            } else {
                              "bg-surface-container-highest text-on-surface-variant"
                            }
                          )}
                          title={if in_ams {
                            format!("Active AMS Slot #{} (ACE #{})", idx + 1, unit_num)
                          } else {
                            "Reserve Spool (use ↑ to move into an active AMS slot)".to_string()
                          }}
                        >
                          {if in_ams {
                            format!("#{} ACE#{}", idx + 1, unit_num)
                          } else {
                            format!("#{} Reserve", idx + 1)
                          }}
                        </span>

                        // Compact Color Swatch Square
                        <div onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                          <ColorPicker
                            compact=true
                            value_hex={spool.color_hex.clone()}
                            title_label={Some(format!("Slot #{} — {} Color", idx + 1, spool.name))}
                            on_change={Callback::from(move |sel: ColorSelection| {
                              let mut updated = sp_col.clone();
                              updated.color_hex = sel.hex;
                              updated.color_name = sel.name;
                              cb_col.emit(updated);
                            })}
                          />
                        </div>

                        <input
                          type="text"
                          value={spool.name.clone()}
                          onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                          class="bg-surface-container-low border border-outline-variant/60 rounded px-2.5 py-1 text-on-surface flex-1 min-w-[140px] text-xs"
                          oninput={Callback::from(move |e: InputEvent| {
                            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                            let mut updated = sp_name.clone();
                            updated.name = input.value();
                            cb_name.emit(updated);
                          })}
                        />

                        // Material Type Selector
                        <select
                          onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                          class="bg-surface-container-high border border-outline-variant/60 rounded px-2 py-1 text-xs font-bold text-primary cursor-pointer shrink-0"
                          value={spool.material_type.clone()}
                          onchange={Callback::from(move |e: Event| {
                            e.stop_propagation();
                            if let Some(new_mat) = extract_event_value(&e) {
                              let mut updated = sp_mat.clone();
                              updated.apply_material_change(&new_mat, &noz_for_mat);
                              cb_mat_upd.emit(updated);
                            }
                          })}
                        >
                          {for mat_options.iter().map(|m_opt| {
                            html! {
                              <option value={m_opt.clone()} selected={spool.material_type.eq_ignore_ascii_case(m_opt)}>
                                {m_opt}
                              </option>
                            }
                          })}
                        </select>
                      </div>

                      <div
                        class="flex items-center gap-1.5 flex-wrap shrink-0"
                        onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                      >
                        // Finish Selector
                        <select
                          class="bg-surface-container-low border border-outline-variant/60 rounded px-2 py-1 text-xs text-on-surface cursor-pointer"
                          value={spool.finish_type.clone()}
                          onchange={Callback::from(move |e: Event| {
                            if let Some(fin) = extract_event_value(&e) {
                              let mut updated = sp_fin.clone();
                              updated.apply_finish_change(&fin, &noz_for_fin);
                              cb_fin.emit(updated);
                            }
                          })}
                        >
                          <option value="Basic" selected={spool.finish_type == "Basic"}>{"Basic"}</option>
                          <option value="Matte" selected={spool.finish_type == "Matte"}>{"Matte"}</option>
                          <option value="Silk" selected={spool.finish_type == "Silk"}>{"Silk"}</option>
                          <option value="Translucent" selected={spool.finish_type == "Translucent"}>{"Translucent"}</option>
                          <option value="Carbon Fiber" selected={spool.finish_type == "Carbon Fiber"}>{"Carbon Fiber (-CF)"}</option>
                          <option value="Glass Fiber" selected={spool.finish_type == "Glass Fiber"}>{"Glass Fiber (-GF)"}</option>
                        </select>

                        <span class="text-xs text-tertiary bg-tertiary-container/20 px-2 py-0.5 rounded shrink-0">
                          {format!("{}°C / {}°C", spool.print_temp, spool.bed_temp)}
                        </span>

                        <span
                          class="text-[10px] bg-surface-container-high text-on-surface-variant px-1.5 py-0.5 rounded shrink-0"
                          title="Click row to inspect deltas & wizards below"
                        >
                          {format!("{} Δ", spool.deltas_from_base.len())}
                        </span>

                        {if is_active_mat {
                          html! {
                            <span class="text-[10px] bg-primary/20 text-primary px-2 py-0.5 rounded font-bold">
                              {"Active"}
                            </span>
                          }
                        } else {
                          html! {
                            <button
                              onclick={Callback::from(move |_| cb_set_act.emit(mat_act.clone()))}
                              class="text-[10px] bg-surface-container-highest hover:bg-surface-bright text-on-surface px-2 py-0.5 rounded font-semibold cursor-pointer"
                            >
                              {"Set Active"}
                            </button>
                          }
                        }}

                        <button
                          title="Export standalone filament .json"
                          onclick={Callback::from(move |_| cb_exp.emit(sp_export.clone()))}
                          class="px-2 py-1 rounded border border-outline-variant text-secondary hover:bg-surface-container-high text-[11px] cursor-pointer"
                        >
                          {".json"}
                        </button>

                        // Reorder Up (`↑`)
                        <button
                          disabled={idx == 0}
                          title="Move spool up"
                          onclick={Callback::from(move |_| {
                            if idx > 0 {
                              cb_swap_up.emit((idx, idx - 1));
                            }
                          })}
                          class={format!(
                            "px-2 py-1 rounded border text-xs {}",
                            if idx == 0 {
                              "border-outline-variant/30 text-on-surface-variant/30 cursor-not-allowed"
                            } else {
                              "border-outline-variant text-on-surface hover:bg-surface-container-high cursor-pointer"
                            }
                          )}
                        >
                          {"↑"}
                        </button>

                        // Reorder Down (`↓`)
                        <button
                          disabled={idx + 1 >= total_len}
                          title="Move spool down"
                          onclick={Callback::from(move |_| {
                            if idx + 1 < total_len {
                              cb_swap_down.emit((idx, idx + 1));
                            }
                          })}
                          class={format!(
                            "px-2 py-1 rounded border text-xs {}",
                            if idx + 1 >= total_len {
                              "border-outline-variant/30 text-on-surface-variant/30 cursor-not-allowed"
                            } else {
                              "border-outline-variant text-on-surface hover:bg-surface-container-high cursor-pointer"
                            }
                          )}
                        >
                          {"↓"}
                        </button>

                        // Remove (`✕`)
                        <button
                          title="Remove spool"
                          onclick={Callback::from(move |_| {
                            cb_del.emit(idx);
                            if cur_sel >= idx && cur_sel > 0 {
                              sel_after_del.set(cur_sel - 1);
                            }
                          })}
                          class="px-2 py-1 rounded border border-error/40 text-error hover:bg-error/20 text-xs cursor-pointer"
                        >
                          {"✕"}
                        </button>
                      </div>
                    </div>
                  }
                })}
              </div>
            }
          }}
        </div>

        // BOTTOM SECTION (FULL WIDTH): Searchable Polymer & Composite Catalog
        <div class="flex flex-col gap-space-sm bg-surface-container-low p-space-md rounded-xl border border-outline-variant">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <span class="text-xs font-bold text-on-surface uppercase tracking-wider font-mono">
              {format!("2. Polymer Catalog ({} Materials — Click \"+ Add\" to Add Spool)", filtered_catalog.len())}
            </span>
            <div class="relative w-full sm:w-72">
              <span class="material-symbols-outlined text-sm text-on-surface-variant absolute left-2.5 top-1/2 -translate-y-1/2">{"search"}</span>
              <input
                type="text"
                value={(*filament_catalog_search).clone()}
                placeholder="Search PEBA, PVA, PHTA, PPA, Rebound TPU, -CF, -GF..."
                class="w-full bg-surface-container-lowest border border-outline-variant rounded-lg pl-8 pr-7 py-1 text-xs text-on-surface font-mono"
                oninput={
                  let fcs = filament_catalog_search.clone();
                  Callback::from(move |e: InputEvent| {
                    let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                    fcs.set(input.value());
                  })
                }
              />
              {if !filament_catalog_search.is_empty() {
                let fcs = filament_catalog_search.clone();
                html! {
                  <button
                    onclick={Callback::from(move |_| fcs.set(String::new()))}
                    class="absolute right-2 top-1/2 -translate-y-1/2 text-on-surface-variant hover:text-on-surface text-xs"
                  >{"✕"}</button>
                }
              } else {
                html! {}
              }}
            </div>
          </div>

          // Category Pills
          <div class="flex items-center gap-1 flex-wrap">
            {for categories.iter().map(|cat| {
              let active = *filament_category_filter == *cat;
              let cat_str = cat.to_string();
              let fcf = filament_category_filter.clone();
              html! {
                <button
                  onclick={Callback::from(move |_| fcf.set(cat_str.clone()))}
                  class={format!(
                    "px-2.5 py-0.5 rounded-full text-[11px] font-mono border transition-colors cursor-pointer {}",
                    if active {
                      "bg-primary text-on-primary border-primary font-bold"
                    } else {
                      "bg-surface-container-lowest text-on-surface-variant border-outline-variant hover:text-on-surface"
                    }
                  )}
                >
                  {*cat}
                </button>
              }
            })}
          </div>

          // Catalog Grid with "+ Add" Button
          <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-2 max-h-72 overflow-y-auto pr-1">
            {for filtered_catalog.iter().map(|mat| {
              let mat_clone = mat.clone();
              let cb_add = on_add_spool.clone();
              let sel_slot = selected_slot_idx.clone();
              let new_idx = props.filaments.len();
              let next_idx = new_idx + 1;
              let noz = active_noz.clone();
              let eff_noz = (mat.base_nozzle_c as i32 + noz_temp_offset).clamp(175, 350);
              let eff_bed = mat.base_bed_c as i32;
              let warn_brass = mat.requires_hardened
                && !nt_low.contains("hardened")
                && !nt_low.contains("diamondback")
                && !nt_low.contains("ruby")
                && !nt_low.contains("cm2");
              let is_ruby_cfgf = mat.requires_hardened && nt_low.contains("ruby");

              html! {
                <div class="flex items-center justify-between bg-surface-container-lowest p-2.5 rounded-lg border border-outline-variant/60 hover:border-outline gap-2">
                  <div class="flex flex-col min-w-0">
                    <div class="flex items-center gap-1.5 flex-wrap">
                      <span class="text-xs font-bold text-on-surface font-mono">{mat.base_material_code()}</span>
                      <span class="text-[10px] font-mono px-1.5 py-0.5 rounded bg-surface-container-high text-on-surface-variant">
                        {&mat.default_finish}
                      </span>
                      {if warn_brass {
                        html! {
                          <span
                            title="Abrasive fiber/glow filament — Hardened, Ruby Tip, or Diamondback nozzle recommended"
                            class="text-[10px] bg-amber-500/20 text-amber-300 px-1.5 py-0.5 rounded font-mono"
                          >
                            {"Abrasive"}
                          </span>
                        }
                      } else if is_ruby_cfgf {
                        html! {
                          <span
                            title="Ruby gemstone tip handles -CF and -GF composite fibers with 0°C Brass thermal conductivity"
                            class="text-[10px] bg-tertiary-container/30 text-tertiary px-1.5 py-0.5 rounded font-mono"
                          >
                            {"Ruby CF/GF Ready"}
                          </span>
                        }
                      } else {
                        html! {}
                      }}
                    </div>
                    <span class="text-[11px] text-on-surface-variant truncate" title={mat.label.clone()}>
                      {&mat.label}
                    </span>
                    <span class="text-[10px] font-mono text-tertiary">
                      {format!(
                        "{}°C / {}°C • {:.1} mm³/s • Fan {}%",
                        eff_noz, eff_bed, mat.max_vol_mm3s, mat.fan_pct
                      )}
                    </span>
                  </div>

                  <button
                    onclick={Callback::from(move |_| {
                      let sp = FilamentSpool::from_catalog_entry(next_idx, &mat_clone, &noz);
                      cb_add.emit(sp);
                      sel_slot.set(new_idx);
                    })}
                    class="bg-primary/20 hover:bg-primary text-primary hover:text-on-primary border border-primary/40 px-3 py-1.5 rounded-lg text-xs font-bold font-mono shrink-0 transition-colors cursor-pointer"
                  >
                    {"+ Add"}
                  </button>
                </div>
              }
            })}
          </div>
        </div>
      </div>

      // Transformation Deltas from System Base Inspector (Requirement #2)
      if let Some(spool) = editing_spool {
        <div class="flex flex-col gap-space-md bg-surface-container-low p-space-xl rounded-xl shadow-md border border-outline-variant">
          <div class="flex items-center justify-between flex-wrap gap-space-sm">
            <div class="flex items-center gap-space-sm">
              <span class="material-symbols-outlined text-primary">{"rebase_edit"}</span>
              <div class="flex flex-col">
                <h3 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
                  {format!("Imported Spool Transformation Analysis: {}", spool.format_concise_title())}
                </h3>
                <span class="text-xs font-mono text-secondary">
                  {format!(
                    "Inherits Base: \"{}\" • Target Nozzle: {} (Required HRC ≥ {} • Brass Base: {}°C / HS Base: {}°C)",
                    spool.inherits,
                    spool.nozzle_type,
                    spool.required_nozzle_hrc,
                    spool.brass_base_temp,
                    spool.hs_base_temp
                  )}
                </span>
                if let Some(warn) = spool.compatibility_warning(
                  &spool.nozzle_type,
                  if spool.nozzle_type.contains("diamondback") {
                    68
                  } else if spool.nozzle_type.contains("ruby") {
                    60
                  } else if spool.nozzle_type.contains("hardened") || spool.nozzle_type.contains("cm2") {
                    55
                  } else if spool.nozzle_type.contains("microswiss_brass") {
                    35
                  } else {
                    0
                  }
                ) {
                  <span class="text-xs font-mono text-error font-bold mt-1">
                    {format!("⚠️ {}", warn)}
                  </span>
                }
              </div>
            </div>

            <button
              class="flex items-center gap-1 bg-surface-container-high hover:bg-surface-bright text-on-surface px-space-md py-1 rounded text-xs font-mono transition-colors"
              onclick={
                let srd = show_raw_disk_json.clone();
                Callback::from(move |_| srd.set(!*srd))
              }
            >
              <span class="material-symbols-outlined text-[14px]">{"code"}</span>
              {if *show_raw_disk_json { "Hide Raw Disk .json" } else { "Compare Our Model vs Raw Disk .json" }}
            </button>
          </div>

          // Transformation Deltas Table (System Base -> User Disk -> Reconstructed Rule)
          <div class="flex flex-col gap-space-xs">
            <span class="text-xs font-semibold text-on-surface-variant">
              {"Values Understood as Transformations from System Base Profile:"}
            </span>
            if spool.deltas_from_base.is_empty() {
              <div class="bg-surface-container-lowest p-space-md rounded text-xs font-mono text-tertiary">
                {"✓ Zero overrides relative to system base profile — 100% stock inheritance."}
              </div>
            } else {
              <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-space-sm">
                {for spool.deltas_from_base.iter().map(|delta| {
                  html! {
                    <div class="bg-surface-container p-space-md rounded-lg border border-outline-variant flex flex-col justify-between gap-1 font-mono text-xs">
                      <div class="flex items-center justify-between gap-1 flex-wrap">
                        <span class="text-on-surface font-bold">{&delta.param_key}</span>
                        <span
                          class={format!(
                            "px-1.5 py-0.5 rounded text-[10px] font-bold {}",
                            if delta.reconstructed_by_lua {
                              "bg-tertiary/20 text-tertiary"
                            } else {
                              "bg-error/20 text-error"
                            }
                          )}
                        >
                          {delta.rule_badge_label()}
                        </span>
                      </div>
                      <div class="flex items-center gap-2 text-xs my-1">
                        <span class="text-on-surface-variant">{format!("Base: {}", delta.base_value)}</span>
                        <span class="text-primary">{"→"}</span>
                        <span class="text-on-surface font-bold">{format!("Disk: {}", delta.disk_value)}</span>
                      </div>
                      <span class="text-[11px] text-secondary">{&delta.rule_explanation}</span>
                    </div>
                  }
                })}
              </div>
            }
          </div>

          if *show_raw_disk_json {
            <div class="grid grid-cols-1 lg:grid-cols-2 gap-space-md pt-space-sm border-t border-surface-container">
              <div class="flex flex-col gap-1 bg-surface-container-lowest p-space-md rounded-lg border border-primary/40">
                <span class="text-xs font-mono uppercase text-primary font-bold">
                  {"Our Transformation Spool Struct (Base + Deltas + Optical Metadata)"}
                </span>
                <pre class="text-[11px] font-mono text-on-surface overflow-x-auto max-h-52 whitespace-pre">
                  {serde_json::to_string_pretty(&spool).unwrap_or_default()}
                </pre>
              </div>
              <div class="flex flex-col gap-1 bg-surface-container-lowest p-space-md rounded-lg border border-secondary/40">
                <span class="text-xs font-mono uppercase text-secondary font-bold">
                  {format!("Raw User Disk File (user/434139/filament/{}.json)", spool.name)}
                </span>
                <pre class="text-[11px] font-mono text-on-surface overflow-x-auto max-h-52 whitespace-pre">
                  {spool.raw_disk_json.clone().unwrap_or_else(|| "{}".to_string())}
                </pre>
              </div>
            </div>
          }

          // Slot Material Type & Spool Name Editor (works after Import or Start From Scratch!)
          <div class="grid grid-cols-1 lg:grid-cols-2 gap-space-md bg-surface-container p-space-md rounded-lg font-mono text-xs">
            <div class="flex flex-col gap-1.5">
              <div class="flex items-center justify-between">
                <span class="text-on-surface-variant text-[10px] uppercase font-bold">
                  {format!("Slot #{} Material Type (Polymer Family)", (*selected_slot_idx) + 1)}
                </span>
                <span class="text-primary font-bold">
                  {format!("{} • {}", spool.material_type, spool.finish_type)}
                </span>
              </div>
              <div class="flex flex-wrap items-center gap-1">
                {for ["PLA", "PLA+", "PETG", "PCTG", "PLA-CF", "TPU", "ASA", "ABS"].into_iter().map(|quick_mat| {
                  let is_sel = spool.material_type.eq_ignore_ascii_case(quick_mat);
                  let updated = spool.clone();
                  let noz = active_noz.clone();
                  let qm = quick_mat.to_string();
                  let cb = on_update_spool.clone();
                  html! {
                    <button
                      class={format!(
                        "px-2 py-1 rounded text-[11px] border transition-colors cursor-pointer {}",
                        if is_sel {
                          "bg-primary text-on-primary border-primary font-bold"
                        } else {
                          "bg-surface-container-lowest text-on-surface-variant border-outline-variant hover:text-on-surface"
                        }
                      )}
                      onclick={Callback::from(move |_| {
                        let mut u = updated.clone();
                        u.apply_material_change(&qm, &noz);
                        cb.emit(u);
                      })}
                    >
                      {quick_mat}
                    </button>
                  }
                })}
                <select
                  class="bg-surface-container-lowest border border-outline-variant rounded px-2 py-1 text-[11px] font-mono text-primary font-bold focus:border-primary outline-none cursor-pointer"
                  value={spool.material_type.clone()}
                  onchange={{
                    let u = spool.clone();
                    let noz = active_noz.clone();
                    let cb = on_update_spool.clone();
                    Callback::from(move |e: Event| {
                      if let Some(new_mat) = extract_event_value(&e) {
                        let mut updated = u.clone();
                        updated.apply_material_change(&new_mat, &noz);
                        cb.emit(updated);
                      }
                    })
                  }}
                >
                  {for material_choices.iter().map(|m_opt| {
                    html! {
                      <option value={m_opt.clone()} selected={spool.material_type.eq_ignore_ascii_case(m_opt)}>
                        {format!("Catalog: {}", m_opt)}
                      </option>
                    }
                  })}
                </select>
              </div>
            </div>

            <div class="flex flex-col justify-between gap-1.5">
              <div class="flex items-center justify-between">
                <span class="text-on-surface-variant text-[10px] uppercase font-bold">
                  {"Spool Display Name & Brand"}
                </span>
                <span class="text-secondary text-[10px]">{format!("Inherits: {}", spool.inherits)}</span>
              </div>
              <input
                type="text"
                value={spool.name.clone()}
                placeholder="e.g. OC - Anycubic PCTG Clear"
                class="w-full bg-surface-container-lowest border border-outline-variant rounded px-2.5 py-1.5 text-xs font-mono text-on-surface focus:border-primary outline-none"
                oninput={{
                  let u = spool.clone();
                  let cb = on_update_spool.clone();
                  Callback::from(move |e: InputEvent| {
                    if let Some(inp) = e.target_dyn_into::<HtmlInputElement>() {
                      let mut updated = u.clone();
                      updated.name = inp.value();
                      cb.emit(updated);
                    }
                  })
                }}
              />
            </div>
          </div>

          // Filament Color Picker (Shortlist, HEX, RGB & HSL)
          <div class="flex items-center justify-between flex-wrap gap-2 bg-surface-container p-space-md rounded-lg">
            <span class="text-on-surface-variant text-[10px] font-mono uppercase">
              {format!("Filament Color — Current: {} ({})", spool.color_name, spool.color_hex)}
            </span>
            <crate::components::color_picker::ColorPicker
              value_hex={spool.color_hex.clone()}
              title_label={Some(format!("{} — Filament Color", spool.name))}
              on_change={{
                let u = spool.clone();
                let cb = on_update_spool.clone();
                Callback::from(move |sel: crate::components::color_picker::ColorSelection| {
                  let mut updated = u.clone();
                  updated.color_hex = sel.hex;
                  updated.color_name = sel.name;
                  cb.emit(updated);
                })
              }}
            />
          </div>

          // Extended Optical & Thermal Controls
          <div class="grid grid-cols-1 md:grid-cols-3 lg:grid-cols-6 gap-space-md text-xs font-mono pt-space-xs">
            // Finish Type
            <div class="flex flex-col gap-1 bg-surface-container p-space-md rounded-lg">
              <span class="text-on-surface-variant text-[10px] uppercase">{"Finish Type"}</span>
              <div class="grid grid-cols-2 gap-1 mt-1">
                {for vec!["Basic", "Matte", "Silk", "Translucent", "Carbon Fiber", "Glass Fiber"].into_iter().map(|fin| {
                  let is_sel = spool.finish_type == fin;
                  let mut updated = spool.clone();
                  updated.finish_type = fin.to_string();
                  if fin == "Translucent" && updated.opacity > 0.6 {
                    updated.opacity = 0.35;
                  }
                  let cb = on_update_spool.clone();

                  html! {
                    <button
                      class={format!(
                        "px-1.5 py-1 rounded text-[11px] border {}",
                        if is_sel {
                          "bg-primary text-on-primary border-primary font-bold"
                        } else {
                          "bg-surface-container-lowest text-on-surface-variant border-outline-variant"
                        }
                      )}
                      onclick={Callback::from(move |_| cb.emit(updated.clone()))}
                    >
                      {fin}
                    </button>
                  }
                })}
              </div>
            </div>

            // Opacity (0.1 to 1.0)
            <div class="flex flex-col justify-between bg-surface-container p-space-md rounded-lg">
              <div class="flex justify-between">
                <span class="text-on-surface-variant text-[10px] uppercase">{"Opacity (0.1–1.0)"}</span>
                <span class="text-primary font-bold">{format!("{:.2}", spool.opacity)}</span>
              </div>
              <div class="flex items-center gap-1 mt-2">
                {for vec![0.25f32, 0.50, 0.85, 1.0].into_iter().map(|op| {
                  let mut updated = spool.clone();
                  updated.opacity = op;
                  let cb = on_update_spool.clone();
                  let is_sel = (spool.opacity - op).abs() < 0.05;
                  html! {
                    <button
                      class={format!(
                        "flex-1 py-1 rounded text-[11px] border {}",
                        if is_sel { "bg-secondary/20 border-secondary text-secondary font-bold" } else { "bg-surface-container-lowest border-outline-variant text-on-surface-variant" }
                      )}
                      onclick={Callback::from(move |_| cb.emit(updated.clone()))}
                    >
                      {format!("{:.2}", op)}
                    </button>
                  }
                })}
              </div>
              <span class="text-[10px] text-on-surface-variant mt-1">
                {"Clear spools scale purge up to 1.5x"}
              </span>
            </div>

            // Pigment Stickiness (0.5 to 1.8)
            <div class="flex flex-col justify-between bg-surface-container p-space-md rounded-lg">
              <div class="flex justify-between">
                <span class="text-on-surface-variant text-[10px] uppercase">{"Pigment Stickiness"}</span>
                <span class="text-tertiary font-bold">{format!("{:.2}x", spool.pigment_stickiness)}</span>
              </div>
              <div class="flex items-center gap-1 mt-2">
                {for vec![0.60f32, 1.00, 1.40, 1.75].into_iter().map(|st| {
                  let mut updated = spool.clone();
                  updated.pigment_stickiness = st;
                  let cb = on_update_spool.clone();
                  let is_sel = (spool.pigment_stickiness - st).abs() < 0.08;
                  html! {
                    <button
                      class={format!(
                        "flex-1 py-1 rounded text-[11px] border {}",
                        if is_sel { "bg-tertiary/20 border-tertiary text-tertiary font-bold" } else { "bg-surface-container-lowest border-outline-variant text-on-surface-variant" }
                      )}
                      onclick={Callback::from(move |_| cb.emit(updated.clone()))}
                    >
                      {format!("{:.2}", st)}
                    </button>
                  }
                })}
              </div>
              <span class="text-[10px] text-on-surface-variant mt-1">
                {"Carbon black & red dyes need extra purge"}
              </span>
            </div>

            // Base Nozzle Temperature
            <div class="flex flex-col justify-between bg-surface-container p-space-md rounded-lg">
              <div class="flex justify-between">
                <span class="text-on-surface-variant text-[10px] uppercase">{"Base Nozzle Temp"}</span>
                <span class="text-on-surface font-bold">{format!("{}°C", spool.base_print_temp)}</span>
              </div>
              <div class="flex items-center gap-2 mt-2">
                <button
                  class="flex-1 bg-surface-container-lowest hover:bg-surface-bright py-1 rounded border border-outline-variant"
                  onclick={{
                    let mut u = spool.clone();
                    u.base_print_temp -= 5;
                    u.print_temp -= 5;
                    let cb = on_update_spool.clone();
                    Callback::from(move |_| cb.emit(u.clone()))
                  }}
                >
                  {"-5°C"}
                </button>
                <button
                  class="flex-1 bg-surface-container-lowest hover:bg-surface-bright py-1 rounded border border-outline-variant"
                  onclick={{
                    let mut u = spool.clone();
                    u.base_print_temp += 5;
                    u.print_temp += 5;
                    let cb = on_update_spool.clone();
                    Callback::from(move |_| cb.emit(u.clone()))
                  }}
                >
                  {"+5°C"}
                </button>
              </div>
              <span class="text-[10px] text-on-surface-variant mt-1">
                {format!("Disk effective: {}°C", spool.print_temp)}
              </span>
            </div>

            // Volumetric Speed Limit
            <div class="flex flex-col justify-between bg-surface-container p-space-md rounded-lg">
              <div class="flex justify-between">
                <span class="text-on-surface-variant text-[10px] uppercase">{"Max Vol Speed"}</span>
                <span class="text-secondary font-bold">
                  {format!("{:.1} mm³/s", spool.max_volumetric_speed)}
                </span>
              </div>
              <div class="flex items-center gap-2 mt-2">
                <button
                  class="flex-1 bg-surface-container-lowest hover:bg-surface-bright py-1 rounded border border-outline-variant"
                  onclick={{
                    let mut u = spool.clone();
                    u.max_volumetric_speed = (u.max_volumetric_speed - 2.0).max(3.0);
                    let cb = on_update_spool.clone();
                    Callback::from(move |_| cb.emit(u.clone()))
                  }}
                >
                  {"-2.0"}
                </button>
                <button
                  class="flex-1 bg-surface-container-lowest hover:bg-surface-bright py-1 rounded border border-outline-variant"
                  onclick={{
                    let mut u = spool.clone();
                    u.max_volumetric_speed += 2.0;
                    let cb = on_update_spool.clone();
                    Callback::from(move |_| cb.emit(u.clone()))
                  }}
                >
                  {"+2.0"}
                </button>
              </div>
              <span class="text-[10px] text-on-surface-variant mt-1">
                {"Feeds Volumetric Speed Ceiling"}
              </span>
            </div>

            // ACE Pro Active Dryer Preset
            <div class="flex flex-col justify-between bg-surface-container p-space-md rounded-lg">
              <div class="flex justify-between">
                <span class="text-on-surface-variant text-[10px] uppercase">{"ACE Dryer Preset"}</span>
                <span class="text-primary font-bold">
                  {format!("{}°C / {}h", spool.drying_temp_c, spool.drying_time_hr)}
                </span>
              </div>
              <div class="flex items-center gap-1 mt-2">
                {for vec![(45, 4, "PLA 45°"), (55, 6, "PETG 55°"), (65, 6, "ASA 65°")].into_iter().map(|(dt, dh, lbl)| {
                  let mut u = spool.clone();
                  u.drying_temp_c = dt;
                  u.drying_time_hr = dh;
                  let cb = on_update_spool.clone();
                  html! {
                    <button
                      class="flex-1 bg-surface-container-lowest hover:bg-surface-bright py-1 rounded border border-outline-variant text-[10px]"
                      onclick={Callback::from(move |_| cb.emit(u.clone()))}
                    >
                      {lbl}
                    </button>
                  }
                })}
              </div>
              <span class="text-[10px] text-on-surface-variant mt-1">
                {"Saved in extended metadata"}
              </span>
            </div>
          </div>

          // Inline Filament Symptom & Post-Print Calibration Q&A Wizards (Scoped directly to this Slot's Filament!)
          <div class="flex flex-col gap-space-sm bg-surface-container p-space-md rounded-xl border border-outline-variant/70 font-mono text-xs mt-space-xs">
            <div class="flex items-center justify-between flex-wrap gap-2">
              <div class="flex items-center gap-2">
                <span class="material-symbols-outlined text-secondary text-lg">{"healing"}</span>
                <div class="flex flex-col">
                  <span class="text-on-surface font-bold text-xs">
                    {format!(
                      "Print Quality Issue & Calibration Wizard Tree for Slot #{} ({})",
                      (*selected_slot_idx) + 1,
                      spool.material_type
                    )}
                  </span>
                  <span class="text-[11px] text-on-surface-variant">
                    {"Browse 26 print quality issues across 7 categories (with visual diagrams & Kobra S1 hardware guides) or 7 post-print calibration tests."}
                  </span>
                </div>
              </div>
              if active_qa_id.is_some() {
                <button
                  class="text-[11px] text-on-surface-variant hover:text-on-surface px-2 py-0.5 rounded bg-surface-container-highest cursor-pointer"
                  onclick={{
                    let aqi = active_qa_id.clone();
                    Callback::from(move |_| aqi.set(None))
                  }}
                >
                  {"Close Wizard ✕"}
                </button>
              }
            </div>

            // Category Tree Selector Bar
            <div class="flex flex-wrap items-center gap-1 pt-1 border-t border-outline-variant/40">
              <span class="text-[10px] uppercase text-secondary font-bold mr-1">
                {"Category Tree:"}
              </span>
              {for ParameterizedCalibrationTemplate::symptom_tree_categories()
                .into_iter()
                .chain(std::iter::once(("Post-Print Calibration", "🎯 Calibration Prints (7)", "🎯")))
                .map(|(cat_id, cat_lbl, cat_icon)| {
                  let is_cat_sel = *qa_tree_cat == cat_id;
                  let qtc = qa_tree_cat.clone();
                  let cid = cat_id.to_string();
                  html! {
                    <button
                      class={format!(
                        "px-2 py-1 rounded text-[10px] border transition-colors cursor-pointer {}",
                        if is_cat_sel {
                          "bg-primary text-on-primary border-primary font-bold"
                        } else {
                          "bg-surface-container-lowest text-on-surface-variant border-outline-variant hover:text-on-surface"
                        }
                      )}
                      onclick={Callback::from(move |_| qtc.set(cid.clone()))}
                    >
                      {format!("{} {}", cat_icon, cat_lbl)}
                    </button>
                  }
                })}
            </div>

            // Issue / Calibration Buttons in Selected Category
            <div class="flex flex-wrap items-center gap-1.5 pt-1">
              {for qa_templates
                .iter()
                .filter(|t| {
                  let cur_cat = (*qa_tree_cat).as_str();
                  if cur_cat == "all" {
                    t.is_symptom_wizard()
                  } else {
                    t.tree_category() == cur_cat
                  }
                })
                .map(|t_item| {
                  let tid = t_item.id.as_str();
                  let is_open = (*active_qa_id).as_deref() == Some(tid);
                  let aqi = active_qa_id.clone();
                  let qa_ans = qa_answers.clone();
                  let qa_list = (*qa_templates).clone();
                  let tid_str = t_item.id.clone();
                  let cur_flow_str = format!("{:.2}", spool.flow_ratio);

                  html! {
                    <button
                      class={format!(
                        "px-2.5 py-1.5 rounded-lg border text-[11px] font-bold transition-colors cursor-pointer {}",
                        if is_open {
                          "bg-secondary text-on-secondary border-secondary shadow"
                        } else {
                          "bg-surface-container-lowest text-on-surface hover:border-secondary/60 border-outline-variant"
                        }
                      )}
                      onclick={Callback::from(move |_| {
                        if is_open {
                          aqi.set(None);
                        } else {
                          if let Some(tpl) = qa_list.iter().find(|t| t.id == tid_str) {
                            let mut def = tpl.default_answers();
                            if tid_str == "cal_yolo_flow" {
                              def.insert("old_flow".to_string(), cur_flow_str.clone());
                            }
                            qa_ans.set(def);
                          }
                          aqi.set(Some(tid_str.clone()));
                        }
                      })}
                    >
                      {format!("{} {}", t_item.icon, t_item.name)}
                    </button>
                  }
                })}
            </div>

            {if let Some(open_id) = &*active_qa_id {
              if let Some(tpl) = qa_templates.iter().find(|t| &t.id == open_id).cloned() {
                let scope_expr = format!("filament:{}", spool.material_type);
                let ans_map = (*qa_answers).clone();
                let (rule_title, rule_desc, computed_badge, lua_code) =
                  tpl.generate_from_answers(&ans_map, &scope_expr);
                let cb_create = props.on_create_rule.clone();
                let aqi_close = active_qa_id.clone();
                let tpl_cat = tpl.category.clone();
                let tpl_icon = tpl.icon.clone();
                let tid_clone = tpl.id.clone();
                let mat_name = spool.material_type.clone();
                let mat_name_btn = mat_name.clone();
                let hw_items = tpl.hardware_checklist();

                html! {
                  <div class="flex flex-col gap-space-sm bg-surface-container-lowest p-space-md rounded-xl border border-secondary/60 mt-space-xs">
                    <div class="flex items-center justify-between flex-wrap gap-2">
                      <div class="flex items-center gap-2">
                        <span class="text-lg">{&tpl.icon}</span>
                        <div class="flex flex-col">
                          <span class="text-xs font-bold text-on-surface">{&tpl.name}</span>
                          <span class="text-[10px] text-secondary">
                            {format!("{} → {}  •  {}", tpl.tree_category(), tpl.tree_subcategory(), tpl.when_to_run)}
                          </span>
                        </div>
                      </div>
                      <span class="bg-tertiary/20 text-tertiary border border-tertiary/40 px-2.5 py-1 rounded font-bold text-xs">
                        {format!("Result: {}", computed_badge)}
                      </span>
                    </div>

                    // Visual Defect & Hardware Illustration SVG
                    {render_issue_illustration(&tpl.id, false)}

                    // Physical / Mechanical Printer Checklist
                    {if !hw_items.is_empty() {
                      html! {
                        <div class="bg-secondary/10 border-l-4 border-secondary p-space-sm rounded-r-lg flex flex-col gap-1">
                          <span class="text-[10px] uppercase font-bold text-secondary">
                            {"🔧 Physical & Mechanical Printer Checks (Anycubic Kobra S1 / ACE Pro):"}
                          </span>
                          {for hw_items.iter().map(|hw_line| {
                            html! {
                              <span class="text-[11px] text-on-surface leading-snug">{*hw_line}</span>
                            }
                          })}
                        </div>
                      }
                    } else {
                      html! {}
                    }}

                    <div class="grid grid-cols-1 md:grid-cols-2 gap-space-sm">
                      {for tpl.questions.iter().map(|q| {
                        let cur_val = ans_map
                          .get(&q.id)
                          .cloned()
                          .unwrap_or_else(|| q.default_value.clone());
                        let q_id = q.id.clone();
                        let qa_state = qa_answers.clone();

                        html! {
                          <div class="bg-surface-container p-space-sm rounded-lg border border-outline-variant/60 flex flex-col justify-between gap-1.5">
                            <div class="flex flex-col gap-0.5">
                              <span class="text-xs font-bold text-on-surface">{&q.question}</span>
                              <span class="text-[10px] text-on-surface-variant">{&q.hint}</span>
                            </div>
                            <div class="flex flex-wrap items-center gap-1 mt-1">
                              {for q.quick_options.iter().map(|(opt_val, opt_lbl)| {
                                let is_sel = &cur_val == opt_val;
                                let ov = opt_val.clone();
                                let qid_b = q_id.clone();
                                let qa_b = qa_state.clone();
                                html! {
                                  <button
                                    class={format!(
                                      "px-2 py-1 rounded text-[10px] border transition-colors cursor-pointer {}",
                                      if is_sel {
                                        "bg-secondary text-on-secondary border-secondary font-bold"
                                      } else {
                                        "bg-surface-container-lowest text-on-surface-variant border-outline-variant hover:text-on-surface"
                                      }
                                    )}
                                    onclick={Callback::from(move |_| {
                                      let mut next = (*qa_b).clone();
                                      next.insert(qid_b.clone(), ov.clone());
                                      qa_b.set(next);
                                    })}
                                  >
                                    {opt_lbl}
                                  </button>
                                }
                              })}
                              <input
                                type="text"
                                value={cur_val}
                                class="w-20 bg-surface-container-lowest border border-outline-variant rounded px-2 py-1 text-[11px] font-mono text-primary font-bold focus:border-primary outline-none ml-auto"
                                oninput={{
                                  let qid_i = q_id.clone();
                                  let qa_i = qa_state.clone();
                                  Callback::from(move |e: InputEvent| {
                                    if let Some(inp) = e.target_dyn_into::<HtmlInputElement>() {
                                      let mut next = (*qa_i).clone();
                                      next.insert(qid_i.clone(), inp.value());
                                      qa_i.set(next);
                                    }
                                  })
                                }}
                              />
                            </div>
                          </div>
                        }
                      })}
                    </div>

                    <div class="flex items-center justify-between flex-wrap gap-2 pt-space-xs border-t border-surface-container">
                      <span class="text-[11px] text-on-surface-variant">
                        {format!("Scope: {} (applies automatically whenever {} is active)", scope_expr, mat_name)}
                      </span>
                      <button
                        class="bg-secondary hover:opacity-90 text-on-secondary px-space-lg py-1.5 rounded-lg text-xs font-mono font-bold shadow transition-opacity cursor-pointer flex items-center gap-1.5"
                        onclick={Callback::from(move |_| {
                          let new_rule = TransformationRule {
                            id: format!("{}_{}", tid_clone, mat_name.to_lowercase().replace('+', "plus").replace('-', "_")),
                            name: format!("{} ({})", rule_title, mat_name),
                            description: rule_desc.clone(),
                            category: tpl_cat.clone(),
                            icon: tpl_icon.clone(),
                            enabled: true,
                            condition_expr: scope_expr.clone(),
                            lua_code: lua_code.clone(),
                            priority: 34,
                          };
                          cb_create.emit(new_rule);
                          aqi_close.set(None);
                        })}
                      >
                        <span class="material-symbols-outlined text-[15px]">{"check_circle"}</span>
                        {format!("Apply Rule to {} Stack", mat_name_btn)}
                      </button>
                    </div>
                  </div>
                }
              } else {
                html! {}
              }
            } else {
              html! {}
            }}
          </div>
        </div>
      }

      // Upgraded N×N (4×4 or 8×8) Flushing Volume Matrix: Imported Disk vs. Live Lua Diff & Multiplier
      <div class="flex flex-col gap-space-md bg-surface-container-low p-space-xl rounded-xl shadow-md border border-outline-variant">
        <div class="flex items-center justify-between flex-wrap gap-space-sm">
              <div class="flex items-center gap-space-sm">
                <span class="material-symbols-outlined text-tertiary text-2xl">{"grid_on"}</span>
                <div class="flex flex-col">
                  <h2 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
                    {format!(
                      "Flushing Volume Matrix ({}×{}, mm³) — Imported Disk vs. Lua Transformation Diff",
                      slot_count, slot_count
                    )}
                  </h2>
                  <span class="text-xs text-on-surface-variant">
                    {"Compares AnycubicSlicerNext.conf's imported flush_volumes_matrix & flush_multiplier against transformations/flush_volumes_matrix.lua"}
                  </span>
                </div>
              </div>

              <div class="flex items-center gap-space-sm flex-wrap">
                // 3-Way Matrix View Mode Toggle
                <div class="flex items-center bg-surface-container-lowest p-1 rounded-lg border border-outline-variant text-xs font-mono">
                  {for [
                    ("diff", "Before → After Diff"),
                    ("lua", "Lua Rule Only"),
                    ("disk", "Imported Disk Only"),
                  ].into_iter().map(|(m_id, m_lbl)| {
                    let is_sel = view_mode == m_id;
                    let mvm = matrix_view_mode.clone();
                    let target = m_id.to_string();
                    html! {
                      <button
                        class={format!(
                          "px-2.5 py-1 rounded transition-colors cursor-pointer {}",
                          if is_sel {
                            "bg-primary text-on-primary font-bold"
                          } else {
                            "text-on-surface-variant hover:text-on-surface"
                          }
                        )}
                        onclick={Callback::from(move |_| mvm.set(target.clone()))}
                      >
                        {m_lbl}
                      </button>
                    }
                  })}
                </div>

                <button
                  class="flex items-center gap-1 bg-primary-container hover:bg-primary text-on-primary-container px-space-md py-1.5 rounded-lg text-xs font-mono font-bold shadow transition-colors cursor-pointer"
                  onclick={Callback::from(move |_| on_stage_flush.emit(()))}
                >
                  <span class="material-symbols-outlined text-[15px]">{"pending_actions"}</span>
                  {"Stage Matrix & Multiplier to Disk (.conf)"}
                </button>
              </div>
            </div>

            // 1. Flushing Multiplier & Melt-Zone Multiplier Controls Bar
            <div class="grid grid-cols-1 lg:grid-cols-3 gap-space-md bg-surface-container p-space-md rounded-xl border border-outline-variant/60 font-mono text-xs">
              <div class="flex flex-col justify-between gap-2">
                <div class="flex items-center justify-between">
                  <span class="text-on-surface-variant text-[10px] uppercase font-bold">
                    {"1. Flushing Multiplier (app.flush_multiplier)"}
                  </span>
                  <span class="text-primary font-bold text-sm">
                    {format!("{:.2}× (Disk Import: {:.2}×)", props.flush_multiplier, props.disk_flush_multiplier)}
                  </span>
                </div>
                <div class="flex items-center gap-1">
                  {for [0.50f64, 0.75, 1.00, 1.25, 1.50].into_iter().map(|m_val| {
                    let is_sel = (props.flush_multiplier - m_val).abs() < 0.02;
                    let cb = on_change_mult.clone();
                    html! {
                      <button
                        class={format!(
                          "flex-1 py-1 rounded border text-[11px] transition-colors cursor-pointer {}",
                          if is_sel {
                            "bg-primary text-on-primary border-primary font-bold"
                          } else {
                            "bg-surface-container-lowest text-on-surface-variant border-outline-variant hover:text-on-surface"
                          }
                        )}
                        onclick={Callback::from(move |_| cb.emit(m_val))}
                      >
                        {if (m_val - 1.0).abs() < 0.01 {
                          "1.00× (Disk)".to_string()
                        } else {
                          format!("{:.2}×", m_val)
                        }}
                      </button>
                    }
                  })}
                  <button
                    class="px-2 py-1 rounded bg-surface-container-lowest hover:bg-surface-bright border border-outline-variant text-on-surface cursor-pointer"
                    title="Decrease multiplier by 0.05x"
                    onclick={{
                      let cb = on_change_mult.clone();
                      let cur = props.flush_multiplier;
                      Callback::from(move |_| cb.emit((cur - 0.05).clamp(0.25, 3.0)))
                    }}
                  >
                    {"-0.05"}
                  </button>
                  <button
                    class="px-2 py-1 rounded bg-surface-container-lowest hover:bg-surface-bright border border-outline-variant text-on-surface cursor-pointer"
                    title="Increase multiplier by 0.05x"
                    onclick={{
                      let cb = on_change_mult.clone();
                      let cur = props.flush_multiplier;
                      Callback::from(move |_| cb.emit((cur + 0.05).clamp(0.25, 3.0)))
                    }}
                  >
                    {"+0.05"}
                  </button>
                </div>
              </div>

              <div class="flex flex-col justify-between gap-1 bg-surface-container-lowest p-space-sm rounded-lg border border-outline-variant/40">
                <div class="flex items-center justify-between">
                  <span class="text-on-surface-variant text-[10px] uppercase font-bold">
                    {"2. Hotend Melt-Zone & Effective Scale"}
                  </span>
                  <span class="text-secondary font-bold">
                    {format!(
                      "{:.2}× × {:.2}× = {:.2}×",
                      props.flush_multiplier,
                      props.melt_zone_multiplier,
                      props.flush_multiplier * props.melt_zone_multiplier
                    )}
                  </span>
                </div>
                <span class="text-[11px] text-on-surface-variant">
                  {format!(
                    "Hotend melt-zone multiplier ({:.2}×) × Global flush_multiplier ({:.2}×) scales every cell in transformations/flush_volumes_matrix.lua.",
                    props.melt_zone_multiplier, props.flush_multiplier
                  )}
                </span>
              </div>

              <div class="flex flex-col justify-between gap-1 bg-surface-container-lowest p-space-sm rounded-lg border border-outline-variant/40">
                <div class="flex items-center justify-between">
                  <span class="text-on-surface-variant text-[10px] uppercase font-bold">
                    {"3. How Import vs. Lua Diff Works"}
                  </span>
                  <span class="text-tertiary font-bold">
                    {if has_disk_matrix {
                      format!("{} / {} cells changed", changed_cells, comparable_cells)
                    } else {
                      "Computed from scratch".to_string()
                    }}
                  </span>
                </div>
                <span class="text-[11px] text-on-surface-variant">
                  {if has_disk_matrix {
                    format!(
                      "Avg Purge: Disk {:.0} mm³ → Lua {:.0} mm³ ({:+.0} mm³) • {} cross-polymer safety boosts (PLA↔PETG↔TPU base 300 × {:.2}x = {:.0} mm³)",
                      avg_disk,
                      avg_lua,
                      avg_lua - avg_disk,
                      cross_polymer_boosts,
                      props.flush_multiplier,
                      (300.0 * props.flush_multiplier).clamp(70.0, 900.0)
                    )
                  } else {
                    "Cross-polymer swaps (PLA↔PETG↔TPU) use a 300 mm³ base (450 mm³ at 1.5x); same-polymer swaps scale by RGB luminance delta, stickiness & translucency.".to_string()
                  }}
                </span>
              </div>
            </div>

            // Important OrcaSlicer / AnycubicSlicerNext Import & Auto-Calculate Warning Callout
            <div class="bg-amber-500/10 border-l-4 border-amber-400 p-space-md rounded-r-lg text-xs text-on-surface flex flex-col gap-1">
              <div class="flex items-center gap-1.5 font-bold text-amber-300 font-mono">
                <span>{"⚠️"}</span>
                <span>{"Important: How OrcaSlicer / AnycubicSlicerNext Handles Flushing Volumes on Import"}</span>
              </div>
              <span class="text-on-surface-variant">
                {"• "}
                <strong class="text-on-surface">{"Importing a .zip via File → Import → Import Configs... does NOT set Flushing Volumes:"}</strong>
                {" OrcaSlicer's config importer (PresetBundle::import_json_presets) only imports .json files with printer_settings_id, print_settings_id, or filament_settings_id and completely ignores .conf files (where global flush_volumes_matrix is stored)."}
              </span>
              <span class="text-on-surface-variant">
                {"• "}
                <strong class="text-on-surface">{"Use Export .3mf to import Flushing Volumes into the slicer:"}</strong>
                {" Opening the exported .3mf project loads Metadata/project_settings.config, which directly populates flush_volumes_matrix, flush_volumes_vector, and flush_multiplier in your project (or use Save to Disk / extract the full .zip directly into your AppData folder while the slicer is closed)."}
              </span>
              <span class="text-on-surface-variant">
                {"• "}
                <strong class="text-on-surface">{"Disable Auto-Recalculate so OrcaSlicer doesn't overwrite the matrix:"}</strong>
                {" In OrcaSlicer / AnycubicSlicerNext "}
                <span class="font-mono text-amber-200">{"Preferences → Control → Behaviour → \"Auto flush after changing...\""}</span>
                {" (auto_calculate_flush), change it from "}
                <span class="font-mono text-amber-200">{"All"}</span>
                {" to "}
                <span class="font-mono text-amber-200">{"None"}</span>
                {". Otherwise, switching any filament or printer preset will automatically recalculate and overwrite your custom flushing volumes!"}
              </span>
            </div>

            // Explanation Callout of What Changed from the Import
            if has_disk_matrix {
              <div class="bg-surface-container-lowest border-l-4 border-primary p-space-md rounded-r-lg text-xs text-on-surface-variant flex flex-col gap-1">
                <span class="font-bold text-on-surface font-mono">
                  {"Why our Lua Flushing Matrix differs from the imported AnycubicSlicerNext.conf matrix:"}
                </span>
                <span>
                  {"• On disk in AnycubicSlicerNext.conf, Anycubic's stock auto-calculator only checks RGB color distance and ignores polymer incompatibility — for example, Slot #3 (PLA+) → Slot #1 (PETG) can be under-purged if both colors have similar luminance."}
                </span>
                <span>
                  {format!(
                    "• Our Lua rule diffs each cell [From Slot #i → To Slot #j] as Disk → Lua (±Δ mm³): incompatible polymer families (PLA ↔ PETG ↔ TPU) use a 300 mm³ base × {:.2}x multiplier = {:.0} mm³, while same-family swaps (including PLA↔PLA-CF and PETG↔PCTG) scale by RGB luminance delta, carbon-fiber stickiness (1.15×), and translucency.",
                    props.flush_multiplier,
                    (300.0 * props.flush_multiplier).clamp(70.0, 900.0)
                  )}
                </span>
              </div>
            }

            <div class="overflow-x-auto">
              <table class="w-full text-center font-mono text-xs border-collapse">
                <thead>
                  <tr>
                    <th class="bg-surface-container-lowest p-2 text-on-surface-variant border border-surface-container">
                      {"From \\ To"}
                    </th>
                    {for active_slots.iter().enumerate().map(|(j, s)| {
                      html! {
                        <th class="bg-surface-container-lowest p-2 border border-surface-container">
                          <div class="flex items-center justify-center gap-1">
                            <span
                              class="w-2.5 h-2.5 rounded-full inline-block"
                              style={format!("background-color: {};", s.color_hex)}
                            ></span>
                            <span class="text-on-surface">{format!("S#{} {}", j + 1, s.material_type)}</span>
                          </div>
                        </th>
                      }
                    })}
                  </tr>
                </thead>
                <tbody>
                  {for active_slots.iter().enumerate().map(|(i, s_out)| {
                    html! {
                      <tr>
                        <td class="bg-surface-container p-2 font-bold border border-surface-container text-left">
                          <div class="flex items-center gap-1.5">
                            <span
                              class="w-2.5 h-2.5 rounded-full inline-block shrink-0"
                              style={format!("background-color: {};", s_out.color_hex)}
                            ></span>
                            <span class="text-on-surface truncate max-w-[150px]">
                              {format!("S#{} {}", i + 1, s_out.name)}
                            </span>
                          </div>
                        </td>
                        {for (0..slot_count).map(|j| {
                          let cell_idx = i * slot_count + j;
                          let lua_str = props
                            .flush_matrix
                            .get(cell_idx)
                            .cloned()
                            .unwrap_or_else(|| if i == j { "0".to_string() } else { "240".to_string() });
                          let lua_num: i32 = lua_str.parse().unwrap_or(0);

                          let disk_opt: Option<i32> = if has_disk_matrix && i < disk_n && j < disk_n {
                            props
                              .disk_flush_matrix
                              .get(i * disk_n + j)
                              .and_then(|s| s.parse().ok())
                          } else {
                            None
                          };

                          let is_cross_polymer = i != j && active_slots[i].material_type != active_slots[j].material_type;

                          let cell_class = if i == j {
                            "bg-surface-container-lowest text-on-surface-variant"
                          } else if lua_num >= 750 {
                            "bg-error-container/25 text-error font-bold"
                          } else if lua_num >= 450 {
                            "bg-secondary/20 text-secondary font-bold"
                          } else {
                            "bg-surface-container-lowest text-tertiary"
                          };

                          html! {
                            <td class={format!("p-2 border border-surface-container {}", cell_class)}>
                              {if i == j {
                                html! { <span>{"0"}</span> }
                              } else if view_mode == "disk" {
                                if let Some(d_val) = disk_opt {
                                  html! { <span>{format!("{} mm³", d_val)}</span> }
                                } else {
                                  html! { <span class="text-on-surface-variant">{"— (N/A on disk)"}</span> }
                                }
                              } else if view_mode == "diff" {
                                if let Some(d_val) = disk_opt {
                                  let delta = lua_num - d_val;
                                  html! {
                                    <div class="flex flex-col items-center leading-tight gap-0.5">
                                      <div class="flex items-center gap-1">
                                        <span class="text-on-surface-variant line-through text-[10px]">{d_val}</span>
                                        <span class="text-primary text-[10px]">{"→"}</span>
                                        <span class="font-bold">{lua_num}</span>
                                      </div>
                                      <span class={if delta > 0 { "text-[10px] opacity-90" } else { "text-[10px] text-tertiary" }}>
                                        {if is_cross_polymer {
                                          format!("{:+} (Cross-Mat)", delta)
                                        } else {
                                          format!("{:+} mm³", delta)
                                        }}
                                      </span>
                                    </div>
                                  }
                                } else {
                                  html! {
                                    <div class="flex flex-col items-center leading-tight">
                                      <span class="font-bold">{lua_num}</span>
                                      <span class="text-[9px] text-on-surface-variant">{"(New Slot)"}</span>
                                    </div>
                                  }
                                }
                              } else {
                                html! { <span>{lua_str}</span> }
                              }}
                            </td>
                          }
                        })}
                      </tr>
                    }
                  })}
                </tbody>
              </table>
            </div>
      </div>
    </div>
  }
}
