use clap::Parser;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    persona: String,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    seed: u64,
}

fn main() {
    let args = Args::parse();
    
    // Create output directory
    fs::create_dir_all(&args.out).expect("Failed to create out dir");
    
    // Write marker file
    let marker_path = args.out.join(".custodian-fixture-root");
    let mut marker = File::create(marker_path).expect("Failed to create marker");
    let id = Uuid::new_v4();
    writeln!(marker, "seed: {}\nid: {}", args.seed, id).unwrap();
    
    // Create dummy trees
    let folders = ["Desktop", "Documents", "Downloads", "Pictures", "node_modules", "target"];
    for folder in folders.iter() {
        fs::create_dir_all(args.out.join(folder)).unwrap();
    }
    
    // Exact duplicate
    let dup_content = "identical content";
    fs::write(args.out.join("Documents/file1.txt"), dup_content).unwrap();
    fs::write(args.out.join("Documents/file2.txt"), dup_content).unwrap();
    
    // Near duplicate
    fs::write(args.out.join("Documents/near1.txt"), "almost identical content A").unwrap();
    fs::write(args.out.join("Documents/near2.txt"), "almost identical content B").unwrap();
    
    println!("Generated fixture for {} at {:?}", args.persona, args.out);
}
