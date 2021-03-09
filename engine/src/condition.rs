pub trait Condition {
    fn applies(&self, touch: &adafruit_mpr121::Mpr121TouchStatus) -> bool;
}

pub struct OneItemCondition {
    item: u8,
}

impl OneItemCondition {
    pub fn new(item: u8) -> Self {
        Self { item }
    }
}

impl Condition for OneItemCondition {
    fn applies(&self, touch: &adafruit_mpr121::Mpr121TouchStatus) -> bool {
        let count = touch.iter().filter(|x| *x == true).count();
        if count == 1 {
            touch.touched(self.item)
        } else {
            false
        }
    }
}

pub struct MultiItemCondition {
    mask: u16,
}

impl MultiItemCondition {
    pub fn new(items: &[u8]) -> Self {
        let mut mask = 0u16;
        for i in items {
            assert!(adafruit_mpr121::Mpr121TouchStatus::first() <= *i);
            assert!(*i <= adafruit_mpr121::Mpr121TouchStatus::last());
            mask |= 1 << i;
        }

        Self { mask }
    }
}

impl Condition for MultiItemCondition {
    fn applies(&self, touch: &adafruit_mpr121::Mpr121TouchStatus) -> bool {
        for i in
            adafruit_mpr121::Mpr121TouchStatus::first()..=adafruit_mpr121::Mpr121TouchStatus::last()
        {
            let is_touched = (self.mask & (1 << i)) != 0;
            if touch.touched(i) != is_touched {
                return false;
            }
        }

        true
    }
}
