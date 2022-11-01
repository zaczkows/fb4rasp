use crossbeam::channel;

use fb4rasp_shared::{notify::NotifyData, RenderState};

#[cfg(feature = "emulation")]
use display::{cairo_ref_context::CairoRefContext, gtk};

#[cfg(feature = "emulation")]
pub fn render_in_emulator(tx: channel::Sender<NotifyData>, rx: channel::Receiver<NotifyData>) {
    use std::sync::Arc;
    gtk::start(
        tx,
        rx,
        Arc::new(move |state, context, width, height| {
            // log::debug!("Called with width={}, height={}", width, height);
            crate::time_net_cpu::render_time_cpu_net(
                state,
                CairoRefContext::new(context, width as usize, height as usize).unwrap(),
            );
        }),
    );
}

#[cfg(not(feature = "emulation"))]
pub fn render_in_pi(_tx: channel::Sender<NotifyData>, _rx: channel::Receiver<NotifyData>) {}

pub fn main_render(tx: channel::Sender<NotifyData>, rx: channel::Receiver<NotifyData>) {
    #[cfg(feature = "emulation")]
    render_in_emulator(tx, rx);

    #[cfg(not(feature = "emulation"))]
    render_in_pi(tx, rx);
}
