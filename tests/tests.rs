#![feature(allocator_api)]

use std::mem::size_of;

use dodgems::BumpCar;

#[test]
fn allocate_vec() {
    let mut b = BumpCar::new(4096 * size_of::<i32>()).unwrap();
    assert_eq!(b.remaining_capacity(), 4096 * size_of::<i32>());
    assert_eq!(b.capacity(), 4096 * size_of::<i32>());

    let mut v = Vec::with_capacity_in(1024, &b);

    for x in 0..1024 {
        v.push(x);
    }
    assert_eq!(b.remaining_capacity(), 3072 * size_of::<i32>());

    // Grow the vector (reallocation in this bump allocator)
    for x in 1024..2048 {
        v.push(x);
    }

    assert_eq!(b.remaining_capacity(), 1024 * size_of::<i32>());

    // Shrink the vector (does not add capacity)
    v.truncate(1024);
    v.shrink_to_fit();

    // Deallocate (noop)
    drop(v);
    assert_eq!(b.remaining_capacity(), 1024 * size_of::<i32>());

    b.reset();
    assert_eq!(b.remaining_capacity(), 4096 * size_of::<i32>());

    drop(b);
}

#[test]
fn allocate_failure() {
    let mut b = BumpCar::new(256).unwrap();

    let big_box = Box::new_in([0u8; 256], &b);
    let mut extra: Vec<u8, _> = Vec::new_in(&b);
    assert!(matches!(extra.try_reserve(128), Err(_)));

    drop(big_box);
    drop(extra);

    b.reset();

    let mut extra2: Vec<u8, _> = Vec::new_in(&b);
    assert!(matches!(extra2.try_reserve(128), Ok(())));
}
