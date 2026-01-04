use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use exiftool::ExifTool;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Order of preference for creation date extraction
const CREATION_DATE_TAGS: &[&str] = &[
    "DateTimeOriginal",
    "MediaCreateDate",
    "CreateDate",
    "TrackCreateDate",
    "CreationDate",
    "ModifyDate",
    "MediaModifyDate",
    "UserComment",
    "TrackModifyDate",
    "FileModifyDate",
];

/// Order of preference for modification date extraction
const MODIFY_DATE_TAGS: &[&str] = &[
    "ModifyDate",
    "UserComment",
    "MediaModifyDate",
    "TrackModifyDate",
    "CreateDate",
    "DateTimeOriginal",
    "TrackCreateDate",
    "MediaCreateDate",
    "CreationDate",
    "FileModifyDate",
];

/// Epoch timestamps to reject (as Unix timestamps)
const REJECTED_EPOCHS: &[i64] = &[
    0,                    // Zero/Unix epoch
    -2209075200,         // 1900-01-01 (NTP epoch)
    -2082844800,         // 1904-01-01 (macOS Classic epoch)
    315532800,           // 1980-01-06 (GPS epoch)
    978307200,           // 2001-01-01 (macOS modern/iOS epoch)
];

const YEAR_2010: i64 = 1262304000; // 2010-01-01 00:00:00 UTC

#[derive(Debug, Clone)]
pub struct MediaDates {
    pub creation_date: DateTime<Utc>,
    pub modify_date: DateTime<Utc>,
}

/// Extract metadata from a file using exiftool
pub fn extract_dates(file_path: &Path) -> Result<MediaDates> {
    // First try fast extraction
    let metadata = extract_with_exiftool(file_path, false)?;

    // Extract dates
    let creation_date = extract_creation_date(&metadata)?;
    let modify_date = extract_modify_date(&metadata)?;

    // If we found valid dates, return them
    if let (Some(creation), Some(modify)) = (creation_date, modify_date) {
        // Warn if dates are before 2010
        if creation.timestamp() < YEAR_2010 {
            eprintln!(
                "Warning: File {} has creation date before 2010: {}",
                file_path.display(),
                creation
            );
        }
        if modify.timestamp() < YEAR_2010 {
            eprintln!(
                "Warning: File {} has modification date before 2010: {}",
                file_path.display(),
                modify
            );
        }

        return Ok(MediaDates {
            creation_date: creation,
            modify_date: modify,
        });
    }

    // Fallback to ExtractEmbedded
    let metadata = extract_with_exiftool(file_path, true)?;
    let creation_date = extract_creation_date(&metadata)?
        .ok_or_else(|| anyhow!("No valid creation date found"))?;
    let modify_date = extract_modify_date(&metadata)?
        .ok_or_else(|| anyhow!("No valid modification date found"))?;

    // Warn if dates are before 2010
    if creation_date.timestamp() < YEAR_2010 {
        eprintln!(
            "Warning: File {} has creation date before 2010: {}",
            file_path.display(),
            creation_date
        );
    }
    if modify_date.timestamp() < YEAR_2010 {
        eprintln!(
            "Warning: File {} has modification date before 2010: {}",
            file_path.display(),
            modify_date
        );
    }

    Ok(MediaDates {
        creation_date,
        modify_date,
    })
}

/// Extract metadata from multiple files in batch using exiftool
/// Returns a HashMap mapping file paths to their extracted dates or errors
/// Uses adaptive batch sizing: if a batch fails, splits it in half and retries
pub fn extract_dates_batch(exiftool: &mut ExifTool, file_paths: &[PathBuf]) -> HashMap<PathBuf, Result<MediaDates>> {
    extract_dates_batch_adaptive(exiftool, file_paths)
}

