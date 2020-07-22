use fb4rasp;

async fn draw_time() {
    let mut fb = fb4rasp::Fb4Rasp::new().unwrap();
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(1000));
    let mut x: i32 = 10;
    let mut x_diff = 1;
    let mut y: i32 = 100;
    let mut y_diff = 1;

    fb.init_events();

    loop {
        fb.clean();
        fb.start();
        let local_time = time::OffsetDateTime::now_local();
        fb.set_color(&fb4rasp::Color {
            red: x as f64 / 480.0,
            green: y as f64 / 320.0,
            blue: 0.0,
            alpha: 1.0,
        });
        fb.set_font_size(34.0);
        fb.render_text(
            &fb4rasp::Point {
                x: x as f64,
                y: y as f64,
            },
            local_time.format("%d.%m.%Y %H:%M:%S").to_string().as_str(),
        );
        fb.set_font_size(26.0);
        fb.render_text(
            &fb4rasp::Point {
                x: x as f64,
                y: (y + 26) as f64,
            },
            format!("CPU Temp: {:.1}°C", fb4rasp::get_cpu_temperature())
                .to_string()
                .as_str(),
        );
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
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();

    tokio::select! {
        _ = draw_time() => {()}
        _ = handle_ctrl_c() => {()}
    };
}
