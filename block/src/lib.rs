#[cfg(all(target_arch = "x86_64", target_feature = "pclmulqdq"))]
pub mod x86;

use bytes::Bytes;
#[cfg(all(target_arch = "x86_64", target_feature = "pclmulqdq"))]
pub use x86::*;

/// Helper trait for a list of blocks. Should be implemented by [Block].
pub trait Blocks {
    /// view the list of blocks as a slice of bytes. This operation is O(1)
    fn as_u8_slice(&self) -> &[u8];

    /// view the list of blocks as a slice of bytes. This operation is O(1)
    fn as_u8_slice_mut(&mut self) -> &mut [u8];

    /// Copy the list of blocks to a byte vector. This operation is O(n).
    fn store_to_bytes(&self) -> Bytes {
        Bytes::copy_from_slice(self.as_u8_slice())
    }
}

pub trait AsBlockSlice {
    fn as_block_slice(&self) -> &[Block];

    fn as_block_slice_mut(&mut self) -> &mut [Block];
}

impl AsBlockSlice for [u8] {
    fn as_block_slice(&self) -> &[Block] {
        Block::batch_cast_from_u8_slice(self)
    }

    fn as_block_slice_mut(&mut self) -> &mut [Block] {
        Block::batch_cast_from_u8_slice_mut(self)
    }
}

#[cfg(not(all(target_arch = "x86_64", target_feature = "pclmulqdq")))]
pub mod fallback {
    compile_error!("This library only supports x86-64 with PCLMULQDQ instruction. If you are already running on x86-64 architecture, try compile it with environment variable RUSTFLAGS='-c target-cpu=native' ");
}

#[cfg(not(all(target_arch = "x86_64", target_feature = "pclmulqdq")))]
pub use fallback::*;
