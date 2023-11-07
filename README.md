# Dodgems - A simple bump allocator library

This crate provides a fast, single-threaded bump allocator for use in performance
sensitive contexts.

**⚠️ It is not a general purpose allocator: you need to have another
(supposedly slower) allocator to back it up.** By default, it is the global allocator.

It can be used for quick and dirty allocation in a loop, where you know memory
can be reclaimed all at once at the end.

# Example
```rust
#![feature(allocator_api)]
use dodgems::BumpCar;

let mut bumpcar = BumpCar::new(1024).unwrap(); // 1kB capacity

for i in 0..100 {
    // allocate with the allocator api
    let mut v = Vec::new_in(&bumpcar); 
    v.push(42);
    // small fast allocations in hot loop

    drop(v);
    // reset the capacity once every allocation has been dropped
    bumpcar.reset(); 
}

drop(bumpcar)
```

Until the `allocator_api` is stable, this crate requires nightly.
