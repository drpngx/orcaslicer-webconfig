use pulldown_cmark::{html, CowStr, Event, Options, Parser, Tag};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CompiledDocPage {
  pub slug: String,
  pub filename: String,
  pub title: String,
  pub summary: String,
  pub html: String,
}

fn extract_title_and_summary(md: &str) -> (String, String) {
  let mut title = String::from("Documentation");
  let mut summary = String::new();
  for line in md.lines() {
    let trimmed = line.trim();
    if let Some(h1) = trimmed.strip_prefix("# ") {
      if title == "Documentation" {
        title = h1.trim().to_string();
      }
    } else if !trimmed.is_empty()
      && !trimmed.starts_with('#')
      && !trimmed.starts_with('!')
      && !trimmed.starts_with('|')
      && !trimmed.starts_with('-')
      && summary.is_empty()
    {
      summary = trimmed.replace("**", "").replace('`', "");
    }
  }
  (title, summary)
}

fn compile_markdown_to_html(md: &str) -> String {
  let mut options = Options::empty();
  options.insert(Options::ENABLE_TABLES);
  options.insert(Options::ENABLE_STRIKETHROUGH);
  options.insert(Options::ENABLE_TASKLISTS);

  let parser = Parser::new_ext(md, options).map(|event| match event {
    Event::Start(Tag::Image {
      link_type,
      dest_url,
      title,
      id,
    }) => {
      // Rewrite relative `images/foo.png` in `doc/*.md` to `/doc/images/foo.png` when rendered inside the web app,
      // while keeping `images/foo.png` in the source `.md` so GitHub's Markdown viewer resolves it natively!
      let rewritten = if dest_url.starts_with("images/") {
        CowStr::from(format!("/doc/{}", dest_url))
      } else {
        dest_url
      };
      Event::Start(Tag::Image {
        link_type,
        dest_url: rewritten,
        title,
        id,
      })
    }
    other => other,
  });

  let mut html_output = String::new();
  html::push_html(&mut html_output, parser);
  html_output
}

fn main() {
  let args: Vec<String> = env::args().collect();
  if args.len() < 3 {
    eprintln!("Usage: compile_docs <output.json> <doc_file1.md> [doc_file2.md ...]");
    std::process::exit(1);
  }
  let output_path = &args[1];
  let input_files = &args[2..];

  // Order pages canonically
  let desired_order = [
    "index.md",
    "getting-started.md",
    "hardware-and-weather.md",
    "filaments-and-brands.md",
    "lua-recipes-and-wizard.md",
  ];

  let mut pages: Vec<CompiledDocPage> = Vec::new();
  for file_path in input_files {
    let path = Path::new(file_path);
    let Some(fname) = path.file_name().and_then(|n| n.to_str()) else {
      continue;
    };
    if !fname.ends_with(".md") {
      continue;
    }
    let md = fs::read_to_string(path).unwrap_or_default();
    let (title, summary) = extract_title_and_summary(&md);
    let html = compile_markdown_to_html(&md);
    let slug = fname.trim_end_matches(".md").to_string();
    pages.push(CompiledDocPage {
      slug,
      filename: fname.to_string(),
      title,
      summary,
      html,
    });
  }

  pages.sort_by_key(|p| {
    desired_order
      .iter()
      .position(|&name| name == p.filename)
      .unwrap_or(999)
  });

  let json = serde_json::to_string_pretty(&pages).expect("Failed to serialize compiled docs");
  fs::write(output_path, json).expect("Failed to write compiled_docs.json");
}
