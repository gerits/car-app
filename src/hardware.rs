//! Hardware abstractions and mock integrations for the car dashboard application.
//! Decoupled interfaces for speedometer, fuel sensor, odometer, blinkers, and warning lights.

/// Trait representing a speedometer.
pub trait SpeedSensor {
    /// Returns the current speed in km/h.
    fn speed_kph(&self) -> f32;
}

/// Trait representing a fuel level sensor.
pub trait FuelSensor {
    /// Returns the current fuel level as a value between 0.0 (empty) and 1.0 (full).
    fn fuel_level(&self) -> f32;
}

/// Trait representing an odometer/distance sensor.
pub trait OdometerSensor {
    /// Returns the total distance traveled by the vehicle in kilometers.
    fn odometer_km(&self) -> f64;
}

/// Combined state of the left and right blinker lights.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlinkerState {
    /// All turn signal lights are currently off.
    AllOff,
    /// Both left and right turn signal lights are currently on (e.g. hazard or self-test).
    AllOn,
    /// Only the left turn signal light is currently on.
    Left,
    /// Only the right turn signal light is currently on.
    Right,
}

/// Trait representing a turn signal/blinker sensor.
pub trait BlinkerSensor {
    /// Returns the current physical state of the blinker bulbs.
    fn blinker_state(&self) -> BlinkerState;
}

/// Trait representing the vehicle's alternator/battery charging warning light.
pub trait ChargeLightSensor {
    /// Returns whether the battery charge/alternator warning light is active.
    fn is_on(&self) -> bool;
}

/// Trait representing the vehicle's low oil pressure warning light.
pub trait OilPressureLightSensor {
    /// Returns whether the low oil pressure warning light is active.
    fn is_on(&self) -> bool;
}

/// Trait representing the vehicle's high beam indicator light.
pub trait HighBeamSensor {
    /// Returns whether the high beam indicator light is active.
    fn is_on(&self) -> bool;
}

/// Trait representing the vehicle's ignition power indicator light.
pub trait IgnitionLightSensor {
    /// Returns whether the ignition warning light is active.
    fn is_on(&self) -> bool;
}

// =========================================================================
// MOCK IMPLEMENTATIONS
// =========================================================================

/// A mock fuel sensor that simulates fuel consumption over distance.
pub struct MockFuelSensor {
    fuel_level: f32,
    consumption_rate_per_km: f32,
}

impl MockFuelSensor {
    /// Creates a new `MockFuelSensor` with an initial fuel level and consumption rate.
    pub fn new(initial_level: f32, consumption_rate_per_km: f32) -> Self {
        Self {
            fuel_level: initial_level.clamp(0.0, 1.0),
            consumption_rate_per_km,
        }
    }

    /// Simulates driving a certain distance, consuming fuel.
    pub fn update(&mut self, distance_km: f32) {
        self.fuel_level -= distance_km * self.consumption_rate_per_km;
        if self.fuel_level <= 0.0 {
            // longevitiy/demo loop: refuel automatically when empty!
            self.fuel_level = 1.0;
        }
    }
}

impl FuelSensor for MockFuelSensor {
    fn fuel_level(&self) -> f32 {
        self.fuel_level
    }
}

/// A mock odometer sensor that tracks total traveled distance.
pub struct MockOdometerSensor {
    total_km: f64,
}

impl MockOdometerSensor {
    /// Creates a new `MockOdometerSensor` starting at the given distance.
    pub fn new(initial_km: f64) -> Self {
        Self { total_km: initial_km }
    }

    /// Simulates driving at a speed for dt seconds, returning distance traveled in km.
    pub fn update(&mut self, dt: f32, speed_kph: f32) -> f32 {
        let delta_km = (speed_kph as f64 / 3600.0) * (dt as f64);
        self.total_km += delta_km;
        delta_km as f32
    }
}

impl OdometerSensor for MockOdometerSensor {
    fn odometer_km(&self) -> f64 {
        self.total_km
    }
}

/// The coordination phases for mock warning lights startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockIgnitionPhase {
    /// Self-test mode: all warning lights are illuminated solid.
    SelfTest,
    /// Crank/Engine start: warning lights flicker as battery voltage fluctuates.
    Cranking,
    /// Running: normal state, warning lights turn off.
    Running,
}

/// A unified mock vehicle system that simulates speed and controls indicator lights cohesively.
pub struct MockVehicleSystem {
    speed_kph: f32,
    elapsed_time: f32,
    ignition_phase: MockIgnitionPhase,
    phase_timer: f32,
    blinker_timer: f32,
    blinker_bulb_on: bool,
}

impl MockVehicleSystem {
    /// Creates a new `MockVehicleSystem`.
    pub fn new() -> Self {
        Self {
            speed_kph: 0.0,
            elapsed_time: 0.0,
            ignition_phase: MockIgnitionPhase::SelfTest,
            phase_timer: 0.0,
            blinker_timer: 0.0,
            blinker_bulb_on: false,
        }
    }

    /// Updates the simulated vehicle systems.
    pub fn update(&mut self, dt: f32) {
        self.elapsed_time += dt;
        self.phase_timer += dt;
        self.blinker_timer += dt;

        // Blinker blinking rate: toggle every 500ms
        if self.blinker_timer >= 0.5 {
            self.blinker_bulb_on = !self.blinker_bulb_on;
            self.blinker_timer -= 0.5;
        }

        // Speed simulation: smooth sine wave over time (30 to 70 km/h)
        let sin_val = (self.elapsed_time / 4.0).sin();
        self.speed_kph = 50.0 + 20.0 * sin_val;

        // Startup/Ignition sequence state machine
        match self.ignition_phase {
            MockIgnitionPhase::SelfTest => {
                if self.phase_timer >= 3.0 {
                    self.ignition_phase = MockIgnitionPhase::Cranking;
                    self.phase_timer = 0.0;
                }
            }
            MockIgnitionPhase::Cranking => {
                if self.phase_timer >= 1.5 {
                    self.ignition_phase = MockIgnitionPhase::Running;
                    self.phase_timer = 0.0;
                }
            }
            MockIgnitionPhase::Running => {
                // Keep running state
            }
        }
    }
}

