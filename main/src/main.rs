use display::{CairoSvg, Color, Display, Fb4Rasp, Point};
use engine::{
    action, condition,
    engine::{AnnotatedSystemInfo, EngineCmdData},
    params::{Layout, Parameters},
    rule, EngineHandle,
};
use fb4rasp_shared::{CpuUsage, MemInfo, NetworkInfo, SystemInfo};
use rand::distributions::Distribution;
use session::{SshSession, WsSession};
use std::{cmp::max, path::PathBuf};
use structopt::StructOpt;
use sysinfo::{ProcessorExt, SystemExt};

mod config;

mod helpers;
use crate::helpers::{PlotData, SeriesData, SummaryMemUsage};

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

async fn render_screen(engine_handle: EngineHandle) {
    async fn render_screen_internal<DB>(mut engine_handle: EngineHandle, mut fb: DB)
    where
        for<'a> DB: Display<'a>,
    {
        fn print_touch_status(ts: &adafruit_mpr121::Mpr121TouchStatus) -> String {
            let mut status = String::new();
            let mut separator = "";
            for i in adafruit_mpr121::Mpr121TouchStatus::first()
                ..=adafruit_mpr121::Mpr121TouchStatus::last()
            {
                if ts.touched(i) {
                    status += separator;
                    status += &format!("{}", i);
                    separator = ", ";
                }
            }

            status
        }

        let mut x: i32;
        let mut y: i32;

        fb.init_events();

        let dist_uni = rand::distributions::Uniform::from(0..5);
        let mut rng = rand::thread_rng();
        let mut system = sysinfo::System::new_all();

        // First we update all information of our system struct.
        system.refresh_all();

        let mut screensaver: usize = 0;
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
            fb.start();
            fb.set_font("DejaVuSansMono");
            fb.set_color(&Color {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            });
            fb.clean();
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

            let _ = engine_handle
                .send(EngineCmdData::SysInfo(AnnotatedSystemInfo {
                    source: engine::engine::DEFAULT_HOST.to_owned(),
                    si: SystemInfo {
                        cpu: cpu_usage,
                        mem: mem_info,
                    },
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
                let (prev, last) = engine_handle.last_net_info().await;
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
                let touch_status = engine_handle.touch_info().await;
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

            y += 12;

            let layout = engine_handle.get_main_layout().await;

            {
                use plotters::prelude::*;

                let mut color_index: usize = 0;
                {
                    let mut cpu_axis_data = Vec::<SeriesData<Vec<f32>>>::new();
                    let mut net_axis_data = Vec::<SeriesData<SummaryMemUsage>>::new();
                    let mut max_net_data_count: u64 = 0;
                    let (left_axis, right_axis) = {
                        let sys_infos = engine_handle.get_system_infos().await;
                        for (name, frb_si) in sys_infos.iter() {
                            let cpu_usage: Vec<f32> = frb_si.iter().map(|x| x.cpu.avg).collect();
                            let mem_data: Vec<MemInfo> = frb_si.iter().map(|x| x.mem).collect();

                            cpu_axis_data.push(SeriesData {
                                data: cpu_usage,
                                name: name.to_owned(),
                            });

                            let smu = SummaryMemUsage {
                                ram: mem_data.iter().map(|mu| mu.used_mem).collect(),
                                swap: mem_data.iter().map(|mu| mu.used_swap).collect(),
                                total_ram: mem_data[0].total_mem,
                                total_swap: mem_data[0].total_swap,
                            };
                            max_net_data_count =
                                max(max_net_data_count, *smu.ram.iter().max().unwrap());
                            net_axis_data.push(SeriesData {
                                data: smu,
                                name: name.to_owned(),
                            });
                        }

                        (
                            PlotData {
                                data: cpu_axis_data,
                                y_range: 0.0..100.0f32,
                                formatter: |v| format!("{:.0}%", v),
                            },
                            PlotData {
                                data: net_axis_data,
                                y_range: 0..max_net_data_count,
                                formatter: |v| {
                                    size::Size::Kibibytes(*v)
                                        .to_string(size::Base::Base2, size::Style::Smart)
                                },
                            },
                        )
                    };

                    let plot = fb.get_backend().unwrap().into_drawing_area();

                    let plot = match layout {
                        Layout::Horizontal => plot.margin(y + 2, 2, 2, (fb.width() / 2) as u32 + 2),
                        Layout::Vertical => {
                            plot.margin(y + 2, ((fb.height() - y as usize) / 2) as u32 + 2, 2, 2)
                        }
                    };
                    helpers::plot_data(&plot, &WHITE, &mut color_index, left_axis, right_axis);
                }

                {
                    let (tx_data, rx_data) =
                        engine_handle.get_net_tx_rx(&NET_REFRESH_TIMEOUT).await;
                    if !tx_data.is_empty() && !rx_data.is_empty() {
                        // Draw a network plot
                        let plot = fb.get_backend().unwrap().into_drawing_area();

                        let plot = match layout {
                            Layout::Horizontal => {
                                plot.margin(y + 2, 2, (fb.width() / 2 + 2) as u32, 2)
                            }
                            Layout::Vertical => plot.margin(
                                y + ((fb.height() - y as usize) / 2) as i32 + 2,
                                2,
                                2,
                                2,
                            ),
                        };

                        let tx_max: i64 = *tx_data.iter().max().unwrap();
                        let rx_max: i64 = *rx_data.iter().max().unwrap();

                        let left_axis = PlotData {
                            data: vec![SeriesData {
                                data: tx_data,
                                name: "localhost".to_owned(),
                            }],
                            y_range: 0..tx_max,
                            formatter: |v| {
                                size::Size::Bytes(*v)
                                    .to_string(size::Base::Base2, size::Style::Smart)
                            },
                        };
                        let right_axis = PlotData {
                            data: vec![SeriesData {
                                data: rx_data,
                                name: "localhost".to_owned(),
                            }],
                            y_range: 0..rx_max,
                            formatter: |v| {
                                size::Size::Bytes(*v)
                                    .to_string(size::Base::Base2, size::Style::Smart)
                            },
                        };
                        helpers::plot_data(&plot, &YELLOW, &mut color_index, left_axis, right_axis);
                    }
                }
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

    if std::path::Path::new("/dev/fb1").exists() {
        render_screen_internal(engine_handle, Fb4Rasp::new().unwrap()).await;
    } else {
        render_screen_internal(engine_handle, CairoSvg::new(1920, 1080).unwrap()).await;
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
    engine_handle: &mut EngineHandle,
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

        let _r = engine_handle.send(EngineCmdData::Net(sd)).await;
        Ok(())
    } else {
        Err(RouterNetInfoError::Parsing)
    }
}

async fn get_router_net_stats(mut engine_handle: EngineHandle) {
    let mut router_stats = SshSession::new("192.168.1.1:2222").ok();

    let mut interval = tokio::time::interval(NET_REFRESH_TIMEOUT);
    loop {
        interval.tick().await;

        match router_stats.as_ref() {
            Some(rs) => {
                if get_router_net_data(rs, &mut engine_handle).await.is_err() {
                    router_stats = None;
                }
            }
            None => router_stats = SshSession::new("192.168.1.1:2222").ok(),
        }
    }
}

async fn update_touch_status(mut engine_handle: EngineHandle) {
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
            let _r = engine_handle.send(EngineCmdData::Touch(status)).await;
        }
    }
}

fn get_remote_sys_data(engine_handle: EngineHandle, config: config::Config) {
    use http::uri::Uri;
    enum Session {
        Unconnected(Uri),
        Connected((WsSession, Uri)),
    }

    async fn handle_session(mut session: Session, mut engine_handle: EngineHandle) {
        loop {
            // Use refrence so in case of the (hopefully) most common case (i.e. connected and got msg),
            // nothing need to be done
            match &mut session {
                Session::Unconnected(address) => match WsSession::new(address.clone()).await {
                    Ok(mut w) => match w
                        .send_text(&format!("refresh {}ms", REMOTE_REFRESH_TIMEOUT.as_millis()))
                        .await
                    {
                        Ok(()) => session = Session::Connected((w, address.clone())),
                        Err(e) => {
                            log::error!(
                                "Failed to start refresh with {} due to {:?}",
                                &address,
                                &e
                            );
                            let _ = engine_handle
                                .send(EngineCmdData::SysInfo(AnnotatedSystemInfo {
                                    source: address.host().unwrap().to_owned(),
                                    si: fb4rasp_shared::SystemInfo::default(),
                                }))
                                .await;
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        }
                    },
                    Err(e) => {
                        log::error!("Failed to connect to {} due {:?}", &address, &e);
                        let _ = engine_handle
                            .send(EngineCmdData::SysInfo(AnnotatedSystemInfo {
                                source: address.host().unwrap().to_owned(),
                                si: fb4rasp_shared::SystemInfo::default(),
                            }))
                            .await;
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                },
                Session::Connected((wss, addr)) => {
                    let _res: Result<(), String> = match wss.read_text().await {
                        Ok(Some(msg)) => {
                            use fb4rasp_shared::VectorSerde;
                            let data = fb4rasp_shared::SystemInfo::deserialize(&msg);
                            log::debug!("Received: {:?}", &data);
                            if data.is_ok() {
                                for d in data.unwrap() {
                                    // TODO: Ignore errors for now
                                    let _ = engine_handle
                                        .send(EngineCmdData::SysInfo(AnnotatedSystemInfo {
                                            source: addr.host().unwrap().to_owned(),
                                            si: d,
                                        }))
                                        .await;
                                }
                            }
                            Ok(())
                        }
                        Ok(None) => Err("Failed to receive text message".to_owned()),
                        Err(e) => {
                            session = Session::Unconnected(addr.clone());
                            Err(format!("Connection error: {:?}", &e))
                        }
                    };
                }
            }
        }
    }

    for r in config.remotes.iter() {
        let uri = Uri::builder()
            .scheme("ws")
            .authority(format!("{}:12345", &r.1.ip).as_str())
            .path_and_query("/ws/sysinfo")
            .build()
            .unwrap();
        tokio::spawn(handle_session(
            Session::Unconnected(uri),
            engine_handle.clone(),
        ));
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

    let cmd_line_opt = CmdLineOptions::from_args();
    log::debug!("Parsed cmd line parameters:\n{:#?}", &cmd_line_opt);

    let config_file = if cmd_line_opt.config.is_some() {
        config::read_toml_config(cmd_line_opt.config.unwrap()).unwrap()
    } else {
        config::Config::new()
    };

    let mut engine_handle = EngineHandle::default();
    {
        // create and add rules
        let mut powerdown_rule = Box::new(rule::AndRule::default());
        powerdown_rule.add_condition(Box::new(condition::MultiItemCondition::new(&[
            2u8, 3, 4, 6, 8,
        ])));
        powerdown_rule.add_action(Box::new(action::ShutdownAction {}));
        engine_handle.add_rule(powerdown_rule).await;

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
        engine_handle.add_rule(swap_layout_rule).await;
    }

    get_remote_sys_data(engine_handle.clone(), config_file);
    tokio::spawn(update_touch_status(engine_handle.clone()));

    tokio::select! {
        _ = {render_screen(engine_handle.clone())} => {}
        _ = {get_router_net_stats(engine_handle)} => {}
        _ = handle_ctrl_c() => {}
    };
}
