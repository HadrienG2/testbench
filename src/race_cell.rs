//! This module contains shareable mutable containers designed for enabling
//! and detecting race conditions in thread synchronization testing code.
//!
//! # Motivation
//!
//! The main purpose of a thread synchronization protocol is to ensure that
//! some operations which are not atomic in hardware, such as writing to two
//! unrelated memory locations, appear to occur as atomic transactions from the
//! point of view of other threads: at any point of time, either a given
//! operation appears to be done, or it appears not to have started.
//!
//! Testing a thread synchronization primitive involves showing that
//! inconsistent states (where an operation appears to be half-performed) have a
//! negligible probability of being exposed to the outside world in all expected
//! usage scenarios. It is typically done by showing that a given non-atomic
//! operation is properly encapsulated by the transactional semantics of the
//! synchronization protocol, and will never appear as half-done to observers.
//!
//! Non-atomic operations are easier said than done, however, when the set of
//! operations which can be atomic in hardware is larger than most people think
//! (at the time of writing, current Intel CPUs can access memory in blocks of
//! 128 bits, and current NVidia GPUs can do so in blocks of 1024 bits), not
//! well-defined by the architecture (and thus subjected to increase in future
//! hardware), and highly dependent on the compiler's optimization choices in a
//! high-level programming language such as Rust.
//!
//! Which is why I think we need some types whose operations are guaranteed to
//! be non-atomic under a set of reasonable assumptions, and which can easily be
//! observed to be in an inconsistent state.
//!
//! # Functionality
//!
//! This module provides the RaceCell type, which can hold a value of a
//! certain type U like a `Cell<U>` would, but is guaranteed **not** to be read
//! or written to in a single atomic operation, even if the corresponding type U
//! can be atomically read or written to in hardware.
//!
//! In addition, a RaceCell can detect scenarios where it is read at the same
//! time as it is being written (which constitutes a read-after-write race
//! condition) and report them to the reader thread.
//!
//! Such "controlled data races" can, in turn, be used to detect failures in
//! thread synchronization protocols, manifesting as inconsistent shared states
//! being exposed to the outside world.
//!
//! # Requirements on U
//!
//! In principle, any Clone + Eq type U whose equality operator and clone()
//! implementation behave well even if the inner data is in a inconsistent state
//! (e.g. filled with random bits) could be used. This is true of all primitive
//! integral types and aggregates thereof, for example.
//!
//! However, in practice, unsynchronized concurrent read/write access to
//! arbitrary data from multiple threads constitutes a data race, which is
//! undefined behaviour in Rust even when it occurs inside an UnsafeCell. This
//! is problematic because rustc's optimizer allows itself to transform code
//! which relies on undefined behaviour however it likes, leading to breakages
//! in release builds such as infinite loops appearing out of nowhere.
//!
//! For this reason, we currently only support some specific types U which have
//! atomic load and store operations, implemented as part of an atomic wrapper
//! T. Note that although individual loads and stores to U are atomic, loads and
//! stores to RaceCell<T> are still guaranteed not to be atomic.

#![deny(missing_docs)]

use std::{
    fmt::Debug,
    marker::PhantomData,
    sync::atomic::{AtomicBool, AtomicIsize, AtomicPtr, AtomicUsize, Ordering},
};


