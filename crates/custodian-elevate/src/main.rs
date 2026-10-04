//! Elevation helper. Accepts exactly one verb from a fixed allow-list so the
//! elevated surface stays as small as possible.
//!
//! Exit codes: 0 success, 1 usage error, 2 unknown verb.

use std::process::ExitCode;

const VERBS: &[&str] = &[
    "register_engine_key",
    "enable_system_restore",
    "create_restore_point",
    "run_verified_installer",
];

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(verb), None) = (args.next(), args.next()) else {
        eprintln!("usage: custodian-elevate <{}>", VERBS.join("|"));
        return ExitCode::from(1);
    };

    if !VERBS.contains(&verb.as_str()) {
        eprintln!("unknown verb: {verb}");
        return ExitCode::from(2);
    }

    // Placeholder: the privileged implementations are not wired in yet.
    println!("{verb}: ok");
    ExitCode::SUCCESS
}
