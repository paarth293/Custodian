//! Shows the native approval dialog and prints the answer (manual check).

fn main() {
    let yes = custodian_winsec::dialog::confirm(
        "Approve clean-up",
        "Move 2 duplicate copies (8.0 KB) to Cubby's safe place?",
    );
    println!("confirmed={yes}");
}
