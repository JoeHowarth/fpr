// src/main.rs
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use globwalk::GlobWalkerBuilder;
use walkdir::WalkDir;

/// Simple file‑print utility (`fpr`).
///
/// Supports:
/// * Plain paths
/// * Shell‑style globs (`*.rs`, `**/*.txt`, etc.)
/// * **Rust‑like grouping** with parentheses and commas, e.g.
///   `src/(main.rs, lib.rs, util/(fs, time), -tests)`.
///     * `-item` or `^item` inside a group **excludes** that path.
///     * Nesting is allowed.
///     * Assume `(`, `)`, and `,` do not appear in actual filenames.
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// Paths, globs, or grouped patterns to print
    #[arg(required = true)]
    inputs: Vec<String>,

    /// Separator printed between files (default: "---")
    #[arg(long, default_value = "---")]
    separator: String,

    /// Recurse into sub‑directories when an input is a directory
    #[arg(short, long, default_value_t = true)]
    recursive: bool,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut files: Vec<PathBuf> = Vec::new();

    for raw in &cli.inputs {
        // 1. Expand custom grouping syntax first.
        let patterns = if raw.contains('(') {
            expand_group_pattern(raw)?
        } else {
            vec![raw.clone()]
        };

        // 2. Handle each resulting pattern as before.
        for pat in patterns {
            if is_glob(&pat) {
                expand_glob(&pat, &mut files)?;
            } else {
                let path = PathBuf::from(&pat);
                if path.is_dir() {
                    expand_dir(&path, cli.recursive, &mut files)?;
                } else if path.is_file() {
                    files.push(path);
                } else {
                    anyhow::bail!("Input `{}` does not exist", pat);
                }
            }
        }
    }

    files.sort();
    files.dedup();

    let cwd = std::env::current_dir()?;

    for (idx, path) in files.iter().enumerate() {
        let rel = path.strip_prefix(&cwd).unwrap_or(path);
        println!("=== {} ===", rel.display());
        let content = fs::read_to_string(path)?;
        print!("{content}");

        if idx + 1 < files.len() {
            println!();
            println!("{}", cli.separator);
            println!();
        }
    }

    Ok(())
}

/// Heuristic: does the string look like a glob?
fn is_glob(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[')
}

/// Expand a glob pattern into actual file paths.
fn expand_glob(pattern: &str, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    let walker = GlobWalkerBuilder::from_patterns(".", &[pattern])
        .case_insensitive(false)
        .build()
        .map_err(|e| anyhow::anyhow!("invalid glob `{pattern}`: {e}"))?;

    for entry in walker
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        out.push(entry.into_path());
    }
    Ok(())
}

/// Recurse through a directory (optionally deeply) collecting files.
fn expand_dir(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    if recursive {
        for entry in WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            out.push(entry.into_path());
        }
    } else {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                out.push(path);
            }
        }
    }
    Ok(())
}

// ───────────────────────────────── GROUP SYNTAX ─────────────────────────────

/// Expand a single argument that may use parenthetical grouping and exclusions.
/// Returns a list of concrete path or glob strings **after** applying exclusions.
fn expand_group_pattern(pattern: &str) -> anyhow::Result<Vec<String>> {
    // Inner recursive function that builds (string, is_excluded) pairs.
    fn expand_rec(span: &str) -> anyhow::Result<Vec<(String, bool)>> {
        let mut acc: Vec<(String, bool)> = vec![(String::new(), false)];
        let chars: Vec<char> = span.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            match chars[i] {
                '(' => {
                    // Parse group and combine cartesian‑style.
                    let (group_items, next_i) = parse_group(&chars, i + 1)?;
                    let mut new_acc = Vec::new();
                    for (prefix, pref_excl) in &acc {
                        for (suffix, suff_excl) in &group_items {
                            new_acc.push((format!("{prefix}{suffix}"), *pref_excl || *suff_excl));
                        }
                    }
                    acc = new_acc;
                    i = next_i;
                }
                _ => {
                    // Append the char to all current strings.
                    for (s, _) in &mut acc {
                        s.push(chars[i]);
                    }
                    i += 1;
                }
            }
        }
        Ok(acc)
    }

    /// Parse the comma‑separated list inside a `(` … `)`.
    fn parse_group(chars: &[char], mut i: usize) -> anyhow::Result<(Vec<(String, bool)>, usize)> {
        let mut segments: Vec<String> = Vec::new();
        let mut depth = 0;
        let mut start = i;

        while i < chars.len() {
            match chars[i] {
                '(' => {
                    depth += 1;
                    i += 1;
                }
                ')' if depth == 0 => {
                    // Push the final segment.
                    segments.push(chars[start..i].iter().collect());
                    i += 1; // consume ')'
                    break;
                }
                ')' => {
                    depth -= 1;
                    i += 1;
                }
                ',' if depth == 0 => {
                    segments.push(chars[start..i].iter().collect());
                    i += 1; // consume ','
                    start = i;
                }
                _ => i += 1,
            }
        }

        if i > chars.len() {
            anyhow::bail!("Unmatched '(' in pattern");
        }

        let mut out: Vec<(String, bool)> = Vec::new();
        for seg in segments {
            let trimmed = seg.trim();
            if trimmed.is_empty() {
                continue;
            }
            let (is_excl, body) = if trimmed.starts_with('-') || trimmed.starts_with('^') {
                (true, &trimmed[1..])
            } else {
                (false, trimmed)
            };
            let sub_items = expand_rec(body)?;
            for (s, sub_excl) in sub_items {
                out.push((s, is_excl || sub_excl));
            }
        }
        Ok((out, i))
    }

    // Kick off recursive expansion for the full pattern.
    let pairs = expand_rec(pattern)?;
    let mut includes = Vec::new();
    let mut excludes: HashSet<String> = HashSet::new();

    for (s, excl) in pairs {
        if excl {
            excludes.insert(s);
        } else {
            includes.push(s);
        }
    }

    // Remove any includes that were marked for exclusion.
    includes.retain(|p| !excludes.contains(p));
    Ok(includes)
}