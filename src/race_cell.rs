//! Shareable mutable containers easing detection of race conditions in thread
//! synchronization testing code.
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
//! certain type T like a `Cell<T>` would, but is guaranteed **not** to be read
//! or written to in a single atomic operation, even if the corresponding type T
//! can be atomically read or written to in hardware.
//!
//! Furthermore, a RaceCell can detect scenarios where it is read at the same
//! time as it is being written (which constitutes a read-after-write race
//! condition) and report them to the reader thread.
//!
//! Such "controlled data races" can, in turn, be used to detect failures in
//! thread synchronization protocols, manifesting as inconsistent shared states
//! being exposed to the outside world.
//!
//! # Requirements on T
//!
//! In principle, any Clone + Eq type T whose equality operator and clone()
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
//! For this reason, we currently only support some specific types T which have
//! atomic load and store operations, implemented as part of an atomic wrapper.
//! Note that although individual loads and stores to T are atomic, loads and
//! stores to RaceCell<T> are still guaranteed not to be atomic.

#![deny(missing_docs)]

use std::sync::atomic::{
    AtomicBool, AtomicI16, AtomicI32, AtomicI64, AtomicI8, AtomicIsize, AtomicPtr, AtomicU16,
    AtomicU32, AtomicU64, AtomicU8, AtomicUsize, Ordering,
};

/// Shareable mutable container for triggering and detecting write-after-read
/// data races in a well-controlled fashion.
#[derive(Debug, Default)]
pub struct RaceCell<T: AtomicData> {
    /// Two copies of a value of type T are made. One is stored on the stack...
    local_contents: T::AtomicWrapper,

    /// ...and one is stored on the heap, which in all popular OSs is too far
    /// away from the stack to allow any significant probability of the hardware
    /// writing both copies in a single atomic transactions.
    ///
    /// Of course, a malicious optimizer could still use hardware transactional
    /// memory or a software emulation thereof to achieve this effect, but there
    /// are no performance benefits in doing so, and in fact it will rather have
    /// an averse effect on performance, so a realistic optimizer won't do it.
    ///
    remote_version: Box<T::AtomicWrapper>,
}
//
impl<T: AtomicData> RaceCell<T> {
    /// Create a new RaceCell with a certain initial content
    pub fn new(value: T) -> Self {
        RaceCell {
            local_contents: T::AtomicWrapper::new(value.clone()),
            remote_version: Box::new(T::AtomicWrapper::new(value)),
        }
    }

    /// Update the internal contents of the RaceCell in a non-atomic fashion
    pub fn set(&self, value: T) {
        self.local_contents.relaxed_store(value.clone());
        self.remote_version.relaxed_store(value);
    }

