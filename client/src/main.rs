use actix::{Actor, StreamHandler};
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use sysinfo::{ProcessorExt, SystemExt};

/// Define HTTP actor
struct SystemInfo {
    system: sysinfo::System,
}

impl SystemInfo {
    fn new() -> Self {
        Self {
            system: sysinfo::System::new_all(),
        }
    }

    fn get_cpu_and_memory(&mut self) -> String {
        use fb4rasp_shared::{MemInfo, CpuUsage, VectorSerde, SystemInfo};

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

        let mem_info = MemInfo{ used_mem: self.system.get_used_memory(),
                                total_mem: self.system.get_total_memory(),
                                used_swap: self.system.get_used_swap(),
                                total_swap: self.system.get_total_swap()};

        SystemInfo::to_json(&vec![SystemInfo{cpu: cpu_usage, mem: mem_info}])
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
            Ok(ws::Message::Text(_text)) => {
                // log::debug!("Handling text websocket message: {:?}", text);
                ctx.text(self.get_cpu_and_memory());
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
