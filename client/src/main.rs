use actix::{Actor, AsyncContext, StreamHandler};
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws::{self, CloseReason};
use sysinfo::{ProcessorExt, SystemExt};

/// Define HTTP actor
struct SystemInfo {
    system: sysinfo::System,
    handling: bool,
}

impl SystemInfo {
    fn new() -> Self {
        Self {
            system: sysinfo::System::new_all(),
            handling: false,
        }
    }

    fn get_cpu_and_memory(&mut self) -> String {
        use fb4rasp_shared::{CpuUsage, MemInfo, SystemInfo, VectorSerde};

        self.system.refresh_cpu();
        self.system.refresh_memory();

        let mut cpu_usage = CpuUsage::default();
        let processors = self.system.get_processors();
        let count = processors.len();
        cpu_usage.detailed.resize(count, 0.0);
        let mut avg: f32 = 0.0;
        for (i, p) in processors.iter().enumerate() {
            let cpu_avg = p.get_cpu_usage();
            avg += cpu_avg;
            cpu_usage.detailed[i] = cpu_avg;
        }
        cpu_usage.avg = avg;

        let mem_info = MemInfo {
            used_mem: self.system.get_used_memory(),
            total_mem: self.system.get_total_memory(),
            used_swap: self.system.get_used_swap(),
            total_swap: self.system.get_total_swap(),
        };

        SystemInfo::to_json(&vec![SystemInfo {
            cpu: cpu_usage,
            mem: mem_info,
        }])
    }

    fn parse_refresh(
        &mut self,
        ctx: &mut ws::WebsocketContext<Self>,
        text: bytestring::ByteString,
    ) -> Result<(), std::io::Error> {
        if self.handling {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "refresh already in progress",
            ));
        }

        let text = text.trim();
        let refresh = "refresh";
        if text.starts_with(&refresh) && text.ends_with(&"ms") {
            let number_to_parse = &text[refresh.len()..text.len() - 2].trim();
            log::debug!("Number to parse: '{}'", &number_to_parse);
            let millis = number_to_parse.parse::<u64>().map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!("refresh timeout value failed due to {}", &e),
                )
            })?;
            self.handling = true;
            ctx.run_interval(std::time::Duration::from_millis(millis), |me, ctx| {
                log::debug!("Producing new cpu and memory data!");
                ctx.text(me.get_cpu_and_memory());
            });
            self.handling = false;
        }

        Ok(())
    }
}

impl Actor for SystemInfo {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        self.system.refresh_all();
        log::debug!("SystemInfo started...");
    }
}

/// Handler for ws::Message message
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for SystemInfo {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        // log::debug!("Handling websocket message: {:?}", &msg);
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                log::debug!("Handling ping websocket message: {:?}", msg);
                ctx.pong(&msg);
            }
            Ok(ws::Message::Text(text)) => {
                log::debug!("Handling text websocket message: {:?}", text);
                match self.parse_refresh(ctx, text) {
                    Ok(()) => log::debug!("Starting new generation of data..."),
                    Err(e) => {
                        log::error!("Failed to parse refresh data: {}", e);
                        ctx.close(Some(CloseReason {
                            code: ws::CloseCode::Error,
                            description: Some("Invalid interval".to_string()),
                        }))
                    }
                }
            }
            Ok(ws::Message::Binary(bin)) => {
                log::debug!("Handling binary websocket message: {:?}", bin);
                ctx.binary(bin);
            }
            _ => (),
        }
    }
}

async fn sysinfo(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    let resp = ws::start(SystemInfo::new(), &req, stream);
    log::debug!("Sysinfo response: {:?}", resp);
    resp
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "debug");
    }
    env_logger::Builder::from_default_env()
        .format_timestamp_millis()
        .init();

    HttpServer::new(|| App::new().route("/ws/sysinfo", web::get().to(sysinfo)))
        .bind("0.0.0.0:12345")?
        .run()
        .await
}
