//! [`Packet`]: one message between players, and its binary encoding.

use glam::{Vec2, Vec3};

use super::PeerId;

/// One value inside a [`Packet`]. Made automatically by [`Packet::with`] from numbers,
/// `bool`, text, [`Vec2`], [`Vec3`], [`PeerId`] and byte vectors.
#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    /// A whole number (all integer types).
    Int(i64),
    /// A decimal number (`f32` and `f64`).
    Float(f64),
    /// `true` or `false`.
    Bool(bool),
    /// Text.
    Text(String),
    /// A 2D vector.
    Vec2(Vec2),
    /// A 3D vector, e.g. a position.
    Vec3(Vec3),
    /// Raw bytes for your own formats.
    Bytes(Vec<u8>),
}

macro_rules! int_args {
    ($($t:ty),*) => {$(
        impl From<$t> for Arg {
            fn from(v: $t) -> Self {
                Arg::Int(v as i64)
            }
        }
    )*};
}
int_args!(i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

impl From<PeerId> for Arg {
    fn from(v: PeerId) -> Self {
        Arg::Int(v.number() as i64)
    }
}

impl From<f32> for Arg {
    fn from(v: f32) -> Self {
        Arg::Float(v as f64)
    }
}

impl From<f64> for Arg {
    fn from(v: f64) -> Self {
        Arg::Float(v)
    }
}

impl From<bool> for Arg {
    fn from(v: bool) -> Self {
        Arg::Bool(v)
    }
}

impl From<&str> for Arg {
    fn from(v: &str) -> Self {
        Arg::Text(v.to_string())
    }
}

impl From<String> for Arg {
    fn from(v: String) -> Self {
        Arg::Text(v)
    }
}

impl From<&String> for Arg {
    fn from(v: &String) -> Self {
        Arg::Text(v.clone())
    }
}

impl From<Vec2> for Arg {
    fn from(v: Vec2) -> Self {
        Arg::Vec2(v)
    }
}

impl From<Vec3> for Arg {
    fn from(v: Vec3) -> Self {
        Arg::Vec3(v)
    }
}

impl From<Vec<u8>> for Arg {
    fn from(v: Vec<u8>) -> Self {
        Arg::Bytes(v)
    }
}

impl From<&[u8]> for Arg {
    fn from(v: &[u8]) -> Self {
        Arg::Bytes(v.to_vec())
    }
}

/// One message between players: a kind (what it is about) and a list of values.
///
/// ```
/// use audio_code::prelude::*;
///
/// let packet = Packet::new("move").with(vec3(3.0, 4.0, 0.0)).with(90.0).with("running");
/// assert_eq!(packet.kind(), "move");
/// assert_eq!(packet.vec3(0), vec3(3.0, 4.0, 0.0));
/// assert_eq!(packet.float(1), 90.0);
/// assert_eq!(packet.text(2), "running");
/// // Missing or other values give a neutral result instead of a crash:
/// assert_eq!(packet.int(7), 0);
/// ```
///
/// Text converts into a packet without values, so `ctx.net.send_all("ready")` works.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Packet {
    kind: String,
    args: Vec<Arg>,
}

impl Packet {
    /// A packet of the given kind without values.
    pub fn new(kind: impl Into<String>) -> Self {
        Self { kind: kind.into(), args: Vec::new() }
    }

    /// Adds a value (builder style).
    pub fn with(mut self, value: impl Into<Arg>) -> Self {
        self.args.push(value.into());
        self
    }

    /// Adds a value.
    pub fn push(&mut self, value: impl Into<Arg>) {
        self.args.push(value.into());
    }

    /// What the packet is about, e.g. `"move"` or `"chat"`.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Whether the packet is of this kind.
    pub fn is(&self, kind: &str) -> bool {
        self.kind == kind
    }

    /// All values.
    pub fn args(&self) -> &[Arg] {
        &self.args
    }

    /// Number of values.
    pub fn len(&self) -> usize {
        self.args.len()
    }

