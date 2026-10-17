//! Simple, predictable 3D physics. Available as `ctx.physics`.
//!
//! Made for the needs of audio games: walls you bump into, floors you stand on, things that
//! fall, triggers that tell you when the player enters a room, and ray casts to "look"
//! around. There are only two shapes (boxes and spheres) and no rotation, which keeps
//! everything easy to reason about.
//!
//! The engine steps the world automatically at a fixed rate (60 Hz by default), so you
//! only add bodies and react to [`PhysicsEvent`]s.
//!
//! ```no_run
//! # use audio_code::prelude::*;
//! # fn f(ctx: &mut Context) {
//! ctx.physics.set_gravity(EARTH_GRAVITY);
//! ctx.physics.add(Body::fixed(vec3(0.0, 0.0, -0.5)).with_box(100.0, 100.0, 1.0).with_tag("floor"));
//! let player = ctx.physics.add(Body::dynamic(vec3(0.0, 0.0, 1.0)).with_box(0.6, 0.6, 1.8).with_tag("player"));
//!
//! // Later: walk forward and find out if a wall was in the way.
//! let result = ctx.physics.move_by(player, vec3(0.0, 1.0, 0.0));
//! if result.blocked {
//!     ctx.audio.play("bump");
//! }
//! # }
//! ```
//! See [the coordinate system](crate::math) for axis directions.

use std::collections::{BTreeMap, HashSet};

use glam::Vec3;

/// Gravity on earth: 9.81 m/s² downwards (towards -z).
pub const EARTH_GRAVITY: Vec3 = Vec3::new(0.0, 0.0, -9.81);

/// Identifies a body in the [`World`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BodyId(u32);

/// Collision shape. Shapes never rotate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// Axis aligned box given by its half size along x, y and z.
    Box(Vec3),
    /// Sphere with a radius.
    Sphere(f32),
}

impl Shape {
    fn half_extents(&self) -> Vec3 {
        match *self {
            Shape::Box(h) => h,
            Shape::Sphere(r) => Vec3::splat(r),
        }
    }
}

/// How a body takes part in the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    /// Moved by velocity and gravity, pushed out of other solids.
    Dynamic,
    /// Never moves on its own (walls, floors). Other bodies collide with it.
    Fixed,
    /// Moved only by your code (velocity is applied, gravity is not). Blocks dynamic bodies.
    Kinematic,
    /// Does not block anything, only reports when bodies enter and leave it.
    Sensor,
}

/// A physical object.
#[derive(Debug, Clone)]
pub struct Body {
    /// Center position.
    pub position: Vec3,
    /// Velocity in meters per second.
    pub velocity: Vec3,
    /// Collision shape.
    pub shape: Shape,
    /// How the body is simulated.
    pub kind: BodyKind,
    /// Multiplier for world gravity (dynamic bodies only).
    pub gravity_scale: f32,
    /// How much speed is kept when bouncing off something, 0.0 (none) to 1.0 (all).
    pub bounciness: f32,
    /// Velocity damping per second while touching the ground, 0.0 = ice.
    pub friction: f32,
    /// Velocity damping per second at all times (air resistance).
    pub damping: f32,
    /// Free text to recognize bodies in events, e.g. `"wall"` or `"coin"`.
    pub tag: String,
    /// Free number for your own data, e.g. an index into your entity list.
    pub user_data: u64,
    /// Compass direction the body faces in degrees (0 = north, 90 = east). Shapes never
    /// rotate; this is used by [`World::move_forward`], [`World::turn`] and the listener
    /// when a body is followed with [`Context::follow`](crate::Context::follow).
    pub facing: f32,
    /// Mass in kilograms (dynamic bodies). Heavier bodies push lighter ones further and lose
    /// less speed when they collide.
    pub mass: f32,
    /// Speed limit in meters per second, e.g. a terminal velocity when falling.
    /// `f32::INFINITY` (the default) means no limit.
    pub max_speed: f32,
    /// Whether the body touched the ground (something below it) during the last step.
    pub on_ground: bool,
    /// The body this one stands on, if any. Bodies standing on a moving kinematic body
    /// (a lift, a platform) are carried along.
    pub ground: Option<BodyId>,
    force: Vec3,
}

impl Body {
    fn new(kind: BodyKind, position: Vec3) -> Self {
        Self {
            position,
            velocity: Vec3::ZERO,
            shape: Shape::Box(Vec3::splat(0.5)),
            kind,
            gravity_scale: 1.0,
            bounciness: 0.0,
            friction: 8.0,
            damping: 0.0,
            tag: String::new(),
            user_data: 0,
            facing: 0.0,
            mass: 1.0,
            max_speed: f32::INFINITY,
            on_ground: false,
            ground: None,
            force: Vec3::ZERO,
        }
    }

    /// A body moved by physics. Default shape: 1 meter cube.
    pub fn dynamic(position: Vec3) -> Self {
        Self::new(BodyKind::Dynamic, position)
    }

    /// An immovable body such as a wall or floor.
    pub fn fixed(position: Vec3) -> Self {
        Self::new(BodyKind::Fixed, position)
    }

    /// A body only moved by your code (doors, platforms, a player you move yourself).
    pub fn kinematic(position: Vec3) -> Self {
        Self::new(BodyKind::Kinematic, position)
    }

    /// An invisible trigger area.
    pub fn sensor(position: Vec3) -> Self {
        Self::new(BodyKind::Sensor, position)
    }

    /// Box shape with full width (x), depth (y) and height (z).
    pub fn with_box(mut self, width: f32, depth: f32, height: f32) -> Self {
        self.shape = Shape::Box(Vec3::new(width, depth, height) * 0.5);
        self
    }

