pub struct Fb4Rasp {
    fb: linuxfb::Framebuffer,
    mmap: memmap::MmapMut,
    original_content: Vec<u8>,
    cairo_ctx: Option<CairoCtx>,
    old_hw_cursor: Option<Vec<u8>>,
}

struct CairoCtx {
    #[allow(dead_code)]
    surface: cairo::Surface,
    context: cairo::Context,
}

#[derive(Debug)]
pub enum Error {
    FramebufferIssue,
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

impl From<linuxfb::Error> for Error {
    fn from(_err: linuxfb::Error) -> Self {
        Error::FramebufferIssue
    }
}

impl std::fmt::Debug for Fb4Rasp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fb4Rasp").field("mmap", &self.mmap).finish()
    }
}
impl Drop for Fb4Rasp {
    fn drop(&mut self) {
        self.mmap.copy_from_slice(&self.original_content);
        if self.old_hw_cursor.is_some() {
            use std::io::prelude::*;

            let filename = Self::get_hw_cursor_filename();
            let file = std::fs::OpenOptions::new().truncate(true).open(filename);
            if file.is_ok() {
                let mut file = file.unwrap();
                file.write(self.old_hw_cursor.as_ref().unwrap())
                    .expect(format!("Writing to a {} file failed", filename).as_str());
            } else {
                log::warn!("Failure to restore cursor in {}", filename);
            }
        }
    }
}

impl Fb4Rasp {
    pub fn new() -> Result<Self, Error> {
        // Instead of hardcoding the path, you could also use `Framebuffer::list()`
        // to find paths to available devices.
        let fb = linuxfb::Framebuffer::new("/dev/fb0")?;

        log::debug!("Size in pixels: {:?}", fb.get_size());
        log::debug!("Bytes per pixel: {:?}", fb.get_bytes_per_pixel());
        log::debug!("Physical size in mm: {:?}", fb.get_physical_size());

        log::debug!("Pixel layout: {:?}", fb.get_pixel_layout());

        let width = fb.get_size().0 as i32;
        let height = fb.get_size().1 as i32;
        log::debug!("Size in pixels: w: {}, h: {}", width, height);

        let mmap = fb.map()?;
        let original_content = mmap.to_vec();

        let mut old_hw_cursor: Option<Vec<u8>> = None;
        {
            use std::io::prelude::*;

            let filename = Self::get_hw_cursor_filename();
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(filename);
            if file.is_ok() {
                let mut file = file.unwrap();
                let mut data = Vec::new();
                if file.read_to_end(&mut data).is_ok() {
                    old_hw_cursor = Some(data);
                }
                file.seek(std::io::SeekFrom::Start(0))
                    .expect(format!("Seeking in a {} file failed", filename).as_str());
                file.write(&[0])
                    .expect(format!("Writing to a {} file failed", filename).as_str());
            } else {
                match file.err().unwrap().kind() {
                    std::io::ErrorKind::PermissionDenied => log::info!(
                        "Failed to disable hw cursor, not enough permissions to modify {}?",
                        filename
                    ),
                    _ => log::info!("Failure to access {}", filename),
                }
            }
        }

        Ok(Fb4Rasp {
            fb,
            mmap,
            original_content,
            cairo_ctx: None,
            old_hw_cursor,
        })
    }

