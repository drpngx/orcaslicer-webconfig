use crate::components::nav::ActiveTab;
use crate::models::{MachineRig, ProcessProfile};
use crate::orca_profile::DiffItem;
use crate::transformations::{ExternalConflict, PendingDiskUpdate, TransformationRule};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct HomeTabProps {
  pub is_config_loaded: bool,
  pub bound_filename: String,
  pub bound_path_hint: String,
  pub detected_hardware_summary: String,
  pub scanned_file_count: usize,
  pub last_saved_time: String,
  pub on_load_reference_snapshot: Callback<()>,
  pub on_import_zip: Callback<Event>,
  pub on_backup_zip: Callback<()>,
  pub on_export_webconfig_json: Callback<()>,
  pub on_import_webconfig_json: Callback<Event>,
  pub on_reset_all: Callback<()>,
  pub on_commit_save: Callback<()>,
  pub on_export_3mf: Callback<()>,
  pub on_export_standalone_json: Callback<()>,
  pub on_switch_tab: Callback<ActiveTab>,
  pub diff_items: Vec<DiffItem>,
  pub rules: Vec<TransformationRule>,
  pub on_toggle_rule: Callback<String>,
  pub conflicts: Vec<ExternalConflict>,
  pub pending_updates: Vec<PendingDiskUpdate>,
  pub on_resolve_conflict: Callback<(String, bool)>,
  pub show_diff_drawer: bool,
  pub on_toggle_diff_drawer: Callback<()>,
  pub active_rig: MachineRig,
  pub processes: Vec<ProcessProfile>,
  pub active_process_id: String,
  pub on_select_process: Callback<String>,
  pub safe_speed_ceiling: f64,
  pub json_diff_text: String,
  pub our_internal_config_json: String,
  pub user_disk_config_json: String,
}

