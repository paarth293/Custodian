use custodian_core::sandbox_factory::SandboxFactory;
use custodian_core::supervisor::Supervisor;
use std::path::Path;

fn main() {
    println!("Spike F: Core sandbox bootstrap.");
    let mut sup = Supervisor::new();
    let factory = SandboxFactory::new("custodian_spike", 512, 10);
    
    let original_exe = Path::new("C:\\Windows\\System32\\cmd.exe");
    let target_exe = Path::new("C:\\Windows\\Temp\\custodian-agent-mock.exe");
    
    if let Err(e) = factory.spawn_agent(original_exe, target_exe) {
        println!("Mock spawn failed safely: {}", e);
    }
    
    sup.add_child("Agent", target_exe.to_str().unwrap(), &["/c", "exit 0"]);
    
    println!("Spike F completed.");
}
