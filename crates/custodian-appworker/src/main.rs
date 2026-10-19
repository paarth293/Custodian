fn main() {
    if custodian_appworker::serve_once(std::io::stdin().lock(), std::io::stdout().lock()).is_err() {
        // No request, document text, paths or key material in diagnostics.
        eprintln!("Application worker request unavailable or invalid.");
        std::process::exit(1);
    }
}
