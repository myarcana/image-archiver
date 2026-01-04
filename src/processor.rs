use anyhow::{Context, Result};
use crossbeam_channel::{bounded, Sender, Receiver};
use exiftool::ExifTool;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use walkdir::WalkDir;

use crate::failed::handle_failed_file;
use crate::filename::{generate_filename, generate_filename_without_counter, get_extension};
use crate::metadata::{extract_dates_batch, MediaDates};

const INITIAL_BATCH_SIZE: usize = 50;
const BATCH_SIZE_INCREMENT: usize = 10;
const MAX_BATCH_SIZE: usize = 1000;

/// Check if two paths are on the same filesystem volume
fn is_same_volume(path1: &Path, path2: &Path) -> Result<bool> {
    let meta1 = fs::metadata(path1)
        .with_context(|| format!("Failed to get metadata for {}", path1.display()))?;
    let meta2 = fs::metadata(path2)
        .with_context(|| format!("Failed to get metadata for {}", path2.display()))?;

    // Compare device IDs (st_dev on Unix)
    Ok(meta1.dev() == meta2.dev())
}

pub struct Processor {
    output_dir: PathBuf,
    failed_cases_dir: PathBuf,
    stats: Arc<Mutex<ProcessingStats>>,
}

#[derive(Debug, Default)]
pub struct ProcessingStats {
    pub total_files: usize,
    pub moved: usize,
    pub copied: usize,
    pub skipped: usize,
    pub failed: usize,
    pub duplicates: Vec<(PathBuf, PathBuf)>, // (source_path, destination_path)
}

/// Work item sent to worker threads
type WorkItem = (PathBuf, bool); // (file_path, should_move)

/// Result sent back from worker threads
#[derive(Debug)]
struct WorkerResult {
    original_path: PathBuf,
    result: Result<ProcessedFile>,
}

#[derive(Debug)]
struct ProcessedFile {
    dates: MediaDates,
    extension: String,
    should_move: bool,
}

impl Processor {
    pub fn new(output_dir: PathBuf) -> Result<Self> {
        // Create output directory if it doesn't exist
        fs::create_dir_all(&output_dir)
            .with_context(|| format!("Failed to create output directory: {}", output_dir.display()))?;

        // Create "Failed Cases" directory
        let failed_cases_dir = output_dir.join("Failed Cases");
        fs::create_dir_all(&failed_cases_dir)
            .with_context(|| format!("Failed to create failed cases directory: {}", failed_cases_dir.display()))?;

        Ok(Processor {
            output_dir,
            failed_cases_dir,
            stats: Arc::new(Mutex::new(ProcessingStats::default())),
        })
    }

    pub fn process_directories(&mut self, input_dirs: &[PathBuf]) -> Result<()> {
        println!("Starting media collection...");
        println!("Output directory: {}", self.output_dir.display());
        println!();

        // Collect all files from all directories upfront
        let mut all_files = Vec::new();
        for input_dir in input_dirs {
            println!("Scanning directory: {}", input_dir.display());
            let files = self.collect_files(input_dir)?;
            all_files.extend(files);
        }

        let total_files = all_files.len();
        {
            let mut stats = self.stats.lock().unwrap();
            stats.total_files = total_files;
        }
        println!("Found {} files to process", total_files);
        println!();

        if total_files == 0 {
            self.print_summary();
            return Ok(());
        }

        // Process files in parallel
        self.process_files_parallel(all_files)?;

        self.print_summary();
        Ok(())
    }

    fn collect_files(&self, dir: &Path) -> Result<Vec<WorkItem>> {
        // Check if this directory is on the same volume as the output
        let same_volume = is_same_volume(dir, &self.output_dir).unwrap_or(false);

        if same_volume {
            println!("  → Same volume detected, files will be moved (not copied)");
        }

        let mut files = Vec::new();

        for entry_result in WalkDir::new(dir)
            .max_depth(1)
            .min_depth(1)
            .into_iter()
        {
            let entry = match entry_result {
                Ok(e) => e,
                Err(err) => {
                    // Handle WalkDir errors
                    if let Some(path) = err.path() {
                        eprintln!("Warning: Failed to access {}: {}", path.display(), err);
                    } else {
                        eprintln!("Warning: WalkDir error: {}", err);
                    }
                    continue;
                }
            };

            let path = entry.path();

            // Skip if not a file
            if !path.is_file() {
                continue;
            }

            // Get filename for filtering
            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Skip AppleDouble files (._*)
            if filename.starts_with("._") {
                continue;
            }

            // Skip .DS_Store files (macOS metadata)
            if filename == ".DS_Store" {
                continue;
            }

            // Skip AAE files (Apple's sidecar files for edits)
            if let Some(ext) = path.extension() {
                if ext.eq_ignore_ascii_case("aae") {
                    continue;
                }
            }

            files.push((path.to_path_buf(), same_volume));
        }

        Ok(files)
    }