    /// Whether the packet has no values.
    pub fn is_empty(&self) -> bool {
        self.args.is_empty()
    }

    /// The value at `index`, if there is one.
    pub fn get(&self, index: usize) -> Option<&Arg> {
        self.args.get(index)
    }

    /// The value at `index` as a whole number. Decimals are cut off, `bool` gives 0 or 1,
    /// text is parsed. Anything else gives 0.
    pub fn int(&self, index: usize) -> i64 {
        match self.get(index) {
            Some(Arg::Int(v)) => *v,
            Some(Arg::Float(v)) => *v as i64,
            Some(Arg::Bool(v)) => *v as i64,
            Some(Arg::Text(v)) => v.trim().parse().unwrap_or(0),
            _ => 0,
        }
    }

    /// The value at `index` as a decimal number. Whole numbers and `bool` convert, text is
    /// parsed. Anything else gives 0.0.
    pub fn float(&self, index: usize) -> f32 {
        match self.get(index) {
            Some(Arg::Int(v)) => *v as f32,
            Some(Arg::Float(v)) => *v as f32,
            Some(Arg::Bool(v)) => *v as u8 as f32,
            Some(Arg::Text(v)) => v.trim().parse().unwrap_or(0.0),
            _ => 0.0,
        }
    }

    /// The value at `index` as `bool`. Numbers are `true` when not zero, text when it is
    /// `"true"`. Anything else gives `false`.
    pub fn bool(&self, index: usize) -> bool {
        match self.get(index) {
            Some(Arg::Bool(v)) => *v,
            Some(Arg::Int(v)) => *v != 0,
            Some(Arg::Float(v)) => *v != 0.0,
            Some(Arg::Text(v)) => v.trim() == "true",
            _ => false,
        }
    }

    /// The value at `index` as text, or `""` if it is not text.
    pub fn text(&self, index: usize) -> &str {
        match self.get(index) {
            Some(Arg::Text(v)) => v,
            _ => "",
        }
    }

    /// The value at `index` as a 2D vector. A 3D vector loses its z. Anything else gives
    /// `Vec2::ZERO`.
    pub fn vec2(&self, index: usize) -> Vec2 {
        match self.get(index) {
            Some(Arg::Vec2(v)) => *v,
            Some(Arg::Vec3(v)) => v.truncate(),
            _ => Vec2::ZERO,
        }
    }

    /// The value at `index` as a 3D vector. A 2D vector gets z = 0. Anything else gives
    /// `Vec3::ZERO`.
    pub fn vec3(&self, index: usize) -> Vec3 {
        match self.get(index) {
            Some(Arg::Vec3(v)) => *v,
            Some(Arg::Vec2(v)) => v.extend(0.0),
            _ => Vec3::ZERO,
        }
    }

    /// The value at `index` as a player id (stored with `.with(peer)`). Anything that is not
    /// a valid number gives [`PeerId::SERVER`].
    pub fn peer(&self, index: usize) -> PeerId {
        PeerId::from_number(u32::try_from(self.int(index)).unwrap_or(0))
    }

    /// The value at `index` as bytes, or an empty slice if it is not bytes.
    pub fn bytes(&self, index: usize) -> &[u8] {
        match self.get(index) {
            Some(Arg::Bytes(v)) => v,
            _ => &[],
        }
    }

    pub(crate) fn encode(&self, out: &mut Vec<u8>) {
        put_str(out, &self.kind);
        out.extend_from_slice(&(self.args.len() as u32).to_le_bytes());
        for arg in &self.args {
            match arg {
                Arg::Int(v) => {
                    out.push(0);
                    out.extend_from_slice(&v.to_le_bytes());
                }
                Arg::Float(v) => {
                    out.push(1);
                    out.extend_from_slice(&v.to_le_bytes());
                }
                Arg::Bool(v) => {
                    out.push(2);
                    out.push(*v as u8);
                }
                Arg::Text(v) => {
                    out.push(3);
                    put_str(out, v);
                }
                Arg::Vec2(v) => {
                    out.push(4);
                    for c in v.to_array() {
                        out.extend_from_slice(&c.to_le_bytes());
                    }
                }
                Arg::Vec3(v) => {
                    out.push(5);
                    for c in v.to_array() {
                        out.extend_from_slice(&c.to_le_bytes());
                    }
                }
                Arg::Bytes(v) => {
                    out.push(6);
                    put_bytes(out, v);
                }
            }
        }
    }

