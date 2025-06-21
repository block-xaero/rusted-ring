use crate::{pool::PoolId, ring::PooledEvent, ring_ptr::RingPtr};

/// Unified wrapper for ring pointers to different pool sizes
#[derive(Debug, Clone)]
pub enum PooledEventPtr {
    Xs(RingPtr<PooledEvent<64>>),
    S(RingPtr<PooledEvent<256>>),
    M(RingPtr<PooledEvent<1024>>),
    L(RingPtr<PooledEvent<4096>>),
    Xl(RingPtr<PooledEvent<16384>>),
}

impl PooledEventPtr {
    pub fn data(&self) -> &[u8] {
        match self {
            PooledEventPtr::Xs(ptr) => &ptr.data[..ptr.len as usize],
            PooledEventPtr::S(ptr) => &ptr.data[..ptr.len as usize],
            PooledEventPtr::M(ptr) => &ptr.data[..ptr.len as usize],
            PooledEventPtr::L(ptr) => &ptr.data[..ptr.len as usize],
            PooledEventPtr::Xl(ptr) => &ptr.data[..ptr.len as usize],
        }
    }

    pub fn event_type(&self) -> u8 {
        match self {
            PooledEventPtr::Xs(ptr) => ptr.event_type as u8,
            PooledEventPtr::S(ptr) => ptr.event_type as u8,
            PooledEventPtr::M(ptr) => ptr.event_type as u8,
            PooledEventPtr::L(ptr) => ptr.event_type as u8,
            PooledEventPtr::Xl(ptr) => ptr.event_type as u8,
        }
    }

    pub fn len(&self) -> u32 {
        match self {
            PooledEventPtr::Xs(ptr) => ptr.len,
            PooledEventPtr::S(ptr) => ptr.len,
            PooledEventPtr::M(ptr) => ptr.len,
            PooledEventPtr::L(ptr) => ptr.len,
            PooledEventPtr::Xl(ptr) => ptr.len,
        }
    }

    pub fn slot_index(&self) -> u32 {
        match self {
            PooledEventPtr::Xs(ptr) => ptr.slot_index,
            PooledEventPtr::S(ptr) => ptr.slot_index,
            PooledEventPtr::M(ptr) => ptr.slot_index,
            PooledEventPtr::L(ptr) => ptr.slot_index,
            PooledEventPtr::Xl(ptr) => ptr.slot_index,
        }
    }

