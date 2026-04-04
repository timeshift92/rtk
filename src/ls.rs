use crate::tracking;
#[cfg(not(target_os = "windows"))]
use crate::utils::resolved_command;
use anyhow::{Context, Result};
use std::path::Path;

/// Noise directories commonly excluded from LLM context
const NOISE_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "__pycache__",
    ".next",
    "dist",
    "build",
    ".cache",
    ".turbo",
    ".vercel",
    ".pytest_cache",
    ".mypy_cache",
    ".tox",
    ".venv",
    "venv",
    "coverage",
    ".nyc_output",
    ".DS_Store",
    "Thumbs.db",
    ".idea",
    ".vscode",
    ".vs",
    "*.egg-info",
    ".eggs",
];

pub fn run(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    // Separate flags from paths
    let show_all = args
        .iter()
        .any(|a| (a.starts_with('-') && !a.starts_with("--") && a.contains('a')) || a == "--all");

    #[cfg(not(target_os = "windows"))]
    let flags: Vec<&str> = args
        .iter()
        .filter(|a| a.starts_with('-'))
        .map(|s| s.as_str())
        .collect();
    let paths: Vec<&str> = args
        .iter()
        .filter(|a| !a.starts_with('-'))
        .map(|s| s.as_str())
        .collect();

    #[cfg(target_os = "windows")]
    let (raw, filtered) = {
        let target_paths = if paths.is_empty() {
            vec!["."]
        } else {
            paths.clone()
        };
        let filtered = list_paths_native(&target_paths, show_all)?;
        (filtered.clone(), filtered)
    };

    #[cfg(not(target_os = "windows"))]
    let (raw, filtered) = {
        // Build ls -la + any extra flags the user passed (e.g. -R)
        // Strip -l, -a, -h (we handle all of these ourselves)
        let mut cmd = resolved_command("ls");
        cmd.arg("-la");
        for flag in &flags {
            if flag.starts_with("--") {
                // Long flags: skip --all (already handled)
                if *flag != "--all" {
                    cmd.arg(flag);
                }
            } else {
                let stripped = flag.trim_start_matches('-');
                let extra: String = stripped
                    .chars()
                    .filter(|c| *c != 'l' && *c != 'a' && *c != 'h')
                    .collect();
                if !extra.is_empty() {
                    cmd.arg(format!("-{}", extra));
                }
            }
        }

        // Add paths (default to "." if none)
        if paths.is_empty() {
            cmd.arg(".");
        } else {
            for p in &paths {
                cmd.arg(p);
            }
        }

        let output = cmd.output().context("Failed to run ls")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprint!("{}", stderr);
            std::process::exit(output.status.code().unwrap_or(1));
        }

        let raw = String::from_utf8_lossy(&output.stdout).to_string();
        let filtered = compact_ls(&raw, show_all);
        (raw, filtered)
    };

    if verbose > 0 {
        eprintln!(
            "Chars: {} → {} ({}% reduction)",
            raw.len(),
            filtered.len(),
            if !raw.is_empty() {
                100 - (filtered.len() * 100 / raw.len())
            } else {
                0
            }
        );
    }

    let target_display = if paths.is_empty() {
        ".".to_string()
    } else {
        paths.join(" ")
    };
    print!("{}", filtered);
    timer.track(
        &format!("ls -la {}", target_display),
        "rtk ls",
        &raw,
        &filtered,
    );

    Ok(())
}

#[derive(Debug)]
struct NativeLsEntry {
    name: String,
    is_dir: bool,
    size: u64,
}

#[cfg(target_os = "windows")]
fn list_paths_native(paths: &[&str], show_all: bool) -> Result<String> {
    let mut sections = Vec::new();

    for path in paths {
        let listing = list_directory_native(Path::new(path), show_all)?;
        if paths.len() == 1 {
            sections.push(listing);
        } else {
            sections.push(format!("{}:\n{}", path, listing));
        }
    }

    Ok(sections.join("\n"))
}

