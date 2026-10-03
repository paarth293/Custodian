//! Generates a synthetic user-profile tree for tests and demos.
//!
//! Every generated tree contains a `.custodian-fixture-root` marker, which the
//! test kits require before they will operate on a directory.

use clap::Parser;
use std::fs;
use std::io;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Persona the fixture represents (recorded in the marker file).
    #[arg(long)]
    persona: String,
    /// Output directory; created if missing.
    #[arg(long)]
    out: PathBuf,
    /// Seed recorded in the marker file for reproducibility.
    #[arg(long, default_value_t = 0)]
    seed: u64,
}

const FOLDERS: &[&str] = &[
    "Desktop",
    "Documents",
    "Downloads",
    "Pictures",
    "node_modules",
    "target",
];

fn main() -> io::Result<()> {
    let args = Args::parse();
    fs::create_dir_all(&args.out)?;

    fs::write(
        args.out.join(".custodian-fixture-root"),
        format!(
            "persona: {}\nseed: {}\nid: {}\n",
            args.persona,
            args.seed,
            Uuid::new_v4()
        ),
    )?;

    for folder in FOLDERS {
        fs::create_dir_all(args.out.join(folder))?;
    }

    let docs = args.out.join("Documents");
    // Exact duplicates.
    fs::write(docs.join("file1.txt"), "identical content")?;
    fs::write(docs.join("file2.txt"), "identical content")?;
    // Near duplicates.
    fs::write(docs.join("near1.txt"), "almost identical content A")?;
    fs::write(docs.join("near2.txt"), "almost identical content B")?;

    println!(
        "Generated fixture for {} at {}",
        args.persona,
        args.out.display()
    );
    Ok(())
}
