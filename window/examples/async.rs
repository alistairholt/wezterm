use ::window::*;
use config::Dimension;
use promise::spawn::spawn;
use std::cell::RefCell;
use std::rc::Rc;
use wezterm_font::FontConfiguration;

struct MyWindow {
    allow_close: bool,
    cursor_pos: Point,
    dims: Dimensions,
}

impl Drop for MyWindow {
    fn drop(&mut self) {
        eprintln!("MyWindow dropped");
    }
}

impl MyWindow {
    fn dispatch(&mut self, event: WindowEvent, win: &Window) {
        match dbg!(event) {
            WindowEvent::CloseRequested => {
                eprintln!("can I close?");
                if self.allow_close {
                    win.close();
                } else {
                    self.allow_close = true;
                }
            }
            WindowEvent::Destroyed => {
                eprintln!("destroy was called!");
                Connection::get().unwrap().terminate_message_loop();
            }
            WindowEvent::Resized {
                dimensions,
                window_state,
                live_resizing,
            } => {
                eprintln!(
                    "resize {:?} live={} state={:?}",
                    dimensions, live_resizing, window_state
                );
                self.dims = dimensions;
            }
            WindowEvent::MouseEvent(event) => {
                self.cursor_pos = event.coords;
                // win.invalidate();
                win.set_cursor(Some(CursorIcon::Default));

                if event.kind == MouseEventKind::Press(MousePress::Left) {
                    eprintln!("{:?}", event);
                }
            }
            WindowEvent::KeyEvent(key) => {
                eprintln!("{:?}", key);
                win.set_cursor(Some(CursorIcon::Text));
            }
            WindowEvent::RawKeyEvent(key) => {
                eprintln!("{:?}", key);
                win.set_cursor(Some(CursorIcon::Text));
            }
            WindowEvent::AppearanceChanged(_)
            | WindowEvent::AdviseDeadKeyStatus(_)
            | WindowEvent::AdviseModifiersLedStatus(_, _)
            | WindowEvent::Notification(_)
            | WindowEvent::FocusChanged(_)
            | WindowEvent::DraggedFile(_)
            | WindowEvent::DroppedFile(_)
            | WindowEvent::DroppedUrl(_)
            | WindowEvent::DroppedString(_)
            | WindowEvent::PerformKeyAssignment(_)
            | WindowEvent::MouseLeave
            | WindowEvent::NeedRepaint
            | WindowEvent::SetInnerSizeCompleted => {}
        }
    }
}

async fn spawn_window() -> Result<(), Box<dyn std::error::Error>> {
    let fontconfig = Rc::new(FontConfiguration::new(
        None,
        ::window::default_dpi() as usize,
    )?);

    let state = Rc::new(RefCell::new(MyWindow {
        allow_close: false,
        cursor_pos: Point::new(100, 200),
        dims: Dimensions {
            pixel_width: 800,
            pixel_height: 600,
            dpi: 0,
        },
    }));

    let cb_state = Rc::clone(&state);
    let win = Window::new_window(
        "myclass",
        "the title",
        RequestedWindowGeometry {
            width: Dimension::Pixels(800.),
            height: Dimension::Pixels(600.),
            ..Default::default()
        },
        None,
        fontconfig,
        move |event, window| {
            let mut state = cb_state.borrow_mut();
            state.dispatch(event, window)
        },
    )
    .await?;

    eprintln!("before show");
    win.show();
    win.invalidate();
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let conn = Connection::init()?;
    spawn(async {
        eprintln!("running this async block");
        dbg!(spawn_window().await).ok();
        eprintln!("end of async block");
    })
    .detach();
    conn.run_message_loop()
}
