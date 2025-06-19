#[allow(dead_code)]
mod allocator;
mod pool;
mod ring;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocator::EventAllocator;
    use crate::ring::{EventSize, PooledEvent, Reader, RingBuffer, Writer};
    use bytemuck::Zeroable;
    use std::sync::Arc;
    use std::thread;

    // Test-specific smaller pools to avoid stack overflow
    struct TestEventAllocator {
        xs_pool: Arc<RingBuffer<64, 100>>, // Much smaller for tests
        s_pool: Arc<RingBuffer<256, 50>>,
        m_pool: Arc<RingBuffer<1024, 20>>,
    }

    impl TestEventAllocator {
        fn new() -> Self {
            Self {
                xs_pool: Arc::new(RingBuffer::new()),
                s_pool: Arc::new(RingBuffer::new()),
                m_pool: Arc::new(RingBuffer::new()),
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
    }

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
        let allocator = TestEventAllocator::new();
        let mut writer = allocator.get_xs_writer();
        let mut reader = allocator.get_xs_reader();

        // Create test event
        let test_event = EventAllocator::create_pooled_event::<64>(b"test event", 1)
            .expect("Failed to create event");

        // Write event
        assert!(writer.add(test_event));

        // Read event
        let read_event = reader.next().expect("Should have event");
        assert_eq!(read_event.len, 10);
        assert_eq!(read_event.event_type, 1);
        assert_eq!(&read_event.data[..10], b"test event");
    }

    #[test]
    fn test_ring_buffer_wraparound() {
        let allocator = TestEventAllocator::new();
        let mut writer = allocator.get_xs_writer();
        let mut reader = allocator.get_xs_reader();

        // Fill the entire ring buffer (test XS has 100 capacity)
        for i in 0..100 {
            let data = format!("event_{}", i);
            let event = EventAllocator::create_pooled_event::<64>(data.as_bytes(), i as u32)
                .expect("Failed to create event");
            assert!(writer.add(event));
        }

        // Add one more to test wraparound
        let wrap_event = EventAllocator::create_pooled_event::<64>(b"wraparound", 9999)
            .expect("Failed to create event");
        assert!(writer.add(wrap_event));

        // Read some events to make space
        for _ in 0..10 {
            reader.next();
        }

        // Should be able to add more after reading
        let another_event = EventAllocator::create_pooled_event::<64>(b"after_wrap", 10000)
            .expect("Failed to create event");
        assert!(writer.add(another_event));
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
    fn test_multiple_pool_sizes() {
        let allocator = TestEventAllocator::new();

        // Test different pool sizes with smaller test data
        let xs_data = b"small";
        let s_data = &[0u8; 200];
        let m_data = &[1u8; 800];

        let xs_event = EventAllocator::create_pooled_event::<64>(xs_data, 1).unwrap();
        let s_event = EventAllocator::create_pooled_event::<256>(s_data, 2).unwrap();
        let m_event = EventAllocator::create_pooled_event::<1024>(m_data, 3).unwrap();

        let mut xs_writer = allocator.get_xs_writer();
        let mut s_writer = allocator.get_s_writer();
        let mut m_writer = allocator.get_m_writer();

        assert!(xs_writer.add(xs_event));
        assert!(s_writer.add(s_event));
        assert!(m_writer.add(m_event));
    }

    #[allow(clippy::while_let_on_iterator)]
    #[test]
    fn test_concurrent_single_writer_multiple_readers() {
        let allocator = Arc::new(TestEventAllocator::new());
        let mut writer = allocator.get_xs_writer();

        // Write some events first
        for i in 0..50 {
            // Reduced from 100 for smaller test pool
            let data = format!("event_{}", i);
            let event = EventAllocator::create_pooled_event::<64>(data.as_bytes(), i)
                .expect("Failed to create event");
            writer.add(event);
        }

        let mut handles = vec![];

        // Spawn multiple readers
        for reader_id in 0..3 {
            let alloc = allocator.clone();
            let handle = thread::spawn(move || {
                let mut reader = alloc.get_xs_reader();
                let mut count = 0;

                // Try to read events
                while let Some(_event) = reader.next() {
                    count += 1;
                    if count >= 15 {
                        // Each reader gets some events
                        break;
                    }
                }

                (reader_id, count)
            });
            handles.push(handle);
        }

        // Wait for all readers
        for handle in handles {
            let (reader_id, count) = handle.join().unwrap();
            assert!(
                count > 0,
                "Reader {} should have read some events",
                reader_id
            );
        }
    }

    #[test]
    fn test_reader_cant_overtake_writer() {
        let allocator = TestEventAllocator::new();
        let writer = allocator.get_xs_writer();
        let mut reader = allocator.get_xs_reader();

        // Try to read from empty buffer
        assert!(
            reader.next().is_none(),
            "Should not be able to read ahead of writer"
        );

        // Write one event
        let mut writer = writer;
        let event = EventAllocator::create_pooled_event::<64>(b"test", 1).unwrap();
        writer.add(event);

        // Should be able to read one event
        assert!(reader.next().is_some());

        // Should not be able to read another
        assert!(
            reader.next().is_none(),
            "Should not be able to read ahead of writer"
        );
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
    fn test_memory_ordering_semantics() {
        use std::sync::atomic::Ordering;

        let allocator = TestEventAllocator::new();
        let mut writer = allocator.get_xs_writer();

        // Verify initial cursor position
        let initial_cursor = writer.ringbuffer.write_cursor.load(Ordering::Acquire);
        assert_eq!(initial_cursor, 0);

        // Add an event and verify cursor advancement
        let event = EventAllocator::create_pooled_event::<64>(b"test", 1).unwrap();
        writer.add(event);

        let new_cursor = writer.ringbuffer.write_cursor.load(Ordering::Acquire);
        assert_eq!(new_cursor, 1);
    }

    #[test]
    fn test_zero_length_event() {
        let event = EventAllocator::create_pooled_event::<64>(&[], 42).unwrap();
        assert_eq!(event.len, 0);
        assert_eq!(event.event_type, 42);
    }
}
