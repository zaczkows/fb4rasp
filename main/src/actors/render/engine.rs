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

// What every rendering engine need to implement
trait Renderer {}

// Input data for renderer, e.g. pins user touched
pub enum Input {
    STOP, // Received CTRL+C
}

pub enum Data {}

// Rendering engine responsible for receiving of data and screen refreshing
pub struct Engine {
    input: mpsc::Receiver<Input>,
    data: mpsc::Receiver<Data>,
    renderer: Box<dyn Renderer>,
}

impl Engine {
    fn new(input: mpsc::Receiver<Input>, data: mpsc::Receiver<Data>) -> Self {
        Self {
            input,
            data,
            renderer: Box::new(),
        }
    }

    fn run(&mut self, wtr_receiver: spmc::Receiver<WhatToRender>) {
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
}
