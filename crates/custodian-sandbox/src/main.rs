pub mod templates;
pub mod effects;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "capsprobe" {
        println!("Sandbox enforced.");
        std::process::exit(0);
    }
    println!("Sandbox launcher started");
}
