use crate::action::Action;
use crate::condition::Condition;
use crate::params::Parameters;

pub trait Rule {
    fn check(&self, params: &mut Parameters) -> bool;
    fn apply(&mut self, params: &mut Parameters) -> bool;
}

#[derive(Default)]
pub struct AndRule {
    conditions: Vec<Box<dyn Condition + Send>>,
    actions: Vec<Box<dyn Action + Send>>,
}

impl AndRule {
    pub fn add_condition(&mut self, condition: Box<dyn Condition + Send>) -> bool {
        self.conditions.push(condition);
        true
    }

    pub fn add_action(&mut self, action: Box<dyn Action + Send>) -> bool {
        self.actions.push(action);
        true
    }
}

impl Rule for AndRule {
    fn check(&self, params: &mut Parameters) -> bool {
        let touch = &params.touch_data.last().unwrap();
        for c in &self.conditions {
            if !c.applies(touch) {
                return false;
            }
        }

        true
    }

    fn apply(&mut self, params: &mut Parameters) -> bool {
        for a in &mut self.actions {
            a.apply(params);
        }

        true
    }
}

#[derive(Default)]
pub struct OrRule {
    conditions: Vec<Box<dyn Condition + Send>>,
    actions: Vec<Box<dyn Action + Send>>,
}

impl OrRule {
    pub fn add_condition(&mut self, condition: Box<dyn Condition + Send>) -> bool {
        self.conditions.push(condition);
        true
    }

    pub fn add_action(&mut self, action: Box<dyn Action + Send>) -> bool {
        self.actions.push(action);
        true
    }
}

impl Rule for OrRule {
    fn check(&self, params: &mut Parameters) -> bool {
        let touch = &params.touch_data.last().unwrap();
        let mut applies = false;
        for c in &self.conditions {
            if c.applies(touch) {
                applies = true;
                break;
            }
        }

        applies
    }

    fn apply(&mut self, params: &mut Parameters) -> bool {
        for a in &mut self.actions {
            a.apply(params);
        }

        true
    }
}

pub struct SimpleRule {
    condition: Box<dyn Condition + Send>,
    action: Box<dyn Action + Send>,
}

impl SimpleRule {
    pub fn new(condition: Box<dyn Condition + Send>, action: Box<dyn Action + Send>) -> Self {
        Self { condition, action }
    }

    pub fn set_condition(&mut self, condition: Box<dyn Condition + Send>) -> bool {
        self.condition = condition;
        true
    }

    pub fn set_action(&mut self, action: Box<dyn Action + Send>) -> bool {
        self.action = action;
        true
    }
}

impl Rule for SimpleRule {
    fn check(&self, params: &mut Parameters) -> bool {
        let touch = &params.touch_data.last().unwrap();
        self.condition.applies(touch)
    }

    fn apply(&mut self, params: &mut Parameters) -> bool {
        self.action.apply(params)
    }
}
