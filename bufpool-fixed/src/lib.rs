pub mod buf;

use buf::FixedBuf;
use off64::usz;
use std::alloc::alloc_zeroed;
use std::alloc::Layout;
use std::cmp::max;
use std::collections::VecDeque;
use std::mem::size_of;
use std::sync::Arc;

// TODO Benchmark parking_lot::Mutex<VecDeque<>> against crossbeam_channel and flume. Also consider one allocator per thread, which could waste a lot of memory but also be very quick.
#[derive(Clone, Default)]
struct BufPoolForSize(Arc<parking_lot::Mutex<VecDeque<usize>>>);

struct Inner {
  align: usize,
  sizes: Vec<BufPoolForSize>,
}

/// Thread-safe pool of `FixedBuf` values, which are byte arrays with a fixed length.
/// This can be cheaply cloned to share the same underlying pool around.
/// The maximum length is 2^64, and the minimum alignment is 64. This allows storing the pointer and capacity in one `usize`, making it much faster to move the `FixedBuf` value around.
#[derive(Clone)]
pub struct FixedBufPool {
  inner: Arc<Inner>,
}

impl FixedBufPool {
  pub fn with_alignment(align: usize) -> Self {
    assert!(align > 64);
    assert!(align.is_power_of_two());
    let mut sizes = Vec::new();
    for _ in 0..64 {
      sizes.push(Default::default());
    }
    Self {
      inner: Arc::new(Inner { align, sizes }),
    }
  }

  pub fn new() -> Self {
    Self::with_alignment(max(64, size_of::<usize>()))
  }

  pub fn allocate_from_data(&self, data: impl AsRef<[u8]>) -> FixedBuf {
    let mut buf = self.allocate_with_zeros(data.as_ref().len());
    buf.copy_from_slice(data.as_ref());
    buf
  }

  /// `cap` must be a power of two. It can safely be zero, but it will still cause an allocation of one byte due to rounding.
  pub fn allocate_with_zeros(&self, cap: usize) -> FixedBuf {
    // FixedBuf values do not have a length + capacity, so check that `cap` will be fully used.
    assert!(cap.is_power_of_two());
    // This will round `0` to `1`.
    let cap = cap.next_power_of_two();
    // Release lock ASAP.
    let existing = self.inner.sizes[usz!(cap.ilog2())].0.lock().pop_front();
    let ptr_and_cap = if let Some(ptr_and_cap) = existing {
      ptr_and_cap
    } else {
      let ptr = unsafe { alloc_zeroed(Layout::from_size_align(cap, self.inner.align).unwrap()) };
      // Failed allocations may return null.
      assert!(!ptr.is_null());
      let raw = ptr as usize;
      assert_eq!(raw & (self.inner.align - 1), 0);
      raw | usz!(cap.ilog2())
    };
    FixedBuf {
      ptr_and_cap,
      pool: self.clone(),
    }
  }
}
