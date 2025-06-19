use crate::{allocator::EventAllocator, pool::PoolId};
use std::{fmt, marker::PhantomData, ops::Deref};
use std::fmt::Debug;

pub struct RingPtr<T> {
    pub pool_id: PoolId,
    pub slot_index: u32,
    pub generation: u32,  // Not atomic in RingPtr - just a snapshot
    allocator: &'static EventAllocator,
    // this phantom data often confuses me, so I am putting a comment here. Go ahead Sue me.
    _phantom: PhantomData<T>, // "Hey Rust, I care about type T, trust me!"
}

// RingPtr can be moved between threads
unsafe impl<T: Send> Send for RingPtr<T> {}

// RingPtr can be shared between threads (for references)
unsafe impl<T: Sync> Sync for RingPtr<T> {}

impl<T> RingPtr<T> {
    pub fn new(pool_id: PoolId, slot_index: u32, generation: u32, allocator: &'static EventAllocator) -> Self {
        Self {
            pool_id,
            slot_index,
            generation,
            allocator,
            _phantom: PhantomData,
        }
    }
}

impl<T> fmt::Debug for RingPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RingPtr")
            .field("pool_id", &self.pool_id)
            .field("slot_index", &self.slot_index)
            .field("generation", &self.generation)
            .finish()
    }
}

impl<T> Deref for RingPtr<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            &*(self.allocator.pools.get_slot_data(self.pool_id, self.slot_index) as *const T)
        }
    }
}

impl<T> Clone for RingPtr<T> {
    fn clone(&self) -> Self {
        // Increment the atomic ref count in the actual slot
        self.allocator.pools.inc_ref_count(self.pool_id, self.slot_index);

        Self {
            pool_id: self.pool_id,           // PoolId - Copy
            slot_index: self.slot_index,     // u32 - Copy
            generation: self.generation,     // u32 - Copy
            allocator: self.allocator,       // &'static - Copy
            _phantom: PhantomData,           // Zero-sized
        }
    }
}

impl<T> Drop for RingPtr<T> {
    fn drop(&mut self) {
        // Decrement the atomic ref count in the actual slot
        let remaining = self.allocator.pools.dec_ref_count(self.pool_id, self.slot_index);

        if remaining == 1 {  // We were the last reference (fetch_sub returns old value)
            // Last reference dropped - mark slot as reusable
            self.allocator.pools.mark_slot_reusable(self.pool_id, self.slot_index);
        }
    }
}