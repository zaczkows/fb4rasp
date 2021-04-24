use std::collections::HashMap;

use crate::params::{Layout, Parameters};
use crate::ring_buffer::FixedRingBuffer;
use crate::rule::Rule;
use fb4rasp_shared::{NetworkInfo, SystemInfo};
use parking_lot::Mutex;
use tokio::sync::mpsc;

pub struct AnnotatedSystemInfo {
    pub source: String,
    pub si: SystemInfo,
}

pub enum EngineCmdData {
    Net(NetworkInfo),
    SysInfo(SystemInfo),
    AnnSysInfo(AnnotatedSystemInfo),
    Touch(adafruit_mpr121::Mpr121TouchStatus),
    RemoteData,
    Stop,
}

impl std::fmt::Debug for EngineCmdData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EngineCmdData")
    }
}

pub struct Engine {
    rules: Mutex<Vec<Box<dyn Rule>>>,
    params: Mutex<Parameters>,
    msg_rx: Mutex<mpsc::Receiver<EngineCmdData>>,
    ext_data: Mutex<HashMap<String, FixedRingBuffer<SystemInfo>>>,
}

#[derive(Default)]
pub struct SummaryMemUsage {
    pub ram: Vec<u64>,
    pub swap: Vec<u64>,
    pub total_ram: u64,
    pub total_swap: u64,
}

impl SummaryMemUsage {
    pub fn is_empty(&self) -> bool {
        self.ram.is_empty() || self.swap.is_empty()
    }
}

impl Engine {
    pub fn new(msg_rx: mpsc::Receiver<EngineCmdData>) -> Self {
        Engine {
            rules: Mutex::new(Vec::new()),
            params: Mutex::new(Parameters::default()),
            msg_rx: Mutex::new(msg_rx),
            ext_data: Mutex::new(HashMap::new()),
        }
    }

    pub fn add_rule(&self, rule: Box<dyn Rule>) {
        self.rules.lock().push(rule)
    }

    const EXT_DATA_SAMPLES: usize = 51;
    pub async fn poll(&self) {
        let mut msg_rx = self.msg_rx.lock();
        loop {
            let msg = msg_rx.recv().await;
            match msg {
                Some(data) => match data {
                    EngineCmdData::Net(ni) => self.params.lock().sys_info_data.add_net_info(ni),
                    EngineCmdData::SysInfo(si) => {
                        self.params.lock().sys_info_data.add_systeminfo(si)
                    }
                    EngineCmdData::AnnSysInfo(asi) => {
                        let mut data = self.ext_data.lock();
                        if !data.contains_key(&asi.source) {
                            data.insert(
                                asi.source.to_owned(),
                                FixedRingBuffer::new(
                                    Engine::EXT_DATA_SAMPLES,
                                    SystemInfo::default(),
                                ),
                            );
                        }
                        let frb = data.get_mut(&asi.source).unwrap();
                        frb.add(asi.si);
                    }
                    EngineCmdData::Touch(t) => {
                        self.params.lock().touch_data.push(t);
                        self.event();
                    }
                    EngineCmdData::RemoteData => (),
                    EngineCmdData::Stop => {
                        log::debug!("STOP command received...");
                        break;
                    }
                },
                None => log::error!("msg channel failure"),
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

    pub fn get_cpu_usage(&self) -> Vec<f32> {
        (*self.params.lock()).sys_info_data.get_cpu_usage()
    }

    pub fn get_mem_usage(&self) -> SummaryMemUsage {
        let mem_data = self.params.lock().sys_info_data.get_memory_usage();
        if mem_data.is_empty() {
            SummaryMemUsage::default()
        } else {
            SummaryMemUsage {
                ram: mem_data.iter().map(|mu| mu.used_mem).collect(),
                swap: mem_data.iter().map(|mu| mu.used_swap).collect(),
                total_ram: mem_data[0].total_mem,
                total_swap: mem_data[0].total_swap,
            }
        }
    }

    pub fn get_net_tx_rx(&self) -> (Vec<i64>, Vec<i64>) {
        let data = &(*self.params.lock()).sys_info_data;
        (data.get_tx_bytes(), data.get_rx_bytes())
    }

    pub fn get_main_layout(&self) -> Layout {
        (*self.params.lock()).options.main_layout
    }
}
