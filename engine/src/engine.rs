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

pub const DEFAULT_HOST: &str = "localhost";

pub enum EngineCmdData {
    Net(NetworkInfo),
    SysInfo(AnnotatedSystemInfo),
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
    sys_infos: Mutex<HashMap<String, FixedRingBuffer<SystemInfo>>>,
}

impl Engine {
    pub fn new(msg_rx: mpsc::Receiver<EngineCmdData>) -> Self {
        let me = Engine {
            rules: Mutex::new(Vec::new()),
            params: Mutex::new(Parameters::default()),
            msg_rx: Mutex::new(msg_rx),
            sys_infos: Mutex::new(HashMap::new()),
        };

        const DATA_SAMPLES: usize = (320 / 2) / 2;
        me.sys_infos.lock().insert(
            DEFAULT_HOST.to_owned(),
            FixedRingBuffer::new(DATA_SAMPLES, SystemInfo::default()),
        );

        me
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
                    EngineCmdData::Net(ni) => self.params.lock().net_infos.add(ni),
                    EngineCmdData::SysInfo(asi) => {
                        let mut data = self.sys_infos.lock();
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
        let data = &self.params.lock().net_infos;
        (*data.item(-2), *data.last())
    }

    pub fn touch_info(&self) -> Vec<adafruit_mpr121::Mpr121TouchStatus> {
        let td = &mut self.params.lock().touch_data;
        let mut v = Vec::new();
        std::mem::swap(td, &mut v);
        v
    }

    pub fn get_system_infos(&self) -> &Mutex<HashMap<String, FixedRingBuffer<SystemInfo>>> {
        &self.sys_infos
    }

    pub fn get_net_tx_rx(&self) -> (Vec<i64>, Vec<i64>) {
        let get_net_bytes = |net_infos: &FixedRingBuffer<NetworkInfo>,
                             accessor: &dyn Fn(&NetworkInfo) -> i64|
         -> Vec<i64> {
            let mut net_bytes = Vec::with_capacity((net_infos.size() - 1) as usize);
            // range is exclusive
            for i in 1..net_infos.size() {
                net_bytes.push(accessor(net_infos.item(i)) - accessor(net_infos.item(i - 1)));
            }
            net_bytes
        };

        let data = &self.params.lock().net_infos;
        (
            get_net_bytes(data, &|ni: &NetworkInfo| ni.tx_bytes),
            get_net_bytes(data, &|ni: &NetworkInfo| ni.rx_bytes),
        )
    }

    pub fn get_main_layout(&self) -> Layout {
        self.params.lock().options.main_layout
    }
}
