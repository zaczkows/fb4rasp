pub mod action;
pub mod condition;
pub mod engine;
mod input;
pub mod ring_buffer;
pub mod rule;
pub mod session;

pub use engine::Engine;
pub use ring_buffer::FixedRingBuffer;

pub struct Fb4Rasp {
    fb: linuxfb::Framebuffer,
    mmap: memmap::MmapMut,
    original_content: Vec<u8>,
    cairo_ctx: Option<CairoCtx>,
    old_hw_cursor: Option<Vec<u8>>,
    ev_devices: Option<Vec<evdev::Device>>,
    touch_calibration: FbTouchCalibration,
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

#[derive(Debug)]
pub enum EventType {
    Touched,
}

#[derive(Debug)]
pub struct Event {
    pub what: EventType,
    pub position: Point,
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
            let file = std::fs::OpenOptions::new().write(true).open(filename);
            if let Ok(mut file) = file {
                file.write_all(self.old_hw_cursor.as_ref().unwrap())
                    .unwrap_or_else(|_| panic!("Writing to a {} file failed", filename));
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
            if let Ok(mut file) = file {
                let mut data = Vec::new();
                if file.read_to_end(&mut data).is_ok() {
                    old_hw_cursor = Some(data);
                }
                file.seek(std::io::SeekFrom::Start(0))
                    .expect(format!("Seeking in a {} file failed", filename).as_str());
                file.write_all(&[0])
                    .unwrap_or_else(|_| panic!("Writing to a {} file failed", filename));
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
            ev_devices: None,
            touch_calibration: FbTouchCalibration::new(238, 3996, 3931, 173, true),
        })
    }

    pub fn width(&self) -> usize {
        self.fb.get_size().0 as usize
    }

    pub fn height(&self) -> usize {
        self.fb.get_size().1 as usize
    }

    pub fn bytes_per_pixel(&self) -> usize {
        self.fb.get_bytes_per_pixel() as usize
    }

    fn clean_int<T: std::convert::From<u8>>(&mut self) {
        // Retrieve a slice for the current backbuffer:
        let frame: &mut [u8] = &mut self.mmap[..];

        // Writing byte-wise is neither very efficient, nor convenient.
        // To write whole pixels, we can cast our buffer to the right format:
        let (prefix, pixels, suffix) = unsafe { frame.align_to_mut::<T>() };

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
                pixels[x + y * width] = T::from(0);
            }
        }
    }

    pub fn clean(&mut self) {
        let bpp = self.bytes_per_pixel();
        if bpp == 2 {
            self.clean_int::<u16>();
        } else {
            self.clean_int::<u32>();
        }
    }

    pub fn start(&mut self) {
        let width = self.width() as i32;
        let height = self.height() as i32;
        let bpp = self.bytes_per_pixel();
        // Retrieve a slice for the current backbuffer:
        let frame: &mut [u8] = &mut self.mmap[..];

        let surface = unsafe {
            let color_format = if bpp == 2 {
                4 /*CAIRO_FORMAT_RGB16_565*/
            } else {
                0 /*CAIRO_FORMAT_ARGB32*/
            };
            let stride = cairo_sys::cairo_format_stride_for_width(color_format, width);
            // log::debug!("Used stride for cairo: {}", stride);
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

    pub fn cairo_context(&self) -> Option<&cairo::Context> {
        if let Some(ctx) = self.cairo_ctx.as_ref() {
            Some(&ctx.context)
        } else {
            None
        }
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
            cairo::FontFace::toy_create(name, cairo::FontSlant::Normal, cairo::FontWeight::Normal);
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

    pub fn init_events(&mut self) {
        let devices = evdev::enumerate();
        if !devices.is_empty() {
            for device in devices.iter() {
                log::debug!("Found input devices: {:?}", device);
            }
            self.ev_devices = Some(devices);
        }
    }

    pub fn get_events(&mut self) -> Vec<Event> {
        struct TempPos {
            x: Option<i32>,
            y: Option<i32>,
        }

        let mut positions = vec![];
        let calibration = self.touch_calibration;
        if let Some(devices) = &mut self.ev_devices {
            for device in devices.iter_mut() {
                let events = &mut device.events();
                match events {
                    Ok(raw_events) => {
                        let mut pos = TempPos { x: None, y: None };
                        for event in raw_events {
                            let e: input::Event = event.into();
                            log::debug!("Raw event: {:?}", &e);
                            match e.r#type {
                                input::EvType::Unknown(_) => log::debug!("Unknown event: {:?}", &e),
                                input::EvType::Relative(_) => {
                                    log::debug!("Not supported relative event: {:?}", &e)
                                }
                                input::EvType::Absolute(a) => match a {
                                    input::Abs::ABS_X => pos.x = Some(e.value),
                                    input::Abs::ABS_Y => pos.y = Some(e.value),
                                    _ => (),
                                },
                            }

                            if pos.x.is_some() && pos.y.is_some() {
                                positions.push(Event {
                                    what: EventType::Touched,
                                    position: calibration.get_pos(&Point {
                                        x: pos.x.unwrap() as f64,
                                        y: pos.y.unwrap() as f64,
                                    }),
                                });

                                pos.x = None;
                                pos.y = None;
                            }
                        }
                    }
                    Err(e) => {
                        log::debug!("error {:?} ", e);
                    }
                }
            }
        }

        positions
    }
}

#[derive(Debug, Clone, Copy)]
struct FbTouchCalibration {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
    swap_axes: bool,

    scale_x: f64,
    scale_y: f64,
}

impl FbTouchCalibration {
    fn new(min_x: isize, max_x: isize, min_y: isize, max_y: isize, swap_axes: bool) -> Self {
        Self {
            min_x: min_x as f64,
            max_x: max_x as f64,
            min_y: min_y as f64,
            max_y: max_y as f64,
            swap_axes,
            scale_x: 320.0 / (max_x - min_x) as f64,
            scale_y: 480.0 / (max_y - min_y) as f64,
        }
    }

    fn get_pos(&self, pos: &Point) -> Point {
        let x = (pos.x - self.min_x) * self.scale_x;
        let y = (pos.y - self.min_y) * self.scale_y;
        if self.swap_axes {
            Point { x: y, y: x }
        } else {
            Point { x, y }
        }
    }
}

impl Fb4Rasp {
    /** PRIVATE PART **/

    fn get_hw_cursor_filename() -> &'static str {
        "/sys/class/graphics/fbcon/cursor_blink"
    }

    // fn is_inside(&self, pt: &Point) -> bool {
    //     pt.x < self.width() as f64 && pt.y < self.height() as f64
    // }
}

pub fn get_cpu_temperature() -> f32 {
    use std::io::prelude::*;

    let filename = "/sys/class/thermal/thermal_zone0/temp";
    let file = std::fs::File::open(filename);
    if let Ok(mut file) = file {
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_ok() {
            let contents = contents.trim();
            return contents.parse::<f32>().unwrap() / 1000.0;
        }
    }
    log::warn!("Failure to get CPU temperature from {}", filename);
    0.0
}