    pub fn pool_id(&self) -> PoolId {
        match self {
            PooledEventPtr::Xs(ptr) => ptr.pool_id,
            PooledEventPtr::S(ptr) => ptr.pool_id,
            PooledEventPtr::M(ptr) => ptr.pool_id,
            PooledEventPtr::L(ptr) => ptr.pool_id,
            PooledEventPtr::Xl(ptr) => ptr.pool_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, OnceLock};

    use super::*;
    use crate::{EventAllocator, pool::PoolId};

    // Shared allocator for tests to avoid stack overflow
    static TEST_ALLOCATOR: OnceLock<Arc<EventAllocator>> = OnceLock::new();

    fn get_test_allocator() -> Arc<EventAllocator> {
        TEST_ALLOCATOR.get_or_init(|| Arc::new(EventAllocator::new())).clone()
    }

    #[test]
    fn test_pooled_event_ptr_xs_creation_and_access() {
        let allocator = get_test_allocator();
        let test_data = b"small data";

        let allocated_event = allocator.allocate_event(test_data, 42).unwrap();
        let pooled_ptr = allocated_event.into_pooled_event_ptr();

        // Should be XS variant for small data
        match &pooled_ptr {
            PooledEventPtr::Xs(_) => {} // Expected
            _ => panic!("Expected XS variant for small data"),
        }

        // Test unified interface
        assert_eq!(pooled_ptr.data(), test_data);
        assert_eq!(pooled_ptr.event_type(), 42);
        assert_eq!(pooled_ptr.len(), test_data.len() as u32);
        assert_eq!(pooled_ptr.pool_id(), PoolId::XS);
    }

    #[test]
    fn test_pooled_event_ptr_s_variant() {
        let allocator = get_test_allocator();
        let test_data = vec![1u8; 128]; // Too big for XS, should use S

        let allocated_event = allocator.allocate_event(&test_data, 50).unwrap();
        let pooled_ptr = allocated_event.into_pooled_event_ptr();

        match &pooled_ptr {
            PooledEventPtr::S(_) => {} // Expected
            _ => panic!("Expected S variant for 128 byte data"),
        }

        assert_eq!(pooled_ptr.data(), &test_data[..]);
        assert_eq!(pooled_ptr.event_type(), 50);
        assert_eq!(pooled_ptr.len(), 128);
        assert_eq!(pooled_ptr.pool_id(), PoolId::S);
    }

    #[test]
    fn test_pooled_event_ptr_m_variant() {
        let allocator = get_test_allocator();
        let test_data = vec![2u8; 512]; // Should use M pool

        let allocated_event = allocator.allocate_event(&test_data, 75).unwrap();
        let pooled_ptr = allocated_event.into_pooled_event_ptr();

        match &pooled_ptr {
            PooledEventPtr::M(_) => {} // Expected
            _ => panic!("Expected M variant for 512 byte data"),
        }

        assert_eq!(pooled_ptr.data(), &test_data[..]);
        assert_eq!(pooled_ptr.event_type(), 75);
        assert_eq!(pooled_ptr.len(), 512);
        assert_eq!(pooled_ptr.pool_id(), PoolId::M);
    }

    #[test]
    fn test_pooled_event_ptr_l_variant() {
        let allocator = get_test_allocator();
        let test_data = vec![3u8; 2048]; // Should use L pool

        let allocated_event = allocator.allocate_event(&test_data, 100).unwrap();
        let pooled_ptr = allocated_event.into_pooled_event_ptr();

        match &pooled_ptr {
            PooledEventPtr::L(_) => {} // Expected
            _ => panic!("Expected L variant for 2KB data"),
        }

        assert_eq!(pooled_ptr.data(), &test_data[..]);
        assert_eq!(pooled_ptr.event_type(), 100);
        assert_eq!(pooled_ptr.len(), 2048);
        assert_eq!(pooled_ptr.pool_id(), PoolId::L);
    }

    #[test]
    fn test_pooled_event_ptr_xl_variant() {
        let allocator = get_test_allocator();
        let test_data = vec![4u8; 8192]; // Should use XL pool

        let allocated_event = allocator.allocate_event(&test_data, 125).unwrap();
        let pooled_ptr = allocated_event.into_pooled_event_ptr();

        match &pooled_ptr {
            PooledEventPtr::Xl(_) => {} // Expected
            _ => panic!("Expected XL variant for 8KB data"),
        }

        assert_eq!(pooled_ptr.data(), &test_data[..]);
        assert_eq!(pooled_ptr.event_type(), 125);
        assert_eq!(pooled_ptr.len(), 8192);
        assert_eq!(pooled_ptr.pool_id(), PoolId::XL);
    }

    #[test]
    fn test_pooled_event_ptr_cloning() {
        let allocator = get_test_allocator();
        let test_data = b"clone test data";

        let allocated_event = allocator.allocate_event(test_data, 42).unwrap();
        let original = allocated_event.into_pooled_event_ptr();
        let cloned = original.clone();

        // Both should have same data
        assert_eq!(original.data(), cloned.data());
        assert_eq!(original.event_type(), cloned.event_type());
        assert_eq!(original.len(), cloned.len());
        assert_eq!(original.pool_id(), cloned.pool_id());

        // Should have same slot index (pointing to same slot)
        assert_eq!(original.slot_index(), cloned.slot_index());

        // Verify underlying ring pointer reference counting works
        // (Both should be able to access the same data)
        assert_eq!(original.data(), test_data);
        assert_eq!(cloned.data(), test_data);
    }

    #[test]
    fn test_pooled_event_ptr_zero_length_data() {
        let allocator = get_test_allocator();

        let allocated_event = allocator.allocate_event(&[], 255).unwrap();
        let pooled_ptr = allocated_event.into_pooled_event_ptr();

        assert_eq!(pooled_ptr.data(), &[]);
        assert_eq!(pooled_ptr.event_type(), 255);
        assert_eq!(pooled_ptr.len(), 0);

        // Zero-length should still go to XS pool
        assert_eq!(pooled_ptr.pool_id(), PoolId::XS);
    }

    #[test]
    fn test_pooled_event_ptr_size_boundaries() {
        let allocator = get_test_allocator();

        // Test exact boundaries between pool sizes
        let boundary_tests = vec![
            (64, PoolId::XS),    // Max for XS
            (65, PoolId::S),     // Min for S
            (256, PoolId::S),    // Max for S
            (257, PoolId::M),    // Min for M
            (1024, PoolId::M),   // Max for M
            (1025, PoolId::L),   // Min for L
            (4096, PoolId::L),   // Max for L
            (4097, PoolId::XL),  // Min for XL
            (16384, PoolId::XL), // Max for XL
        ];

        for (size, expected_pool) in boundary_tests {
            let test_data = vec![0u8; size];
            let allocated_event = allocator.allocate_event(&test_data, 1).unwrap();
            let pooled_ptr = allocated_event.into_pooled_event_ptr();

            assert_eq!(
                pooled_ptr.pool_id(),
                expected_pool,
                "Size {} should use pool {:?}",
                size,
                expected_pool
            );
            assert_eq!(pooled_ptr.len(), size as u32);
            assert_eq!(pooled_ptr.data().len(), size);
        }
    }

    #[test]
    fn test_pooled_event_ptr_too_large_data() {
        let allocator = get_test_allocator();
        let huge_data = vec![0u8; 32000]; // Too big for any pool

        let result = allocator.allocate_event(&huge_data, 1);
        assert!(result.is_err());

        match result.unwrap_err() {
            crate::AllocationError::TooLarge { data_len, max_size } => {
                assert_eq!(data_len, 32000);
                assert_eq!(max_size, 16384);
            }
            _ => panic!("Expected TooLarge error"),
        }
    }

    #[test]
    fn test_pooled_event_ptr_uniform_interface() {
        let allocator = get_test_allocator();

        // Create events of different sizes
        let allocated_events = vec![
            allocator.allocate_event(b"xs", 1).unwrap(),
            allocator.allocate_event(&[0u8; 100], 2).unwrap(),
            allocator.allocate_event(&vec![0u8; 500], 3).unwrap(),
            allocator.allocate_event(&vec![0u8; 2000], 4).unwrap(),
            allocator.allocate_event(&vec![0u8; 8000], 5).unwrap(),
        ];

        // Convert to PooledEventPtr
        let events: Vec<_> = allocated_events
            .into_iter()
            .map(|ae| ae.into_pooled_event_ptr())
            .collect();

        // All should work with the same interface
        for (i, event) in events.iter().enumerate() {
            assert_eq!(event.event_type(), (i + 1) as u8);
            assert!(event.len() > 0);
            assert!(!event.data().is_empty());

            // Should be able to call slot_index on all variants
            let _slot = event.slot_index();
        }

        // Verify they use different pool sizes
        let pool_ids: Vec<_> = events.iter().map(|e| e.pool_id()).collect();
        let expected = vec![PoolId::XS, PoolId::S, PoolId::M, PoolId::L, PoolId::XL];
        assert_eq!(pool_ids, expected);
    }

    #[test]
    fn test_pooled_event_ptr_debug_display() {
        let allocator = get_test_allocator();
        let allocated_event = allocator.allocate_event(b"debug test", 42).unwrap();
        let pooled_ptr = allocated_event.into_pooled_event_ptr();

        // Should be able to debug print
        let debug_str = format!("{:?}", pooled_ptr);
        println!("Debug output: {}", debug_str); // For debugging the test

        // Should contain variant info (the actual enum variant names)
        match &pooled_ptr {
            PooledEventPtr::Xs(_) => assert!(debug_str.contains("Xs")),
            PooledEventPtr::S(_) => assert!(debug_str.contains("S")),
            PooledEventPtr::M(_) => assert!(debug_str.contains("M")),
            PooledEventPtr::L(_) => assert!(debug_str.contains("L")),
            PooledEventPtr::Xl(_) => assert!(debug_str.contains("Xl")),
        }

        // Should contain some indication this is a ring pointer structure
        assert!(
            debug_str.contains("RingPtr")
                || debug_str.contains("Xs")
                || debug_str.contains("S")
                || debug_str.contains("M")
                || debug_str.contains("L")
                || debug_str.contains("Xl")
        );
    }

    #[test]
    fn test_pooled_event_ptr_concurrent_access() {
        use std::thread;

        let allocator = Arc::new(EventAllocator::new());
        let test_data = b"concurrent test data";

        // Create event in main thread
        let allocated_event = allocator.allocate_event(test_data, 42).unwrap();
        let pooled_ptr = allocated_event.into_pooled_event_ptr();
        let pooled_ptr_arc = Arc::new(pooled_ptr);

        let mut handles = vec![];

        // Spawn multiple threads that access the same PooledEventPtr
        for i in 0..5 {
            let ptr_clone = pooled_ptr_arc.clone();
            let handle = thread::spawn(move || {
                // Each thread should be able to read the data
                assert_eq!(ptr_clone.data(), test_data);
                assert_eq!(ptr_clone.event_type(), 42);
                assert_eq!(ptr_clone.len(), test_data.len() as u32);

                format!("Thread {} completed", i)
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            let result = handle.join().unwrap();
            assert!(result.contains("completed"));
        }

        // Original should still be valid
        assert_eq!(pooled_ptr_arc.data(), test_data);
    }

    #[test]
    fn test_pooled_event_ptr_drop_behavior() {
        let allocator = get_test_allocator();
        let test_data = b"drop test";

        // Create and immediately scope the PooledEventPtr
        {
            let allocated_event = allocator.allocate_event(test_data, 42).unwrap();
            let pooled_ptr = allocated_event.into_pooled_event_ptr();

            // Verify it works
            assert_eq!(pooled_ptr.data(), test_data);
        } // pooled_ptr dropped here

        // After drop, should be able to allocate to the same slot again
        // (This tests that the slot is properly freed)
        let new_data = b"new data after drop";
        let new_allocated = allocator.allocate_event(new_data, 100).unwrap();
        let new_ptr = new_allocated.into_pooled_event_ptr();

        // Might or might not be the same slot (depends on allocation strategy)
        // but should definitely be able to allocate successfully
        assert_eq!(new_ptr.data(), new_data);
        assert_eq!(new_ptr.event_type(), 100);
    }

    #[test]
    fn test_pooled_event_ptr_reference_counting() {
        let allocator = get_test_allocator();
        let test_data = b"ref count test";

        let allocated_event = allocator.allocate_event(test_data, 42).unwrap();
        let original = allocated_event.into_pooled_event_ptr();
        let slot_index = original.slot_index();

        // Clone it multiple times
        let clone1 = original.clone();
        let clone2 = original.clone();
        let clone3 = clone1.clone();

        // All should point to same data
        assert_eq!(original.data(), test_data);
        assert_eq!(clone1.data(), test_data);
        assert_eq!(clone2.data(), test_data);
        assert_eq!(clone3.data(), test_data);

        // All should have same slot index
        assert_eq!(original.slot_index(), slot_index);
        assert_eq!(clone1.slot_index(), slot_index);
        assert_eq!(clone2.slot_index(), slot_index);
        assert_eq!(clone3.slot_index(), slot_index);

        // Drop some clones
        drop(clone1);
        drop(clone2);

        // Remaining should still work
        assert_eq!(original.data(), test_data);
        assert_eq!(clone3.data(), test_data);

        // Drop the rest
        drop(original);
        drop(clone3);

        // Should be able to allocate again (slot freed)
        let new_allocated = allocator.allocate_event(b"after drop", 200).unwrap();
        let new_ptr = new_allocated.into_pooled_event_ptr();
        assert_eq!(new_ptr.data(), b"after drop");
    }
}
