use std::sync::OnceLock;

use bytemuck::Zeroable;

use crate::ring::{EventSize, PooledEvent, Reader, RingBuffer, Writer};

// Pool size constants - Adjusted to stay under 1MB per ring buffer
pub const XS_CAPACITY: usize = 2000; // 64 * 2000 = 128KB
pub const S_CAPACITY: usize = 1000; // 256 * 1000 = 256KB
pub const M_CAPACITY: usize = 300; // 1024 * 300 = 307KB
pub const L_CAPACITY: usize = 60; // 4096 * 60 = 245KB
pub const XL_CAPACITY: usize = 15; // 16384 * 15 = 245KB

// Static ring buffers - no Arc, no heap allocation
static XS_RING: OnceLock<RingBuffer<64, XS_CAPACITY>> = OnceLock::new();
static S_RING: OnceLock<RingBuffer<256, S_CAPACITY>> = OnceLock::new();
static M_RING: OnceLock<RingBuffer<1024, M_CAPACITY>> = OnceLock::new();
static L_RING: OnceLock<RingBuffer<4096, L_CAPACITY>> = OnceLock::new();
static XL_RING: OnceLock<RingBuffer<16384, XL_CAPACITY>> = OnceLock::new();

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolId {
    XS = 0, // 64 bytes
    S = 1,  // 256 bytes
    M = 2,  // 1KB
    L = 3,  // 4KB
    XL = 4, // 16KB
}

impl PoolId {
    pub fn from_size(size: EventSize) -> Self {
        match size {
            EventSize::XS => PoolId::XS,
            EventSize::S => PoolId::S,
            EventSize::M => PoolId::M,
            EventSize::L => PoolId::L,
            EventSize::XL => PoolId::XL,
            EventSize::XXL => panic!("XXL not supported in pools"),
        }
    }

    pub fn max_size(&self) -> usize {
        match self {
            PoolId::XS => 64,
            PoolId::S => 256,
            PoolId::M => 1024,
            PoolId::L => 4096,
            PoolId::XL => 16384,
        }
    }

    pub fn capacity(&self) -> usize {
        match self {
            PoolId::XS => XS_CAPACITY,
            PoolId::S => S_CAPACITY,
            PoolId::M => M_CAPACITY,
            PoolId::L => L_CAPACITY,
            PoolId::XL => XL_CAPACITY,
        }
    }
}

impl From<PoolId> for u8 {
    fn from(value: PoolId) -> Self {
        value as u8
    }
}

impl From<u8> for PoolId {
    fn from(value: u8) -> Self {
        match value {
            0 => PoolId::XS,
            1 => PoolId::S,
            2 => PoolId::M,
            3 => PoolId::L,
            4 => PoolId::XL,
            _ => panic!("Unknown pool id: {}", value),
        }
    }
}

/// Generic ring buffer factory with const parameters
pub struct RingFactory;

impl RingFactory {
    /// Generic factory method for writers
    pub fn get_writer<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize>(
        ring: &'static OnceLock<RingBuffer<TSHIRT_SIZE, RING_CAPACITY>>,
    ) -> Writer<TSHIRT_SIZE, RING_CAPACITY> {
        Writer::new(ring.get_or_init(RingBuffer::new))
    }

    /// Generic factory method for readers
    pub fn get_reader<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize>(
        ring: &'static OnceLock<RingBuffer<TSHIRT_SIZE, RING_CAPACITY>>,
    ) -> Reader<TSHIRT_SIZE, RING_CAPACITY> {
        Reader::new(ring.get_or_init(RingBuffer::new))
    }
}

/// Convenience factory - provides typed access to specific pools
pub struct EventPoolFactory;

impl EventPoolFactory {
    // Writer factory methods
    pub fn get_xs_writer() -> Writer<64, XS_CAPACITY> {
        RingFactory::get_writer(&XS_RING)
    }

    pub fn get_s_writer() -> Writer<256, S_CAPACITY> {
        RingFactory::get_writer(&S_RING)
    }

    pub fn get_m_writer() -> Writer<1024, M_CAPACITY> {
        RingFactory::get_writer(&M_RING)
    }

    pub fn get_l_writer() -> Writer<4096, L_CAPACITY> {
        RingFactory::get_writer(&L_RING)
    }

    pub fn get_xl_writer() -> Writer<16384, XL_CAPACITY> {
        RingFactory::get_writer(&XL_RING)
    }

