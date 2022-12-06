use std::{ops::Deref, sync::Arc};
#[macro_export]
macro_rules! const_assert {
    ($cond:expr) => {
        const _: () = if !($cond) {
            panic!("assertion failed")
        };
    };
    ($cond:expr, $msg:expr) => {
        const _: () = if !($cond) {
            panic!($msg)
        };
    };
}

pub trait SliceExt<T> {
    fn zip_map<V, F: Fn(&T, &T) -> V>(&self, other: &Self, f: F) -> Vec<V>;
}

impl<T> SliceExt<T> for [T] {
    #[inline]
    fn zip_map<V, F: Fn(&T, &T) -> V>(&self, other: &Self, f: F) -> Vec<V> {
        assert_eq!(self.len(), other.len());
        self.iter()
            .zip(other.iter())
            .map(|(a, b)| f(a, b))
            .collect()
    }
}

pub struct IndexedArc<T> {
    pub index: usize,
    pub arc: Arc<[T]>,
}

pub trait ArcSliceExt<T> {
    fn clone_at(&self, index: usize) -> IndexedArc<T>;
    fn _len(&self) -> usize;
}
#[inline]
pub fn iter_arc<T>(arc: &Arc<[T]>) -> impl Iterator<Item = IndexedArc<T>> + '_ {
    (0..arc.len()).map(move |i| arc.clone_at(i))
}

impl<T> ArcSliceExt<T> for Arc<[T]> {
    #[inline]
    fn clone_at(&self, index: usize) -> IndexedArc<T> {
        assert!(index < self.len());
        IndexedArc {
            index,
            arc: self.clone(),
        }
    }
    #[inline]
    fn _len(&self) -> usize {
        self.len()
    }
}

// we can dereference the IndexedArc to get underlying element
impl<T> Deref for IndexedArc<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        &self.arc[self.index]
    }
}

/// `Hook` serves as a reminder to clean up unfinished tasks.
/// If `Hook` is dropped but is not done, it will panic.
pub struct Hook {
    done: bool,
}

impl Hook {
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Hook { done: false }
    }
    #[inline]
    pub fn done(mut self) {
        self.done = true;
    }
}

impl Drop for Hook {
    #[inline]
    fn drop(&mut self) {
        if !self.done {
            panic!("internal error: Hook dropped without being done");
        }
    }
}

#[inline]
pub fn log_verify_status(num_verified: usize, num_total: usize, name: &str) {
    if num_verified == num_total {
        tracing::info!("[{}] All passed!", name);
    } else {
        tracing::error!(
            "[{}] # successful verifications: {}/{}",
            name,
            num_verified,
            num_total
        );
    }
}

pub fn bytes_to_seed_pairs(bytes: &[u8]) -> (u64, u64) {
    // XXX:This is for a proof for concept, as the entropy is only 64 bits
    let mut seed1 = [0u8; 8];
    let mut seed2 = [0u8; 8];
    seed1.copy_from_slice(&bytes[0..8]);
    seed2.copy_from_slice(&bytes[8..16]);
    (u64::from_le_bytes(seed1), u64::from_le_bytes(seed2))
}

pub fn batch_xor(a: &[u64], b: &[u64]) -> Vec<u64> {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b.iter()).map(|(a, b)| a ^ b).collect()
}
