use crate::rule::Rule;

pub struct Engine {
    rules: Vec<Box<dyn Rule>>,
}

impl Engine {
    pub fn new() -> Self {
        Engine { rules: Vec::new() }
    }

    pub fn add(&mut self, rule: Box<dyn Rule>) {
        self.rules.push(rule)
    }

    pub fn event(&mut self, touch: &adafruit_mpr121::Mpr121TouchStatus) {
        for rule in &mut self.rules {
            rule.check(touch);
        }
    }
}
