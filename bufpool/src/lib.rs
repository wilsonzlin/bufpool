pub mod buf;

use buf::Buf;
use once_cell::sync::Lazy;
use std::alloc::alloc;
use std::alloc::Layout;
use std::collections::VecDeque;
use std::mem::size_of;
use std::panic::RefUnwindSafe;
use std::panic::UnwindSafe;
use std::sync::Arc;

// TODO Benchmark parking_lot::Mutex<VecDeque<>> against crossbeam_channel and flume. Also consider one allocator per thread, which could waste a lot of memory but also be very quick.
#[derive(Clone, Default)]
struct BufPoolForSize(Arc<parking_lot::Mutex<VecDeque<*mut u8>>>);

unsafe impl Send for BufPoolForSize {}
unsafe impl Sync for BufPoolForSize {}
impl UnwindSafe for BufPoolForSize {}
impl RefUnwindSafe for BufPoolForSize {}

struct BufPoolInner {
  align: usize,
  #[cfg(not(feature = "no-pool"))]
  sizes: Vec<BufPoolForSize>,
}

#[derive(Clone)]
pub struct BufPool {
  inner: Arc<BufPoolInner>,
}

impl BufPool {
  pub fn with_alignment(align: usize) -> Self {
    assert!(align > 0);
    assert!(align.is_power_of_two());
    Self {
      inner: Arc::new(BufPoolInner {
        align,
        #[cfg(not(feature = "no-pool"))]
        sizes: (0..(size_of::<usize>() * 8))
          .map(|_| Default::default())
          .collect(),
      }),
    }
  }

  pub fn new() -> Self {
    Self::with_alignment(size_of::<usize>())
  }

  fn system_allocate_raw(&self, cap: usize) -> *mut u8 {
    unsafe { alloc(Layout::from_size_align(cap, self.inner.align).unwrap()) }
  }

  /// NOTE: This provides a Buf that can grow to `cap`, but has an initial length of zero. Use `allocate_with_zeros` to return something equivalent to `vec![0u8; cap]`.
  /// `cap` can safely be zero, but it will still cause an allocation of one byte due to rounding.
  pub fn allocate(&self, cap: usize) -> Buf {
    // This will round `0` to `1`.
    let cap = cap.next_power_of_two();

    #[cfg(not(feature = "no-pool"))]
    let data = if let Some(data) = self.inner.sizes[cap.ilog2() as usize].0.lock().pop_front() {
      data
    } else {
      self.system_allocate_raw(cap)
    };
    #[cfg(feature = "no-pool")]
    let data = self.system_allocate_raw(cap);

    // Failed allocations may return null.
    assert!(!data.is_null());

    Buf {
      data,
      len: 0,
      cap,
      pool: self.clone(),
    }
  }

  pub fn allocate_from_data(&self, data: impl AsRef<[u8]>) -> Buf {
    let mut buf = self.allocate(data.as_ref().len());
    buf.extend_from_slice(data.as_ref());
    buf
  }

  pub fn allocate_from_iter(&self, data: impl IntoIterator<Item = u8>, len: usize) -> Buf {
    let mut buf = self.allocate(len);
    buf.extend(data);
    buf
  }

  /// The returned Buf will have a length equal to the capacity, filled with uninitialised bytes.
  pub fn allocate_uninitialised(&self, len: usize) -> Buf {
    let mut buf = self.allocate(len);
    unsafe { buf.set_len(len) };
    buf
  }

  pub fn allocate_with_fill(&self, val: u8, len: usize) -> Buf {
    let mut buf = self.allocate_uninitialised(len);
    buf.fill(val);
    buf
  }

  pub fn allocate_with_zeros(&self, len: usize) -> Buf {
    self.allocate_with_fill(0, len)
  }
}

pub static BUFPOOL: Lazy<BufPool> = Lazy::new(|| BufPool::new());
