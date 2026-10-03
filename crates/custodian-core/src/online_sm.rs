#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnlineState {
    Offline,
    Online,
}

/// Gatekeeper for online mode: going online requires a verified Windows Hello
/// prompt; going offline is always allowed and tears down the broker.
#[derive(Debug)]
pub struct OnlineStateMachine {
    state: OnlineState,
}

impl Default for OnlineStateMachine {
    fn default() -> Self {
        Self {
            state: OnlineState::Offline,
        }
    }
}

impl OnlineStateMachine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> OnlineState {
        self.state
    }

    pub fn go_online(&mut self, hello_verified: bool) {
        if hello_verified {
            self.state = OnlineState::Online;
        }
    }

    pub fn go_offline(&mut self) {
        self.state = OnlineState::Offline;
        Self::kill_broker_job();
    }

    /// Placeholder for `TerminateJobObject` on the broker's job.
    fn kill_broker_job() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_hello_to_go_online() {
        let mut sm = OnlineStateMachine::new();
        sm.go_online(false);
        assert_eq!(sm.state(), OnlineState::Offline);
        sm.go_online(true);
        assert_eq!(sm.state(), OnlineState::Online);
        sm.go_offline();
        assert_eq!(sm.state(), OnlineState::Offline);
    }
}