    /// Read the current contents of the RaceCell, detecting any data race
    /// caused by a concurrently occurring write along the way.
    pub fn get(&self) -> Racey<T> {
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
impl<T: AtomicData> Clone for RaceCell<T> {
    /// Making RaceCells cloneable allows putting them in concurrent containers
    fn clone(&self) -> Self {
        let local_copy = self.local_contents.relaxed_load();
        let remote_copy = self.remote_version.relaxed_load();
        RaceCell {
            local_contents: T::AtomicWrapper::new(local_copy),
            remote_version: Box::new(T::AtomicWrapper::new(remote_copy)),
        }
    }
}

/// This is the result of a RaceCell read
#[derive(Debug, Eq, PartialEq)]
pub enum Racey<U: AtomicData> {
    /// The RaceCell was internally consistent, and its content was copied
    Consistent(U),

    /// The RaceCell was internally inconsistent: a data race has occurred
    Inconsistent,
}

/// Requirements on the data held by a RaceCell
pub trait AtomicData: Clone + Eq + Sized {
    /// Atomic wrapper type for this data implementing relaxed atomic load/store
    type AtomicWrapper: AtomicLoadStore<Content = Self>;
}
///
/// Atomic wrapper type for a certain kind of value
///
/// For the implementation of RaceCell not to be considered as undefined
/// behaviour and trashed by the Rust compiler in release mode, the underlying
/// type must be loaded and stored atomically. This requires synchronizing
/// accesses to it using things like the std::sync::atomic::AtomicXyz wrappers.
///
/// The only guarantee that we need is that loads and stores are atomic. We do
/// not need any other memory ordering guarantee. This is why we allow for
/// more wrapper type implementation freedom by not specifying memory orderings,
/// and internally relying only on relaxed ordering.
///
pub trait AtomicLoadStore: Sized {
    /// Type of data that is being wrapped
    type Content: AtomicData<AtomicWrapper = Self>;

    /// Create an atomic wrapper for a value of type U
    fn new(v: Self::Content) -> Self;

    /// Atomically load a value from the wrapper
    fn relaxed_load(&self) -> Self::Content;

    /// Atomically store a new value into the wrapper
    fn relaxed_store(&self, val: Self::Content);
}
///
/// This macro implements support for non-generic standard atomic types
///
macro_rules! impl_atomic_data {
    ($($data:ty => $wrapper:ty),*) => ($(
        impl AtomicData for $data {
            type AtomicWrapper = $wrapper;
        }

        impl AtomicLoadStore for $wrapper {
            type Content = $data;

            fn new(v: $data) -> $wrapper {
                <$wrapper>::new(v)
            }

            fn relaxed_load(&self) -> $data {
                <$wrapper>::load(self, Ordering::Relaxed)
            }

            fn relaxed_store(&self, val: $data) {
                <$wrapper>::store(self, val, Ordering::Relaxed)
            }
        }
    )*)
}
//
impl_atomic_data! {
    bool  => AtomicBool,
    i8    => AtomicI8,
    i16   => AtomicI16,
    i32   => AtomicI32,
    i64   => AtomicI64,
    isize => AtomicIsize,
    u8    => AtomicU8,
    u16   => AtomicU16,
    u32   => AtomicU32,
    u64   => AtomicU64,
    usize => AtomicUsize
}
//
// Atomic pointers are a bit special as they are generic, for now we will just
// treat them as a special case.
//
impl<V> AtomicData for *mut V {
    type AtomicWrapper = AtomicPtr<V>;
}
//
impl<V> AtomicLoadStore for AtomicPtr<V> {
    type Content = *mut V;

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

// FIXME: The astute reader will have noted that any data could be theoretically
//        put in a RaceCell by using a Mutex as the AtomicWrapper. However, this
//        will only be implemented once Rust has specialization, to avoid
//        pessimizing the common case where a primitive type is enough.

/// Here are some RaceCell tests
#[cfg(test)]
mod tests {
    use super::{AtomicLoadStore, RaceCell, Racey};
    use std::sync::Mutex;

    /// A RaceCell should be created in a consistent and correct state
    #[test]
    fn initial_state() {
        let cell = RaceCell::new(true);
        assert!(cell.local_contents.relaxed_load());
        assert!(cell.remote_version.relaxed_load());
    }

    /// Reading a consistent RaceCell should work as expected
    #[test]
    fn consistent_read() {
        let cell = RaceCell::new(-42_isize);
        assert_eq!(cell.get(), Racey::Consistent(-42));
    }

    /// Reading an inconsistent RaceCell should work as expected
    #[test]
    fn inconsistent_read() {
        let cell = RaceCell::new(0xbad_usize);
        cell.local_contents.relaxed_store(0xdead);
        assert_eq!(cell.get(), Racey::Inconsistent);
    }

    /// RaceCells should be cloned as-is, even if in an inconsistent state
    #[test]
    fn clone() {
        let cell = RaceCell::new(0xbeef_usize);
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
        let cell = RaceCell::new(0);

        // Make sure that RaceCell does expose existing data races, with a
        // detection probability better than 1% for very obvious ones :)
        crate::concurrent_test_2(
            || {
                for i in 1..=WRITES_COUNT {
                    cell.set(i);
                }
            },
            || {
                let mut last_value = 0;
                let mut data_race_count = 0usize;
                while last_value != WRITES_COUNT {
                    match cell.get() {
                        Racey::Consistent(value) => last_value = value,
                        Racey::Inconsistent => data_race_count += 1,
                    }
                }
                println!("{data_race_count} races detected");
                assert!(data_race_count > WRITES_COUNT / 100);
            },
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
        let cell = Mutex::new(RaceCell::new(0));

        // Make sure that RaceCell does not incorrectly detect race conditions
        crate::concurrent_test_2(
            || {
                for i in 1..=WRITES_COUNT {
                    cell.lock().unwrap().set(i);
                }
            },
            || {
                let mut last_value = 0;
                let mut data_race_count = 0usize;
                while last_value != WRITES_COUNT {
                    match cell.lock().unwrap().get() {
                        Racey::Consistent(value) => last_value = value,
                        Racey::Inconsistent => data_race_count += 1,
                    }
                }
                assert_eq!(data_race_count, 0);
            },
        );
    }
}
