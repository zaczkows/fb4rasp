use fb4rasp;
use rand::distributions::Distribution;
use std::cell::RefCell;
use sysinfo::{ProcessorExt, SystemExt};

struct SharedData {
    tx_bytes: i64,
    rx_bytes: i64,
    tx_old: i64,
    rx_old: i64,
}

const NET_REFRESH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

async fn draw_time(shared_data: &RefCell<SharedData>) {
    let mut fb = fb4rasp::Fb4Rasp::new().unwrap();
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(1000));
    let mut x: i32;
    let mut y: i32;

    fb.init_events();
    fb.set_font("DejaVuSansMono");

    let dist_uni = rand::distributions::Uniform::from(0..20);
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
        let (cpu_avg_usage, cpu_info) = {
            let processors = system.get_processors();
            let count = processors.len();
            let mut ci = String::new();
            let mut avg: f32 = 0.0;
            let mut separator = "";
            for p in processors {
                ci.push_str(&format!("{}{:>2.0}", separator, p.get_cpu_usage()));
                separator = ", ";
                avg += p.get_cpu_usage();
            }
            (avg / count as f32, ci)
        };

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
            let sd = shared_data.borrow();
            fb.render_text(
                &fb4rasp::Point {
                    x: x as f64,
                    y: y as f64,
                },
                &format!(
                    "Bytes tx: {}, rx: {}",
                    size::Size::Bytes(sd.tx_bytes).to_string(size::Base::Base2, size::Style::Smart),
                    size::Size::Bytes(sd.rx_bytes).to_string(size::Base::Base2, size::Style::Smart),
                ),
            );
        }
        y = y + 26;
        {
            let sd = shared_data.borrow();
            let secs = NET_REFRESH_TIMEOUT.as_secs() as i64;
            fb.render_text(
                &fb4rasp::Point {
                    x: x as f64,
                    y: y as f64,
                },
                &format!(
                    "Bytes tx/s: {}, rx/s: {}",
                    size::Size::Bytes((sd.tx_bytes - sd.tx_old) / secs)
                        .to_string(size::Base::Base2, size::Style::Smart),
                    size::Size::Bytes((sd.rx_bytes - sd.rx_old) / secs)
                        .to_string(size::Base::Base2, size::Style::Smart),
                ),
            );
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

fn parse_xx_to_i64(s: &str) -> Option<i64> {
    s.split(|c| c == ' ' || c == '\n')
        .nth(0)
        .unwrap_or("")
        .parse::<i64>()
        .ok()
}

async fn get_router_net_stats(shared_data: &RefCell<SharedData>) {
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
                let mut sd = shared_data.borrow_mut();
                sd.tx_old = sd.tx_bytes;
                sd.rx_old = sd.rx_bytes;
                sd.tx_bytes = tx_value;
                sd.rx_bytes = rx_value;
                log::debug!(
                    "Current usage is tx: {}, rx: {}",
                    size::Size::Bytes(tx_value).to_string(size::Base::Base2, size::Style::Smart),
                    size::Size::Bytes(rx_value).to_string(size::Base::Base2, size::Style::Smart),
                );
            }
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

    let shared_data = std::cell::RefCell::new(SharedData {
        tx_bytes: 0,
        rx_bytes: 0,
        tx_old: 0,
        rx_old: 0,
    });
    tokio::select! {
        _ = draw_time(&shared_data) => {()}
        _ = get_router_net_stats(&shared_data) => {()}
        _ = handle_ctrl_c() => {()}
    };
}
