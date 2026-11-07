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

fn html_escape_math(s: &str) -> String {
  s.replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
}

fn protect_math_segments(md: &str) -> (String, Vec<(String, String, bool)>) {
  let bytes = md.as_bytes();
  let len = bytes.len();
  let mut out = String::with_capacity(len);
  let mut replacements: Vec<(String, String, bool)> = Vec::new();
  let mut i = 0;
  let mut in_fence = false;

  while i < len {
    // Check for fenced code block at start of line
    if (i == 0 || bytes[i - 1] == b'\n') && i + 3 <= len && &bytes[i..i + 3] == b"```" {
      in_fence = !in_fence;
      out.push_str("```");
      i += 3;
      continue;
    }

    if in_fence {
      let ch = md[i..].chars().next().unwrap();
      out.push(ch);
      i += ch.len_utf8();
      continue;
    }

    // Inline code span `...`
    if bytes[i] == b'`' {
      let mut tick_count = 0;
      while i + tick_count < len && bytes[i + tick_count] == b'`' {
        tick_count += 1;
      }
      let opener = &md[i..i + tick_count];
      let opener_bytes = opener.as_bytes();
      out.push_str(opener);
      i += tick_count;
      while i < len {
        if i + tick_count <= len && &bytes[i..i + tick_count] == opener_bytes {
          out.push_str(opener);
          i += tick_count;
          break;
        }
        let ch = md[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
      }
      continue;
    }

    // Display math $$ ... $$
    if i + 2 <= len && &bytes[i..i + 2] == b"$$" {
      if let Some(rel_end) = md[i + 2..].find("$$") {
        let math_body = &md[i + 2..i + 2 + rel_end];
        let token = format!("MATHDISPLAYTOKEN{}END", replacements.len());
        replacements.push((token.clone(), math_body.to_string(), true));
        out.push_str(&token);
        i += 2 + rel_end + 2;
        continue;
      }
    }

    // Inline math $ ... $ (single line)
    if bytes[i] == b'$' && (i == 0 || bytes[i - 1] != b'\\') {
      let mut j = i + 1;
      let mut found_end = None;
      while j < len && bytes[j] != b'\n' {
        if bytes[j] == b'$' && bytes[j - 1] != b'\\' {
          found_end = Some(j);
          break;
        }
        let ch = md[j..].chars().next().unwrap();
        j += ch.len_utf8();
      }
      if let Some(end_idx) = found_end {
        if end_idx > i + 1 {
          let math_body = &md[i + 1..end_idx];
          let token = format!("MATHINLINETOKEN{}END", replacements.len());
          replacements.push((token.clone(), math_body.to_string(), false));
          out.push_str(&token);
          i = end_idx + 1;
          continue;
        }
      }
    }

    let ch = md[i..].chars().next().unwrap();
    out.push(ch);
    i += ch.len_utf8();
  }

  (out, replacements)
}

fn compile_markdown_to_html(md: &str) -> String {
  let (protected_md, math_segments) = protect_math_segments(md);
  let mut options = Options::empty();
  options.insert(Options::ENABLE_TABLES);
  options.insert(Options::ENABLE_STRIKETHROUGH);
  options.insert(Options::ENABLE_TASKLISTS);

  let parser = Parser::new_ext(&protected_md, options).map(|event| match event {
    Event::Start(Tag::Image {
      link_type,
      dest_url,
      title,
      id,
    }) => {
      // Rewrite relative `images/foo.png` in `doc/*.md` to `doc/images/foo.png` when rendered inside the web app,
      // while keeping `images/foo.png` in the source `.md` so GitHub's Markdown viewer resolves it natively!
      let rewritten = if dest_url.starts_with("images/") {
        CowStr::from(format!("doc/{}", dest_url))
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
  for (token, math_body, is_display) in math_segments {
    let escaped = html_escape_math(&math_body);
    let rendered = if is_display {
      format!("<code class=\"math-display\">$${}$$</code>", escaped)
    } else {
      format!("<code class=\"math-inline\">${}$</code>", escaped)
    };
    html_output = html_output.replace(&token, &rendered);
  }
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
    "pwl-nozzle-compensation-math.md",
    "coupled-parameter-physics-math.md",
    "dev.md",
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
