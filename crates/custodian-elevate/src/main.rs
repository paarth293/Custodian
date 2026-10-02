use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && args[1] == "--register-engine" {
        println!("Writing to HKLM\\SOFTWARE\\Custodian (simulated UAC elevation)");
    } else {
        println!("Usage: custodian-elevate --register-engine");
    }
}
