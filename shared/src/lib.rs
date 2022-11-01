pub mod notify;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct CpuUsage {
    pub avg: f32,
    pub detailed: Vec<f32>,
}

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize)]
pub struct MemInfo {
    pub used_mem: u64,
    pub total_mem: u64,
    pub used_swap: u64,
    pub total_swap: u64,
}

#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub tx_bytes: i64,
    pub rx_bytes: i64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SystemInfo {
    pub cpu: CpuUsage,
    pub mem: MemInfo,
}

pub trait VectorSerde {
    fn deserialize<'a>(input: &'a str) -> Result<Vec<Self>, String>
    where
        Self: Sized + Deserialize<'a>,
    {
        match serde_json::from_str(input) {
            Ok(o) => Ok(o),
            Err(e) => Err(format!("{:?}", &e)),
        }
    }

    fn serialize(input: &[Self]) -> String
    where
        Self: Sized + Serialize,
    {
        serde_json::to_string(&input).unwrap()
    }
}

impl VectorSerde for CpuUsage {}
impl VectorSerde for MemInfo {}
impl VectorSerde for SystemInfo {}

pub struct RenderState {
    pub net_tx: Vec<i64>,
    pub net_rx: Vec<i64>,
}
