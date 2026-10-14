//! Manual check for the settings catalogue.
//!
//! With no arguments, prints every setting Cubby knows, what this PC has it
//! set to, and the words it accepts. Read-only.
//!
//!   cargo run -p custodian-winsec --example settings
//!
//! With a setting and a value, it is a real acceptance test of the write path:
//! the value is read, changed, read back, and put back exactly as it was.
//!
//!   cargo run -p custodian-winsec --example settings -- show_file_extensions on
//!
//! Compare a line or two with Windows itself, for example:
//!   reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize" /v AppsUseLightTheme

use custodian_winsec::settings::{self, Change};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => list(),
        [id, value] => change(id, value),
        _ => {
            eprintln!("usage: settings [<setting> <value>]");
            std::process::exit(2);
        }
    }
}

fn list() {
    let mut category = None;
    for setting in settings::all() {
        if category != Some(setting.category) {
            category = Some(setting.category);
            println!("\n{}:", setting.category.label());
        }
        let now = match setting.current() {
            Some(choice) => choice.key.to_string(),
            None => "(Windows has no value)".to_string(),
        };
        println!(
            "  {:<22} {:<28} {:<24} now: {now}",
            setting.id,
            setting.label,
            format!("choose: {}", setting.options()),
        );
    }
    println!("\nReversible: every change records the value it replaced.");
}

/// Changes a setting, reads it back, and puts it back, so the whole write path
/// is exercised on a real PC without leaving anything changed.
fn change(id: &str, value: &str) {
    let Some(setting) = settings::find(id) else {
        eprintln!("no setting called \"{id}\"; run with no arguments to list them");
        std::process::exit(2);
    };
    let before = setting.current();
    println!(
        "{} is {}",
        setting.label,
        before.map_or("unset".to_string(), |c| c.key.to_string())
    );

    let change = match setting.plan(value) {
        Ok(change) => change,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    };
    println!("changing to {}…", change.value);
    if let Err(err) = setting.apply(&change.value) {
        eprintln!("could not change it: {err}");
        std::process::exit(1);
    }
    match setting.current() {
        Some(now) if now.key == change.value => println!("  read back: {} ✓", now.key),
        other => {
            eprintln!("  read back: {other:?} — the change did not take");
            revert(&change);
            std::process::exit(1);
        }
    }

    println!("putting it back to {}…", before.map_or("unset", |c| c.key));
    revert(&change);

    let after = setting.current();
    let expected = before.map(|c| c.key);
    if after.map(|c| c.key) == expected {
        println!(
            "  read back: {} ✓ nothing left changed",
            after.map_or("unset".to_string(), |c| c.key.to_string())
        );
    } else {
        eprintln!("  read back: {after:?} — expected {expected:?}");
        std::process::exit(1);
    }
}

/// Puts back what was there: the old value, or removed when Windows had none.
fn revert(change: &Change) {
    match settings::revert(change) {
        Ok(()) => {}
        Err(err) => {
            eprintln!("could not put it back: {err}");
            std::process::exit(1);
        }
    }
}
