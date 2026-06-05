// short_message_corpus_ingest
//
// Dev-only binary that normalizes user-import corpus files into
// `normalized/` and writes rejected lines to `rejected/`.
//
// Supports `.txt`, `.md`, `.json`, `.jsonl`, and `.csv` input files.
// No network access. All work is local.
//
// USAGE:
//   short_message_corpus_ingest [FLAGS] [--input-dir DIR] [--output-dir DIR] [--file FILE ...]
//
// FLAGS:
//   --input-dir PATH      Read all supported files from PATH (non-recursive).
//   --output-dir PATH     Where to write normalized/ and rejected/. Default:
//                         untracked/commander-v1.4/short-message-corpus-sources
//   --max-len N           Reject any normalized line longer than N chars
//                         (default 160).
//   --include-public      Also read from public-style/ under --output-dir.
//   --h, --help

use serde::Deserialize;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Default, Debug, Clone)]
#[allow(dead_code)]
struct Meta {
    #[serde(default)]
    source_id: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    source_type: Option<String>,
    #[serde(default)]
    license_or_terms_note: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

struct CliArgs {
    input_dir: Option<PathBuf>,
    output_dir: PathBuf,
    files: Vec<PathBuf>,
    max_len: usize,
    include_public: bool,
}

fn parse_args() -> Result<CliArgs, String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut input_dir: Option<PathBuf> = None;
    let mut output_dir = PathBuf::from("untracked/commander-v1.4/short-message-corpus-sources");
    let mut files: Vec<PathBuf> = Vec::new();
    let mut max_len = 160usize;
    let mut include_public = false;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--input-dir" => {
                let v = args.get(i + 1).ok_or("--input-dir requires PATH")?;
                input_dir = Some(PathBuf::from(v));
                i += 2;
            }
            "--output-dir" => {
                let v = args.get(i + 1).ok_or("--output-dir requires PATH")?;
                output_dir = PathBuf::from(v);
                i += 2;
            }
            "--max-len" => {
                let v = args.get(i + 1).ok_or("--max-len requires N")?;
                max_len = v.parse().map_err(|e| format!("--max-len invalid: {}", e))?;
                i += 2;
            }
            "--include-public" => {
                include_public = true;
                i += 1;
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => {
                if other.starts_with("--") {
                    return Err(format!("unknown flag: {}", other));
                }
                files.push(PathBuf::from(other));
                i += 1;
            }
        }
    }
    Ok(CliArgs {
        input_dir,
        output_dir,
        files,
        max_len,
        include_public,
    })
}

fn print_help() {
    println!("short_message_corpus_ingest");
    println!();
    println!("USAGE:");
    println!("    short_message_corpus_ingest [FLAGS] [--input-dir DIR] [--file FILE ...]");
    println!();
    println!("FLAGS:");
    println!("    --input-dir PATH      Read all supported files from PATH (non-recursive).");
    println!("    --output-dir PATH     Where to write normalized/ and rejected/.");
    println!(
        "                         Default: untracked/commander-v1.4/short-message-corpus-sources"
    );
    println!(
        "    --max-len N           Reject any normalized line longer than N chars (default 160)."
    );
    println!("    --include-public      Also read from public-style/ under --output-dir.");
    println!("    -h, --help            Show this help.");
}

#[derive(Default, Debug)]
struct IngestStats {
    files_processed: usize,
    lines_seen: usize,
    lines_kept: usize,
    lines_rejected: usize,
}

fn ensure_dir(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

fn list_supported_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase());
        match ext.as_deref() {
            Some("txt") | Some("md") | Some("json") | Some("jsonl") | Some("csv") => {
                // Skip our own outputs.
                if path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.starts_with("rejected-") || s.starts_with("normalized-"))
                    .unwrap_or(false)
                {
                    continue;
                }
                out.push(path);
            }
            _ => {}
        }
    }
    out.sort();
    Ok(out)
}