#[cfg(target_os = "windows")]
fn list_directory_native(path: &Path, show_all: bool) -> Result<String> {
    let mut entries = Vec::new();

    for entry in std::fs::read_dir(path)
        .with_context(|| format!("Failed to read directory '{}'", path.display()))?
    {
        let entry = entry
            .with_context(|| format!("Failed to read directory entry in '{}'", path.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("Failed to inspect '{}'", entry.path().display()))?;
        let metadata = entry
            .metadata()
            .with_context(|| format!("Failed to read metadata for '{}'", entry.path().display()))?;
        let name = entry.file_name().to_string_lossy().to_string();

        if !show_all && NOISE_DIRS.iter().any(|noise| name == *noise) {
            continue;
        }

        entries.push(NativeLsEntry {
            name,
            is_dir: file_type.is_dir(),
            size: if file_type.is_dir() {
                0
            } else {
                metadata.len()
            },
        });
    }

    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(render_native_entries(&entries))
}

#[cfg(target_os = "windows")]
fn render_native_entries(entries: &[NativeLsEntry]) -> String {
    use std::collections::HashMap;

    if entries.is_empty() {
        return "(empty)\n".to_string();
    }

    let mut dirs = Vec::new();
    let mut files = Vec::new();
    let mut by_ext: HashMap<String, usize> = HashMap::new();

    for entry in entries {
        if entry.is_dir {
            dirs.push(entry.name.clone());
        } else {
            let ext = if let Some(pos) = entry.name.rfind('.') {
                entry.name[pos..].to_string()
            } else {
                "no ext".to_string()
            };
            *by_ext.entry(ext).or_insert(0) += 1;
            files.push((entry.name.clone(), human_size(entry.size)));
        }
    }

    let mut out = String::new();

    for dir in &dirs {
        out.push_str(dir);
        out.push_str("/\n");
    }

    for (name, size) in &files {
        out.push_str(name);
        out.push_str("  ");
        out.push_str(size);
        out.push('\n');
    }

    out.push('\n');
    let mut summary = format!("{} files, {} dirs", files.len(), dirs.len());
    if !by_ext.is_empty() {
        let mut ext_counts: Vec<_> = by_ext.iter().collect();
        ext_counts.sort_by(|a, b| b.1.cmp(a.1));
        let ext_parts: Vec<String> = ext_counts
            .iter()
            .take(5)
            .map(|(ext, count)| format!("{} {}", count, ext))
            .collect();
        summary.push_str(" (");
        summary.push_str(&ext_parts.join(", "));
        if ext_counts.len() > 5 {
            summary.push_str(&format!(", +{} more", ext_counts.len() - 5));
        }
        summary.push(')');
    }
    out.push_str(&summary);
    out.push('\n');

    out
}

/// Format bytes into human-readable size
fn human_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1}M", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1}K", bytes as f64 / 1024.0)
    } else {
        format!("{}B", bytes)
    }
}

