use crate::ring_buffer::FixedRingBuffer;

pub struct Parameters {
    pub sys_info_data: SysInfoData,
    pub touch_data: Vec<adafruit_mpr121::Mpr121TouchStatus>,
    pub options: Options,
}

impl Parameters {
    pub fn new() -> Self {
        Self {
            sys_info_data: SysInfoData::new(),
            touch_data: Vec::new(),
            options: Options::new(),
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct NetworkInfo {
    pub tx_bytes: i64,
    pub rx_bytes: i64,
}

#[derive(Default, Clone, Copy)]
pub struct CpuUsage {
    pub avg: f32,
    pub cores: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
pub enum Layout {
    Horizontal,
    Vertical,
}

pub struct Options {
    pub main_layout: Layout,
}

impl Options {
    pub fn new() -> Self {
        Self {
            main_layout: Layout::Vertical,
        }
    }
}

pub struct SysInfoData {
    pub net_infos: FixedRingBuffer<NetworkInfo>,
    pub cpu_usage: FixedRingBuffer<CpuUsage>,
}

impl SysInfoData {
    pub fn new() -> Self {
        const DATA_SAMPLES: usize = (320 / 2) / 2 + 1;
        Self {
            net_infos: FixedRingBuffer::<NetworkInfo>::new_with(DATA_SAMPLES, || {
                NetworkInfo::default()
            }),
            cpu_usage: FixedRingBuffer::<CpuUsage>::new_with(DATA_SAMPLES - 1, || {
                CpuUsage::default()
            }),
        }
    }

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

    pub fn add_cpu_usage(&mut self, cpu_usage: CpuUsage) {
        self.cpu_usage.add(cpu_usage);
    }

    pub fn get_cpu_usage(&self) -> Vec<f32> {
        self.cpu_usage.iter().map(|x| x.avg).collect()
    }
}
