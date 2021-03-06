use std::collections::HashMap;

use crate::params::{Layout, Parameters};
use crate::ring_buffer::FixedRingBuffer;
use crate::rule::Rule;
use fb4rasp_shared::{NetworkInfo, SystemInfo};
use tokio::sync::{mpsc, oneshot};

pub struct AnnotatedSystemInfo {
    pub source: String,
    pub si: SystemInfo,
}

pub const DEFAULT_HOST: &str = "localhost";

pub enum EngineCmdData {
    Net(NetworkInfo),
    SysInfo(AnnotatedSystemInfo),
    Touch(adafruit_mpr121::Mpr121TouchStatus),
    AddRule(Box<dyn Rule + Send>),
    GetLastNetInfo(oneshot::Sender<(NetworkInfo, NetworkInfo)>),
    GetTouchInfo(oneshot::Sender<Vec<adafruit_mpr121::Mpr121TouchStatus>>),
    GetNetTxRx {
        sender: oneshot::Sender<(Vec<i64>, Vec<i64>)>,
        refresh_rate: std::time::Duration,
    },
    GetLayout(oneshot::Sender<Layout>),
    GetSystemInfos(oneshot::Sender<HashMap<String, FixedRingBuffer<SystemInfo>>>),
}

impl std::fmt::Debug for EngineCmdData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EngineCmdData")
    }
}

#[derive(Clone)]
pub struct EngineHandle {
    sender: mpsc::Sender<EngineCmdData>,
}

impl EngineHandle {
    pub fn default() -> Self {
        let (tx, rx) = mpsc::channel(100);

        let engine = Engine::new(rx);
        tokio::spawn(run_engine(engine));

        Self { sender: tx }
    }

    pub async fn send(&mut self, cmd: EngineCmdData) {
        let _ = self.sender.send(cmd).await;
    }

    pub async fn add_rule(&mut self, rule: Box<dyn Rule + Send>) {
        let _ = self.sender.send(EngineCmdData::AddRule(rule)).await;
    }

    pub async fn last_net_info(&self) -> (NetworkInfo, NetworkInfo) {
        let (sender, receiver) = oneshot::channel();
        let _ = self
            .sender
            .send(EngineCmdData::GetLastNetInfo(sender))
            .await;
        receiver.await.unwrap()
    }

    pub async fn touch_info(&mut self) -> Vec<adafruit_mpr121::Mpr121TouchStatus> {
        let (sender, receiver) = oneshot::channel();
        let _ = self.sender.send(EngineCmdData::GetTouchInfo(sender)).await;
        receiver.await.unwrap()
    }

    pub async fn get_system_infos(&self) -> HashMap<String, FixedRingBuffer<SystemInfo>> {
        let (sender, receiver) = oneshot::channel();
        let _ = self
            .sender
            .send(EngineCmdData::GetSystemInfos(sender))
            .await;
        receiver.await.unwrap()
    }

    pub async fn get_net_tx_rx(&self, refresh_rate: &std::time::Duration) -> (Vec<i64>, Vec<i64>) {
        let (sender, receiver) = oneshot::channel();
        let _ = self
            .sender
            .send(EngineCmdData::GetNetTxRx {
                sender,
                refresh_rate: *refresh_rate,
            })
            .await;
        receiver.await.unwrap()
    }

    pub async fn get_main_layout(&self) -> Layout {
        let (sender, receiver) = oneshot::channel();
        let _ = self.sender.send(EngineCmdData::GetLayout(sender)).await;
        receiver.await.unwrap()
    }
}

struct Engine {
    rules: Vec<Box<dyn Rule + Send>>,
    params: Parameters,
    msg_rx: mpsc::Receiver<EngineCmdData>,
    sys_infos: HashMap<String, FixedRingBuffer<SystemInfo>>,
}

impl Engine {
    const DATA_SAMPLES: usize = (320 / 2) / 2;

    fn new(msg_rx: mpsc::Receiver<EngineCmdData>) -> Self {
        let mut me = Engine {
            rules: Vec::new(),
            params: Parameters::default(),
            msg_rx,
            sys_infos: HashMap::new(),
        };

        me.sys_infos.insert(
            DEFAULT_HOST.to_owned(),
            FixedRingBuffer::new(Self::DATA_SAMPLES, SystemInfo::default()),
        );

        me
    }

    fn handle_message(&mut self, msg: EngineCmdData) {
        match msg {
            EngineCmdData::Net(ni) => self.params.net_infos.add(ni),
            EngineCmdData::SysInfo(asi) => {
                if !self.sys_infos.contains_key(&asi.source) {
                    self.sys_infos.insert(
                        asi.source.to_owned(),
                        FixedRingBuffer::new(Self::DATA_SAMPLES, SystemInfo::default()),
                    );
                }
                let frb = self.sys_infos.get_mut(&asi.source).unwrap();
                frb.add(asi.si);
            }
            EngineCmdData::Touch(t) => {
                self.params.touch_data.push(t);
                self.event();
            }
            EngineCmdData::AddRule(rule) => self.rules.push(rule),
            EngineCmdData::GetLastNetInfo(sender) => {
                let data = &self.params.net_infos;
                let _ = sender.send((*data.item(-2), *data.last()));
            }
            EngineCmdData::GetTouchInfo(sender) => {
                let td = &mut self.params.touch_data;
                let mut v = Vec::new();
                std::mem::swap(td, &mut v);
                let _ = sender.send(v);
            }
            EngineCmdData::GetNetTxRx {
                sender,
                refresh_rate,
            } => {
                let get_net_bytes = |net_infos: &FixedRingBuffer<NetworkInfo>,
                                     refresh_rate: f32,
                                     accessor: &dyn Fn(&NetworkInfo) -> i64|
                 -> Vec<i64> {
                    let mut net_bytes = Vec::with_capacity((net_infos.size() - 1) as usize);
                    // range is exclusive
                    for i in 1..net_infos.size() {
                        net_bytes.push(
                            ((accessor(net_infos.item(i)) - accessor(net_infos.item(i - 1))) as f32
                                / refresh_rate)
                                .round() as i64,
                        );
                    }
                    net_bytes
                };

                let rt = refresh_rate.as_secs_f32();
                let data = &self.params.net_infos;
                let _ = sender.send((
                    get_net_bytes(data, rt, &|ni: &NetworkInfo| ni.tx_bytes),
                    get_net_bytes(data, rt, &|ni: &NetworkInfo| ni.rx_bytes),
                ));
            }
            EngineCmdData::GetLayout(sender) => {
                let _ = sender.send(self.params.options.main_layout);
            }
            EngineCmdData::GetSystemInfos(sender) => {
                let _ = sender.send(self.sys_infos.clone());
            }
        }
    }

    fn event(&mut self) {
        let mut applied = false;
        for rule in &*self.rules {
            applied = applied || rule.check(&mut self.params);
        }
        if applied {
            self.params.touch_data.clear();
        }
    }
}

async fn run_engine(mut engine: Engine) {
    while let Some(msg) = engine.msg_rx.recv().await {
        engine.handle_message(msg);
    }
}
