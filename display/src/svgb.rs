use crate::{Color, Display, Event, Point, TextSize};

pub struct CairoSvg {
    #[allow(dead_code)]
    surface: Option<cairo::SvgSurface>,
    context: Option<cairo::Context>,
    width: usize,
    height: usize,
    started: bool,
}

unsafe impl Send for CairoSvg {}

#[derive(Debug)]
pub enum CairoSvgError {
    Cairo(String),
}

impl From<cairo::Error> for CairoSvgError {
    fn from(err: cairo::Error) -> Self {
        CairoSvgError::Cairo(format!("{}", err))
    }
}

impl<'a> Display<'a> for CairoSvg {
    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn bytes_per_pixel(&self) -> usize {
        0
    }

    fn clean(&mut self) {
        assert!(self.started());
        let context = self.context.as_mut().unwrap();
        context.rectangle(0.0, 0.0, self.width as f64, self.height as f64);
        let _ = context.fill();
    }

    fn start(&mut self) {
        self.surface = Some(
            cairo::SvgSurface::new(
                self.width() as f64,
                self.height() as f64,
                Some("output/test.svg"),
            )
            .unwrap(),
        );
        self.context = Some(cairo::Context::new(self.surface.as_ref().unwrap()).unwrap());
        self.started = true;
    }

    fn started(&self) -> bool {
        self.started
    }

    fn set_color(&mut self, color: &Color) {
        assert!(self.started());
        let context = self.context.as_mut().unwrap();
        context.set_source_rgba(color.red, color.green, color.blue, color.alpha);
    }

    fn text_size(&self, what: &str) -> TextSize {
        assert!(self.started());
        let context = self.context.as_ref().unwrap();
        let extents = context.text_extents(what).unwrap();
        TextSize {
            width: extents.width,
            height: extents.height,
        }
    }

    fn render_text(&mut self, r#where: &Point, what: &str) -> Option<TextSize> {
        if !self.started() {
            return None;
        }

        assert!(self.started());
        let context = self.context.as_mut().unwrap();
        context.move_to(r#where.x, r#where.y);
        let extents = context.text_extents(what).unwrap();
        let _ = context.show_text(what);
        Some(TextSize {
            width: extents.width,
            height: extents.height,
        })
    }

    fn render_circle(&mut self, r#where: &Point, radius: f64, fill_color: Option<&Color>) {
        assert!(self.started());
        let context = self.context.as_mut().unwrap();
        context.arc(r#where.x, r#where.y, radius, 0.0, 360.0);
        match fill_color {
            Some(c) => {
                self.set_color(c);
                let context = self.context.as_mut().unwrap();
                let _ = context.fill();
            }
            None => context.stroke().unwrap(),
        }
    }

    fn render_rectangle(
        &mut self,
        r#where: &Point,
        width: f64,
        height: f64,
        fill_color: Option<&Color>,
    ) {
        assert!(self.started());
        let context = self.context.as_mut().unwrap();
        context.rectangle(r#where.x, r#where.y, width, height);
        match fill_color {
            Some(c) => {
                self.set_color(c);
                let context = self.context.as_mut().unwrap();
                let _ = context.fill();
            }
            None => context.stroke().unwrap(),
        }
    }

    fn set_font(&mut self, name: &str) {
        assert!(self.started());
        let context = self.context.as_mut().unwrap();
        let font =
            cairo::FontFace::toy_create(name, cairo::FontSlant::Normal, cairo::FontWeight::Normal)
                .unwrap();
        context.set_font_face(&font);
    }

    fn set_font_size(&mut self, size: f64) {
        assert!(self.started());
        let context = self.context.as_mut().unwrap();
        context.set_font_size(size);
    }

    fn finish(&mut self) {
        self.surface = None;
        self.context = None;
        self.started = false;
    }

    fn init_events(&mut self) {}

    fn get_events(&mut self) -> Vec<Event> {
        vec![]
    }

    type DrawingBackend = plotters_cairo::CairoBackend<'a>;
    type BackendError = plotters_cairo::CairoError;
    fn get_backend(&'a self) -> Result<Self::DrawingBackend, Self::BackendError> {
        assert!(self.started());
        let context = self.context.as_ref().unwrap();
        plotters_cairo::CairoBackend::new(&context, (self.width() as u32, self.height() as u32))
    }
}

impl CairoSvg {
    pub fn new(width: usize, height: usize) -> Result<Self, CairoSvgError> {
        Ok(Self {
            surface: None,
            context: None,
            width,
            height,
            started: false,
        })
    }
}