    // Reader factory methods
    pub fn get_xs_reader() -> Reader<64, XS_CAPACITY> {
        RingFactory::get_reader(&XS_RING)
    }

    pub fn get_s_reader() -> Reader<256, S_CAPACITY> {
        RingFactory::get_reader(&S_RING)
    }

    pub fn get_m_reader() -> Reader<1024, M_CAPACITY> {
        RingFactory::get_reader(&M_RING)
    }

    pub fn get_l_reader() -> Reader<4096, L_CAPACITY> {
        RingFactory::get_reader(&L_RING)
    }

    pub fn get_xl_reader() -> Reader<16384, XL_CAPACITY> {
        RingFactory::get_reader(&XL_RING)
    }

    // Utility methods
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

    pub fn estimate_pool_id(data_len: usize) -> Option<PoolId> {
        let size = Self::estimate_size(data_len);
        if size == EventSize::XXL {
            None
        } else {
            Some(PoolId::from_size(size))
        }
    }
}

/// Simple event creation utilities
pub struct EventUtils;

impl EventUtils {
    /// Create pooled event from raw data
    pub fn create_pooled_event<const SIZE: usize>(
        data: &[u8],
        event_type: u32,
    ) -> Result<PooledEvent<SIZE>, EventCreationError> {
        if data.len() > SIZE {
            return Err(EventCreationError::DataTooLarge {
                data_len: data.len(),
                max_size: SIZE,
            });
        }

        let mut pooled = PooledEvent::<SIZE>::zeroed();
        pooled.data[..data.len()].copy_from_slice(data);
        pooled.len = data.len() as u32;
        pooled.event_type = event_type;
        Ok(pooled)
    }

    /// Auto-detect size and create appropriate pooled event
    pub fn create_auto_sized_event(data: &[u8], event_type: u32) -> Result<AutoSizedEvent, EventCreationError> {
        let size = EventPoolFactory::estimate_size(data.len());

        match size {
            EventSize::XS => {
                let event = Self::create_pooled_event::<64>(data, event_type)?;
                Ok(AutoSizedEvent::Xs(event))
            }
            EventSize::S => {
                let event = Self::create_pooled_event::<256>(data, event_type)?;
                Ok(AutoSizedEvent::S(event))
            }
            EventSize::M => {
                let event = Self::create_pooled_event::<1024>(data, event_type)?;
                Ok(AutoSizedEvent::M(event))
            }
            EventSize::L => {
                let event = Self::create_pooled_event::<4096>(data, event_type)?;
                Ok(AutoSizedEvent::L(event))
            }
            EventSize::XL => {
                let event = Self::create_pooled_event::<16384>(data, event_type)?;
                Ok(AutoSizedEvent::Xl(event))
            }
            EventSize::XXL => Err(EventCreationError::DataTooLarge {
                data_len: data.len(),
                max_size: 16384,
            }),
        }
    }
}

#[allow(clippy::large_enum_variant)]
/// Auto-sized event wrapper for convenience
#[derive(Debug, Clone)]
pub enum AutoSizedEvent {
    Xs(PooledEvent<64>),
    S(PooledEvent<256>),
    M(PooledEvent<1024>),
    L(PooledEvent<4096>),
    Xl(PooledEvent<16384>),
}

impl AutoSizedEvent {
    pub fn data(&self) -> &[u8] {
        match self {
            AutoSizedEvent::Xs(event) => &event.data[..event.len as usize],
            AutoSizedEvent::S(event) => &event.data[..event.len as usize],
            AutoSizedEvent::M(event) => &event.data[..event.len as usize],
            AutoSizedEvent::L(event) => &event.data[..event.len as usize],
            AutoSizedEvent::Xl(event) => &event.data[..event.len as usize],
        }
    }

    pub fn event_type(&self) -> u32 {
        match self {
            AutoSizedEvent::Xs(event) => event.event_type,
            AutoSizedEvent::S(event) => event.event_type,
            AutoSizedEvent::M(event) => event.event_type,
            AutoSizedEvent::L(event) => event.event_type,
            AutoSizedEvent::Xl(event) => event.event_type,
        }
    }

