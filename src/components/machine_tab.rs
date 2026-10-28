use crate::models::{MachineRig, ProcessProfile, RigAddOn};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct MachineTabProps {
  pub is_config_loaded: bool,
  pub machines: Vec<MachineRig>,
  pub active_machine_id: String,
  pub on_set_active_machine: Callback<String>,
  pub on_duplicate_active_rig: Callback<()>,
  pub on_add_machine: Callback<MachineRig>,
  pub on_change_machine_type: Callback<String>,
  pub on_change_nozzle: Callback<String>, // "diamondback", "brass", "hardened_steel", "cht"
  pub on_change_nozzle_diameter: Callback<String>, // "0.4", "0.6", "0.8"
  pub on_toggle_addon: Callback<String>,
  pub on_change_ace_units: Callback<usize>, // 1 (4 slots) or 2 (8 slots Double ACE Pro 2)
  pub on_save_rig_to_vault: Callback<()>,
  pub on_open_bind_modal: Callback<()>,
  pub active_process: ProcessProfile,
  pub safe_speed_ceiling: f64,
}

#[function_component]
pub fn MachineTab(props: &MachineTabProps) -> Html {
  let show_template_picker = use_state(|| false);

  let on_set_active = props.on_set_active_machine.clone();
  let on_change_nozzle = props.on_change_nozzle.clone();
  let on_change_diam = props.on_change_nozzle_diameter.clone();
  let on_toggle_addon = props.on_toggle_addon.clone();
  let on_change_ace = props.on_change_ace_units.clone();
  let on_change_machine_type = props.on_change_machine_type.clone();
  let on_add_machine = props.on_add_machine.clone();

  if !props.is_config_loaded || props.machines.is_empty() {
    return html! {
      <div class="flex flex-col items-center justify-center bg-surface-container-low p-space-xl rounded-2xl border border-outline-variant gap-space-md text-center my-8">
        <span class="material-symbols-outlined text-primary text-5xl">{"precision_manufacturing"}</span>
        <h2 class="font-headline-lg text-headline-lg text-on-surface font-bold">
          {"No Machine Rigs Loaded Yet"}
        </h2>
        <p class="text-body-md text-on-surface-variant max-w-xl">
          {"Nothing is loaded. Use the startup Import Modal (or click Reset All in the top right) to import your configuration directory first."}
        </p>
      </div>
    };
  }

  let active_rig = props
    .machines
    .iter()
    .find(|m| m.id == props.active_machine_id)
    .cloned()
    .unwrap_or_else(MachineRig::kobra_s1_hardened_steel_png);

  let addon_catalog = RigAddOn::get_catalog();
  let template_catalog = MachineRig::get_template_catalog();

  html! {
    <div class="flex flex-col w-full gap-space-lg">
      // Top Header Banner with Duplicate Rig & Add New Rig Actions
      <div class="flex flex-col lg:flex-row lg:items-center justify-between gap-space-lg bg-surface-container-low p-space-xl rounded-xl shadow-md border border-outline-variant">
        <div class="flex flex-col gap-space-xs">
          <div class="flex items-center gap-space-sm">
            <span class="material-symbols-outlined text-primary text-3xl">
              {"precision_manufacturing"}
            </span>
            <h1 class="font-headline-xl text-headline-xl text-on-surface font-bold">
              {"Printer & Rig Vault — Machine Type + Nozzle + Add-Ons"}
            </h1>
          </div>
          <p class="text-body-md text-on-surface-variant max-w-3xl">
            {"Each rig is composed of (1) Machine Type, (2) Nozzle Material & Diameter, and (3) Hardware Add-Ons (such as Hollow Front/Back Covers or a MicroSwiss Dual-Fang Duct that modify cooling schedules). Duplicate the current rig to experiment with a new nozzle or add-on without mutating your imported baseline."}
          </p>
        </div>

        <div class="flex flex-wrap items-center gap-space-sm shrink-0">
          <button
            class="flex items-center gap-space-xs bg-primary-container hover:bg-primary text-on-primary-container px-space-lg py-space-md rounded-lg font-bold shadow-md transition-colors text-xs font-mono cursor-pointer"
            title="Duplicate the currently active rig so you can swap its nozzle (e.g. Diamondback) or add cooling mods"
            onclick={
              let cb = props.on_duplicate_active_rig.clone();
              Callback::from(move |_| cb.emit(()))
            }
          >
            <span class="material-symbols-outlined text-[18px]">{"content_copy"}</span>
            {"Duplicate Current Rig & Modify"}
          </button>

          <button
            class="flex items-center gap-space-xs bg-surface-container-highest hover:bg-surface-bright text-secondary border border-secondary/40 px-space-lg py-space-md rounded-lg font-bold transition-colors text-xs font-mono cursor-pointer"
            onclick={
              let st = show_template_picker.clone();
              Callback::from(move |_| st.set(!*st))
            }
          >
            <span class="material-symbols-outlined text-[18px]">{"add_circle"}</span>
            {"Select New Rig Template..."}
          </button>
        </div>
      </div>

      // New Rig Template Picker Drawer
      if *show_template_picker {
        <div class="bg-surface-container p-space-lg rounded-xl border border-secondary shadow-lg flex flex-col gap-space-md">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-space-sm">
              <span class="material-symbols-outlined text-secondary">{"library_add"}</span>
              <span class="font-headline-sm text-body-md text-on-surface font-bold">
                {"Select a New Rig Template to Add to Your Machine Vault"}
              </span>
            </div>
            <button
              class="text-xs text-on-surface-variant hover:text-on-surface"
              onclick={
                let st = show_template_picker.clone();
                Callback::from(move |_| st.set(false))
              }
            >
              {"Close"}
            </button>
          </div>
          <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-space-md">
            {for template_catalog.into_iter().map(|tpl| {
              let cb_add = on_add_machine.clone();
              let st = show_template_picker.clone();
              let mut new_rig = tpl.clone();
              new_rig.id = format!("{}_{}", tpl.id, props.machines.len() + 1);

              html! {
                <div class="bg-surface-container-low p-space-md rounded-lg border border-outline-variant flex flex-col justify-between gap-space-sm">
                  <div class="flex flex-col gap-1">
                    <span class="text-[10px] font-mono uppercase text-primary font-bold">{&tpl.printer_model}</span>
                    <span class="font-headline-sm text-xs text-on-surface font-bold">{&tpl.name}</span>
                    <span class="text-[11px] font-mono text-secondary">
                      {format!("Nozzle: {} ({}mm)", tpl.nozzle_material, tpl.nozzle_diameter)}
                    </span>
                    <span class="text-[11px] text-on-surface-variant">
                      {format!("Add-ons: {}", if tpl.addons.is_empty() { "Stock".to_string() } else { tpl.addons.join(", ") })}
                    </span>
                  </div>
                  <button
                    class="bg-secondary/20 hover:bg-secondary/30 text-secondary border border-secondary/40 py-1.5 rounded text-xs font-mono font-bold transition-colors"
                    onclick={Callback::from(move |_| {
                      cb_add.emit(new_rig.clone());
                      st.set(false);
                    })}
                  >
                    {"+ Add Rig to Vault"}
                  </button>
                </div>
              }
            })}
          </div>
        </div>
      }

      // Machine Bank Grid
      <div class="flex flex-col gap-space-md">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-space-md">
            <span class="font-headline-sm text-headline-sm text-on-surface font-semibold">
              {"Imported & Custom Rigs in Vault"}
            </span>
            <span class="bg-surface-container-highest text-on-surface px-space-sm py-0.5 rounded text-xs font-mono">
              {format!("{} Rigs Available", props.machines.len())}
            </span>
          </div>
        </div>

        <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-space-lg">
          {for props.machines.iter().map(|machine| {
            let is_active = machine.id == props.active_machine_id;
            let mid = machine.id.clone();
            let cb_set = on_set_active.clone();
            let concise_title = machine.format_concise_title();
            let show_disk_name = machine.name != concise_title;

            html! {
              <div
                class={format!(
                  "flex flex-col justify-between p-space-lg rounded-xl border transition-all duration-200 {}",
                  if is_active {
                    "bg-surface-container border-primary shadow-lg ring-1 ring-primary/40"
                  } else {
                    "bg-surface-container-low border-outline-variant hover:border-surface-bright"
                  }
                )}
              >
                <div class="flex flex-col gap-space-md">
                  <div class="flex items-start justify-between gap-space-sm">
                    <div class="flex flex-col">
                      <div class="flex items-center gap-space-xs flex-wrap">
                        <span class="text-xs uppercase text-primary font-mono font-semibold">
                          {&machine.printer_model}
                        </span>
                        <span class="text-[10px] font-mono bg-surface-container-highest text-tertiary px-1.5 py-0.5 rounded">
                          {if machine.ace_pro_units == 2 {
                            "Double ACE Pro 2 (8-Slot)"
                          } else {
                            "Single ACE Pro (4-Slot)"
                          }}
                        </span>
                      </div>
                      <h2 class="font-headline-sm text-body-md text-on-surface font-bold mt-0.5">
                        {&concise_title}
                      </h2>
                      if show_disk_name {
                        <span class="text-[11px] font-mono text-on-surface-variant truncate" title={machine.name.clone()}>
                          {format!("Disk Preset: {}", machine.name)}
                        </span>
                      }
                    </div>
                    if is_active {
                      <span class="bg-primary text-on-primary text-[10px] font-bold uppercase px-2 py-0.5 rounded-full shrink-0">
                        {"Active Rig"}
                      </span>
                    }
                  </div>

                  // 3-Part Rig Summary: Machine Type, Nozzle, Add-Ons
                  <div class="flex flex-col gap-1.5 text-xs text-on-surface-variant bg-surface-container-lowest p-space-sm rounded font-mono border border-outline-variant/50">
                    <div class="flex justify-between">
                      <span>{"1. Machine Type:"}</span>
                      <span class="text-on-surface font-bold">{&machine.printer_model}</span>
                    </div>
                    <div class="flex justify-between">
                      <span>{"2. Nozzle:"}</span>
                      <span class="text-primary font-bold">
                        {format!("{} ({}mm)", machine.nozzle_material, machine.nozzle_diameter)}
                      </span>
                    </div>
                    <div class="flex justify-between">
                      <span>{"OrcaSlicer Nozzle / HRC:"}</span>
                      <span class="text-tertiary font-bold">
                        {{
                          let (orca_noz, hrc) = machine.orca_machine_nozzle_type_and_hrc();
                          format!("nozzle_type={} • HRC {}", orca_noz, hrc)
                        }}
                      </span>
                    </div>
                    <div class="flex justify-between items-start">
                      <span>{"3. Add-Ons:"}</span>
                      <span class="text-secondary text-right font-medium max-w-[180px] truncate">
                        {if machine.addons.is_empty() {
                          "None (Stock Covers/Duct)".to_string()
                        } else {
                          format!("{} installed", machine.addons.len())
                        }}
                      </span>
                    </div>
                    if let Some(src) = &machine.source_disk_profile {
                      <div class="flex justify-between pt-1 border-t border-surface-container text-[10px]">
                        <span>{"Disk Source:"}</span>
                        <span class="text-tertiary truncate max-w-[190px]">{src}</span>
                      </div>
                    }
                  </div>

                  if !machine.notes.is_empty() {
                    <div class="bg-surface-container-lowest/80 p-space-sm rounded border border-outline-variant/40 flex flex-col gap-1">
                      <span class="text-[10px] uppercase font-mono text-secondary font-bold">
                        {"Machine Profile Notes (printer_notes):"}
                      </span>
                      <pre class="text-[10px] font-mono text-on-surface-variant whitespace-pre-wrap leading-relaxed">
                        {&machine.notes}
                      </pre>
                    </div>
                  }
                </div>

                // Card Footer: Set Active Button
                <div class="flex items-center justify-between pt-space-md mt-space-md border-t border-surface-container">
                  <span class="text-xs text-on-surface-variant font-mono">
                    {format!("Retract: {:.2}mm • Max: {}", machine.base_retraction_length, machine.max_volumetric_flow)}
                  </span>

                  if is_active {
                    <div class="flex items-center gap-1 text-primary text-xs font-bold font-mono">
                      <span class="material-symbols-outlined text-[16px]">{"check_circle"}</span>
                      {"Current Active"}
                    </div>
                  } else {
                    <button
                      class="bg-surface-container-highest hover:bg-surface-bright text-on-surface px-space-md py-1 rounded text-xs font-semibold transition-colors"
                      onclick={Callback::from(move |_| cb_set.emit(mid.clone()))}
                    >
                      {"Set Active"}
                    </button>
                  }
                </div>
              </div>
            }
          })}
        </div>
      </div>

      // Active Rig 3-Part Configurator: (1) Machine Type, (2) Nozzle, (3) Add-Ons
      <div class="flex flex-col gap-space-md bg-surface-container-low p-space-xl rounded-xl shadow-md border border-outline-variant">
        <div class="flex items-center justify-between flex-wrap gap-space-sm">
          <div class="flex items-center gap-space-sm">
            <span class="material-symbols-outlined text-secondary">{"hardware"}</span>
            <div class="flex flex-col">
              <h2 class="font-headline-sm text-headline-sm text-on-surface font-bold">
                {format!("Configure Active Rig: {}", active_rig.format_concise_title())}
              </h2>
              if active_rig.name != active_rig.format_concise_title() {
                <span class="text-xs font-mono text-on-surface-variant">
                  {format!("Disk Preset Name: {}", active_rig.name)}
                </span>
              }
            </div>
          </div>
          <span class="text-xs text-secondary font-mono bg-surface-container-lowest px-2 py-1 rounded">
            {"Changes apply as Lua transformations without mutating base profiles"}
          </span>
        </div>

        <div class="grid grid-cols-1 lg:grid-cols-12 gap-space-lg items-start">
          // Left 4 cols: Part 1 — Machine Type & ACE Pro Hub
          <div class="lg:col-span-4 flex flex-col gap-space-md bg-surface-container p-space-lg rounded-xl border border-outline-variant/60">
            <span class="font-headline-sm text-body-md text-primary font-bold flex items-center gap-2">
              <span class="material-symbols-outlined text-[18px]">{"precision_manufacturing"}</span>
              {"1. Machine Type & Chassis"}
            </span>

            <div class="flex flex-col gap-space-xs">
              <label class="text-xs text-on-surface-variant font-semibold">
                {"Select Base Machine Type:"}
              </label>
              <div class="flex flex-col gap-space-xs">
                {for vec![
                  ("Anycubic Kobra S1", "CoreXY Enclosed • 250×250×250mm • Kobra OS"),
                  ("Anycubic Kobra 3", "Bedslinger High-Speed • 250×250×260mm • Kobra OS"),
                  ("Voron 2.4 R2 (350mm)", "CoreXY Flying Gantry • 350×350×330mm • Klipper"),
                ].into_iter().map(|(m_model, m_desc)| {
                  let is_sel = active_rig.printer_model == m_model;
                  let mm = m_model.to_string();
                  let cb = on_change_machine_type.clone();

                  html! {
                    <button
                      class={format!(
                        "p-space-sm rounded-lg border text-xs font-mono text-left flex flex-col transition-all {}",
                        if is_sel {
                          "bg-primary-container/20 border-primary text-primary font-bold"
                        } else {
                          "bg-surface-container-lowest border-outline-variant text-on-surface-variant hover:text-on-surface"
                        }
                      )}
                      onclick={Callback::from(move |_| cb.emit(mm.clone()))}
                    >
                      <span>{m_model}</span>
                      <span class="text-[10px] text-on-surface-variant mt-0.5">{m_desc}</span>
                    </button>
                  }
                })}
              </div>
            </div>

            // ACE Pro Hub Units Selector
            <div class="flex flex-col gap-space-xs pt-space-xs border-t border-surface-container-high">
              <label class="text-xs text-on-surface-variant font-semibold">
                {"ACE Pro Multi-Material Hub:"}
              </label>
              <div class="grid grid-cols-2 gap-space-xs">
                <button
                  class={format!(
                    "p-space-sm rounded-lg border text-xs font-mono text-center flex flex-col items-center transition-all {}",
                    if active_rig.ace_pro_units == 1 {
                      "bg-primary-container/20 border-primary text-primary font-bold"
                    } else {
                      "bg-surface-container-lowest border-outline-variant text-on-surface-variant hover:text-on-surface"
                    }
                  )}
                  onclick={
                    let cb = on_change_ace.clone();
                    Callback::from(move |_| cb.emit(1))
                  }
                >
                  <span>{"Single ACE (4-Slot)"}</span>
                  <span class="text-[10px] text-on-surface-variant">{"4×4 Matrix"}</span>
                </button>
                <button
                  class={format!(
                    "p-space-sm rounded-lg border text-xs font-mono text-center flex flex-col items-center transition-all {}",
                    if active_rig.ace_pro_units == 2 {
                      "bg-primary-container/20 border-primary text-primary font-bold"
                    } else {
                      "bg-surface-container-lowest border-outline-variant text-on-surface-variant hover:text-on-surface"
                    }
                  )}
                  onclick={
                    let cb = on_change_ace.clone();
                    Callback::from(move |_| cb.emit(2))
                  }
                >
                  <span>{"Double ACE 2 (8-Slot)"}</span>
                  <span class="text-[10px] text-tertiary">{"8×8 Matrix"}</span>
                </button>
              </div>
            </div>
          </div>

          // Middle 4 cols: Part 2 — Nozzle Material & Orifice Diameter
          <div class="lg:col-span-4 flex flex-col gap-space-md bg-surface-container p-space-lg rounded-xl border border-outline-variant/60">
            <span class="font-headline-sm text-body-md text-secondary font-bold flex items-center gap-2">
              <span class="material-symbols-outlined text-[18px]">{"local_fire_department"}</span>
              {"2. Nozzle Material & Diameter"}
            </span>

            <div class="flex flex-col gap-space-xs">
              <label class="text-xs text-on-surface-variant font-semibold">
                {"Installed Nozzle Material (Automatically Applies Default Rules):"}
              </label>
              <div class="flex flex-col gap-space-xs">
                {for vec![
                  ("brass", "🟡 Standard Brass (OrcaSlicer Default)", "0°C Factory Baseline Conductivity"),
                  ("microswiss_brass", "🟡 MicroSwiss Plated Brass (TwinClad XT™)", "0°C Offset • Low-Friction 0.99 Flow • 0.2/0.4/0.6mm"),
                  ("ruby_brass", "🔴 Brass-Body Ruby Tip (Olsson / Gemstone)", "0°C Brass Thermal Conductivity + Handles -CF & -GF Composites"),
                  ("hardened_steel", "🛡️ Hardened Steel", "+5°C Thermal Compensation (230°C → 235°C PETG)"),
                  ("diamondback", "💎 MicroSwiss Diamondback™ PCD", "-10°C Temp, 0.94 Flow, 0.60mm Retract (0.4mm)"),
                  ("cht", "🔥 MicroSwiss / Bi-Metal CHT High-Flow", "+25% High-Flow Volumetric Capacity"),
                ].into_iter().map(|(noz_id, noz_label, sub)| {
                  let is_sel = active_rig.nozzle_type == noz_id;
                  let nid = noz_id.to_string();
                  let cb_noz = on_change_nozzle.clone();

                  html! {
                    <button
                      class={format!(
                        "p-space-sm rounded-lg border text-xs font-mono text-left flex flex-col justify-between transition-all {}",
                        if is_sel {
                          "bg-primary-container/20 border-primary text-primary font-bold shadow-sm"
                        } else {
                          "bg-surface-container-lowest border-outline-variant text-on-surface-variant hover:text-on-surface"
                        }
                      )}
                      onclick={Callback::from(move |_| cb_noz.emit(nid.clone()))}
                    >
                      <span>{noz_label}</span>
                      <span class="text-[10px] text-on-surface-variant mt-0.5">{sub}</span>
                    </button>
                  }
                })}
              </div>
            </div>

            // Nozzle Diameter Selector (0.4, 0.6, 0.8)
            <div class="flex flex-col gap-space-xs pt-space-xs border-t border-surface-container-high">
              <label class="text-xs text-on-surface-variant font-semibold">
                {"Nozzle Orifice Diameter (mm):"}
              </label>
              <div class="grid grid-cols-3 gap-space-xs">
                {for vec![
                  ("0.4", "0.40 mm"),
                  ("0.6", "0.60 mm"),
                  ("0.8", "0.80 mm"),
                ].into_iter().map(|(d_val, d_label)| {
                  let is_sel = active_rig.nozzle_diameter == d_val;
                  let dv = d_val.to_string();
                  let cb_d = on_change_diam.clone();

                  html! {
                    <button
                      class={format!(
                        "p-space-sm rounded-lg border text-xs font-mono text-center transition-all {}",
                        if is_sel {
                          "bg-secondary/20 border-secondary text-secondary font-bold"
                        } else {
                          "bg-surface-container-lowest border-outline-variant text-on-surface-variant hover:text-on-surface"
                        }
                      )}
                      onclick={Callback::from(move |_| cb_d.emit(dv.clone()))}
                    >
                      {d_label}
                    </button>
                  }
                })}
              </div>
            </div>
          </div>

          // Right 4 cols: Part 3 — Physical Add-Ons (Cooling Schedules, Ducts, Hotend)
          <div class="lg:col-span-4 flex flex-col gap-space-md bg-surface-container p-space-lg rounded-xl border border-outline-variant/60">
            <span class="font-headline-sm text-body-md text-tertiary font-bold flex items-center gap-2">
              <span class="material-symbols-outlined text-[18px]">{"air"}</span>
              {"3. Rig Add-Ons & Cooling Mods"}
            </span>
            <p class="text-xs text-on-surface-variant">
              {"Toggle physical toolhead mods installed on this rig. Each add-on dynamically patches cooling fan schedules or retraction via transformations/rig_addons_cooling.lua:"}
            </p>

            <div class="flex flex-col gap-space-sm">
              {for addon_catalog.into_iter().map(|addon| {
                let is_installed = active_rig.addons.iter().any(|a| a == &addon.id);
                let aid = addon.id.clone();
                let cb_toggle = on_toggle_addon.clone();

                html! {
                  <div
                    class={format!(
                      "p-space-md rounded-lg border flex flex-col gap-1 cursor-pointer transition-all {}",
                      if is_installed {
                        "bg-tertiary-container/20 border-tertiary shadow-sm"
                      } else {
                        "bg-surface-container-lowest border-outline-variant hover:border-surface-bright"
                      }
                    )}
                    onclick={Callback::from(move |_| cb_toggle.emit(aid.clone()))}
                  >
                    <div class="flex items-center justify-between">
                      <span class="font-headline-sm text-xs text-on-surface font-bold">
                        {&addon.name}
                      </span>
                      <span
                        class={format!(
                          "px-2 py-0.5 rounded text-[10px] font-mono font-bold {}",
                          if is_installed {
                            "bg-tertiary text-on-tertiary"
                          } else {
                            "bg-surface-container-highest text-on-surface-variant"
                          }
                        )}
                      >
                        {if is_installed { "INSTALLED" } else { "OFF" }}
                      </span>
                    </div>
                    <span class="text-[10px] font-mono text-secondary uppercase">
                      {&addon.category}
                    </span>
                    <p class="text-[11px] text-on-surface-variant mt-0.5">
                      {&addon.effect_summary}
                    </p>
                  </div>
                }
              })}
            </div>
          </div>
        </div>
      </div>
    </div>
  }
}
