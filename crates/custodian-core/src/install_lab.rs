/// Reports whether Windows Sandbox is available.
///
/// Placeholder: always `true` until the optional-feature probe lands.
pub fn detect_sandbox() -> bool {
    true
}

/// Builds a Windows Sandbox (`.wsb`) configuration that maps the installer
/// folder read-only and disables networking.
pub fn generate_wsb(installer_dir: &str) -> String {
    format!(
        "<Configuration>\
         <Networking>Disable</Networking>\
         <MappedFolders><MappedFolder>\
         <HostFolder>{}</HostFolder>\
         <ReadOnly>true</ReadOnly>\
         </MappedFolder></MappedFolders>\
         </Configuration>",
        xml_escape(installer_dir)
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wsb_disables_networking_and_escapes_path() {
        let wsb = generate_wsb(r"C:\a&b\<x>");
        assert!(wsb.contains("<Networking>Disable</Networking>"));
        assert!(wsb.contains(r"<HostFolder>C:\a&amp;b\&lt;x&gt;</HostFolder>"));
        assert!(wsb.contains("<ReadOnly>true</ReadOnly>"));
    }
}
