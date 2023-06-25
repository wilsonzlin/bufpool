use crate::BufPool;
use std::borrow::Borrow;
use std::borrow::BorrowMut;
use std::cmp::Ordering;
use std::fmt;
use std::fmt::Debug;
use std::hash::Hash;
use std::hash::Hasher;
use std::io;
use std::io::Write;
use std::ops::Deref;
use std::ops::DerefMut;
use std::ops::Index;
use std::ops::IndexMut;
use std::ops::RangeBounds;
use std::ptr;
use std::slice;
use std::slice::SliceIndex;

// We could've made this simpler instead of trying to copy Vec<u8>, but:
// - It would expose uninitialised data, unless we zero-fill every allocation (whether new or from the pool).
// - It would limit the usability, as it wouldn't be a drop in (or almost) replacement for Vec<u8>.
pub struct Buf {
  pub(crate) data: *mut u8,
  pub(crate) len: usize,
  pub(crate) cap: usize,
  pub(crate) pool: BufPool,
}

unsafe impl Send for Buf {}
unsafe impl Sync for Buf {}

// Not implemented:
// - `from_raw_parts*, into_*, leak, new*, reserve*, resize*, shrink_to*, try_reserve*, with_capacity*`: not applicable.
// - `as_mut_ptr, as_ptr, is_empty, len`: already available on `Deref/DerefMut`.
// - `insert, remove, retain*, swap_remove`: unlikely to be used.
// - `dedup*, drain*, spare_capacity_*, splice, split_*`: complex, may implement if required.
impl Buf {
  fn _as_full_slice(&mut self) -> &mut [u8] {
    unsafe { slice::from_raw_parts_mut(self.data, self.cap) }
  }

  pub fn allocator(&self) -> &BufPool {
    &self.pool
  }

  pub fn append(&mut self, other: &mut Buf) {
    // SAFETY: This will panic if out of bounds.
    self.extend_from_slice(other.as_slice());
    other.clear();
  }

  pub fn as_slice(&self) -> &[u8] {
    unsafe { slice::from_raw_parts(self.data, self.len) }
  }

  pub fn as_mut_slice(&mut self) -> &mut [u8] {
    unsafe { slice::from_raw_parts_mut(self.data, self.len) }
  }

  pub fn capacity(&self) -> usize {
    self.cap
  }

  pub fn clear(&mut self) {
    self.len = 0;
  }

  pub fn extend_from_slice(&mut self, other: &[u8]) {
    let idx = self.len;
    // SAFETY: This will panic if out of bounds.
    self._as_full_slice()[idx..idx + other.len()].copy_from_slice(other);
    self.len += other.len();
  }

  pub fn extend_from_within(&mut self, src: impl RangeBounds<usize>) {
    let idx = self.len;
    // SAFETY: This will panic if out of bounds.
    self._as_full_slice().copy_within(src, idx);
  }

  pub fn push(&mut self, v: u8) {
    // SAFETY: This will panic if out of bounds.
    self.extend_from_slice(&[v]);
  }

  pub fn pop(&mut self) -> Option<u8> {
    if self.len == 0 {
      return None;
    };
    self.len -= 1;
    let idx = self.len;
    Some(self._as_full_slice()[idx])
  }

  pub unsafe fn set_len(&mut self, len: usize) {
    assert!(len <= self.cap);
    self.len = len;
  }

  pub fn truncate(&mut self, len: usize) {
    if len >= self.len {
      return;
    };
    self.len = len;
  }
}

impl AsRef<[u8]> for Buf {
  fn as_ref(&self) -> &[u8] {
    self.as_slice()
  }
}

impl AsMut<[u8]> for Buf {
  fn as_mut(&mut self) -> &mut [u8] {
    self.as_mut_slice()
  }
}

impl Borrow<[u8]> for Buf {
  fn borrow(&self) -> &[u8] {
    self.as_slice()
  }
}

impl BorrowMut<[u8]> for Buf {
  fn borrow_mut(&mut self) -> &mut [u8] {
    self.as_mut_slice()
  }
}

impl Clone for Buf {
  /// Uses the same pool that the current `Buf` was allocated from.
  fn clone(&self) -> Self {
    let mut clone = self.pool.allocate(self.len());
    clone.extend_from_slice(self.as_slice());
    clone
  }
}

impl Debug for Buf {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Buf")
      .field("data", &self.as_slice())
      .finish()
  }
}

impl Deref for Buf {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    self.as_slice()
  }
}

impl DerefMut for Buf {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.as_mut_slice()
  }
}

impl Drop for Buf {
  fn drop(&mut self) {
    #[cfg(not(feature = "no-pool"))]
    self.pool.inner.sizes[self.capacity().ilog2() as usize]
      .0
      .lock()
      .push_back(self.data);
    #[cfg(feature = "no-pool")]
    unsafe {
      let layout = std::alloc::Layout::from_size_align(self.cap, self.pool.inner.align).unwrap();
      std::alloc::dealloc(self.data, layout)
    }
  }
}

impl Eq for Buf {}

impl<'a> Extend<&'a u8> for Buf {
  fn extend<T: IntoIterator<Item = &'a u8>>(&mut self, iter: T) {
    for b in iter {
      self.push(*b);
    }
  }
}

impl Extend<u8> for Buf {
  fn extend<T: IntoIterator<Item = u8>>(&mut self, iter: T) {
    for b in iter {
      self.push(b);
    }
  }
}

impl Hash for Buf {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.as_slice().hash(state);
  }
}

// Copied from Vec implementation.
impl<I: SliceIndex<[u8]>> Index<I> for Buf {
  type Output = I::Output;

  fn index(&self, index: I) -> &Self::Output {
    Index::index(self.as_slice(), index)
  }
}

// Copied from Vec implementation.
impl<I: SliceIndex<[u8]>> IndexMut<I> for Buf {
  fn index_mut(&mut self, index: I) -> &mut Self::Output {
    IndexMut::index_mut(self.as_mut_slice(), index)
  }
}

impl Ord for Buf {
  fn cmp(&self, other: &Self) -> Ordering {
    self.as_slice().cmp(other.as_slice())
  }
}

impl PartialEq for Buf {
  fn eq(&self, other: &Self) -> bool {
    self.len == other.len && (ptr::eq(self.data, other.data) || self.as_slice() == other.as_slice())
  }
}

impl PartialOrd for Buf {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    self.as_slice().partial_cmp(other.as_slice())
  }
}

impl Write for Buf {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    self.extend_from_slice(buf);
    Ok(buf.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}
