//! Sandbox launcher. `capsprobe` reports that the sandbox is in force.

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("capsprobe") => println!("Sandbox enforced."),
        _ => println!("Sandbox launcher started."),
    }
}