/// Adaptive batch processing: tries to process files in batches, splitting on failure
fn extract_dates_batch_adaptive(
    exiftool: &mut ExifTool,
    file_paths: &[PathBuf],
) -> HashMap<PathBuf, Result<MediaDates>> {
    let mut results: HashMap<PathBuf, Result<MediaDates>> = HashMap::new();

    if file_paths.is_empty() {
        return results;
    }

    // Try extracting the full batch
    match try_extract_batch(exiftool, file_paths) {
        Ok(batch_results) => {
            // Batch succeeded, add all results
            results.extend(batch_results);
        }
        Err(batch_err) if file_paths.len() == 1 => {
            // Single file failed, return the error
            results.insert(
                file_paths[0].clone(),
                Err(anyhow!("Failed to extract metadata: {}", batch_err)),
            );
        }
        Err(_) => {
            // Batch failed, split in half and retry each half
            let mid = file_paths.len() / 2;
            let (left, right) = file_paths.split_at(mid);

            eprintln!(
                "Batch of {} files failed, splitting into {} + {} and retrying...",
                file_paths.len(),
                left.len(),
                right.len()
            );

            results.extend(extract_dates_batch_adaptive(exiftool, left));
            results.extend(extract_dates_batch_adaptive(exiftool, right));
        }
    }

    results
}

/// Try to extract dates from a batch of files
/// Returns Err if the exiftool batch operation fails (allows retry with smaller batch)
fn try_extract_batch(
    exiftool: &mut ExifTool,
    file_paths: &[PathBuf],
) -> Result<HashMap<PathBuf, Result<MediaDates>>> {
    // Always use -ee (ExtractEmbedded) for thorough metadata extraction
    let metadata_map = extract_batch_with_exiftool(exiftool, file_paths, true)?;

    let mut results = HashMap::new();
    for (path, metadata_result) in metadata_map {
        let result = metadata_result
            .and_then(|metadata| extract_dates_from_metadata(&path, &metadata));
        results.insert(path, result);
    }

    Ok(results)
}

/// Helper to extract dates from already-parsed metadata
fn extract_dates_from_metadata(file_path: &Path, metadata: &HashMap<String, Value>) -> Result<MediaDates> {
    let creation_date = extract_creation_date(metadata)?
        .ok_or_else(|| anyhow!("No valid creation date found"))?;
    let modify_date = extract_modify_date(metadata)?
        .ok_or_else(|| anyhow!("No valid modification date found"))?;

    // Warn if dates are before 2010
    if creation_date.timestamp() < YEAR_2010 {
        eprintln!(
            "Warning: File {} has creation date before 2010: {}",
            file_path.display(),
            creation_date
        );
    }
    if modify_date.timestamp() < YEAR_2010 {
        eprintln!(
            "Warning: File {} has modification date before 2010: {}",
            file_path.display(),
            modify_date
        );
    }

    Ok(MediaDates {
        creation_date,
        modify_date,
    })
}

/// Extract metadata for multiple files using exiftool json_batch
/// Returns Result to allow adaptive retry on batch-level failures
fn extract_batch_with_exiftool(
    exiftool: &mut ExifTool,
    file_paths: &[PathBuf],
    extract_embedded: bool,
) -> Result<HashMap<PathBuf, Result<HashMap<String, Value>>>> {
    let mut results = HashMap::new();

    // Build arguments
    let mut args = vec!["-G"];
    if extract_embedded {
        args.push("-ee");
    }

    // Call json_batch - bubble up batch-level errors for retry
    let metadata_array = exiftool.json_batch(file_paths, &args)
        .context("Exiftool batch execution failed")?;

    // Each element in the array corresponds to a file in file_paths
    for (i, metadata_value) in metadata_array.into_iter().enumerate() {
        if i >= file_paths.len() {
            break;
        }

        let path = &file_paths[i];

        // Convert Value to HashMap<String, Value>
        match serde_json::from_value::<HashMap<String, Value>>(metadata_value) {
            Ok(metadata) => {
                results.insert(path.clone(), Ok(metadata));
            }
            Err(e) => {
                results.insert(
                    path.clone(),
                    Err(anyhow!("Failed to parse metadata: {}", e)),
                );
            }
        }
    }

    Ok(results)
}

