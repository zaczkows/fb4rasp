use display::{CairoSvg, Fb4Rasp};
use engine::{
    action, condition,
    engine::{AnnotatedSystemInfo, EngineCmdData},
    params::{Layout, Parameters},
    rule, EngineHandle,
};
use fb4rasp_shared::NetworkInfo;
use session::{SshSession, WsSession};
use std::path::PathBuf;
use structopt::StructOpt;

mod config;
mod helpers;
mod render;
mod timeouts;
use timeouts::{NET_REFRESH_TIMEOUT, REMOTE_REFRESH_TIMEOUT, TOUCH_REFRESH_TIMEOUT};

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

async fn render_screen(engine_handle: EngineHandle) {
    if std::path::Path::new("/dev/fb1").exists() {
        render::render_time_cpu_net(engine_handle, Fb4Rasp::new().unwrap()).await;
    } else {
        render::render_time_cpu_net(engine_handle, CairoSvg::new(1920, 1080).unwrap()).await;
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
