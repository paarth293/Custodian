use crate::router::{Role, Router};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("IPC route not permitted")]
    IpcRoute,
}

/// Moves parsed document excerpts from the parser to the agent's broker.
pub struct ExcerptPipeline {
    router: Router,
}

impl ExcerptPipeline {
    pub fn new(router: Router) -> Self {
        Self { router }
    }

    /// Routes a parsed excerpt through the IPC policy.
    ///
    /// Placeholder: the sandboxed `custodian-parser` invocation is not wired
    /// in yet, so the caller supplies the parsed content.
    pub fn process_excerpt(&self, content: &[u8]) -> Result<(), PipelineError> {
        self.router
            .route_frame(Role::Agent, Role::Broker, content)
            .map_err(|_| PipelineError::IpcRoute)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_excerpt() {
        let pipeline = ExcerptPipeline::new(Router::new());
        assert!(pipeline.process_excerpt(b"parsed content").is_ok());
    }
}
