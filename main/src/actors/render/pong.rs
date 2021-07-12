use display::Color;
use std::time::Duration;

use super::WhatToRender;
use display::Display;

trait RenderObject {
    fn render<DB>(&mut self, fb: &mut DB)
    where
        for<'a> DB: Display<'a>;
}

#[derive(Debug)]
struct Vector {
    x: f64,
    y: f64,
}

#[derive(Debug)]
struct Ball {
    pos: Vector,
    dir: Vector,
    radius: f64,
}

impl RenderObject for Ball {
    fn render<DB>(&mut self, fb: &mut DB)
    where
        for<'a> DB: Display<'a>,
    {
        log::debug!("Rendering the Ball at {:#?}", self);
        fb.render_circle(
            &display::Point {
                x: self.pos.x,
                y: self.pos.y,
            },
            self.radius,
        );
    }
}

#[derive(Debug)]
struct Palette {
    pos: Vector,
    width: f64,
    height: f64,
}

impl RenderObject for Palette {
    fn render<DB>(&mut self, fb: &mut DB)
    where
        for<'a> DB: Display<'a>,
    {
        log::debug!("Rendering the Palette: {:#?}", self);
        fb.render_rectangle(
            &display::Point {
                x: self.pos.x,
                y: self.pos.y,
            },
            self.width,
            self.height,
        );
    }
}

struct Game {
    ball: Ball,
    left_palette: Palette,
    right_palette: Palette,
    direction: Vector,
    speed: f64,
}

impl Game {
    fn new(max_width: f64, max_height: f64, palette_width: f64, palette_height: f64) -> Self {
        Self {
            ball: Ball {
                pos: Vector {
                    x: max_width / 2.0,
                    y: max_height / 2.0,
                },
                dir: Vector { x: 1.0, y: 1.0 },
                radius: 5.0,
            },
            left_palette: Palette {
                pos: Vector {
                    x: 10.0,
                    y: max_height / 2.0 - palette_height / 2.0,
                },
                width: palette_width,
                height: palette_height,
            },
            right_palette: Palette {
                pos: Vector {
                    x: max_width - 10.0,
                    y: max_height / 2.0 - palette_height / 2.0,
                },
                width: palette_width,
                height: palette_height,
            },
            direction: Vector { x: 1.0, y: 1.0 },
            speed: 1.0,
        }
    }

    fn next(&mut self) {}
}

impl RenderObject for Game {
    fn render<DB>(&mut self, fb: &mut DB)
    where
        for<'a> DB: Display<'a>,
    {
        log::debug!("Rendering the Game");
        self.ball.render(fb);
        self.left_palette.render(fb);
        self.right_palette.render(fb);
    }
}

pub(crate) async fn render_pong<DB>(mut fb: DB, wtrn: super::engine::WTRHandler) -> WhatToRender
where
    for<'a> DB: Display<'a>,
{
    let max_height = fb.height();
    let max_width = fb.width();
    let mut game = Game::new(max_width as f64, max_height as f64, 10.0, 30.0);
    let mut interval = tokio::time::interval(Duration::from_millis(1000));
    loop {
        fb.start();
        fb.set_font("DejaVuSansMono");
        fb.set_color(&Color {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        });
        fb.clean();
        fb.set_color(&Color {
            red: 0.9,
            green: 0.9,
            blue: 0.9,
            alpha: 1.0,
        });
        game.next();
        game.render(&mut fb);
        fb.finish();
        tokio::select! {
            _ = interval.tick() => {},
            wrt = wtrn.check() => { return wrt; }
        }
    }
}
