pub mod allocator;
pub mod pool;
pub mod ring;
pub mod ring_ptr;

pub use allocator::*;
pub use pool::*;
pub use ring::*;
pub use ring_ptr::*;

#[cfg(test)]
mod tests {
    use std::{sync::Arc, thread};

    use bytemuck::Zeroable;

    use super::*;

    // Test-specific smaller pools to avoid stack overflow
    struct TestEventAllocator {
        xs_pool: Arc<RingBuffer<64, 100>>, // Much smaller for tests
        s_pool: Arc<RingBuffer<256, 50>>,
        m_pool: Arc<RingBuffer<1024, 20>>,
        l_pool: Arc<RingBuffer<4096, 10>>,
        xl_pool: Arc<RingBuffer<16384, 5>>,
    }

    impl TestEventAllocator {
        fn new() -> Self {
            Self {
                xs_pool: Arc::new(RingBuffer::new()),
                s_pool: Arc::new(RingBuffer::new()),
                m_pool: Arc::new(RingBuffer::new()),
                l_pool: Arc::new(RingBuffer::new()),
                xl_pool: Arc::new(RingBuffer::new()),
            }
        }

        fn get_xs_writer(&self) -> Writer<64, 100> {
            Writer {
                ringbuffer: self.xs_pool.clone(),
                last_ts: 0,
            }
        }

        fn get_xs_reader(&self) -> Reader<64, 100> {
            Reader {
                ringbuffer: self.xs_pool.clone(),
                cursor: 0,
                last_ts: 0,
            }
        }

        fn get_s_writer(&self) -> Writer<256, 50> {
            Writer {
                ringbuffer: self.s_pool.clone(),
                last_ts: 0,
            }
        }

        fn get_m_writer(&self) -> Writer<1024, 20> {
            Writer {
                ringbuffer: self.m_pool.clone(),
                last_ts: 0,
            }
        }

        fn get_l_writer(&self) -> Writer<4096, 10> {
            Writer {
                ringbuffer: self.l_pool.clone(),
                last_ts: 0,
            }
        }

        fn get_xl_writer(&self) -> Writer<16384, 5> {
            Writer {
                ringbuffer: self.xl_pool.clone(),
                last_ts: 0,
            }
        }

        // Mock implementation of EventPools methods for testing
        fn get_slot_data(&self, pool_id: PoolId, slot_index: u32) -> *const u8 {
            unsafe {
                match pool_id {
                    PoolId::XS => {
                        let ptr = self.xs_pool.data.get();
                        &(*ptr)[slot_index as usize].data as *const u8
                    }
                    PoolId::S => {
                        let ptr = self.s_pool.data.get();
                        &(*ptr)[slot_index as usize].data as *const u8
                    }
                    PoolId::M => {
                        let ptr = self.m_pool.data.get();
                        &(*ptr)[slot_index as usize].data as *const u8
                    }
                    PoolId::L => {
                        let ptr = self.l_pool.data.get();
                        &(*ptr)[slot_index as usize].data as *const u8
                    }
                    PoolId::XL => {
                        let ptr = self.xl_pool.data.get();
                        &(*ptr)[slot_index as usize].data as *const u8
                    }
                }
            }
        }

        fn inc_ref_count(&self, pool_id: PoolId, slot_index: u32) -> u8 {
            unsafe {
                match pool_id {
                    PoolId::XS => {
                        let ptr = self.xs_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .ref_count
                            .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
                    }
                    PoolId::S => {
                        let ptr = self.s_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .ref_count
                            .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
                    }
                    PoolId::M => {
                        let ptr = self.m_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .ref_count
                            .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
                    }
                    PoolId::L => {
                        let ptr = self.l_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .ref_count
                            .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
                    }
                    PoolId::XL => {
                        let ptr = self.xl_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .ref_count
                            .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
                    }
                }
            }
        }

