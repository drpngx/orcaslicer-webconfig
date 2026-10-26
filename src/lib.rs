pub mod components;
pub mod fs_access;
pub mod lua_engine;
pub mod models;
pub mod orca_profile;
pub mod transformations;

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::components::filament_tab::FilamentTab;
use crate::components::home_tab::HomeTab;
use crate::components::machine_tab::MachineTab;
use crate::components::nav::{ActiveTab, NavigationBar};
use crate::components::wizard_tab::WizardTab;
use crate::fs_access::{show_open_file_picker, FileSystemFileHandle, FsManager};
use crate::models::{FilamentSpool, MachineRig};
use crate::orca_profile::OrcaProfile;
use crate::transformations::{ExternalConflict, StackEvaluationResult, TransformationEngine, TransformationRule};

#[function_component]
pub fn App() -> Html {
    let active_tab = use_state(|| ActiveTab::Home);

    // Machine Rigs
    let machines = use_state(|| vec![
        MachineRig::default_kobra_diamondback(),
        MachineRig::default_kobra_stock(),
        MachineRig::default_voron_24(),
    ]);
    let active_machine_id = use_state(|| "kobra_s1_diamondback".to_string());

    // Filaments
    let filaments = use_state(FilamentSpool::get_default_spools);
    let active_filament_type = use_state(|| "PETG".to_string());

    // Profiles & Rules
    let base_profile = use_state(OrcaProfile::get_stock_kobra_s1);
    let rules = use_state(TransformationEngine::get_default_rules);

    // File Access State
    let file_handle = use_state(|| None::<FileSystemFileHandle>);
    let bound_filename = use_state(|| "kobra_s1_diamondback_0.4.json".to_string());
    let bound_path_hint = use_state(|| "~/AppData/Roaming/OrcaSlicer/system/kobra_s1_diamondback_0.4.json".to_string());
    let last_saved_time = use_state(|| "Not saved in session".to_string());
    let disconnect_after_save = use_state(|| true);

    // UI state
    let show_diff_drawer = use_state(|| false);
    let toast_message = use_state(|| None::<String>);
    let conflicts = use_state(|| Vec::<ExternalConflict>::new());
    let execution_logs = use_state(|| Vec::<String>::new());

    // Context for Lua Evaluation
    let active_rig = machines.iter().find(|m| m.id == *active_machine_id).cloned().unwrap_or_else(MachineRig::default_kobra_diamondback);
    let eval_context = serde_json::json!({
        "printer_model": active_rig.printer_model,
        "nozzle_type": active_rig.nozzle_type,
        "nozzle_diameter": active_rig.nozzle_diameter,
        "filament_type": *active_filament_type,
    });

    // Evaluate Stack
    let eval_result: StackEvaluationResult = TransformationEngine::evaluate_stack(&base_profile, &rules, &eval_context);

    // Compute JSON diff preview
    let base_json_str = base_profile.to_json_pretty();
    let derived_json_str = eval_result.resulting_profile.to_json_pretty();

    let mut diff_lines = Vec::new();
    let base_lines: Vec<&str> = base_json_str.lines().collect();
    let derived_lines: Vec<&str> = derived_json_str.lines().collect();

    for (b_line, d_line) in base_lines.iter().zip(derived_lines.iter()) {
        if b_line == d_line {
            diff_lines.push(format!("  {}", b_line));
        } else {
            diff_lines.push(format!("- {}", b_line));
            diff_lines.push(format!("+ {}", d_line));
        }
    }
    let json_diff_text = diff_lines.join("\n");

    // Handlers
    let on_select_tab = {
        let active_tab = active_tab.clone();
        Callback::from(move |tab: ActiveTab| {
            active_tab.set(tab);
        })
    };

    let on_toggle_rule = {
        let rules = rules.clone();
        Callback::from(move |rule_id: String| {
            let mut list = (*rules).clone();
            if let Some(r) = list.iter_mut().find(|r| r.id == rule_id) {
                r.enabled = !r.enabled;
            }
            rules.set(list);
        })
    };

    let on_update_rule_lua = {
        let rules = rules.clone();
        Callback::from(move |(rule_id, new_lua): (String, String)| {
            let mut list = (*rules).clone();
            if let Some(r) = list.iter_mut().find(|r| r.id == rule_id) {
                r.lua_code = new_lua;
            }
            rules.set(list);
        })
    };

    let on_change_nozzle = {
        let machines = machines.clone();
        let active_machine_id = active_machine_id.clone();
        let rules = rules.clone();
        let toast = toast_message.clone();

        Callback::from(move |new_nozzle_type: String| {
            let mut mach_list = (*machines).clone();
            if let Some(m) = mach_list.iter_mut().find(|m| m.id == *active_machine_id) {
                m.nozzle_type = new_nozzle_type.clone();
                if new_nozzle_type == "diamondback" {
                    m.nozzle_name = "Diamondback PCD Diamond 0.40mm".to_string();
                    // Auto-enable Diamondback transformation rule
                    let mut r_list = (*rules).clone();
                    if let Some(r) = r_list.iter_mut().find(|r| r.id == "diamondback_nozzle") {
                        r.enabled = true;
                    }
                    rules.set(r_list);
                    toast.set(Some("Installed Diamondback PCD Nozzle: auto-activated Diamondback thermal tuning!".to_string()));
                } else if new_nozzle_type == "brass" {
                    m.nozzle_name = "Standard Volcano Brass 0.40mm".to_string();
                    // Disable Diamondback rule
                    let mut r_list = (*rules).clone();
                    if let Some(r) = r_list.iter_mut().find(|r| r.id == "diamondback_nozzle") {
                        r.enabled = false;
                    }
                    rules.set(r_list);
                    toast.set(Some("Installed Standard Brass Nozzle: reverted to stock thermal profile.".to_string()));
                } else {
                    m.nozzle_name = "Hardened Steel Wear-Resistant 0.40mm".to_string();
                    toast.set(Some("Installed Hardened Steel Nozzle.".to_string()));
                }
            }
            machines.set(mach_list);
        })
    };

    let on_select_active_filament = {
        let active_filament_type = active_filament_type.clone();
        let toast = toast_message.clone();
        Callback::from(move |mat: String| {
            active_filament_type.set(mat.clone());
            toast.set(Some(format!("Active slicing material set to {}. Slicing rules updated!", mat)));
        })
    };

    let on_set_active_machine = {
        let active_machine_id = active_machine_id.clone();
        let machines = machines.clone();
        let toast = toast_message.clone();
        Callback::from(move |mid: String| {
            active_machine_id.set(mid.clone());
            let mut list = (*machines).clone();
            for m in &mut list {
                m.is_active = m.id == mid;
            }
            machines.set(list);
            toast.set(Some(format!("Active machine rig updated.")));
        })
    };

    let on_toggle_disconnect_after_save = {
        let disconnect_after_save = disconnect_after_save.clone();
        Callback::from(move |_| {
            disconnect_after_save.set(!*disconnect_after_save);
        })
    };

    let on_disconnect_file = {
        let file_handle = file_handle.clone();
        let bound_filename = bound_filename.clone();
        let toast = toast_message.clone();
        Callback::from(move |_| {
            file_handle.set(None);
            toast.set(Some(format!("Disconnected from {}. File lock released!", *bound_filename)));
        })
    };

    let on_commit_save = {
        let file_handle = file_handle.clone();
        let bound_filename = bound_filename.clone();
        let disconnect_after_save = disconnect_after_save.clone();
        let last_saved_time = last_saved_time.clone();
        let toast = toast_message.clone();
        let resulting_json = eval_result.resulting_profile.to_json_pretty();

        Callback::from(move |_| {
            let handle_opt = (*file_handle).clone();
            let filename = (*bound_filename).clone();
            let should_disconnect = *disconnect_after_save;
            let fh = file_handle.clone();
            let lst = last_saved_time.clone();
            let t = toast.clone();
            let content = resulting_json.clone();

            spawn_local(async move {
                if let Some(handle) = handle_opt {
                    match FsManager::write_file(&handle, &content).await {
                        Ok(()) => {
                            lst.set("Just now".to_string());
                            if should_disconnect {
                                fh.set(None);
                                t.set(Some(format!("Committed changes to '{}' and released file lock successfully!", filename)));
                            } else {
                                t.set(Some(format!("Committed changes to '{}' successfully!", filename)));
                            }
                        }
                        Err(err) => {
                            t.set(Some(format!("Failed to write to file: {}", err)));
                        }
                    }
                } else {
                    // Trigger download fallback
                    if let Err(e) = FsManager::trigger_browser_download(&filename, &content) {
                        t.set(Some(format!("Download error: {}", e)));
                    } else {
                        lst.set("Just now".to_string());
                        t.set(Some(format!("Exported '{}' to disk.", filename)));
                    }
                }
            });
        })
    };

    let on_rebind_file = {
        let file_handle = file_handle.clone();
        let bound_filename = bound_filename.clone();
        let bound_path_hint = bound_path_hint.clone();
        let base_profile = base_profile.clone();
        let conflicts = conflicts.clone();
        let toast = toast_message.clone();
        let rules_list = (*rules).clone();
        let current_derived = eval_result.resulting_profile.clone();

        Callback::from(move |_| {
            let fh = file_handle.clone();
            let bfn = bound_filename.clone();
            let bph = bound_path_hint.clone();
            let bp = base_profile.clone();
            let c_state = conflicts.clone();
            let t = toast.clone();
            let r_list = rules_list.clone();
            let derived = current_derived.clone();

            spawn_local(async move {
                let options = js_sys::Object::new();
                let types = js_sys::Array::new();
                let type0 = js_sys::Object::new();
                let _ = js_sys::Reflect::set(&type0, &"description".into(), &"OrcaSlicer Profile (*.json, *.conf)".into());
                let accept = js_sys::Object::new();
                let exts = js_sys::Array::new();
                exts.push(&".json".into());
                exts.push(&".conf".into());
                let _ = js_sys::Reflect::set(&accept, &"application/json".into(), &exts);
                let _ = js_sys::Reflect::set(&type0, &"accept".into(), &accept);
                types.push(&type0);
                let _ = js_sys::Reflect::set(&options, &"types".into(), &types);

                let promise = show_open_file_picker(&options.into());
                match wasm_bindgen_futures::JsFuture::from(promise).await {
                    Ok(handles_val) => {
                        let handles: js_sys::Array = handles_val.unchecked_into();
                        if handles.length() > 0 {
                            let handle: FileSystemFileHandle = handles.get(0).unchecked_into();
                            match FsManager::read_file(&handle).await {
                                Ok((name, content)) => {
                                    match OrcaProfile::from_json_str(&content) {
                                        Ok(parsed_profile) => {
                                            // Detect drift / conflicts against our transformation expectations
                                            let detected = TransformationEngine::detect_conflicts(
                                                &bp,
                                                &derived,
                                                &parsed_profile,
                                                &r_list,
                                            );

                                            bfn.set(name.clone());
                                            bph.set(format!("Direct File Bound: {}", name));
                                            bp.set(parsed_profile);
                                            fh.set(Some(handle));

                                            if !detected.is_empty() {
                                                c_state.set(detected);
                                                t.set(Some(format!("Connected to '{}'. External conflicts detected!", name)));
                                            } else {
                                                t.set(Some(format!("Connected to '{}' directly without conflicts!", name)));
                                            }
                                        }
                                        Err(e) => {
                                            t.set(Some(format!("JSON parsing error: {}", e)));
                                        }
                                    }
                                }
                                Err(e) => {
                                    t.set(Some(format!("Failed to read file: {}", e)));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::debug!("Picker cancelled or error: {:?}", e);
                    }
                }
            });
        })
    };

    let on_resolve_conflict = {
        let conflicts = conflicts.clone();
        let base_profile = base_profile.clone();
        let toast = toast_message.clone();

        Callback::from(move |(param_key, accept): (String, bool)| {
            let mut cur_conflicts = (*conflicts).clone();
            if let Some(pos) = cur_conflicts.iter().position(|c| c.param_key == param_key) {
                let conf = cur_conflicts.remove(pos);
                if accept {
                    // Update baseline with external value
                    let mut bp = (*base_profile).clone();
                    bp.set_str(&conf.param_key, &conf.actual_external_value);
                    base_profile.set(bp);
                    toast.set(Some(format!("Accepted external value '{}' for {}", conf.actual_external_value, conf.param_label)));
                } else {
                    toast.set(Some(format!("Discarded external value for {}. Restored transformation rule state.", conf.param_label)));
                }
                conflicts.set(cur_conflicts);
            }
        })
    };

    let on_reset_defaults = {
        let base_profile = base_profile.clone();
        let rules = rules.clone();
        let toast = toast_message.clone();
        Callback::from(move |_| {
            base_profile.set(OrcaProfile::get_stock_kobra_s1());
            rules.set(TransformationEngine::get_default_rules());
            toast.set(Some("Reset configuration and rules to stock defaults.".to_string()));
        })
    };

    let on_export_package = {
        let rules = rules.clone();
        let active_machine_id = active_machine_id.clone();
        let toast = toast_message.clone();
        Callback::from(move |_| {
            let pkg = TransformationEngine::export_package(&rules, &active_machine_id);
            if let Err(e) = FsManager::trigger_browser_download("orcaslicer_config_rules.json", &pkg) {
                toast.set(Some(format!("Export error: {}", e)));
            } else {
                toast.set(Some("1-Click Export: saved 'orcaslicer_config_rules.json'!".to_string()));
            }
        })
    };

    let on_import_package_trigger = {
        let rules = rules.clone();
        let active_machine_id = active_machine_id.clone();
        let toast = toast_message.clone();

        Callback::from(move |_| {
            let r_state = rules.clone();
            let m_state = active_machine_id.clone();
            let t = toast.clone();

            spawn_local(async move {
                let options = js_sys::Object::new();
                let types = js_sys::Array::new();
                let type0 = js_sys::Object::new();
                let _ = js_sys::Reflect::set(&type0, &"description".into(), &"OrcaSlicer Transformation Rules (*.json)".into());
                let accept = js_sys::Object::new();
                let exts = js_sys::Array::new();
                exts.push(&".json".into());
                let _ = js_sys::Reflect::set(&accept, &"application/json".into(), &exts);
                let _ = js_sys::Reflect::set(&type0, &"accept".into(), &accept);
                types.push(&type0);
                let _ = js_sys::Reflect::set(&options, &"types".into(), &types);

                let promise = show_open_file_picker(&options.into());
                match wasm_bindgen_futures::JsFuture::from(promise).await {
                    Ok(handles_val) => {
                        let handles: js_sys::Array = handles_val.unchecked_into();
                        if handles.length() > 0 {
                            let handle: FileSystemFileHandle = handles.get(0).unchecked_into();
                            if let Ok((_, content)) = FsManager::read_file(&handle).await {
                                match TransformationEngine::import_package(&content) {
                                    Ok(pkg) => {
                                        r_state.set(pkg.rules);
                                        m_state.set(pkg.active_rig_id);
                                        t.set(Some("1-Click Import: successfully loaded transformation rules and rigs!".to_string()));
                                    }
                                    Err(e) => {
                                        t.set(Some(format!("Import parsing failed: {}", e)));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::debug!("Import picker cancelled: {:?}", e);
                    }
                }
            });
        })
    };

    let on_add_custom_rule = {
        let rules = rules.clone();
        let toast = toast_message.clone();
        Callback::from(move |_| {
            let mut list = (*rules).clone();
            let new_rule = TransformationRule {
                id: format!("custom_rule_{}", list.len() + 1),
                name: format!("Custom User Rule #{}", list.len() + 1),
                description: "User-defined Lua transformation rule".to_string(),
                category: "Custom".to_string(),
                icon: "⚙️".to_string(),
                enabled: true,
                condition_expr: "all".to_string(),
                lua_code: r#"-- Custom User Transformation
function apply(config, ctx)
    -- Modify parameters here:
    print("[CustomRule] Executing custom Lua rule...")
end
"#.to_string(),
                priority: (list.len() as i32 + 1) * 10,
            };
            list.push(new_rule);
            rules.set(list);
            toast.set(Some("Created new custom Lua transformation rule!".to_string()));
        })
    };

    let on_save_rig_to_vault = {
        let toast = toast_message.clone();
        Callback::from(move |_| {
            toast.set(Some("Saved active rig configuration to vault!".to_string()));
        })
    };

    let on_clear_logs = {
        let execution_logs = execution_logs.clone();
        Callback::from(move |_| {
            execution_logs.set(Vec::new());
        })
    };

    let is_file_bound = file_handle.is_some();

    html! {
        <div class="min-h-screen bg-surface text-on-surface font-sans flex flex-col">
            // Navigation Bar
            <NavigationBar 
                active_tab={*active_tab}
                on_select_tab={on_select_tab}
                bound_filename={(*bound_filename).clone()}
                is_file_bound={is_file_bound}
                active_rig_name={active_rig.name.clone()}
                on_commit_save={on_commit_save.clone()}
                on_toggle_diff={
                    let sdd = show_diff_drawer.clone();
                    Callback::from(move |_| sdd.set(!*sdd))
                }
                on_disconnect_file={on_disconnect_file.clone()}
                on_rebind_file={on_rebind_file.clone()}
                has_unapplied_changes={eval_result.diff_items.iter().any(|d| d.is_modified)}
            />

            // Main Content Body (with padding-top to clear fixed header)
            <main class="w-full pt-32 px-4 lg:px-8 pb-12 flex-1 max-w-7xl mx-auto">
                // Toast notification
                if let Some(msg) = &*toast_message {
                    <div class="mb-4 p-space-md rounded-lg bg-surface-container-high border border-primary text-primary font-code-sm text-code-sm flex items-center justify-between shadow-lg animate-fade-in">
                        <div class="flex items-center gap-space-sm">
                            <span class="material-symbols-outlined text-[16px]">{"info"}</span>
                            <span>{msg}</span>
                        </div>
                        <button 
                            class="text-on-surface-variant hover:text-on-surface"
                            onclick={
                                let tm = toast_message.clone();
                                Callback::from(move |_| tm.set(None))
                            }
                        >
                            <span class="material-symbols-outlined text-[16px]">{"close"}</span>
                        </button>
                    </div>
                }

                // Render Active Tab
                {match *active_tab {
                    ActiveTab::Home => html! {
                        <HomeTab 
                            is_file_bound={is_file_bound}
                            bound_filename={(*bound_filename).clone()}
                            bound_path_hint={(*bound_path_hint).clone()}
                            last_saved_time={(*last_saved_time).clone()}
                            disconnect_after_save={*disconnect_after_save}
                            on_toggle_disconnect_after_save={on_toggle_disconnect_after_save}
                            on_disconnect_file={on_disconnect_file}
                            on_rebind_file={on_rebind_file}
                            on_commit_save={on_commit_save}
                            on_reset_defaults={on_reset_defaults}
                            on_switch_tab={
                                let at = active_tab.clone();
                                Callback::from(move |tab: ActiveTab| at.set(tab))
                            }
                            diff_items={eval_result.diff_items.clone()}
                            rules={(*rules).clone()}
                            on_toggle_rule={on_toggle_rule.clone()}
                            conflicts={(*conflicts).clone()}
                            on_resolve_conflict={on_resolve_conflict}
                            show_diff_drawer={*show_diff_drawer}
                            on_toggle_diff_drawer={
                                let sdd = show_diff_drawer.clone();
                                Callback::from(move |_| sdd.set(!*sdd))
                            }
                            active_rig_name={active_rig.name.clone()}
                            json_diff_text={json_diff_text}
                        />
                    },
                    ActiveTab::Machine => html! {
                        <MachineTab 
                            machines={(*machines).clone()}
                            active_machine_id={(*active_machine_id).clone()}
                            on_set_active_machine={on_set_active_machine}
                            on_add_machine={
                                let m_state = machines.clone();
                                Callback::from(move |new_m: MachineRig| {
                                    let mut list = (*m_state).clone();
                                    list.push(new_m);
                                    m_state.set(list);
                                })
                            }
                            on_change_nozzle={on_change_nozzle}
                            on_save_rig_to_vault={on_save_rig_to_vault}
                        />
                    },
                    ActiveTab::Filament => html! {
                        <FilamentTab 
                            filaments={(*filaments).clone()}
                            active_filament_type={(*active_filament_type).clone()}
                            on_select_active_filament={on_select_active_filament}
                        />
                    },
                    ActiveTab::Wizard => html! {
                        <WizardTab 
                            rules={(*rules).clone()}
                            on_toggle_rule={on_toggle_rule}
                            on_update_rule_lua={on_update_rule_lua}
                            on_add_custom_rule={on_add_custom_rule}
                            on_export_package={on_export_package}
                            on_import_package_trigger={on_import_package_trigger}
                            on_commit_save={on_commit_save}
                            execution_logs={eval_result.execution_logs.clone()}
                            on_clear_logs={on_clear_logs}
                        />
                    },
                }}
            </main>
        </div>
    }
}
