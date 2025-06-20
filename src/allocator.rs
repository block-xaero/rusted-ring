use std::sync::Arc;
use bytemuck::Zeroable;

use crate::{
    pool::{EventPools, PoolId},
    ring::{EventSize, PooledEvent, Reader, RingBuffer, Writer},
    ring_ptr::RingPtr,
};

pub struct EventAllocator {
    pub pools: Arc<EventPools>,
}

impl EventAllocator {
        pub fn new() -> Self {
            Self {
                pools: Arc::new(EventPools {
                    xs_pool: Arc::new(RingBuffer::new()),
                    s_pool: Arc::new(RingBuffer::new()),
                    m_pool: Arc::new(RingBuffer::new()),
                    l_pool: Arc::new(RingBuffer::new()),
                    xl_pool: Arc::new(RingBuffer::new()),
                }),
            }
        }

    // Get writers for each size (for LMAX-style sequential writing if needed)
    pub fn get_xs_writer(&self) -> Writer<64, 2000> {
        Writer {
            ringbuffer: self.pools.xs_pool.clone(),
            last_ts: 0,
        }
    }

    pub fn get_s_writer(&self) -> Writer<256, 1000> {
        Writer {
            ringbuffer: self.pools.s_pool.clone(),
            last_ts: 0,
        }
    }

    pub fn get_m_writer(&self) -> Writer<1024, 300> {
        Writer {
            ringbuffer: self.pools.m_pool.clone(),
            last_ts: 0,
        }
    }

    pub fn get_l_writer(&self) -> Writer<4096, 60> {
        Writer {
            ringbuffer: self.pools.l_pool.clone(),
            last_ts: 0,
        }
    }

    pub fn get_xl_writer(&self) -> Writer<16384, 15> {
        Writer {
            ringbuffer: self.pools.xl_pool.clone(),
            last_ts: 0,
        }
    }

    // Get readers for each size
    pub fn get_xs_reader(&self) -> Reader<64, 2000> {
        Reader {
            ringbuffer: self.pools.xs_pool.clone(),
            cursor: 0,
            last_ts: 0,
        }
    }

    pub fn get_s_reader(&self) -> Reader<256, 1000> {
        Reader {
            ringbuffer: self.pools.s_pool.clone(),
            cursor: 0,
            last_ts: 0,
        }
    }

    pub fn get_m_reader(&self) -> Reader<1024, 300> {
        Reader {
            ringbuffer: self.pools.m_pool.clone(),
            cursor: 0,
            last_ts: 0,
        }
    }

    pub fn get_l_reader(&self) -> Reader<4096, 60> {
        Reader {
            ringbuffer: self.pools.l_pool.clone(),
            cursor: 0,
            last_ts: 0,
        }
    }

    pub fn get_xl_reader(&self) -> Reader<16384, 15> {
        Reader {
            ringbuffer: self.pools.xl_pool.clone(),
            cursor: 0,
            last_ts: 0,
        }
    }

    // Size-based allocation (the smart interface)
    pub fn estimate_size(data_len: usize) -> EventSize {
        match data_len {
            0..=64 => EventSize::XS,
            65..=256 => EventSize::S,
            257..=1024 => EventSize::M,
            1025..=4096 => EventSize::L,
            4097..=16384 => EventSize::XL,
            _ => EventSize::XXL,
        }
    }

