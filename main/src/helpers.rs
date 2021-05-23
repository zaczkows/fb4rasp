use plotters::coord::ranged1d::{AsRangedCoord, ValueFormatter};
use plotters::coord::Shift;
use plotters::prelude::*;
use plotters::style::text_anchor;
use std::cmp::max;

#[derive(Default)]
pub struct SummaryMemUsage {
    pub ram: Vec<u64>,
    pub swap: Vec<u64>,
    pub total_ram: u64,
    pub total_swap: u64,
}

impl IntoIterator for SummaryMemUsage {
    type Item = u64;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.ram.into_iter()
    }
}

pub struct SeriesData<T> {
    pub data: T,
    pub name: String,
}

pub trait Countable {
    fn count(&self) -> usize;
}

impl Countable for SummaryMemUsage {
    fn count(&self) -> usize {
        self.ram.len()
    }
}

impl<T> Countable for Vec<T> {
    fn count(&self) -> usize {
        self.len()
    }
}

impl<T> Countable for &SeriesData<T>
where
    T: Countable + IntoIterator,
{
    fn count(&self) -> usize {
        self.data.count()
    }
}

impl<T> IntoIterator for SeriesData<T>
where
    T: IntoIterator,
{
    type Item = <T as IntoIterator>::Item;
    type IntoIter = <T as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

pub struct PlotData<T>
where
    T: IntoIterator,
{
    pub data: Vec<SeriesData<T>>,
    pub y_range: std::ops::Range<<SeriesData<T> as IntoIterator>::Item>,
    pub formatter: fn(&<SeriesData<T> as IntoIterator>::Item) -> String,
}

pub fn plot_data<T, V>(
    plot: &DrawingArea<plotters_cairo::CairoBackend<'_>, Shift>,
    text_color: &RGBColor,
    color_index: &mut usize,
    left_axis: PlotData<T>,
    right_axis: PlotData<V>,
) where
    std::ops::Range<<SeriesData<T> as IntoIterator>::Item>:
        AsRangedCoord<Value = <SeriesData<T> as IntoIterator>::Item>,
    std::ops::Range<<SeriesData<V> as IntoIterator>::Item>:
        AsRangedCoord<Value = <SeriesData<V> as IntoIterator>::Item>,
    <std::ops::Range<<SeriesData<T> as IntoIterator>::Item> as AsRangedCoord>::CoordDescType:
        ValueFormatter<<SeriesData<T> as IntoIterator>::Item>,
    <std::ops::Range<<SeriesData<V> as IntoIterator>::Item> as AsRangedCoord>::CoordDescType:
        ValueFormatter<<SeriesData<V> as IntoIterator>::Item>,
    T: Countable + IntoIterator,
    SeriesData<T>: IntoIterator,
    <SeriesData<T> as IntoIterator>::Item: Clone + 'static,
    V: Countable + IntoIterator,
    SeriesData<V>: IntoIterator,
    <SeriesData<V> as IntoIterator>::Item: Clone + 'static,
{
    fn max_axis_count<X>(d: X) -> usize
    where
        X: Iterator,
        <X as Iterator>::Item: Countable,
    {
        let mut max_left_count: usize = 0;
        d.for_each(|x| max_left_count = max(max_left_count, x.count()));
        max_left_count
    }

    let mut chart = ChartBuilder::on(plot)
        .y_label_area_size(4)
        .right_y_label_area_size(4)
        .build_cartesian_2d(
            0..max_axis_count(left_axis.data.iter()),
            left_axis.y_range.clone(),
        )
        .unwrap()
        .set_secondary_coord(
            0..max_axis_count(right_axis.data.iter()),
            right_axis.y_range.clone(),
        );

    let series_count = left_axis.data.len();
    let should_draw_legend = series_count > 1;
    left_axis.data.into_iter().for_each(|series| {
        let name = series.name.to_owned();
        let ci = *color_index;
        *color_index += 1;

        let ls = LineSeries::new(
            series.into_iter().enumerate().map(|(i, v)| (i, v)),
            &Palette99::pick(ci),
        );
        let line_series = chart.draw_series(ls).unwrap();

        if should_draw_legend {
            line_series.label(name).legend(move |(x, y)| {
                PathElement::new(vec![(x - 50, y - 5), (x - 30, y - 5)], &Palette99::pick(ci))
            });
        }
    });

    right_axis.data.into_iter().for_each(|series| {
        chart
            .draw_secondary_series(LineSeries::new(
                series.into_iter().enumerate().map(|(i, v)| (i, v)),
                &Palette99::pick(*color_index),
            ))
            .unwrap();
        *color_index += 1;
    });

    let labels_font = TextStyle {
        font: FontDesc::new(FontFamily::Monospace, 12.0, FontStyle::Normal),
        color: plotters_backend::BackendColor {
            alpha: 1.0,
            rgb: text_color.rgb(),
        },
        pos: text_anchor::Pos::new(text_anchor::HPos::Left, text_anchor::VPos::Center),
    };

    chart
        .configure_mesh()
        .disable_x_mesh()
        .disable_y_mesh()
        .y_labels(5)
        .set_tick_mark_size(LabelAreaPosition::Left, -5)
        .y_label_formatter(&left_axis.formatter)
        .axis_style(&RED)
        .label_style(labels_font.clone())
        .draw()
        .unwrap();

    chart
        .configure_secondary_axes()
        .y_labels(5)
        .set_tick_mark_size(LabelAreaPosition::Right, -5)
        .y_label_formatter(&right_axis.formatter)
        .axis_style(&RED)
        .label_style(labels_font)
        .draw()
        .unwrap();

    if should_draw_legend {
        let legent_font = TextStyle {
            font: FontDesc::new(FontFamily::Monospace, 10.0, FontStyle::Normal),
            color: plotters_backend::BackendColor {
                alpha: 0.6,
                rgb: (200, 200, 200),
            },
            pos: text_anchor::Pos::new(text_anchor::HPos::Right, text_anchor::VPos::Center),
        };
        chart
            .configure_series_labels()
            .background_style(&TRANSPARENT)
            .border_style(&TRANSPARENT)
            .label_font(legent_font)
            .position(SeriesLabelPosition::Coordinate(90, 10))
            .draw()
            .unwrap();
    }
}
