//! Build levels from text maps.
//!
//! Drawing a map as text is the quickest way to make a level:
//!
//! ```no_run
//! # use audio_code::prelude::*;
//! # fn f(ctx: &mut Context) {
//! let level = ctx.physics.build_level(
//!     "
//!     ##########
//!     #P...#..C#
//!     #....#...#
//!     #..C.....#
//!     ##########
//!     ",
//!     2.0, // every character is a 2 x 2 meter tile
//! );
//! let start = level.point('P').unwrap();
//! for coin in level.points('C') {
//!     // place a coin at `coin`
//! }
//! # }
//! ```
//!
//! * `#` becomes a fixed wall tagged `"wall"` (3 meters high). Neighbouring walls in a row are
//!   merged into one body.
//! * `.` and spaces are empty floor.
//! * Every other character is remembered as a point you can look up with [`Level::point`]
//!   and [`Level::points`].
//!
//! The first line of the map is north (highest y), columns go east (x). Leading and trailing
//! blank lines and the common indentation are ignored, so the map can be written inline.

use std::collections::HashMap;

use glam::Vec3;

use crate::physics::{Body, BodyId, World};

const WALL_HEIGHT: f32 = 3.0;

/// The result of [`World::build_level`].
#[derive(Debug, Clone, Default)]
pub struct Level {
    /// Wall bodies that were created.
    pub walls: Vec<BodyId>,
    points: HashMap<char, Vec<Vec3>>,
    width: f32,
    depth: f32,
}

impl Level {
    /// Center of the first tile marked with `marker`, e.g. the player start.
    pub fn point(&self, marker: char) -> Option<Vec3> {
        self.points.get(&marker).and_then(|p| p.first().copied())
    }

    /// Centers of all tiles marked with `marker`, from north-west to south-east.
    pub fn points(&self, marker: char) -> Vec<Vec3> {
        self.points.get(&marker).cloned().unwrap_or_default()
    }

    /// Size of the map in meters (east-west, north-south).
    pub fn size(&self) -> (f32, f32) {
        (self.width, self.depth)
    }
}

impl World {
    /// Creates walls from a text map. See the [`level`](crate::level) module.
    pub fn build_level(&mut self, map: &str, tile_size: f32) -> Level {
        let lines: Vec<&str> = map.lines().collect();
        let first = lines.iter().position(|l| !l.trim().is_empty()).unwrap_or(lines.len());
        let last = lines.iter().rposition(|l| !l.trim().is_empty()).map_or(first, |i| i + 1);
        let lines = &lines[first..last];
        let indent = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len() - l.trim_start().len())
            .min()
            .unwrap_or(0);
        let rows: Vec<Vec<char>> = lines.iter().map(|l| l.chars().skip(indent).collect()).collect();

        let tile = tile_size.max(0.01);
        let height = rows.len();
        let mut level = Level {
            width: rows.iter().map(Vec::len).max().unwrap_or(0) as f32 * tile,
            depth: height as f32 * tile,
            ..Level::default()
        };
        let center = |col: f32, row: usize| Vec3::new((col + 0.5) * tile, (height - 1 - row) as f32 * tile + tile * 0.5, 0.0);

        for (r, row) in rows.iter().enumerate() {
            let mut c = 0;
            while c < row.len() {
                match row[c] {
                    '#' => {
                        let start = c;
                        while c < row.len() && row[c] == '#' {
                            c += 1;
                        }
                        let run = (c - start) as f32;
                        let mut pos = center(start as f32 + (run - 1.0) / 2.0, r);
                        pos.z = WALL_HEIGHT / 2.0;
                        let wall = Body::fixed(pos).with_box(run * tile, tile, WALL_HEIGHT).with_tag("wall");
                        level.walls.push(self.add(wall));
                        continue;
                    }
                    '.' | ' ' => {}
                    marker => level.points.entry(marker).or_default().push(center(c as f32, r)),
                }
                c += 1;
            }
        }
        level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_walls_and_points() {
        let mut w = World::new();
        let level = w.build_level(
            "
            ####
            #P.#
            #.C#
            ####
            ",
            2.0,
        );
        // Top and bottom rows merge into one body each, the side walls are single tiles.
        assert_eq!(level.walls.len(), 6);
        assert_eq!(level.point('P'), Some(Vec3::new(3.0, 5.0, 0.0)));
        assert_eq!(level.points('C'), vec![Vec3::new(5.0, 3.0, 0.0)]);
        assert_eq!(level.size(), (8.0, 8.0));
        assert!(w.at_point(Vec3::new(1.0, 7.0, 1.0)).len() == 1, "north west corner is a wall");

        let player = w.add(Body::kinematic(level.point('P').unwrap()).with_sphere(0.4));
        assert!(w.move_by(player, Vec3::new(0.0, 5.0, 0.0)).blocked);
    }
}