    // Create pooled event from raw data
    pub fn create_pooled_event<const SIZE: usize>(
        data: &[u8],
        event_type: u32,
    ) -> Result<PooledEvent<SIZE>, &'static str> {
        if data.len() > SIZE {
            return Err("Data too large for this pool size");
        }
        let mut pooled = PooledEvent::<SIZE>::zeroed();
        pooled.data[..data.len()].copy_from_slice(data);
        pooled.len = data.len() as u32;
        pooled.event_type = event_type;
        Ok(pooled)
    }

    // NEW: Slot allocation methods for XaeroFlux pattern

    /// Find an available slot in the specified pool
    pub fn find_available_slot(&self, pool_id: PoolId) -> Option<u32> {
        let capacity = match pool_id {
            PoolId::XS => 2000,
            PoolId::S => 1000,
            PoolId::M => 300,
            PoolId::L => 60,
            PoolId::XL => 15,
        };

        // Simple linear scan for available slot
        // TODO: Could optimize with free list for better O(1) allocation
        for slot_index in 0..capacity {
            if self.pools.try_allocate_slot(pool_id, slot_index as u32) {
                return Some(slot_index as u32);
            }
        }

        None // Pool is full
    }

    /// Allocate event to ring buffer slot and return RingPtr - separate methods for each size
    pub fn allocate_xs_event(
        &self,
        data: &[u8],
        event_type: u32,
    ) -> Result<RingPtr<PooledEvent<64>>, AllocationError> {
        // 1. Create the pooled event
        let pooled_event = Self::create_pooled_event::<64>(data, event_type)
            .map_err(|e| AllocationError::EventCreation(e))?;

        // 2. Find available slot
        let slot_index = self.find_available_slot(PoolId::XS)
            .ok_or(AllocationError::PoolFull(PoolId::XS))?;

        // 3. Place event in slot
        self.pools.place_xs_event(pooled_event, slot_index);

        // 4. Get generation for ABA protection
        let generation = self.pools.get_generation(PoolId::XS, slot_index);

        // 5. Create RingPtr pointing to the slot
        Ok(RingPtr::new(PoolId::XS, slot_index, generation, unsafe {
            // SAFETY: We know this EventAllocator lives for the program duration
            // because it's stored in a global OnceLock in the pool manager
            std::mem::transmute::<&EventAllocator, &'static EventAllocator>(self)
        }))
    }

    pub fn allocate_s_event(
        &self,
        data: &[u8],
        event_type: u32,
    ) -> Result<RingPtr<PooledEvent<256>>, AllocationError> {
        let pooled_event = Self::create_pooled_event::<256>(data, event_type)
            .map_err(|e| AllocationError::EventCreation(e))?;

        let slot_index = self.find_available_slot(PoolId::S)
            .ok_or(AllocationError::PoolFull(PoolId::S))?;

        self.pools.place_s_event(pooled_event, slot_index);
        let generation = self.pools.get_generation(PoolId::S, slot_index);

        Ok(RingPtr::new(PoolId::S, slot_index, generation, unsafe {
            std::mem::transmute::<&EventAllocator, &'static EventAllocator>(self)
        }))
    }

    pub fn allocate_m_event(
        &self,
        data: &[u8],
        event_type: u32,
    ) -> Result<RingPtr<PooledEvent<1024>>, AllocationError> {
        let pooled_event = Self::create_pooled_event::<1024>(data, event_type)
            .map_err(|e| AllocationError::EventCreation(e))?;

        let slot_index = self.find_available_slot(PoolId::M)
            .ok_or(AllocationError::PoolFull(PoolId::M))?;

        self.pools.place_m_event(pooled_event, slot_index);
        let generation = self.pools.get_generation(PoolId::M, slot_index);

        Ok(RingPtr::new(PoolId::M, slot_index, generation, unsafe {
            std::mem::transmute::<&EventAllocator, &'static EventAllocator>(self)
        }))
    }

    pub fn allocate_l_event(
        &self,
        data: &[u8],
        event_type: u32,
    ) -> Result<RingPtr<PooledEvent<4096>>, AllocationError> {
        let pooled_event = Self::create_pooled_event::<4096>(data, event_type)
            .map_err(|e| AllocationError::EventCreation(e))?;

        let slot_index = self.find_available_slot(PoolId::L)
            .ok_or(AllocationError::PoolFull(PoolId::L))?;

        self.pools.place_l_event(pooled_event, slot_index);
        let generation = self.pools.get_generation(PoolId::L, slot_index);

        Ok(RingPtr::new(PoolId::L, slot_index, generation, unsafe {
            std::mem::transmute::<&EventAllocator, &'static EventAllocator>(self)
        }))
    }

    pub fn allocate_xl_event(
        &self,
        data: &[u8],
        event_type: u32,
    ) -> Result<RingPtr<PooledEvent<16384>>, AllocationError> {
        let pooled_event = Self::create_pooled_event::<16384>(data, event_type)
            .map_err(|e| AllocationError::EventCreation(e))?;

        let slot_index = self.find_available_slot(PoolId::XL)
            .ok_or(AllocationError::PoolFull(PoolId::XL))?;

        self.pools.place_xl_event(pooled_event, slot_index);
        let generation = self.pools.get_generation(PoolId::XL, slot_index);

        Ok(RingPtr::new(PoolId::XL, slot_index, generation, unsafe {
            std::mem::transmute::<&EventAllocator, &'static EventAllocator>(self)
        }))
    }

    /// Convenience method that auto-detects size and allocates
    pub fn allocate_event(
        &self,
        data: &[u8],
        event_type: u32,
    ) -> Result<AllocatedEvent, AllocationError> {
        let size = Self::estimate_size(data.len());

        match size {
            EventSize::XS => {
                let ring_ptr = self.allocate_xs_event(data, event_type)?;
                Ok(AllocatedEvent::Xs(ring_ptr))
            }
            EventSize::S => {
                let ring_ptr = self.allocate_s_event(data, event_type)?;
                Ok(AllocatedEvent::S(ring_ptr))
            }
            EventSize::M => {
                let ring_ptr = self.allocate_m_event(data, event_type)?;
                Ok(AllocatedEvent::M(ring_ptr))
            }
            EventSize::L => {
                let ring_ptr = self.allocate_l_event(data, event_type)?;
                Ok(AllocatedEvent::L(ring_ptr))
            }
            EventSize::XL => {
                let ring_ptr = self.allocate_xl_event(data, event_type)?;
                Ok(AllocatedEvent::Xl(ring_ptr))
            }
            EventSize::XXL => Err(AllocationError::TooLarge {
                data_len: data.len(),
                max_size: 16384,
            }),
        }
    }
}