        fn dec_ref_count(&self, pool_id: PoolId, slot_index: u32) -> u8 {
            unsafe {
                match pool_id {
                    PoolId::XS => {
                        let ptr = self.xs_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .ref_count
                            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel)
                    }
                    PoolId::S => {
                        let ptr = self.s_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .ref_count
                            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel)
                    }
                    PoolId::M => {
                        let ptr = self.m_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .ref_count
                            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel)
                    }
                    PoolId::L => {
                        let ptr = self.l_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .ref_count
                            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel)
                    }
                    PoolId::XL => {
                        let ptr = self.xl_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .ref_count
                            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel)
                    }
                }
            }
        }

        fn mark_slot_reusable(&self, pool_id: PoolId, slot_index: u32) {
            unsafe {
                match pool_id {
                    PoolId::XS => {
                        let ptr = self.xs_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .generation
                            .fetch_add(1, std::sync::atomic::Ordering::Release);
                        (*ptr)[slot_index as usize]
                            .is_allocated
                            .store(0, std::sync::atomic::Ordering::Release);
                    }
                    PoolId::S => {
                        let ptr = self.s_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .generation
                            .fetch_add(1, std::sync::atomic::Ordering::Release);
                        (*ptr)[slot_index as usize]
                            .is_allocated
                            .store(0, std::sync::atomic::Ordering::Release);
                    }
                    PoolId::M => {
                        let ptr = self.m_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .generation
                            .fetch_add(1, std::sync::atomic::Ordering::Release);
                        (*ptr)[slot_index as usize]
                            .is_allocated
                            .store(0, std::sync::atomic::Ordering::Release);
                    }
                    PoolId::L => {
                        let ptr = self.l_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .generation
                            .fetch_add(1, std::sync::atomic::Ordering::Release);
                        (*ptr)[slot_index as usize]
                            .is_allocated
                            .store(0, std::sync::atomic::Ordering::Release);
                    }
                    PoolId::XL => {
                        let ptr = self.xl_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .generation
                            .fetch_add(1, std::sync::atomic::Ordering::Release);
                        (*ptr)[slot_index as usize]
                            .is_allocated
                            .store(0, std::sync::atomic::Ordering::Release);
                    }
                }
            }
        }

        fn get_generation(&self, pool_id: PoolId, slot_index: u32) -> u32 {
            unsafe {
                match pool_id {
                    PoolId::XS => {
                        let ptr = self.xs_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .generation
                            .load(std::sync::atomic::Ordering::Acquire) as u32
                    }
                    PoolId::S => {
                        let ptr = self.s_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .generation
                            .load(std::sync::atomic::Ordering::Acquire) as u32
                    }
                    PoolId::M => {
                        let ptr = self.m_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .generation
                            .load(std::sync::atomic::Ordering::Acquire) as u32
                    }
                    PoolId::L => {
                        let ptr = self.l_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .generation
                            .load(std::sync::atomic::Ordering::Acquire) as u32
                    }
                    PoolId::XL => {
                        let ptr = self.xl_pool.metadata.get();
                        (*ptr)[slot_index as usize]
                            .generation
                            .load(std::sync::atomic::Ordering::Acquire) as u32
                    }
                }
            }
        }

        // Helper to create a RingPtr for testing - creates a mock EventAllocator interface
        fn create_ring_ptr<T>(&'static self, pool_id: PoolId, slot_index: u32) -> TestRingPtr<T> {
            let generation = self.get_generation(pool_id, slot_index);

            // Set initial ref count to 1
            match pool_id {
                PoolId::XS => unsafe {
                    let ptr = self.xs_pool.metadata.get();
                    (*ptr)[slot_index as usize]
                        .ref_count
                        .store(1, std::sync::atomic::Ordering::Release);
                    (*ptr)[slot_index as usize]
                        .is_allocated
                        .store(1, std::sync::atomic::Ordering::Release);
                },
                PoolId::S => unsafe {
                    let ptr = self.s_pool.metadata.get();
                    (*ptr)[slot_index as usize]
                        .ref_count
                        .store(1, std::sync::atomic::Ordering::Release);
                    (*ptr)[slot_index as usize]
                        .is_allocated
                        .store(1, std::sync::atomic::Ordering::Release);
                },
                PoolId::M => unsafe {
                    let ptr = self.m_pool.metadata.get();
                    (*ptr)[slot_index as usize]
                        .ref_count
                        .store(1, std::sync::atomic::Ordering::Release);
                    (*ptr)[slot_index as usize]
                        .is_allocated
                        .store(1, std::sync::atomic::Ordering::Release);
                },
                PoolId::L => unsafe {
                    let ptr = self.l_pool.metadata.get();
                    (*ptr)[slot_index as usize]
                        .ref_count
                        .store(1, std::sync::atomic::Ordering::Release);
                    (*ptr)[slot_index as usize]
                        .is_allocated
                        .store(1, std::sync::atomic::Ordering::Release);
                },
                PoolId::XL => unsafe {
                    let ptr = self.xl_pool.metadata.get();
                    (*ptr)[slot_index as usize]
                        .ref_count
                        .store(1, std::sync::atomic::Ordering::Release);
                    (*ptr)[slot_index as usize]
                        .is_allocated
                        .store(1, std::sync::atomic::Ordering::Release);
                },
            }

            TestRingPtr::new(pool_id, slot_index, generation, self)
        }
    }

    // Test-specific RingPtr that uses TestEventAllocator
    struct TestRingPtr<T> {
        pool_id: PoolId,
        slot_index: u32,
        generation: u32,
        allocator: &'static TestEventAllocator,
        _phantom: std::marker::PhantomData<T>,
    }

    impl<T> TestRingPtr<T> {
        fn new(pool_id: PoolId, slot_index: u32, generation: u32, allocator: &'static TestEventAllocator) -> Self {
            Self {
                pool_id,
                slot_index,
                generation,
                allocator,
                _phantom: std::marker::PhantomData,
            }
        }
    }

    impl<T> std::ops::Deref for TestRingPtr<T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            unsafe { &*(self.allocator.get_slot_data(self.pool_id, self.slot_index) as *const T) }
        }
    }

    impl<T> Clone for TestRingPtr<T> {
        fn clone(&self) -> Self {
            self.allocator.inc_ref_count(self.pool_id, self.slot_index);
            Self {
                pool_id: self.pool_id,
                slot_index: self.slot_index,
                generation: self.generation,
                allocator: self.allocator,
                _phantom: std::marker::PhantomData,
            }
        }
    }

    impl<T> Drop for TestRingPtr<T> {
        fn drop(&mut self) {
            let remaining = self.allocator.dec_ref_count(self.pool_id, self.slot_index);
            if remaining == 1 {
                // We were the last reference
                self.allocator.mark_slot_reusable(self.pool_id, self.slot_index);
            }
        }
    }

    // Thread-safe static allocator for testing
    use std::sync::OnceLock;
    static TEST_ALLOCATOR: OnceLock<TestEventAllocator> = OnceLock::new();

    fn get_test_allocator() -> &'static TestEventAllocator {
        TEST_ALLOCATOR.get_or_init(TestEventAllocator::new)
    }

    // Original tests
    #[test]
    fn test_pooled_event_creation() {
        let mut event = PooledEvent::<64>::zeroed();
        let test_data = b"hello world";
        event.data[..test_data.len()].copy_from_slice(test_data);
        event.len = test_data.len() as u32;
        event.event_type = 42;

        assert_eq!(event.len, 11);
        assert_eq!(event.event_type, 42);
        assert_eq!(&event.data[..11], test_data);
    }

    #[test]
    fn test_ring_buffer_single_writer_reader() {
        let allocator = get_test_allocator();
        let mut writer = allocator.get_xs_writer();
        let mut reader = allocator.get_xs_reader();

        // Create test event
        let test_event = EventAllocator::create_pooled_event::<64>(b"test event", 1).expect("Failed to create event");

        // Write event
        assert!(writer.add(test_event));

        // Read event
        let read_event = reader.next().expect("Should have event");
        assert_eq!(read_event.len, 10);
        assert_eq!(read_event.event_type, 1);
        assert_eq!(&read_event.data[..10], b"test event");
    }

    #[test]
    fn test_size_estimation() {
        assert_eq!(EventAllocator::estimate_size(32), EventSize::XS);
        assert_eq!(EventAllocator::estimate_size(64), EventSize::XS);
        assert_eq!(EventAllocator::estimate_size(128), EventSize::S);
        assert_eq!(EventAllocator::estimate_size(512), EventSize::M);
        assert_eq!(EventAllocator::estimate_size(2048), EventSize::L);
        assert_eq!(EventAllocator::estimate_size(8192), EventSize::XL);
        assert_eq!(EventAllocator::estimate_size(32768), EventSize::XXL);
    }

    #[test]
    fn test_data_too_large_for_pool() {
        let large_data = vec![0u8; 128]; // Too big for XS pool (64 bytes)
        let result = EventAllocator::create_pooled_event::<64>(&large_data, 1);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Data too large for this pool size");
    }

    #[test]
    fn test_cache_line_alignment() {
        let ring = RingBuffer::<64, 10>::new(); // Small test ring
        let ring_ptr = &ring as *const _ as usize;

        // Check that RingBuffer is cache-line aligned (64 bytes)
        assert_eq!(ring_ptr % 64, 0, "RingBuffer should be 64-byte aligned");

        let event = PooledEvent::<64>::zeroed();
        let event_ptr = &event as *const _ as usize;

        // Check that PooledEvent is cache-line aligned
        assert_eq!(event_ptr % 64, 0, "PooledEvent should be 64-byte aligned");
    }

    #[test]
    fn test_zero_length_event() {
        let event = EventAllocator::create_pooled_event::<64>(&[], 42).unwrap();
        assert_eq!(event.len, 0);
        assert_eq!(event.event_type, 42);
    }

    // RingPtr Tests
    #[test]
    fn test_ring_ptr_basic_creation_and_deref() {
        let allocator = get_test_allocator();

        // Manually place test data in slot 0
        let test_event = EventAllocator::create_pooled_event::<64>(b"test data", 42).expect("Failed to create event");

        unsafe {
            let ptr = allocator.xs_pool.data.get();
            (*ptr)[0] = test_event;
        }

        // Create RingPtr pointing to slot 0
        let ring_ptr: TestRingPtr<PooledEvent<64>> = allocator.create_ring_ptr(PoolId::XS, 0);

        // Test dereferencing
        let event_ref = &*ring_ptr;
        assert_eq!(event_ref.len, 9);
        assert_eq!(event_ref.event_type, 42);
        assert_eq!(&event_ref.data[..9], b"test data");
    }

    #[test]
    fn test_ring_ptr_reference_counting() {
        let allocator = get_test_allocator();

        // Manually place test data in slot 1
        let test_event =
            EventAllocator::create_pooled_event::<256>(b"ref count test", 100).expect("Failed to create event");

        unsafe {
            let ptr = allocator.s_pool.data.get();
            (*ptr)[1] = test_event;
        }

        // Create first RingPtr (ref_count should be 1)
        let ring_ptr1: TestRingPtr<PooledEvent<256>> = allocator.create_ring_ptr(PoolId::S, 1);

        // Verify initial ref count
        unsafe {
            let ptr = allocator.s_pool.metadata.get();
            let ref_count = (*ptr)[1].ref_count.load(std::sync::atomic::Ordering::Acquire);
            assert_eq!(ref_count, 1);
        }

        // Clone the RingPtr (ref_count should become 2)
        let ring_ptr2 = ring_ptr1.clone();

        // Verify ref count increased
        unsafe {
            let ptr = allocator.s_pool.metadata.get();
            let ref_count = (*ptr)[1].ref_count.load(std::sync::atomic::Ordering::Acquire);
            assert_eq!(ref_count, 2);
        }

        // Both pointers should point to same data
        assert_eq!(ring_ptr1.len, ring_ptr2.len);
        assert_eq!(ring_ptr1.event_type, ring_ptr2.event_type);

        // Drop one reference
        drop(ring_ptr1);

        // Verify ref count decreased
        unsafe {
            let ptr = allocator.s_pool.metadata.get();
            let ref_count = (*ptr)[1].ref_count.load(std::sync::atomic::Ordering::Acquire);
            assert_eq!(ref_count, 1);
        }

        // Second pointer should still work
        assert_eq!(ring_ptr2.len, 14);
        assert_eq!(ring_ptr2.event_type, 100);
    }

    #[test]
    fn test_ring_ptr_slot_reuse_on_drop() {
        let allocator = get_test_allocator();

        // Manually place test data in slot 2
        let test_event =
            EventAllocator::create_pooled_event::<1024>(b"slot reuse test", 200).expect("Failed to create event");

        unsafe {
            let ptr = allocator.m_pool.data.get();
            (*ptr)[2] = test_event;
        }

        let original_generation = unsafe {
            let ptr = allocator.m_pool.metadata.get();
            (*ptr)[2].generation.load(std::sync::atomic::Ordering::Acquire)
        };

        // Create RingPtr and immediately drop it
        {
            let ring_ptr: TestRingPtr<PooledEvent<1024>> = allocator.create_ring_ptr(PoolId::M, 2);
            assert_eq!(ring_ptr.len, 15);
        } // ring_ptr dropped here

        // Verify slot was marked as reusable
        unsafe {
            let ptr = allocator.m_pool.metadata.get();
            let is_allocated = (*ptr)[2].is_allocated.load(std::sync::atomic::Ordering::Acquire);
            let generation = (*ptr)[2].generation.load(std::sync::atomic::Ordering::Acquire);

            assert_eq!(is_allocated, 0); // Should be marked as free
            assert_eq!(generation, original_generation + 1); // Generation should increment
        }
    }

    #[test]
    fn test_ring_ptr_multi_threaded_sharing() {
        let allocator = get_test_allocator();

        // Manually place test data in slot 3
        let test_event =
            EventAllocator::create_pooled_event::<64>(b"thread test", 300).expect("Failed to create event");

        unsafe {
            let ptr = allocator.xs_pool.data.get();
            (*ptr)[3] = test_event;
        }

        // Create RingPtr
        let ring_ptr: TestRingPtr<PooledEvent<64>> = allocator.create_ring_ptr(PoolId::XS, 3);

        // Clone for multiple threads (simulating Arc<XaeroEvent> pattern)
        let ptr1 = ring_ptr.clone();
        let ptr2 = ring_ptr.clone();
        let ptr3 = ring_ptr.clone();

        // Verify all threads can access the same data
        let handles: Vec<_> = vec![ptr1, ptr2, ptr3]
            .into_iter()
            .enumerate()
            .map(|(i, ptr)| {
                thread::spawn(move || {
                    // Each thread verifies it can read the same data
                    assert_eq!(ptr.len, 11);
                    assert_eq!(ptr.event_type, 300);
                    assert_eq!(&ptr.data[..11], b"thread test");
                    format!("Thread {} OK", i)
                })
            })
            .collect();

        // Wait for all threads and collect results
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(results.len(), 3);

        // Original ring_ptr should still be valid
        assert_eq!(ring_ptr.len, 11);
        assert_eq!(ring_ptr.event_type, 300);
    }

    #[test]
    fn test_ring_ptr_zero_copy_semantics() {
        let allocator = get_test_allocator();

        // Create test event with known data pattern
        let test_data = b"zero copy validation test data for ring buffer";
        let test_event = EventAllocator::create_pooled_event::<256>(test_data, 400).expect("Failed to create event");

        // Manually place in slot 0
        unsafe {
            let ptr = allocator.s_pool.data.get();
            (*ptr)[0] = test_event;
        }

        // Create RingPtr
        let ring_ptr: TestRingPtr<PooledEvent<256>> = allocator.create_ring_ptr(PoolId::S, 0);

        // Get pointer to the data through RingPtr
        let ring_data_ptr = ring_ptr.data.as_ptr();

        // Get pointer to the same data directly from ring buffer
        let direct_data_ptr = unsafe {
            let ptr = allocator.s_pool.data.get();
            (*ptr)[0].data.as_ptr()
        };

        // They should be the same pointer (zero-copy)
        assert_eq!(ring_data_ptr, direct_data_ptr);

        // Verify data integrity
        assert_eq!(ring_ptr.len, test_data.len() as u32);
        assert_eq!(&ring_ptr.data[..test_data.len()], test_data);
    }

    #[test]
    fn test_ring_ptr_aba_protection() {
        let allocator = get_test_allocator();

        // Manually place first event in slot 4
        let event1 = EventAllocator::create_pooled_event::<64>(b"first event", 500).expect("Failed to create event");

        unsafe {
            let ptr = allocator.xs_pool.data.get();
            (*ptr)[4] = event1;
        }

        // Get initial generation
        let initial_generation = unsafe {
            let ptr = allocator.xs_pool.metadata.get();
            (*ptr)[4].generation.load(std::sync::atomic::Ordering::Acquire) as u32
        };

        // Create and immediately drop RingPtr (this should increment generation)
        {
            let ring_ptr: TestRingPtr<PooledEvent<64>> = allocator.create_ring_ptr(PoolId::XS, 4);
            assert_eq!(ring_ptr.generation, initial_generation);
        } // Dropped here, generation should increment

        // Create second event in same slot (simulating slot reuse)
        let event2 = EventAllocator::create_pooled_event::<64>(b"second event", 501).expect("Failed to create event");
        // Manually overwrite slot 4 (simulating writer reusing the slot)
        unsafe {
            let ptr = allocator.xs_pool.data.get();
            (*ptr)[4] = event2;
        }

        // Get new generation (should be different)
        let new_generation = unsafe {
            let ptr = allocator.xs_pool.metadata.get();
            (*ptr)[4].generation.load(std::sync::atomic::Ordering::Acquire) as u32
        };

        // Verify generation changed (ABA protection)
        assert_ne!(initial_generation, new_generation);
        assert_eq!(new_generation, initial_generation + 1);
    }

    #[test]
    fn test_ring_ptr_different_pool_sizes() {
        let allocator = get_test_allocator();

        // Test XS pool (64 bytes)
        let xs_ptr: TestRingPtr<PooledEvent<64>> = allocator.create_ring_ptr(PoolId::XS, 5);
        assert_eq!(xs_ptr.pool_id as u8, PoolId::XS as u8);

        // Test S pool (256 bytes)
        let s_ptr: TestRingPtr<PooledEvent<256>> = allocator.create_ring_ptr(PoolId::S, 5);
        assert_eq!(s_ptr.pool_id as u8, PoolId::S as u8);

        // Test M pool (1024 bytes)
        let m_ptr: TestRingPtr<PooledEvent<1024>> = allocator.create_ring_ptr(PoolId::M, 5);
        assert_eq!(m_ptr.pool_id as u8, PoolId::M as u8);

        // Verify they point to different pools
        assert_ne!(xs_ptr.pool_id as u8, s_ptr.pool_id as u8);
        assert_ne!(s_ptr.pool_id as u8, m_ptr.pool_id as u8);
    }

    #[test]
    fn test_ring_ptr_batch_release_pattern() {
        let allocator = get_test_allocator();

        // Simulate operator pipeline: create 5 events to "fold/reduce"
        let mut ring_ptrs = Vec::new();
        for i in 0..5 {
            let test_event =
                EventAllocator::create_pooled_event::<64>(format!("event {}", i).as_bytes(), 600 + i as u32)
                    .expect("Failed to create event");

            // Manually place in specific slots
            unsafe {
                let ptr = allocator.xs_pool.data.get();
                (*ptr)[10 + i] = test_event;
            }

            let ring_ptr: TestRingPtr<PooledEvent<64>> = allocator.create_ring_ptr(PoolId::XS, 10 + i as u32);
            ring_ptrs.push(ring_ptr);
        }

        // Verify all 5 events are accessible
        for (i, ptr) in ring_ptrs.iter().enumerate() {
            assert_eq!(ptr.event_type, 600 + i as u32);
        }

        // Simulate processing: clone all RingPtrs (increment ref counts)
        let cloned_ptrs: Vec<_> = ring_ptrs.iter().cloned().collect();

        // Verify ref counts are 2 for each slot
        for i in 0..5 {
            unsafe {
                let ptr = allocator.xs_pool.metadata.get();
                let ref_count = (*ptr)[10 + i].ref_count.load(std::sync::atomic::Ordering::Acquire);
                assert_eq!(ref_count, 2);
            }
        }

        // Simulate batch release: drop all original RingPtrs
        drop(ring_ptrs);

        // Verify ref counts are now 1 for each slot
        for i in 0..5 {
            unsafe {
                let ptr = allocator.xs_pool.metadata.get();
                let ref_count = (*ptr)[10 + i].ref_count.load(std::sync::atomic::Ordering::Acquire);
                assert_eq!(ref_count, 1);
            }
        }

        // Cloned ptrs should still be valid
        for (i, ptr) in cloned_ptrs.iter().enumerate() {
            assert_eq!(ptr.event_type, 600 + i as u32);
        }

        // Final cleanup - drop cloned ptrs (slots should be marked reusable)
        drop(cloned_ptrs);

        // Verify all slots are now marked as free
        for i in 0..5 {
            unsafe {
                let ptr = allocator.xs_pool.metadata.get();
                let is_allocated = (*ptr)[10 + i].is_allocated.load(std::sync::atomic::Ordering::Acquire);
                assert_eq!(is_allocated, 0);
            }
        }
    }
}
