use std::collections::HashSet;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Role {
    Agent,
    Engine,
    Sandbox,
    Broker,
}

/// Deny-by-default allow-list of IPC routes between roles.
pub struct Router {
    allowed_routes: HashSet<(Role, Role)>,
}

impl Default for Router {
    fn default() -> Self {
        Self {
            allowed_routes: HashSet::from([
                (Role::Agent, Role::Broker),
                (Role::Broker, Role::Agent),
            ]),
        }
    }
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn route_frame(&self, from: Role, to: Role, _payload: &[u8]) -> Result<(), &'static str> {
        if self.allowed_routes.contains(&(from, to)) {
            Ok(())
        } else {
            Err("Route not permitted by policy")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_policy() {
        let router = Router::new();
        assert!(router
            .route_frame(Role::Agent, Role::Broker, b"data")
            .is_ok());
        assert!(router
            .route_frame(Role::Agent, Role::Sandbox, b"data")
            .is_err());
        assert!(router
            .route_frame(Role::Agent, Role::Engine, b"data")
            .is_err());
    }
}
