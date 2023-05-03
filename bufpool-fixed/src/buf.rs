use crate::FixedBufPool;
use off64::usz;
use std::borrow::Borrow;
use std::borrow::BorrowMut;
use std::cmp::Ordering;
use std::fmt;
use std::fmt::Debug;
use std::hash::Hash;
use std::hash::Hasher;
use std::ops::Deref;
use std::ops::DerefMut;
use std::ops::Index;
use std::ops::IndexMut;
use std::slice;
use std::slice::SliceIndex;

pub struct FixedBuf {
  pub(crate) ptr_and_cap: usize,
  pub(crate) pool: FixedBufPool,
}

unsafe impl Send for FixedBuf {}
unsafe impl Sync for FixedBuf {}

impl FixedBuf {
  fn ptr(&self) -> *mut u8 {
    let raw = self.ptr_and_cap & !(self.pool.inner.align - 1);
    raw as *mut u8
  }

  pub fn allocator(&self) -> &FixedBufPool {
    &self.pool
  }

  pub fn as_slice(&self) -> &[u8] {
    unsafe { slice::from_raw_parts(self.ptr(), self.capacity()) }
  }

  pub fn as_mut_slice(&mut self) -> &mut [u8] {
    unsafe { slice::from_raw_parts_mut(self.ptr(), self.capacity()) }
  }

  pub fn capacity(&self) -> usize {
    let l2 = self.ptr_and_cap & (self.pool.inner.align - 1);
    1 << l2
  }
}

impl AsRef<[u8]> for FixedBuf {
  fn as_ref(&self) -> &[u8] {
    self.as_slice()
  }
}

impl AsMut<[u8]> for FixedBuf {
  fn as_mut(&mut self) -> &mut [u8] {
    self.as_mut_slice()
  }
}

impl Borrow<[u8]> for FixedBuf {
  fn borrow(&self) -> &[u8] {
    self.as_slice()
  }
}

impl BorrowMut<[u8]> for FixedBuf {
  fn borrow_mut(&mut self) -> &mut [u8] {
    self.as_mut_slice()
  }
}

impl Clone for FixedBuf {
  /// Uses the same pool that the current `FixedBuf` was allocated from.
  fn clone(&self) -> Self {
    self.pool.allocate_from_data(self)
  }
}

impl Debug for FixedBuf {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("FixedBuf")
      .field("data", &self.as_slice())
      .finish()
  }
}

impl Deref for FixedBuf {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    self.as_slice()
  }
}

impl DerefMut for FixedBuf {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.as_mut_slice()
  }
}

impl Drop for FixedBuf {
  fn drop(&mut self) {
    self.pool.inner.sizes[usz!(self.capacity().ilog2())]
      .0
      .lock()
      .push_back(self.ptr_and_cap);
  }
}

impl Eq for FixedBuf {}

impl Hash for FixedBuf {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.as_slice().hash(state);
  }
}

// Copied from Vec implementation.
impl<I: SliceIndex<[u8]>> Index<I> for FixedBuf {
  type Output = I::Output;

  fn index(&self, index: I) -> &Self::Output {
    Index::index(self.as_slice(), index)
  }
}

// Copied from Vec implementation.
impl<I: SliceIndex<[u8]>> IndexMut<I> for FixedBuf {
  fn index_mut(&mut self, index: I) -> &mut Self::Output {
    IndexMut::index_mut(self.as_mut_slice(), index)
  }
}

impl Ord for FixedBuf {
  fn cmp(&self, other: &Self) -> Ordering {
    self.as_slice().cmp(other.as_slice())
  }
}

impl PartialEq for FixedBuf {
  fn eq(&self, other: &Self) -> bool {
    self.ptr_and_cap == other.ptr_and_cap || self.as_slice() == other.as_slice()
  }
}

impl PartialOrd for FixedBuf {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    self.as_slice().partial_cmp(other.as_slice())
  }
}
