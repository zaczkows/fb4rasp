use std::{
    future::Future,
    pin::Pin,
    sync::{atomic::AtomicBool, Arc, Mutex},
    task::{Context, Poll},
    thread::{self, JoinHandle},
};

use super::WhatToRender;
use display::{CairoSvg, Fb4Rasp};
use engine::EngineHandle;
use tokio::sync::mpsc;

struct WTRReceiverData {
    receiver: spmc::Receiver<WhatToRender>,
    result: Mutex<Option<WhatToRender>>,
    thread_started: AtomicBool,
    thread: Mutex<Option<JoinHandle<()>>>,
}

/*
 * Not needed for now since program is either stopped or switch to different
 * rendering (which stops the thread).
impl Drop for WTRReceiverData {
    fn drop(&mut self) {
        match self.thread.lock() {
            Some(jt) => jt.kill(),
            None => (),
        }
    }
}
*/

pub(crate) struct WTRReceiver {
    data: Arc<WTRReceiverData>,
}

impl WTRReceiver {
    fn new(receiver: spmc::Receiver<WhatToRender>) -> Self {
        Self {
            data: Arc::new(WTRReceiverData {
                receiver,
                result: Mutex::new(None),
                thread_started: AtomicBool::new(false),
                thread: Mutex::new(None),
            }),
        }
    }
}

impl Clone for WTRReceiver {
    fn clone(&self) -> Self {
        let me = Self {
            data: Arc::clone(&self.data),
        };
        *me.data.result.lock().unwrap() = None;
        me
    }
}

pub(crate) struct WTRHandler {
    receiver: WTRReceiver,
}

impl WTRHandler {
    fn new(receiver: spmc::Receiver<WhatToRender>) -> Self {
        Self {
            receiver: WTRReceiver::new(receiver),
        }
    }

    pub(crate) fn check(&self) -> WTRReceiver {
        self.receiver.clone()
    }
}

impl Future for WTRReceiver {
    type Output = WhatToRender;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<<Self as Future>::Output> {
        match *self.data.result.lock().unwrap() {
            None => {
                if self
                    .data
                    .thread_started
                    .compare_exchange(
                        false,
                        true,
                        std::sync::atomic::Ordering::Acquire,
                        std::sync::atomic::Ordering::Acquire,
                    )
                    .is_ok()
                {
                    let data = Arc::clone(&self.data);
                    let waker = context.waker().clone();
                    log::warn!("Creating new thread!");
                    *self.data.thread.lock().unwrap() = Some(thread::spawn(move || {
                        match data.receiver.recv() {
                            Ok(v) => *data.result.lock().unwrap() = Some(v),
                            Err(e) => log::error!("Failed to receive data: {}", e),
                        }
                        waker.wake();
                    }));
                }
                Poll::Pending
            }
            Some(wtr) => Poll::Ready(wtr),
        }
    }
}

#[derive(Clone)]
pub(crate) struct RendererHandle {
    tx: mpsc::Sender<RendererCommands>,
}

impl RendererHandle {
    pub fn new(engine_handle: EngineHandle, wtr_receiver: spmc::Receiver<WhatToRender>) -> Self {
        let (tx, rx) = mpsc::channel(100);

        let renderer = Renderer::new(rx, engine_handle);
        tokio::spawn(start_renderer(renderer, wtr_receiver));

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

async fn start_renderer(mut renderer: Renderer, wtr_receiver: spmc::Receiver<WhatToRender>) {
    tokio::spawn(render_screen(renderer.engine_handle.clone(), wtr_receiver));
    while let Some(msg) = renderer.rx.recv().await {
        renderer.handle_message(msg);
    }
}

async fn render_screen(engine_handle: EngineHandle, wtr_receiver: spmc::Receiver<WhatToRender>) {
    let use_framebuffer = std::path::Path::new("/dev/fb1").exists();
    let mut renderer = WhatToRender::SysInfo;

    loop {
        let receiver = WTRHandler::new(wtr_receiver.clone());
        let handle: tokio::task::JoinHandle<WhatToRender> = match renderer {
            WhatToRender::SysInfo => {
                let engine_handle = engine_handle.clone();
                if use_framebuffer {
                    tokio::spawn(super::time_net_cpu::render_time_cpu_net(
                        engine_handle,
                        Fb4Rasp::new().unwrap(),
                        receiver,
                    ))
                } else {
                    tokio::spawn(super::time_net_cpu::render_time_cpu_net(
                        engine_handle,
                        CairoSvg::new(1920, 1080).unwrap(),
                        receiver,
                    ))
                }
            }
            WhatToRender::Pong => {
                if use_framebuffer {
                    tokio::spawn(super::pong::render_pong(Fb4Rasp::new().unwrap(), receiver))
                } else {
                    tokio::spawn(super::pong::render_pong(
                        CairoSvg::new(1920, 1080).unwrap(),
                        receiver,
                    ))
                }
            }
        };

        match handle.await {
            Err(_) => break,
            Ok(r) => renderer = r,
        }
    }
}
