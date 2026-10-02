use std::collections::HashSet;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Role {
    Agent,
    Engine,
    Sandbox,
    Broker,
}

pub struct Router {
    allowed_routes: HashSet<(Role, Role)>,
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Router {
    pub fn new() -> Self {
        let mut allowed = HashSet::new();
        allowed.insert((Role::Agent, Role::Broker));
        allowed.insert((Role::Broker, Role::Agent));
        
        Self {
            allowed_routes: allowed,
        }
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
    fn test_routing_policy() {
        let router = Router::new();
        assert!(router.route_frame(Role::Agent, Role::Broker, b"data").is_ok());
        assert!(router.route_frame(Role::Agent, Role::Sandbox, b"data").is_err());
    }
}
