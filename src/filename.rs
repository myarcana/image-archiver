use anyhow::Result;
use chrono::{DateTime, Datelike, Timelike, Utc};
use std::path::{Path, PathBuf};

use crate::metadata::MediaDates;

/// Generate a normalized filename based on creation and modification dates
pub fn generate_filename(
    dates: &MediaDates,
    original_extension: &str,
    counter: u32,
) -> String {
    let creation = format_date(&dates.creation_date);
    let modification = format_date(&dates.modify_date);
    let ext = normalize_extension(original_extension);

    format!("{} {} {}.{}", creation, modification, counter, ext)
}

/// Generate filename without counter (for parallel processing)
/// Returns the base filename that will have a counter appended
pub fn generate_filename_without_counter(
    dates: &MediaDates,
    original_extension: &str,
) -> String {
    let creation = format_date(&dates.creation_date);
    let modification = format_date(&dates.modify_date);
    let ext = normalize_extension(original_extension);

    format!("{} {}.{}", creation, modification, ext)
}

/// Format a date as YYYY-MM-DD_HH.mm.SS.NNN
fn format_date(date: &DateTime<Utc>) -> String {
    format!(
        "{:04}-{:02}-{:02}_{:02}.{:02}.{:02}.{:03}",
        date.year(),
        date.month(),
        date.day(),
        date.hour(),
        date.minute(),
        date.second(),
        date.timestamp_subsec_millis()
    )
}

/// Normalize file extension: uppercase, JPEG -> JPG
pub fn normalize_extension(ext: &str) -> String {
    let upper = ext.to_uppercase();
    if upper == "JPEG" {
        "JPG".to_string()
    } else {
        upper
    }
}

/// Get the file extension from a path
pub fn get_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

/// Find the next available filename with incrementing counter
pub fn find_available_filename(
    output_dir: &Path,
    dates: &MediaDates,
    original_extension: &str,
    existing_content: Option<&[u8]>,
) -> Result<(PathBuf, u32)> {
    let mut counter = 1;

    loop {
        let filename = generate_filename(dates, original_extension, counter);
        let target_path = output_dir.join(&filename);

        if !target_path.exists() {
            return Ok((target_path, counter));
        }

        // File exists, check if it's the same content
        if let Some(content) = existing_content {
            let existing = std::fs::read(&target_path)?;
            if existing == content {
                // Same file already exists, no need to copy
                return Ok((target_path, counter));
            }
        }

        // Different file or we don't have content to compare, increment counter
        counter += 1;

        if counter > 10000 {
            anyhow::bail!("Too many filename collisions for the same date pair");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_format_date() {
        let date = Utc.with_ymd_and_hms(2025, 12, 17, 21, 58, 0).unwrap();
        let date = date + chrono::Duration::milliseconds(816);
        assert_eq!(format_date(&date), "2025-12-17_21.58.00.816");
    }

    #[test]
    fn test_normalize_extension() {
        assert_eq!(normalize_extension("jpg"), "JPG");
        assert_eq!(normalize_extension("JPG"), "JPG");
        assert_eq!(normalize_extension("jpeg"), "JPG");
        assert_eq!(normalize_extension("JPEG"), "JPG");
        assert_eq!(normalize_extension("mov"), "MOV");
        assert_eq!(normalize_extension("heic"), "HEIC");
    }

    #[test]
    fn test_generate_filename() {
        let creation = Utc.with_ymd_and_hms(2025, 8, 10, 3, 43, 16).unwrap();
        let modification = Utc.with_ymd_and_hms(2025, 8, 10, 3, 43, 16).unwrap();

        let dates = MediaDates {
            creation_date: creation,
            modify_date: modification,
        };

        let filename = generate_filename(&dates, "MOV", 1);
        assert_eq!(
            filename,
            "2025-08-10_03.43.16.000 2025-08-10_03.43.16.000 1.MOV"
        );
    }
}