    /// Sphere shape.
    pub fn with_sphere(mut self, radius: f32) -> Self {
        self.shape = Shape::Sphere(radius);
        self
    }

    /// Sets a tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = tag.into();
        self
    }

    /// Sets the initial velocity.
    pub fn with_velocity(mut self, velocity: Vec3) -> Self {
        self.velocity = velocity;
        self
    }

    /// Sets bounciness (0.0 to 1.0).
    pub fn with_bounciness(mut self, bounciness: f32) -> Self {
        self.bounciness = bounciness.clamp(0.0, 1.0);
        self
    }

    /// Sets ground friction.
    pub fn with_friction(mut self, friction: f32) -> Self {
        self.friction = friction.max(0.0);
        self
    }

    /// Sets air damping.
    pub fn with_damping(mut self, damping: f32) -> Self {
        self.damping = damping.max(0.0);
        self
    }

    /// Sets the gravity multiplier.
    pub fn with_gravity_scale(mut self, scale: f32) -> Self {
        self.gravity_scale = scale;
        self
    }

    /// Sets the mass in kilograms (default 1.0).
    pub fn with_mass(mut self, mass: f32) -> Self {
        self.mass = mass.max(0.001);
        self
    }

    /// Sets a speed limit in meters per second (terminal velocity).
    pub fn with_max_speed(mut self, max_speed: f32) -> Self {
        self.max_speed = max_speed.max(0.0);
        self
    }

    /// Sets the compass direction the body faces.
    pub fn with_facing(mut self, degrees: f32) -> Self {
        self.facing = crate::math::normalize_angle(degrees);
        self
    }

    /// Sets user data.
    pub fn with_user_data(mut self, data: u64) -> Self {
        self.user_data = data;
        self
    }

    fn min(&self) -> Vec3 {
        self.position - self.shape.half_extents()
    }

    fn max(&self) -> Vec3 {
        self.position + self.shape.half_extents()
    }

    fn is_solid(&self) -> bool {
        self.kind != BodyKind::Sensor
    }

    /// Whether this body overlaps a point.
    pub fn contains(&self, point: Vec3) -> bool {
        match self.shape {
            Shape::Box(_) => point.cmpge(self.min()).all() && point.cmple(self.max()).all(),
            Shape::Sphere(r) => point.distance_squared(self.position) <= r * r,
        }
    }
}

/// Something that happened during the physics steps of this frame.
#[derive(Debug, Clone, PartialEq)]
pub enum PhysicsEvent {
    /// Two solid bodies started touching. `speed` is how fast they hit each other.
    CollisionStarted {
        /// The moving body.
        a: BodyId,
        /// The body it hit.
        b: BodyId,
        /// Impact speed in meters per second.
        speed: f32,
    },
    /// Two solid bodies stopped touching.
    CollisionEnded {
        /// First body.
        a: BodyId,
        /// Second body.
        b: BodyId,
    },
    /// A dynamic body touched the ground after being in the air. Perfect for landing sounds
    /// and fall damage.
    Landed {
        /// The body that landed.
        body: BodyId,
        /// What it landed on.
        on: BodyId,
        /// Falling speed at impact in meters per second.
        speed: f32,
    },
    /// A body entered a sensor.
    Entered {
        /// The sensor.
        sensor: BodyId,
        /// The body that entered.
        body: BodyId,
    },
    /// A body left a sensor.
    Left {
        /// The sensor.
        sensor: BodyId,
        /// The body that left.
        body: BodyId,
    },
}

impl PhysicsEvent {
    /// Whether the event involves this body.
    pub fn involves(&self, id: BodyId) -> bool {
        match *self {
            PhysicsEvent::CollisionStarted { a, b, .. } | PhysicsEvent::CollisionEnded { a, b } => a == id || b == id,
            PhysicsEvent::Landed { body, on, .. } => body == id || on == id,
            PhysicsEvent::Entered { sensor, body } | PhysicsEvent::Left { sensor, body } => sensor == id || body == id,
        }
    }

    /// For events involving `id`, the other body.
    pub fn other(&self, id: BodyId) -> Option<BodyId> {
        let (x, y) = match *self {
            PhysicsEvent::CollisionStarted { a, b, .. } | PhysicsEvent::CollisionEnded { a, b } => (a, b),
            PhysicsEvent::Landed { body, on, .. } => (body, on),
            PhysicsEvent::Entered { sensor, body } | PhysicsEvent::Left { sensor, body } => (sensor, body),
        };
        if x == id {
            Some(y)
        } else if y == id {
            Some(x)
        } else {
            None
        }
    }
}

/// Result of [`World::raycast`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    /// The body hit.
    pub body: BodyId,
    /// Where the ray hit.
    pub point: Vec3,
    /// Distance from the ray origin.
    pub distance: f32,
    /// Surface direction at the hit point.
    pub normal: Vec3,
}

/// Result of [`World::move_by`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MoveResult {
    /// How far the body actually moved.
    pub moved: Vec3,
    /// `true` if something stopped part of the movement.
    pub blocked: bool,
    /// Bodies that were hit, in order.
    pub hits: Vec<BodyId>,
}