fn extract_with_exiftool(file_path: &Path, extract_embedded: bool) -> Result<HashMap<String, Value>> {
    let mut exiftool = ExifTool::new()?;

    // Build arguments - include the file path and flags
    let file_path_str = file_path.to_str()
        .ok_or_else(|| anyhow!("File path contains invalid UTF-8"))?;

    let mut args = vec!["-G"];
    if extract_embedded {
        args.push("-ee");
    }
    args.push(file_path_str);

    // Use json_execute to get metadata with custom args
    let output = exiftool
        .json_execute(&args)
        .context("Failed to run exiftool")?;

    // The output is already a Value, convert it to Vec<HashMap>
    let data: Vec<HashMap<String, Value>> = serde_json::from_value(output)
        .context("Failed to parse exiftool JSON output")?;

    data.into_iter()
        .next()
        .ok_or_else(|| anyhow!("No metadata returned from exiftool"))
}

fn extract_creation_date(metadata: &HashMap<String, Value>) -> Result<Option<DateTime<Utc>>> {
    extract_date_by_priority(metadata, CREATION_DATE_TAGS)
}

fn extract_modify_date(metadata: &HashMap<String, Value>) -> Result<Option<DateTime<Utc>>> {
    extract_date_by_priority(metadata, MODIFY_DATE_TAGS)
}

fn extract_date_by_priority(
    metadata: &HashMap<String, Value>,
    priority_list: &[&str],
) -> Result<Option<DateTime<Utc>>> {
    // Get timezone offset if available
    let timezone_offset = extract_timezone_offset(metadata);

    for tag_name in priority_list {
        if *tag_name == "UserComment" {
            // Special handling for UserComment JSON field
            if let Some(date) = extract_date_from_user_comment(metadata)? {
                if is_valid_date(date) {
                    return Ok(Some(date));
                }
            }
        } else {
            // Try to find the tag with various group prefixes
            let date = find_and_parse_date(metadata, tag_name, timezone_offset)?;
            if let Some(d) = date {
                if is_valid_date(d) {
                    return Ok(Some(d));
                }
            }
        }
    }

    Ok(None)
}

fn find_and_parse_date(
    metadata: &HashMap<String, Value>,
    tag_name: &str,
    timezone_offset: Option<i32>,
) -> Result<Option<DateTime<Utc>>> {
    // Try different tag name formats
    let possible_keys = vec![
        tag_name.to_string(),
        format!("EXIF:{}", tag_name),
        format!("QuickTime:{}", tag_name),
        format!("XMP:{}", tag_name),
        format!("Composite:{}", tag_name),
        format!("File:{}", tag_name),
    ];

    for key in possible_keys {
        if let Some(value) = metadata.get(&key) {
            // Handle arrays (for Track/Media dates)
            if let Some(arr) = value.as_array() {
                let dates = parse_date_array(arr, timezone_offset)?;
                if !dates.is_empty() {
                    return Ok(Some(find_mode_or_earliest(dates)));
                }
            } else if let Some(s) = value.as_str() {
                if let Some(date) = parse_date_string(s, timezone_offset)? {
                    return Ok(Some(date));
                }
            }
        }
    }

    Ok(None)
}

fn parse_date_array(
    arr: &[Value],
    timezone_offset: Option<i32>,
) -> Result<Vec<DateTime<Utc>>> {
    let mut dates = Vec::new();

    for val in arr {
        if let Some(s) = val.as_str() {
            if let Some(date) = parse_date_string(s, timezone_offset)? {
                dates.push(date);
            }
        }
    }

    Ok(dates)
}

fn find_mode_or_earliest(dates: Vec<DateTime<Utc>>) -> DateTime<Utc> {
    let mut counts: HashMap<i64, usize> = HashMap::new();

    for date in &dates {
        *counts.entry(date.timestamp()).or_insert(0) += 1;
    }

    // Find the mode (most common)
    let max_count = counts.values().max().copied().unwrap_or(0);
    let modes: Vec<_> = dates
        .iter()
        .filter(|d| counts.get(&d.timestamp()) == Some(&max_count))
        .collect();

    // If there's a tie, return the earliest
    **modes.iter().min().unwrap()
}