    fn process_files_parallel(&self, files: Vec<WorkItem>) -> Result<()> {
        // Determine number of worker threads (CPU cores / 2)
        let num_workers = (num_cpus::get() / 2).max(1);
        println!("Starting {} worker threads", num_workers);

        // Create channels
        let (work_sender, work_receiver) = bounded::<WorkItem>(num_workers * 2);
        let (result_sender, result_receiver) = bounded::<WorkerResult>(num_workers * 2);

        // Spawn worker threads
        let mut worker_handles = Vec::new();
        for worker_id in 0..num_workers {
            let work_rx = work_receiver.clone();
            let result_tx = result_sender.clone();

            let handle = thread::spawn(move || {
                worker_thread(worker_id, work_rx, result_tx);
            });

            worker_handles.push(handle);
        }

        // Drop our copies of the channels
        drop(work_receiver);
        drop(result_sender);

        // Send all work items to workers
        let total_files = files.len();
        thread::spawn(move || {
            for work_item in files {
                if work_sender.send(work_item).is_err() {
                    break; // Workers have shut down
                }
            }
            // Channel closes when work_sender is dropped
        });

        // Process results from workers
        let mut processed = 0;

        for worker_result in result_receiver {
            processed += 1;
            if processed % 100 == 0 {
                println!("Progress: {}/{} files processed", processed, total_files);
            }

            self.handle_worker_result(worker_result);
        }

        // Wait for all workers to finish
        for handle in worker_handles {
            let _ = handle.join();
        }

        Ok(())
    }