/// The physics world. Reach it through `ctx.physics`.
#[derive(Debug, Clone)]
pub struct World {
    bodies: BTreeMap<BodyId, Body>,
    next_id: u32,
    gravity: Vec3,
    contacts: HashSet<(BodyId, BodyId)>,
    sensor_overlaps: HashSet<(BodyId, BodyId)>,
    events: Vec<PhysicsEvent>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

/// Minimum translation to separate two overlapping shapes: (direction to push `a`, depth).
fn penetration(a: &Body, b: &Body) -> Option<(Vec3, f32)> {
    match (a.shape, b.shape) {
        (Shape::Sphere(ra), Shape::Sphere(rb)) => {
            let d = a.position - b.position;
            let dist = d.length();
            let depth = ra + rb - dist;
            if depth <= 0.0 {
                return None;
            }
            let n = if dist > 1e-6 { d / dist } else { Vec3::Z };
            Some((n, depth))
        }
        (Shape::Sphere(r), Shape::Box(_)) => sphere_box(a.position, r, b),
        (Shape::Box(_), Shape::Sphere(r)) => sphere_box(b.position, r, a).map(|(n, d)| (-n, d)),
        (Shape::Box(_), Shape::Box(_)) => {
            let overlap = a.max().min(b.max()) - a.min().max(b.min());
            if overlap.x <= 0.0 || overlap.y <= 0.0 || overlap.z <= 0.0 {
                return None;
            }
            let d = a.position - b.position;
            let sign = |v: f32| if v >= 0.0 { 1.0 } else { -1.0 };
            if overlap.x < overlap.y && overlap.x < overlap.z {
                Some((Vec3::X * sign(d.x), overlap.x))
            } else if overlap.y < overlap.z {
                Some((Vec3::Y * sign(d.y), overlap.y))
            } else {
                Some((Vec3::Z * sign(d.z), overlap.z))
            }
        }
    }
}

/// Sphere at `center` against a box body: direction to push the sphere and depth.
fn sphere_box(center: Vec3, radius: f32, b: &Body) -> Option<(Vec3, f32)> {
    let closest = center.clamp(b.min(), b.max());
    let d = center - closest;
    let dist = d.length();
    if dist > 1e-6 {
        let depth = radius - dist;
        return (depth > 0.0).then(|| (d / dist, depth));
    }
    // Center inside the box: push out through the nearest face.
    let to_max = b.max() - center;
    let to_min = center - b.min();
    let candidates = [
        (Vec3::X, to_max.x),
        (-Vec3::X, to_min.x),
        (Vec3::Y, to_max.y),
        (-Vec3::Y, to_min.y),
        (Vec3::Z, to_max.z),
        (-Vec3::Z, to_min.z),
    ];
    let (n, d) = candidates.into_iter().min_by(|a, b| a.1.total_cmp(&b.1))?;
    Some((n, d + radius))
}

fn pair(a: BodyId, b: BodyId) -> (BodyId, BodyId) {
    if a < b { (a, b) } else { (b, a) }
}

impl World {
    /// Creates an empty world without gravity.
    pub fn new() -> Self {
        Self {
            bodies: BTreeMap::new(),
            next_id: 1,
            gravity: Vec3::ZERO,
            contacts: HashSet::new(),
            sensor_overlaps: HashSet::new(),
            events: Vec::new(),
        }
    }

    /// Sets gravity, e.g. [`EARTH_GRAVITY`]. Default is no gravity, which suits top down
    /// games. Also available as [`Engine::gravity`](crate::Engine::gravity).
    pub fn set_gravity(&mut self, gravity: Vec3) {
        self.gravity = gravity;
    }

    /// Direction pointing away from gravity (`+z` when there is no gravity).
    pub fn up(&self) -> Vec3 {
        (-self.gravity).normalize_or(Vec3::Z)
    }

    /// Current gravity.
    pub fn gravity(&self) -> Vec3 {
        self.gravity
    }

    /// Adds a body and returns its id.
    pub fn add(&mut self, body: Body) -> BodyId {
        let id = BodyId(self.next_id);
        self.next_id += 1;
        self.bodies.insert(id, body);
        id
    }

    /// Removes a body. Returns it if it existed.
    pub fn remove(&mut self, id: BodyId) -> Option<Body> {
        self.contacts.retain(|&(a, b)| a != id && b != id);
        self.sensor_overlaps.retain(|&(a, b)| a != id && b != id);
        self.bodies.remove(&id)
    }

    /// Removes every body.
    pub fn clear(&mut self) {
        self.bodies.clear();
        self.contacts.clear();
        self.sensor_overlaps.clear();
        self.events.clear();
    }

    /// Access a body.
    pub fn get(&self, id: BodyId) -> Option<&Body> {
        self.bodies.get(&id)
    }

    /// Mutable access to a body.
    pub fn get_mut(&mut self, id: BodyId) -> Option<&mut Body> {
        self.bodies.get_mut(&id)
    }

    /// Position of a body (zero if it does not exist).
    pub fn position(&self, id: BodyId) -> Vec3 {
        self.get(id).map_or(Vec3::ZERO, |b| b.position)
    }

    /// Teleports a body without collision checks.
    pub fn set_position(&mut self, id: BodyId, position: Vec3) {
        if let Some(b) = self.get_mut(id) {
            b.position = position;
        }
    }

    /// Velocity of a body (zero if it does not exist).
    pub fn velocity(&self, id: BodyId) -> Vec3 {
        self.get(id).map_or(Vec3::ZERO, |b| b.velocity)
    }

    /// Sets the velocity of a body.
    pub fn set_velocity(&mut self, id: BodyId, velocity: Vec3) {
        if let Some(b) = self.get_mut(id) {
            b.velocity = velocity;
        }
    }

    /// Adds an instant change of velocity, e.g. a jump: `apply_impulse(player, vec3(0.0, 0.0, 5.0))`.
    pub fn apply_impulse(&mut self, id: BodyId, impulse: Vec3) {
        if let Some(b) = self.get_mut(id) {
            b.velocity += impulse;
        }
    }

    /// Pushes a body with a force in newtons during the next physics step, e.g. wind,
    /// a conveyor belt or a rocket. Call it every frame for a continuous force.
    pub fn apply_force(&mut self, id: BodyId, force: Vec3) {
        if let Some(b) = self.get_mut(id) {
            b.force += force;
        }
    }

