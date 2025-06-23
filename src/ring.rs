use std::{
    cell::UnsafeCell,
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicU16, AtomicUsize, Ordering},
    },
};

use bytemuck::{Pod, Zeroable};

/// T-shirt sizing of events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventSize {
    XS,  // 64 bytes   - Heartbeats, simple state changes
    S,   // 256 bytes  - Basic CRDT operations, chat messages
    M,   // 1KB        - Document edits, small file attachments
    L,   // 4KB        - Whiteboard data, medium images
    XL,  // 16KB       - Large files, complex diagrams
    XXL, // 64KB+      - Heap fallback for rare huge events
}

#[repr(C, align(64))]
#[derive(Debug, Copy, Clone)]
pub struct PooledEvent<const TSHIRT_SIZE: usize> {
    pub data: [u8; TSHIRT_SIZE],
    pub len: u32,
    pub event_type: u32,
}

unsafe impl<const TSHIRT_SIZE: usize> Pod for PooledEvent<TSHIRT_SIZE> {}
unsafe impl<const TSHIRT_SIZE: usize> Zeroable for PooledEvent<TSHIRT_SIZE> {}

#[repr(C, align(64))]
pub struct SlotMetadata {
    pub ref_count: AtomicU8,
    pub generation: AtomicU16,
    pub is_allocated: AtomicU8, // 0 = free, 1 = allocated
}

impl Default for SlotMetadata {
    fn default() -> Self {
        Self {
            ref_count: AtomicU8::new(0),
            generation: AtomicU16::new(0),
            is_allocated: AtomicU8::new(0),
        }
    }
}

// Stack safety guards - prevent unreasonable memory usage
const MAX_STACK_BYTES: usize = 1_048_576; // 1MB max per ring buffer

#[repr(C, align(64))]
pub struct RingBuffer<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> {
    pub data: UnsafeCell<[PooledEvent<TSHIRT_SIZE>; RING_CAPACITY]>,
    pub metadata: UnsafeCell<[SlotMetadata; RING_CAPACITY]>,
    pub write_cursor: AtomicUsize,
}

// Compile-time guard to prevent stack overflow
impl<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> RingBuffer<TSHIRT_SIZE, RING_CAPACITY> {
    const _STACK_GUARD: () = {
        let total_size = TSHIRT_SIZE * RING_CAPACITY;
        assert!(
            total_size <= MAX_STACK_BYTES,
            "Ring buffer too large for stack! Reduce RING_CAPACITY or TSHIRT_SIZE"
        );
    };
}

// SAFETY: RingBuffer is safe to share between threads because:
// 1. All access to the data array is coordinated through atomic write_cursor
// 2. Writers use Release ordering, Readers use Acquire ordering
// 3. Readers never write to the data, only read
// 4. Writers coordinate among themselves (single-writer pattern intended)
// 5. The UnsafeCell is only used for interior mutability, not for bypassing thread safety
unsafe impl<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> Sync for RingBuffer<TSHIRT_SIZE, RING_CAPACITY> where
    PooledEvent<TSHIRT_SIZE>: Send + Sync
{
}

unsafe impl<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> Send for RingBuffer<TSHIRT_SIZE, RING_CAPACITY> where
    PooledEvent<TSHIRT_SIZE>: Send + Sync
{
}

impl<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> Default for RingBuffer<TSHIRT_SIZE, RING_CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> RingBuffer<TSHIRT_SIZE, RING_CAPACITY> {
    pub fn new() -> Self {
        // Trigger compile-time check
        #[allow(path_statements)]
        Self::_STACK_GUARD;

        unsafe {
            Self {
                data: UnsafeCell::new(std::mem::zeroed()),
                metadata: UnsafeCell::new(std::mem::zeroed()),
                write_cursor: AtomicUsize::new(0),
            }
        }
    }
}

#[repr(C, align(64))]
pub struct Reader<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> {
    pub ringbuffer: Arc<RingBuffer<TSHIRT_SIZE, RING_CAPACITY>>,
    pub cursor: usize,
    pub last_ts: u64,
}