fn read_meta(path: &Path) -> Meta {
    let meta_path = path.with_extension("meta.json");
    if !meta_path.exists() {
        return Meta::default();
    }
    let text = match fs::read_to_string(&meta_path) {
        Ok(t) => t,
        Err(_) => return Meta::default(),
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn category_from_name(name: &str) -> &'static str {
    let s = name.to_ascii_lowercase();
    if s.contains("casual")
        || s.contains("chat")
        || s.contains("greeting")
        || s.contains("social")
        || s.contains("everyday")
    {
        "casual_chat"
    } else if s.contains("business")
        || s.contains("commerce")
        || s.contains("invoice")
        || s.contains("order")
        || s.contains("payment")
        || s.contains("sales")
    {
        "business_payment"
    } else if s.contains("operations")
        || s.contains("ops")
        || s.contains("support")
        || s.contains("ticket")
        || s.contains("incident")
        || s.contains("status")
    {
        "operations_support"
    } else if s.contains("asset")
        || s.contains("holder")
        || s.contains("ipfs")
        || s.contains("nft")
        || s.contains("announce")
    {
        "asset_holder"
    } else if s.contains("logistics")
        || s.contains("ship")
        || s.contains("trace")
        || s.contains("warehouse")
        || s.contains("delivery")
        || s.contains("provenance")
        || s.contains("harvest")
        || s.contains("lab")
        || s.contains("coa")
    {
        "logistics_trace"
    } else if s.contains("crypto")
        || s.contains("hemp0x")
        || s.contains("wallet")
        || s.contains("block")
        || s.contains("chain")
        || s.contains("tx")
        || s.contains("mining")
        || s.contains("forum")
    {
        "crypto_hemp0x"
    } else if s.contains("edge") {
        "edge_cases"
    } else {
        "mixed_realistic"
    }
}

fn normalize_line(s: &str) -> String {
    // Strip leading bullet markers, list markers, and surrounding quotes.
    let mut t = s.trim().to_string();
    while t.starts_with('>') || t.starts_with('-') || t.starts_with('*') {
        t = t[1..].trim().to_string();
    }
    // Collapse multiple spaces to one, but preserve emoji and basic punctuation.
    let mut out = String::with_capacity(t.len());
    let mut prev_space = false;
    for ch in t.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

/// Try to extract a candidate short-message line from any input format.
fn extract_lines(path: &Path) -> Vec<String> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());
    match ext.as_deref() {
        Some("json") => extract_json(&text, false),
        Some("jsonl") => extract_json(&text, true),
        Some("csv") => extract_csv(&text),
        _ => extract_text(&text),
    }
}

fn extract_text(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with("//") {
            continue;
        }
        out.push(t.to_string());
    }
    out
}

fn extract_json(text: &str, jsonl: bool) -> Vec<String> {
    let mut out = Vec::new();
    if jsonl {
        for line in text.lines() {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            if let Some(s) = parse_json_object_text(t) {
                out.push(s);
            }
        }
    } else if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        collect_json_strings(&v, &mut out, 0);
    }
    out
}

fn collect_json_strings(v: &serde_json::Value, out: &mut Vec<String>, depth: usize) {
    if depth > 6 {
        return;
    }
    match v {
        serde_json::Value::String(s) => {
            if !s.is_empty() {
                out.push(s.clone());
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                collect_json_strings(item, out, depth + 1);
            }
        }
        serde_json::Value::Object(map) => {
            for (k, vv) in map {
                let kl = k.to_ascii_lowercase();
                if (kl == "text"
                    || kl == "message"
                    || kl == "body"
                    || kl == "content"
                    || kl == "msg")
                    && vv.is_string()
                {
                    if let Some(s) = vv.as_str() {
                        if !s.is_empty() {
                            out.push(s.to_string());
                        }
                    }
                } else {
                    collect_json_strings(vv, out, depth + 1);
                }
            }
        }
        _ => {}
    }
}

fn parse_json_object_text(s: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(s).ok()?;
    let mut out = Vec::new();
    collect_json_strings(&v, &mut out, 0);
    out.into_iter().next()
}

fn extract_csv(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut first = true;
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        // Simple CSV: split on commas, ignore quoted fields for now.
        let mut parts = t.split(',');
        let cell0 = parts.next().unwrap_or("").trim();
        if first {
            first = false;
            // If first line looks like a header, skip.
            if cell0.to_ascii_lowercase().contains("text")
                || cell0.to_ascii_lowercase().contains("message")
                || cell0.to_ascii_lowercase().contains("body")
            {
                continue;
            }
        }
        if !cell0.is_empty() {
            out.push(cell0.to_string());
        }
    }
    out
}

fn ingest_file(
    path: &Path,
    default_category: &str,
    max_len: usize,
    seen: &mut BTreeSet<String>,
    out_normalized: &mut Vec<(String, String)>,
    out_rejected: &mut Vec<(String, String)>,
    stats: &mut IngestStats,
) {
    stats.files_processed += 1;
    let meta = read_meta(path);
    let effective_category = meta.category.as_deref().unwrap_or(default_category);
    let raw_lines = extract_lines(path);
    for raw in raw_lines {
        stats.lines_seen += 1;
        let norm = normalize_line(&raw);
        if norm.is_empty() {
            out_rejected.push((
                norm.clone(),
                format!("empty after normalize; raw={:?}", raw),
            ));
            stats.lines_rejected += 1;
            continue;
        }
        if norm.chars().count() > max_len {
            out_rejected.push((
                norm.clone(),
                format!("len={} > max_len={}", norm.chars().count(), max_len),
            ));
            stats.lines_rejected += 1;
            continue;
        }
        if !seen.insert(norm.clone()) {
            // Within-file dedup; cross-file dedup is applied at the end.
            continue;
        }
        out_normalized.push((effective_category.to_string(), norm));
        stats.lines_kept += 1;
    }
    let _ = meta;
}

