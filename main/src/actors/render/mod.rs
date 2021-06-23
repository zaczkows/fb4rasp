mod engine;
pub(crate) use self::engine::RendererHandle;
mod pong;
mod time_net_cpu;

#[derive(Clone, Copy)]
pub(crate) enum WhatToRender {
    SysInfo,
    Pong,
}
