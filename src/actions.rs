pub struct Parameters {}

pub trait Action {
    fn execute(&mut self, params: &Parameters) -> bool;
}

struct ShutdownAction {}

impl Action for ShutdownAction {
    fn execute(&mut self, params: &Parameters) -> bool {
        std::proccess::Command::new("poweroff")
            .spawn()
            .expect("Failed to shutdown the system");
    }
}