/// Shareable mutable container for triggering and detecting write-after-read
/// data races in a well-controlled fashion. Operates on a type U through an
/// atomic wrapper type T.
#[derive(Debug)]
pub struct RaceCell<T, U> where T: AtomicLoadStore<U>,
                                U: Clone + Debug + Eq {
    /// Two copies of a value of type U are made. One is stored on the stack...
    local_contents: T,

    /// ...and one is stored on the heap, which in all popular OSs is too far
    /// away from the stack to allow any significant probability of the hardware
    /// writing both copies in a single atomic transactions.
    ///
    /// Of course, a malicious optimizer could still use hardware transactional
    /// memory or a software emulation thereof to achieve this effect, but there
    /// are no performance benefits in doing so, and in fact it will rather have
    /// an averse effect on performance, so a realistic optimizer won't do it.
    ///
    remote_version: Box<T>,

    /// Rust really dislikes unused generic struct parameters, even if they are
    /// used in the implementation, so a PhantomData marker must be added for U
    unused: PhantomData<U>,
}
//
impl<T, U> RaceCell<T, U> where T: AtomicLoadStore<U>,
                                U: Clone + Debug + Eq {
    /// Create a new RaceCell with a certain initial content
    pub fn new(value: U) -> Self {
        RaceCell {
            local_contents: T::new(value.clone()),
            remote_version: Box::new(T::new(value)),
            unused: PhantomData,
        }
    }

    /// Update the internal contents of the RaceCell in a non-atomic fashion
    pub fn set(&self, value: U) {
        self.local_contents.relaxed_store(value.clone());
        self.remote_version.relaxed_store(value);
    }

    /// Read the current contents of the RaceCell, detecting any data race
    /// caused by a concurrently occurring write along the way.
    pub fn get(&self) -> Racey<U> {
        let local_data = self.local_contents.relaxed_load();
        let remote_data = self.remote_version.relaxed_load();
        if local_data == remote_data {
            Racey::Consistent(local_data)
        } else {
            Racey::Inconsistent
        }
    }
}
//
impl<T, U> Clone for RaceCell<T, U> where T: AtomicLoadStore<U>,
                                          U: Clone + Debug + Eq {
    /// Making RaceCells cloneable allows putting them in concurrent containers
    fn clone(&self) -> Self {
        let local_copy = self.local_contents.relaxed_load();
        let remote_copy = self.remote_version.relaxed_load();
        RaceCell {
            local_contents: T::new(local_copy),
            remote_version: Box::new(T::new(remote_copy)),
            unused: PhantomData,
        }
    }
}
//
impl<T, U> Default for RaceCell<T, U> where T: AtomicLoadStore<U>,
                                            U: Clone + Default + Debug + Eq {
    /// A RaceCell has a default value if the inner type has
    fn default() -> Self {
        Self::new(U::default())
    }
}


/// This is the result of a RaceCell read
#[derive(Debug, Eq, PartialEq)]
pub enum Racey<U: Debug + Eq> {
    /// The RaceCell was internally consistent, and its content was copied
    Consistent(U),

    /// The RaceCell was internally inconsistent: a data race has occurred
    Inconsistent,
}


/// Atomic wrapper type for a value of type U
///
/// For the implementation of RaceCell not to be considered as undefined
/// behaviour and trashed by the Rust compiler in release mode, the underlying
/// type U must be loaded and stored atomically. This requires synchronizing
/// accesses to it using things like the std::sync::atomic::AtomicXyz wrappers.
///
/// The only guarantee that we need is that loads and stores are atomic. We do
/// not need any other memory ordering guarantee. This is why we allow for
/// more wrapper type implementation freedom by not specifying memory orderings,
/// and internally relying only on relaxed ordering.
///
pub trait AtomicLoadStore<U> {
    /// Create an atomic wrapper for a value of type U
    fn new(v: U) -> Self;

    /// Atomically load a value from the wrapper
    fn relaxed_load(&self) -> U;

    /// Atomically store a new value into the wrapper
    fn relaxed_store(&self, val: U);
}
///
/// This macro implements support for non-generic standard atomic types
///
macro_rules! impl_load_store {
    ($($U:ty, $T:ty);*) => ($(
        impl AtomicLoadStore<$U> for $T {
            fn new(v: $U) -> $T {
                <$T>::new(v)
            }

            fn relaxed_load(&self) -> $U {
                <$T>::load(self, Ordering::Relaxed)
            }

            fn relaxed_store(&self, val: $U) {
                <$T>::store(self, val, Ordering::Relaxed)
            }
        }
    )*)
}
///
impl_load_store!{ bool,  AtomicBool;
                  isize, AtomicIsize;
                  usize, AtomicUsize }
///
/// Atomic pointers are a bit special as they are generic, for now we will just
/// treat them as a special case.
///
impl<V> AtomicLoadStore<*mut V> for AtomicPtr<V> {
    fn new(v: *mut V) -> AtomicPtr<V> {
        <AtomicPtr<V>>::new(v)
    }

    fn relaxed_load(&self) -> *mut V {
        <AtomicPtr<V>>::load(self, Ordering::Relaxed)
    }

    fn relaxed_store(&self, val: *mut V) {
        <AtomicPtr<V>>::store(self, val, Ordering::Relaxed)
    }
}


