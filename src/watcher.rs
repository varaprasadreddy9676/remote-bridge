use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use crate::transport::Transport;

/// Recursively snapshots all file modification times under `dir`,
/// skipping entries whose path contains any of the `exclude` patterns.
pub fn snapshot_dir(
    dir: &str,
    exclude: &[String],
) -> Result<HashMap<PathBuf, SystemTime>, Box<dyn std::error::Error>> {
    let mut map = HashMap::new();
    collect_mtimes(Path::new(dir), &mut map, exclude)?;
    Ok(map)
}

fn collect_mtimes(
    dir: &Path,
    map: &mut HashMap<PathBuf, SystemTime>,
    exclude: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        if is_excluded(&name, exclude) {
            continue;
        }

        if path.is_dir() {
            collect_mtimes(&path, map, exclude)?;
        } else {
            if let Ok(meta) = entry.metadata() {
                if let Ok(mtime) = meta.modified() {
                    map.insert(path, mtime);
                }
            }
        }
    }
    Ok(())
}

fn is_excluded(name: &str, exclude: &[String]) -> bool {
    exclude
        .iter()
        .any(|pat| name.starts_with(pat.trim_end_matches('/')))
}

/// Returns the set of paths that are new or have a changed mtime.
pub fn changed_paths(
    before: &HashMap<PathBuf, SystemTime>,
    after: &HashMap<PathBuf, SystemTime>,
) -> Vec<PathBuf> {
    after
        .iter()
        .filter(|(path, &new_mtime)| {
            before
                .get(*path)
                .map(|&old_mtime| old_mtime != new_mtime)
                .unwrap_or(true) // new file
        })
        .map(|(p, _)| p.clone())
        .collect()
}

/// Polls `local_path` every `interval_secs` seconds.
/// Triggers `transport.sync_files` when any file changes.
/// Runs until Ctrl-C (SIGINT).
pub fn watch_and_sync(
    local_path: &str,
    transport: &Transport,
    exclude: Vec<String>,
    interval_secs: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "Watching '{}' for changes every {}s — press Ctrl-C to stop.",
        local_path, interval_secs
    );

    let mut snapshot = snapshot_dir(local_path, &exclude)?;

    loop {
        thread::sleep(Duration::from_secs(interval_secs));

        let new_snapshot = match snapshot_dir(local_path, &exclude) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Watch: error scanning directory: {}", e);
                continue;
            }
        };

        let changed = changed_paths(&snapshot, &new_snapshot);

        if !changed.is_empty() {
            println!(
                "Detected {} changed file(s). Syncing...",
                changed.len()
            );
            for path in &changed {
                println!("  ~ {}", path.display());
            }

            if let Err(e) = transport.sync_files(local_path, exclude.clone(), false) {
                eprintln!("Sync error: {}", e);
            }

            snapshot = new_snapshot;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_temp_dir_with_files(files: &[(&str, &str)]) -> TempDir {
        let dir = TempDir::new().unwrap();
        for (name, content) in files {
            let path = dir.path().join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&path, content).unwrap();
        }
        dir
    }

    #[test]
    fn test_snapshot_captures_files() {
        let dir = make_temp_dir_with_files(&[("a.txt", "hello"), ("b.txt", "world")]);
        let snapshot = snapshot_dir(dir.path().to_str().unwrap(), &[]).unwrap();
        assert_eq!(snapshot.len(), 2);
    }

    #[test]
    fn test_snapshot_empty_dir() {
        let dir = TempDir::new().unwrap();
        let snapshot = snapshot_dir(dir.path().to_str().unwrap(), &[]).unwrap();
        assert!(snapshot.is_empty());
    }

    #[test]
    fn test_snapshot_respects_exclude() {
        let dir = TempDir::new().unwrap();
        // Create app.rs (should appear in snapshot)
        fs::write(dir.path().join("app.rs"), "fn main() {}").unwrap();
        // Create .git directory with a file inside (should be excluded)
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join(".git/HEAD"), "ref: refs/heads/main").unwrap();

        let snapshot =
            snapshot_dir(dir.path().to_str().unwrap(), &[".git".to_string()]).unwrap();

        // .git and its contents should be excluded; only app.rs should appear
        assert_eq!(snapshot.len(), 1);
        for path in snapshot.keys() {
            assert!(
                !path.to_string_lossy().contains(".git"),
                "Excluded path leaked: {}",
                path.display()
            );
        }
    }

    #[test]
    fn test_snapshot_detects_new_file() {
        let dir = make_temp_dir_with_files(&[("a.txt", "hello")]);
        let before = snapshot_dir(dir.path().to_str().unwrap(), &[]).unwrap();

        // Create a new file
        fs::write(dir.path().join("b.txt"), "new").unwrap();

        let after = snapshot_dir(dir.path().to_str().unwrap(), &[]).unwrap();
        let changed = changed_paths(&before, &after);

        assert_eq!(changed.len(), 1);
        assert!(changed[0].to_string_lossy().contains("b.txt"));
    }

    #[test]
    fn test_snapshot_detects_modified_file() {
        let dir = make_temp_dir_with_files(&[("a.txt", "original")]);
        let before = snapshot_dir(dir.path().to_str().unwrap(), &[]).unwrap();

        // Overwrite with new content — mtime will change (may need a small sleep
        // on fast filesystems with 1-second mtime granularity)
        thread::sleep(Duration::from_millis(10));
        let path = dir.path().join("a.txt");
        let mut f = fs::OpenOptions::new().write(true).open(&path).unwrap();
        f.write_all(b"modified").unwrap();
        f.flush().unwrap();
        drop(f);
        // Force mtime bump on systems with coarse-grained timestamps
        let t = std::time::UNIX_EPOCH + Duration::from_secs(
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 1,
        );
        filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(t)).ok();

        let after = snapshot_dir(dir.path().to_str().unwrap(), &[]).unwrap();
        let changed = changed_paths(&before, &after);
        assert_eq!(changed.len(), 1);
    }

    #[test]
    fn test_changed_paths_no_changes() {
        let dir = make_temp_dir_with_files(&[("a.txt", "hello")]);
        let snapshot = snapshot_dir(dir.path().to_str().unwrap(), &[]).unwrap();
        let changed = changed_paths(&snapshot, &snapshot);
        assert!(changed.is_empty());
    }

    #[test]
    fn test_is_excluded_matches_prefix() {
        assert!(is_excluded(".git", &[".git".to_string()]));
        assert!(is_excluded("node_modules", &["node_modules/".to_string()]));
        assert!(!is_excluded("src", &[".git".to_string()]));
    }

    #[test]
    fn test_snapshot_recursive() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("src/nested")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("src/nested/utils.rs"), "// util").unwrap();

        let snapshot = snapshot_dir(dir.path().to_str().unwrap(), &[]).unwrap();
        assert_eq!(snapshot.len(), 2);
    }
}
