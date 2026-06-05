// short_message_split
//
// Dev-only helper that reads user-import corpus files and produces
// `*-split.txt` files containing the natural-language splits of any
// line that exceeds a soft cap. Each split group is preceded by a
// `// split of: <original line>` comment so the operator can trace
// fragments back to the source.
//
// The splitter is intentionally simple and rule-based. It does not
// perform NLP. The intent is to give the audit enough short,
// natural lines that the dictionary coverage question can be
// answered for real-world text shapes.
//
// USAGE:
//   short_message_split [FLAGS] [--input-dir DIR] [--file FILE ...]
//
// FLAGS:
//   --input-dir PATH     Read every supported file in PATH.
//   --output-dir PATH    Where to write <basename>-split.txt files.
//                         Default: same as --input-dir.
//   --soft-cap N         Lines longer than N characters are split.
//                         Default: 80.
//   --max-frag-len N     Cap each fragment at N characters.
//                         Default: 60.
//   --h, --help

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

struct CliArgs {
    input_dir: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    files: Vec<PathBuf>,
    soft_cap: usize,
    max_frag_len: usize,
}

fn parse_args() -> Result<CliArgs, String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut input_dir: Option<PathBuf> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut files: Vec<PathBuf> = Vec::new();
    let mut soft_cap = 80usize;
    let mut max_frag_len = 60usize;
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
                output_dir = Some(PathBuf::from(v));
                i += 2;
            }
            "--soft-cap" => {
                let v = args.get(i + 1).ok_or("--soft-cap requires N")?;
                soft_cap = v
                    .parse()
                    .map_err(|e| format!("--soft-cap invalid: {}", e))?;
                i += 2;
            }
            "--max-frag-len" => {
                let v = args.get(i + 1).ok_or("--max-frag-len requires N")?;
                max_frag_len = v
                    .parse()
                    .map_err(|e| format!("--max-frag-len invalid: {}", e))?;
                i += 2;
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
        soft_cap,
        max_frag_len,
    })
}

fn print_help() {
    println!("short_message_split");
    println!();
    println!("USAGE:");
    println!("    short_message_split [FLAGS] [--input-dir DIR] [--file FILE ...]");
    println!();
    println!("FLAGS:");
    println!("    --input-dir PATH     Read every supported file in PATH.");
    println!("    --output-dir PATH    Where to write <basename>-split.txt files.");
    println!("                         Default: same as --input-dir.");
    println!("    --soft-cap N         Lines longer than N are split. Default: 80.");
    println!("    --max-frag-len N     Cap each fragment at N. Default: 60.");
    println!("    -h, --help           Show this help.");
}

fn list_supported_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.ends_with("-split.txt") || s.ends_with(".meta.json"))
            .unwrap_or(false)
        {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase());
        match ext.as_deref() {
            Some("txt") | Some("md") => out.push(path),
            _ => {}
        }
    }
    out.sort();
    Ok(out)
}

/// Split a long line at natural boundaries. Returns a Vec of fragments
/// (each at most `max_frag_len` characters, longer than 4 chars).
fn split_line(line: &str, max_frag_len: usize) -> Vec<String> {
    if line.chars().count() <= max_frag_len {
        return vec![line.to_string()];
    }

    // We split on a hierarchy of delimiters. Each delimiter is tried in
    // order; if a piece is still too long we recurse.
    fn recursive_split(text: &str, max_len: usize) -> Vec<String> {
        if text.chars().count() <= max_len {
            if text.trim().is_empty() {
                return Vec::new();
            }
            return vec![text.trim().to_string()];
        }

        // Try a list of delimiter patterns. Pick the split point that
        // produces the most balanced pieces and is closest to the
        // middle of the input.
        const DELIMS: &[&str] = &[
            ". ",
            "! ",
            "? ",
            "; ",
            ", ",
            " and ",
            " or ",
            " but ",
            " so ",
            " because ",
            " then ",
            " when ",
            " with ",
            " from ",
            " for ",
            " - ",
            " — ",
            " : ",
        ];
        let best = DELIMS
            .iter()
            .filter_map(|d| text.find(d).map(|pos| (pos, pos + d.len(), d)))
            .min_by_key(|(pos, end, _)| {
                let mid = text.chars().count() / 2;
                let split_char = pos + (end - pos) / 2;
                (mid as isize - split_char as isize).abs()
            });
        if let Some((pos, end, _)) = best {
            let left = &text[..pos];
            let right = &text[end..];
            let mut out = recursive_split(left, max_len);
            out.extend(recursive_split(right, max_len));
            return out;
        }
        // No explicit delimiter found. Prefer splitting at the last
        // single space at or before the max_len cut point so we
        // never break a word.
        let last_space = text
            .char_indices()
            .take(max_len)
            .filter(|(_, c)| *c == ' ')
            .last()
            .map(|(i, _)| i);
        let cut = if let Some(idx) = last_space {
            if idx >= 8 {
                idx
            } else {
                // No reasonable space. Hard cut.
                text.char_indices()
                    .nth(max_len.saturating_sub(1))
                    .map(|(i, _)| i)
                    .unwrap_or(text.len())
            }
        } else {
            // Single very long word. Hard cut.
            text.char_indices()
                .nth(max_len.saturating_sub(1))
                .map(|(i, _)| i)
                .unwrap_or(text.len())
        };
        let left = &text[..cut];
        let right = &text[cut..].trim_start();
        let mut out = recursive_split(left, max_len);
        out.extend(recursive_split(right, max_len));
        out
    }

    recursive_split(line, max_frag_len)
}

