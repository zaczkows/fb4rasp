use crate::params::{CpuUsage, Layout, NetworkInfo, Parameters};
use crate::rule::Rule;
use std::cell::RefCell;

pub enum EngineCmdData {
    NET(NetworkInfo),
    CPU(CpuUsage),
    TOUCH(adafruit_mpr121::Mpr121TouchStatus),
}

impl std::fmt::Debug for EngineCmdData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EngineCmdData")
    }
}

pub struct Engine {
    rules: RefCell<Vec<Box<dyn Rule>>>,
    params: RefCell<Parameters>,
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            rules: RefCell::new(Vec::new()),
            params: RefCell::new(Parameters::new()),
        }
    }

    pub fn add_rule(&self, rule: Box<dyn Rule>) {
        self.rules.borrow_mut().push(rule)
    }

    pub fn add_new_data(&self, data: EngineCmdData) {
        match data {
            EngineCmdData::NET(ni) => self.params.borrow_mut().sys_info_data.add_net_info(ni),
            EngineCmdData::CPU(cu) => self.params.borrow_mut().sys_info_data.add_cpu_usage(cu),
            EngineCmdData::TOUCH(t) => self.params.borrow_mut().touch_data.push(t),
        }
    }

    pub fn last_net_info(&self) -> (NetworkInfo, NetworkInfo) {
        let data = &(*self.params.borrow()).sys_info_data;
        (*data.prev_net_info(), *data.last_net_info())
    }

    pub fn touch_info(&self) -> Vec<adafruit_mpr121::Mpr121TouchStatus> {
        let td = &mut (*self.params.borrow_mut()).touch_data;
        let mut v = Vec::new();
        std::mem::swap(td, &mut v);
        v
    }

    pub fn event(&mut self, touch: &adafruit_mpr121::Mpr121TouchStatus) {
        let rules = self.rules.borrow_mut();
        for rule in &*rules {
            rule.check(touch);
        }
    }

    pub fn get_cpu_usage(&self, timeout: &tokio::time::Duration) -> Vec<f32> {
        (*self.params.borrow()).sys_info_data.get_cpu_usage(timeout)
    }

    pub fn get_net_tx_rx(&self, timeout: &tokio::time::Duration) -> (Vec<i64>, Vec<i64>) {
        let data = &(*self.params.borrow()).sys_info_data;
        (data.get_tx_bytes(timeout), data.get_rx_bytes(timeout))
    }

    pub fn get_main_layout(&self) -> Layout {
        (*self.params.borrow()).options.main_layout()
    }
}

// unsafe impl Send for Engine {}
// unsafe impl Sync for Engine {}
