use std::sync::OnceLock;
use bytemuck::Zeroable;
use crate::ring::{EventSize, PooledEvent, RingBuffer, Reader, Writer};

// Pool size constants - Adjusted to stay under 1MB per ring buffer
pub const XS_CAPACITY: usize = 2000; // 64 * 2000 = 128KB
pub const S_CAPACITY: usize = 1000;  // 256 * 1000 = 256KB
pub const M_CAPACITY: usize = 300;   // 1024 * 300 = 307KB
pub const L_CAPACITY: usize = 60;    // 4096 * 60 = 245KB
pub const XL_CAPACITY: usize = 15;   // 16384 * 15 = 245KB

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

/// Factory functions for LMAX writers - each gets static ring buffer reference
pub struct EventPoolFactory;

impl EventPoolFactory {
    // Writer factory methods
    pub fn get_xs_writer() -> Writer<64, XS_CAPACITY> {
        Writer::new(XS_RING.get_or_init(|| RingBuffer::new()))
    }

    pub fn get_s_writer() -> Writer<256, S_CAPACITY> {
        Writer::new(S_RING.get_or_init(|| RingBuffer::new()))
    }

    pub fn get_m_writer() -> Writer<1024, M_CAPACITY> {
        Writer::new(M_RING.get_or_init(|| RingBuffer::new()))
    }

    pub fn get_l_writer() -> Writer<4096, L_CAPACITY> {
        Writer::new(L_RING.get_or_init(|| RingBuffer::new()))
    }

    pub fn get_xl_writer() -> Writer<16384, XL_CAPACITY> {
        Writer::new(XL_RING.get_or_init(|| RingBuffer::new()))
    }

    // Reader factory methods
    pub fn get_xs_reader() -> Reader<64, XS_CAPACITY> {
        Reader::new(XS_RING.get_or_init(|| RingBuffer::new()))
    }

    pub fn get_s_reader() -> Reader<256, S_CAPACITY> {
        Reader::new(S_RING.get_or_init(|| RingBuffer::new()))
    }

    pub fn get_m_reader() -> Reader<1024, M_CAPACITY> {
        Reader::new(M_RING.get_or_init(|| RingBuffer::new()))
    }

    pub fn get_l_reader() -> Reader<4096, L_CAPACITY> {
        Reader::new(L_RING.get_or_init(|| RingBuffer::new()))
    }