    /// Makes a body standing on the ground jump so that it reaches `height` meters,
    /// taking gravity into account. Returns `false` (and does nothing) while in the air.
    ///
    /// ```no_run
    /// # use audio_code::prelude::*;
    /// # fn f(ctx: &mut Context, player: BodyId) {
    /// if ctx.input.pressed(Key::Space) && ctx.physics.jump(player, 1.5) {
    ///     ctx.audio.play("jump");
    /// }
    /// # }
    /// ```
    pub fn jump(&mut self, id: BodyId, height: f32) -> bool {
        let (up, gravity) = (self.up(), self.gravity.length());
        let Some(b) = self.get_mut(id) else {
            return false;
        };
        let g = gravity * b.gravity_scale;
        if !b.on_ground || g <= 0.0 {
            return false;
        }
        let speed = (2.0 * g * height.max(0.0)).sqrt();
        b.velocity += up * (speed - b.velocity.dot(up));
        b.on_ground = false;
        true
    }

    /// Whether a body stands on something.
    pub fn on_ground(&self, id: BodyId) -> bool {
        self.get(id).is_some_and(|b| b.on_ground)
    }

    /// The tag of a body ("" if it does not exist).
    pub fn tag(&self, id: BodyId) -> &str {
        self.get(id).map_or("", |b| b.tag.as_str())
    }

    /// Iterates over all bodies.
    pub fn bodies(&self) -> impl Iterator<Item = (BodyId, &Body)> {
        self.bodies.iter().map(|(id, b)| (*id, b))
    }

    /// All bodies with a tag.
    pub fn with_tag<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = BodyId> + 'a {
        self.bodies.iter().filter(move |(_, b)| b.tag == tag).map(|(id, _)| *id)
    }

    /// Number of bodies.
    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    /// Whether the world is empty.
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    /// Events from the physics steps of this frame.
    pub fn events(&self) -> &[PhysicsEvent] {
        &self.events
    }

    pub(crate) fn clear_events(&mut self) {
        self.events.clear();
    }

    /// Sensors `body` is currently inside.
    pub fn sensors_of(&self, body: BodyId) -> Vec<BodyId> {
        self.sensor_overlaps.iter().filter(|(_, b)| *b == body).map(|(s, _)| *s).collect()
    }

    /// Whether two bodies overlap right now.
    pub fn overlaps(&self, a: BodyId, b: BodyId) -> bool {
        match (self.get(a), self.get(b)) {
            (Some(x), Some(y)) => penetration(x, y).is_some(),
            _ => false,
        }
    }

    /// Bodies whose shape contains `point`.
    pub fn at_point(&self, point: Vec3) -> Vec<BodyId> {
        self.bodies.iter().filter(|(_, b)| b.contains(point)).map(|(id, _)| *id).collect()
    }

    /// Bodies whose center lies within `radius` of `center`, nearest first.
    pub fn in_radius(&self, center: Vec3, radius: f32) -> Vec<BodyId> {
        let mut found: Vec<(f32, BodyId)> = self
            .bodies
            .iter()
            .map(|(id, b)| (b.position.distance(center), *id))
            .filter(|(d, _)| *d <= radius)
            .collect();
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        found.into_iter().map(|(_, id)| id).collect()
    }

    /// Nearest body with the given tag.
    pub fn nearest_with_tag(&self, center: Vec3, tag: &str) -> Option<BodyId> {
        self.bodies
            .iter()
            .filter(|(_, b)| b.tag == tag)
            .min_by(|a, b| a.1.position.distance_squared(center).total_cmp(&b.1.position.distance_squared(center)))
            .map(|(id, _)| *id)
    }

    /// Casts a ray and returns the first solid body hit within `max_distance`.
    /// `ignore` is skipped (pass the caster itself). Sensors are ignored.
    pub fn raycast(&self, origin: Vec3, direction: Vec3, max_distance: f32, ignore: Option<BodyId>) -> Option<RayHit> {
        let dir = direction.normalize_or_zero();
        if dir == Vec3::ZERO {
            return None;
        }
        let mut best: Option<RayHit> = None;
        for (id, body) in &self.bodies {
            if Some(*id) == ignore || !body.is_solid() {
                continue;
            }
            let hit = match body.shape {
                Shape::Box(_) => ray_box(origin, dir, body.min(), body.max()),
                Shape::Sphere(r) => ray_sphere(origin, dir, body.position, r),
            };
            if let Some((t, normal)) = hit
                && t <= max_distance
                && best.as_ref().is_none_or(|b| t < b.distance)
            {
                best = Some(RayHit { body: *id, point: origin + dir * t, distance: t, normal });
            }
        }
        best
    }

    /// Moves a body by `delta` right now, sliding along and stopping at solid bodies.
    /// This is the easiest way to move a player: walls block, and you learn what was hit.
    pub fn move_by(&mut self, id: BodyId, delta: Vec3) -> MoveResult {
        let Some(body) = self.bodies.get(&id) else {
            return MoveResult::default();
        };
        let start = body.position;
        let size = body.shape.half_extents().min_element().max(0.01);
        let steps = ((delta.length() / size).ceil() as usize).clamp(1, 1000);
        let step = delta / steps as f32;
        let mut result = MoveResult::default();
        for _ in 0..steps {
            if let Some(b) = self.bodies.get_mut(&id) {
                b.position += step;
            }
            for (other, normal, _) in self.resolve(id) {
                if !result.hits.contains(&other) {
                    result.hits.push(other);
                }
                // Stop moving into the obstacle; slide along it.
                if step.dot(normal) < 0.0 {
                    result.blocked = true;
                }
            }
        }
        result.moved = self.position(id) - start;
        self.update_sensors_for(id);
        result
    }

