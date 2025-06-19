use std::{
    cell::UnsafeCell,
    sync::{
        Arc,
        atomic::{AtomicUsize, AtomicU16, AtomicU8, Ordering},
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
    pub is_allocated: AtomicU8,  // 0 = free, 1 = allocated
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
    pub(crate) data: UnsafeCell<[PooledEvent<TSHIRT_SIZE>; RING_CAPACITY]>,
    pub(crate) metadata: UnsafeCell<[SlotMetadata; RING_CAPACITY]>,
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

impl<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> Iterator for Reader<TSHIRT_SIZE, RING_CAPACITY> {
    type Item = PooledEvent<TSHIRT_SIZE>;

    fn next(&mut self) -> Option<Self::Item> {
        let writer_pos = self.ringbuffer.write_cursor.load(Ordering::Acquire);

        // Check if there's data to read at current cursor position
        if self.cursor >= writer_pos {
            // No new data available
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
        // we do this before we are sharing ringbuffer where writer slots into
        // but readers need to read as well.
        let ptr = self.ringbuffer.data.get();
        unsafe { (*ptr)[cursor_val] = e };
        self.ringbuffer
            .write_cursor
            .store((cursor_val + 1) % RING_CAPACITY, Ordering::Release);
        true
    }
}