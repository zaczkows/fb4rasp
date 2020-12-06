use fb4rasp;
use rand::distributions::Distribution;
use sysinfo::{ProcessorExt, SystemExt};

async fn draw_time() {
    let mut fb = fb4rasp::Fb4Rasp::new().unwrap();
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(1000));
    let mut x: i32;
    let mut x_diff = 1;
    let mut y: i32;
    let mut y_diff = 1;

    fb.init_events();

    let dist_uni = rand::distributions::Uniform::from(0..20);
    let mut rng = rand::thread_rng();
    let mut system = sysinfo::System::new_all();

    // First we update all information of our system struct.
    system.refresh_all();

    loop {
        system.refresh_cpu();
        system.refresh_memory();

        x = 0;
        y = 80 + dist_uni.sample(&mut rng);
        fb.clean();
        fb.start();
        let local_time = chrono::Local::now();
        fb.set_color(&fb4rasp::Color {
            red: 0.0,
            green: 1.0,
            blue: 0.0,
            alpha: 1.0,
        });
        fb.set_font_size(34.0);
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

        let (cpu_avg_usage, cpu_info) = {
            let processors = system.get_processors();
            let count = processors.len();
            let mut ci = String::new();
            let mut avg: f32 = 0.0;
            let mut separator = "";
            for p in processors {
                ci.push_str(&format!("{}{:.0}", separator, p.get_cpu_usage()));
                separator = ", ";
                avg += p.get_cpu_usage();
            }
            (avg / count as f32, ci)
        };

        fb.set_font_size(26.0);
        fb.render_text(
            &fb4rasp::Point {
                x: x as f64,
                y: y as f64,
            },
            &format!(
                "CPU: {:.0}% [{}] ({:.1}°C)",
                cpu_avg_usage,
                &cpu_info,
                fb4rasp::get_cpu_temperature()
            ),
        );
        y = y + 24;

        x = x + x_diff;
        y = y + y_diff;
        if x > 50 || x < 1 {
            x_diff = -x_diff;
        }
        if y > 300 || x < 10 {
            y_diff = -y_diff;
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

    tokio::select! {
        _ = draw_time() => {()}
        _ = handle_ctrl_c() => {()}
    };
}
