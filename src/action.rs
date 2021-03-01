use crate::params::Parameters;

pub trait Action {
    fn apply(&self, params: &mut Parameters) -> bool;
}

pub struct ShutdownAction {}

impl Action for ShutdownAction {
    fn apply(&self, _params: &mut Parameters) -> bool {
        std::process::Command::new("poweroff")
            .spawn()
            .expect("Failed to shutdown the system");
        true
    }
}
