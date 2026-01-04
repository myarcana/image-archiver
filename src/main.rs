use collect_media::args::Args;
use collect_media::processor::Processor;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    // Parse command line arguments
    let args = Args::parse()?;

    // Create processor
    let mut processor = Processor::new(args.output_dir)?;

    // Process all input directories
    processor.process_directories(&args.input_dirs)?;

    Ok(())
}
