//! Shows a real Windows Hello prompt and prints the outcome (manual check).
//! Run with `cargo run -p custodian-winsec --example hello`.

fn main() {
    println!("available={}", custodian_winsec::hello::available());
    let outcome = custodian_winsec::hello::verify(0, "Custodian test: verify, or cancel.");
    println!("outcome={outcome:?}");
}
