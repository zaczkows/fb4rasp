mod fb4rasp;
mod input;
mod svgb;
mod utils;

pub use crate::{fb4rasp::Fb4Rasp, svgb::CairoSvg, utils::get_cpu_temperature};

pub trait Display<'a> {
    fn width(&self) -> usize;
    fn height(&self) -> usize;
    fn bytes_per_pixel(&self) -> usize;
    fn clean(&mut self);
    fn start(&mut self);
    fn started(&self) -> bool;
    fn set_color(&mut self, color: &Color);
    fn text_size(&self, what: &str) -> TextSize;
    fn render_text(&mut self, r#where: &Point, what: &str) -> Option<TextSize>;
    fn render_circle(&mut self, r#where: &Point, radius: f64);
    fn render_rectangle(&mut self, r#where: &Point, width: f64, height: f64);
    fn set_font(&mut self, name: &str);
    fn set_font_size(&mut self, size: f64);
    fn finish(&mut self);
    fn init_events(&mut self);
    fn get_events(&mut self) -> Vec<Event>;

    type DrawingBackend: plotters_backend::DrawingBackend + Sized;
    type BackendError: std::error::Error + Send + Sync;
    fn get_backend(&'a self) -> Result<Self::DrawingBackend, Self::BackendError>;
}

#[derive(Debug, Clone)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub struct Color {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

#[derive(Debug)]
pub struct TextSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug)]
pub enum EventType {
    Touched,
}

#[derive(Debug)]
pub struct Event {
    pub what: EventType,
    pub position: Point,
}
