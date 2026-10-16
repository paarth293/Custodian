/// Status of Windows Sandbox availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxStatus {
    Available,
    NotInstalled,
    Disabled,
    Unavailable,
}

/// Reports whether Windows Sandbox is available.
///
/// F29/F31: Probes system feature or returns NotInstalled rather than constant `true`.
pub fn detect_sandbox() -> SandboxStatus {
    let wsb_exe = std::path::Path::new(r"C:\Windows\System32\WindowsSandbox.exe");
    if wsb_exe.exists() {
        SandboxStatus::Available
    } else {
        SandboxStatus::NotInstalled
    }
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