    fn get_hw_cursor_filename() -> &'static str {
        "/sys/class/graphics/fbcon/cursor_blink"
    }

    pub fn width(&self) -> usize {
        self.fb.get_size().0 as usize
    }

    pub fn height(&self) -> usize {
        self.fb.get_size().1 as usize
    }

    pub fn clean(&mut self) {
        // Retrieve a slice for the current backbuffer:
        let frame: &mut [u8] = &mut self.mmap[..];

        // Writing byte-wise is neither very efficient, nor convenient.
        // To write whole pixels, we can cast our buffer to the right
        // format (u32 in this case):
        let (prefix, pixels, suffix) = unsafe { frame.align_to_mut::<u32>() };

        // Since we are using a type that can hold a whole pixel, it should
        // always align nicely.
        // Thus there is no prefix or suffix here:
        assert_eq!(prefix.len(), 0);
        assert_eq!(suffix.len(), 0);

        // Now we can start filling the pixels:
        let width = self.fb.get_size().0 as usize;
        let height = self.fb.get_size().1 as usize;
        for y in 0..height {
            for x in 0..width {
                pixels[x + y * width] = 0xFF000000;
            }
        }
    }

    fn is_inside(&self, pt: &Point) -> bool {
        pt.x < self.width() as f64 && pt.y < self.height() as f64
    }

    pub fn start(&mut self) {
        let width = self.width() as i32;
        let height = self.height() as i32;
        // Retrieve a slice for the current backbuffer:
        let frame: &mut [u8] = &mut self.mmap[..];

        let surface = unsafe {
            let color_format = 0/*CAIRO_FORMAT_ARGB32*/;
            let stride = cairo_sys::cairo_format_stride_for_width(color_format, width);
            log::debug!("Used stride for cairo: {}", stride);
            cairo::Surface::from_raw_none(cairo_sys::cairo_image_surface_create_for_data(
                frame.as_mut_ptr(),
                color_format,
                width,
                height,
                stride,
            ))
        };

        let context = cairo::Context::new(&surface);
        self.cairo_ctx = Some(CairoCtx { surface, context });
    }

    pub fn started(&self) -> bool {
        self.cairo_ctx.is_some()
    }

    pub fn set_color(&mut self, color: &Color) {
        if !self.started() {
            return;
        }

        let context = &self.cairo_ctx.as_ref().unwrap().context;
        context.set_source_rgba(color.red, color.green, color.blue, color.alpha);
    }

    pub fn render_text(&mut self, r#where: &Point, what: &str) {
        if !self.started() {
            return;
        }

        if !self.is_inside(r#where) {
            return;
        }

        let context = &self.cairo_ctx.as_ref().unwrap().context;
        context.move_to(r#where.x, r#where.y);
        context.show_text(what);
    }

    pub fn set_font(&mut self, name: &str) {
        if !self.started() {
            return;
        }

        let context = &self.cairo_ctx.as_ref().unwrap().context;
        let font =
            cairo::FontFace::toy_create(name, cairo::FontSlant::Italic, cairo::FontWeight::Normal);
        context.set_font_face(&font);
    }

    pub fn set_font_size(&mut self, size: f64) {
        if !self.started() {
            return;
        }

        let context = &self.cairo_ctx.as_ref().unwrap().context;
        context.set_font_size(size);
    }

    pub fn finish(&mut self) {
        self.cairo_ctx = None;
    }
}

/*

{
    // Retrieve a slice for the current backbuffer:
    let frame: &mut [u8] = &mut buffer[..];

    let surface = unsafe {
        let stride = cairo_sys::cairo_format_stride_for_width(color_format, width);
        log::debug!("Used stride for cairo: {}", stride);
        cairo::Surface::from_raw_none(cairo_sys::cairo_image_surface_create_for_data(
            frame.as_mut_ptr(),
            color_format,
            width,
            height,
            stride,
        ))
    };

    let context = cairo::Context::new(&surface);
    context.set_source_rgba(1.0, 0.0, 0.0, 1.0);
    context.move_to(0.0, 0.0);
    context.line_to(480.0, 320.0);
    context.move_to(0.0, 320.0);
    context.line_to(480.0, 0.0);
    context.set_line_width(11.0);
    context.stroke();
    context.move_to(100.0, 100.0);
    context.set_source_rgba(0.0, 0.0, 1.0, 1.0);
    let font = cairo::FontFace::toy_create(
        "DejaVu Sans",
        cairo::FontSlant::Italic,
        cairo::FontWeight::Normal,
    );
    context.set_font_face(&font);
    context.set_font_size(32.0);
    context.show_text("WOOOOOOOORRRRKKKK!!!!");
}
    */
