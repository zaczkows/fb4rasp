use crate::action::Action;
use crate::condition::Condition;
use crate::params::Parameters;

pub trait Rule {
    fn check(&self, params: &mut Parameters) -> bool;
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
    fn check(&self, params: &mut Parameters) -> bool {
        let touch = &params.touch_data.last().unwrap();
        for c in &self.conditions {
            if !c.applies(touch) {
                return false;
            }
        }

        for a in &self.actions {
            a.apply(params);
        }

        true
    }
}

pub struct OrRule {
    conditions: Vec<Box<dyn Condition>>,
    actions: Vec<Box<dyn Action>>,
}

impl OrRule {
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

        if !applies {
            return false;
        }

        for a in &self.actions {
            a.apply(params);
        }

        true
    }
}

pub struct SimpleRule {
    condition: Box<dyn Condition>,
    action: Box<dyn Action>,
}

impl SimpleRule {
    pub fn new(condition: Box<dyn Condition>, action: Box<dyn Action>) -> Self {
        Self { condition, action }
    }

    pub fn set_condition(&mut self, condition: Box<dyn Condition>) -> bool {
        self.condition = condition;
        true
    }

    pub fn set_action(&mut self, action: Box<dyn Action>) -> bool {
        self.action = action;
        true
    }
}

impl Rule for SimpleRule {
    fn check(&self, params: &mut Parameters) -> bool {
        let touch = &params.touch_data.last().unwrap();
        if self.condition.applies(touch) {
            return self.action.apply(params);
        }

        false
    }
}