fn main() {
    let cli = match parse_args() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("error: {}", err);
            std::process::exit(2);
        }
    };

    let normalized_dir = cli.output_dir.join("normalized");
    let rejected_dir = cli.output_dir.join("rejected");
    if let Err(err) = ensure_dir(&normalized_dir) {
        eprintln!("error creating normalized dir: {}", err);
        std::process::exit(2);
    }
    if let Err(err) = ensure_dir(&rejected_dir) {
        eprintln!("error creating rejected dir: {}", err);
        std::process::exit(2);
    }

    let mut all_files: Vec<PathBuf> = Vec::new();
    if let Some(dir) = &cli.input_dir {
        match list_supported_files(dir) {
            Ok(files) => all_files.extend(files),
            Err(err) => eprintln!("warning: cannot read input dir {}: {}", dir.display(), err),
        }
    }
    for f in &cli.files {
        if f.is_file() {
            all_files.push(f.clone());
        } else {
            eprintln!("warning: not a file: {}", f.display());
        }
    }
    if cli.include_public {
        let pub_dir = cli.output_dir.join("public-style");
        match list_supported_files(&pub_dir) {
            Ok(files) => all_files.extend(files),
            Err(_) => {
                // public-style/ might not exist yet; that's fine.
            }
        }
    }

    if all_files.is_empty() {
        eprintln!("no input files to process");
        std::process::exit(2);
    }

    let mut stats = IngestStats::default();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut normalized: Vec<(String, String)> = Vec::new();
    let mut rejected: Vec<(String, String)> = Vec::new();

    for path in &all_files {
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_string();
        let default_category = category_from_name(&name);
        ingest_file(
            path,
            default_category,
            cli.max_len,
            &mut seen,
            &mut normalized,
            &mut rejected,
            &mut stats,
        );
    }

    // Cross-file dedup pass.
    let mut final_seen: BTreeSet<String> = BTreeSet::new();
    let mut final_normalized: Vec<(String, String)> = Vec::with_capacity(normalized.len());
    let mut cross_dup = 0usize;
    for (cat, line) in normalized {
        if final_seen.insert(line.clone()) {
            final_normalized.push((cat, line));
        } else {
            cross_dup += 1;
        }
    }

    // Write one normalized/<category>.txt per category, plus an "all.txt".
    let mut by_category: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for (cat, line) in &final_normalized {
        by_category
            .entry(cat.clone())
            .or_default()
            .push(line.clone());
    }

    for (cat, lines) in &by_category {
        let safe = cat.replace(|c: char| !c.is_alphanumeric() && c != '_', "_");
        let path = normalized_dir.join(format!("{}.txt", safe));
        let mut f = match fs::File::create(&path) {
            Ok(f) => f,
            Err(err) => {
                eprintln!("error creating {}: {}", path.display(), err);
                continue;
            }
        };
        let _ = writeln!(f, "# category: {} (source-backed normalized samples)", cat);
        let _ = writeln!(f, "# generated by short_message_corpus_ingest");
        let _ = writeln!(f, "# max_len: {}", cli.max_len);
        let _ = writeln!(f);
        for line in lines {
            let _ = writeln!(f, "{}", line);
        }
    }

    let all_path = normalized_dir.join("all.txt");
    let mut f = match fs::File::create(&all_path) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("error creating {}: {}", all_path.display(), err);
            std::process::exit(2);
        }
    };
    let _ = writeln!(
        f,
        "# category\\tmessage (one normalized line per source sample)"
    );
    let _ = writeln!(f, "# generated by short_message_corpus_ingest");
    for (cat, line) in &final_normalized {
        let _ = writeln!(f, "{}\t{}", cat, line);
    }

    let rej_path = rejected_dir.join("rejected.txt");
    let mut f = match fs::File::create(&rej_path) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("error creating {}: {}", rej_path.display(), err);
            std::process::exit(2);
        }
    };
    let _ = writeln!(f, "# reason\\tmessage");
    for (line, reason) in &rejected {
        let _ = writeln!(f, "{}\t{}", reason, line);
    }

    let stats_path = cli.output_dir.join("ingest-report.txt");
    let mut f = match fs::File::create(&stats_path) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("error creating {}: {}", stats_path.display(), err);
            std::process::exit(2);
        }
    };
    let _ = writeln!(f, "Ingest report");
    let _ = writeln!(f, "=============");
    let _ = writeln!(f, "files processed: {}", stats.files_processed);
    let _ = writeln!(f, "lines seen:      {}", stats.lines_seen);
    let _ = writeln!(
        f,
        "lines kept:      {} (after cross-file dedup: {})",
        stats.lines_kept,
        final_normalized.len()
    );
    let _ = writeln!(f, "lines rejected:  {}", stats.lines_rejected);
    let _ = writeln!(f, "cross-file dups: {}", cross_dup);
    let _ = writeln!(f, "by category:");
    for (cat, lines) in &by_category {
        let _ = writeln!(f, "  - {}: {}", cat, lines.len());
    }
    let _ = writeln!(f);
    let _ = writeln!(f, "Inputs:");
    for path in &all_files {
        let _ = writeln!(f, "  - {}", path.display());
    }

    println!(
        "ingest: {} files, {} lines seen, {} kept, {} rejected, {} cross-file dups",
        stats.files_processed,
        stats.lines_seen,
        final_normalized.len(),
        stats.lines_rejected,
        cross_dup
    );
    println!("normalized dir: {}", normalized_dir.display());
    println!("rejected dir:   {}", rejected_dir.display());
    println!("ingest report:  {}", stats_path.display());
}