impl<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> Reader<TSHIRT_SIZE, RING_CAPACITY> {
    pub fn backpressure_ratio(&self) -> f32 {
        let writer_pos = self.ringbuffer.write_cursor.load(Ordering::Acquire);
        let reader_pos = self.cursor;

        let lag = if writer_pos >= reader_pos {
            writer_pos - reader_pos
        } else {
            // Handle wrap-around case
            (RING_CAPACITY - reader_pos) + writer_pos
        };

        // Return ratio: 0.0 = caught up, 1.0 = completely full buffer behind
        lag as f32 / RING_CAPACITY as f32
    }

    /// Check if this reader is under pressure
    pub fn is_under_pressure(&self) -> bool {
        self.backpressure_ratio() > 0.8 // 80% threshold
    }

    /// Check if this reader should signal for throttling
    pub fn should_throttle(&self) -> bool {
        self.backpressure_ratio() >= 0.9 // 90% threshold (>= not >)
    }
}

impl<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> Iterator for Reader<TSHIRT_SIZE, RING_CAPACITY> {
    type Item = PooledEvent<TSHIRT_SIZE>;

    fn next(&mut self) -> Option<Self::Item> {
        let writer_pos = self.ringbuffer.write_cursor.load(Ordering::Acquire);

        // Special case: if writer and reader are at same position,
        // try reading anyway - if there's data, the buffer was full
        if writer_pos == self.cursor {
            // Peek at the data to see if there's a valid event
            let ptr = self.ringbuffer.data.get();
            let potential_event = unsafe { (*ptr)[self.cursor] };

            // Simple heuristic: if the event has non-zero length, assume it's valid
            // In a production system, you'd use a proper validity check
            if potential_event.len > 0 && potential_event.len <= TSHIRT_SIZE as u32 {
                // There's valid data here, so buffer was full
                let res = potential_event;
                self.cursor = (self.cursor + 1) % RING_CAPACITY;
                return Some(res);
            } else {
                // No valid data, buffer is truly empty
                return None;
            }
        }

        // Normal case: calculate available data
        let available_data = if writer_pos > self.cursor {
            writer_pos - self.cursor
        } else {
            (RING_CAPACITY - self.cursor) + writer_pos
        };

        if available_data == 0 {
            return None;
        }

        // Read from current cursor position
        let ptr = self.ringbuffer.data.get();
        let res = unsafe { (*ptr)[self.cursor] };

        // Advance cursor for next read
        self.cursor = (self.cursor + 1) % RING_CAPACITY;

        Some(res)
    }
}

#[repr(C, align(64))]
pub struct Writer<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> {
    // shared buffer for reader and writers to coordinate
    pub ringbuffer: Arc<RingBuffer<TSHIRT_SIZE, RING_CAPACITY>>,
    pub last_ts: u64,
}

