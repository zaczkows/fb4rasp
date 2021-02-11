use fb4rasp;
use rand::distributions::Distribution;
use std::cell::RefCell;
use std::sync::mpsc;
use sysinfo::{ProcessorExt, SystemExt};

#[derive(Default)]
struct NetworkInfo {
    tx_bytes: i64,
    rx_bytes: i64,
}

#[derive(Default)]
struct CpuUsage {
    avg: f32,
    cores: [f32; 4],
}

struct SharedData {
    net_infos: fb4rasp::FixedRingBuffer<NetworkInfo>,
    cpu_usage: fb4rasp::FixedRingBuffer<CpuUsage>,
}

const DRAW_REFRESH_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_millis(1000);
const NET_REFRESH_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_secs(3);
const TOUCH_REFRESH_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_millis(100);
const DATA_SAMPLES: usize = 41;

impl SharedData {
    pub fn new() -> Self {
        Self {
            net_infos: fb4rasp::FixedRingBuffer::<NetworkInfo>::new_with(DATA_SAMPLES, || {
                NetworkInfo::default()
            }),
            cpu_usage: fb4rasp::FixedRingBuffer::<CpuUsage>::new_with(DATA_SAMPLES, || {
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

    pub fn get_rx_bytes(&self) -> Vec<i64> {
        let secs = NET_REFRESH_TIMEOUT.as_secs() as i64;
        let mut rxs = Vec::with_capacity(self.net_infos.size() as usize);
        for i in 1..DATA_SAMPLES as isize - 1 {
            rxs.push(
                (self.net_infos.item(i).rx_bytes - self.net_infos.item(i - 1).rx_bytes) / secs,
            );
        }
        rxs
    }

    pub fn get_tx_bytes(&self) -> Vec<i64> {
        let secs = NET_REFRESH_TIMEOUT.as_secs() as i64;
        let mut txs = Vec::with_capacity(self.net_infos.size() as usize);
        for i in 1..DATA_SAMPLES as isize - 1 {
            txs.push(
                (self.net_infos.item(i).tx_bytes - self.net_infos.item(i - 1).tx_bytes) / secs,
            );
        }
        txs
    }

    pub fn add_cpu_usage(&mut self, cpu_usage: CpuUsage) {
        self.cpu_usage.add(cpu_usage);
    }
}

fn print_touch_status(ts: &adafruit_mpr121::Mpr121TouchStatus) -> String {
    let mut status = String::new();
    let mut separator = "";
    for i in
        adafruit_mpr121::Mpr121TouchStatus::first()..=adafruit_mpr121::Mpr121TouchStatus::last()
    {
        if ts.touched(i) {
            status += separator;
            status += &format!("{}", i);
            separator = ", ";
        }
    }

    status
}

async fn render_screen(
    shared_data: &RefCell<SharedData>,
    touch_status: mpsc::Receiver<adafruit_mpr121::Mpr121TouchStatus>,
) {
    let mut fb = fb4rasp::Fb4Rasp::new().unwrap();
    let mut interval = tokio::time::interval(DRAW_REFRESH_TIMEOUT);
    let mut x: i32;
    let mut y: i32;

    fb.init_events();
    fb.set_font("DejaVuSansMono");

    let dist_uni = rand::distributions::Uniform::from(0..5);
    let mut rng = rand::thread_rng();
    let mut system = sysinfo::System::new_all();

    // First we update all information of our system struct.
    system.refresh_all();

    let mut screensaver = 0;
    let mut shift = 0;
    loop {
        system.refresh_cpu();
        system.refresh_memory();

        if screensaver == 33 {
            shift = dist_uni.sample(&mut rng);
            screensaver = 0;
        } else {
            screensaver += 1;
        }

        x = 0;
        y = 20 + shift;
        fb.clean();
        fb.start();
        let local_time = chrono::Local::now();
        fb.set_color(&fb4rasp::Color {
            red: 0.0,
            green: 0.5,
            blue: 1.0,
            alpha: 1.0,
        });
        fb.set_font_size(24.0);
        fb.render_text(
            &fb4rasp::Point {
                x: x as f64,
                y: y as f64,
            },
            local_time
                .format("%a, %d.%m.%Y, %H:%M:%S")
                .to_string()
                .as_str(),
        );
        y = y + 26;

        fb.set_font_size(20.0);
        fb.set_color(&fb4rasp::Color {
            red: 0.0,
            green: 1.0,
            blue: 0.0,
            alpha: 1.0,
        });

        let mut cpu_usage = CpuUsage::default();
        let (cpu_avg_usage, cpu_info) = {
            let processors = system.get_processors();
            let count = processors.len();
            let mut ci = String::new();
            let mut avg: f32 = 0.0;
            let mut separator = "";
            for (i, p) in processors.iter().enumerate() {
                ci.push_str(&format!("{}{:>2.0}", separator, p.get_cpu_usage()));
                separator = ", ";
                let cpu_avg = p.get_cpu_usage();
                avg += cpu_avg;
                cpu_usage.cores[i] = cpu_avg;
            }
            cpu_usage.avg = avg;
            (avg / count as f32, ci)
        };
        shared_data.borrow_mut().add_cpu_usage(cpu_usage);

        fb.render_text(
            &fb4rasp::Point {
                x: x as f64,
                y: y as f64,
            },
            &format!(
                "CPU: {:>2.0}% [{}] ({:.1}°C)",
                cpu_avg_usage,
                &cpu_info,
                fb4rasp::get_cpu_temperature()
            ),
        );
        y = y + 26;

        fb.render_text(
            &fb4rasp::Point {
                x: x as f64,
                y: y as f64,
            },
            &format!(
                "Memory: {} / {}",
                size::Size::Kibibytes(system.get_used_memory())
                    .to_string(size::Base::Base2, size::Style::Smart),
                size::Size::Kibibytes(system.get_total_memory())
                    .to_string(size::Base::Base2, size::Style::Smart),
            ),
        );
        y = y + 26;

        fb.set_color(&fb4rasp::Color {
            red: 0.5,
            green: 1.0,
            blue: 0.0,
            alpha: 1.0,
        });
        y = y + 26;

        {
            let brw = shared_data.borrow();
            let last = brw.last_net_info();
            let prev = brw.prev_net_info();
            fb.render_text(
                &fb4rasp::Point {
                    x: x as f64,
                    y: y as f64,
                },
                &format!(
                    "Bytes tx: {}, rx: {}",
                    size::Size::Bytes(last.tx_bytes)
                        .to_string(size::Base::Base2, size::Style::Smart),
                    size::Size::Bytes(last.rx_bytes)
                        .to_string(size::Base::Base2, size::Style::Smart),
                ),
            );
            y = y + 26;

            let secs = NET_REFRESH_TIMEOUT.as_secs() as i64;
            fb.render_text(
                &fb4rasp::Point {
                    x: x as f64,
                    y: y as f64,
                },
                &format!(
                    "Bytes tx/s: {}, rx/s: {}",
                    size::Size::Bytes((last.tx_bytes - prev.tx_bytes) / secs)
                        .to_string(size::Base::Base2, size::Style::Smart),
                    size::Size::Bytes((last.rx_bytes - prev.rx_bytes) / secs)
                        .to_string(size::Base::Base2, size::Style::Smart),
                ),
            );
            y = y + 26;
        }
        {
            fb.set_font_size(12.0);
            let mut space = 0;
            while let Ok(msg) = touch_status.try_recv() {
                y = y + space;
                if space == 0 {
                    space = 10;
                }
                fb.render_text(
                    &fb4rasp::Point {
                        x: x as f64,
                        y: y as f64,
                    },
                    &format!("Touched pins: {}", &print_touch_status(&msg)),
                );
            }
        }

        {
            use plotters::prelude::*;

            let _rx_data;
            let tx_data;
            {
                let brw = shared_data.borrow();
                _rx_data = brw.get_rx_bytes();
                tx_data = brw.get_tx_bytes();
                assert_eq!(tx_data.len(), _rx_data.len());
            }

            // Draw a network plot
            let plot = plotters_cairo::CairoBackend::new(
                fb.cairo_context().unwrap(),
                (fb.width() as u32, fb.height() as u32),
            )
            .unwrap()
            .into_drawing_area()
            .margin(y, 5, 5, 5);

            plot.fill(&GREEN).unwrap();

            let tx_max = tx_data.iter().fold(0, |acc, &x| std::cmp::max(acc, x));
            let mut net_chart = plotters::chart::ChartBuilder::on(&plot)
                .y_label_area_size(30)
                .build_cartesian_2d(0..tx_data.len(), 0i64..tx_max)
                .unwrap();

            net_chart
                .configure_mesh()
                .disable_x_mesh()
                .disable_x_axis()
                .y_labels(5)
                .draw()
                .unwrap();

            net_chart
                .draw_series(LineSeries::new(
                    tx_data.iter().enumerate().map(|(i, v)| (i, *v)),
                    &RED,
                ))
                .unwrap();
        }

        let events = fb.get_events();
        for e in events {
            log::debug!("Events {:?}", &e);
            fb.render_text(
                &fb4rasp::Point {
                    x: e.position.x,
                    y: e.position.y,
                },
                "X",
            );
        }
        fb.finish();

        interval.tick().await;
    }
}

async fn get_router_net_stats(shared_data: &RefCell<SharedData>) {
    fn parse_xx_to_i64(s: &str) -> Option<i64> {
        s.split(|c| c == ' ' || c == '\n')
            .nth(0)
            .unwrap_or("")
            .parse::<i64>()
            .ok()
    }

    let mut interval = tokio::time::interval(NET_REFRESH_TIMEOUT);
    let router_stats = fb4rasp::session::Session::new("192.168.1.1:2222").unwrap();

    loop {
        interval.tick().await;

        let rx_bytes = router_stats.read_remote_file("/sys/class/net/br0/statistics/rx_bytes");
        let tx_bytes = router_stats.read_remote_file("/sys/class/net/br0/statistics/tx_bytes");

        if rx_bytes.is_ok() && tx_bytes.is_ok() {
            let rx_bytes = String::from_utf8(rx_bytes.unwrap()).unwrap();
            let tx_bytes = String::from_utf8(tx_bytes.unwrap()).unwrap();

            // log::debug!("Content of rx: \'{}\'", rx_bytes);
            // log::debug!("Content of tx: \'{}\'", tx_bytes);

            let tx_value = parse_xx_to_i64(&tx_bytes.as_str());
            let rx_value = parse_xx_to_i64(&rx_bytes.as_str());
            if tx_value.is_some() && rx_value.is_some() {
                let tx_value = tx_value.unwrap();
                let rx_value = rx_value.unwrap();
                let sd = NetworkInfo {
                    tx_bytes: tx_value,
                    rx_bytes: rx_value,
                };
                log::debug!(
                    "Current usage is tx: {}, rx: {}",
                    size::Size::Bytes(tx_value).to_string(size::Base::Base2, size::Style::Smart),
                    size::Size::Bytes(rx_value).to_string(size::Base::Base2, size::Style::Smart),
                );
                shared_data.borrow_mut().add_net_info(sd);
            }
        }
    }
}

async fn update_touch_status(touch_status: mpsc::Sender<adafruit_mpr121::Mpr121TouchStatus>) {
    let mut interval = tokio::time::interval(TOUCH_REFRESH_TIMEOUT);
    log::debug!("Enabling MPR121 sensor");
    let touch_sensor = adafruit_mpr121::Mpr121::new_default(1);
    if touch_sensor.is_err() {
        log::error!("Failed to initialize MPR121 sensor");
        return;
    }
    let mut touch_sensor = touch_sensor.unwrap();
    if touch_sensor.reset().is_err() {
        log::error!("Failed to reset MPR121 sensor");
        return;
    }
    let mut engine = fb4rasp::Engine::new();
    {
        // create and add rules
        let mut powerdown_rule = Box::new(fb4rasp::rule::AndRule::new());
        powerdown_rule.add_condition(Box::new(fb4rasp::condition::MultiItemCondition::new(&[
            2u8, 3, 4, 6, 8,
        ])));
        powerdown_rule.add_action(Box::new(fb4rasp::action::ShutdownAction {}));
        engine.add(powerdown_rule);
    }
    loop {
        interval.tick().await;
        let status = touch_sensor.touch_status().unwrap();
        // log::debug!("MPR121 sensor touch status: {}", status);
        if status.was_touched() {
            engine.event(&status);
            touch_status.send(status).expect("Channel is broken");
        }
    }
}

async fn handle_ctrl_c() {
    let _ = tokio::signal::ctrl_c().await;
    log::info!("Received CTRL_C signal, exiting...");
}

#[tokio::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "debug");
    }
    env_logger::Builder::from_default_env()
        .format_timestamp_millis()
        .init();

    let shared_data = std::cell::RefCell::new(SharedData::new());
    let (touch_status_tx, touch_status_rx) = mpsc::channel(
        // (DRAW_REFRESH_TIMEOUT.as_secs_f64() / TOUCH_REFRESH_TIMEOUT.as_secs_f64()).ceil() as usize,
    );
    tokio::select! {
        _ = render_screen(&shared_data, touch_status_rx) => {()}
       _ = get_router_net_stats(&shared_data) => {()}
        _ = update_touch_status(touch_status_tx) => {()}
        _ = handle_ctrl_c() => {()}
    };
}
