use serde::{Deserialize, Serialize};
use yew::prelude::*;

const COMPILED_DOCS_JSON: &str = include_str!(env!("COMPILED_DOCS_JSON"));

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CompiledDocPage {
  pub slug: String,
  pub filename: String,
  pub title: String,
  pub summary: String,
  pub html: String,
}

pub fn load_compiled_docs() -> Vec<CompiledDocPage> {
  serde_json::from_str::<Vec<CompiledDocPage>>(COMPILED_DOCS_JSON).unwrap_or_default()
}

#[derive(Properties, PartialEq)]
pub struct DocsViewProps {
  pub on_close_docs: Callback<()>,
}

#[function_component]
pub fn DocsView(props: &DocsViewProps) -> Html {
  let pages = use_memo((), |_| load_compiled_docs());
  let active_slug = use_state(|| {
    pages
      .first()
      .map(|p| p.slug.clone())
      .unwrap_or_else(|| "index".to_string())
  });

  let active_page = pages
    .iter()
    .find(|p| p.slug == *active_slug)
    .or_else(|| pages.first())
    .cloned();

  let active_idx = pages
    .iter()
    .position(|p| p.slug == *active_slug)
    .unwrap_or(0);

  html! {
    <div class="flex flex-col w-full gap-space-lg">
      // Top Banner with prominent "Back to Configurator" button
      <div class="bg-surface-container-low border-2 border-primary/60 p-space-md rounded-xl shadow-md flex flex-wrap items-center justify-between gap-space-md">
        <div class="flex items-center gap-space-md">
          <button
            class="flex items-center gap-space-xs bg-primary hover:opacity-90 text-on-primary px-space-lg py-2 rounded-lg text-xs font-mono font-bold shadow transition-opacity cursor-pointer"
            onclick={
              let cb = props.on_close_docs.clone();
              Callback::from(move |_| cb.emit(()))
            }
          >
            <span class="material-symbols-outlined text-[16px]">{"arrow_back"}</span>
            {"Back to Configurator"}
          </button>
          <div class="flex flex-col">
            <div class="flex items-center gap-2">
              <span class="bg-primary/20 text-primary text-[10px] font-mono uppercase px-2 py-0.5 rounded font-bold">
                {"Documentation & User Guide"}
              </span>
              <span class="text-xs font-mono text-on-surface-variant">
                {"Precompiled via Bazel from doc/*.md"}
              </span>
            </div>
            <h1 class="font-headline-sm text-headline-sm text-on-surface font-bold mt-0.5">
              {"OrcaSlicer WebConfig — Transformation Architecture & Tuning Recipes"}
            </h1>
          </div>
        </div>

        <button
          class="flex items-center gap-space-xs bg-surface-container-highest hover:bg-surface-bright text-on-surface border border-outline-variant px-space-md py-1.5 rounded-lg text-xs font-mono font-bold transition-colors cursor-pointer"
          onclick={
            let cb = props.on_close_docs.clone();
            Callback::from(move |_| cb.emit(()))
          }
        >
          <span class="material-symbols-outlined text-[16px]">{"close"}</span>
          {"Return to App"}
        </button>
      </div>

      <div class="grid grid-cols-1 lg:grid-cols-12 gap-space-lg items-start">
        // Left Sidebar: Chapter Navigation
        <aside class="lg:col-span-3 bg-surface-container-low p-space-md rounded-xl border border-outline-variant flex flex-col gap-space-xs sticky top-32">
          <span class="text-[11px] font-mono uppercase text-primary font-bold px-2 pb-1 border-b border-surface-container">
            {"Chapters (doc/)"}
          </span>
          {for pages.iter().enumerate().map(|(idx, page)| {
            let is_active = page.slug == *active_slug;
            let slug = page.slug.clone();
            let sel = active_slug.clone();
            html! {
              <button
                class={format!(
                  "text-left px-space-md py-2.5 rounded-lg transition-colors flex flex-col gap-0.5 cursor-pointer {}",
                  if is_active {
                    "bg-primary text-on-primary font-bold shadow"
                  } else {
                    "hover:bg-surface-container text-on-surface-variant hover:text-on-surface"
                  }
                )}
                onclick={Callback::from(move |_| sel.set(slug.clone()))}
              >
                <span class="text-xs font-mono">
                  {format!("{}. {}", idx + 1, page.title)}
                </span>
                <span class={format!(
                  "text-[10px] font-mono truncate {}",
                  if is_active { "opacity-90" } else { "text-on-surface-variant/70" }
                )}>
                  {format!("doc/{}", page.filename)}
                </span>
              </button>
            }
          })}
        </aside>

        // Right Main Column: Precompiled Markdown HTML + Prev/Next & Back to Configurator Footer
        <article class="lg:col-span-9 bg-surface-container-low p-space-xl rounded-xl border border-outline-variant shadow-md flex flex-col gap-space-lg">
          if let Some(page) = active_page {
            <>
              <div class="flex items-center justify-between border-b border-surface-container pb-space-sm">
                <span class="text-xs font-mono text-secondary">
                  {format!("Source: doc/{}", page.filename)}
                </span>
                <span class="text-xs font-mono text-on-surface-variant">
                  {format!("Chapter {} of {}", active_idx + 1, pages.len())}
                </span>
              </div>

              <div class="doc-markdown-content text-on-surface space-y-4 leading-relaxed">
                {Html::from_html_unchecked(AttrValue::from(page.html.clone()))}
              </div>

              <div class="flex flex-wrap items-center justify-between gap-space-sm pt-space-md border-t border-surface-container">
                <div class="flex items-center gap-space-sm">
                  if active_idx > 0 {
                    <button
                      class="flex items-center gap-1 bg-surface-container-highest hover:bg-surface-bright text-on-surface px-space-md py-2 rounded-lg text-xs font-mono font-bold transition-colors cursor-pointer"
                      onclick={
                        let prev_slug = pages[active_idx - 1].slug.clone();
                        let sel = active_slug.clone();
                        Callback::from(move |_| sel.set(prev_slug.clone()))
                      }
                    >
                      <span class="material-symbols-outlined text-[16px]">{"chevron_left"}</span>
                      {format!("Prev: {}", pages[active_idx - 1].title)}
                    </button>
                  }
                  if active_idx + 1 < pages.len() {
                    <button
                      class="flex items-center gap-1 bg-primary-container hover:bg-primary text-on-primary-container px-space-md py-2 rounded-lg text-xs font-mono font-bold transition-colors cursor-pointer"
                      onclick={
                        let next_slug = pages[active_idx + 1].slug.clone();
                        let sel = active_slug.clone();
                        Callback::from(move |_| sel.set(next_slug.clone()))
                      }
                    >
                      {format!("Next: {}", pages[active_idx + 1].title)}
                      <span class="material-symbols-outlined text-[16px]">{"chevron_right"}</span>
                    </button>
                  }
                </div>

                <button
                  class="flex items-center gap-space-xs bg-primary hover:opacity-90 text-on-primary px-space-lg py-2 rounded-lg text-xs font-mono font-bold shadow transition-opacity cursor-pointer"
                  onclick={
                    let cb = props.on_close_docs.clone();
                    Callback::from(move |_| cb.emit(()))
                  }
                >
                  <span class="material-symbols-outlined text-[16px]">{"arrow_back"}</span>
                  {"Back to Configurator"}
                </button>
              </div>
            </>
          }
        </article>
      </div>
    </div>
  }
}