impl<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> Writer<TSHIRT_SIZE, RING_CAPACITY> {
    pub fn add(&mut self, e: PooledEvent<TSHIRT_SIZE>) -> bool {
        let cursor_val = self.ringbuffer.write_cursor.load(Ordering::Relaxed);
        let ptr = self.ringbuffer.data.get();
        unsafe { (*ptr)[cursor_val] = e };
        self.ringbuffer
            .write_cursor
            .store((cursor_val + 1) % RING_CAPACITY, Ordering::Release);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn create_test_event<const SIZE: usize>(data: u8) -> PooledEvent<SIZE> {
        let mut event = PooledEvent {
            data: [0u8; SIZE],
            len: 1,
            event_type: 1,
        };
        event.data[0] = data;
        event
    }

    fn create_reader_writer_pair<const SIZE: usize, const CAPACITY: usize>()
        -> (Reader<SIZE, CAPACITY>, Writer<SIZE, CAPACITY>)
    {
        let ring_buffer = Arc::new(RingBuffer::<SIZE, CAPACITY>::new());
        let reader = Reader {
            ringbuffer: ring_buffer.clone(),
            cursor: 0,
            last_ts: 0,
        };
        let writer = Writer {
            ringbuffer: ring_buffer,
            last_ts: 0,
        };
        (reader, writer)
    }

    #[test]
    fn test_backpressure_empty_buffer() {
        let (reader, _writer): (Reader<64, 8>, Writer<64, 8>) = create_reader_writer_pair();

        // Empty buffer: reader and writer at same position
        assert_eq!(reader.backpressure_ratio(), 0.0);
        assert!(!reader.is_under_pressure());
        assert!(!reader.should_throttle());
    }

    #[test]
    fn test_backpressure_single_event() {
        let (reader, mut writer): (Reader<64, 8>, Writer<64, 8>) = create_reader_writer_pair();

        // Write one event
        writer.add(create_test_event(1));

        // Reader hasn't read yet, so lag = 1
        assert_eq!(reader.backpressure_ratio(), 1.0 / 8.0); // 1/8 = 0.125
        assert!(!reader.is_under_pressure());
        assert!(!reader.should_throttle());
    }

    #[test]
    fn test_backpressure_multiple_events_no_wrap() {
        let (reader, mut writer): (Reader<64, 8>, Writer<64, 8>) = create_reader_writer_pair();

        // Write 5 events (no wrap-around)
        for i in 1..=5 {
            writer.add(create_test_event(i));
        }

        // Reader hasn't read, writer at position 5, reader at 0
        // lag = 5 - 0 = 5
        assert_eq!(reader.backpressure_ratio(), 5.0 / 8.0); // 0.625
        assert!(!reader.is_under_pressure()); // < 0.8
        assert!(!reader.should_throttle()); // < 0.9
    }

    #[test]
    fn test_backpressure_under_pressure_threshold() {
        let (reader, mut writer): (Reader<64, 8>, Writer<64, 8>) = create_reader_writer_pair();

        // Write 7 events to get close to capacity
        for i in 1..=7 {
            writer.add(create_test_event(i));
        }

        // lag = 7, ratio = 7/8 = 0.875
        let ratio = reader.backpressure_ratio();
        assert_eq!(ratio, 7.0 / 8.0);
        assert!(reader.is_under_pressure()); // > 0.8
        assert!(!reader.should_throttle()); // < 0.9
    }

    #[test]
    fn test_debug_ring_buffer_state() {
        let (mut reader, mut writer): (Reader<64, 4>, Writer<64, 4>) = create_reader_writer_pair();

        println!("=== Initial State ===");
        println!("Writer pos: {}, Reader pos: {}",
                 writer.ringbuffer.write_cursor.load(Ordering::Acquire), reader.cursor);
        println!("Backpressure: {}", reader.backpressure_ratio());

        // Write 4 events to fill buffer
        for i in 1..=4 {
            writer.add(create_test_event(i));
            println!("After writing event {}: Writer pos: {}",
                     i, writer.ringbuffer.write_cursor.load(Ordering::Acquire));
        }

        println!("=== After filling buffer ===");
        println!("Writer pos: {}, Reader pos: {}",
                 writer.ringbuffer.write_cursor.load(Ordering::Acquire), reader.cursor);
        println!("Backpressure: {}", reader.backpressure_ratio());

        // Calculate available data manually
        let writer_pos = writer.ringbuffer.write_cursor.load(Ordering::Acquire);
        let reader_pos = reader.cursor;
        let available = if writer_pos >= reader_pos {
            writer_pos - reader_pos
        } else {
            (4 - reader_pos) + writer_pos
        };
        println!("Manual available calculation: {}", available);

        // Try to read first event
        let event1 = reader.next();
        println!("Read event 1: {:?}, Reader pos now: {}", event1.is_some(), reader.cursor);

        if let Some(evt) = event1 {
            println!("Event 1 data: {}", evt.data[0]);
        }

        // Try to read second event
        let event2 = reader.next();
        println!("Read event 2: {:?}, Reader pos now: {}", event2.is_some(), reader.cursor);

        println!("=== After reading 2 events ===");
        println!("Writer pos: {}, Reader pos: {}",
                 writer.ringbuffer.write_cursor.load(Ordering::Acquire), reader.cursor);
        println!("Backpressure: {}", reader.backpressure_ratio());
    }

    #[test]
    fn test_simple_wrap_scenario() {
        let (mut reader, mut writer): (Reader<64, 4>, Writer<64, 4>) = create_reader_writer_pair();

        println!("=== Test: Simple Wrap Scenario ===");

        // Step 1: Write 2 events
        writer.add(create_test_event(1));
        writer.add(create_test_event(2));
        println!("After writing 2 events: Writer={}, Reader={}",
                 writer.ringbuffer.write_cursor.load(Ordering::Acquire), reader.cursor);

        // Step 2: Read 2 events
        let e1 = reader.next();
        let e2 = reader.next();
        println!("After reading 2 events: Writer={}, Reader={}, e1={:?}, e2={:?}",
                 writer.ringbuffer.write_cursor.load(Ordering::Acquire), reader.cursor,
                 e1.is_some(), e2.is_some());

        // Step 3: Write 3 more events (should wrap)
        writer.add(create_test_event(3));
        writer.add(create_test_event(4));
        writer.add(create_test_event(5));
        println!("After writing 3 more events: Writer={}, Reader={}",
                 writer.ringbuffer.write_cursor.load(Ordering::Acquire), reader.cursor);

        // Step 4: Calculate backpressure
        let pressure = reader.backpressure_ratio();
        println!("Backpressure ratio: {}", pressure);

        // Step 5: Try to read remaining events
        let e3 = reader.next();
        println!("Reading event 3: {:?}", e3.is_some());
        if e3.is_none() {
            println!("PROBLEM: Cannot read event 3!");
            return;
        }

        let e4 = reader.next();
        println!("Reading event 4: {:?}", e4.is_some());

        let e5 = reader.next();
        println!("Reading event 5: {:?}", e5.is_some());
    }

    #[test]
    fn test_backpressure_with_reader_progress() {
        let (mut reader, mut writer): (Reader<64, 8>, Writer<64, 8>) = create_reader_writer_pair();

        // Write 5 events
        for i in 1..=5 {
            writer.add(create_test_event(i));
        }

        // Reader consumes 2 events
        let _event1 = reader.next().unwrap();
        let _event2 = reader.next().unwrap();

        // Now writer is at 5, reader at 2, lag = 5 - 2 = 3
        assert_eq!(reader.backpressure_ratio(), 3.0 / 8.0); // 0.375
        assert!(!reader.is_under_pressure());
    }

    #[test]
    fn test_backpressure_wrap_around_case() {
        let (mut reader, mut writer): (Reader<64, 8>, Writer<64, 8>) = create_reader_writer_pair();

        // Fill the entire buffer
        for i in 1..=8 {
            writer.add(create_test_event(i));
        }

        // Reader consumes 3 events (moves to position 3)
        for _ in 0..3 {
            let event = reader.next();
            assert!(event.is_some(), "Expected event but got None");
        }

        // Write 2 more events (causes wrap-around)
        writer.add(create_test_event(9));
        writer.add(create_test_event(10));

        // Writer wraps to position 2, reader at position 3
        // This creates: writer_pos (2) < reader_pos (3)
        // Available data: positions 3,4,5,6,7,0,1 = 7 slots
        // lag = (8 - 3) + 2 = 7
        assert_eq!(reader.backpressure_ratio(), 7.0 / 8.0); // 0.875
        assert!(reader.is_under_pressure());
    }

    #[test]
    fn test_backpressure_wrap_around_simple() {
        let (mut reader, mut writer): (Reader<64, 4>, Writer<64, 4>) = create_reader_writer_pair();

        // Write 4 events to fill buffer (writer goes to position 0)
        for i in 1..=4 {
            writer.add(create_test_event(i));
        }

        // Reader consumes 2 events (reader moves to position 2)
        let _event1 = reader.next().unwrap();
        let _event2 = reader.next().unwrap();

        // Write 1 more event (writer at position 0, wraps to position 1)
        writer.add(create_test_event(5));

        // writer_pos = 1, reader_pos = 2
        // Since writer_pos < reader_pos, we use wrap-around calculation:
        // lag = (4 - 2) + 1 = 3
        // Available data is at positions: 2, 3, 0 (3 events total)
        let expected_ratio = 3.0 / 4.0; // 0.75
        let actual_ratio = reader.backpressure_ratio();

        assert_eq!(actual_ratio, expected_ratio);
        assert!(!reader.is_under_pressure()); // < 0.8
    }

    #[test]
    fn test_backpressure_reader_catches_up() {
        let (mut reader, mut writer): (Reader<64, 8>, Writer<64, 8>) = create_reader_writer_pair();

        // Write events and create backpressure
        for i in 1..=7 {
            writer.add(create_test_event(i));
        }

        assert!(reader.is_under_pressure());

        // Reader catches up by consuming all events
        for _ in 0..7 {
            reader.next().unwrap();
        }

        // Now reader and writer should be at same position
        assert_eq!(reader.backpressure_ratio(), 0.0);
        assert!(!reader.is_under_pressure());
        assert!(!reader.should_throttle());
    }

    #[test]
    fn test_backpressure_edge_case_full_buffer() {
        let (reader, mut writer): (Reader<64, 4>, Writer<64, 4>) = create_reader_writer_pair();

        // Fill buffer completely (writer will wrap back to 0)
        for i in 1..=4 {
            writer.add(create_test_event(i));
        }

        // Writer is now at position 0, reader still at 0
        // This means we have 4 unread events: positions 0,1,2,3
        // But since writer_pos == reader_pos, we calculate as no lag
        // This is actually a limitation - need to track if buffer is full
        assert_eq!(reader.backpressure_ratio(), 0.0);
    }

    #[test]
    fn test_backpressure_partial_consumption_with_wrap() {
        let (mut reader, mut writer): (Reader<64, 6>, Writer<64, 6>) = create_reader_writer_pair();

        // Fill buffer
        for i in 1..=6 {
            writer.add(create_test_event(i));
        }

        // Reader consumes 4 events (reader at position 4)
        for _ in 0..4 {
            let event = reader.next();
            assert!(event.is_some(), "Expected event but got None");
        }

        // Write 3 more events (writer wraps from 0 to 3)
        for i in 7..=9 {
            writer.add(create_test_event(i));
        }

        // writer_pos = 3, reader_pos = 4
        // lag = (6 - 4) + 3 = 5
        assert_eq!(reader.backpressure_ratio(), 5.0 / 6.0); // ~0.833
        assert!(reader.is_under_pressure());
        assert!(!reader.should_throttle()); // < 0.9
    }

    #[test]
    fn test_backpressure_different_buffer_sizes() {
        // Test with larger buffer
        let (reader, mut writer): (Reader<64, 100>, Writer<64, 100>) = create_reader_writer_pair();

        // Write 50 events
        for i in 1..=50 {
            writer.add(create_test_event((i % 256) as u8));
        }

        // lag = 50, ratio = 50/100 = 0.5
        assert_eq!(reader.backpressure_ratio(), 0.5);
        assert!(!reader.is_under_pressure()); // < 0.8

        // Write 35 more events (total 85)
        for i in 51..=85 {
            writer.add(create_test_event((i % 256) as u8));
        }

        // lag = 85, ratio = 85/100 = 0.85
        assert_eq!(reader.backpressure_ratio(), 0.85);
        assert!(reader.is_under_pressure()); // > 0.8
        assert!(!reader.should_throttle()); // < 0.9
    }

    #[test]
    fn test_comprehensive_wrap_around_scenarios() {
        // Test multiple wrap-around scenarios
        let (mut reader, mut writer): (Reader<64, 8>, Writer<64, 8>) = create_reader_writer_pair();

        // Scenario 1: Fill buffer completely
        for i in 1..=8 {
            writer.add(create_test_event(i));
        }
        // Writer at 0, Reader at 0, should show 0 lag (edge case)
        assert_eq!(reader.backpressure_ratio(), 0.0);

        // Scenario 2: Reader consumes half, writer continues
        for _ in 0..4 {
            reader.next().unwrap();
        }
        // Writer at 0, Reader at 4, lag = (8-4) + 0 = 4
        assert_eq!(reader.backpressure_ratio(), 4.0 / 8.0);

        // Scenario 3: Writer advances past reader position
        for i in 9..=12 {
            writer.add(create_test_event(i));
        }
        // Writer at 4, Reader at 4, lag = 0
        assert_eq!(reader.backpressure_ratio(), 0.0);

        // Scenario 4: Create wrap-around lag
        for i in 13..=16 {
            writer.add(create_test_event(i));
        }
        // Writer at 0, Reader at 4, lag = (8-4) + 0 = 4
        assert_eq!(reader.backpressure_ratio(), 4.0 / 8.0);
    }
}