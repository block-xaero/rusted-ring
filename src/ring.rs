use std::{
    cell::UnsafeCell,
    sync::atomic::{Ordering, fence},
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
    pub len: u32,
    pub event_type: u32,
    pub data: [u8; TSHIRT_SIZE],
}

unsafe impl<const TSHIRT_SIZE: usize> Pod for PooledEvent<TSHIRT_SIZE> {}
unsafe impl<const TSHIRT_SIZE: usize> Zeroable for PooledEvent<TSHIRT_SIZE> {}

#[repr(C, align(64))]
pub struct SlotMetadata {
    pub ref_count: u8,
    pub is_allocated: u8, // 0 = free, 1 = allocated
    pub generation: u16,
}

impl Default for SlotMetadata {
    fn default() -> Self {
        Self {
            ref_count: 0u8,
            is_allocated: 0u8,
            generation: 0u16,
        }
    }
}

// Stack safety guards - prevent unreasonable memory usage
const MAX_STACK_BYTES: usize = 1_048_576; // 1MB max per ring buffer

#[repr(C, align(64))]
pub struct RingBuffer<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> {
    // Single Writer updates this
    pub published_sequence: UnsafeCell<usize>,
    pub metadata: UnsafeCell<[SlotMetadata; RING_CAPACITY]>,
    pub data: UnsafeCell<[PooledEvent<TSHIRT_SIZE>; RING_CAPACITY]>,
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
                published_sequence: UnsafeCell::new(0usize),
                data: UnsafeCell::new(std::mem::zeroed()),
                metadata: UnsafeCell::new(std::mem::zeroed()),
            }
        }
    }
}

#[repr(C, align(64))]
pub struct Reader<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> {
    pub cursor: usize,
    pub ringbuffer: &'static RingBuffer<TSHIRT_SIZE, RING_CAPACITY>,
}

impl<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> Reader<TSHIRT_SIZE, RING_CAPACITY> {
    pub fn new(ringbuffer: &'static RingBuffer<TSHIRT_SIZE, RING_CAPACITY>) -> Self {
        Self { cursor: 0, ringbuffer }
    }

    pub fn backpressure_ratio(&self) -> f32 {
        // no fence -- this may not be most accurate but certainly fast!
        let ps = self.ringbuffer.published_sequence.get();
        let write_cursor = unsafe { *ps };
        let reader_pos = self.cursor;
        let lag = write_cursor - reader_pos;
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
        // how far has writer written
        let published_sequence_ptr = self.ringbuffer.published_sequence.get();
        let published_sequence = unsafe { *published_sequence_ptr };
        // reader cannot be ahead of writer
        if self.cursor > published_sequence {
            panic!("read cursor ahead of writer!")
        } else if self.cursor == published_sequence {
            return None;
        }
        fence(Ordering::Acquire);
        // Calculate slot from cursor
        let slot = self.cursor % RING_CAPACITY;
        let current_buffer = self.ringbuffer.data.get();
        let event_read = unsafe { (*current_buffer)[slot] };
        self.cursor += 1;
        Some(event_read)
    }
}

#[repr(C, align(64))]
pub struct Writer<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> {
    pub ringbuffer: &'static RingBuffer<TSHIRT_SIZE, RING_CAPACITY>,
}

impl<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> Writer<TSHIRT_SIZE, RING_CAPACITY> {
    pub fn new(ringbuffer: &'static RingBuffer<TSHIRT_SIZE, RING_CAPACITY>) -> Self {
        Self { ringbuffer }
    }

    pub fn add(&mut self, e: PooledEvent<TSHIRT_SIZE>) -> bool {
        let published_sequence_ptr = self.ringbuffer.published_sequence.get();
        let current_sequence = unsafe { *published_sequence_ptr };
        let slot = current_sequence % RING_CAPACITY;
        let ptr = self.ringbuffer.data.get();
        unsafe { (*ptr)[slot] = e };
        fence(Ordering::Release);
        unsafe {
            *published_sequence_ptr = current_sequence + 1;
        }
        true
    }
}
