use display::{Color, Display, Point};
use engine::{
    engine::{AnnotatedSystemInfo, EngineCmdData},
    params::Layout,
    EngineHandle,
};
use fb4rasp_shared::{CpuUsage, MemInfo, SystemInfo};
use rand::{distributions::Distribution, SeedableRng};
use std::cmp::max;
use sysinfo::{ProcessorExt, SystemExt};

use crate::timeouts::{DRAW_REFRESH_TIMEOUT, NET_REFRESH_TIMEOUT};
use crate::{
    actors::render::WhatToRender,
    helpers::{PlotData, SeriesData, SummaryMemUsage},
};

pub(crate) async fn render_time_cpu_net<DB>(
    mut engine_handle: EngineHandle,
    mut fb: DB,
    wtrn: crate::actors::render::engine::WTRHandler,
) -> WhatToRender
where
    for<'a> DB: Display<'a>,
{
    fn print_touch_status(ts: &adafruit_mpr121::Mpr121TouchStatus) -> String {
        let mut status = String::new();
        let mut separator = "";
        for i in
            adafruit_mpr121::Mpr121TouchStatus::first()..=adafruit_mpr121::Mpr121TouchStatus::last()
        {
            if ts.touched(i) {
                status += separator;
                status += &format!("{}", i);
                separator = ", ";
            }
        }

        status
    }

    let mut x: i32;
    let mut y: i32;

    fb.init_events();

    let dist_uni = rand::distributions::Uniform::from(0..20);
    let mut rng = rand::rngs::SmallRng::from_entropy();
    let mut system = sysinfo::System::new_all();

    // First we update all information of our system struct.
    system.refresh_all();

    let mut screensaver: usize = 0;
    let mut shift = 0;

    let mut interval = tokio::time::interval(DRAW_REFRESH_TIMEOUT);
    loop {
        system.refresh_cpu();
        system.refresh_memory();

        if screensaver == 33 {
            shift = dist_uni.sample(&mut rng);
            screensaver = 0;
        } else {
            screensaver += 1;
        }

        x = shift;
        y = 16;

        // Loop initialization
        let local_time = chrono::Local::now();
        let mut cpu_usage = CpuUsage::default();
        let mut cpu_info_str = String::new();
        {
            let processors = system.get_processors();
            let count = processors.len();
            cpu_usage.detailed.resize(count, 0.0);
            let mut avg: f32 = 0.0;
            let mut separator = "";
            for (i, p) in processors.iter().enumerate() {
                let p_usage = p.get_cpu_usage();
                cpu_info_str.push_str(&format!("{}{:>2.0}", separator, p_usage));
                separator = ", ";
                cpu_usage.detailed[i] = p_usage;
                avg += p_usage;
            }
            cpu_usage.avg = avg / count as f32;
        }

        let mem_info = MemInfo {
            used_mem: system.get_used_memory(),
            total_mem: system.get_total_memory(),
            used_swap: system.get_used_swap(),
            total_swap: system.get_total_swap(),
        };

        let avg_cpu_usage = cpu_usage.avg;
        let _ = engine_handle
            .send(EngineCmdData::SysInfo(AnnotatedSystemInfo {
                source: engine::engine::DEFAULT_HOST.to_owned(),
                si: SystemInfo {
                    cpu: cpu_usage,
                    mem: mem_info,
                },
            }))
            .await;

        let layout = engine_handle.get_main_layout().await;
        let touch_status = engine_handle.touch_info().await;
        let (prev, last) = engine_handle.last_net_info().await;
        let sys_infos = engine_handle.get_system_infos().await;
        let (tx_data, rx_data) = engine_handle.get_net_tx_rx(&NET_REFRESH_TIMEOUT).await;
        // end

        // Rendering start - no heavy operation after this!
        let rendering_time = std::time::Instant::now();
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
        fb.set_font_size(22.0);
        fb.render_text(
            &Point {
                x: x as f64,
                y: y as f64,
            },
            local_time
                .format("%a, %d.%m.%Y, %H:%M:%S")
                .to_string()
                .as_str(),
        );
        y += 20;

        fb.set_font_size(18.0);
        fb.set_color(&Color {
            red: 0xff as f64 / 256f64,
            green: 0xbf as f64 / 256f64,
            blue: 0.0,
            alpha: 1.0,
        });
        fb.render_text(
            &Point {
                x: x as f64,
                y: y as f64,
            },
            &format!(
                "CPU: {:>2.0}% [{}] ({:.1}°C)",
                avg_cpu_usage,
                &cpu_info_str,
                display::get_cpu_temperature()
            ),
        );
        y += 18;

        fb.set_color(&Color {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        });

        fb.render_text(
            &Point {
                x: x as f64,
                y: y as f64,
            },
            &format!(
                "Memory: {} / {}",
                size::Size::Kibibytes(mem_info.used_mem)
                    .to_string(size::Base::Base2, size::Style::Smart),
                size::Size::Kibibytes(mem_info.total_mem)
                    .to_string(size::Base::Base2, size::Style::Smart),
            ),
        );

        {
            y += 20;

            fb.set_font_size(14.0);
            fb.set_color(&Color {
                red: 0.5,
                green: 1.0,
                blue: 0.0,
                alpha: 1.0,
            });

            let secs = NET_REFRESH_TIMEOUT.as_secs() as i64;
            fb.render_text(
                &Point {
                    x: x as f64,
                    y: y as f64,
                },
                &format!(
                    "Bytes tx: {}, tx/s: {}",
                    size::Size::Bytes(last.tx_bytes)
                        .to_string(size::Base::Base2, size::Style::Smart),
                    size::Size::Bytes((last.tx_bytes - prev.tx_bytes) / secs)
                        .to_string(size::Base::Base2, size::Style::Smart),
                ),
            );
            y += 14;

            fb.set_color(&Color {
                red: 0.18,
                green: 0.56,
                blue: 0.83,
                alpha: 1.0,
            });
            fb.render_text(
                &Point {
                    x: x as f64,
                    y: y as f64,
                },
                &format!(
                    "Bytes rx: {}, rx/s: {}",
                    size::Size::Bytes(last.rx_bytes)
                        .to_string(size::Base::Base2, size::Style::Smart),
                    size::Size::Bytes((last.rx_bytes - prev.rx_bytes) / secs)
                        .to_string(size::Base::Base2, size::Style::Smart),
                ),
            );
        }

        {
            fb.set_font_size(10.0);
            let mut space = 0;
            for msg in touch_status {
                y += space;
                if space == 0 {
                    y += 22;
                    space = 10;
                }
                fb.render_text(
                    &Point {
                        x: x as f64,
                        y: y as f64,
                    },
                    &format!("Touched pins: {}", &print_touch_status(&msg)),
                );
            }
        }

        y += 12;

        {
            use plotters::prelude::*;

            let mut color_index: usize = 0;
            {
                let mut cpu_axis_data = Vec::<SeriesData<Vec<f32>>>::new();
                let mut net_axis_data = Vec::<SeriesData<SummaryMemUsage>>::new();
                let mut max_net_data_count: u64 = 0;
                let (left_axis, right_axis) = {
                    for (name, frb_si) in sys_infos.iter() {
                        let cpu_usage: Vec<f32> = frb_si.iter().map(|x| x.cpu.avg).collect();
                        let mem_data: Vec<MemInfo> = frb_si.iter().map(|x| x.mem).collect();

                        cpu_axis_data.push(SeriesData {
                            data: cpu_usage,
                            name: name.to_owned(),
                        });

                        let smu = SummaryMemUsage {
                            ram: mem_data.iter().map(|mu| mu.used_mem).collect(),
                            swap: mem_data.iter().map(|mu| mu.used_swap).collect(),
                            total_ram: mem_data[0].total_mem,
                            total_swap: mem_data[0].total_swap,
                        };
                        max_net_data_count =
                            max(max_net_data_count, *smu.ram.iter().max().unwrap());
                        net_axis_data.push(SeriesData {
                            data: smu,
                            name: name.to_owned(),
                        });
                    }

                    (
                        PlotData {
                            data: cpu_axis_data,
                            y_range: 0.0..100.0f32,
                            formatter: |v| format!("{:.0}%", v),
                        },
                        PlotData {
                            data: net_axis_data,
                            y_range: 0..max_net_data_count,
                            formatter: |v| {
                                size::Size::Kibibytes(*v)
                                    .to_string(size::Base::Base2, size::Style::Smart)
                            },
                        },
                    )
                };

                let plot = fb.get_backend().unwrap().into_drawing_area();

                let plot = match layout {
                    Layout::Horizontal => plot.margin(y + 2, 2, 2, (fb.width() / 2) as u32 + 2),
                    Layout::Vertical => {
                        plot.margin(y + 2, ((fb.height() - y as usize) / 2) as u32 + 2, 2, 2)
                    }
                };
                crate::helpers::plot_data(&plot, &WHITE, &mut color_index, left_axis, right_axis);
            }

            {
                if !tx_data.is_empty() && !rx_data.is_empty() {
                    // Draw a network plot
                    let plot = fb.get_backend().unwrap().into_drawing_area();

                    let plot = match layout {
                        Layout::Horizontal => plot.margin(y + 2, 2, (fb.width() / 2 + 2) as u32, 2),
                        Layout::Vertical => {
                            plot.margin(y + ((fb.height() - y as usize) / 2) as i32 + 2, 2, 2, 2)
                        }
                    };

                    let tx_max: i64 = *tx_data.iter().max().unwrap();
                    let rx_max: i64 = *rx_data.iter().max().unwrap();

                    let left_axis = PlotData {
                        data: vec![SeriesData {
                            data: tx_data,
                            name: "localhost".to_owned(),
                        }],
                        y_range: 0..tx_max,
                        formatter: |v| {
                            size::Size::Bytes(*v).to_string(size::Base::Base2, size::Style::Smart)
                        },
                    };
                    let right_axis = PlotData {
                        data: vec![SeriesData {
                            data: rx_data,
                            name: "localhost".to_owned(),
                        }],
                        y_range: 0..rx_max,
                        formatter: |v| {
                            size::Size::Bytes(*v).to_string(size::Base::Base2, size::Style::Smart)
                        },
                    };
                    crate::helpers::plot_data(
                        &plot,
                        &YELLOW,
                        &mut color_index,
                        left_axis,
                        right_axis,
                    );
                }
            }
        }

        let events = fb.get_events();
        for e in events {
            log::debug!("Events {:?}", &e);
            fb.render_text(
                &Point {
                    x: e.position.x,
                    y: e.position.y,
                },
                "X",
            );
        }

        fb.finish();
        log::debug!("Rendering time: {}us", rendering_time.elapsed().as_micros());
        // Rendering finished

        tokio::select! {
            _ = interval.tick() => {},
            wrt = wtrn.check() => { return wrt; }
        }
    }
}
