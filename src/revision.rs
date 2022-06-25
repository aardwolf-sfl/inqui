use std::num::NonZeroU64;

const START: u64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(NonZeroU64);

impl Revision {
    pub fn new() -> Self {
        Self::from(START)
    }

    pub fn increment(&mut self) {
        *self = Self::from(self.0.get() + 1)
    }

    pub fn as_raw(&self) -> u64 {
        self.0.get()
    }

    fn from(raw: u64) -> Self {
        Self(NonZeroU64::new(raw).unwrap())
    }
}

impl Default for Revision {
    fn default() -> Self {
        Self::new()
    }
}
