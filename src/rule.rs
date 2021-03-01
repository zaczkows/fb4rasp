use crate::action::Action;
use crate::condition::Condition;
use crate::params::Parameters;

pub trait Rule {
    fn check(&self, touch: &adafruit_mpr121::Mpr121TouchStatus) -> bool;
}

pub struct AndRule {
    conditions: Vec<Box<dyn Condition>>,
    actions: Vec<Box<dyn Action>>,
}

impl AndRule {
    pub fn new() -> Self {
        Self {
            conditions: Vec::new(),
            actions: Vec::new(),
        }
    }

    pub fn add_condition(&mut self, condition: Box<dyn Condition>) -> bool {
        self.conditions.push(condition);
        true
    }

    pub fn add_action(&mut self, action: Box<dyn Action>) -> bool {
        self.actions.push(action);
        true
    }
}

impl Rule for AndRule {
    fn check(&self, touch: &adafruit_mpr121::Mpr121TouchStatus) -> bool {
        for c in &self.conditions {
            if !c.applies(touch) {
                return false;
            }
        }

        let mut params = Parameters::new();
        for a in &self.actions {
            a.apply(&mut params);
        }

        true
    }
}
