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
    pub bound_filename: String,
    pub is_file_bound: bool,
    pub active_rig_name: String,
    pub on_commit_save: Callback<()>,
    pub on_toggle_diff: Callback<()>,
    pub on_disconnect_file: Callback<()>,
    pub on_rebind_file: Callback<()>,
    pub has_unapplied_changes: bool,
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

    html! {
        <header class="fixed top-0 left-0 right-0 z-50 bg-surface-container-low shadow-[0_1px_8px_rgba(0,0,0,0.45)]">
            <div class="h-28 w-full px-margin-desktop flex flex-col justify-between py-space-xs">
                // Top row
                <div class="flex items-center justify-between gap-space-lg w-full h-14">
                    <div class="flex items-center gap-space-lg">
                        <div class="flex items-center gap-space-sm cursor-pointer" onclick={
                            let cb = on_select_tab.clone();
                            Callback::from(move |_| cb.emit(ActiveTab::Home))
                        }>
                            <span class="text-2xl">{"🐋"}</span>
                            <span class="font-headline-sm text-headline-sm text-on-surface tracking-tight font-bold">{"Orca Studio"}</span>
                            <span class="font-code-sm text-code-sm text-primary bg-surface-container px-space-sm py-space-xs rounded">{"v2.1"}</span>
                        </div>
                        <div class="h-5 w-px bg-surface-bright"></div>

                        // File bound indicator
                        if props.is_file_bound {
                            <div class="flex items-center gap-space-xs bg-surface-container-lowest px-space-md py-space-xs rounded border border-outline-variant">
                                <div class="w-2 h-2 rounded-full bg-primary animate-pulse"></div>
                                <div class="flex flex-col text-left">
                                    <span class="font-label-caps text-label-caps text-primary uppercase font-semibold">{"Direct File Bound"}</span>
                                    <span class="font-code-sm text-code-sm text-on-surface">{&props.bound_filename}</span>
                                </div>
                                <button 
                                    class="ml-space-sm text-on-surface-variant hover:text-error text-xs p-1 rounded hover:bg-surface-container transition-colors"
                                    title="Disconnect file to release file lock"
                                    onclick={
                                        let cb = props.on_disconnect_file.clone();
                                        Callback::from(move |_| cb.emit(()))
                                    }
                                >
                                    <span class="material-symbols-outlined text-[14px]">{"link_off"}</span>
                                </button>
                            </div>
                        } else {
                            <button 
                                class="flex items-center gap-space-sm bg-surface-container-lowest px-space-md py-space-xs rounded border border-outline-variant hover:border-primary-container transition-colors text-left"
                                onclick={
                                    let cb = props.on_rebind_file.clone();
                                    Callback::from(move |_| cb.emit(()))
                                }
                            >
                                <div class="w-2 h-2 rounded-full bg-outline-variant"></div>
                                <div class="flex flex-col">
                                    <span class="font-label-caps text-label-caps text-on-surface-variant uppercase font-semibold">{"Not Bound (Using Preset)"}</span>
                                    <span class="font-code-sm text-code-sm text-primary hover:underline">{"Click to Connect File..."}</span>
                                </div>
                                <span class="material-symbols-outlined text-[14px] text-on-surface-variant ml-space-xs">{"folder_open"}</span>
                            </button>
                        }

                        // Active rig tag
                        <div 
                            class="flex items-center gap-space-sm bg-surface-container-highest px-space-md py-space-xs rounded cursor-pointer hover:bg-surface-bright transition-colors"
                            onclick={
                                let cb = on_select_tab.clone();
                                Callback::from(move |_| cb.emit(ActiveTab::Machine))
                            }
                        >
                            <span class="material-symbols-outlined text-primary text-[18px]">{"tune"}</span>
                            <span class="font-code-md text-code-md text-on-surface font-medium">{&props.active_rig_name}</span>
                            <span class="font-label-caps text-label-caps uppercase text-primary-fixed-dim bg-on-primary-fixed px-space-xs py-space-xs rounded">{"Active Rig"}</span>
                        </div>

                        // ACE Pro status
                        <div 
                            class="flex items-center gap-space-xs px-space-md py-space-xs rounded bg-surface-container-lowest text-tertiary cursor-pointer hover:bg-surface-container transition-colors"
                            onclick={
                                let cb = on_select_tab.clone();
                                Callback::from(move |_| cb.emit(ActiveTab::Filament))
                            }
                        >
                            <span class="w-1.5 h-1.5 rounded-full bg-tertiary"></span>
                            <span class="font-code-sm text-code-sm">{"ACE Pro: 4/4 Ready"}</span>
                        </div>
                    </div>

                    // Right header actions
                    <div class="flex items-center gap-space-md">
                        // Live diff drawer toggle
                        <button 
                            class="flex items-center gap-space-xs bg-surface-container-high text-on-surface px-space-md py-space-xs rounded hover:bg-surface-bright transition-colors"
                            onclick={
                                let cb = props.on_toggle_diff.clone();
                                Callback::from(move |_| cb.emit(()))
                            }
                        >
                            <span class="material-symbols-outlined text-secondary text-[16px]">{"difference"}</span>
                            <span class="font-code-md text-code-md font-medium">{"Live Diff"}</span>
                        </button>

                        // Commit to Local Orca button
                        <button 
                            class={if props.has_unapplied_changes {
                                "flex items-center gap-space-xs bg-primary-container text-on-primary-container px-space-md py-space-xs rounded font-medium hover:bg-primary transition-colors shadow-lg animate-pulse"
                            } else {
                                "flex items-center gap-space-xs bg-surface-container-highest text-on-surface px-space-md py-space-xs rounded font-medium hover:bg-surface-bright transition-colors"
                            }}
                            onclick={
                                let cb = props.on_commit_save.clone();
                                Callback::from(move |_| cb.emit(()))
                            }
                        >
                            <span class="material-symbols-outlined text-[16px]">{"save"}</span>
                            <span class="font-code-md text-code-md font-medium">{"Commit to Local Orca"}</span>
                        </button>
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
                            {"Overview & Config Sync"}
                        </button>
                        <button 
                            class={tab_class(ActiveTab::Machine)}
                            onclick={
                                let cb = on_select_tab.clone();
                                Callback::from(move |_| cb.emit(ActiveTab::Machine))
                            }
                        >
                            {"Printer & Rig Vault"}
                        </button>
                        <button 
                            class={tab_class(ActiveTab::Filament)}
                            onclick={
                                let cb = on_select_tab.clone();
                                Callback::from(move |_| cb.emit(ActiveTab::Filament))
                            }
                        >
                            {"Filaments & ACE Pro Bank"}
                        </button>
                        <button 
                            class={tab_class(ActiveTab::Wizard)}
                            onclick={
                                let cb = on_select_tab.clone();
                                Callback::from(move |_| cb.emit(ActiveTab::Wizard))
                            }
                        >
                            {"Wizard / Config Changer"}
                        </button>
                    </nav>

                    <div class="flex items-center gap-space-md">
                        <div class="flex items-center gap-space-xs text-on-surface-variant font-code-sm text-code-sm">
                            <span>{"Active Profile:"}</span>
                            <span class="font-code-sm text-code-sm text-secondary bg-surface-container-lowest px-space-sm py-space-xs rounded">
                                {&props.bound_filename}
                            </span>
                        </div>
                    </div>
                </div>
            </div>
        </header>
    }
}
