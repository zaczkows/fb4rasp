use fb4rasp;

async fn draw_time() {
    let mut fb = fb4rasp::Fb4Rasp::new().unwrap();
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(1000));
    loop {
        fb.clean();
        fb.start();
        let local_time: chrono::DateTime<chrono::Local> = chrono::Local::now();
        fb.render_text(
            &fb4rasp::Point { x: 10.0, y: 100.0 },
            local_time.format("%e.%m.%Y %k:%M:%S").to_string().as_str(),
        );
        fb.finish();
        // std::thread::sleep(std::time::Duration::from_millis(1000));
        interval.tick().await;
    }
}

async fn handle_ctrl_c() {
    tokio::signal::ctrl_c().await;
    log::info!("Received CTRL_C signal, exiting...");
}

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    tokio::select! {
        _ = draw_time() => {()}
        _ = handle_ctrl_c() => {()}
    };
}
