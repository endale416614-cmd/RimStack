//! An id from an earlier reading finds nothing, rather than the wrong mod.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::atomic::{AtomicU32, Ordering};

/// An id is one 32-bit number: the place in the low 16 bits, the reading in the 15 above.
/// A reading of more mods than the 16 bits hold names the rest with -1, none.
pub const MOST_MODS: usize = 1 << 16;
/// Readings are told apart before the count starts again: far more than one run reads.
const READINGS: u32 = 1 << 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModId {
    index: usize,
    reading: u32,
}

impl ModId {
    /// Only a library being built, and tests, make one.
    pub fn new(reading: u32, index: usize) -> Self {
        Self {
            index,
            reading: reading % READINGS,
        }
    }

    pub(crate) fn index(self) -> usize {
        self.index
    }

    pub(crate) fn reading(self) -> u32 {
        self.reading
    }

    pub fn to_packed(self) -> i32 {
        match self.index < MOST_MODS {
            true => ((self.reading << 16) as i32) | self.index as i32,
            false => -1,
        }
    }

    /// A negative number is no mod.
    pub fn from_packed(held: i32) -> Option<Self> {
        let held = u32::try_from(held).ok()?;
        Some(Self::new(held >> 16, (held & 0xffff) as usize))
    }
}

pub(crate) fn next_reading() -> u32 {
    static READ: AtomicU32 = AtomicU32::new(1);
    READ.fetch_add(1, Ordering::Relaxed) % READINGS
}

impl Serialize for ModId {
    fn serialize<S: Serializer>(&self, to: S) -> Result<S::Ok, S::Error> {
        to.serialize_i32(self.to_packed())
    }
}

impl<'de> Deserialize<'de> for ModId {
    fn deserialize<D: Deserializer<'de>>(from: D) -> Result<Self, D::Error> {
        let held = i32::deserialize(from)?;
        Self::from_packed(held).ok_or_else(|| serde::de::Error::custom("no mod"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_goes_to_the_window_and_back_unchanged() {
        for id in [
            ModId::new(0, 0),
            ModId::new(7, 2_300),
            ModId::new(READINGS - 1, MOST_MODS - 1),
        ] {
            assert!(id.to_packed() >= 0);
            assert_eq!(ModId::from_packed(id.to_packed()), Some(id));
            let said = serde_json::to_string(&id).unwrap();
            assert_eq!(serde_json::from_str::<ModId>(&said).unwrap(), id);
        }
        assert_eq!(ModId::from_packed(-1), None);
        assert!(serde_json::from_str::<ModId>("-1").is_err());
    }

    #[test]
    fn the_same_place_in_two_readings_is_two_ids() {
        let (first, second) = (next_reading(), next_reading());
        assert_ne!(ModId::new(first, 3), ModId::new(second, 3));
    }

    #[test]
    fn a_place_past_the_most_mods_is_no_mod_to_the_window() {
        assert_eq!(ModId::new(1, MOST_MODS).to_packed(), -1);
    }
}