/// Wrapper for allocated events of different sizes
#[derive(Debug, Clone)]
pub enum AllocatedEvent {
    Xs(RingPtr<PooledEvent<64>>),
    S(RingPtr<PooledEvent<256>>),
    M(RingPtr<PooledEvent<1024>>),
    L(RingPtr<PooledEvent<4096>>),
    Xl(RingPtr<PooledEvent<16384>>),
}

impl AllocatedEvent {
    /// Get the underlying data regardless of pool size
    pub fn data(&self) -> &[u8] {
        match self {
            AllocatedEvent::Xs(ring_ptr) => &ring_ptr.data[..ring_ptr.len as usize],
            AllocatedEvent::S(ring_ptr) => &ring_ptr.data[..ring_ptr.len as usize],
            AllocatedEvent::M(ring_ptr) => &ring_ptr.data[..ring_ptr.len as usize],
            AllocatedEvent::L(ring_ptr) => &ring_ptr.data[..ring_ptr.len as usize],
            AllocatedEvent::Xl(ring_ptr) => &ring_ptr.data[..ring_ptr.len as usize],
        }
    }

    pub fn event_type(&self) -> u32 {
        match self {
            AllocatedEvent::Xs(ring_ptr) => ring_ptr.event_type,
            AllocatedEvent::S(ring_ptr) => ring_ptr.event_type,
            AllocatedEvent::M(ring_ptr) => ring_ptr.event_type,
            AllocatedEvent::L(ring_ptr) => ring_ptr.event_type,
            AllocatedEvent::Xl(ring_ptr) => ring_ptr.event_type,
        }
    }

    pub fn len(&self) -> u32 {
        match self {
            AllocatedEvent::Xs(ring_ptr) => ring_ptr.len,
            AllocatedEvent::S(ring_ptr) => ring_ptr.len,
            AllocatedEvent::M(ring_ptr) => ring_ptr.len,
            AllocatedEvent::L(ring_ptr) => ring_ptr.len,
            AllocatedEvent::Xl(ring_ptr) => ring_ptr.len,
        }
    }

