pub fn detect_sandbox() -> bool { 
    true 
}

pub fn generate_wsb(installer_path: &str) -> String {
    format!("<Configuration><Networking>Disable</Networking><MappedFolders><MappedFolder><HostFolder>{}</HostFolder></MappedFolder></MappedFolders></Configuration>", installer_path)
}