    pub fn get_xl_reader() -> Reader<16384, XL_CAPACITY> {
        Reader::new(XL_RING.get_or_init(|| RingBuffer::new()))
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
    pub fn create_auto_sized_event(
        data: &[u8],
        event_type: u32
    ) -> Result<AutoSizedEvent, EventCreationError> {
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
                let mut writer = EventPoolFactory::get_xs_writer();
                if writer.add(event) {
                    Ok(())
                } else {
                    Err(EmitError::RingFull(PoolId::XS))
                }
            }
            AutoSizedEvent::S(event) => {
                let mut writer = EventPoolFactory::get_s_writer();
                if writer.add(event) {
                    Ok(())
                } else {
                    Err(EmitError::RingFull(PoolId::S))
                }
            }
            AutoSizedEvent::M(event) => {
                let mut writer = EventPoolFactory::get_m_writer();
                if writer.add(event) {
                    Ok(())
                } else {
                    Err(EmitError::RingFull(PoolId::M))
                }
            }
            AutoSizedEvent::L(event) => {
                let mut writer = EventPoolFactory::get_l_writer();
                if writer.add(event) {
                    Ok(())
                } else {
                    Err(EmitError::RingFull(PoolId::L))
                }
            }
            AutoSizedEvent::Xl(event) => {
                let mut writer = EventPoolFactory::get_xl_writer();
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
        let reader = EventPoolFactory::get_xs_reader();
        Self {
            pool_id: PoolId::XS,
            capacity: XS_CAPACITY,
            current_backpressure: reader.backpressure_ratio(),
        }
    }

    pub fn collect_s() -> Self {
        let reader = EventPoolFactory::get_s_reader();
        Self {
            pool_id: PoolId::S,
            capacity: S_CAPACITY,
            current_backpressure: reader.backpressure_ratio(),
        }
    }

    pub fn collect_m() -> Self {
        let reader = EventPoolFactory::get_m_reader();
        Self {
            pool_id: PoolId::M,
            capacity: M_CAPACITY,
            current_backpressure: reader.backpressure_ratio(),
        }
    }

    pub fn collect_l() -> Self {
        let reader = EventPoolFactory::get_l_reader();
        Self {
            pool_id: PoolId::L,
            capacity: L_CAPACITY,
            current_backpressure: reader.backpressure_ratio(),
        }
    }

    pub fn collect_xl() -> Self {
        let reader = EventPoolFactory::get_xl_reader();
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
mod tests {
    use super::*;

    #[test]
    fn test_pool_factory() {
        // Test writer creation
        let mut xs_writer = EventPoolFactory::get_xs_writer();
        let mut s_writer = EventPoolFactory::get_s_writer();

        // Test reader creation
        let mut xs_reader = EventPoolFactory::get_xs_reader();
        let mut s_reader = EventPoolFactory::get_s_reader();

        // Should be able to write and read
        let xs_event = EventUtils::create_pooled_event::<64>(b"test", 42).unwrap();
        assert!(xs_writer.add(xs_event));

        let s_event = EventUtils::create_pooled_event::<256>(b"larger test", 100).unwrap();
        assert!(s_writer.add(s_event));

        // Should be able to read back
        if let Some(read_xs) = xs_reader.next() {
            assert_eq!(&read_xs.data[..4], b"test");
            assert_eq!(read_xs.event_type, 42);
        }

        if let Some(read_s) = s_reader.next() {
            assert_eq!(&read_s.data[..11], b"larger test");
            assert_eq!(read_s.event_type, 100);
        }
    }

    #[test]
    fn test_auto_sized_events() {
        // Small data -> XS
        let xs_event = EventUtils::create_auto_sized_event(b"small", 1).unwrap();
        assert!(matches!(xs_event, AutoSizedEvent::Xs(_)));
        assert_eq!(xs_event.pool_id(), PoolId::XS);

        // Medium data -> S
        let s_data = vec![0u8; 128];
        let s_event = EventUtils::create_auto_sized_event(&s_data, 2).unwrap();
        assert!(matches!(s_event, AutoSizedEvent::S(_)));
        assert_eq!(s_event.pool_id(), PoolId::S);

        // Large data -> M
        let m_data = vec![0u8; 512];
        let m_event = EventUtils::create_auto_sized_event(&m_data, 3).unwrap();
        assert!(matches!(m_event, AutoSizedEvent::M(_)));
        assert_eq!(m_event.pool_id(), PoolId::M);
    }

    #[test]
    fn test_emit_to_ring() {
        let event = EventUtils::create_auto_sized_event(b"test emit", 42).unwrap();

        // Should successfully emit to ring
        assert!(event.emit_to_ring().is_ok());

        // Should be able to read it back
        let mut reader = EventPoolFactory::get_xs_reader();
        if let Some(read_event) = reader.next() {
            assert_eq!(&read_event.data[..9], b"test emit");
            assert_eq!(read_event.event_type, 42);
        }
    }

    #[test]
    fn test_pool_stats() {
        // Emit some events
        let event1 = EventUtils::create_auto_sized_event(b"test1", 1).unwrap();
        let event2 = EventUtils::create_auto_sized_event(b"test2", 2).unwrap();

        event1.emit_to_ring().unwrap();
        event2.emit_to_ring().unwrap();

        // Check stats
        let stats = PoolStats::collect_xs();
        assert_eq!(stats.pool_id, PoolId::XS);
        assert_eq!(stats.capacity, XS_CAPACITY);
        assert!(stats.current_backpressure > 0.0);

        // Collect all stats
        let all_stats = PoolStats::collect_all();
        assert_eq!(all_stats.len(), 5);
    }

    #[test]
    fn test_size_estimation() {
        assert_eq!(EventPoolFactory::estimate_size(32), EventSize::XS);
        assert_eq!(EventPoolFactory::estimate_size(64), EventSize::XS);
        assert_eq!(EventPoolFactory::estimate_size(65), EventSize::S);
        assert_eq!(EventPoolFactory::estimate_size(256), EventSize::S);
        assert_eq!(EventPoolFactory::estimate_size(257), EventSize::M);
        assert_eq!(EventPoolFactory::estimate_size(1024), EventSize::M);
        assert_eq!(EventPoolFactory::estimate_size(1025), EventSize::L);
        assert_eq!(EventPoolFactory::estimate_size(4096), EventSize::L);
        assert_eq!(EventPoolFactory::estimate_size(4097), EventSize::XL);
        assert_eq!(EventPoolFactory::estimate_size(16384), EventSize::XL);
        assert_eq!(EventPoolFactory::estimate_size(16385), EventSize::XXL);
    }

    #[test]
    fn test_data_too_large() {
        let huge_data = vec![0u8; 20000];
        let result = EventUtils::create_auto_sized_event(&huge_data, 1);
        assert!(matches!(result, Err(EventCreationError::DataTooLarge { .. })));
    }
}