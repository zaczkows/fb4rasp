use crate::params::Parameters;

pub trait Action {
    fn apply(&mut self, params: &Parameters) -> bool;
}

pub struct ShutdownAction {}

impl Action for ShutdownAction {
    fn apply(&mut self, _params: &Parameters) -> bool {
        std::process::Command::new("poweroff")
            .spawn()
            .expect("Failed to shutdown the system");
        true
    }
}
