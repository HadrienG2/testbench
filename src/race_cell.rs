//! This module contains shareable mutable containers designed for triggering
//! and detecting data races in thread synchronization testing code.
//!
//! # Motivation
//!
//! The main purpose of a thread synchronization protocol is to ensure that
//! some operations which are not atomic in hardware, such as writing to two
//! unrelated memory locations, appear to occur as atomic transactions from the
//! point of view of other threads: at any point of time, either a given
//! operation appears to be done, or it appears not to have started.
//!
//! Testing a thread synchronization primitive entails showing that inconsistent
//! states (where a transaction appears to be partially committed) have a
//! negligible probability of being exposed to the outside world in all expected
//! usage scenarios. It is done by showing that a given non-atomic operation is
//! properly encapsulated by the transactional semantics of the synchronization
//! protocol, and will not appear to be half-done to observers.
//!
//! Non-atomic operations are easier said than done, however, when the set of
//! operations which are atomic in hardware is larger than most people think
//! (at the time of writing, current Intel CPUs can access memory in blocks of
//! 128 bits, and current NVidia GPUs can do so in blocks of 1024 bits), not
//! well-defined by the architecture (and thus subjected to change in future
//! hardware), and dependent on the compiler's optimization choices in a
//! high-level programming language such as Rust.
//!
//! # Contents
//!
//! This module provides the RaceCell type, which can hold a value of a
//! user-chosen type T like a Cell<T> would, but is guaranteed not to be read
//! or written to in a single atomic operation even if the corresponding type T
//! can be atomically read or written to in hardware.
//!
//! Given the interface requirement that the equality operator of T works well
//! even when T is in an inconsistent state, the RaceCell type can detect the
//! situation where it is itself in an inconsistent state due to a
//! read-after-write data race, and report this situation to its reader.
//!
//! This can, in turn, be used to detect failures in thread synchronization
//! protocols, manifesting as inconsistent shared state being exposed to the
//! outside world.

#![deny(missing_docs)]

use std::boxed::Box;
use std::cell::UnsafeCell;
use std::fmt::Debug;


/// RaceCell is a container for triggering and detecting write-after-read
/// data races in a well-controlled fashion.
pub struct RaceCell<T: Debug + RaceProofEqAndClone> {
    /// Two copies of a value of type T are made. One is stored on the stack...
    local_contents: UnsafeCell<T>,

    /// ...and one is stored on the heap, which in all popular OSs is too far
    /// away from the stack to allow any significant probability of the hardware
    /// being able to write both copies in a single atomic transactions.
    remote_version: Box<UnsafeCell<T>>,
}
//
impl<T: Debug + RaceProofEqAndClone> RaceCell<T> {
    /// Create a new RaceCell with a certain initial content
    pub fn new(value: T) -> Self {
        Self {
            local_contents: UnsafeCell::new(value.clone()),
            remote_version: Box::new(UnsafeCell::new(value)),
        }
    }

    /// Update the internal contents of the RaceCell in a non-atomic fashion
    pub fn set(&self, value: T) {
        let local_ptr = self.local_contents.get();
        let remote_ptr = self.remote_version.get();
        unsafe {
            *local_ptr = value.clone();
            *remote_ptr = value;
        }
    }

    /// Read the current contents of the RaceCell, detecting any data race
    /// caused by a concurrently occurring write along the way.
    pub fn get(&self) -> Racey<T> {
        let local_ptr = self.local_contents.get();
        let remote_ptr = self.remote_version.get();
        unsafe {
            let local_data = (*local_ptr).clone();
            if local_data == (*remote_ptr) {
                return Racey::Consistent(local_data);
            } else {
                return Racey::Inconsistent;
            }
        }
    }
}
//
unsafe impl<T: Debug + RaceProofEqAndClone> Sync for RaceCell<T> {}


/// This is the result of a RaceCell read
#[derive(Debug, PartialEq)]
pub enum Racey<T: Debug> {
    /// The RaceCell was internally consistent, and its content was copied
    Consistent(T),

    /// The RaceCell was internally inconsistent, so a data race has occurred
    Inconsistent,
}


