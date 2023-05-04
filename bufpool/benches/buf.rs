use bufpool::BUFPOOL;
use criterion::black_box;
use criterion::criterion_group;
use criterion::criterion_main;
use criterion::Criterion;
use std::alloc::alloc;
use std::alloc::alloc_zeroed;
use std::alloc::Layout;

fn criterion_benchmark(c: &mut Criterion) {
  // WARNING: `black_box` prevents the value from being dropped, so our pool allocations never go back into the pool and get reused; the Drop is therefore also not benchmarked.
  // WARNING: Because of the above reason, your system may run out of memory and freeze or crash; each `bench_function` could run hundreds of millions of times. **Be extra careful when changing `size`.**
  let size = 1;
  c.bench_function("malloc", |b| {
    b.iter(|| black_box(unsafe { libc::malloc(size) }))
  });
  c.bench_function("calloc", |b| {
    b.iter(|| black_box(unsafe { libc::calloc(size, 1) }))
  });
  c.bench_function("alloc", |b| {
    b.iter(|| black_box(unsafe { alloc(Layout::from_size_align(size, 1).unwrap()) }))
  });
  c.bench_function("alloc_zeroed", |b| {
    b.iter(|| black_box(unsafe { alloc_zeroed(Layout::from_size_align(size, 1).unwrap()) }))
  });
  c.bench_function("alloc unchecked", |b| {
    b.iter(|| black_box(unsafe { alloc(Layout::from_size_align_unchecked(size, 1)) }))
  });
  c.bench_function("alloc_zeroed unchecked", |b| {
    b.iter(|| black_box(unsafe { alloc_zeroed(Layout::from_size_align_unchecked(size, 1)) }))
  });
  c.bench_function("BufPoll::allocate", |b| {
    b.iter(|| black_box(BUFPOOL.allocate(size)))
  });
  c.bench_function("Vec::with_capacity", |b| {
    b.iter(|| black_box(Vec::<u8>::with_capacity(size)))
  });
  c.bench_function("BufPoll::allocate_uninitialised", |b| {
    b.iter(|| black_box(BUFPOOL.allocate_uninitialised(size)))
  });
  c.bench_function("BufPoll::allocate_with_zeros", |b| {
    b.iter(|| black_box(BUFPOOL.allocate_with_zeros(size)))
  });
  c.bench_function("vec![0u8; size]", |b| b.iter(|| black_box(vec![0u8; size])));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