// Here are implementations of RaceCell for all stable hardware atomic types

/// Implementation of RaceCell for bool
pub type BoolRaceCell = RaceCell<AtomicBool, bool>;
/// Implementation of RaceCell for isize
pub type IsizeRaceCell = RaceCell<AtomicIsize, isize>;
/// Implementation of RaceCell for pointers
pub type PtrRaceCell<V> = RaceCell<AtomicPtr<V>, *mut V>;
/// Implementation of RaceCell for usize
pub type UsizeRaceCell = RaceCell<AtomicUsize, usize>;


/// Here are some RaceCell tests
#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use super::{
        AtomicLoadStore,
        BoolRaceCell,
        IsizeRaceCell,
        Racey,
        UsizeRaceCell
    };

    /// A RaceCell should be created in a consistent and correct state
    #[test]
    fn initial_state() {
        let cell = BoolRaceCell::new(true);
        assert_eq!(cell.local_contents.relaxed_load(), true);
        assert_eq!(cell.remote_version.relaxed_load(), true);
    }

    /// Reading a consistent RaceCell should work as expected
    #[test]
    fn consistent_read() {
        let cell = IsizeRaceCell::new(-42);
        assert_eq!(cell.get(), Racey::Consistent(-42));
    }

    /// Reading an inconsistent RaceCell should work as expected
    #[test]
    fn inconsistent_read() {
        let cell = UsizeRaceCell::new(0xbad);
        cell.local_contents.relaxed_store(0xdead);
        assert_eq!(cell.get(), Racey::Inconsistent);
    }

    /// RaceCells should be cloned as-is, even if in an inconsistent state
    #[test]
    fn clone() {
        let cell = UsizeRaceCell::new(0xbeef);
        cell.local_contents.relaxed_store(0xdeaf);
        let clone = cell.clone();
        assert_eq!(clone.local_contents.relaxed_load(), 0xdeaf);
        assert_eq!(clone.remote_version.relaxed_load(), 0xbeef);
    }

    /// Unprotected concurrent reads and writes to a RaceCell should trigger
    /// detectable race conditions, illustrating its non-atomic nature.
    ///
    /// To maximize the odds of race conditions, this kind of test should be run
    /// in single-threaded mode.
    ///
    #[test]
    #[ignore]
    fn unprotected_race() {
        // Amount of writes to carry out
        const WRITES_COUNT: usize = 100_000_000;

        // RaceCell in which the writes will be carried out
        let initial_value = 0;
        let cell1 = Arc::new(UsizeRaceCell::new(initial_value));
        let cell2 = cell1.clone();

        // Make sure that RaceCell does expose existing data races, with a
        // detection probability better than 1% for very obvious ones :)
        ::concurrent_test_2(
            move || {
                for i in 1..(WRITES_COUNT+1) {
                    cell1.set(i);
                }
            },
            move || {
                let mut last_value = 0;
                let mut data_race_count = 0usize;
                while last_value != WRITES_COUNT {
                    match cell2.get() {
                        Racey::Consistent(value) => last_value = value,
                        Racey::Inconsistent => data_race_count += 1,
                    }
                }
                print!("{} races detected: ", data_race_count);
                assert!(data_race_count > WRITES_COUNT/100);
            }
        );
    }

    /// Appropriately protected concurrent reads and writes to a RaceCell should
    /// not yield any detectable race conditions.
    ///
    /// To maximize the odds of race conditions, this kind of test should be run
    /// in single-threaded mode.
    ///
    #[test]
    #[ignore]
    fn protected_transaction() {
        // Amount of writes to carry out
        const WRITES_COUNT: usize = 10_000_000;

        // Mutex-protected RaceCell in which the writes will be carried out
        let initial_value = 0;
        let cell1 = Arc::new(Mutex::new(UsizeRaceCell::new(initial_value)));
        let cell2 = cell1.clone();

        // Make sure that RaceCell does not incorrectly detect race conditions
        ::concurrent_test_2(
            move || {
                for i in 1..(WRITES_COUNT+1) {
                    cell1.lock().unwrap().set(i);
                }
            },
            move || {
                let mut last_value = 0;
                let mut data_race_count = 0usize;
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
