use crate::params::{CpuUsage, Layout, NetworkInfo, Parameters};
use crate::rule::Rule;
use parking_lot::Mutex;
use std::sync::mpsc;

pub enum EngineCmdData {
    NET(NetworkInfo),
    CPU(CpuUsage),
    TOUCH(adafruit_mpr121::Mpr121TouchStatus),
    STOP,
}

impl std::fmt::Debug for EngineCmdData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EngineCmdData")
    }
}

pub struct Engine {
    rules: Mutex<Vec<Box<dyn Rule>>>,
    params: Mutex<Parameters>,
    msg_rx: mpsc::Receiver<EngineCmdData>,
}

impl Engine {
    pub fn new(msg_rx: mpsc::Receiver<EngineCmdData>) -> Self {
        Engine {
            rules: Mutex::new(Vec::new()),
            params: Mutex::new(Parameters::new()),
            msg_rx,
        }
    }

    pub fn add_rule(&self, rule: Box<dyn Rule>) {
        self.rules.lock().push(rule)
    }

    pub fn poll(&self) {
        loop {
            let msg = self.msg_rx.recv();
            match msg {
                Ok(data) => match data {
                    EngineCmdData::NET(ni) => self.params.lock().sys_info_data.add_net_info(ni),
                    EngineCmdData::CPU(cu) => self.params.lock().sys_info_data.add_cpu_usage(cu),
                    EngineCmdData::TOUCH(t) => {
                        self.params.lock().touch_data.push(t);
                        self.event();
                    }
                    EngineCmdData::STOP => {
                        log::debug!("STOP command received...");
                        break;
                    }
                },
                Err(e) => log::error!("msg channel failure: {}", e),
            }
        }
    }

    fn event(&self) {
        let rules = self.rules.lock();
        let mut params = self.params.lock();
        let mut applied = false;
        for rule in &*rules {
            applied = applied || rule.check(&mut params);
        }
        if applied {
            params.touch_data.clear();
        }
    }

    pub fn last_net_info(&self) -> (NetworkInfo, NetworkInfo) {
        let data = &(*self.params.lock()).sys_info_data;
        (*data.prev_net_info(), *data.last_net_info())
    }

    pub fn touch_info(&self) -> Vec<adafruit_mpr121::Mpr121TouchStatus> {
        let td = &mut (*self.params.lock()).touch_data;
        let mut v = Vec::new();
        std::mem::swap(td, &mut v);
        v
    }

    pub fn get_cpu_usage(&self, timeout: &std::time::Duration) -> Vec<f32> {
        (*self.params.lock()).sys_info_data.get_cpu_usage(timeout)
    }

    pub fn get_net_tx_rx(&self, timeout: &std::time::Duration) -> (Vec<i64>, Vec<i64>) {
        let data = &(*self.params.lock()).sys_info_data;
        (data.get_tx_bytes(timeout), data.get_rx_bytes(timeout))
    }

    pub fn get_main_layout(&self) -> Layout {
        (*self.params.lock()).options.main_layout
    }
}

unsafe impl Send for Engine {}
unsafe impl Sync for Engine {}
