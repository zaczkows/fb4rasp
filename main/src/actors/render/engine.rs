use display::{CairoSvg, Fb4Rasp};
use engine::EngineHandle;
use tokio::sync::mpsc;

#[derive(Clone)]
pub(crate) struct RendererHandle {
    tx: mpsc::Sender<RendererCommands>,
}

impl RendererHandle {
    pub fn new(engine_handle: EngineHandle) -> Self {
        let (tx, rx) = mpsc::channel(100);

        let renderer = Renderer::new(rx, engine_handle);
        tokio::spawn(start_renderer(renderer));

        Self { tx }
    }
}

struct Renderer {
    rx: mpsc::Receiver<RendererCommands>,
    engine_handle: EngineHandle,
}

enum RendererCommands {}

impl Renderer {
    fn new(rx: mpsc::Receiver<RendererCommands>, engine_handle: EngineHandle) -> Self {
        Self { rx, engine_handle }
    }

    fn handle_message(&mut self, _msg: RendererCommands) {}
}

async fn start_renderer(mut renderer: Renderer) {
    render_screen(renderer.engine_handle.clone()).await;
    while let Some(msg) = renderer.rx.recv().await {
        renderer.handle_message(msg);
    }
}

async fn render_screen(engine_handle: EngineHandle) {
    if std::path::Path::new("/dev/fb1").exists() {
        tokio::spawn(super::time_net_cpu::render_time_cpu_net(
            engine_handle,
            Fb4Rasp::new().unwrap(),
        ));
    } else {
        tokio::spawn(super::time_net_cpu::render_time_cpu_net(
            engine_handle,
            CairoSvg::new(1920, 1080).unwrap(),
        ));
    }
}
