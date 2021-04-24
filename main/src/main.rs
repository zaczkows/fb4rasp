use display::{Color, Fb4Rasp, Point};
use engine::{
    action, condition,
    engine::{AnnotatedSystemInfo, EngineCmdData},
    params::{Layout, Parameters},
    rule, Engine,
};
use fb4rasp_shared::{CpuUsage, MemInfo, NetworkInfo, SystemInfo};
use rand::distributions::Distribution;
use session::{SshSession, WsSession};
use std::{path::PathBuf, sync::Arc};
use structopt::StructOpt;
use sysinfo::{ProcessorExt, SystemExt};
use tokio::sync::mpsc;

mod config;

// A basic example
#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct CmdLineOptions {
    // The number of occurrences of the `v/verbose` flag
    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[structopt(short, long, parse(from_occurrences))]
    verbose: u8,

    /// Output file
    #[structopt(short, long, parse(from_os_str))]
    config: Option<PathBuf>,
}

const DRAW_REFRESH_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1000);
const NET_REFRESH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
const TOUCH_REFRESH_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(100);
const REMOTE_REFRESH_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1000);

async fn render_screen(tx: mpsc::Sender<EngineCmdData>, engine: Arc<Engine>) {
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

    let mut fb = Fb4Rasp::new().unwrap();
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

    let mut interval = tokio::time::interval(DRAW_REFRESH_TIMEOUT);
    loop {
        system.refresh_cpu();
        system.refresh_memory();

        if screensaver == 33 {
            shift = dist_uni.sample(&mut rng);
            screensaver = 0;
        } else {
            screensaver += 1;
        }

        x = shift;
        y = 16;
        fb.clean();
        fb.start();
        let local_time = chrono::Local::now();
        fb.set_color(&Color {
            red: 0.9,
            green: 0.9,
            blue: 0.9,
            alpha: 1.0,
        });
        fb.set_font_size(22.0);
        fb.render_text(
            &Point {
                x: x as f64,
                y: y as f64,
            },
            local_time
                .format("%a, %d.%m.%Y, %H:%M:%S")
                .to_string()
                .as_str(),
        );
        y += 20;

        let mut cpu_usage = CpuUsage::default();
        let mut cpu_info_str = String::new();
        {
            let processors = system.get_processors();
            let count = processors.len();
            cpu_usage.detailed.resize(count, 0.0);
            let mut avg: f32 = 0.0;
            let mut separator = "";
            for (i, p) in processors.iter().enumerate() {
                let p_usage = p.get_cpu_usage();
                cpu_info_str.push_str(&format!("{}{:>2.0}", separator, p_usage));
                separator = ", ";
                cpu_usage.detailed[i] = p_usage;
                avg += p_usage;
            }
            cpu_usage.avg = avg / count as f32;
        }

        let mem_info = MemInfo {
            used_mem: system.get_used_memory(),
            total_mem: system.get_total_memory(),
            used_swap: system.get_used_swap(),
            total_swap: system.get_total_swap(),
        };

        fb.set_font_size(18.0);
        fb.set_color(&Color {
            red: 0xff as f64 / 256f64,
            green: 0xbf as f64 / 256f64,
            blue: 0.0,
            alpha: 1.0,
        });
        fb.render_text(
            &Point {
                x: x as f64,
                y: y as f64,
            },
            &format!(
                "CPU: {:>2.0}% [{}] ({:.1}°C)",
                cpu_usage.avg,
                &cpu_info_str,
                display::get_cpu_temperature()
            ),
        );
        y += 18;

        fb.set_color(&Color {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        });

        fb.render_text(
            &Point {
                x: x as f64,
                y: y as f64,
            },
            &format!(
                "Memory: {} / {}",
                size::Size::Kibibytes(mem_info.used_mem)
                    .to_string(size::Base::Base2, size::Style::Smart),
                size::Size::Kibibytes(mem_info.total_mem)
                    .to_string(size::Base::Base2, size::Style::Smart),
            ),
        );

        let _ = tx
            .send(EngineCmdData::SysInfo(SystemInfo {
                cpu: cpu_usage,
                mem: mem_info,
            }))
            .await;

        {
            y += 20;

            fb.set_font_size(14.0);
            fb.set_color(&Color {
                red: 0.5,
                green: 1.0,
                blue: 0.0,
                alpha: 1.0,
            });

            let secs = NET_REFRESH_TIMEOUT.as_secs() as i64;
            let (prev, last) = engine.last_net_info();
            fb.render_text(
                &Point {
                    x: x as f64,
                    y: y as f64,
                },
                &format!(
                    "Bytes tx: {}, tx/s: {}",
                    size::Size::Bytes(last.tx_bytes)
                        .to_string(size::Base::Base2, size::Style::Smart),
                    size::Size::Bytes((last.tx_bytes - prev.tx_bytes) / secs)
                        .to_string(size::Base::Base2, size::Style::Smart),
                ),
            );
            y += 14;

            fb.set_color(&Color {
                red: 0.18,
                green: 0.56,
                blue: 0.83,
                alpha: 1.0,
            });
            fb.render_text(
                &Point {
                    x: x as f64,
                    y: y as f64,
                },
                &format!(
                    "Bytes rx: {}, rx/s: {}",
                    size::Size::Bytes(last.rx_bytes)
                        .to_string(size::Base::Base2, size::Style::Smart),
                    size::Size::Bytes((last.rx_bytes - prev.rx_bytes) / secs)
                        .to_string(size::Base::Base2, size::Style::Smart),
                ),
            );
        }

        {
            fb.set_font_size(10.0);
            let mut space = 0;
            let touch_status = engine.touch_info();
            for msg in touch_status {
                y += space;
                if space == 0 {
                    y += 22;
                    space = 10;
                }
                fb.render_text(
                    &Point {
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

            y += 12;

            let layout = engine.get_main_layout();

            let plot_cpu_mem_data = |engine: &Engine| {
                let cpu_usage = engine.get_cpu_usage();
                let mem_usage = engine.get_mem_usage();
                if cpu_usage.is_empty() || mem_usage.is_empty() {
                    return;
                }

                // Draw a network plot
                let plot = plotters_cairo::CairoBackend::new(
                    fb.cairo_context().unwrap(),
                    (fb.width() as u32, fb.height() as u32),
                )
                .unwrap()
                .into_drawing_area();

                let plot = match layout {
                    Layout::Horizontal => plot.margin(y + 2, 2, 2, (fb.width() / 2) as u32 + 2),
                    Layout::Vertical => {
                        plot.margin(y + 2, ((fb.height() - y as usize) / 2) as u32 + 2, 2, 2)
                    }
                };

                let max_mem_usage = *mem_usage.ram.iter().max().unwrap();
                let mut net_chart = plotters::chart::ChartBuilder::on(&plot)
                    .y_label_area_size(5)
                    .right_y_label_area_size(5)
                    .build_cartesian_2d(0..cpu_usage.len(), 0f32..100f32)
                    .unwrap()
                    .set_secondary_coord(0..mem_usage.ram.len(), 0u64..max_mem_usage);

                let labels_font = TextStyle {
                    font: FontDesc::new(FontFamily::Monospace, 12.0, FontStyle::Normal),
                    color: plotters_backend::BackendColor {
                        alpha: 1.0,
                        rgb: (255, 255, 255),
                    },
                    pos: text_anchor::Pos::new(text_anchor::HPos::Left, text_anchor::VPos::Center),
                };

                net_chart
                    .draw_series(LineSeries::new(
                        cpu_usage.iter().enumerate().map(|(i, v)| (i, *v)),
                        &RGBColor(0xff, 0xbf, 0),
                    ))
                    .unwrap();
                net_chart
                    .draw_secondary_series(LineSeries::new(
                        mem_usage.ram.iter().enumerate().map(|(i, v)| (i, *v)),
                        &RED,
                    ))
                    .unwrap();

                net_chart
                    .configure_mesh()
                    .disable_x_mesh()
                    .disable_y_mesh()
                    .y_labels(5)
                    .set_tick_mark_size(LabelAreaPosition::Left, -5)
                    .y_label_formatter(&|v| format!("{:.0}%", v))
                    .axis_style(&RED)
                    .label_style(labels_font.clone())
                    .draw()
                    .unwrap();

                net_chart
                    .configure_secondary_axes()
                    .y_labels(5)
                    .set_tick_mark_size(LabelAreaPosition::Right, -5)
                    .y_label_formatter(&|v| {
                        size::Size::Kibibytes(*v).to_string(size::Base::Base2, size::Style::Smart)
                    })
                    .axis_style(&RED)
                    .label_style(labels_font)
                    .draw()
                    .unwrap();
            };

            plot_cpu_mem_data(&engine);

            let plot_network_information = |engine: &Engine| {
                let (tx_data, rx_data) = engine.get_net_tx_rx();
                if tx_data.is_empty() || rx_data.is_empty() {
                    return;
                }

                // Draw a network plot
                let plot = plotters_cairo::CairoBackend::new(
                    fb.cairo_context().unwrap(),
                    (fb.width() as u32, fb.height() as u32),
                )
                .unwrap()
                .into_drawing_area();

                let plot = match layout {
                    Layout::Horizontal => plot.margin(y + 2, 2, (fb.width() / 2 + 2) as u32, 2),
                    Layout::Vertical => {
                        plot.margin(y + ((fb.height() - y as usize) / 2) as i32 + 2, 2, 2, 2)
                    }
                };

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
                        &RGBColor(47, 144, 212),
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
            };

            plot_network_information(&engine);
        }

        let events = fb.get_events();
        for e in events {
            log::debug!("Events {:?}", &e);
            fb.render_text(
                &Point {
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

enum RouterNetInfoError {
    Ssh,
    StringConversion,
    Parsing,
}

impl From<session::Error> for RouterNetInfoError {
    fn from(e: session::Error) -> RouterNetInfoError {
        log::error!("Ssh error: {}", e);
        RouterNetInfoError::Ssh
    }
}

impl From<std::string::FromUtf8Error> for RouterNetInfoError {
    fn from(e: std::string::FromUtf8Error) -> RouterNetInfoError {
        log::error!("Converting data to utf8 failed due to {}", e);
        RouterNetInfoError::StringConversion
    }
}

async fn get_router_net_data(
    router_stats: &SshSession,
    tx: &mpsc::Sender<EngineCmdData>,
) -> Result<(), RouterNetInfoError> {
    fn parse_xx_to_i64(s: &str) -> Option<i64> {
        s.split(|c| c == ' ' || c == '\n')
            .next()
            .unwrap_or("")
            .parse::<i64>()
            .ok()
    }

    let rx_bytes = router_stats.read_remote_file("/sys/class/net/br0/statistics/rx_bytes")?;
    let tx_bytes = router_stats.read_remote_file("/sys/class/net/br0/statistics/tx_bytes")?;

    let rx_bytes = String::from_utf8(rx_bytes)?;
    let tx_bytes = String::from_utf8(tx_bytes)?;

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

        let _r = tx.send(EngineCmdData::Net(sd)).await;
        Ok(())
    } else {
        Err(RouterNetInfoError::Parsing)
    }
}

async fn get_router_net_stats(tx: mpsc::Sender<EngineCmdData>) {
    let mut router_stats = SshSession::new("192.168.1.1:2222").ok();

    let mut interval = tokio::time::interval(NET_REFRESH_TIMEOUT);
    loop {
        interval.tick().await;

        match router_stats.as_ref() {
            Some(rs) => {
                if get_router_net_data(rs, &tx).await.is_err() {
                    router_stats = None;
                }
            }
            None => router_stats = SshSession::new("192.168.1.1:2222").ok(),
        }
    }
}

async fn update_touch_status(tx: mpsc::Sender<EngineCmdData>) {
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

    let mut interval = tokio::time::interval(TOUCH_REFRESH_TIMEOUT);
    loop {
        interval.tick().await;

        let status = touch_sensor.touch_status().unwrap();
        // log::debug!("MPR121 sensor touch status: {}", status);
        if status.was_touched() {
            let _r = tx.send(EngineCmdData::Touch(status)).await;
        }
    }
}

fn get_remote_sys_data(tx: mpsc::Sender<EngineCmdData>, config: config::Config) {
    enum Session {
        Unconnected(String),
        Connected((WsSession, String)),
    }

    async fn handle_session(mut session: Session, tx: mpsc::Sender<EngineCmdData>) {
        loop {
            match &mut session {
                Session::Unconnected(address) => match WsSession::new(&address).await {
                    Ok(mut w) => match w
                        .send_text(&format!("refresh {}ms", REMOTE_REFRESH_TIMEOUT.as_millis()))
                        .await
                    {
                        Ok(()) => session = Session::Connected((w, address.to_string())),
                        Err(e) => {
                            log::error!(
                                "Failed to start refresh with {} due to {:?}",
                                &address,
                                &e
                            );
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        }
                    },
                    Err(e) => {
                        log::error!("Failed to connect to {} due {:?}", &address, &e);
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                },
                Session::Connected((wss, addr)) => {
                    let _res: Result<(), String> = match wss.read_text().await {
                        Ok(Some(msg)) => {
                            use fb4rasp_shared::VectorSerde;
                            let data = fb4rasp_shared::SystemInfo::from_json(&msg);
                            log::debug!("Received: {:?}", &data);
                            if data.is_ok() {
                                for d in data.unwrap() {
                                    // TODO: Ignore errors for now
                                    let _ = tx
                                        .send(EngineCmdData::AnnSysInfo(AnnotatedSystemInfo {
                                            source: addr.to_owned(),
                                            si: d,
                                        }))
                                        .await;
                                }
                            }
                            Ok(())
                        }
                        Ok(None) => Err("Failed to receive text message".to_owned()),
                        Err(e) => {
                            session = Session::Unconnected(addr.to_string());
                            Err(format!("Connection error: {:?}", &e))
                        }
                    };
                }
            }
        }
    }

    for r in config.remotes.iter() {
        tokio::spawn(handle_session(
            Session::Unconnected(format!("ws://{}:12345/ws/sysinfo", r.1.ip)),
            tx.clone(),
        ));
    }
}

async fn handle_engine_poll(engine: Arc<Engine>) {
    engine.poll().await
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

    let cmd_line_opt = CmdLineOptions::from_args();
    log::debug!("Parsed cmd line parameters:\n{:#?}", &cmd_line_opt);

    let config_file = if cmd_line_opt.config.is_some() {
        config::read_toml_config(cmd_line_opt.config.unwrap()).unwrap()
    } else {
        config::Config::new()
    };

    let (tx, rx) = mpsc::channel(100);
    let engine = Arc::new(Engine::new(rx));
    {
        // create and add rules
        let mut powerdown_rule = Box::new(rule::AndRule::default());
        powerdown_rule.add_condition(Box::new(condition::MultiItemCondition::new(&[
            2u8, 3, 4, 6, 8,
        ])));
        powerdown_rule.add_action(Box::new(action::ShutdownAction {}));
        engine.add_rule(powerdown_rule);

        struct ChangeLayoutAction {}
        impl action::Action for ChangeLayoutAction {
            fn apply(&self, params: &mut Parameters) -> bool {
                match params.options.main_layout {
                    Layout::Vertical => params.options.main_layout = Layout::Horizontal,
                    Layout::Horizontal => params.options.main_layout = Layout::Vertical,
                }
                true
            }
        }

        let swap_layout_rule = Box::new(rule::SimpleRule::new(
            Box::new(condition::OneItemCondition::new(2)),
            Box::new(ChangeLayoutAction {}),
        ));
        engine.add_rule(swap_layout_rule);
    }

    get_remote_sys_data(tx.clone(), config_file);

    tokio::select! {
        _ = {let engine = Arc::clone(&engine); handle_engine_poll(engine)} => {()}
        _ = {let tx = tx.clone(); render_screen(tx, engine)} => {()}
        _ = {let tx = tx.clone(); get_router_net_stats(tx)} => {()}
        _ = {let tx = tx.clone(); update_touch_status(tx)} => {()}
        _ = handle_ctrl_c() => {()}
    };
}
