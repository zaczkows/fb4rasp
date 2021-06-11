use display::{Color, Display, Point};
use engine::{
    engine::{AnnotatedSystemInfo, EngineCmdData},
    params::Layout,
    EngineHandle,
};
// use rand::distributions::Distribution;
// use std::cmp::max;

// use crate::timeouts::{DRAW_REFRESH_TIMEOUT, NET_REFRESH_TIMEOUT};

trait RenderObject {
    fn render(&mut self);
}

#[derive(Default)]
struct Vector {
    x: f64,
    y: f64,
}

#[derive(Default)]
struct Ball {
    pos: Vector,
    dir: Vector,
}

impl RenderObject for Ball {
    fn render(&mut self) {}
}

#[derive(Default)]
struct Palette {
    pos: Vector,
    width: usize,
    height: usize,
}

#[derive(Default)]
struct State {
    ball: Ball,
    left_palette: Palette,
    right_palette: Palette,
    direction: Vector,
    speed: f64,
}

pub(crate) async fn render_pong<DB>(mut engine_handle: EngineHandle, mut fb: DB)
where
    for<'a> DB: Display<'a>,
{
    let mut state = State::default();
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
    loop {
        interval.tick().await;
    }
}