    /// Pushes body `id` out of every solid it overlaps.
    /// Returns (other body, push direction, depth) for each contact.
    fn resolve(&mut self, id: BodyId) -> Vec<(BodyId, Vec3, f32)> {
        let mut contacts = Vec::new();
        let Some(body) = self.bodies.get(&id) else {
            return contacts;
        };
        if !body.is_solid() {
            return contacts;
        }
        // A few iterations settle corners where two walls meet.
        for _ in 0..4 {
            let body = &self.bodies[&id];
            let mut push = None;
            for (other_id, other) in &self.bodies {
                if *other_id == id || !other.is_solid() {
                    continue;
                }
                if let Some((n, depth)) = penetration(body, other)
                    && push.as_ref().is_none_or(|(_, _, d)| depth > *d)
                {
                    push = Some((*other_id, n, depth));
                }
            }
            let Some((other, n, depth)) = push else {
                break;
            };
            // How much of the overlap the other body takes: none for walls, all of it when a
            // kinematic body pushes a dynamic one, shared by mass between dynamic bodies.
            let other_body = &self.bodies[&other];
            let share = match (body.kind, other_body.kind) {
                (BodyKind::Kinematic, BodyKind::Dynamic) => 1.0,
                (BodyKind::Dynamic, BodyKind::Dynamic) => body.mass / (body.mass + other_body.mass),
                _ => 0.0,
            };
            let mut moved_other = 0.0;
            if share > 0.0 {
                let old = other_body.position;
                if let Some(o) = self.bodies.get_mut(&other) {
                    o.position -= n * depth * share;
                }
                if self.obstructed(other, id, -n) {
                    // Something solid is behind it (a crate against a wall): it does not move.
                    if let Some(o) = self.bodies.get_mut(&other) {
                        o.position = old;
                    }
                } else {
                    moved_other = share;
                }
            }
            if let Some(b) = self.bodies.get_mut(&id) {
                b.position += n * depth * (1.0 - moved_other);
            }
            contacts.push((other, n, depth));
        }
        contacts
    }

    /// Whether body `id`, pushed in `direction`, now overlaps a solid in that direction
    /// (ignoring `pusher`).
    fn obstructed(&self, id: BodyId, pusher: BodyId, direction: Vec3) -> bool {
        let body = &self.bodies[&id];
        self.bodies.iter().any(|(other_id, other)| {
            *other_id != id
                && *other_id != pusher
                && other.is_solid()
                && penetration(body, other).is_some_and(|(n, _)| n.dot(direction) < -0.5)
        })
    }

    fn update_sensors_for(&mut self, id: BodyId) {
        let Some(body) = self.bodies.get(&id) else {
            return;
        };
        if !body.is_solid() {
            return;
        }
        for (sid, sensor) in &self.bodies {
            if sensor.kind != BodyKind::Sensor {
                continue;
            }
            let inside = penetration(body, sensor).is_some();
            let key = (*sid, id);
            if inside && self.sensor_overlaps.insert(key) {
                self.events.push(PhysicsEvent::Entered { sensor: *sid, body: id });
            } else if !inside && self.sensor_overlaps.remove(&key) {
                self.events.push(PhysicsEvent::Left { sensor: *sid, body: id });
            }
        }
    }

    /// Advances the simulation by `dt` seconds. The engine calls this for you.
    pub fn step(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        let ids: Vec<BodyId> = self.bodies.keys().copied().collect();
        let gravity = self.gravity;
        let up = self.up();
        let mut touching = HashSet::new();

        // Kinematic bodies move first, so bodies standing on them can be carried along.
        let mut carried = Vec::new();
        for (id, body) in self.bodies.iter_mut() {
            if body.kind == BodyKind::Kinematic {
                let delta = body.velocity * dt;
                body.position += delta;
                if delta != Vec3::ZERO {
                    carried.push((*id, delta));
                }
            }
        }

        for &id in &ids {
            let Some(body) = self.bodies.get_mut(&id) else {
                continue;
            };
            if body.kind != BodyKind::Dynamic {
                continue;
            }
            if let Some((_, delta)) = carried.iter().find(|(platform, _)| Some(*platform) == body.ground) {
                body.position += *delta;
            }

            let force = std::mem::take(&mut body.force);
            body.velocity += (gravity * body.gravity_scale + force / body.mass) * dt;
            if body.damping > 0.0 {
                body.velocity *= (1.0 - body.damping * dt).max(0.0);
            }
            if body.on_ground && body.friction > 0.0 {
                let keep = (1.0 - body.friction * dt).max(0.0);
                let vertical = up * body.velocity.dot(up);
                body.velocity = vertical + (body.velocity - vertical) * keep;
            }
            if body.max_speed.is_finite() {
                body.velocity = body.velocity.clamp_length_max(body.max_speed);
            }

            let was_on_ground = body.on_ground;
            let delta = body.velocity * dt;
            let size = body.shape.half_extents().min_element().max(0.01);
            let substeps = ((delta.length() / size).ceil() as usize).clamp(1, 64);
            let step = delta / substeps as f32;
            let mut ground = None;
            let mut landing_speed = 0.0f32;
            for _ in 0..substeps {
                if let Some(b) = self.bodies.get_mut(&id) {
                    b.position += step;
                }
                for (other, n, _) in self.resolve(id) {
                    let impact = self.bounce(id, other, n);
                    if impact > 0.0 {
                        let key = pair(id, other);
                        if !self.contacts.contains(&key) && !touching.contains(&key) {
                            self.events.push(PhysicsEvent::CollisionStarted { a: id, b: other, speed: impact });
                        }
                    }
                    if n.dot(up) > 0.7 {
                        ground = Some(other);
                        landing_speed = landing_speed.max(impact);
                    }
                    touching.insert(pair(id, other));
                }
            }
            // A body resting on the floor is pushed into it by gravity every step and so keeps
            // its contact; a body that moved away loses it.
            if let Some(b) = self.bodies.get_mut(&id) {
                b.on_ground = ground.is_some();
                b.ground = ground;
            }
            if let (false, Some(on)) = (was_on_ground, ground)
                && landing_speed > 0.0
            {
                self.events.push(PhysicsEvent::Landed { body: id, on, speed: landing_speed });
            }
        }

        for &(a, b) in self.contacts.difference(&touching) {
            self.events.push(PhysicsEvent::CollisionEnded { a, b });
        }
        self.contacts = touching;

        for &id in &ids {
            self.update_sensors_for(id);
        }
    }

