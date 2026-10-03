fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        std::process::exit(1);
    }
    let verb = &args[1];
    match verb.as_str() {
        "register_engine_key" => println!("Key registered."),
        "enable_system_restore" => println!("Restore enabled."),
        "create_restore_point" => println!("Point created."),
        "run_verified_installer" => println!("Installer run."),
        _ => std::process::exit(2),
    }
}