    fn handle_worker_result(
        &self,
        worker_result: WorkerResult,
    ) {
        let WorkerResult { original_path, result } = worker_result;

        match result {
            Ok(processed) => {
                // Worker successfully extracted metadata
                let ProcessedFile { dates, extension, should_move } = processed;

                // Read source file content
                let content = match fs::read(&original_path) {
                    Ok(c) => c,
                    Err(e) => {
                        let mut stats = self.stats.lock().unwrap();
                        stats.failed += 1;
                        let err = anyhow::anyhow!("Failed to read file: {}", e);
                        if let Err(handle_err) = handle_failed_file(&original_path, &self.failed_cases_dir, &err) {
                            eprintln!("Error handling failed file: {}", handle_err);
                        }
                        return;
                    }
                };

                // Check existing files on disk starting from counter 1
                let mut check_counter = 1;
                let mut found_duplicate = false;

                loop {
                    let check_filename = generate_filename(&dates, &extension, check_counter);
                    let check_path = self.output_dir.join(&check_filename);

                    if !check_path.exists() {
                        // File doesn't exist - this is the counter to use
                        // No need to check higher counters (they won't exist either)
                        break;
                    }

                    // File exists, check if it's a duplicate
                    match fs::read(&check_path) {
                        Ok(existing_content) => {
                            if existing_content == content {
                                // Duplicate found! Skip this file
                                found_duplicate = true;
                                let mut stats = self.stats.lock().unwrap();
                                stats.skipped += 1;
                                stats.duplicates.push((original_path.clone(), check_path.clone()));
                                println!("- Skipped (already exists): {}", original_path.display());
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("Warning: failed to read {}: {}", check_path.display(), e);
                        }
                    }

                    // Not a duplicate, increment and check next counter
                    check_counter += 1;

                    if check_counter > 10000 {
                        // Safety limit
                        let mut stats = self.stats.lock().unwrap();
                        stats.failed += 1;
                        let err = anyhow::anyhow!("Too many filename collisions for the same date pair");
                        if let Err(handle_err) = handle_failed_file(&original_path, &self.failed_cases_dir, &err) {
                            eprintln!("Error handling failed file: {}", handle_err);
                        }
                        return;
                    }
                }

                // If not a duplicate, transfer the file
                if !found_duplicate {
                    match self.transfer_file(&original_path, &dates, &extension, check_counter, should_move, &content) {
                        Ok(ProcessResult::Moved) => {
                            let mut stats = self.stats.lock().unwrap();
                            stats.moved += 1;
                            println!("✓ Moved: {}", original_path.display());
                        }
                        Ok(ProcessResult::Copied) => {
                            let mut stats = self.stats.lock().unwrap();
                            stats.copied += 1;
                            println!("✓ Copied: {}", original_path.display());
                        }
                        Ok(ProcessResult::Skipped(dest_path)) => {
                            let mut stats = self.stats.lock().unwrap();
                            stats.skipped += 1;
                            stats.duplicates.push((original_path.clone(), dest_path));
                            println!("- Skipped (already exists): {}", original_path.display());
                        }
                        Err(e) => {
                            let mut stats = self.stats.lock().unwrap();
                            stats.failed += 1;
                            if let Err(handle_err) = handle_failed_file(&original_path, &self.failed_cases_dir, &e) {
                                eprintln!("Error handling failed file: {}", handle_err);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                // Worker failed to extract metadata
                let mut stats = self.stats.lock().unwrap();
                stats.failed += 1;
                if let Err(handle_err) = handle_failed_file(&original_path, &self.failed_cases_dir, &e) {
                    eprintln!("Error handling failed file: {}", handle_err);
                }
            }
        }
    }

    fn transfer_file(
        &self,
        file_path: &Path,
        dates: &MediaDates,
        extension: &str,
        counter: u32,
        should_move: bool,
        content: &[u8],
    ) -> Result<ProcessResult> {
        // Generate target filename with counter
        let filename = generate_filename(dates, extension, counter);
        let target_path = self.output_dir.join(&filename);

        // File shouldn't exist at this point since we already checked
        // But double-check just in case
        if target_path.exists() {
            let existing_content = fs::read(&target_path)
                .with_context(|| format!("Failed to read existing file: {}", target_path.display()))?;

            if existing_content == content {
                return Ok(ProcessResult::Skipped(target_path));
            }
        }

        // Transfer file to destination (move or copy depending on volume)
        if should_move {
            // Use rename for same-volume transfers (fast, atomic)
            fs::rename(file_path, &target_path)
                .with_context(|| format!("Failed to move file to {}", target_path.display()))?;
            Ok(ProcessResult::Moved)
        } else {
            // Use copy for cross-volume transfers
            fs::copy(file_path, &target_path)
                .with_context(|| format!("Failed to copy file to {}", target_path.display()))?;

            // Delete source file after successful copy
            fs::remove_file(file_path)
                .with_context(|| format!("Failed to delete source file after copy: {}", file_path.display()))?;

            Ok(ProcessResult::Copied)
        }
    }

    fn print_summary(&self) {
        let stats = self.stats.lock().unwrap();

        println!();
        println!("=== PROCESSING COMPLETE ===");
        println!("Total files scanned: {}", stats.total_files);

        let total_processed = stats.moved + stats.copied;
        println!("Successfully processed: {}", total_processed);

        if stats.moved > 0 {
            println!("  - Moved (same volume): {}", stats.moved);
        }
        if stats.copied > 0 {
            println!("  - Copied (cross volume): {}", stats.copied);
        }

        println!("Skipped (already exist): {}", stats.skipped);
        println!("Failed: {}", stats.failed);

        if stats.failed > 0 {
            println!();
            println!(
                "Failed cases have been logged in: {}",
                self.failed_cases_dir.display()
            );
        }

        // Handle duplicates cleanup
        if !stats.duplicates.is_empty() {
            println!();
            println!("=== DUPLICATE FILES ===");
            println!();

            // Calculate total size
            let mut total_size: u64 = 0;
            for (source, _) in &stats.duplicates {
                if let Ok(metadata) = fs::metadata(source) {
                    total_size += metadata.len();
                }
            }

            // Display each duplicate with its match
            for (source, dest) in &stats.duplicates {
                println!("Source: {}", source.display());
                println!("   → Duplicate of: {}", dest.display());
                println!();
            }

            // Show summary
            let size_mb = total_size as f64 / 1_048_576.0;
            println!("Total: {} duplicates ({:.2} MB)", stats.duplicates.len(), size_mb);
            println!();

            // We need to drop the lock before prompting for input
            // Clone the duplicates list so we can use it after dropping the lock
            let duplicates = stats.duplicates.clone();
            drop(stats);

            // Prompt for confirmation
            print!("Delete these {} duplicate source files? (y/n): ", duplicates.len());
            io::stdout().flush().unwrap();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_ok() {
                let input = input.trim().to_lowercase();
                if input == "y" || input == "yes" {
                    println!();
                    println!("Deleting duplicate source files...");
                    let mut deleted = 0;
                    let mut failed = 0;

                    for (source, _) in &duplicates {
                        match fs::remove_file(source) {
                            Ok(_) => {
                                deleted += 1;
                                println!("✓ Deleted: {}", source.display());
                            }
                            Err(e) => {
                                failed += 1;
                                eprintln!("✗ Failed to delete {}: {}", source.display(), e);
                            }
                        }
                    }

                    println!();
                    println!("Cleanup complete: {} deleted, {} failed", deleted, failed);
                } else {
                    println!();
                    println!("Duplicate source files were not deleted.");
                }
            }
        }
    }
}

enum ProcessResult {
    Moved,
    Copied,
    Skipped(PathBuf), // Contains the destination path it's a duplicate of
}

/// Worker thread function
fn worker_thread(
    worker_id: usize,
    work_receiver: Receiver<WorkItem>,
    result_sender: Sender<WorkerResult>,
) {
    // Create ExifTool instance for this worker
    let mut exiftool = match ExifTool::new() {
        Ok(tool) => tool,
        Err(e) => {
            eprintln!("Worker {}: Failed to initialize ExifTool: {}", worker_id, e);
            return;
        }
    };

    // Process work items in batches with progressive sizing
    let mut batch = Vec::new();
    let mut batch_info = Vec::new(); // Store (path, should_move) tuples
    let mut current_batch_size = INITIAL_BATCH_SIZE; // Start at 50

    for (file_path, should_move) in work_receiver {
        batch.push(file_path.clone());
        batch_info.push((file_path, should_move));

        if batch.len() >= current_batch_size {
            process_batch(&mut exiftool, &batch, &batch_info, &result_sender);
            batch.clear();
            batch_info.clear();

            // Grow batch size: 50 → 60 → 70 → ... → MAX_BATCH_SIZE
            current_batch_size = (current_batch_size + BATCH_SIZE_INCREMENT).min(MAX_BATCH_SIZE);
        }
    }

    // Process remaining files in the last batch
    if !batch.is_empty() {
        process_batch(&mut exiftool, &batch, &batch_info, &result_sender);
    }
}

fn process_batch(
    exiftool: &mut ExifTool,
    batch: &[PathBuf],
    batch_info: &[(PathBuf, bool)],
    result_sender: &Sender<WorkerResult>,
) {
    // Extract metadata for all files in batch
    let metadata_results = extract_dates_batch(exiftool, batch);

    // Process each file with its metadata
    for (file_path, should_move) in batch_info {
        let dates_result = metadata_results.get(file_path);

        let result = match dates_result {
            Some(Ok(dates)) => {
                // We have metadata, extract extension
                match get_extension(file_path) {
                    Some(extension) => Ok(ProcessedFile {
                        dates: dates.clone(),
                        extension,
                        should_move: *should_move,
                    }),
                    None => Err(anyhow::anyhow!("File has no extension")),
                }
            }
            Some(Err(e)) => {
                // Metadata extraction failed
                Err(anyhow::anyhow!("{}", e))
            }
            None => {
                // Shouldn't happen, but handle gracefully
                Err(anyhow::anyhow!("No metadata result for file"))
            }
        };

        let worker_result = WorkerResult {
            original_path: file_path.clone(),
            result,
        };

        // Send result back to main thread
        if result_sender.send(worker_result).is_err() {
            break; // Main thread has shut down
        }
    }
}
