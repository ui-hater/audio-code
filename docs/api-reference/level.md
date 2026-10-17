# Levels from text maps

Draw a level as text and turn it into wall bodies with one call.

```rust
use audio_code::prelude::*;   // Level, World
```

## Map format

| Character | Becomes |
|-----------|---------|
| `#` | a fixed wall, tagged `"wall"`, 3 m high, standing on z = 0. Neighbouring `#` in one row are merged into one body. |
| `.` or space | empty floor |
| anything else | a remembered point (the tile center at z = 0), looked up with `Level::point` / `Level::points` |

The first line is north (highest y), columns go east (x). Tile (0, 0) is the south-west
corner. Leading and trailing blank lines and the common indentation are ignored, so the
map can be written inline in a string. No floor body is created; add one yourself if you
use gravity.

### `ctx.physics.build_level(map: &str, tile_size: f32) -> Level`

Creates the walls of the map in the world and returns a `Level` with the marked points.
`tile_size` is the width and depth of one character in meters (minimum 0.01).

```rust
const MAP: &str = "
    ##########
    #P...#..C#
    #....#...#
    #..C...E.#
    ##########
";

let level = ctx.physics.build_level(MAP, 2.0);   // every character is 2 x 2 m
let start = level.point('P').unwrap_or(Vec3::ZERO);
let player = ctx.physics.add(Body::kinematic(start).with_sphere(0.4));
ctx.follow(player);
for coin in level.points('C') {
    let id = ctx.physics.add(Body::sensor(coin).with_box(1.0, 1.0, 2.0).with_tag("coin"));
    ctx.loop_on(id, "coin");
}
```

## Level

### `level.point(marker: char) -> Option<Vec3>`

Center of the first tile marked with `marker` (north-west first), for example the player
start. `None` if the marker does not appear.

```rust
let level = ctx.physics.build_level("#P#", 1.0);
let start = level.point('P');   // Some(vec3(1.5, 0.5, 0.0))
```

### `level.points(marker: char) -> Vec<Vec3>`

Centers of all tiles marked with `marker`, from north-west to south-east. Empty if none.

```rust
let level = ctx.physics.build_level("E.E\n.E.", 2.0);
let enemies = level.points('E');   // three positions
```

### `level.size() -> (f32, f32)`

Size of the map in meters: (east-west, north-south).

```rust
let level = ctx.physics.build_level("####\n#..#\n####", 2.0);
let (width, depth) = level.size();   // (8.0, 6.0)
```

### `level.walls: Vec<BodyId>`

The wall bodies that were created, for example to remove the level again.

```rust
let level = ctx.physics.build_level("###", 1.0);
for wall in &level.walls {
    ctx.physics.remove(*wall);
}
```
