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

impl SharedData {
    pub fn new() -> Self {
        const DATA_SAMPLES: usize = (320 / 2) / 2 + 1;
        Self {
            net_infos: fb4rasp::FixedRingBuffer::<NetworkInfo>::new_with(DATA_SAMPLES, || {
                NetworkInfo::default()
            }),
            cpu_usage: fb4rasp::FixedRingBuffer::<CpuUsage>::new_with(DATA_SAMPLES - 1, || {
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
        let secs = NET_REFRESH_TIMEOUT.as_secs() as i64;
        let mut net_bytes = Vec::with_capacity((self.net_infos.size() - 1) as usize);
        // range is exclusive
        for i in 1..self.net_infos.size() {
            net_bytes.push(
                (accessor(self.net_infos.item(i)) - accessor(self.net_infos.item(i - 1))) / secs,
            );
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
        let secs = NET_REFRESH_TIMEOUT.as_secs() as f32;
        let mut cpu_usage = Vec::with_capacity((self.cpu_usage.size() - 1) as usize);
        // range is exclusive
        for i in 0..self.cpu_usage.size() {
            cpu_usage.push(self.cpu_usage.item(i).avg / secs);
        }
        cpu_usage
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
        y = 16 + shift;
        fb.clean();
        fb.start();
        let local_time = chrono::Local::now();
        fb.set_color(&fb4rasp::Color {
            red: 0.0,
            green: 0.5,
            blue: 1.0,
            alpha: 1.0,
        });
        fb.set_font_size(22.0);
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
        y = y + 22;

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

        fb.set_font_size(18.0);
        fb.set_color(&fb4rasp::Color {
            red: 0xff as f64 / 256f64,
            green: 0xbf as f64 / 256f64,
            blue: 0.0,
            alpha: 1.0,
        });
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
        y = y + 22;

        fb.set_color(&fb4rasp::Color {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        });
        fb.set_font_size(14.0);

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

        {
            y = y + 14;

            fb.set_color(&fb4rasp::Color {
                red: 0.5,
                green: 1.0,
                blue: 0.0,
                alpha: 1.0,
            });

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
            y = y + 22;

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
        }

        {
            fb.set_font_size(10.0);
            let mut space = 0;
            while let Ok(msg) = touch_status.try_recv() {
                y = y + space;
                if space == 0 {
                    y = y + 22;
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
            use plotters::style::text_anchor;

            //Plot CPU data
            {
                let cpu_usage = shared_data.borrow().get_cpu_usage();
                // Draw a network plot
                let plot = plotters_cairo::CairoBackend::new(
                    fb.cairo_context().unwrap(),
                    (fb.width() as u32, fb.height() as u32),
                )
                .unwrap()
                .into_drawing_area()
                .margin(y + 2, 2, 2, (fb.width() / 2) as u32 + 2);

                let mut net_chart = plotters::chart::ChartBuilder::on(&plot)
                    .y_label_area_size(5)
                    .build_cartesian_2d(0..cpu_usage.len(), 0f32..100f32)
                    .unwrap();

                let labels_font = TextStyle {
                    font: FontDesc::new(FontFamily::Monospace, 12.0, FontStyle::Normal),
                    color: plotters_backend::BackendColor {
                        alpha: 1.0,
                        rgb: (255, 255, 255),
                    },
                    pos: text_anchor::Pos::new(text_anchor::HPos::Left, text_anchor::VPos::Center),
                };

                net_chart
                    .draw_series(AreaSeries::new(
                        cpu_usage.iter().enumerate().map(|(i, v)| (i, *v)),
                        0.0,
                        &RGBColor(0xff, 0xbf, 0),
                    ))
                    .unwrap();

                net_chart
                    .configure_mesh()
                    .disable_x_mesh()
                    .disable_x_axis()
                    .disable_y_mesh()
                    .y_labels(5)
                    .set_tick_mark_size(LabelAreaPosition::Left, -5)
                    .y_label_formatter(&|v| format!("{:.0}%", v))
                    .axis_style(&RED)
                    .label_style(labels_font)
                    .draw()
                    .unwrap();
            }

            // Plot network information
            {
                let tx_data;
                let rx_data;
                {
                    let brw = shared_data.borrow();
                    tx_data = brw.get_tx_bytes();
                    rx_data = brw.get_rx_bytes();
                    assert_eq!(tx_data.len(), rx_data.len());
                }

                // Draw a network plot
                let plot = plotters_cairo::CairoBackend::new(
                    fb.cairo_context().unwrap(),
                    (fb.width() as u32, fb.height() as u32),
                )
                .unwrap()
                .into_drawing_area()
                .margin(y + 2, 2, (fb.width() / 2 + 2) as u32, 2);

                let tx_max = tx_data.iter().fold(0, |acc, &x| std::cmp::max(acc, x));
                let rx_max = rx_data.iter().fold(0, |acc, &x| std::cmp::max(acc, x));
                let mut net_chart = plotters::chart::ChartBuilder::on(&plot)
                    .y_label_area_size(5)
                    .right_y_label_area_size(5)
                    .build_cartesian_2d(0..tx_data.len(), 0i64..tx_max)
                    .unwrap()
                    .set_secondary_coord(0..rx_data.len(), 0i64..rx_max);

                let labels_font = TextStyle {
                    font: FontDesc::new(FontFamily::Monospace, 12.0, FontStyle::Normal),
                    color: plotters_backend::BackendColor {
                        alpha: 1.0,
                        rgb: (255, 255, 0),
                    },
                    pos: text_anchor::Pos::new(text_anchor::HPos::Left, text_anchor::VPos::Center),
                };

                net_chart
                    .draw_series(LineSeries::new(
                        tx_data.iter().enumerate().map(|(i, v)| (i, *v)),
                        &GREEN,
                    ))
                    .unwrap();

                net_chart
                    .draw_secondary_series(LineSeries::new(
                        rx_data.iter().enumerate().map(|(i, v)| (i, *v)),
                        &BLUE,
                    ))
                    .unwrap();

                net_chart
                    .configure_mesh()
                    .disable_x_mesh()
                    .disable_x_axis()
                    .disable_y_mesh()
                    .y_labels(5)
                    .set_tick_mark_size(LabelAreaPosition::Left, -5)
                    .y_label_formatter(&|v| {
                        size::Size::Bytes(*v).to_string(size::Base::Base2, size::Style::Smart)
                    })
                    .axis_style(&RED)
                    .label_style(labels_font.clone())
                    .draw()
                    .unwrap();

                net_chart
                    .configure_secondary_axes()
                    .y_labels(5)
                    .set_tick_mark_size(LabelAreaPosition::Right, -5)
                    .y_label_formatter(&|v| {
                        size::Size::Bytes(*v).to_string(size::Base::Base2, size::Style::Smart)
                    })
                    .axis_style(&RED)
                    .label_style(labels_font)
                    .draw()
                    .unwrap();
            }
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
