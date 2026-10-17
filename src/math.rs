//! Vectors, angles and direction helpers made for audio games.
//!
//! ## Coordinate system
//!
//! The engine uses a right-handed system with **z pointing up**:
//!
//! * `+x` is east (right on a map)
//! * `+y` is north (forward on a map)
//! * `+z` is up
//!
//! Angles ("yaw" or "facing") are **compass degrees**: 0 = north, 90 = east, 180 = south,
//! 270 = west. A 2D side scroller simply uses `x` for left/right and `z` for up/down, a top
//! down game uses `x` and `y`.

pub use glam::{Vec2, Vec3, vec2, vec3};

/// Wraps an angle into the range `0.0..360.0`.
pub fn normalize_angle(degrees: f32) -> f32 {
    degrees.rem_euclid(360.0)
}

/// Shortest signed difference `to - from` in degrees, in the range `-180.0..=180.0`.
/// Positive means turning clockwise (to the right).
pub fn angle_difference(from: f32, to: f32) -> f32 {
    let d = normalize_angle(to - from);
    if d > 180.0 { d - 360.0 } else { d }
}

/// Unit vector pointing in the compass direction `yaw` on the ground plane.
pub fn forward(yaw: f32) -> Vec3 {
    let r = yaw.to_radians();
    Vec3::new(r.sin(), r.cos(), 0.0)
}

/// Unit vector pointing to the right of the compass direction `yaw`.
pub fn right(yaw: f32) -> Vec3 {
    forward(yaw + 90.0)
}

/// Compass bearing in degrees from `from` to `to`, ignoring height.
pub fn bearing(from: Vec3, to: Vec3) -> f32 {
    let d = to - from;
    if d.x == 0.0 && d.y == 0.0 {
        return 0.0;
    }
    normalize_angle(d.x.atan2(d.y).to_degrees())
}

/// Moves `position` by `distance` in compass direction `yaw` on the ground plane.
pub fn move_towards(position: Vec3, yaw: f32, distance: f32) -> Vec3 {
    position + forward(yaw) * distance
}

/// Converts a compass angle to a name such as "north east". Uses 8 directions.
pub fn direction_name(degrees: f32) -> &'static str {
    const NAMES: [&str; 8] =
        ["north", "north east", "east", "south east", "south", "south west", "west", "north west"];
    let i = ((normalize_angle(degrees) + 22.5) / 45.0) as usize % 8;
    NAMES[i]
}

/// Converts an angle relative to the player (0 = straight ahead) to a clock position,
/// e.g. 90 degrees becomes 3 (o'clock). Returns 1..=12.
pub fn clock_position(relative_degrees: f32) -> u32 {
    let hour = ((normalize_angle(relative_degrees) + 15.0) / 30.0) as u32 % 12;
    if hour == 0 { 12 } else { hour }
}

/// Describes where `target` is relative to someone at `from` facing `yaw`,
/// e.g. `"3 o'clock, 12 meters"`. Ready to be spoken.
pub fn describe_position(from: Vec3, yaw: f32, target: Vec3) -> String {
    let relative = angle_difference(yaw, bearing(from, target));
    let distance = (target - from).length().round() as i64;
    let unit = if distance == 1 { "meter" } else { "meters" };
    format!("{} o'clock, {distance} {unit}", clock_position(relative))
}

/// Linear interpolation between `a` and `b`.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Converts decibels to a linear volume factor (0 dB = 1.0, -6 dB ≈ 0.5).
pub fn db_to_linear(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Converts a linear volume factor to decibels.
pub fn linear_to_db(linear: f32) -> f32 {
    20.0 * linear.max(1e-6).log10()
}

/// Random helpers so you do not need an extra crate for the basics.
pub mod random {
    /// Random float in `min..max`.
    pub fn range(min: f32, max: f32) -> f32 {
        min + fastrand::f32() * (max - min)
    }

    /// Random integer in `min..=max`.
    pub fn int(min: i64, max: i64) -> i64 {
        fastrand::i64(min..=max)
    }

    /// Returns `true` with the given probability (0.0 to 1.0).
    pub fn chance(probability: f32) -> bool {
        fastrand::f32() < probability
    }

    /// Picks a random element, or `None` for an empty slice.
    pub fn pick<T>(items: &[T]) -> Option<&T> {
        if items.is_empty() { None } else { Some(&items[fastrand::usize(..items.len())]) }
    }

    /// Seeds the random generator for reproducible results.
    pub fn seed(seed: u64) {
        fastrand::seed(seed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compass() {
        assert!((forward(90.0) - Vec3::X).length() < 1e-5);
        assert!((bearing(Vec3::ZERO, Vec3::new(0.0, -5.0, 0.0)) - 180.0).abs() < 1e-3);
        assert_eq!(direction_name(44.0), "north east");
        assert_eq!(direction_name(359.0), "north");
        assert_eq!(angle_difference(350.0, 10.0), 20.0);
        assert_eq!(clock_position(90.0), 3);
        assert_eq!(clock_position(0.0), 12);
        assert_eq!(describe_position(Vec3::ZERO, 0.0, Vec3::new(5.0, 0.0, 0.0)), "3 o'clock, 5 meters");
    }
}