    /// Changes the velocities of `id` and `other` after they touched along normal `n`
    /// (pointing towards `id`). Returns the impact speed, 0.0 if they were moving apart.
    fn bounce(&mut self, id: BodyId, other: BodyId, n: Vec3) -> f32 {
        let (Some(a), Some(b)) = (self.bodies.get(&id), self.bodies.get(&other)) else {
            return 0.0;
        };
        let other_velocity = if b.kind == BodyKind::Fixed { Vec3::ZERO } else { b.velocity };
        let closing = (a.velocity - other_velocity).dot(n);
        if closing >= 0.0 {
            return 0.0;
        }
        let restitution = a.bounciness.max(b.bounciness);
        if b.kind == BodyKind::Dynamic {
            // Exchange momentum like billiard balls.
            let (ma, mb) = (a.mass, b.mass);
            let j = -(1.0 + restitution) * closing / (1.0 / ma + 1.0 / mb);
            if let Some(a) = self.bodies.get_mut(&id) {
                a.velocity += n * (j / ma);
            }
            if let Some(b) = self.bodies.get_mut(&other) {
                b.velocity -= n * (j / mb);
            }
        } else if let Some(a) = self.bodies.get_mut(&id) {
            // Walls and platforms do not move: remove the speed going into them.
            a.velocity -= n * closing * (1.0 + restitution);
        }
        -closing
    }
}


/// Convenience helpers for typical audio game code.
impl World {
    /// Compass direction a body faces (0 if it does not exist).
    pub fn facing(&self, id: BodyId) -> f32 {
        self.get(id).map_or(0.0, |b| b.facing)
    }

    /// Sets the compass direction a body faces.
    pub fn set_facing(&mut self, id: BodyId, degrees: f32) {
        if let Some(b) = self.get_mut(id) {
            b.facing = crate::math::normalize_angle(degrees);
        }
    }

    /// Turns a body by `degrees` (positive = clockwise / right) and returns the new facing.
    pub fn turn(&mut self, id: BodyId, degrees: f32) -> f32 {
        let facing = crate::math::normalize_angle(self.facing(id) + degrees);
        self.set_facing(id, facing);
        facing
    }

    /// Moves a body `distance` meters in the direction it faces (negative = backwards),
    /// sliding along walls. See [`World::move_by`].
    pub fn move_forward(&mut self, id: BodyId, distance: f32) -> MoveResult {
        let delta = crate::math::forward(self.facing(id)) * distance;
        self.move_by(id, delta)
    }

    /// Moves a body sideways, `distance` meters to its right (negative = left).
    pub fn strafe(&mut self, id: BodyId, distance: f32) -> MoveResult {
        let delta = crate::math::right(self.facing(id)) * distance;
        self.move_by(id, delta)
    }

    /// Distance between two bodies' centers (infinity if one does not exist).
    pub fn distance(&self, a: BodyId, b: BodyId) -> f32 {
        match (self.get(a), self.get(b)) {
            (Some(a), Some(b)) => a.position.distance(b.position),
            _ => f32::INFINITY,
        }
    }

    /// Where `target` is from the point of view of `from`, ready to speak:
    /// `"3 o'clock, 12 meters"`. Uses the facing of `from`.
    ///
    /// ```no_run
    /// # use audio_code::prelude::*;
    /// # fn f(ctx: &mut Context, player: BodyId, beacon: BodyId) {
    /// ctx.speak(ctx.physics.describe(player, beacon));
    /// # }
    /// ```
    pub fn describe(&self, from: BodyId, target: BodyId) -> String {
        crate::math::describe_position(self.position(from), self.facing(from), self.position(target))
    }

    // ------------------------------------------------------------- event shortcuts

    /// If `body` landed on something this frame: the falling speed.
    pub fn landed(&self, body: BodyId) -> Option<f32> {
        self.events.iter().find_map(|e| match *e {
            PhysicsEvent::Landed { body: b, speed, .. } if b == body => Some(speed),
            _ => None,
        })
    }

    /// If `body` started touching another solid body this frame: that body and the impact
    /// speed.
    pub fn collided(&self, body: BodyId) -> Option<(BodyId, f32)> {
        self.events.iter().find_map(|e| match *e {
            PhysicsEvent::CollisionStarted { a, b, speed } if a == body => Some((b, speed)),
            PhysicsEvent::CollisionStarted { a, b, speed } if b == body => Some((a, speed)),
            _ => None,
        })
    }

    /// If `body` walked into a sensor this frame: the sensor.
    pub fn entered(&self, body: BodyId) -> Option<BodyId> {
        self.events.iter().find_map(|e| match *e {
            PhysicsEvent::Entered { sensor, body: b } if b == body => Some(sensor),
            _ => None,
        })
    }

