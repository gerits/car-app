use chrono::Timelike;
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

use car_app::AppWindow;
use car_app::hardware::{
    self, BlinkerSensor, BlinkerState, ChargeLightSensor, FuelSensor, HighBeamSensor,
    IgnitionLightSensor, OdometerSensor, OilPressureLightSensor, SpeedSensor,
};
use car_app::map_renderer::{MapView, render_map};

struct RenderRequest {
    offset_x: f32,
    offset_y: f32,
    width: u32,
    height: u32,
    scale: f32,
    is_dark: bool,
    center_x: u64,
    center_y: u64,
    world_x: f32,
    world_y: f32,
}

fn is_night_time() -> bool {
    is_night_time_at(chrono::Local::now().hour())
}

fn is_night_time_at(hour: u32) -> bool {
    !(6..18).contains(&hour)
}

#[cfg(test)]
fn calculate_simulated_speed(elapsed: f32) -> i32 {
    let sin_val = (elapsed / 4.0).sin();
    (50.0 + 20.0 * sin_val) as i32
}

fn main() -> Result<(), slint::PlatformError> {
    env_logger::init();
    log::info!("Application starting...");

    let ui = AppWindow::new()?;

    let initial_is_dark = is_night_time();
    ui.set_is_dark_mode(initial_is_dark);

    let offset_x = Rc::new(RefCell::new(0.0f32));
    let offset_y = Rc::new(RefCell::new(0.0f32));
    let rotation_degrees = Rc::new(RefCell::new(0.0f32));
    let center_x = Rc::new(RefCell::new(33756u64));
    let center_y = Rc::new(RefCell::new(21962u64));
    let pause_simulation_until = Rc::new(RefCell::new(None::<std::time::Instant>));
    let current_dist = Rc::new(RefCell::new(0.0f32));
    let last_tick_time = Rc::new(RefCell::new(std::time::Instant::now()));

    let mock_vehicle = Rc::new(RefCell::new(hardware::MockVehicleSystem::new()));
    let mock_fuel = Rc::new(RefCell::new(hardware::MockFuelSensor::new(0.75, 0.005)));
    let mock_odometer = Rc::new(RefCell::new(hardware::MockOdometerSensor::new(3.0)));

    let last_map_render_time = Rc::new(RefCell::new(std::time::Instant::now()));
    let startup_time = Rc::new(std::time::Instant::now());
    let first_render_finished = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let last_requested_world_x = Rc::new(RefCell::new(33756.0f32));
    let last_requested_world_y = Rc::new(RefCell::new(21962.0f32));
    ui.set_map_rendered_world_x(33756.0);
    ui.set_map_rendered_world_y(21962.0);
    ui.set_current_world_x(33756.0);
    ui.set_current_world_y(21962.0);

    // Define waypoints and precompute cumulative segment distances for the driving loop
    const WAYPOINTS: &[(f32, f32)] = &[
        (33754.7859, 21966.5124),
        (33754.6452, 21966.3007),
        (33754.0536, 21965.9959),
        (33753.2983, 21965.6355),
        (33752.5932, 21965.1040),
        (33752.2435, 21964.8649),
        (33749.5649, 21963.7388),
        (33746.9688, 21962.3417),
        (33746.2151, 21962.0639),
        (33745.6832, 21962.0980),
        (33745.2291, 21962.2317),
        (33745.1441, 21962.1820),
        (33744.9015, 21962.1136),
        (33744.2288, 21961.5960),
        (33743.6048, 21961.0505),
        (33743.0608, 21960.7204),
        (33741.6021, 21960.1675),
        (33740.9192, 21960.0685),
        (33740.7186, 21960.1034),
        (33740.3331, 21960.2296),
        (33740.0784, 21960.2253),
        (33739.7989, 21960.1794),
        (33739.6782, 21960.1011),
        (33739.7989, 21960.1794),
        (33740.0784, 21960.2253),
        (33740.3331, 21960.2296),
        (33740.7186, 21960.1034),
        (33740.9192, 21960.0685),
        (33741.6021, 21960.1675),
        (33743.0608, 21960.7204),
        (33743.6048, 21961.0505),
        (33744.2288, 21961.5960),
        (33744.9015, 21962.1136),
        (33745.1441, 21962.1820),
        (33745.2291, 21962.2317),
        (33745.6832, 21962.0980),
        (33746.2151, 21962.0639),
        (33746.9688, 21962.3417),
        (33749.5649, 21963.7388),
        (33752.2435, 21964.8649),
        (33752.5932, 21965.1040),
        (33753.2983, 21965.6355),
        (33754.0536, 21965.9959),
        (33754.6452, 21966.3007),
        (33754.7859, 21966.5124),
    ];

    let mut distances = Vec::new();
    let mut total_length = 0.0;
    distances.push(0.0);
    for i in 0..WAYPOINTS.len() {
        let p1 = WAYPOINTS[i];
        let p2 = WAYPOINTS[(i + 1) % WAYPOINTS.len()];
        let dx = p2.0 - p1.0;
        let dy = p2.1 - p1.1;
        let dist = (dx * dx + dy * dy).sqrt();
        total_length += dist;
        distances.push(total_length);
    }
    let distances = Rc::new(distances);

    // Background rendering setup
    let (tx, rx) = mpsc::channel::<RenderRequest>();

    let ui_handle_thread = ui.as_weak();
    let first_render_finished_thread = first_render_finished.clone();
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
                latest_req.scale,
                &MapView {
                    center_x: latest_req.center_x,
                    center_y: latest_req.center_y,
                    zoom: 16,
                    camera_zoom: 0.25,
                },
                latest_req.is_dark,
                0.0, // ALWAYS render unrotated map for Slint to rotate using GPU!
            );

            let req_world_x = latest_req.world_x;
            let req_world_y = latest_req.world_y;

            let first_render_finished_inner = first_render_finished_thread.clone();
            let ui_handle_inner = ui_handle_thread.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_handle_inner.upgrade() {
                    ui.set_map_bg(slint::Image::from_rgba8_premultiplied(buffer));
                    ui.set_map_rendered_world_x(req_world_x);
                    ui.set_map_rendered_world_y(req_world_y);
                    first_render_finished_inner.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            });
        }
    });

    let initial_size = ui.window().size();
    let display_size = initial_size.width.min(initial_size.height);
    let map_sz = (display_size as f32 * 1.5) as u32;
    let scale = (display_size as f32 / 800.0).max(display_size as f32 / 480.0);
    let _ = tx.send(RenderRequest {
        offset_x: 0.0,
        offset_y: 0.0,
        width: map_sz,
        height: map_sz,
        scale,
        is_dark: initial_is_dark,
        center_x: *center_x.borrow(),
        center_y: *center_y.borrow(),
        world_x: 33756.0,
        world_y: 21962.0,
    });

    let tx_drag = tx.clone();
    let offset_x_clone = offset_x.clone();
    let offset_y_clone = offset_y.clone();
    let center_x_drag = center_x.clone();
    let center_y_drag = center_y.clone();
    let pause_simulation_until_drag = pause_simulation_until.clone();
    let ui_handle_drag = ui.as_weak();

    ui.on_map_dragged(move |dx, dy| {
        *offset_x_clone.borrow_mut() += dx;
        *offset_y_clone.borrow_mut() += dy;

        // Pause simulation for 5 seconds when manually dragged
        *pause_simulation_until_drag.borrow_mut() =
            Some(std::time::Instant::now() + std::time::Duration::from_secs(5));

        if let Some(ui) = ui_handle_drag.upgrade() {
            let sz = ui.window().size();
            let d_sz = sz.width.min(sz.height);
            // Render a 1.5x larger map to prevent corners from clipping when Slint rotates it
            let map_sz = (d_sz as f32 * 1.5) as u32;
            let scale = (d_sz as f32 / 800.0).max(d_sz as f32 / 480.0);
            let _ = tx_drag.send(RenderRequest {
                offset_x: *offset_x_clone.borrow(),
                offset_y: *offset_y_clone.borrow(),
                width: map_sz,
                height: map_sz,
                scale,
                is_dark: ui.get_is_dark_mode(),
                center_x: *center_x_drag.borrow(),
                center_y: *center_y_drag.borrow(),
                world_x: *center_x_drag.borrow() as f32, // simplified for drag
                world_y: *center_y_drag.borrow() as f32,
            });
        }
    });

    let tx_toggle = tx.clone();
    let offset_x_toggle = offset_x.clone();
    let offset_y_toggle = offset_y.clone();
    let center_x_toggle = center_x.clone();
    let center_y_toggle = center_y.clone();
    let ui_handle_toggle = ui.as_weak();
    ui.on_toggle_theme(move || {
        if let Some(ui) = ui_handle_toggle.upgrade() {
            let next_dark = !ui.get_is_dark_mode();
            ui.set_is_dark_mode(next_dark);

            let sz = ui.window().size();
            let d_sz = sz.width.min(sz.height);
            let map_sz = (d_sz as f32 * 1.5) as u32;
            let scale = (d_sz as f32 / 800.0).max(d_sz as f32 / 480.0);
            let _ = tx_toggle.send(RenderRequest {
                offset_x: *offset_x_toggle.borrow(),
                offset_y: *offset_y_toggle.borrow(),
                width: map_sz,
                height: map_sz,
                scale,
                is_dark: next_dark,
                center_x: *center_x_toggle.borrow(),
                center_y: *center_y_toggle.borrow(),
                world_x: ui.get_map_rendered_world_x(),
                world_y: ui.get_map_rendered_world_y(),
            });
        }
    });

    // Reactive resize handling using a Timer to check for window size changes
    let ui_handle_resize = ui.as_weak();
    let tx_resize = tx.clone();
    let last_size = Rc::new(RefCell::new(initial_size));
    let offset_x_resize = offset_x.clone();
    let offset_y_resize = offset_y.clone();
    let center_x_resize = center_x.clone();
    let center_y_resize = center_y.clone();

    let resize_timer = slint::Timer::default();
    resize_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(200),
        move || {
            if let Some(ui) = ui_handle_resize.upgrade() {
                let sz = ui.window().size();
                if *last_size.borrow() != sz {
                    *last_size.borrow_mut() = sz;
                    let d_sz = sz.width.min(sz.height);
                    let map_sz = (d_sz as f32 * 1.5) as u32;
                    let scale = (d_sz as f32 / 800.0).max(d_sz as f32 / 480.0);
                    let _ = tx_resize.send(RenderRequest {
                        offset_x: *offset_x_resize.borrow(),
                        offset_y: *offset_y_resize.borrow(),
                        width: map_sz,
                        height: map_sz,
                        scale,
                        is_dark: ui.get_is_dark_mode(),
                        center_x: *center_x_resize.borrow(),
                        center_y: *center_y_resize.borrow(),
                        world_x: ui.get_map_rendered_world_x(),
                        world_y: ui.get_map_rendered_world_y(),
                    });
                }
            }
        },
    );

    // Speed and driving simulation timer
    let ui_handle_speed = ui.as_weak();
    let speed_timer = slint::Timer::default();

    let loading_rot = Rc::new(RefCell::new(0.0f32));

    let offset_x_speed = offset_x.clone();
    let offset_y_speed = offset_y.clone();
    let rotation_degrees_speed = rotation_degrees.clone();
    let center_x_speed = center_x.clone();
    let center_y_speed = center_y.clone();
    let pause_simulation_until_speed = pause_simulation_until.clone();
    let current_dist_speed = current_dist.clone();
    let last_tick_time_speed = last_tick_time.clone();
    let distances_speed = distances.clone();
    let tx_speed = tx.clone();
    let last_req_world_x = last_requested_world_x.clone();
    let last_req_world_y = last_requested_world_y.clone();
    let last_map_req_time = last_map_render_time.clone();
    let loading_rot_speed = loading_rot.clone();
    let startup_time_speed = startup_time.clone();
    let first_render_finished_speed = first_render_finished.clone();

    let mock_vehicle_speed = mock_vehicle.clone();
    let mock_fuel_speed = mock_fuel.clone();
    let mock_odometer_speed = mock_odometer.clone();

    speed_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(100),
        move || {
            if let Some(ui) = ui_handle_speed.upgrade() {
                let now = std::time::Instant::now();
                let elapsed_loading = now.duration_since(*startup_time_speed).as_secs_f32();
                let is_render_finished =
                    first_render_finished_speed.load(std::sync::atomic::Ordering::SeqCst);

                // Animate radar sweeper if map is still loading
                if !ui.get_map_loaded() {
                    let mut r = *loading_rot_speed.borrow_mut();
                    r = (r + 12.0) % 360.0;
                    *loading_rot_speed.borrow_mut() = r;
                    ui.set_loading_rotation(r);

                    if is_render_finished && elapsed_loading >= 2.0 {
                        ui.set_map_loaded(true);
                    }
                }

                // Get dt
                let dt = now
                    .duration_since(*last_tick_time_speed.borrow())
                    .as_secs_f32();
                *last_tick_time_speed.borrow_mut() = now;

                // Update mock hardware
                mock_vehicle_speed.borrow_mut().update(dt);
                let current_speed = if ui.get_map_loaded() {
                    mock_vehicle_speed.borrow().speed_kph()
                } else {
                    0.0
                };

                // Update UI Speedometer and indicators
                ui.set_current_speed(current_speed as i32);
                ui.set_charge_light_on(ChargeLightSensor::is_on(&*mock_vehicle_speed.borrow()));
                ui.set_oil_light_on(OilPressureLightSensor::is_on(&*mock_vehicle_speed.borrow()));
                ui.set_ignition_light_on(IgnitionLightSensor::is_on(&*mock_vehicle_speed.borrow()));
                ui.set_high_beam_on(HighBeamSensor::is_on(&*mock_vehicle_speed.borrow()));

                let blinker = mock_vehicle_speed.borrow().blinker_state();
                ui.set_turn_signal_on(blinker != BlinkerState::AllOff);

                // Check if simulation is paused due to manual drag
                let is_paused = if let Some(until) = *pause_simulation_until_speed.borrow() {
                    now < until
                } else {
                    false
                };

                if !is_paused && ui.get_map_loaded() {
                    // Update odometer and fuel based on current_speed
                    let delta_km = mock_odometer_speed.borrow_mut().update(dt, current_speed);
                    mock_fuel_speed.borrow_mut().update(delta_km);

                    // Update UI Fuel and Odometer
                    ui.set_fuel_level(mock_fuel_speed.borrow().fuel_level());

                    let odo_digits =
                        hardware::format_odometer(mock_odometer_speed.borrow().odometer_km());
                    let odo_model = Rc::new(slint::VecModel::from(odo_digits));
                    ui.set_odometer(odo_model.into());

                    // Advance position along waypoints
                    // Speed is in km/h. Convert to tile units per second.
                    // speed_in_tiles_per_second = speed * 0.0007218
                    let delta_dist = current_speed * 0.0007218 * dt;
                    let mut dist = *current_dist_speed.borrow_mut() + delta_dist;
                    if dist >= total_length {
                        dist %= total_length;
                    }
                    *current_dist_speed.borrow_mut() = dist;

                    // Interpolate
                    let mut seg_idx = 0;
                    for i in 0..WAYPOINTS.len() {
                        if dist >= distances_speed[i] && dist <= distances_speed[i + 1] {
                            seg_idx = i;
                            break;
                        }
                    }

                    let p1 = WAYPOINTS[seg_idx];
                    let p2 = WAYPOINTS[(seg_idx + 1) % WAYPOINTS.len()];
                    let seg_start_dist = distances_speed[seg_idx];
                    let seg_len = distances_speed[seg_idx + 1] - seg_start_dist;

                    let t = if seg_len > 0.0 {
                        (dist - seg_start_dist) / seg_len
                    } else {
                        0.0
                    };

                    let x = p1.0 + t * (p2.0 - p1.0);
                    let y = p1.1 + t * (p2.1 - p1.1);

                    let dx = p2.0 - p1.0;
                    let dy = p2.1 - p1.1;
                    let mut target_rot = dy.atan2(dx).to_degrees() + 90.0;
                    if target_rot < 0.0 {
                        target_rot += 360.0;
                    }

                    // Angular LERP to smoothly transition map rotation
                    let current_rot = *rotation_degrees_speed.borrow();
                    let mut diff = (target_rot - current_rot) % 360.0;
                    if diff > 180.0 {
                        diff -= 360.0;
                    } else if diff < -180.0 {
                        diff += 360.0;
                    }

                    let mut smooth_rot = current_rot + diff * 0.05;
                    if smooth_rot < 0.0 {
                        smooth_rot += 360.0;
                    } else if smooth_rot >= 360.0 {
                        smooth_rot -= 360.0;
                    }

                    // Set current offsets, center, and rotation
                    let new_center_x = x.floor() as u64;
                    let new_center_y = y.floor() as u64;
                    let frac_x = x.fract();
                    let frac_y = y.fract();

                    let new_offset_x = (0.0061724 - frac_x) * 512.0;
                    let new_offset_y = (0.5817041 - frac_y) * 512.0;

                    *offset_x_speed.borrow_mut() = new_offset_x;
                    *offset_y_speed.borrow_mut() = new_offset_y;
                    *rotation_degrees_speed.borrow_mut() = smooth_rot;
                    *center_x_speed.borrow_mut() = new_center_x;
                    *center_y_speed.borrow_mut() = new_center_y;

                    // Update Slint Map Properties for GPU rotation and translation
                    ui.set_map_rotation(smooth_rot);

                    let current_world_x = new_center_x as f32 + frac_x;
                    let current_world_y = new_center_y as f32 + frac_y;

                    let sz = ui.window().size();
                    let d_sz = sz.width.min(sz.height);
                    let scale = (d_sz as f32 / 800.0).max(d_sz as f32 / 480.0);
                    let pixels_per_unit = 512.0 * scale * 0.25; // 512 * scale * camera_zoom

                    ui.set_current_world_x(current_world_x);
                    ui.set_current_world_y(current_world_y);

                    // Use the last REQUESTED coordinates to check if we need to request again
                    let req_wx = *last_req_world_x.borrow();
                    let req_wy = *last_req_world_y.borrow();
                    let req_shift_x = (req_wx - current_world_x) * pixels_per_unit;
                    let req_shift_y = (req_wy - current_world_y) * pixels_per_unit;

                    // Only request a new map frame if we moved more than 150 pixels or 2 seconds passed
                    // from our LAST REQUEST
                    let dist_sq = req_shift_x * req_shift_x + req_shift_y * req_shift_y;
                    let time_since_render = now
                        .duration_since(*last_map_req_time.borrow())
                        .as_secs_f32();

                    if dist_sq > 150.0 * 150.0 || time_since_render > 2.0 {
                        *last_req_world_x.borrow_mut() = current_world_x;
                        *last_req_world_y.borrow_mut() = current_world_y;
                        *last_map_req_time.borrow_mut() = now;

                        let map_sz = (d_sz as f32 * 1.5) as u32;
                        let _ = tx_speed.send(RenderRequest {
                            offset_x: new_offset_x,
                            offset_y: new_offset_y,
                            width: map_sz,
                            height: map_sz,
                            scale,
                            is_dark: ui.get_is_dark_mode(),
                            center_x: new_center_x,
                            center_y: new_center_y,
                            world_x: current_world_x,
                            world_y: current_world_y,
                        });
                    }
                }
            }
        },
    );

    ui.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_night_time_at() {
        assert!(is_night_time_at(0)); // Midnight
        assert!(is_night_time_at(5)); // 5 AM
        assert!(!is_night_time_at(6)); // 6 AM
        assert!(!is_night_time_at(12)); // Noon
        assert!(!is_night_time_at(17)); // 5 PM
        assert!(is_night_time_at(18)); // 6 PM
        assert!(is_night_time_at(23)); // 11 PM
    }

    #[test]
    fn test_calculate_simulated_speed() {
        let s0 = calculate_simulated_speed(0.0);
        assert!(s0 >= 30 && s0 <= 70);

        let s_pi = calculate_simulated_speed(std::f32::consts::PI);
        assert!(s_pi >= 30 && s_pi <= 70);
    }
    #[test]
    fn test_is_night_time() {
        // Just execute it to ensure it doesn't panic and we get coverage.
        let _ = is_night_time();
    }
}
