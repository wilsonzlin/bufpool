pub mod buf;

use buf::Buf;
use off64::usz;
use once_cell::sync::Lazy;
use std::collections::VecDeque;
use std::mem::forget;
use std::mem::size_of;
use std::sync::Arc;

#[derive(Clone, Default)]
struct BufPoolForSize(Arc<parking_lot::Mutex<VecDeque<*mut u8>>>);

unsafe impl Send for BufPoolForSize {}
unsafe impl Sync for BufPoolForSize {}

#[derive(Clone)]
pub struct BufPool {
  sizes: Arc<Vec<BufPoolForSize>>,
}

impl BufPool {
  pub fn new() -> Self {
    let mut sizes = Vec::new();
    for _ in 0..(size_of::<usize>() * 8) {
      sizes.push(Default::default());
    }
    Self {
      sizes: Arc::new(sizes),
    }
  }

  /// NOTE: This provides a Buf that can grow to `cap`, but it has an initial length of zero. Use `allocate_with_zeros` to return something equivalent to `vec![0u8; cap]`.
  pub fn allocate(&self, cap: usize) -> Buf {
    let cap = cap.next_power_of_two();
    let pool = self.sizes[usz!(cap.ilog2())].clone();
    // Release lock ASAP.
    let existing = pool.0.lock().pop_front();
    let data = if let Some(data) = existing {
      data
    } else {
      // We can't use `Box::new([0u8; cap])` because `cap` isn't constant.
      let mut new = vec![0u8; cap];
      assert_eq!(new.capacity(), cap);
      let data = new.as_mut_ptr();
      forget(new);
      data
    };
    Buf {
      data,
      len: 0,
      cap,
      pool,
    }
  }

  pub fn allocate_with_zeros(&self, len: usize) -> Buf {
    let mut buf = self.allocate(len);
    unsafe { buf.set_len(len) };
    buf.fill(0);
    buf
  }
}

pub static BUFPOOL: Lazy<BufPool> = Lazy::new(|| BufPool::new());