fn parse_date_string(s: &str, timezone_offset: Option<i32>) -> Result<Option<DateTime<Utc>>> {
    // Try parsing with chrono
    // ExifTool date format: "YYYY:MM:DD HH:MM:SS" or "YYYY:MM:DD HH:MM:SS.SSS" or with timezone

    // Try with timezone first
    if let Ok(dt) = DateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S%z") {
        return Ok(Some(dt.with_timezone(&Utc)));
    }

    if let Ok(dt) = DateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S%.3f%z") {
        return Ok(Some(dt.with_timezone(&Utc)));
    }

    // Try without timezone
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S") {
        let dt = apply_timezone(naive, timezone_offset);
        return Ok(Some(dt));
    }

    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S%.3f") {
        let dt = apply_timezone(naive, timezone_offset);
        return Ok(Some(dt));
    }

    Ok(None)
}

fn apply_timezone(naive: NaiveDateTime, offset_seconds: Option<i32>) -> DateTime<Utc> {
    if let Some(offset) = offset_seconds {
        // Apply timezone offset
        let offset_duration = chrono::Duration::seconds(offset as i64);
        let utc = naive - offset_duration;
        Utc.from_utc_datetime(&utc)
    } else {
        // Assume UTC
        Utc.from_utc_datetime(&naive)
    }
}

fn extract_timezone_offset(metadata: &HashMap<String, Value>) -> Option<i32> {
    // Look for timezone offset tags
    let offset_tags = ["OffsetTime", "OffsetTimeOriginal", "OffsetTimeDigitized"];

    for tag in offset_tags {
        for key in &[tag.to_string(), format!("EXIF:{}", tag)] {
            if let Some(Value::String(s)) = metadata.get(key) {
                return parse_timezone_offset(s);
            }
        }
    }

    None
}

fn parse_timezone_offset(s: &str) -> Option<i32> {
    // Format: "+08:00" or "-05:00"
    if s.len() != 6 {
        return None;
    }

    let sign = if s.starts_with('+') { 1 } else { -1 };
    let hours: i32 = s[1..3].parse().ok()?;
    let minutes: i32 = s[4..6].parse().ok()?;

    Some(sign * (hours * 3600 + minutes * 60))
}

fn extract_date_from_user_comment(metadata: &HashMap<String, Value>) -> Result<Option<DateTime<Utc>>> {
    // Try to find UserComment field
    let possible_keys = vec!["UserComment", "EXIF:UserComment"];

    for key in possible_keys {
        if let Some(value) = metadata.get(key) {
            if let Some(s) = value.as_str() {
                // Try to parse as JSON
                if let Ok(json) = serde_json::from_str::<Value>(s) {
                    if let Some(date_str) = json.get("orgFileModifiedDate").and_then(|v| v.as_str()) {
                        // Parse the date string
                        if let Some(date) = parse_date_string(date_str, None)? {
                            return Ok(Some(date));
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

fn is_valid_date(date: DateTime<Utc>) -> bool {
    let now = Utc::now();
    let timestamp = date.timestamp();

    // Check if date is in the future
    if date > now {
        return false;
    }

    // Check if date is a rejected epoch
    for &epoch in REJECTED_EPOCHS {
        if (timestamp - epoch).abs() < 86400 {
            // Within a day of a rejected epoch
            return false;
        }
    }

    // Check if timestamp is zero
    if timestamp == 0 {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timezone_offset_parsing() {
        assert_eq!(parse_timezone_offset("+08:00"), Some(8 * 3600));
        assert_eq!(parse_timezone_offset("-05:00"), Some(-5 * 3600));
        assert_eq!(parse_timezone_offset("+00:00"), Some(0));
    }
}