#[function_component]
pub fn HomeTab(props: &HomeTabProps) -> Html {
  let on_toggle_rule = props.on_toggle_rule.clone();
  let on_resolve_conflict = props.on_resolve_conflict.clone();
  let on_select_process = props.on_select_process.clone();

  if !props.is_config_loaded {
    return html! {
      <div class="flex flex-col items-center justify-center bg-surface-container-low p-space-xl rounded-2xl border border-outline-variant gap-space-md text-center my-8">
        <span class="material-symbols-outlined text-primary text-5xl">{"folder_managed"}</span>
        <h2 class="font-headline-lg text-headline-lg text-on-surface font-bold">
          {"Nothing Loaded — Use the Startup Import Modal"}
        </h2>
        <p class="text-body-md text-on-surface-variant max-w-xl">
          {"The application state is completely clean. Use the Import Modal (or click Reset All in the top right) to import your configuration directory."}
        </p>
      </div>
    };
  }

  let active_proc = props
    .processes
    .iter()
    .find(|p| p.id == props.active_process_id)
    .cloned()
    .unwrap_or_else(|| ProcessProfile::get_default_processes().remove(0));

  let changed_pending_updates: Vec<&PendingDiskUpdate> = props
    .pending_updates
    .iter()
    .filter(|u| u.has_value_change())
    .collect();

  html! {
    <div class="flex flex-col w-full gap-space-lg">
      // Auto-Detected Hardware & Directory Backup Banner
      <div class="bg-surface-container-low border border-primary/50 p-space-md rounded-xl shadow-md flex flex-wrap items-center justify-between gap-space-md">
        <div class="flex items-center gap-space-md">
          <div class="w-10 h-10 rounded-lg bg-primary-container/30 border border-primary flex items-center justify-center text-primary">
            <span class="material-symbols-outlined">{"verified"}</span>
          </div>
          <div class="flex flex-col">
            <div class="flex items-center gap-space-xs">
              <span class="text-[10px] font-mono uppercase bg-primary text-on-primary px-2 py-0.5 rounded font-bold">
                {"Auto-Detected From Disk"}
              </span>
              <span class="text-xs font-mono text-tertiary">
                {format!("{} files scanned in directory", props.scanned_file_count)}
              </span>
            </div>
            <span class="font-headline-sm text-body-md text-on-surface font-bold mt-0.5">
              {&props.detected_hardware_summary}
            </span>
          </div>
        </div>

        <div class="flex items-center gap-space-sm flex-wrap">
          <button
            class="flex items-center gap-space-xs bg-primary-container hover:bg-primary text-on-primary-container px-space-md py-1.5 rounded-lg text-xs font-mono font-bold transition-colors cursor-pointer"
            title="Export the entire site state (machines, filaments, processes, Lua rules) as an Orca WebConfig JSON"
            onclick={
              let cb = props.on_export_webconfig_json.clone();
              Callback::from(move |_| cb.emit(()))
            }
          >
            <span class="material-symbols-outlined text-[16px]">{"data_object"}</span>
            {"Export Orca WebConfig (.json)"}
          </button>
          <label
            class="flex items-center gap-space-xs bg-surface-container-highest hover:bg-surface-bright text-primary border border-primary/50 px-space-md py-1.5 rounded-lg text-xs font-mono font-bold transition-colors cursor-pointer"
            title="Load a previously exported orcaslicer_webconfig.json state file"
          >
            <span class="material-symbols-outlined text-[16px]">{"upload_file"}</span>
            {"Load WebConfig (.json)"}
            <input
              type="file"
              accept=".json,application/json"
              class="hidden"
              onchange={props.on_import_webconfig_json.clone()}
            />
          </label>
          <button
            class="flex items-center gap-space-xs bg-secondary/20 hover:bg-secondary/30 text-secondary border border-secondary/50 px-space-md py-1.5 rounded-lg text-xs font-mono font-bold transition-colors cursor-pointer"
            onclick={
              let cb = props.on_backup_zip.clone();
              Callback::from(move |_| cb.emit(()))
            }
          >
            <span class="material-symbols-outlined text-[16px]">{"folder_zip"}</span>
            {"Download Full Directory Backup (.zip)"}
          </button>
          <button
            class="flex items-center gap-space-xs bg-error-container/30 hover:bg-error-container text-error border border-error/50 px-space-md py-1.5 rounded-lg text-xs font-mono font-bold transition-colors cursor-pointer"
            title="Clear everything and return to the startup Import Modal"
            onclick={
              let cb = props.on_reset_all.clone();
              Callback::from(move |_| cb.emit(()))
            }
          >
            <span class="material-symbols-outlined text-[16px]">{"restart_alt"}</span>
            {"Reset & Clear All"}
          </button>
        </div>
      </div>

      // Re-Created Configs vs Disk Differences Banner (Requirement #4)
      if !props.conflicts.is_empty() {
        <div class="bg-error-container/15 border border-error p-space-lg rounded-xl shadow-lg flex flex-col gap-space-sm">
          <div class="flex items-center justify-between flex-wrap gap-space-sm">
            <div class="flex items-center gap-space-sm text-error">
              <span class="material-symbols-outlined">{"rule"}</span>
              <span class="font-headline-sm text-headline-sm font-bold">
                {"Re-Created Configs via Transformations: Flagged Disk One-Off Differences"}
              </span>
            </div>
            <div class="flex items-center gap-space-sm">
              <span class="text-xs bg-error/20 text-error px-2.5 py-0.5 rounded-full font-mono font-bold">
                {format!("{} parameter(s) differ from clean base + rules", props.conflicts.len())}
              </span>
              <button
                class="bg-error text-on-error hover:opacity-90 px-space-md py-1 rounded text-xs font-bold transition-opacity cursor-pointer"
                onclick={
                  let cb = props.on_reset_all.clone();
                  Callback::from(move |_| cb.emit(()))
                }
              >
                {"Cancel Import & Reset"}
              </button>
            </div>
          </div>
          <p class="text-body-sm text-on-surface-variant">
            {"We reconstructed your disk profiles using system base profiles + Lua transformation rules (Hardened Steel +5°C thermal rule & +5°C PETG bed rule). Where your disk files contain additional one-off tweaks, you can accept them as new Lua transformation rules in our stack, discard them to keep the clean rule value, or cancel and reset."}
          </p>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-space-md mt-space-xs">
            {for props.conflicts.iter().map(|c| {
              let k_accept = c.param_key.clone();
              let k_discard = c.param_key.clone();
              let cb_accept = on_resolve_conflict.clone();
              let cb_discard = on_resolve_conflict.clone();

              html! {
                <div class="bg-surface-container p-space-md rounded-lg flex flex-col justify-between border border-outline-variant gap-space-sm">
                  <div class="flex justify-between items-start gap-2">
                    <div class="flex flex-col">
                      <span class="font-code-md text-code-md text-on-surface font-semibold">
                        {&c.param_label}
                      </span>
                      <span class="font-code-sm text-code-sm text-on-surface-variant font-mono">
                        {format!("Key: {} (System Base: {})", c.param_key, c.base_value)}
                      </span>
                    </div>
                    <span class="text-[11px] text-primary font-mono text-right">{&c.modifying_rule_id}</span>
                  </div>

                  <div class="grid grid-cols-2 gap-space-sm bg-surface-container-lowest p-space-sm rounded font-code-sm text-code-sm">
                    <div class="flex flex-col">
                      <span class="text-on-surface-variant text-[10px] uppercase font-bold">
                        {"User Disk Config Value:"}
                      </span>
                      <span class="text-error font-bold">{&c.actual_external_value}</span>
                    </div>
                    <div class="flex flex-col">
                      <span class="text-on-surface-variant text-[10px] uppercase font-bold">
                        {"Re-Created by Lua Rule:"}
                      </span>
                      <span class="text-tertiary font-bold">{&c.expected_derived_value}</span>
                    </div>
                  </div>

                  <div class="flex items-center gap-space-sm justify-end pt-space-xs">
                    <button
                      class="bg-primary-container hover:bg-primary text-on-primary-container px-space-md py-1 rounded text-xs font-semibold transition-colors cursor-pointer"
                      onclick={Callback::from(move |_| cb_accept.emit((k_accept.clone(), true)))}
                    >
                      {"Accept One-Off as Lua Transform"}
                    </button>
                    <button
                      class="bg-surface-container-high hover:bg-surface-bright text-on-surface px-space-md py-1 rounded text-xs font-semibold transition-colors cursor-pointer"
                      onclick={Callback::from(move |_| cb_discard.emit((k_discard.clone(), false)))}
                    >
                      {"Keep Clean Rule Value (Stage for Save)"}
                    </button>
                  </div>
                </div>
              }
            })}
          </div>
        </div>
      }

      // Staged Clean Rule Values Ready to Save to Disk Banner (only shown when values actually differ!)
      if !changed_pending_updates.is_empty() {
        <div class="bg-primary-container/15 border-2 border-primary p-space-lg rounded-xl shadow-lg flex flex-col gap-space-sm">
          <div class="flex items-center justify-between flex-wrap gap-space-sm">
            <div class="flex items-center gap-space-sm text-primary">
              <span class="material-symbols-outlined">{"pending_actions"}</span>
              <span class="font-headline-sm text-headline-sm font-bold">
                {format!(
                  "{} Staged Clean Rule Value(s) Ready to Save to Disk",
                  changed_pending_updates.len()
                )}
              </span>
            </div>
            <button
              class="flex items-center gap-space-xs bg-primary-container hover:bg-primary text-on-primary-container px-space-lg py-2 rounded-lg text-xs font-mono font-bold shadow-lg transition-colors cursor-pointer"
              onclick={
                let cb = props.on_commit_save.clone();
                Callback::from(move |_| cb.emit(()))
              }
            >
              <span class="material-symbols-outlined text-[16px]">{"save"}</span>
              {format!("Save {} Staged Change(s) to Disk Now", changed_pending_updates.len())}
            </button>
          </div>
          <p class="text-body-sm text-on-surface-variant">
            {"You selected 'Keep Clean Rule Value' for the parameters below. Your disk files have NOT been modified yet — click 'Save Staged Change(s) to Disk Now' above (or 'Save to Disk' in the top bar) to write these clean rule values to your local configuration files."}
          </p>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-space-sm mt-space-xs">
            {for changed_pending_updates.iter().map(|u| {
              html! {
                <div class="bg-surface-container p-space-sm rounded-lg border border-primary/40 flex flex-col gap-1 font-mono text-xs">
                  <div class="flex items-center justify-between">
                    <span class="text-on-surface font-bold">{&u.param_label}</span>
                    <span class="bg-primary/20 text-primary px-2 py-0.5 rounded text-[10px] font-bold">
                      {"Pending Save"}
                    </span>
                  </div>
                  <span class="text-[11px] text-on-surface-variant truncate">
                    {format!("File: {}", u.source_relative_path)}
                  </span>
                  <div class="flex items-center gap-2 text-[11px] mt-0.5">
                    <span class="text-error line-through">{format!("Disk: {}", u.old_disk_value)}</span>
                    <span class="text-on-surface-variant">{"→"}</span>
                    <span class="text-tertiary font-bold">{format!("Clean Rule: {}", u.new_clean_value)}</span>
                  </div>
                </div>
              }
            })}
          </div>
        </div>
      }

      // 1-Click Loadout & Dual-Export Strategy Banner (No scattered import buttons here!)
      <div class="grid grid-cols-1 lg:grid-cols-12 gap-space-lg">
        // Left 7 cols: Active Directory Status & Dual-Export Actions
        <div class="lg:col-span-7 bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant flex flex-col justify-between gap-space-md">
          <div class="flex flex-col gap-space-xs">
            <div class="flex items-center justify-between flex-wrap gap-space-sm">
              <div class="flex items-center gap-space-sm">
                <span class="material-symbols-outlined text-primary">{"link"}</span>
                <h2 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
                  {"Active Config Source & Dual-Export"}
                </h2>
                <span class="bg-tertiary-container/30 text-tertiary text-xs px-2 py-0.5 rounded-full font-mono flex items-center gap-1">
                  <span class="w-1.5 h-1.5 rounded-full bg-tertiary animate-ping"></span>
                  {"Local FS / ZIP Active"}
                </span>
              </div>

              <div class="flex items-center gap-space-xs flex-wrap">
                <button
                  class="flex items-center gap-space-xs bg-surface-container-highest hover:bg-surface-bright text-secondary border border-secondary/40 px-space-md py-space-xs rounded text-xs font-semibold transition-colors cursor-pointer"
                  title="Export Standalone OrcaSlicer Filament/Machine Delta Preset (.json)"
                  onclick={
                    let cb = props.on_export_standalone_json.clone();
                    Callback::from(move |_| cb.emit(()))
                  }
                >
                  <span class="material-symbols-outlined text-[14px]">{"data_object"}</span>
                  {"1. Standalone Preset (.json)"}
                </button>
                <button
                  class="flex items-center gap-space-xs bg-surface-container-highest hover:bg-surface-bright text-tertiary border border-tertiary/40 px-space-md py-space-xs rounded text-xs font-semibold transition-colors cursor-pointer"
                  title="Export Project Container (.3mf) with 4/8-slot ACE Pro arrays & flush_volumes_matrix"
                  onclick={
                    let cb = props.on_export_3mf.clone();
                    Callback::from(move |_| cb.emit(()))
                  }
                >
                  <span class="material-symbols-outlined text-[14px]">{"deployed_code"}</span>
                  {"2. Project Container (.3mf)"}
                </button>
                <button
                  class="flex items-center gap-space-xs bg-primary-container hover:bg-primary text-on-primary-container px-space-md py-space-xs rounded text-xs font-bold transition-colors shadow cursor-pointer"
                  onclick={
                    let cb = props.on_commit_save.clone();
                    Callback::from(move |_| cb.emit(()))
                  }
                >
                  <span class="material-symbols-outlined text-[14px]">{"save"}</span>
                  if !props.pending_updates.is_empty() {
                    {format!("Save to Disk ({} Staged)", props.pending_updates.len())}
                  } else {
                    {"Save Changes to Disk"}
                  }
                </button>
              </div>
            </div>

            <div class="text-body-sm text-on-surface-variant font-mono mt-1 bg-surface-container-lowest p-space-sm rounded border border-outline-variant flex items-center justify-between">
              <span class="truncate">{&props.bound_path_hint}</span>
              <span class="text-xs text-on-surface-variant shrink-0 ml-2">
                {format!("Status: {}", props.last_saved_time)}
              </span>
            </div>
          </div>

          // Footer controls: Local FS Refresh + Import/Export ZIP Backup
          <div class="flex items-center justify-between flex-wrap gap-space-md pt-space-xs border-t border-surface-container">
            <span class="text-xs text-on-surface-variant font-mono">
              {"Sources: Local Filesystem Bridge • Start-From-Scratch Wizard • ZIP Archive"}
            </span>
            <div class="flex items-center gap-space-sm flex-wrap">
              <label class="flex items-center gap-1 bg-surface-container-highest hover:bg-surface-bright text-secondary border border-secondary/40 px-2.5 py-1 rounded text-xs font-mono font-semibold cursor-pointer transition-colors">
                <span class="material-symbols-outlined text-[14px]">{"upload_file"}</span>
                {"Import .zip"}
                <input
                  type="file"
                  accept=".zip"
                  class="hidden"
                  onchange={props.on_import_zip.clone()}
                />
              </label>
              <button
                class="flex items-center gap-1 bg-surface-container-highest hover:bg-surface-bright text-secondary border border-secondary/40 px-2.5 py-1 rounded text-xs font-mono font-semibold cursor-pointer transition-colors"
                onclick={
                  let cb = props.on_backup_zip.clone();
                  Callback::from(move |_| cb.emit(()))
                }
              >
                <span class="material-symbols-outlined text-[14px]">{"folder_zip"}</span>
                {"Export .zip"}
              </button>
              <button
                class="flex items-center gap-1 text-xs text-primary hover:underline font-semibold cursor-pointer"
                onclick={
                  let cb = props.on_load_reference_snapshot.clone();
                  Callback::from(move |_| cb.emit(()))
                }
              >
                <span class="material-symbols-outlined text-[14px]">{"sync"}</span>
                {"Refresh Local FS"}
              </button>
            </div>
          </div>
        </div>

        // Right 5 cols: Three Modular Pillars Summary (Hardware + Process + Safe Speed Ceiling)
        <div class="lg:col-span-5 bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant flex flex-col justify-between gap-space-md">
          <div class="flex items-start justify-between">
            <div class="flex flex-col">
              <span class="text-xs uppercase tracking-wider text-primary font-bold">
                {"Pillar 1 & 3: Hardware + Process Combination"}
              </span>
              <h3 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
                {props.active_rig.format_concise_title()}
              </h3>
              if props.active_rig.name != props.active_rig.format_concise_title() {
                <span class="text-[11px] font-mono text-on-surface-variant truncate">
                  {format!("Disk Preset: {}", props.active_rig.name)}
                </span>
              }
            </div>
            <button
              class="text-xs text-secondary hover:underline flex items-center gap-0.5 cursor-pointer"
              onclick={
                let cb = props.on_switch_tab.clone();
                Callback::from(move |_| cb.emit(ActiveTab::Machine))
              }
            >
              {"Duplicate / Mod Rig"}
              <span class="material-symbols-outlined text-[14px]">{"arrow_forward"}</span>
            </button>
          </div>

          <div class="grid grid-cols-3 gap-space-xs bg-surface-container-lowest p-space-sm rounded text-xs font-mono text-center">
            <div class="flex flex-col">
              <span class="text-on-surface-variant text-[10px] uppercase">{"Nozzle"}</span>
              <span class="text-primary font-medium">
                {format!("{} {}mm", props.active_rig.nozzle_material, props.active_rig.nozzle_diameter)}
              </span>
            </div>
            <div class="flex flex-col">
              <span class="text-on-surface-variant text-[10px] uppercase">{"Add-Ons"}</span>
              <span class="text-on-surface font-medium">
                {format!("{} active", props.active_rig.addons.len())}
              </span>
            </div>
            <div class="flex flex-col">
              <span class="text-on-surface-variant text-[10px] uppercase">{"Hotend Cap"}</span>
              <span class="text-secondary font-medium">
                {format!("{:.1} mm³/s", props.active_rig.max_volumetric_flow_limit)}
              </span>
            </div>
          </div>

          // 3-Way Quality / Speed Mode Toggle (High Quality / Standard / High Speed)
          <div class="flex flex-col gap-space-xs">
            <div class="flex items-center justify-between text-xs">
              <span class="text-on-surface-variant font-semibold">
                {"Print Quality / Speed Mode:"}
              </span>
              <span class="font-mono text-tertiary text-[11px]">
                {active_proc.format_concise_title(&props.active_rig.short_model_tag())}
              </span>
            </div>
            <div class="grid grid-cols-3 gap-space-xs bg-surface-container-lowest p-1 rounded-lg border border-outline-variant">
              {for [
                ("high_quality", "🎯 High Quality", "60/180 mm/s"),
                ("standard", "⚖️ Standard", "200/300 mm/s"),
                ("high_speed", "🚀 High Speed", "300/350 mm/s"),
              ].into_iter().map(|(tier_id, tier_label, tier_sub)| {
                let matching_proc = props
                  .processes
                  .iter()
                  .find(|p| p.quality_tier() == tier_id)
                  .or_else(|| props.processes.first());
                let is_sel = active_proc.quality_tier() == tier_id;
                let target_pid = matching_proc.map(|p| p.id.clone()).unwrap_or_default();
                let cb = on_select_process.clone();

                html! {
                  <button
                    class={format!(
                      "px-space-sm py-1.5 rounded text-center text-xs font-mono border transition-all flex flex-col items-center cursor-pointer {}",
                      if is_sel {
                        "bg-primary text-on-primary border-primary font-bold shadow"
                      } else {
                        "bg-transparent border-transparent text-on-surface-variant hover:text-on-surface hover:bg-surface-container"
                      }
                    )}
                    onclick={Callback::from(move |_| {
                      if !target_pid.is_empty() {
                        cb.emit(target_pid.clone());
                      }
                    })}
                  >
                    <span>{tier_label}</span>
                    <span class={if is_sel { "text-[10px] opacity-90" } else { "text-[10px] text-on-surface-variant" }}>
                      {tier_sub}
                    </span>
                  </button>
                }
              })}
            </div>
            if !props.active_rig.notes.is_empty() || !active_proc.notes.is_empty() {
              <div class="grid grid-cols-1 md:grid-cols-2 gap-space-xs mt-1">
                <div class="bg-surface-container-lowest p-space-xs rounded border border-outline-variant/50 flex flex-col gap-0.5">
                  <span class="text-[10px] uppercase font-mono text-primary font-bold">
                    {"Machine Notes (printer_notes):"}
                  </span>
                  <pre class="text-[10px] font-mono text-on-surface-variant whitespace-pre-wrap leading-tight max-h-24 overflow-y-auto">
                    {&props.active_rig.notes}
                  </pre>
                </div>
                <div class="bg-surface-container-lowest p-space-xs rounded border border-outline-variant/50 flex flex-col gap-0.5">
                  <span class="text-[10px] uppercase font-mono text-tertiary font-bold">
                    {"Process Notes (notes):"}
                  </span>
                  <pre class="text-[10px] font-mono text-on-surface-variant whitespace-pre-wrap leading-tight max-h-24 overflow-y-auto">
                    {&active_proc.notes}
                  </pre>
                </div>
              </div>
            }
          </div>
        </div>
      </div>

      // Dual Config Architecture View: Our Internal Config Data Structure vs User Disk Configs
      <div class="bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant flex flex-col gap-space-md">
        <div class="flex items-center justify-between flex-wrap gap-space-sm">
          <div class="flex items-center gap-space-sm">
            <span class="material-symbols-outlined text-primary">{"compare_arrows"}</span>
            <div class="flex flex-col">
              <h3 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
                {"Two Distinct Config Models: Our Transformation Data Structure vs User Disk Configs"}
              </h3>
              <span class="text-xs text-on-surface-variant">
                {"Left: Our internal relational/transformation state (Rig + Add-ons + Base Deltas + Lua Rules). Right: The flat OrcaSlicer/AnycubicSlicerNext JSON files on disk."}
              </span>
            </div>
          </div>
        </div>

        <div class="grid grid-cols-1 lg:grid-cols-2 gap-space-md">
          <div class="flex flex-col gap-1 bg-surface-container-lowest p-space-md rounded-lg border border-primary/40">
            <div class="flex items-center justify-between text-xs font-mono pb-1 border-b border-surface-container">
              <span class="text-primary font-bold uppercase">
                {"1. Our Config (Transformation Stack & Staged Clean State)"}
              </span>
              <span class="text-on-surface-variant">{"orcaslicer_webconfig_state.json"}</span>
            </div>
            <pre class="text-[11px] font-mono text-on-surface overflow-x-auto max-h-56 whitespace-pre mt-1">
              {&props.our_internal_config_json}
            </pre>
          </div>

          <div class="flex flex-col gap-1 bg-surface-container-lowest p-space-md rounded-lg border border-secondary/40">
            <div class="flex items-center justify-between text-xs font-mono pb-1 border-b border-surface-container">
              <span class="text-secondary font-bold uppercase">
                {"2. User Config on Disk (Actual Disk Files — Updates on Save)"}
              </span>
              if !props.pending_updates.is_empty() {
                <span class="bg-error/20 text-error px-2 py-0.5 rounded text-[10px] font-bold">
                  {format!("{} Unsaved Staged Change(s)", props.pending_updates.len())}
                </span>
              } else {
                <span class="text-on-surface-variant">{&props.bound_filename}</span>
              }
            </div>
            <pre class="text-[11px] font-mono text-on-surface overflow-x-auto max-h-56 whitespace-pre mt-1">
              {&props.user_disk_config_json}
            </pre>
          </div>
        </div>
      </div>

      // Middle Grid: Computed Overrides Table (Left) + Quick Symptom Resolver (Right)
      <div class="grid grid-cols-1 lg:grid-cols-12 gap-space-lg items-start">
        // Computed Overrides vs Stock Defaults Table (8 cols)
        <div class="lg:col-span-8 flex flex-col gap-space-md bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant">
          <div class="flex items-center justify-between flex-wrap gap-space-sm">
            <div class="flex items-center gap-space-sm">
              <span class="material-symbols-outlined text-secondary">{"tune"}</span>
              <h3 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
                {"Re-Created Overrides vs System Base Defaults"}
              </h3>
            </div>
            <span class="text-xs text-on-surface-variant font-mono">
              {format!(
                "{} modified parameters ({} active Lua rules)",
                props.diff_items.iter().filter(|d| d.is_modified).count(),
                props.rules.iter().filter(|r| r.enabled).count()
              )}
            </span>
          </div>

          <div class="flex flex-col overflow-x-auto">
            // Header
            <div class="grid grid-cols-12 px-space-md py-space-xs bg-surface-container-lowest text-on-surface-variant font-label-caps text-label-caps uppercase rounded">
              <div class="col-span-4">{"Parameter"}</div>
              <div class="col-span-2">{"Category"}</div>
              <div class="col-span-2 text-right">{"System Base"}</div>
              <div class="col-span-2 text-right">{"Re-Created"}</div>
              <div class="col-span-2 text-right">{"Lua Rule Source"}</div>
            </div>

            // Rows
            <div class="flex flex-col gap-1 mt-1">
              {for props.diff_items.iter().map(|item| {
                let row_bg = if item.is_modified {
                  "bg-surface-container border-l-2 border-primary"
                } else {
                  "bg-surface-container/40 opacity-70"
                };

                let unit_suffix = if item.unit.is_empty() {
                  String::new()
                } else {
                  format!(" {}", item.unit)
                };

                html! {
                  <div class={format!("grid grid-cols-12 px-space-md py-space-sm items-center rounded hover:bg-surface-container-high transition-colors font-code-sm text-code-sm {}", row_bg)}>
                    <div class="col-span-4 flex flex-col">
                      <span class="text-on-surface font-medium">{&item.label}</span>
                      <span class="text-on-surface-variant text-[10px] font-mono">{&item.key}</span>
                    </div>
                    <div class="col-span-2 text-on-surface-variant text-xs">{&item.category}</div>
                    <div class="col-span-2 text-right text-on-surface-variant font-mono">
                      {format!("{}{}", item.base_value, unit_suffix)}
                    </div>
                    <div class={format!("col-span-2 text-right font-mono font-bold {}", if item.is_modified { "text-primary" } else { "text-on-surface" })}>
                      {format!("{}{}", item.override_value, unit_suffix)}
                    </div>
                    <div class="col-span-2 text-right text-xs truncate text-secondary" title={item.source_rule.clone()}>
                      {&item.source_rule}
                    </div>
                  </div>
                }
              })}
            </div>
          </div>
        </div>

        // Quick Symptom & Dynamic Rule Resolver (4 cols)
        <div class="lg:col-span-4 flex flex-col gap-space-md bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-space-sm">
              <span class="material-symbols-outlined text-primary">{"healing"}</span>
              <h3 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
                {"Lua Transformation Stack"}
              </h3>
            </div>
            <button
              class="text-xs text-secondary hover:underline cursor-pointer"
              onclick={
                let cb = props.on_switch_tab.clone();
                Callback::from(move |_| cb.emit(ActiveTab::Wizard))
              }
            >
              {"Edit Lua Scripts"}
            </button>
          </div>

          <p class="text-body-sm text-on-surface-variant">
            {"Toggle dynamic combination rules, hardware add-on schedules, and accepted one-offs:"}
          </p>

          <div class="flex flex-col gap-space-sm mt-space-xs">
            {for props.rules.iter().map(|rule| {
              let rid = rule.id.clone();
              let cb = on_toggle_rule.clone();
              let is_enabled = rule.enabled;

              html! {
                <div
                  class={format!(
                    "p-space-md rounded-lg flex flex-col gap-space-xs border transition-all cursor-pointer {}",
                    if is_enabled {
                      "bg-surface-container border-primary shadow-sm"
                    } else {
                      "bg-surface-container-lowest border-outline-variant hover:border-surface-bright"
                    }
                  )}
                  onclick={Callback::from(move |_| cb.emit(rid.clone()))}
                >
                  <div class="flex items-center justify-between">
                    <div class="flex items-center gap-space-sm">
                      <span class="text-lg">{&rule.icon}</span>
                      <div class="flex flex-col">
                        <span class="font-headline-sm text-body-md text-on-surface font-semibold">
                          {&rule.name}
                        </span>
                        <span class="text-[10px] font-mono text-secondary uppercase">
                          {&rule.category}
                        </span>
                      </div>
                    </div>
                    <div
                      class={format!(
                        "w-5 h-5 rounded flex items-center justify-center text-xs {}",
                        if is_enabled {
                          "bg-primary text-on-primary font-bold"
                        } else {
                          "bg-surface-container-highest text-transparent"
                        }
                      )}
                    >
                      {if is_enabled { "✓" } else { "" }}
                    </div>
                  </div>
                  <p class="text-xs text-on-surface-variant line-clamp-2 mt-1">
                    {&rule.description}
                  </p>
                </div>
              }
            })}
          </div>
        </div>
      </div>

      // Interactive JSON Diff Drawer Preview
      <div class="bg-surface-container-low p-space-lg rounded-xl shadow-md border border-outline-variant flex flex-col gap-space-md">
        <div class="flex items-center justify-between flex-wrap gap-space-sm">
          <div
            class="flex items-center gap-space-sm cursor-pointer"
            onclick={
              let cb = props.on_toggle_diff_drawer.clone();
              Callback::from(move |_| cb.emit(()))
            }
          >
            <span class="material-symbols-outlined text-secondary">{"difference"}</span>
            <h3 class="font-headline-sm text-headline-sm text-on-surface font-semibold">
              {"Interactive JSON Diff Preview (System Base vs Re-Created Output)"}
            </h3>
            <span class="material-symbols-outlined text-on-surface-variant text-[18px]">
              {if props.show_diff_drawer { "expand_less" } else { "expand_more" }}
            </span>
          </div>

          <div class="flex items-center gap-space-sm">
            <button
              class="flex items-center gap-space-xs bg-primary hover:bg-primary-fixed-dim text-on-primary-fixed px-space-md py-space-xs rounded text-xs font-semibold transition-colors cursor-pointer"
              onclick={
                let cb = props.on_commit_save.clone();
                Callback::from(move |_| cb.emit(()))
              }
            >
              <span class="material-symbols-outlined text-[14px]">{"save"}</span>
              {"Commit Changes to Local Disk"}
            </button>
          </div>
        </div>

        if props.show_diff_drawer {
          <div class="bg-surface-container-lowest p-space-md rounded-lg font-mono text-xs overflow-x-auto max-h-96 border border-outline-variant">
            <pre class="text-on-surface whitespace-pre">{&props.json_diff_text}</pre>
          </div>
        }
      </div>
    </div>
  }
}
