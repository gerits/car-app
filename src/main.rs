use std::rc::Rc;
use std::cell::RefCell;
use std::sync::mpsc;
use std::thread;
use slint::ComponentHandle;
use chrono::Timelike;

mod map_renderer;
use map_renderer::{render_map, MapView};

slint::include_modules!();

struct RenderRequest {
    offset_x: f32,
    offset_y: f32,
    width: u32,
    height: u32,
    is_dark: bool,
}

fn is_night_time() -> bool {
    is_night_time_at(chrono::Local::now().hour())
}

fn is_night_time_at(hour: u32) -> bool {
    hour < 6 || hour >= 18
}

fn calculate_simulated_speed(elapsed: f32) -> i32 {
    let sin_val = (elapsed / 2.0).sin();
    10 + (60.0 + 60.0 * sin_val) as i32
}

fn main() -> Result<(), slint::PlatformError> {
    env_logger::init();

    let ui = AppWindow::new()?;

    let initial_is_dark = is_night_time();
    ui.set_is_dark_mode(initial_is_dark);
    
    let offset_x = Rc::new(RefCell::new(0.0f32));
    let offset_y = Rc::new(RefCell::new(0.0f32));

    // Background rendering setup
    let (tx, rx) = mpsc::channel::<RenderRequest>();
    
    let ui_handle_thread = ui.as_weak();
    thread::spawn(move || {
        while let Ok(req) = rx.recv() {
            // Drain the channel to only process the LATEST request
            let mut latest_req = req;
            while let Ok(next_req) = rx.try_recv() {
                latest_req = next_req;
            }
            
            let buffer = render_map(
                latest_req.offset_x, 
                latest_req.offset_y, 
                latest_req.width, 
                latest_req.height, 
                &MapView {
                    center_x: 33756,
                    center_y: 21962,
                    zoom: 16,
                },
                latest_req.is_dark
            );
            
            let ui_handle_inner = ui_handle_thread.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_handle_inner.upgrade() {
                    ui.set_map_bg(slint::Image::from_rgba8_premultiplied(buffer));
                }
            });
        }
    });

    let initial_size = ui.window().size();
    let display_size = initial_size.width.min(initial_size.height);
    let _ = tx.send(RenderRequest {
        offset_x: 0.0,
        offset_y: 0.0,
        width: display_size,
        height: display_size,
        is_dark: initial_is_dark,
    });

    let tx_drag = tx.clone();
    let offset_x_clone = offset_x.clone();
    let offset_y_clone = offset_y.clone();
    let ui_handle_drag = ui.as_weak();
    
    ui.on_map_dragged(move |dx, dy| {
        *offset_x_clone.borrow_mut() += dx;
        *offset_y_clone.borrow_mut() += dy;
        
        if let Some(ui) = ui_handle_drag.upgrade() {
            let sz = ui.window().size();
            let d_sz = sz.width.min(sz.height);
            let _ = tx_drag.send(RenderRequest {
                offset_x: *offset_x_clone.borrow(),
                offset_y: *offset_y_clone.borrow(),
                width: d_sz,
                height: d_sz,
                is_dark: ui.get_is_dark_mode(),
            });
        }
    });

    let tx_toggle = tx.clone();
    let offset_x_toggle = offset_x.clone();
    let offset_y_toggle = offset_y.clone();
    let ui_handle_toggle = ui.as_weak();
    ui.on_toggle_theme(move || {
        if let Some(ui) = ui_handle_toggle.upgrade() {
            let next_dark = !ui.get_is_dark_mode();
            ui.set_is_dark_mode(next_dark);
            
            let sz = ui.window().size();
            let d_sz = sz.width.min(sz.height);
            let _ = tx_toggle.send(RenderRequest {
                offset_x: *offset_x_toggle.borrow(),
                offset_y: *offset_y_toggle.borrow(),
                width: d_sz,
                height: d_sz,
                is_dark: next_dark,
            });
        }
    });

    // Reactive resize handling using a Timer to check for window size changes
    let ui_handle_resize = ui.as_weak();
    let tx_resize = tx.clone();
    let last_size = Rc::new(RefCell::new(initial_size));
    let offset_x_resize = offset_x.clone();
    let offset_y_resize = offset_y.clone();

    let resize_timer = slint::Timer::default();
    resize_timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(50), move || {
        if let Some(ui) = ui_handle_resize.upgrade() {
            let sz = ui.window().size();
            if *last_size.borrow() != sz {
                *last_size.borrow_mut() = sz;
                let d_sz = sz.width.min(sz.height);
                let _ = tx_resize.send(RenderRequest {
                    offset_x: *offset_x_resize.borrow(),
                    offset_y: *offset_y_resize.borrow(),
                    width: d_sz,
                    height: d_sz,
                    is_dark: ui.get_is_dark_mode(),
                });
            }
        }
    });
    
    // Speed simulation timer
    let ui_handle_speed = ui.as_weak();
    let speed_timer = slint::Timer::default();
    let start_time = std::time::Instant::now();
    speed_timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(20), move || {
        if let Some(ui) = ui_handle_speed.upgrade() {
            let elapsed = start_time.elapsed().as_secs_f32();
            let simulated_speed = calculate_simulated_speed(elapsed);
            ui.set_current_speed(simulated_speed);
        }
    });

    ui.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_night_time_at() {
        assert!(is_night_time_at(0));   // Midnight
        assert!(is_night_time_at(5));   // 5 AM
        assert!(!is_night_time_at(6));  // 6 AM
        assert!(!is_night_time_at(12)); // Noon
        assert!(!is_night_time_at(17)); // 5 PM
        assert!(is_night_time_at(18));  // 6 PM
        assert!(is_night_time_at(23));  // 11 PM
    }

    #[test]
    fn test_calculate_simulated_speed() {
        let s0 = calculate_simulated_speed(0.0);
        assert!(s0 >= 10 && s0 <= 130);
        
        let s_pi = calculate_simulated_speed(std::f32::consts::PI);
        assert!(s_pi >= 10 && s_pi <= 130);
    }
}
