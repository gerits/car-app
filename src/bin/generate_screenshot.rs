use slint::ComponentHandle;
use std::path::Path;
use car_app::map_renderer::{MapView, render_map};
use car_app::AppWindow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the UI
    let ui = AppWindow::new()?;

    // Set mock data for the screenshot
    ui.set_current_speed(78);
    ui.set_is_dark_mode(true);

    // Render the map background synchronously for the screenshot
    let view = MapView {
        center_x: 33756,
        center_y: 21962,
        zoom: 16,
        camera_zoom: 0.25,
    };
    
    let buffer = render_map(
        0.0,
        0.0,
        800, 
        800,
        1.0,
        &view,
        true,
        0.0,
    );
    ui.set_map_bg(slint::Image::from_rgba8_premultiplied(buffer));

    // We need to show the window to initialize the renderer
    ui.show()?;

    // Create a timer to capture the snapshot after a brief delay
    // This allows the renderer to initialize and draw the first frame
    let ui_handle = ui.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::SingleShot, std::time::Duration::from_millis(100), move || {
        if let Some(ui) = ui_handle.upgrade() {
            let window = ui.window();
            match window.take_snapshot() {
                Ok(screenshot) => {
                    let path = Path::new("assets/screenshot.png");
                    let (width, height) = (screenshot.width(), screenshot.height());
                    let pixels = screenshot.as_bytes();
                    
                    if let Err(e) = image::save_buffer(
                        path,
                        pixels,
                        width,
                        height,
                        image::ExtendedColorType::Rgba8,
                    ) {
                        eprintln!("Failed to save screenshot: {}", e);
                    } else {
                        println!("Screenshot saved to {:?}", path);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to take snapshot: {:?}", e);
                }
            }
            // Quit the event loop to exit the application
            let _ = slint::quit_event_loop();
        }
    });

    // Run the event loop
    ui.run()?;

    Ok(())
}
