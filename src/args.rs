use anyhow::{anyhow, bail, Result};
use std::path::PathBuf;

#[derive(Debug)]
pub struct Args {
    pub input_dirs: Vec<PathBuf>,
    pub output_dir: PathBuf,
}

impl Args {
    /// Parse and validate command line arguments
    pub fn parse() -> Result<Self> {
        let args: Vec<String> = std::env::args().collect();

        if args.len() < 3 {
            bail!("Usage: collect_media <dirs...> -o <output_dir>\n\nExample:\n  collect_media /Volumes/Thumb/One /Volumes/Thumb/Two -o /Users/me/Pictures/Library");
        }

        let mut output_dir: Option<PathBuf> = None;
        let mut input_dirs: Vec<PathBuf> = Vec::new();
        let mut i = 1; // Skip program name

        // Check if output flag is first
        if args[i] == "-o" || args[i] == "--output-directory" || args[i] == "--output-dir" {
            if i + 1 >= args.len() {
                bail!("Output directory flag provided but no directory specified");
            }
            output_dir = Some(PathBuf::from(&args[i + 1]));
            i += 2;

            // Collect remaining args as input directories
            while i < args.len() {
                input_dirs.push(PathBuf::from(&args[i]));
                i += 1;
            }
        } else {
            // Output flag must be last
            // Collect input directories until we hit the output flag
            while i < args.len() {
                let arg = &args[i];
                if arg == "-o" || arg == "--output-directory" || arg == "--output-dir" {
                    if i + 1 >= args.len() {
                        bail!("Output directory flag provided but no directory specified");
                    }
                    output_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                    break;
                }
                input_dirs.push(PathBuf::from(arg));
                i += 1;
            }

            // Check if there are any arguments after the output directory
            if i < args.len() {
                bail!("Output directory flag must be either first or last in the argument list");
            }
        }

        let output_dir = output_dir
            .ok_or_else(|| anyhow!("Output directory must be specified with -o, --output-directory, or --output-dir"))?;

        if input_dirs.is_empty() {
            bail!("At least one input directory must be specified");
        }

        // Validate input directories exist and are directories
        for dir in &input_dirs {
            if !dir.exists() {
                bail!("Input directory does not exist: {}", dir.display());
            }
            if !dir.is_dir() {
                bail!("Input path is not a directory: {}", dir.display());
            }
        }

        Ok(Args {
            input_dirs,
            output_dir,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arg_parsing_logic() {
        // Note: These tests would need to mock std::env::args
        // For now, they serve as documentation of expected behavior
    }
}
