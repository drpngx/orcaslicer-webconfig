use crate::catalog::load_merged_catalog;
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
  pub on_create_rule: Callback<TransformationRule>,
  pub on_export_spool_json: Callback<FilamentSpool>,
  pub on_export_3mf: Callback<()>,
  pub on_open_bind_modal: Callback<()>,
}

#[function_component]
pub fn FilamentTab(props: &FilamentTabProps) -> Html {
  let filter_material = use_state(|| "ALL".to_string());
  let selected_slot_idx = use_state(|| 0usize);
  let show_raw_disk_json = use_state(|| false);
  // "diff" = Before (Disk) -> After (Lua), "lua" = Lua Output Only, "disk" = Imported Disk Only
  let matrix_view_mode = use_state(|| "diff".to_string());
  let qa_templates = use_memo((), |_| {
    TransformationEngine::get_parameterized_calibration_templates()
  });
  let qa_tree_cat = use_state(|| "all".to_string());
  let active_qa_id = use_state(|| None::<String>);
  let qa_answers = use_state(HashMap::<String, String>::new);

  if !props.is_config_loaded || props.filaments.is_empty() {
    return html! {
      <div class="flex flex-col items-center justify-center bg-surface-container-low p-space-xl rounded-2xl border border-outline-variant gap-space-md text-center my-8">
        <span class="material-symbols-outlined text-primary text-5xl">{"swap_horiz"}</span>
        <h2 class="font-headline-lg text-headline-lg text-on-surface font-bold">
          {"No Filaments Loaded Yet"}
        </h2>
        <p class="text-body-md text-on-surface-variant max-w-xl">
          {"Nothing is loaded. Use the startup Import Modal (or click Reset All in the top right) to import your configuration directory first."}
        </p>
      </div>
    };
  }

  let current_filter = (*filter_material).clone();
  let slot_count = (props.ace_pro_units * 4).min(props.filaments.len());
  let active_slots: Vec<FilamentSpool> = props.filaments.iter().take(slot_count).cloned().collect();

  let filtered_spools: Vec<FilamentSpool> = props
    .filaments
    .iter()
    .filter(|s| current_filter == "ALL" || s.material_type.eq_ignore_ascii_case(&current_filter))
    .cloned()
    .collect();

  let on_select = props.on_select_active_filament.clone();
  let on_change_ace = props.on_change_ace_units.clone();
  let on_update_spool = props.on_update_spool.clone();
  let on_swap_slot = props.on_swap_slot_spool.clone();
  let on_add_spool = props.on_add_spool.clone();
  let on_export_spool = props.on_export_spool_json.clone();
  let active_noz = props.active_nozzle_type.clone();
  let catalog = use_memo((), |_| load_merged_catalog());
  let material_choices: Vec<String> = catalog.filaments.iter().map(|f| f.code.clone()).collect();

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

  html! {
    <div class="flex flex-col w-full gap-space-lg">
      // Physical AMS / ACE Pro / Double ACE Pro 2 Bank Header & Importer
      <div class="flex flex-wrap items-center justify-between gap-space-md bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant">
        <div class="flex items-center gap-space-md flex-wrap">
          <span class="material-symbols-outlined text-primary text-2xl">{"swap_horiz"}</span>
          <div class="flex flex-col">
            <div class="flex items-center gap-space-sm">
              <span class="font-headline-sm text-headline-sm text-on-surface font-semibold">
                {if props.ace_pro_units == 2 {
                  "Anycubic Double ACE Pro 2 — 8-Slot Dual Hub Bank"
                } else {
                  "Anycubic ACE Pro — Auto-Imported Filaments & Base Transformations"
                }}
              </span>
              <span class="bg-tertiary-container/30 text-tertiary text-xs px-2 py-0.5 rounded-full font-mono">
                {format!("{} Imported Spools ({} Active Slots)", props.filaments.len(), slot_count)}
              </span>
            </div>
            <span class="text-xs text-on-surface-variant">
              {"Each imported filament is understood as a set of transformations (deltas) from its system base profile (e.g. Anycubic PETG @Anycubic Kobra S1 0.4 nozzle)."}
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
            title="Add a new filament spool to the bank (and next available ACE Pro slot)"
            onclick={{
              let cb = on_add_spool.clone();
              let next_idx = props.filaments.len() + 1;
              let noz = active_noz.clone();
              Callback::from(move |_| {
                let mut sp = FilamentSpool {
                  id: format!("spool_custom_{}", next_idx),
                  name: format!("OC: Anycubic PETG #{}", next_idx),
                  brand: "Anycubic".to_string(),
                  inherits: "Anycubic PETG @Anycubic Kobra S1 0.4 nozzle".to_string(),
                  material_type: "PETG".to_string(),
                  nozzle_type: noz.clone(),
                  tuned_for_nozzle_type: noz.clone(),
                  required_nozzle_hrc: 3,
                  color_hex: "#00D4FF".to_string(),
                  color_name: "Cyan".to_string(),
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
                sp.apply_material_change("PETG", &noz);
                cb.emit(sp);
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

      // Active Spool Slots Grid (4 Slots or 8 Slots for Double ACE Pro 2)
      <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-space-md">
        {for active_slots.iter().enumerate().map(|(idx, spool)| {
          let is_selected_slot = *selected_slot_idx == idx;
          let is_active_mat = spool.material_type.eq_ignore_ascii_case(&props.active_filament_type);
          let mat = spool.material_type.clone();
          let cb_mat = on_select.clone();
          let sel_slot = selected_slot_idx.clone();
          let sp_export = spool.clone();
          let cb_exp = on_export_spool.clone();
          let unit_num = if idx < 4 { 1 } else { 2 };
          let temp_delta = spool.print_temp - spool.base_print_temp;
          let bed_delta = spool.bed_temp - spool.base_bed_temp;
          let sp_for_mat_change = spool.clone();
          let cb_update_mat = on_update_spool.clone();
          let noz_for_mat = active_noz.clone();
          let mat_options = material_choices.clone();
          let all_spools_for_swap = props.filaments.clone();
          let cb_swap = on_swap_slot.clone();

          html! {
            <div
              class={format!(
                "flex flex-col justify-between p-space-md rounded-xl border transition-all cursor-pointer {}",
                if is_selected_slot {
                  "bg-surface-container border-primary shadow-lg ring-1 ring-primary/40"
                } else {
                  "bg-surface-container-low border-outline-variant hover:border-surface-bright"
                }
              )}
              onclick={Callback::from(move |_| sel_slot.set(idx))}
            >
              <div class="flex flex-col">
                // Slot header
                <div class="flex items-center justify-between mb-space-xs">
                  <div class="flex items-center gap-space-xs font-code-sm text-code-sm text-on-surface-variant">
                    <span class="font-bold text-on-surface">{format!("Slot #{}", idx + 1)}</span>
                    <span class="text-[10px] uppercase bg-surface-container-highest px-1.5 py-0.5 rounded text-secondary">
                      {format!("ACE #{}", unit_num)}
                    </span>
                    <span
                      class={format!(
                        "text-[10px] uppercase px-1.5 py-0.5 rounded font-mono flex items-center gap-0.5 {}",
                        if spool.deltas_from_base.is_empty() {
                          "bg-surface-container-lowest text-on-surface-variant"
                        } else if is_selected_slot {
                          "bg-tertiary text-on-tertiary font-bold"
                        } else {
                          "bg-tertiary/15 text-tertiary font-bold"
                        }
                      )}
                      title="Click card to view full delta details in the summary"
                    >
                      {if spool.deltas_from_base.len() == 1 {
                        "1 delta".to_string()
                      } else {
                        format!("{} deltas", spool.deltas_from_base.len())
                      }}
                    </span>
                  </div>
                  <div
                    class="w-4 h-4 rounded-full shadow-inner border border-white/20"
                    style={format!("background-color: {}; opacity: {};", spool.color_hex, spool.opacity.max(0.35))}
                  ></div>
                </div>

                // Spool name & inherits base
                <div class="flex flex-col">
                  <span class="font-headline-sm text-body-md text-on-surface font-bold truncate" title={spool.name.clone()}>
                    {spool.format_concise_title()}
                  </span>
                  <div class="flex items-center justify-between gap-1 mt-0.5">
                    <span class="text-[10px] font-mono text-on-surface-variant truncate">
                      {format!("Base: {}", spool.inherits)}
                    </span>
                    <span class="text-[10px] font-mono bg-surface-container-highest text-primary px-1.5 py-0.5 rounded shrink-0">
                      {format!("Nozzle: {} (HRC {})", spool.nozzle_type, spool.required_nozzle_hrc)}
                    </span>
                  </div>
                </div>

                // Inline Slot Material & Spool Assignment Controls (works immediately after Import!)
                <div
                  class="grid grid-cols-2 gap-1.5 mt-space-sm pt-space-xs border-t border-surface-container/80 font-mono text-[11px]"
                  onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                >
                  <div class="flex flex-col gap-0.5">
                    <span class="text-[9px] uppercase text-on-surface-variant">{"Slot Material"}</span>
                    <select
                      class="bg-surface-container-lowest border border-outline-variant rounded px-1.5 py-1 text-[11px] font-mono text-primary font-bold focus:border-primary outline-none cursor-pointer"
                      value={spool.material_type.clone()}
                      onchange={Callback::from(move |e: Event| {
                        e.stop_propagation();
                        if let Some(new_mat) = extract_event_value(&e) {
                          let mut updated = sp_for_mat_change.clone();
                          updated.apply_material_change(&new_mat, &noz_for_mat);
                          cb_update_mat.emit(updated);
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

                  <div class="flex flex-col gap-0.5">
                    <span class="text-[9px] uppercase text-on-surface-variant">{"Assign Spool"}</span>
                    <select
                      class="bg-surface-container-lowest border border-outline-variant rounded px-1.5 py-1 text-[11px] font-mono text-on-surface focus:border-primary outline-none cursor-pointer truncate"
                      value={idx.to_string()}
                      onchange={Callback::from(move |e: Event| {
                        e.stop_propagation();
                        if let Some(val_str) = extract_event_value(&e) {
                          if let Ok(target_idx) = val_str.parse::<usize>() {
                            if target_idx != idx {
                              cb_swap.emit((idx, target_idx));
                            }
                          }
                        }
                      })}
                    >
                      {for all_spools_for_swap.iter().enumerate().map(|(s_idx, s_item)| {
                        html! {
                          <option value={s_idx.to_string()} selected={s_idx == idx}>
                            {format!("#{} {} ({})", s_idx + 1, s_item.material_type, s_item.format_concise_title())}
                          </option>
                        }
                      })}
                    </select>
                  </div>
                </div>

                // Base -> Derived Transformation Summary + Inline Delta Details
                <div class="flex flex-col gap-1.5 bg-surface-container-lowest p-space-sm rounded text-xs font-mono mt-space-sm">
                  <div class="grid grid-cols-2 gap-space-xs">
                    <div class="flex flex-col">
                      <span class="text-on-surface-variant text-[9px] uppercase">{"Nozzle (Base→Disk)"}</span>
                      <span class="text-primary font-bold">
                        {format!("{}°→{}°C ({:+})", spool.base_print_temp, spool.print_temp, temp_delta)}
                      </span>
                    </div>
                    <div class="flex flex-col">
                      <span class="text-on-surface-variant text-[9px] uppercase">{"Bed (Base→Disk)"}</span>
                      <span class="text-secondary font-bold">
                        {format!("{}°→{}°C ({:+})", spool.base_bed_temp, spool.bed_temp, bed_delta)}
                      </span>
                    </div>
                  </div>

                  // Compact inline delta preview (expands with full explanation when card is clicked/selected)
                  if !spool.deltas_from_base.is_empty() {
                    <div class="flex flex-col gap-1 pt-1 border-t border-surface-container text-[10px]">
                      {for spool.deltas_from_base.iter().map(|d| {
                        html! {
                          <div class="flex flex-col gap-0.5">
                            <div class="flex items-center justify-between gap-1">
                              <span class="text-tertiary font-bold truncate">{&d.param_key}</span>
                              <span class="text-on-surface shrink-0">
                                {format!("{} → {}", d.base_value, d.disk_value)}
                              </span>
                            </div>
                            <div class="flex items-center justify-between gap-1">
                              <span
                                class={format!(
                                  "px-1 py-0.2 rounded text-[9px] font-bold truncate {}",
                                  if d.reconstructed_by_lua {
                                    "bg-tertiary/20 text-tertiary"
                                  } else {
                                    "bg-error/20 text-error"
                                  }
                                )}
                              >
                                {d.rule_badge_label()}
                              </span>
                            </div>
                            if is_selected_slot {
                              <span class="text-[9px] text-on-surface-variant leading-tight">
                                {&d.rule_explanation}
                              </span>
                            }
                          </div>
                        }
                      })}
                    </div>
                  }
                </div>
              </div>

              // Action buttons
              <div class="pt-space-sm mt-space-sm border-t border-surface-container flex items-center justify-between gap-space-xs">
                <button
                  class="text-[11px] font-mono text-secondary hover:underline flex items-center gap-0.5"
                  onclick={Callback::from(move |e: MouseEvent| {
                    e.stop_propagation();
                    cb_exp.emit(sp_export.clone());
                  })}
                >
                  <span class="material-symbols-outlined text-[13px]">{"download"}</span>
                  {"Export .json"}
                </button>

                if is_active_mat {
                  <span class="text-[11px] text-primary font-bold font-mono">
                    {"Active Material"}
                  </span>
                } else {
                  <button
                    class="bg-surface-container-highest hover:bg-surface-bright text-on-surface px-space-sm py-0.5 rounded text-[11px] font-semibold transition-colors"
                    onclick={Callback::from(move |e: MouseEvent| {
                      e.stop_propagation();
                      cb_mat.emit(mat.clone());
                    })}
                  >
                    {"Set Active"}
                  </button>
                }
              </div>
            </div>
          }
        })}
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
                <span class="text-primary font-bold">{&spool.material_type}</span>
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
                placeholder="e.g. OC: Anycubic PCTG Clear"
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

      // Saved Filament Bank & Standalone .json Exporter Table
      <div class="flex flex-col gap-space-md bg-surface-container-low p-space-xl rounded-xl shadow-md border border-outline-variant">
        <div class="flex flex-wrap items-center justify-between gap-space-sm">
          <div class="flex items-center gap-space-sm">
            <span class="material-symbols-outlined text-secondary">{"inventory_2"}</span>
            <span class="font-headline-lg text-headline-sm text-on-surface font-semibold">
              {"All Imported Filaments & Base Transformation Summary"}
            </span>
          </div>

          <div class="flex items-center gap-1 bg-surface-container rounded-lg p-0.5 border border-outline-variant flex-wrap">
            {for vec!["ALL", "PLA", "PLA+", "PETG", "PCTG", "PLA-CF", "TPU", "ASA"].into_iter().map(|mat| {
              let is_active = (*filter_material) == mat;
              let m = mat.to_string();
              let fm = filter_material.clone();

              html! {
                <button
                  class={format!(
                    "px-space-md py-1 rounded text-xs font-mono transition-colors {}",
                    if is_active {
                      "bg-primary text-on-primary font-bold"
                    } else {
                      "text-on-surface-variant hover:text-on-surface"
                    }
                  )}
                  onclick={Callback::from(move |_| fm.set(m.clone()))}
                >
                  {mat}
                </button>
              }
            })}
          </div>
        </div>

        <div class="flex flex-col overflow-x-auto">
          <div class="grid grid-cols-12 px-space-md py-space-xs bg-surface-container-lowest text-on-surface-variant font-label-caps text-label-caps uppercase rounded">
            <div class="col-span-3">{"Imported Spool & Base"}</div>
            <div class="col-span-2">{"Material & Finish"}</div>
            <div class="col-span-2 text-right">{"Base → Disk Nozzle"}</div>
            <div class="col-span-2 text-right">{"Base → Disk Bed"}</div>
            <div class="col-span-1 text-right">{"Deltas"}</div>
            <div class="col-span-2 text-right">{"Standalone .json"}</div>
          </div>

          <div class="flex flex-col gap-1 mt-1">
            {for filtered_spools.iter().map(|spool| {
              let actual_idx = props
                .filaments
                .iter()
                .position(|s| s.id == spool.id)
                .unwrap_or(0);
              let is_row_selected = *selected_slot_idx == actual_idx;
              let sp_exp = spool.clone();
              let cb_exp = on_export_spool.clone();
              let sel_slot = selected_slot_idx.clone();

              html! {
                <div class="flex flex-col rounded overflow-hidden border border-transparent hover:border-outline-variant/60 transition-all">
                  <div
                    class={format!(
                      "grid grid-cols-12 px-space-md py-space-sm items-center transition-colors font-code-sm text-code-sm cursor-pointer {}",
                      if is_row_selected {
                        "bg-surface-container-high border-l-4 border-primary"
                      } else {
                        "bg-surface-container hover:bg-surface-container-high"
                      }
                    )}
                    onclick={Callback::from(move |_| sel_slot.set(actual_idx))}
                  >
                    <div class="col-span-3 flex items-center gap-space-sm">
                      <div
                        class="w-3.5 h-3.5 rounded-full shrink-0"
                        style={format!("background-color: {};", spool.color_hex)}
                      ></div>
                      <div class="flex flex-col truncate">
                        <span class="text-on-surface font-semibold truncate">{spool.format_concise_title()}</span>
                        <span class="text-on-surface-variant text-[10px] truncate">{format!("Inherits: {}", spool.inherits)}</span>
                      </div>
                    </div>
                    <div class="col-span-2 flex items-center gap-1">
                      <span class="bg-surface-container-highest px-2 py-0.5 rounded text-xs text-primary font-bold">
                        {&spool.material_type}
                      </span>
                      <span class="text-xs text-on-surface-variant">{&spool.finish_type}</span>
                    </div>
                    <div class="col-span-2 text-right font-mono text-xs text-primary">
                      {format!("{}°C → {}°C ({:+})", spool.base_print_temp, spool.print_temp, spool.print_temp - spool.base_print_temp)}
                    </div>
                    <div class="col-span-2 text-right font-mono text-xs text-secondary">
                      {format!("{}°C → {}°C ({:+})", spool.base_bed_temp, spool.bed_temp, spool.bed_temp - spool.base_bed_temp)}
                    </div>
                    <div class="col-span-1 text-right font-mono text-xs text-tertiary font-bold">
                      {if spool.deltas_from_base.len() == 1 {
                        "1 delta ▾".to_string()
                      } else {
                        format!("{} deltas ▾", spool.deltas_from_base.len())
                      }}
                    </div>
                    <div class="col-span-2 text-right">
                      <button
                        class="bg-surface-container-highest hover:bg-surface-bright text-primary px-space-sm py-1 rounded text-xs font-mono transition-colors"
                        onclick={Callback::from(move |e: MouseEvent| {
                          e.stop_propagation();
                          cb_exp.emit(sp_exp.clone());
                        })}
                      >
                        {"Export .json"}
                      </button>
                    </div>
                  </div>

                  // Inline Expanded Delta & Parameter Details when this row is clicked/selected
                  if is_row_selected {
                    <div class="bg-surface-container-lowest px-space-lg py-space-md border-t border-outline-variant/40 flex flex-col gap-space-sm font-mono text-xs">
                      <div class="flex flex-wrap items-center justify-between gap-2 text-[11px] text-on-surface-variant">
                        <span>
                          {format!(
                            "Nozzle: {} (Required HRC ≥ {}) • Brass Base: {}°C • Hardened Steel Base: {}°C",
                            spool.nozzle_type, spool.required_nozzle_hrc, spool.brass_base_temp, spool.hs_base_temp
                          )}
                        </span>
                        <span>
                          {format!(
                            "Flow: {:.2} • PA: {:.3} • Max Flow: {:.1} mm³/s • ACE Dryer: {}°C / {}h • Stickiness: {:.2}x",
                            spool.flow_ratio, spool.pressure_advance, spool.max_volumetric_speed, spool.drying_temp_c, spool.drying_time_hr, spool.pigment_stickiness
                          )}
                        </span>
                      </div>

                      if spool.deltas_from_base.is_empty() {
                        <div class="text-tertiary text-xs">
                          {"✓ Zero overrides relative to system base profile — 100% stock inheritance."}
                        </div>
                      } else {
                        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-space-sm">
                          {for spool.deltas_from_base.iter().map(|delta| {
                            html! {
                              <div class="bg-surface-container p-space-sm rounded border border-outline-variant/60 flex flex-col gap-1">
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
                                <div class="flex items-center gap-2">
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
                  }
                </div>
              }
            })}
          </div>
        </div>
      </div>
    </div>
  }
}
