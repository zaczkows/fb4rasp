use crate::ring_buffer::FixedRingBuffer;
use fb4rasp_shared::{MemInfo, NetworkInfo, SystemInfo};

#[derive(Default)]
pub struct Parameters {
    pub sys_info_data: SysInfoData,
    pub touch_data: Vec<adafruit_mpr121::Mpr121TouchStatus>,
    pub options: Options,
}

#[derive(Clone, Copy, Debug)]
pub enum Layout {
    Horizontal,
    Vertical,
}

pub struct Options {
    pub main_layout: Layout,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            main_layout: Layout::Vertical,
        }
    }
}

pub struct SysInfoData {
    pub net_infos: FixedRingBuffer<NetworkInfo>,
    pub system_infos: FixedRingBuffer<SystemInfo>,
}

impl Default for SysInfoData {
    fn default() -> Self {
        const DATA_SAMPLES: usize = (320 / 2) / 2 + 1;
        Self {
            net_infos: FixedRingBuffer::<NetworkInfo>::new_with(DATA_SAMPLES, || {
                NetworkInfo::default()
            }),
            system_infos: FixedRingBuffer::<SystemInfo>::new_with(DATA_SAMPLES - 1, || {
                SystemInfo::default()
            }),
        }
    }
}

impl SysInfoData {
    pub fn add_net_info(&mut self, ni: NetworkInfo) {
        self.net_infos.add(ni);
    }

    pub fn last_net_info(&self) -> &NetworkInfo {
        self.net_infos.last()
    }

    pub fn prev_net_info(&self) -> &NetworkInfo {
        self.net_infos.item(-2)
    }

    fn get_net_bytes<F>(&self, accessor: F) -> Vec<i64>
    where
        F: Fn(&NetworkInfo) -> i64,
    {
        let mut net_bytes = Vec::with_capacity((self.net_infos.size() - 1) as usize);
        // range is exclusive
        for i in 1..self.net_infos.size() {
            net_bytes.push(accessor(self.net_infos.item(i)) - accessor(self.net_infos.item(i - 1)));
        }
        net_bytes
    }

    pub fn get_rx_bytes(&self) -> Vec<i64> {
        self.get_net_bytes(|ni| ni.rx_bytes)
    }

    pub fn get_tx_bytes(&self) -> Vec<i64> {
        self.get_net_bytes(|ni| ni.tx_bytes)
    }

    pub fn add_systeminfo(&mut self, system_info: SystemInfo) {
        self.system_infos.add(system_info);
    }

    pub fn get_cpu_usage(&self) -> Vec<f32> {
        self.system_infos.iter().map(|x| x.cpu.avg).collect()
    }

    pub fn get_memory_usage(&self) -> Vec<MemInfo> {
        self.system_infos.iter().map(|x| x.mem).collect()
    }
}