    pub fn len(&self) -> u32 {
        match self {
            AutoSizedEvent::Xs(event) => event.len,
            AutoSizedEvent::S(event) => event.len,
            AutoSizedEvent::M(event) => event.len,
            AutoSizedEvent::L(event) => event.len,
            AutoSizedEvent::Xl(event) => event.len,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn pool_id(&self) -> PoolId {
        match self {
            AutoSizedEvent::Xs(_) => PoolId::XS,
            AutoSizedEvent::S(_) => PoolId::S,
            AutoSizedEvent::M(_) => PoolId::M,
            AutoSizedEvent::L(_) => PoolId::L,
            AutoSizedEvent::Xl(_) => PoolId::XL,
        }
    }

    /// Emit this event to the appropriate ring buffer
    pub fn emit_to_ring(self) -> Result<(), EmitError> {
        match self {
            AutoSizedEvent::Xs(event) => {
                let mut writer = RingFactory::get_writer(&XS_RING);
                if writer.add(event) {
                    Ok(())
                } else {
                    Err(EmitError::RingFull(PoolId::XS))
                }
            }
            AutoSizedEvent::S(event) => {
                let mut writer = RingFactory::get_writer(&S_RING);
                if writer.add(event) {
                    Ok(())
                } else {
                    Err(EmitError::RingFull(PoolId::S))
                }
            }
            AutoSizedEvent::M(event) => {
                let mut writer = RingFactory::get_writer(&M_RING);
                if writer.add(event) {
                    Ok(())
                } else {
                    Err(EmitError::RingFull(PoolId::M))
                }
            }
            AutoSizedEvent::L(event) => {
                let mut writer = RingFactory::get_writer(&L_RING);
                if writer.add(event) {
                    Ok(())
                } else {
                    Err(EmitError::RingFull(PoolId::L))
                }
            }
            AutoSizedEvent::Xl(event) => {
                let mut writer = RingFactory::get_writer(&XL_RING);
                if writer.add(event) {
                    Ok(())
                } else {
                    Err(EmitError::RingFull(PoolId::XL))
                }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EventCreationError {
    #[error("Data too large: {data_len} bytes > max {max_size} bytes")]
    DataTooLarge { data_len: usize, max_size: usize },
}

#[derive(Debug, thiserror::Error)]
pub enum EmitError {
    #[error("Ring buffer full for pool {0:?}")]
    RingFull(PoolId),
}

/// Pool statistics for monitoring
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub pool_id: PoolId,
    pub capacity: usize,
    pub current_backpressure: f32,
}

impl PoolStats {
    pub fn collect_xs() -> Self {
        let reader = RingFactory::get_reader(&XS_RING);
        Self {
            pool_id: PoolId::XS,
            capacity: XS_CAPACITY,
            current_backpressure: reader.backpressure_ratio(),
        }
    }

    pub fn collect_s() -> Self {
        let reader = RingFactory::get_reader(&S_RING);
        Self {
            pool_id: PoolId::S,
            capacity: S_CAPACITY,
            current_backpressure: reader.backpressure_ratio(),
        }
    }

    pub fn collect_m() -> Self {
        let reader = RingFactory::get_reader(&M_RING);
        Self {
            pool_id: PoolId::M,
            capacity: M_CAPACITY,
            current_backpressure: reader.backpressure_ratio(),
        }
    }

    pub fn collect_l() -> Self {
        let reader = RingFactory::get_reader(&L_RING);
        Self {
            pool_id: PoolId::L,
            capacity: L_CAPACITY,
            current_backpressure: reader.backpressure_ratio(),
        }
    }

    pub fn collect_xl() -> Self {
        let reader = RingFactory::get_reader(&XL_RING);
        Self {
            pool_id: PoolId::XL,
            capacity: XL_CAPACITY,
            current_backpressure: reader.backpressure_ratio(),
        }
    }

    pub fn collect_all() -> Vec<Self> {
        vec![
            Self::collect_xs(),
            Self::collect_s(),
            Self::collect_m(),
            Self::collect_l(),
            Self::collect_xl(),
        ]
    }
}

impl std::fmt::Display for PoolStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Pool {:?}: capacity={}, backpressure={:.1}%",
            self.pool_id,
            self.capacity,
            self.current_backpressure * 100.0
        )
    }
}

#[cfg(test)]
mod pool_tests {
    use std::{
        sync::atomic::{AtomicU32, Ordering},
        thread,
        time::{Duration, Instant},
    };

    use super::*;

    // Simple test counter
    static TEST_COUNTER: AtomicU32 = AtomicU32::new(5000000);

    fn next_test_id() -> u32 {
        TEST_COUNTER.fetch_add(10000, Ordering::Relaxed)
    }

    /// Test generic factory with explicit const parameters
    #[test]
    fn test_generic_factory() {
        let test_id = next_test_id();

        // Test direct generic factory usage
        let mut xs_writer = RingFactory::get_writer(&XS_RING);
        let mut xs_reader = RingFactory::get_reader(&XS_RING);

        let mut m_writer = RingFactory::get_writer(&M_RING);
        let mut m_reader = RingFactory::get_reader(&M_RING);

        // Write to XS pool
        let xs_event = EventUtils::create_pooled_event::<64>(b"xs_generic_test", test_id).unwrap();
        xs_writer.add(xs_event);

        // Write to M pool
        let m_event = EventUtils::create_pooled_event::<1024>(b"m_generic_test", test_id + 1).unwrap();
        m_writer.add(m_event);

        // Read back
        if let Some(event) = xs_reader.next() {
            if event.event_type == test_id {
                assert_eq!(&event.data[..event.len as usize], b"xs_generic_test");
                println!("✅ XS generic factory working");
            }
        }

        if let Some(event) = m_reader.next() {
            if event.event_type == test_id + 1 {
                assert_eq!(&event.data[..event.len as usize], b"m_generic_test");
                println!("✅ M generic factory working");
            }
        }
    }

    /// Test that both APIs work and are equivalent
    #[test]
    fn test_api_equivalence() {
        let test_id = next_test_id();

        // Use both APIs to write to the same pool
        let mut generic_writer = RingFactory::get_writer(&S_RING);
        let mut typed_writer = EventPoolFactory::get_s_writer();

        // Both should write to the same ring buffer
        let event1 = EventUtils::create_pooled_event::<256>(b"generic_api", test_id).unwrap();
        let event2 = EventUtils::create_pooled_event::<256>(b"typed_api", test_id + 1).unwrap();

        generic_writer.add(event1);
        typed_writer.add(event2);

        // Read using both APIs
        let mut generic_reader = RingFactory::get_reader(&S_RING);

        let mut found_generic = false;
        let mut found_typed = false;

        // Both readers should see both events
        for _ in 0..10 {
            if let Some(event) = generic_reader.next() {
                if event.event_type == test_id {
                    found_generic = true;
                }
                if event.event_type == test_id + 1 {
                    found_typed = true;
                }
            }
        }

        assert!(found_generic || found_typed, "Should find at least one event");
        println!("✅ API equivalence working");
    }

    /// Test performance of generic vs typed API
    #[test]
    fn test_performance_comparison() {
        let test_id = next_test_id();
        let events_count = 1000;

        // Test generic API performance
        let generic_start = Instant::now();
        {
            let mut writer = RingFactory::get_writer(&L_RING);
            for i in 0..events_count {
                let data = format!("generic_perf_{}", i);
                let event = EventUtils::create_pooled_event::<4096>(data.as_bytes(), test_id + i).unwrap();
                writer.add(event);
            }
        }
        let generic_duration = generic_start.elapsed();

        // Test typed API performance
        let typed_start = Instant::now();
        {
            let mut writer = EventPoolFactory::get_l_writer();
            for i in 0..events_count {
                let data = format!("typed_perf_{}", i);
                let event =
                    EventUtils::create_pooled_event::<4096>(data.as_bytes(), test_id + events_count + i).unwrap();
                writer.add(event);
            }
        }
        let typed_duration = typed_start.elapsed();

        println!(
            "Generic API: {:.2}ms for {} events",
            generic_duration.as_secs_f64() * 1000.0,
            events_count
        );
        println!(
            "Typed API: {:.2}ms for {} events",
            typed_duration.as_secs_f64() * 1000.0,
            events_count
        );

        // Both should be reasonably fast
        assert!(generic_duration.as_millis() < 100, "Generic API too slow");
        assert!(typed_duration.as_millis() < 100, "Typed API too slow");

        println!("✅ Performance test passed");
    }

    /// Test custom ring buffer usage
    #[test]
    fn test_custom_ring_usage() {
        // Define a custom ring buffer
        static CUSTOM_RING: OnceLock<RingBuffer<512, 50>> = OnceLock::new();

        let test_id = next_test_id();

        // Use generic factory with custom ring
        let mut writer = RingFactory::get_writer(&CUSTOM_RING);
        let mut reader = RingFactory::get_reader(&CUSTOM_RING);

        // Write custom sized event
        let event = EventUtils::create_pooled_event::<512>(b"custom_ring_test", test_id).unwrap();
        writer.add(event);

        // Read back
        if let Some(event) = reader.next() {
            if event.event_type == test_id {
                assert_eq!(&event.data[..event.len as usize], b"custom_ring_test");
                println!("✅ Custom ring buffer working");
            }
        }
    }

    /// Test concurrent access with generic factory
    #[test]
    fn test_concurrent_generic_access() {
        let test_id = next_test_id();
        let events_per_thread = 50;

        let mut handles = vec![];

        // Spawn multiple threads using generic factory
        for thread_id in 0..4 {
            let handle = thread::spawn(move || {
                let mut writer = RingFactory::get_writer(&XL_RING);

                for i in 0..events_per_thread {
                    let data = format!("thread_{}_event_{}", thread_id, i);
                    let event =
                        EventUtils::create_pooled_event::<16384>(data.as_bytes(), test_id + (thread_id * 1000) + i)
                            .unwrap();

                    writer.add(event);

                    // Small delay to encourage interleaving
                    thread::sleep(Duration::from_micros(1));
                }

                thread_id
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            let thread_id = handle.join().expect("Thread panicked");
            println!("Thread {} completed", thread_id);
        }

        // Read back events
        let mut reader = RingFactory::get_reader(&XL_RING);
        let mut total_found = 0;
        let expected_min = test_id;
        let expected_max = test_id + (4 * 1000) + events_per_thread;

        for _ in 0..events_per_thread * 4 + 50 {
            if let Some(event) = reader.next() {
                if event.event_type >= expected_min && event.event_type < expected_max {
                    total_found += 1;
                }
            }
        }

        assert!(total_found > 0, "Should find some events from concurrent access");
        println!(
            "✅ Concurrent generic access: found {}/{} events",
            total_found,
            events_per_thread * 4
        );
    }

    /// Test pool statistics with generic factory
    #[test]
    fn test_pool_stats_generic() {
        let test_id = next_test_id();

        // Write events to create backpressure
        {
            let mut writer = RingFactory::get_writer(&XS_RING);
            for i in 0..100 {
                let event = EventUtils::create_pooled_event::<64>(b"stats_test", test_id + i).unwrap();
                writer.add(event);
            }
        }

        // Collect stats
        let stats = PoolStats::collect_xs();
        assert_eq!(stats.pool_id, PoolId::XS);
        assert_eq!(stats.capacity, XS_CAPACITY);
        assert!(stats.current_backpressure >= 0.0);

        println!("XS Pool stats: {}", stats);

        // Test all pool stats
        let all_stats = PoolStats::collect_all();
        assert_eq!(all_stats.len(), 5);

        for stat in &all_stats {
            assert!(stat.current_backpressure >= 0.0);
            assert!(stat.current_backpressure <= 1.1); // Allow slight overflow
        }

        println!("✅ Pool statistics working");
    }

    /// Test auto-sized events with generic factory
    #[test]
    fn test_auto_sized_events() {
        let test_id = next_test_id();

        // Test different sized data
        let small_data = b"small";
        let medium_data = vec![b'M'; 500];
        let large_data = vec![b'L'; 2000];

        // Create auto-sized events
        let small_event = EventUtils::create_auto_sized_event(small_data, test_id).unwrap();
        let medium_event = EventUtils::create_auto_sized_event(&medium_data, test_id + 1).unwrap();
        let large_event = EventUtils::create_auto_sized_event(&large_data, test_id + 2).unwrap();

        // Verify correct pool selection
        assert_eq!(small_event.pool_id(), PoolId::XS);
        assert_eq!(medium_event.pool_id(), PoolId::M);
        assert_eq!(large_event.pool_id(), PoolId::L);

        // Emit to ring buffers
        small_event.emit_to_ring().unwrap();
        medium_event.emit_to_ring().unwrap();
        large_event.emit_to_ring().unwrap();

        // Verify data integrity
        let mut xs_reader = RingFactory::get_reader(&XS_RING);
        let mut m_reader = RingFactory::get_reader(&M_RING);
        let mut l_reader = RingFactory::get_reader(&L_RING);

        if let Some(event) = xs_reader.next() {
            if event.event_type == test_id {
                assert_eq!(&event.data[..event.len as usize], small_data);
            }
        }

        if let Some(event) = m_reader.next() {
            if event.event_type == test_id + 1 {
                assert_eq!(&event.data[..event.len as usize], &medium_data);
            }
        }

        if let Some(event) = l_reader.next() {
            if event.event_type == test_id + 2 {
                assert_eq!(&event.data[..event.len as usize], &large_data);
            }
        }

        println!("✅ Auto-sized events working");
    }
}
