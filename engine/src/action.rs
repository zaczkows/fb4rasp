use crate::params::Parameters;

pub trait Action {
    fn apply(&mut self, params: &mut Parameters) -> bool;
}