impl SpeedSensor for MockVehicleSystem {
    fn speed_kph(&self) -> f32 {
        self.speed_kph
    }
}

impl BlinkerSensor for MockVehicleSystem {
    fn blinker_state(&self) -> BlinkerState {
        // Cycles blinker states every 8s: Left -> Off -> Right -> Off -> Hazard -> Off
        let cycle = (self.elapsed_time / 8.0) as i32 % 6;
        let active_mode = match cycle {
            0 => BlinkerState::Left,
            1 => BlinkerState::AllOff,
            2 => BlinkerState::Right,
            3 => BlinkerState::AllOff,
            4 => BlinkerState::AllOn, // Hazard
            _ => BlinkerState::AllOff,
        };

        if !self.blinker_bulb_on {
            return BlinkerState::AllOff;
        }

        active_mode
    }
}

impl ChargeLightSensor for MockVehicleSystem {
    fn is_on(&self) -> bool {
        match self.ignition_phase {
            MockIgnitionPhase::SelfTest => true,
            MockIgnitionPhase::Cranking => {
                // Flicker charge light during crank
                (self.phase_timer * 15.0).sin() > 0.0
            }
            MockIgnitionPhase::Running => false,
        }
    }
}

impl OilPressureLightSensor for MockVehicleSystem {
    fn is_on(&self) -> bool {
        match self.ignition_phase {
            MockIgnitionPhase::SelfTest => true,
            MockIgnitionPhase::Cranking => {
                // Flicker oil light during crank
                (self.phase_timer * 15.0).sin() > 0.0
            }
            MockIgnitionPhase::Running => false,
        }
    }
}

impl HighBeamSensor for MockVehicleSystem {
    fn is_on(&self) -> bool {
        // Toggle high beams periodically only once running
        if self.ignition_phase == MockIgnitionPhase::Running {
            (self.elapsed_time % 12.0) > 6.0
        } else {
            false
        }
    }
}

impl IgnitionLightSensor for MockVehicleSystem {
    fn is_on(&self) -> bool {
        match self.ignition_phase {
            MockIgnitionPhase::SelfTest | MockIgnitionPhase::Cranking => true,
            MockIgnitionPhase::Running => false,
        }
    }
}

// =========================================================================
// HELPERS
// =========================================================================

/// Formats a kilometer distance into a 6-digit vector format for Slint UI.
/// Supports a fixed decimal place in the 6th element (tenths of a kilometer).
pub fn format_odometer(km: f64) -> Vec<slint::SharedString> {
    let tenths = (km * 10.0).round() as i64;
    let tenths = tenths.max(0).min(999999);
    let s = format!("{:06}", tenths);
    s.chars()
        .map(|c| slint::SharedString::from(c.to_string()))
        .collect()
}

// =========================================================================
// TESTS
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_odometer() {
        assert_eq!(
            format_odometer(3.0),
            vec!["0", "0", "0", "0", "3", "0"]
        );
        assert_eq!(
            format_odometer(123.45),
            vec!["0", "0", "1", "2", "3", "5"] // rounded from 123.45 -> 1234.5 tenths -> 1235
        );
        assert_eq!(
            format_odometer(99999.9),
            vec!["9", "9", "9", "9", "9", "9"]
        );
        assert_eq!(
            format_odometer(-10.0),
            vec!["0", "0", "0", "0", "0", "0"]
        );
    }

    #[test]
    fn test_mock_odometer() {
        let mut odo = MockOdometerSensor::new(10.0);
        let dist = odo.update(1.0, 36.0); // 36 km/h for 1 second = 0.01 km
        assert_eq!(dist, 0.01);
        assert!((odo.odometer_km() - 10.01).abs() < 1e-6);
    }

    #[test]
    fn test_mock_fuel() {
        let mut fuel = MockFuelSensor::new(0.5, 0.01); // 1% per km
        fuel.update(10.0); // consumes 10%
        assert!((fuel.fuel_level() - 0.4).abs() < 1e-6);

        // Test refueling wrap
        fuel.update(50.0); // should drop below 0.0 and wrap to 1.0
        assert_eq!(fuel.fuel_level(), 1.0);
    }

    #[test]
    fn test_mock_vehicle_system_startup() {
        let mut sys = MockVehicleSystem::new();
        
        // At start: Self-test phase. Warning lights should be solid ON.
        assert!(ChargeLightSensor::is_on(&sys)); // charge light
        assert!(<MockVehicleSystem as OilPressureLightSensor>::is_on(&sys));
        assert!(<MockVehicleSystem as IgnitionLightSensor>::is_on(&sys));

        // Advance past 3 seconds: Cranking phase.
        sys.update(3.1);
        assert_eq!(sys.ignition_phase, MockIgnitionPhase::Cranking);

        // Advance past 1.5 seconds cranking: Running phase.
        sys.update(1.6);
        assert_eq!(sys.ignition_phase, MockIgnitionPhase::Running);
        
        // Warning lights should be OFF in running state.
        assert!(!ChargeLightSensor::is_on(&sys));
        assert!(!<MockVehicleSystem as OilPressureLightSensor>::is_on(&sys));
        assert!(!<MockVehicleSystem as IgnitionLightSensor>::is_on(&sys));
    }
}
