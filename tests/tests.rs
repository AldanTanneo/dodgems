#![feature(allocator_api)]

use std::{
    alloc::{Allocator, Layout},
    mem::size_of,
};

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

#[test]
fn allocate_zero_size() {
    let b = BumpCar::new(256).unwrap();

    let _zero_size_alloc = (&b)
        .allocate(Layout::from_size_align(0, 1).unwrap())
        .unwrap();

    assert_eq!(b.remaining_capacity(), 256);

    let _fill_allocator = Vec::<u8, _>::with_capacity_in(256, &b);

    assert_eq!(b.remaining_capacity(), 0);

    let _zero_size_alloc = (&b)
        .allocate(Layout::from_size_align(0, 1).unwrap())
        .unwrap();

    assert_eq!(b.remaining_capacity(), 0);
}

#[test]
fn allocate_vary_alignment() {
    let b = BumpCar::new(24).unwrap();

    let _byte = Box::new_in(1i8, &b);
    assert_eq!(b.remaining_capacity(), 23);

    let _short = Box::new_in(2i16, &b);
    assert_eq!(b.remaining_capacity(), 20);

    // increase alignment, but add an extra byte to offset the current alignment
    let _byte = Box::new_in(1i8, &b);
    let _int = Box::new_in(4i32, &b);
    assert_eq!(b.remaining_capacity(), 12);

    let _byte = Box::new_in(1i8, &b);
    let _long = Box::new_in(8i64, &b);
    assert_eq!(b.remaining_capacity(), 0);
}

#[test]
fn allocate_checkpoint() {
    let b = BumpCar::new(256).unwrap();

    let alloc_half = Vec::<u8, _>::with_capacity_in(128, &b);
    drop(alloc_half);

    assert_eq!(b.remaining_capacity(), 128);

    let mut checkpoint = b.checkpoint();

    assert_eq!(b.remaining_capacity(), 0);
    assert_eq!(checkpoint.capacity(), 128);
    assert_eq!(checkpoint.remaining_capacity(), 128);

    let alloc_rest = Vec::<u8, _>::with_capacity_in(128, &checkpoint);
    drop(alloc_rest);

    assert_eq!(checkpoint.remaining_capacity(), 0);

    let checkpoint2 = checkpoint.checkpoint();
    assert_eq!(checkpoint2.capacity(), 0);
    drop(checkpoint2);

    checkpoint.reset();
    assert_eq!(checkpoint.remaining_capacity(), 128);

    drop(checkpoint);
    drop(b);
}