/// Parse ls -la output into compact format:
///   name/  (dirs)
///   name  size  (files)
#[cfg_attr(target_os = "windows", allow(dead_code))]
fn compact_ls(raw: &str, show_all: bool) -> String {
    use std::collections::HashMap;

    let mut dirs: Vec<String> = Vec::new();
    let mut files: Vec<(String, String)> = Vec::new(); // (name, size)
    let mut by_ext: HashMap<String, usize> = HashMap::new();

    for line in raw.lines() {
        // Skip total, empty, . and ..
        if line.starts_with("total ") || line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 9 {
            continue;
        }

        // Filename is everything from column 9 onward (handles spaces)
        let name = parts[8..].join(" ");

        // Skip . and ..
        if name == "." || name == ".." {
            continue;
        }

        // Filter noise dirs unless -a
        if !show_all && NOISE_DIRS.iter().any(|noise| name == *noise) {
            continue;
        }

        let is_dir = parts[0].starts_with('d');

        if is_dir {
            dirs.push(name);
        } else if parts[0].starts_with('-') || parts[0].starts_with('l') {
            let size: u64 = parts[4].parse().unwrap_or(0);
            let ext = if let Some(pos) = name.rfind('.') {
                name[pos..].to_string()
            } else {
                "no ext".to_string()
            };
            *by_ext.entry(ext).or_insert(0) += 1;
            files.push((name, human_size(size)));
        }
    }

    if dirs.is_empty() && files.is_empty() {
        return "(empty)\n".to_string();
    }

    let mut out = String::new();

    // Dirs first, compact
    for d in &dirs {
        out.push_str(d);
        out.push_str("/\n");
    }

    // Files with size
    for (name, size) in &files {
        out.push_str(name);
        out.push_str("  ");
        out.push_str(size);
        out.push('\n');
    }

    // Summary line
    out.push('\n');
    let mut summary = format!("{} files, {} dirs", files.len(), dirs.len());
    if !by_ext.is_empty() {
        let mut ext_counts: Vec<_> = by_ext.iter().collect();
        ext_counts.sort_by(|a, b| b.1.cmp(a.1));
        let ext_parts: Vec<String> = ext_counts
            .iter()
            .take(5)
            .map(|(ext, count)| format!("{} {}", count, ext))
            .collect();
        summary.push_str(" (");
        summary.push_str(&ext_parts.join(", "));
        if ext_counts.len() > 5 {
            summary.push_str(&format!(", +{} more", ext_counts.len() - 5));
        }
        summary.push(')');
    }
    out.push_str(&summary);
    out.push('\n');

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_compact_basic() {
        let input = "total 48\n\
                     drwxr-xr-x  2 user  staff    64 Jan  1 12:00 .\n\
                     drwxr-xr-x  2 user  staff    64 Jan  1 12:00 ..\n\
                     drwxr-xr-x  2 user  staff    64 Jan  1 12:00 src\n\
                     -rw-r--r--  1 user  staff  1234 Jan  1 12:00 Cargo.toml\n\
                     -rw-r--r--  1 user  staff  5678 Jan  1 12:00 README.md\n";
        let output = compact_ls(input, false);
        assert!(output.contains("src/"));
        assert!(output.contains("Cargo.toml"));
        assert!(output.contains("README.md"));
        assert!(output.contains("1.2K")); // 1234 bytes
        assert!(output.contains("5.5K")); // 5678 bytes
        assert!(!output.contains("drwx")); // no permissions
        assert!(!output.contains("staff")); // no group
        assert!(!output.contains("total")); // no total
        assert!(!output.contains("\n.\n")); // no . entry
        assert!(!output.contains("\n..\n")); // no .. entry
    }

    #[test]
    fn test_compact_filters_noise() {
        let input = "total 8\n\
                     drwxr-xr-x  2 user  staff  64 Jan  1 12:00 node_modules\n\
                     drwxr-xr-x  2 user  staff  64 Jan  1 12:00 .git\n\
                     drwxr-xr-x  2 user  staff  64 Jan  1 12:00 target\n\
                     drwxr-xr-x  2 user  staff  64 Jan  1 12:00 src\n\
                     -rw-r--r--  1 user  staff  100 Jan  1 12:00 main.rs\n";
        let output = compact_ls(input, false);
        assert!(!output.contains("node_modules"));
        assert!(!output.contains(".git"));
        assert!(!output.contains("target"));
        assert!(output.contains("src/"));
        assert!(output.contains("main.rs"));
    }

    #[test]
    fn test_compact_show_all() {
        let input = "total 8\n\
                     drwxr-xr-x  2 user  staff  64 Jan  1 12:00 .git\n\
                     drwxr-xr-x  2 user  staff  64 Jan  1 12:00 src\n";
        let output = compact_ls(input, true);
        assert!(output.contains(".git/"));
        assert!(output.contains("src/"));
    }

    #[test]
    fn test_compact_empty() {
        let input = "total 0\n";
        let output = compact_ls(input, false);
        assert_eq!(output, "(empty)\n");
    }

    #[test]
    fn test_compact_summary() {
        let input = "total 48\n\
                     drwxr-xr-x  2 user  staff    64 Jan  1 12:00 src\n\
                     -rw-r--r--  1 user  staff  1234 Jan  1 12:00 main.rs\n\
                     -rw-r--r--  1 user  staff  5678 Jan  1 12:00 lib.rs\n\
                     -rw-r--r--  1 user  staff   100 Jan  1 12:00 Cargo.toml\n";
        let output = compact_ls(input, false);
        assert!(output.contains("3 files, 1 dirs"));
        assert!(output.contains(".rs"));
        assert!(output.contains(".toml"));
    }

    #[test]
    fn test_human_size() {
        assert_eq!(human_size(0), "0B");
        assert_eq!(human_size(500), "500B");
        assert_eq!(human_size(1024), "1.0K");
        assert_eq!(human_size(1234), "1.2K");
        assert_eq!(human_size(1_048_576), "1.0M");
        assert_eq!(human_size(2_500_000), "2.4M");
    }

    #[test]
    fn test_compact_handles_filenames_with_spaces() {
        let input = "total 8\n\
                     -rw-r--r--  1 user  staff  1234 Jan  1 12:00 my file.txt\n";
        let output = compact_ls(input, false);
        assert!(output.contains("my file.txt"));
    }

    #[test]
    fn test_compact_symlinks() {
        let input = "total 8\n\
                     lrwxr-xr-x  1 user  staff  10 Jan  1 12:00 link -> target\n";
        let output = compact_ls(input, false);
        assert!(output.contains("link -> target"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_list_directory_native_windows_formats_entries() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let root = temp_dir.path();

        fs::create_dir(root.join("src")).expect("failed to create src dir");
        fs::create_dir(root.join("target")).expect("failed to create target dir");
        fs::write(root.join("Cargo.toml"), b"[package]\nname = \"demo\"\n")
            .expect("failed to write Cargo.toml");

        let output = list_directory_native(root, false).expect("native listing should succeed");

        assert!(output.contains("src/"));
        assert!(output.contains("Cargo.toml"));
        assert!(output.contains("3 files, 1 dirs") || output.contains("1 files, 1 dirs"));
        assert!(!output.contains("target/"));
    }
}
