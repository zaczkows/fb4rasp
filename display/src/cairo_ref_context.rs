use crate::{Color, Display, Event, Point, TextSize};

pub struct CairoRefContext<'a> {
    context: &'a cairo::Context,
    width: usize,
    height: usize,
}

// unsafe impl Send for CairoRefContext {}

#[derive(Debug)]
pub enum CairoRefContextError {
    Cairo(String),
}

impl From<cairo::Error> for CairoRefContextError {
    fn from(err: cairo::Error) -> Self {
        CairoRefContextError::Cairo(format!("{}", err))
    }
}

impl<'a> Display<'a> for CairoRefContext<'_> {
    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn bytes_per_pixel(&self) -> usize {
        4
    }

    fn clean(&mut self) {
        self.context
            .rectangle(0.0, 0.0, self.width as f64, self.height as f64);
        let _ = self.context.fill();
    }

    fn start(&mut self) {}

    fn started(&self) -> bool {
        true
    }

    fn set_color(&mut self, color: &Color) {
        self.context
            .set_source_rgba(color.red, color.green, color.blue, color.alpha);
    }

    fn text_size(&self, what: &str) -> TextSize {
        let extents = self.context.text_extents(what).unwrap();
        TextSize {
            width: extents.width,
            height: extents.height,
        }
    }

    fn render_text(&mut self, r#where: &Point, what: &str) -> Option<TextSize> {
        self.context.move_to(r#where.x, r#where.y);
        let extents = self.context.text_extents(what).unwrap();
        let _ = self.context.show_text(what);
        Some(TextSize {
            width: extents.width,
            height: extents.height,
        })
    }

    fn render_circle(&mut self, r#where: &Point, radius: f64, fill_color: Option<&Color>) {
        self.context.arc(r#where.x, r#where.y, radius, 0.0, 360.0);
        match fill_color {
            Some(c) => {
                self.set_color(c);
                let _ = self.context.fill();
            }
            None => self.context.stroke().unwrap(),
        }
    }

    fn render_rectangle(
        &mut self,
        r#where: &Point,
        width: f64,
        height: f64,
        fill_color: Option<&Color>,
    ) {
        self.context.rectangle(r#where.x, r#where.y, width, height);
        match fill_color {
            Some(c) => {
                self.set_color(c);
                let _ = self.context.fill();
            }
            None => self.context.stroke().unwrap(),
        }
    }

    fn set_font(&mut self, name: &str) {
        let font =
            cairo::FontFace::toy_create(name, cairo::FontSlant::Normal, cairo::FontWeight::Normal)
                .unwrap();
        self.context.set_font_face(&font);
    }

    fn set_font_size(&mut self, size: f64) {
        self.context.set_font_size(size);
    }

    fn finish(&mut self) {}

    fn init_events(&mut self) {}

    fn get_events(&mut self) -> Vec<Event> {
        vec![]
    }

    type DrawingBackend = plotters_cairo::CairoBackend<'a>;
    type BackendError = plotters_cairo::CairoError;
    fn get_backend(&'a self) -> Result<Self::DrawingBackend, Self::BackendError> {
        plotters_cairo::CairoBackend::new(self.context, (self.width() as u32, self.height() as u32))
    }
}

impl<'a> CairoRefContext<'a> {
    pub fn new(
        context: &'a cairo::Context,
        width: usize,
        height: usize,
    ) -> Result<Self, CairoRefContextError> {
        Ok(Self {
            context,
            width,
            height,
        })
    }
}