/// By implementing the RaceProofEqAndClone unsafe trait, a user testifies that
/// a type's equality operator and clone() method are both guaranteed to work
/// (in the sense of not panicking and outputting valid results) even if an
/// object is in an inconsistent state (a pessimistic definition being that the
/// object and any piece of data under its control to are full of random bits).
///
pub unsafe trait RaceProofEqAndClone: Clone + PartialEq {}
//
// All built-in numerical types fulfill this requirement:
//
unsafe impl RaceProofEqAndClone for f32 {}
unsafe impl RaceProofEqAndClone for f64 {}
unsafe impl RaceProofEqAndClone for i8 {}
unsafe impl RaceProofEqAndClone for i16 {}
unsafe impl RaceProofEqAndClone for i32 {}
unsafe impl RaceProofEqAndClone for i64 {}
unsafe impl RaceProofEqAndClone for isize {}
unsafe impl RaceProofEqAndClone for u8 {}
unsafe impl RaceProofEqAndClone for u16 {}
unsafe impl RaceProofEqAndClone for u32 {}
unsafe impl RaceProofEqAndClone for u64 {}
unsafe impl RaceProofEqAndClone for usize {}
//
// Here are some example of types which do not fulfill this requirement:
//
// * bool (Manipulating bools which are not true/false is undefined behaviour)
// * char (Same problem as bool: as the char type must contain a valid Unicode
//   scalar value, it has "forbidden values" that must never be manipulated)
// * Slices (the slice size and associated storage buffer may not be in sync)
// * Anything that is accessed through a pointer/ref (which may be invalid)


/// Here are some RaceCell tests
#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use super::{RaceCell, Racey};

    /// A RaceCell should be created in a consistent and correct state
    #[test]
    fn initial_state() {
        let cell = RaceCell::new(42);
        unsafe {
            assert_eq!(*cell.local_contents.get(), 42);
            assert_eq!(*cell.remote_version.get(), 42);
        }
    }

    /// Reading a consistent RaceCell should work as expected
    #[test]
    fn consistent_read() {
        let cell = RaceCell::new(4.2);
        assert_eq!(cell.get(), Racey::Consistent(4.2));
    }

    /// Reading an inconsistent RaceCell should work as expected
    #[test]
    fn inconsistent_read() {
        let cell = RaceCell::new(0xbad);
        unsafe {
            *cell.local_contents.get() = 0xdead;
        }
        assert_eq!(cell.get(), Racey::Inconsistent);
    }

    /// Unprotected concurrent reads and writes to a RaceCell should trigger
    /// detectable race conditions, illustrating its non-atomic nature.
    #[test]
    fn unprotected_race() {
        // Amount of writes to carry out
        const WRITES_COUNT: u64 = 1_000_000_000;

        // RaceCell in which the writes will be carried out
        let initial_value = 0u64;
        let cell1 = Arc::new(RaceCell::new(initial_value));
        let cell2 = cell1.clone();

        // Make sure that RaceCell does expose existing data races :)
        ::concurrent_test_2(
            move || {
                for i in 1..(WRITES_COUNT+1) {
                    cell1.set(i);
                }
            },
            move || {
                let mut last_value = 0u64;
                let data_race_count = Mutex::new(0u64);
                while last_value != WRITES_COUNT {
                    match cell2.get() {
                        Racey::Consistent(value) => last_value = value,
                        Racey::Inconsistent => {
                            // DEBUG: We must access this variable through a
                            //        mutex to please the LLVM God of Undefined
                            //        Behaviour, otherwise this match arm will
                            //        hang the thread in release builds...
                            *data_race_count.lock().unwrap() += 1;
                        },
                    }
                }
                let data_race_count = *data_race_count.lock().unwrap();
                print!("{} races detected, ", data_race_count);
                assert!(data_race_count > 0);
            }
        );
    }

    /// Appropriately protected concurrent reads and writes to a RaceCell should
    /// not yield any detectable race conditions.
    #[test]
    fn protected_transaction() {
        // Amount of writes to carry out
        const WRITES_COUNT: u64 = 10_000_000;

        // Mutex-protected RaceCell in which the writes will be carried out
        let initial_value = 0u64;
        let cell1 = Arc::new(Mutex::new(RaceCell::new(initial_value)));
        let cell2 = cell1.clone();

        // Make sure that RaceCell does not incorrectly detect race conditions
        ::concurrent_test_2(
            move || {
                for i in 1..(WRITES_COUNT+1) {
                    cell1.lock().unwrap().set(i);
                }
            },
            move || {
                let mut last_value = 0u64;
                let mut data_race_count = 0u64;
                while last_value != WRITES_COUNT {
                    match cell2.lock().unwrap().get() {
                        Racey::Consistent(value) => last_value = value,
                        Racey::Inconsistent => data_race_count += 1,
                    }
                }
                assert_eq!(data_race_count, 0);
            }
        );
    }
}