    pub fn pool_id(&self) -> PoolId {
        match self {
            AllocatedEvent::Xs(_) => PoolId::XS,
            AllocatedEvent::S(_) => PoolId::S,
            AllocatedEvent::M(_) => PoolId::M,
            AllocatedEvent::L(_) => PoolId::L,
            AllocatedEvent::Xl(_) => PoolId::XL,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AllocationError {
    #[error("Event creation failed: {0}")]
    EventCreation(&'static str),

    #[error("Pool {0:?} is full")]
    PoolFull(PoolId),

    #[error("Data too large: {data_len} bytes > max {max_size} bytes")]
    TooLarge { data_len: usize, max_size: usize },
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::sync::{Arc, OnceLock};


    //#[test]
    // fn test_smaller_stack_allocations() {
    //     println!("Testing smaller stack allocations...");
    //
    //     // Try smaller sizes to find the limit
    //     let small_array = [0u8; 100_000]; // 100KB
    //     println!("100KB allocation: OK, first byte: {}", small_array[0]);
    //
    //     let medium_array = [0u8; 500_000]; // 500KB
    //     println!("500KB allocation: OK, first byte: {}", medium_array[0]);
    //
    //     // This might fail
    //     let large_array = [0u8; 800_000]; // 800KB
    //     println!("800KB allocation: OK, first byte: {}", large_array[0]);
    // }
    //
    // #[test]
    // fn test_stack_space() {
    //     // Let's see how much stack space we actually have
    //     println!("Testing stack space...");
    //
    //     // Try to allocate a large array on stack to see the limit
    //     let large_array = [0u8; 1_000_000]; // 1MB
    //     println!("Successfully allocated 1MB array on stack");
    //     println!("Array first byte: {}", large_array[0]);
    //
    //     // Try to allocate 2MB
    //     let larger_array = [0u8; 2_000_000]; // 2MB
    //     println!("Successfully allocated 2MB array on stack");
    //     println!("Array first byte: {}", larger_array[0]);
    // }

    // Shared allocator for all tests to avoid stack overflow
    static TEST_ALLOCATOR: OnceLock<Arc<EventAllocator>> = OnceLock::new();

    fn get_test_allocator() -> Arc<EventAllocator> {
        TEST_ALLOCATOR.get_or_init(|| Arc::new(EventAllocator::new())).clone()
    }

    #[test]
    fn test_simple_creation() {
        println!("Creating EventAllocator...");
        let allocator = get_test_allocator();
        println!("EventAllocator created successfully!");

        // Just test that we can access the pools
        println!("Testing pool access...");
        let stats = allocator.pools.get_pool_stats(PoolId::XS);
        println!("XS Pool stats: {}", stats);
        assert_eq!(stats.total_slots, 2000);
    }

    #[test]
    fn test_basic_allocation() {
        let allocator = get_test_allocator();

        // Test XS allocation
        let xs_event = allocator.allocate_xs_event(b"small", 42).unwrap();
        assert_eq!(&xs_event.data[..5], b"small");
        assert_eq!(xs_event.len, 5);
        assert_eq!(xs_event.event_type, 42);

        // Test S allocation
        let medium_data = vec![1u8; 128];
        let s_event = allocator.allocate_s_event(&medium_data, 100).unwrap();
        assert_eq!(s_event.len, 128);
        assert_eq!(s_event.event_type, 100);
        assert_eq!(&s_event.data[..128], &medium_data[..]);
    }

    #[test]
    fn test_auto_size_detection() {
        let allocator = EventAllocator::new();

        // Small data -> XS pool
        let small_event = allocator.allocate_event(b"tiny", 1).unwrap();
        assert!(matches!(small_event, AllocatedEvent::Xs(_)));

        // Medium data -> S pool
        let medium_data = vec![0u8; 128];
        let medium_event = allocator.allocate_event(&medium_data, 2).unwrap();
        assert!(matches!(medium_event, AllocatedEvent::S(_)));

        // Large data -> M pool
        let large_data = vec![0u8; 512];
        let large_event = allocator.allocate_event(&large_data, 3).unwrap();
        assert!(matches!(large_event, AllocatedEvent::M(_)));

        // XL data -> L pool
        let xl_data = vec![0u8; 2048];
        let xl_event = allocator.allocate_event(&xl_data, 4).unwrap();
        assert!(matches!(xl_event, AllocatedEvent::L(_)));

        // XXL data -> XL pool
        let xxl_data = vec![0u8; 8192];
        let xxl_event = allocator.allocate_event(&xxl_data, 5).unwrap();
        assert!(matches!(xxl_event, AllocatedEvent::Xl(_)));
    }

    #[test]
    fn test_data_too_large() {
        let allocator = EventAllocator::new();

        // Data larger than XL pool should fail
        let huge_data = vec![0u8; 32000];
        let result = allocator.allocate_event(&huge_data, 1);
        assert!(matches!(result, Err(AllocationError::TooLarge { .. })));

        // Test exact boundary
        let max_xl_data = vec![0u8; 16384];
        let result = allocator.allocate_event(&max_xl_data, 1);
        assert!(result.is_ok());

        let over_xl_data = vec![0u8; 16385];
        let result = allocator.allocate_event(&over_xl_data, 1);
        assert!(matches!(result, Err(AllocationError::TooLarge { .. })));
    }

    #[test]
    fn test_ring_ptr_cloning_and_reference_counting() {
        let allocator = EventAllocator::new();

        // Allocate an event
        let event = allocator.allocate_xs_event(b"test", 42).unwrap();
        let pool_id = PoolId::XS;
        let slot_index = event.slot_index;

        // Initial ref count should be 1
        let initial_ref_count = allocator.pools.get_ref_count(pool_id, slot_index);
        assert_eq!(initial_ref_count, 1);

        // Clone the RingPtr (should increment ref count)
        let cloned_event = event.clone();
        let ref_count_after_clone = allocator.pools.get_ref_count(pool_id, slot_index);
        assert_eq!(ref_count_after_clone, 2);

        // Both should point to same data
        assert_eq!(&event.data[..4], b"test");
        assert_eq!(&cloned_event.data[..4], b"test");
        assert_eq!(event.len, cloned_event.len);
        assert_eq!(event.event_type, cloned_event.event_type);

        // Drop one reference
        drop(event);
        let ref_count_after_drop = allocator.pools.get_ref_count(pool_id, slot_index);
        assert_eq!(ref_count_after_drop, 1);

        // Clone should still be valid
        assert_eq!(cloned_event.len, 4);
        assert_eq!(cloned_event.event_type, 42);
    }

    #[test]
    fn test_slot_reuse_after_all_references_dropped() {
        let allocator = EventAllocator::new();

        // Allocate event and get slot info
        let event = allocator.allocate_xs_event(b"test", 42).unwrap();
        let pool_id = PoolId::XS;
        let slot_index = event.slot_index;
        let initial_generation = event.generation;

        // Verify slot is allocated
        assert!(!allocator.pools.is_slot_available(pool_id, slot_index));

        // Drop the event (should make slot available)
        drop(event);

        // Slot should now be available with incremented generation
        assert!(allocator.pools.is_slot_available(pool_id, slot_index));
        let new_generation = allocator.pools.get_generation(pool_id, slot_index);
        assert_eq!(new_generation, initial_generation + 1);

        // Should be able to allocate to same slot again
        let new_event = allocator.allocate_xs_event(b"new", 100).unwrap();
        // Note: slot_index might be the same or different depending on allocation strategy
        assert_eq!(new_event.len, 3);
        assert_eq!(new_event.event_type, 100);
    }

    #[test]
    fn test_multiple_pool_allocation() {
        let allocator = EventAllocator::new();

        // Allocate events of different sizes simultaneously
        let xs_event = allocator.allocate_xs_event(b"xs", 1).unwrap();
        let s_event = allocator.allocate_s_event(&vec![0u8; 100], 2).unwrap();
        let m_event = allocator.allocate_m_event(&vec![0u8; 500], 3).unwrap();
        let l_event = allocator.allocate_l_event(&vec![0u8; 2000], 4).unwrap();
        let xl_event = allocator.allocate_xl_event(&vec![0u8; 8000], 5).unwrap();

        // All should have different pool IDs
        assert_eq!(xs_event.pool_id, PoolId::XS);
        assert_eq!(s_event.pool_id, PoolId::S);
        assert_eq!(m_event.pool_id, PoolId::M);
        assert_eq!(l_event.pool_id, PoolId::L);
        assert_eq!(xl_event.pool_id, PoolId::XL);

        // All should be accessible
        assert_eq!(xs_event.len, 2);
        assert_eq!(s_event.len, 100);
        assert_eq!(m_event.len, 500);
        assert_eq!(l_event.len, 2000);
        assert_eq!(xl_event.len, 8000);
    }

    #[test]
    fn test_multithreaded_allocation() {
        let allocator = Arc::new(EventAllocator::new());
        let mut handles = vec![];

        // Spawn 10 threads that each allocate 10 events
        for thread_id in 0..10 {
            let allocator_clone = allocator.clone();
            let handle = thread::spawn(move || {
                let mut allocated_events = vec![];

                for i in 0..10 {
                    let data = format!("thread_{}_event_{}", thread_id, i);
                    let event = allocator_clone.allocate_event(
                        data.as_bytes(),
                        (thread_id * 10 + i) as u32
                    ).unwrap();

                    // Verify the data
                    assert_eq!(event.data(), data.as_bytes());
                    assert_eq!(event.event_type(), (thread_id * 10 + i) as u32);

                    allocated_events.push(event);
                }

                allocated_events
            });
            handles.push(handle);
        }

        // Wait for all threads and collect results
        let mut all_events = vec![];
        for handle in handles {
            let thread_events = handle.join().unwrap();
            all_events.extend(thread_events);
        }

        // Should have 100 events total
        assert_eq!(all_events.len(), 100);

        // All events should be valid and unique
        for (i, event) in all_events.iter().enumerate() {
            let expected_data = format!("thread_{}_event_{}", i / 10, i % 10);
            assert_eq!(event.data(), expected_data.as_bytes());
            assert_eq!(event.event_type(), i as u32);
        }
    }

    #[test]
    fn test_pool_exhaustion() {
        let allocator = EventAllocator::new();
        let mut events = vec![];

        // Try to allocate more events than XS pool capacity (2000)
        // This tests the "pool full" error condition
        for i in 0..2001 {
            match allocator.allocate_xs_event(b"test", i as u32) {
                Ok(event) => {
                    events.push(event);
                }
                Err(AllocationError::PoolFull(PoolId::XS)) => {
                    // Should happen when we run out of slots
                    println!("Pool exhausted after {} allocations", events.len());
                    break;
                }
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }

        // Should have allocated close to capacity
        assert!(events.len() >= 1000); // At least got some allocations

        // Drop half the events
        let half = events.len() / 2;
        events.truncate(half);

        // Should be able to allocate again
        let new_event = allocator.allocate_xs_event(b"after_drop", 9999).unwrap();
        assert_eq!(new_event.len, 10);
        assert_eq!(new_event.event_type, 9999);
    }

    #[test]
    fn test_zero_length_events() {
        let allocator = EventAllocator::new();

        let empty_event = allocator.allocate_event(&[], 42).unwrap();
        assert_eq!(empty_event.len(), 0);
        assert_eq!(empty_event.event_type(), 42);
        assert_eq!(empty_event.data(), &[]);
    }

    #[test]
    fn test_allocated_event_enum_methods() {
        let allocator = EventAllocator::new();

        // Test each variant of AllocatedEvent
        let xs_event = allocator.allocate_event(b"xs", 1).unwrap();
        let s_event = allocator.allocate_event(&vec![0u8; 100], 2).unwrap();
        let m_event = allocator.allocate_event(&vec![0u8; 500], 3).unwrap();
        let l_event = allocator.allocate_event(&vec![0u8; 2000], 4).unwrap();
        let xl_event = allocator.allocate_event(&vec![0u8; 8000], 5).unwrap();

        // Test data() method
        assert_eq!(xs_event.data(), b"xs");
        assert_eq!(s_event.data().len(), 100);
        assert_eq!(m_event.data().len(), 500);
        assert_eq!(l_event.data().len(), 2000);
        assert_eq!(xl_event.data().len(), 8000);

        // Test event_type() method
        assert_eq!(xs_event.event_type(), 1);
        assert_eq!(s_event.event_type(), 2);
        assert_eq!(m_event.event_type(), 3);
        assert_eq!(l_event.event_type(), 4);
        assert_eq!(xl_event.event_type(), 5);

        // Test len() method
        assert_eq!(xs_event.len(), 2);
        assert_eq!(s_event.len(), 100);
        assert_eq!(m_event.len(), 500);
        assert_eq!(l_event.len(), 2000);
        assert_eq!(xl_event.len(), 8000);

        // Test pool_id() method
        assert_eq!(xs_event.pool_id(), PoolId::XS);
        assert_eq!(s_event.pool_id(), PoolId::S);
        assert_eq!(m_event.pool_id(), PoolId::M);
        assert_eq!(l_event.pool_id(), PoolId::L);
        assert_eq!(xl_event.pool_id(), PoolId::XL);
    }

    #[test]
    fn test_clone_semantics() {
        let allocator = EventAllocator::new();

        let original = allocator.allocate_event(b"clone test", 123).unwrap();
        let cloned = original.clone();

        // Both should have same data
        assert_eq!(original.data(), cloned.data());
        assert_eq!(original.event_type(), cloned.event_type());
        assert_eq!(original.len(), cloned.len());

        // Should be independent drops
        drop(original);

        // Clone should still work
        assert_eq!(cloned.data(), b"clone test");
        assert_eq!(cloned.event_type(), 123);
    }

    #[test]
    fn test_size_boundaries() {
        let allocator = EventAllocator::new();

        // Test exact size boundaries
        let boundary_tests = vec![
            (64, "XS -> S boundary"),
            (256, "S -> M boundary"),
            (1024, "M -> L boundary"),
            (4096, "L -> XL boundary"),
            (16384, "XL -> XXL boundary"),
        ];

        for (size, description) in boundary_tests {
            let data = vec![0u8; size];
            let result = allocator.allocate_event(&data, 1);

            if size <= 16384 {
                assert!(result.is_ok(), "Failed at {}: {}", size, description);
                let event = result.unwrap();
                assert_eq!(event.len(), size as u32);
            } else {
                assert!(result.is_err(), "Should fail at {}: {}", size, description);
            }
        }
    }

    #[test]
    fn test_generation_increment() {
        let allocator = EventAllocator::new();

        // Allocate and drop multiple events to same slot to test generation increment
        for expected_gen in 1..=5 {
            let event = allocator.allocate_xs_event(b"gen_test", 42).unwrap();
            let slot_index = event.slot_index;
            let generation = event.generation;

            // Drop the event
            drop(event);

            // Generation should have incremented
            let new_generation = allocator.pools.get_generation(PoolId::XS, slot_index);
            assert_eq!(new_generation, generation + 1);
        }
    }

    #[test]
    fn test_aba_protection() {
        let allocator = EventAllocator::new();

        // Allocate event and record its details
        let event1 = allocator.allocate_xs_event(b"aba1", 1).unwrap();
        let slot_index = event1.slot_index;
        let generation1 = event1.generation;

        // Drop it
        drop(event1);

        // Allocate again (might get same slot)
        let event2 = allocator.allocate_xs_event(b"aba2", 2).unwrap();

        if event2.slot_index == slot_index {
            // Same slot reused - generation should be different
            assert_ne!(event2.generation, generation1);
            assert_eq!(event2.generation, generation1 + 1);
        }

        // Both events should have different data even if same slot
        assert_eq!(event2.data[..4], *b"aba2");
        assert_eq!(event2.event_type, 2);
    }

    #[test]
    fn test_concurrent_clone_and_drop() {
        let allocator = Arc::new(EventAllocator::new());

        // Allocate an event
        let event = allocator.allocate_xs_event(b"concurrent", 42).unwrap();
        let event_arc = Arc::new(event);

        let mut handles = vec![];

        // Spawn multiple threads that clone and drop
        for _ in 0..10 {
            let event_clone = event_arc.clone();
            let handle = thread::spawn(move || {
                // Each thread clones the event multiple times
                let mut local_clones = vec![];
                for _ in 0..5 {
                    local_clones.push((*event_clone).clone());
                }

                // Verify all clones work
                for clone in &local_clones {
                    assert_eq!(&clone.data[..10], b"concurrent");
                    assert_eq!(clone.event_type, 42);
                }

                // Drop them (automatic when local_clones goes out of scope)
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Original should still be valid since it's in Arc
        assert_eq!(&event_arc.data[..10], b"concurrent");
    }
}