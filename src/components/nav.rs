use yew::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveTab {
  Home,     // Overview & Config Sync
  Machine,  // Printer & Rig Vault
  Filament, // Filaments & ACE Pro Bank
  Wizard,   // Symptom-Driven Auto-Tuner & Config Changer
}

#[derive(Properties, PartialEq)]
pub struct NavProps {
  pub active_tab: ActiveTab,
  pub on_select_tab: Callback<ActiveTab>,
  pub is_config_loaded: bool,
  pub is_tauri: bool,
  pub config_version: u32,
  pub bound_filename: String,
  pub active_rig_name: String,
  pub ace_pro_units: usize,
  pub show_help_docs: bool,
  pub on_toggle_help_docs: Callback<()>,
  pub on_open_export_modal: Callback<()>,
  pub on_reset_all: Callback<()>,
  pub has_unapplied_changes: bool,
  pub pending_save_count: usize,
}

#[function_component]
pub fn NavigationBar(props: &NavProps) -> Html {
  let active_tab = props.active_tab;
  let on_select_tab = props.on_select_tab.clone();

  let tab_class = |tab: ActiveTab| -> &'static str {
    if active_tab == tab {
      "bg-surface-container-highest text-primary font-semibold border-b-2 border-primary shadow-inner px-space-md py-space-xs rounded-t text-body-md font-body-md transition-colors cursor-pointer"
    } else {
      "text-on-surface-variant hover:text-on-surface px-space-md py-space-xs rounded-t text-body-md font-body-md transition-colors border-b-2 border-transparent cursor-pointer"
    }
  };

  let _ = (&props.active_rig_name, props.ace_pro_units);

  html! {
    <header class="fixed top-0 left-0 right-0 z-40 bg-surface-container-low shadow-[0_1px_8px_rgba(0,0,0,0.45)]">
      <div class="h-28 w-full px-margin-desktop flex flex-col justify-between py-space-xs">
        // Top row
        <div class="flex items-center justify-between gap-space-lg w-full h-14">
          <div class="flex items-center gap-space-lg">
            <div
              class="flex items-center gap-space-sm cursor-pointer"
              onclick={
                let cb = on_select_tab.clone();
                Callback::from(move |_| cb.emit(ActiveTab::Home))
              }
            >
              <span class="text-2xl">{"🐋"}</span>
              <span class="font-headline-sm text-headline-sm text-on-surface tracking-tight font-bold">
                {"OrcaSlicer WebConfig"}
              </span>
              <span class="font-code-sm text-code-sm text-primary bg-surface-container px-space-sm py-space-xs rounded">
                {"for v2.2"}
              </span>
            </div>
            <div class="h-5 w-px bg-surface-bright"></div>

            // Status Badge
            <div class="flex items-center gap-space-xs bg-surface-container-lowest px-space-md py-space-xs rounded border border-outline-variant">
              <div
                class={format!(
                  "w-2 h-2 rounded-full {}",
                  if props.is_config_loaded {
                    "bg-primary animate-pulse"
                  } else {
                    "bg-error"
                  }
                )}
              ></div>
              <div class="flex flex-col text-left">
                <span class="font-label-caps text-label-caps text-primary uppercase font-semibold">
                  {if props.is_config_loaded {
                    format!("Config Loaded (v{})", props.config_version)
                  } else {
                    "Awaiting Initialization".to_string()
                  }}
                </span>
                <span class="font-code-sm text-code-sm text-on-surface">
                  {&props.bound_filename}
                </span>
              </div>
            </div>
          </div>

          // Right header actions: Help Button + Prominent Reset Button + Export (Website) / Save & Export (Tauri)
          <div class="flex items-center gap-space-sm">
            // Help / Documentation Button (toggles built-in precompiled Markdown documentation viewer)
            <button
              class={if props.show_help_docs {
                "flex items-center gap-space-xs bg-secondary text-on-secondary px-space-md py-space-xs rounded font-bold transition-colors shadow cursor-pointer"
              } else {
                "flex items-center gap-space-xs bg-surface-container-highest hover:bg-surface-bright text-secondary border border-secondary/40 px-space-md py-space-xs rounded font-bold transition-colors shadow cursor-pointer"
              }}
              title="Read OrcaSlicer WebConfig documentation and tuning recipes"
              onclick={
                let cb = props.on_toggle_help_docs.clone();
                Callback::from(move |_| cb.emit(()))
              }
            >
              <span class="material-symbols-outlined text-[16px]">
                {if props.show_help_docs { "arrow_back" } else { "help" }}
              </span>
              <span class="font-code-md text-code-md font-bold">
                {if props.show_help_docs { "Back to App" } else { "Help" }}
              </span>
            </button>

            // Reset Button: opens confirmation dialog before clearing everything and returning to Initialization
            <button
              class="flex items-center gap-space-xs bg-error-container hover:bg-error text-on-error-container hover:text-on-error border border-error px-space-md py-space-xs rounded font-bold transition-colors shadow cursor-pointer"
              title="Clear all loaded configs, rigs, filaments, and transformations and return to Initialization"
              onclick={
                let cb = props.on_reset_all.clone();
                Callback::from(move |_| cb.emit(()))
              }
            >
              <span class="material-symbols-outlined text-[16px]">{"restart_alt"}</span>
              <span class="font-code-md text-code-md font-bold">{"Reset All"}</span>
            </button>

            if props.is_config_loaded {
              // Export (Website mode) / Save & Export (Tauri Desktop mode)
              <button
                class={if props.is_tauri && (props.pending_save_count > 0 || props.has_unapplied_changes) {
                  "flex items-center gap-space-xs bg-primary-container text-on-primary-container px-space-md py-space-xs rounded font-bold hover:bg-primary transition-colors shadow-lg border border-primary cursor-pointer"
                } else {
                  "flex items-center gap-space-xs bg-primary text-on-primary px-space-md py-space-xs rounded font-bold hover:opacity-90 transition-opacity shadow cursor-pointer"
                }}
                title={if props.is_tauri {
                  "Open Save & Export menu (Orca WebConfig, OrcaSlicer Config ZIP, 3MF Loadout, Save to Disk)"
                } else {
                  "Open Export menu (Orca WebConfig, OrcaSlicer Config ZIP, 3MF Loadout)"
                }}
                onclick={
                  let cb = props.on_open_export_modal.clone();
                  Callback::from(move |_| cb.emit(()))
                }
              >
                <span class="material-symbols-outlined text-[16px]">{"ios_share"}</span>
                <span class="font-code-md text-code-md font-bold">
                  if props.is_tauri {
                    if props.pending_save_count > 0 {
                      {format!("Save & Export ({} Staged)", props.pending_save_count)}
                    } else {
                      {"Save & Export"}
                    }
                  } else {
                    {"Export"}
                  }
                </span>
              </button>
            }
          </div>
        </div>

        // Bottom row: Navigation Tabs
        <div class="flex items-center justify-between gap-space-md w-full h-12 pt-space-xs">
          <nav class="flex items-center gap-space-xs">
            <button
              class={tab_class(ActiveTab::Home)}
              onclick={
                let cb = on_select_tab.clone();
                Callback::from(move |_| cb.emit(ActiveTab::Home))
              }
            >
              {"1. Overview & Config Sync"}
            </button>
            <button
              class={tab_class(ActiveTab::Machine)}
              onclick={
                let cb = on_select_tab.clone();
                Callback::from(move |_| cb.emit(ActiveTab::Machine))
              }
            >
              {"2. Printer & Rig Vault"}
            </button>
            <button
              class={tab_class(ActiveTab::Filament)}
              onclick={
                let cb = on_select_tab.clone();
                Callback::from(move |_| cb.emit(ActiveTab::Filament))
              }
            >
              {"3. Filaments & Base Deltas"}
            </button>
            <button
              class={tab_class(ActiveTab::Wizard)}
              onclick={
                let cb = on_select_tab.clone();
                Callback::from(move |_| cb.emit(ActiveTab::Wizard))
              }
            >
              {"4. Wizard / Lua Changer"}
            </button>
          </nav>
        </div>
      </div>
    </header>
  }
}
