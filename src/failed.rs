use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Handle a failed file by creating a symlink and debug info file
pub fn handle_failed_file(
    file_path: &Path,
    failed_cases_dir: &Path,
    error: &anyhow::Error,
) -> Result<()> {
    // Get original filename
    let original_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    // Find available symlink name
    let symlink_path = find_available_symlink_name(failed_cases_dir, original_name)?;

    // Create symlink to original file
    unix_fs::symlink(file_path, &symlink_path)
        .with_context(|| format!("Failed to create symlink at {}", symlink_path.display()))?;

    // Create debug info file
    let debug_file_path = symlink_path.with_extension(
        format!(
            "{}.txt",
            symlink_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
        )
        .trim_start_matches('.')
    );

    let debug_info = generate_debug_info(file_path, error)?;
    fs::write(&debug_file_path, debug_info)
        .with_context(|| format!("Failed to write debug info to {}", debug_file_path.display()))?;

    println!(
        "Failed to process {}: {} (see {})",
        file_path.display(),
        error,
        debug_file_path.display()
    );

    Ok(())
}

/// Find an available symlink name (add counter if needed)
fn find_available_symlink_name(failed_cases_dir: &Path, original_name: &str) -> Result<PathBuf> {
    let base_path = failed_cases_dir.join(original_name);

    if !base_path.exists() {
        return Ok(base_path);
    }

    // Add counter to make unique
    let stem = Path::new(original_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(original_name);
    let ext = Path::new(original_name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    for counter in 1..10000 {
        let new_name = if ext.is_empty() {
            format!("{}-{}", stem, counter)
        } else {
            format!("{}-{}.{}", stem, counter, ext)
        };

        let path = failed_cases_dir.join(&new_name);
        if !path.exists() {
            return Ok(path);
        }
    }

    anyhow::bail!("Could not find available symlink name for {}", original_name);
}

/// Generate debug information for a failed file
fn generate_debug_info(file_path: &Path, error: &anyhow::Error) -> Result<String> {
    let mut info = String::new();

    // Filename and extension
    info.push_str("=== FILE INFORMATION ===\n");
    info.push_str(&format!("File: {}\n", file_path.display()));
    if let Some(ext) = file_path.extension() {
        info.push_str(&format!("Extension: {}\n", ext.to_string_lossy()));
    }
    info.push_str("\n");

    // File metadata (times)
    info.push_str("=== FILE TIMESTAMPS ===\n");
    if let Ok(metadata) = fs::metadata(file_path) {
        if let Ok(created) = metadata.created() {
            info.push_str(&format!("Created: {:?}\n", created));
        }
        if let Ok(accessed) = metadata.accessed() {
            info.push_str(&format!("Accessed: {:?}\n", accessed));
        }
        if let Ok(modified) = metadata.modified() {
            info.push_str(&format!("Modified: {:?}\n", modified));
        }
    }
    info.push_str("\n");

    // File command (MIME type)
    info.push_str("=== MIME TYPE (file command) ===\n");
    match Command::new("file")
        .arg("--mime-type")
        .arg(file_path)
        .output()
    {
        Ok(output) => {
            info.push_str(&String::from_utf8_lossy(&output.stdout));
        }
        Err(e) => {
            info.push_str(&format!("Error running file command: {}\n", e));
        }
    }
    info.push_str("\n");

    // mdls command (macOS metadata)
    info.push_str("=== macOS METADATA (mdls) ===\n");
    match Command::new("mdls")
        .arg("-name")
        .arg("kMDItemContentTypeTree")
        .arg("-name")
        .arg("kMDItemKind")
        .arg(file_path)
        .output()
    {
        Ok(output) => {
            info.push_str(&String::from_utf8_lossy(&output.stdout));
        }
        Err(e) => {
            info.push_str(&format!("Error running mdls command: {}\n", e));
        }
    }
    info.push_str("\n");

    // Error information
    info.push_str("=== ERROR ===\n");
    info.push_str(&format!("{:#}\n", error));

    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_available_symlink_name() {
        // This would need a temporary directory to test properly
    }
}
