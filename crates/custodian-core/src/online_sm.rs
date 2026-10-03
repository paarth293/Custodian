#[derive(Debug, PartialEq)]
pub enum OnlineState {
    Offline,
    Enabling,
    Online,
}

pub struct OnlineStateMachine {
    pub state: OnlineState,
}

impl OnlineStateMachine {
    pub fn new() -> Self { Self { state: OnlineState::Offline } }
    
    pub fn go_online(&mut self, hello_verified: bool) {
        if hello_verified {
            self.state = OnlineState::Online;
        }
    }
    
    pub fn go_offline(&mut self) {
        self.state = OnlineState::Offline;
    }
}
