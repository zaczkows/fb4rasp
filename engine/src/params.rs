use crate::ring_buffer::FixedRingBuffer;
use fb4rasp_shared::NetworkInfo;

pub struct Parameters {
    pub net_infos: FixedRingBuffer<NetworkInfo>,
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

impl Default for Parameters {
    fn default() -> Self {
        const DATA_SAMPLES: usize = (320 / 2) / 2 + 1;
        Self {
            net_infos: FixedRingBuffer::<NetworkInfo>::new(DATA_SAMPLES, NetworkInfo::default()),
            touch_data: Vec::default(),
            options: Options::default(),
        }
    }
}
