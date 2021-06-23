use crate::params::Parameters;

pub trait Action {
    fn apply(&self, params: &mut Parameters) -> bool;
}