    pub(crate) fn decode(r: &mut Reader) -> Option<Self> {
        let kind = r.str()?;
        let count = r.u32()? as usize;
        // Every value takes at least two bytes, so a huge count is rejected early.
        if count > r.remaining() / 2 {
            return None;
        }
        let mut args = Vec::with_capacity(count);
        for _ in 0..count {
            args.push(match r.u8()? {
                0 => Arg::Int(i64::from_le_bytes(r.array()?)),
                1 => Arg::Float(f64::from_le_bytes(r.array()?)),
                2 => Arg::Bool(r.u8()? != 0),
                3 => Arg::Text(r.str()?),
                4 => Arg::Vec2(Vec2::new(r.f32()?, r.f32()?)),
                5 => Arg::Vec3(Vec3::new(r.f32()?, r.f32()?, r.f32()?)),
                6 => Arg::Bytes(r.bytes()?.to_vec()),
                _ => return None,
            });
        }
        Some(Self { kind, args })
    }
}

impl From<&str> for Packet {
    fn from(kind: &str) -> Self {
        Packet::new(kind)
    }
}

impl From<String> for Packet {
    fn from(kind: String) -> Self {
        Packet::new(kind)
    }
}

impl From<&Packet> for Packet {
    fn from(packet: &Packet) -> Self {
        packet.clone()
    }
}

pub(crate) fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

pub(crate) fn put_str(out: &mut Vec<u8>, text: &str) {
    put_bytes(out, text.as_bytes());
}

/// Reads values from a received frame. Every method returns `None` when the data ends early.
pub(crate) struct Reader<'a> {
    data: &'a [u8],
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.data.len()
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if n > self.data.len() {
            return None;
        }
        let (head, tail) = self.data.split_at(n);
        self.data = tail;
        Some(head)
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }

    pub(crate) fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    pub(crate) fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.array()?))
    }

    pub(crate) fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.array()?))
    }

    pub(crate) fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.array()?))
    }

    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.array()?))
    }

    pub(crate) fn bytes(&mut self) -> Option<&'a [u8]> {
        let len = self.u32()? as usize;
        self.take(len)
    }

    pub(crate) fn str(&mut self) -> Option<String> {
        Some(String::from_utf8_lossy(self.bytes()?).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let packet = Packet::new("all")
            .with(-5)
            .with(2.5)
            .with(true)
            .with("hä?")
            .with(Vec2::new(1.0, 2.0))
            .with(Vec3::new(1.0, 2.0, 3.0))
            .with(vec![1u8, 2, 3]);
        let mut bytes = Vec::new();
        packet.encode(&mut bytes);
        let decoded = Packet::decode(&mut Reader::new(&bytes)).unwrap();
        assert_eq!(decoded, packet);
        assert!(Packet::decode(&mut Reader::new(&bytes[..bytes.len() - 1])).is_none());
    }

    #[test]
    fn lenient_getters() {
        let p = Packet::new("x").with(3).with("4.5").with(Vec2::new(1.0, 2.0));
        assert_eq!(p.float(0), 3.0);
        assert_eq!(p.float(1), 4.5);
        assert_eq!(p.int(1), 0);
        assert_eq!(p.text(0), "");
        assert_eq!(p.vec3(2), Vec3::new(1.0, 2.0, 0.0));
        assert!(p.bool(0));
        assert!(!p.bool(9));
    }
}