    /// If `body` walked into a sensor with this tag this frame: the sensor.
    ///
    /// ```no_run
    /// # use audio_code::prelude::*;
    /// # fn f(ctx: &mut Context, player: BodyId) {
    /// if let Some(coin) = ctx.physics.entered_tag(player, "coin") {
    ///     ctx.physics.remove(coin);   // sounds attached with ctx.loop_on stop by themselves
    ///     ctx.play("pickup");
    /// }
    /// # }
    /// ```
    pub fn entered_tag(&self, body: BodyId, tag: &str) -> Option<BodyId> {
        self.events.iter().find_map(|e| match *e {
            PhysicsEvent::Entered { sensor, body: b } if b == body && self.tag(sensor) == tag => Some(sensor),
            _ => None,
        })
    }

    /// If `body` left a sensor this frame: the sensor.
    pub fn left(&self, body: BodyId) -> Option<BodyId> {
        self.events.iter().find_map(|e| match *e {
            PhysicsEvent::Left { sensor, body: b } if b == body => Some(sensor),
            _ => None,
        })
    }

    /// Whether anything entered `sensor` this frame; returns the body.
    pub fn triggered(&self, sensor: BodyId) -> Option<BodyId> {
        self.events.iter().find_map(|e| match *e {
            PhysicsEvent::Entered { sensor: s, body } if s == sensor => Some(body),
            _ => None,
        })
    }
}