fn process_file(
    path: &Path,
    output_dir: &Path,
    soft_cap: usize,
    max_frag_len: usize,
) -> std::io::Result<(usize, usize)> {
    let text = fs::read_to_string(path)?;
    let mut split_out: Vec<(String, Vec<String>)> = Vec::new();
    let mut non_split: Vec<String> = Vec::new();
    let mut total = 0usize;
    let mut split_count = 0usize;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            // Pass through comments and blanks; they don't count as
            // samples.
            continue;
        }
        total += 1;
        if trimmed.chars().count() > soft_cap {
            let frags = split_line(trimmed, max_frag_len);
            if frags.len() > 1 {
                split_out.push((trimmed.to_string(), frags));
                split_count += 1;
            } else {
                non_split.push(trimmed.to_string());
            }
        } else {
            non_split.push(trimmed.to_string());
        }
    }

    if split_out.is_empty() {
        return Ok((total, split_count));
    }

    let basename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    let split_path = output_dir.join(format!("{}-split.txt", basename.trim_end_matches(".txt")));

    let mut f = fs::File::create(&split_path)?;
    let _ = writeln!(
        f,
        "# Auto-split fragments of long lines from {} (soft_cap={}, max_frag_len={})",
        basename, soft_cap, max_frag_len
    );
    let _ = writeln!(f, "# Generated by short_message_split.");
    let _ = writeln!(
        f,
        "# Each split group is one original line that exceeded soft_cap."
    );
    let _ = writeln!(
        f,
        "# Comments use `// split of:` so the operator can trace them back."
    );
    let _ = writeln!(f);
    for (original, frags) in &split_out {
        let _ = writeln!(f, "// split of: {}", original);
        for frag in frags {
            let _ = writeln!(f, "{}", frag);
        }
        let _ = writeln!(f);
    }
    // Also pass through the short lines for completeness. The audit
    // will dedup against the main file.
    if !non_split.is_empty() {
        let _ = writeln!(f, "# --- short lines from same source (pass-through) ---");
        for s in &non_split {
            let _ = writeln!(f, "{}", s);
        }
    }
    Ok((total, split_count))
}

fn main() {
    let cli = match parse_args() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("error: {}", err);
            std::process::exit(2);
        }
    };

    let mut all_files: Vec<PathBuf> = Vec::new();
    if let Some(dir) = &cli.input_dir {
        match list_supported_files(dir) {
            Ok(f) => all_files.extend(f),
            Err(err) => {
                eprintln!("error reading input dir {}: {}", dir.display(), err);
                std::process::exit(2);
            }
        }
    }
    for f in &cli.files {
        all_files.push(f.clone());
    }
    if all_files.is_empty() {
        eprintln!("no input files");
        std::process::exit(2);
    }

    let output_dir = cli
        .output_dir
        .clone()
        .or_else(|| cli.input_dir.clone())
        .expect("input dir or output dir required");
    fs::create_dir_all(&output_dir).expect("create output dir");

    let mut total_lines = 0usize;
    let mut total_split = 0usize;
    for path in &all_files {
        match process_file(&path, &output_dir, cli.soft_cap, cli.max_frag_len) {
            Ok((n, s)) => {
                total_lines += n;
                total_split += s;
            }
            Err(err) => eprintln!("error processing {}: {}", path.display(), err),
        }
    }
    println!(
        "split: processed {} files, {} long lines split, {} total short+long lines seen",
        all_files.len(),
        total_split,
        total_lines
    );
}
