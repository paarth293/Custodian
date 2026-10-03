use crate::router::{Router, Role};
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("Parse failed")]
    Parse,
    #[error("IPC Routing failed")]
    IpcRoute,
}

pub struct ExcerptPipeline {
    router: Router,
}

impl ExcerptPipeline {
    pub fn new(router: Router) -> Self {
        Self { router }
    }

    pub fn process_file(&self, path: &str) -> Result<(), PipelineError> {
        let _parser_output = Command::new("custodian-parser")
            .arg(path)
            .output();
            
        let content = b"mock_parsed_content";
        
        self.router.route_frame(Role::Agent, Role::Broker, content).map_err(|_| PipelineError::IpcRoute)?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_flow() {
        let router = Router::new();
        let pipeline = ExcerptPipeline::new(router);
        
        let result = pipeline.process_file("dummy.txt");
        assert!(result.is_ok());
    }
}
