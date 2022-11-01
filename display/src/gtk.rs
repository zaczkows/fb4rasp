use crossbeam::channel;
use fb4rasp_shared::{notify::NotifyData, RenderState};
use glib;
use gtk::{
    prelude::*, traits::WidgetExt, Application, ApplicationWindow, Button, DrawingArea, Grid,
};
use log;
use std::{cell::RefCell, rc::Rc, sync::Arc};

pub fn start(
    tx: channel::Sender<NotifyData>,
    rx: channel::Receiver<NotifyData>,
    renderer: Arc<dyn Fn(&RenderState, &gtk::cairo::Context, i32, i32)>,
) {
    let state = Rc::new(RefCell::new(RenderState {
        net_tx: vec![],
        net_rx: vec![],
    }));
    let app = Application::builder()
        .application_id("test.app.fb4rasp")
        .build();

    app.connect_activate(move |app| {
        let renderer = Arc::clone(&renderer);
        let rx = rx.clone();
        // We create the main window.
        let window = ApplicationWindow::builder()
            .application(app)
            .default_width(1024)
            .default_height(768)
            .title("fb4rasp emulator")
            .build();

        let grid = Grid::builder().expand(true).build();

        let mut buttons = vec![];
        for i in 0i32..7i32 {
            let b = Button::builder()
                .label(&i.to_string())
                .hexpand(true)
                .build();
            b.connect_clicked(move |_| {
                log::debug!("Clicked: {}", i);
            });
            grid.attach(&b, i, 0, 1, 1);
            buttons.push(b);
        }

        let drawing_area = DrawingArea::builder().expand(true).build();
        {
            let state = Rc::clone(&state);
            drawing_area.connect_draw(move |widget, context| {
                let renderer = Arc::clone(&renderer);
                // let context = widget.style_context();
                let width = widget.allocated_width();
                let height = widget.allocated_height();
                // log::debug!("Widget dimentions: {} x {}", width, height);
                renderer(&state.borrow(), context, width, height);
                return gtk::Inhibit(true);
            });
            grid.attach(&drawing_area, 0, 1, buttons.len() as i32, 1);
        }

        window.add(&grid);

        {
            let mut state = Rc::clone(&state);
            glib::source::idle_add_local(move || {
                if let Ok(msg) = rx.recv() {
                    match msg {
                        NotifyData::NetworkData(tx, rx) => {
                            let mut state = state.borrow_mut();
                            state.net_tx = tx;
                            state.net_rx = rx;
                        }
                        _ => (),
                    }
                    // println!("Signal received");
                    drawing_area.queue_draw();
                }
                glib::Continue(true)
            });
        }

        // Don't forget to make all widgets visible.
        window.show_all();
    });

    app.run();
    let _ = tx.send(NotifyData::STOP);
}