/// Ray against axis aligned box (slab method). Returns distance and normal.
fn ray_box(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<(f32, Vec3)> {
    let mut t_min = 0.0f32;
    let mut t_max = f32::INFINITY;
    let mut normal = Vec3::ZERO;
    for axis in 0..3 {
        let (o, d, lo, hi) = (origin[axis], dir[axis], min[axis], max[axis]);
        if d.abs() < 1e-8 {
            if o < lo || o > hi {
                return None;
            }
            continue;
        }
        let (mut t1, mut t2) = ((lo - o) / d, (hi - o) / d);
        let mut n = Vec3::ZERO;
        n[axis] = -d.signum();
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        if t1 > t_min {
            t_min = t1;
            normal = n;
        }
        t_max = t_max.min(t2);
        if t_min > t_max {
            return None;
        }
    }
    Some((t_min, normal))
}

fn ray_sphere(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<(f32, Vec3)> {
    let oc = origin - center;
    let b = oc.dot(dir);
    let c = oc.length_squared() - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let sq = disc.sqrt();
    let t = if -b - sq >= 0.0 { -b - sq } else { -b + sq };
    if t < 0.0 {
        return None;
    }
    let point = origin + dir * t;
    Some((t, (point - center).normalize_or_zero()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::vec3;

    #[test]
    fn falls_and_lands() {
        let mut w = World::new();
        w.set_gravity(vec3(0.0, 0.0, -9.81));
        let floor = w.add(Body::fixed(vec3(0.0, 0.0, -0.5)).with_box(10.0, 10.0, 1.0));
        let ball = w.add(Body::dynamic(vec3(0.0, 0.0, 2.0)).with_sphere(0.5));
        let mut landed = false;
        for _ in 0..180 {
            w.clear_events();
            w.step(1.0 / 60.0);
            landed |= w.events().iter().any(|e| matches!(e, PhysicsEvent::CollisionStarted { b, .. } if *b == floor));
        }
        assert!(landed);
        assert!(w.on_ground(ball));
        assert!((w.position(ball).z - 0.5).abs() < 0.05, "z = {}", w.position(ball).z);
    }

    #[test]
    fn walls_block_movement() {
        let mut w = World::new();
        let wall = w.add(Body::fixed(vec3(0.0, 3.0, 0.0)).with_box(10.0, 1.0, 3.0).with_tag("wall"));
        let player = w.add(Body::kinematic(Vec3::ZERO).with_box(0.5, 0.5, 1.8));
        let r = w.move_by(player, vec3(0.0, 5.0, 0.0));
        assert!(r.blocked);
        assert_eq!(r.hits, vec![wall]);
        assert!(w.position(player).y < 2.3);
        let r = w.move_by(player, vec3(0.0, -1.0, 0.0));
        assert!(!r.blocked);
    }

    #[test]
    fn sensors_and_rays() {
        let mut w = World::new();
        let zone = w.add(Body::sensor(vec3(5.0, 0.0, 0.0)).with_box(2.0, 2.0, 2.0));
        let p = w.add(Body::kinematic(Vec3::ZERO).with_sphere(0.3));
        w.move_by(p, vec3(5.0, 0.0, 0.0));
        assert!(w.events().contains(&PhysicsEvent::Entered { sensor: zone, body: p }));
        let target = w.add(Body::fixed(vec3(5.0, 10.0, 0.0)).with_box(1.0, 1.0, 1.0));
        let hit = w.raycast(w.position(p), Vec3::Y, 50.0, Some(p)).unwrap();
        assert_eq!(hit.body, target);
        assert!((hit.distance - 9.5).abs() < 1e-3);
    }

    fn earth() -> (World, BodyId) {
        let mut w = World::new();
        w.set_gravity(EARTH_GRAVITY);
        let floor = w.add(Body::fixed(vec3(0.0, 0.0, -0.5)).with_box(100.0, 100.0, 1.0));
        (w, floor)
    }

    fn run(w: &mut World, seconds: f32) -> Vec<PhysicsEvent> {
        let mut events = Vec::new();
        for _ in 0..(seconds * 60.0) as usize {
            w.clear_events();
            w.step(1.0 / 60.0);
            events.extend_from_slice(w.events());
        }
        events
    }

    #[test]
    fn landed_event_reports_fall_speed() {
        let (mut w, floor) = earth();
        let ball = w.add(Body::dynamic(vec3(0.0, 0.0, 5.5)).with_sphere(0.5));
        let events = run(&mut w, 2.0);
        let landed: Vec<_> = events.iter().filter(|e| matches!(e, PhysicsEvent::Landed { .. })).collect();
        assert_eq!(landed.len(), 1);
        let PhysicsEvent::Landed { body, on, speed } = landed[0] else { unreachable!() };
        assert_eq!((*body, *on), (ball, floor));
        // Falling 5 m: v = sqrt(2 * 9.81 * 5) ≈ 9.9 m/s.
        assert!((speed - 9.9).abs() < 0.6, "speed {speed}");
    }

    #[test]
    fn jump_reaches_height() {
        let (mut w, _) = earth();
        let p = w.add(Body::dynamic(vec3(0.0, 0.0, 1.0)).with_box(1.0, 1.0, 2.0));
        run(&mut w, 0.5);
        assert!(w.on_ground(p));
        assert!(w.jump(p, 2.0));
        assert!(!w.jump(p, 2.0), "no double jump");
        let mut highest: f32 = 0.0;
        for _ in 0..120 {
            w.step(1.0 / 60.0);
            highest = highest.max(w.position(p).z);
        }
        assert!((highest - 3.0).abs() < 0.15, "highest {highest}");
    }

    #[test]
    fn forces_mass_and_speed_limit() {
        let mut w = World::new();
        let light = w.add(Body::dynamic(Vec3::ZERO));
        let heavy = w.add(Body::dynamic(vec3(10.0, 0.0, 0.0)).with_mass(10.0));
        w.apply_force(light, vec3(60.0, 0.0, 0.0));
        w.apply_force(heavy, vec3(60.0, 0.0, 0.0));
        w.step(1.0 / 60.0);
        assert!((w.velocity(light).x - 1.0).abs() < 1e-4);
        assert!((w.velocity(heavy).x - 0.1).abs() < 1e-4);
        // The force only lasts one step.
        w.step(1.0 / 60.0);
        assert!((w.velocity(light).x - 1.0).abs() < 1e-4);

        let (mut w, _) = earth();
        let rock = w.add(Body::dynamic(vec3(0.0, 0.0, 100.0)).with_max_speed(5.0));
        run(&mut w, 1.0);
        assert!(w.velocity(rock).length() <= 5.0 + 1e-3);
    }

    #[test]
    fn heavy_ball_pushes_light_ball() {
        let mut w = World::new();
        let heavy = w.add(
            Body::dynamic(Vec3::ZERO).with_sphere(0.5).with_mass(10.0).with_bounciness(1.0).with_velocity(vec3(5.0, 0.0, 0.0)),
        );
        let light = w.add(Body::dynamic(vec3(2.0, 0.0, 0.0)).with_sphere(0.5).with_bounciness(1.0));
        let events = run(&mut w, 0.5);
        assert!(events.iter().any(|e| matches!(e, PhysicsEvent::CollisionStarted { .. })));
        // Elastic collision: the light ball flies off at about 9.1 m/s, the heavy one keeps 4.1.
        assert!((w.velocity(light).x - 9.09).abs() < 0.2, "light {}", w.velocity(light).x);
        assert!((w.velocity(heavy).x - 4.09).abs() < 0.2, "heavy {}", w.velocity(heavy).x);
    }

    #[test]
    fn player_pushes_crate_until_wall() {
        let (mut w, _) = earth();
        w.add(Body::fixed(vec3(5.0, 0.0, 1.0)).with_box(1.0, 10.0, 2.0));
        let crate_ = w.add(Body::dynamic(vec3(2.0, 0.0, 0.5)).with_box(1.0, 1.0, 1.0));
        let player = w.add(Body::kinematic(vec3(0.0, 0.0, 0.5)).with_box(1.0, 1.0, 1.0));
        run(&mut w, 0.3);
        let mut blocked = false;
        for _ in 0..40 {
            blocked = w.move_by(player, vec3(0.1, 0.0, 0.0)).blocked;
            run(&mut w, 1.0 / 60.0);
        }
        // The crate ends at the wall (x = 4.0) and the player behind it.
        assert!((w.position(crate_).x - 4.0).abs() < 0.1, "crate {}", w.position(crate_).x);
        assert!(w.position(player).x < 3.1);
        assert!(blocked);
    }

    #[test]
    fn platforms_carry_bodies() {
        let mut w = World::new();
        w.set_gravity(EARTH_GRAVITY);
        let lift = w.add(Body::kinematic(Vec3::ZERO).with_box(4.0, 4.0, 1.0).with_velocity(vec3(1.0, 0.0, 0.0)));
        let p = w.add(Body::dynamic(vec3(0.0, 0.0, 1.5)).with_box(1.0, 1.0, 1.0));
        run(&mut w, 0.5);
        assert_eq!(w.get(p).unwrap().ground, Some(lift));
        let offset = w.position(p).x - w.position(lift).x;
        run(&mut w, 1.5);
        assert!((w.position(p).x - w.position(lift).x - offset).abs() < 0.02);
    }

    #[test]
    fn facing_and_event_helpers() {
        let mut w = World::new();
        let p = w.add(Body::kinematic(Vec3::ZERO).with_sphere(0.3).with_facing(90.0));
        let coin = w.add(Body::sensor(vec3(2.0, 0.0, 0.0)).with_sphere(0.5).with_tag("coin"));
        assert_eq!(w.turn(p, -90.0), 0.0);
        w.turn(p, 90.0);
        assert_eq!(w.describe(p, coin), "12 o'clock, 2 meters");
        w.move_forward(p, 2.0);
        assert_eq!(w.entered_tag(p, "coin"), Some(coin));
        assert_eq!(w.triggered(coin), Some(p));
        assert_eq!(w.entered_tag(p, "door"), None);
        assert!((w.position(p).x - 2.0).abs() < 1e-4);
    }
}
