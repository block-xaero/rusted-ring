# rusted-ring

A high-performance, cache-optimized ring buffer library for Rust, designed for lock-free, zero-copy event processing with smart pointer semantics for slot-based memory management.

## Features

- **Cache-line aligned** ring buffers for optimal CPU cache performance
- **Lock-free** operations using atomic memory ordering
- **T-shirt sized pools** for different event categories (XS, S, M, L, XL)
- **Zero-copy** operations with Pod/Zeroable support
- **Smart pointer semantics** with `RingPtr<T>` for automatic memory management
- **Reference counting** for safe slot reuse across multiple consumers
- **Mobile optimized** for ARM and x86 architectures
- **Single-writer, multi-reader** design inspired by LMAX Disruptor

## Core Architecture

### RingPtr<T> - Smart Pointer to Ring Buffer Slots

The key innovation is `RingPtr<T>`, which acts like `Arc<T>` but points to ring buffer memory instead of heap:

```rust
use rusted_ring::{RingPtr, EventAllocator, PoolId};

// Allocate event in ring buffer
let allocator = EventAllocator::new();
let ring_ptr: RingPtr<Event> = allocate_from_pool(&allocator, data, event_type);

// Clone for sharing (increments ref count in slot metadata)
let shared_ptr = ring_ptr.clone();

// Access data (zero-copy deref to ring buffer slot)
let event_data = ring_ptr.deref();

// When all RingPtrs drop, slot automatically becomes reusable
```

### Dual Memory Architecture

- **Ring Buffer Slots**: Store actual event data (`PooledEvent<SIZE>`)
- **Slot Metadata**: Store reference counting and generation info (`SlotMetadata`)

```rust
#[repr(C, align(64))]
pub struct RingBuffer<const SIZE: usize, const CAPACITY: usize> {
    pub(crate) data: UnsafeCell<[PooledEvent<SIZE>; CAPACITY]>,
    pub(crate) metadata: UnsafeCell<[SlotMetadata; CAPACITY]>,
    pub write_cursor: AtomicUsize,
}

#[repr(C, align(64))]
pub struct SlotMetadata {
    pub ref_count: AtomicU8,     // Thread-safe reference counting
    pub generation: AtomicU16,   // ABA protection for safe reuse
    pub is_allocated: AtomicU8,  // Allocation state
}
```

## Quick Start

### Basic Ring Buffer Operations

```rust
use rusted_ring::{RingBuffer, Reader, Writer, PooledEvent};
use std::sync::Arc;

// Create a ring buffer for 256-byte events with 1000 capacity
let ring = Arc::new(RingBuffer::<256, 1000>::new());

// Create writer and readers
let mut writer = Writer {
    ringbuffer: ring.clone(),
    last_ts: 0,
};

let mut reader = Reader {
    ringbuffer: ring.clone(),
    cursor: 0,
    last_ts: 0,
};

// Write events
let event = PooledEvent::<256>::zeroed();
writer.add(event);

// Read events
while let Some(event) = reader.next() {
    println!("Event type: {}", event.event_type);
}
```

### Smart Pointer Usage (Recommended)

```rust
use rusted_ring::{EventAllocator, RingPtr, PoolId};
use std::sync::LazyLock;

// Global allocator (typically in your application)
static ALLOCATOR: LazyLock<EventAllocator> = LazyLock::new(|| EventAllocator::new());

// Allocate event from appropriate pool
fn create_event(data: &[u8], event_type: u8) -> RingPtr<Event> {
    let allocator = &*ALLOCATOR;
    let size = EventAllocator::estimate_size(data.len());
    let pool_id = PoolId::from_size(size);
    
    // Allocate slot and get RingPtr
    let slot_index = allocate_to_pool(allocator, data, event_type);
    let generation = allocator.pools.get_generation(pool_id, slot_index);
    
    RingPtr::new(pool_id, slot_index, generation, allocator)
}

// Share across threads/actors
fn distribute_event(ring_ptr: RingPtr<Event>) {
    // Send to multiple consumers
    tx1.send(ring_ptr.clone()).unwrap(); // Consumer 1
    tx2.send(ring_ptr.clone()).unwrap(); // Consumer 2  
    tx3.send(ring_ptr.clone()).unwrap(); // Consumer 3
    // Slot freed automatically when all consumers finish
}
```

## Core Types

### PooledEvent<const TSHIRT_SIZE: usize>

Fixed-size event structure that implements Copy, Clone, Pod and Zeroable for zero-copy operations:

```rust
#[repr(C, align(64))]
#[derive(Debug, Copy, Clone)]
pub struct PooledEvent<const TSHIRT_SIZE: usize> {
    pub data: [u8; TSHIRT_SIZE],
    pub len: u32,
    pub event_type: u32,
}
```

### EventPools

Manage multiple ring buffers by size category with automatic size estimation:

```rust
use rusted_ring::{EventPools, EventSize, PoolId};

let pools = EventPools::new();

// Automatic size estimation
let size = EventAllocator::estimate_size(data.len());
let pool_id = PoolId::from_size(size);

// Access specific pools
pools.get_slot_data(pool_id, slot_index);
pools.inc_ref_count(pool_id, slot_index);
pools.dec_ref_count(pool_id, slot_index);
```

## T-Shirt Sizing

Pre-defined event sizes for optimal memory usage:

```rust
pub enum EventSize {
    XS,  // 64 bytes   - Heartbeats, simple state changes
    S,   // 256 bytes  - Basic CRDT operations, chat messages  
    M,   // 1KB        - Document edits, small file attachments
    L,   // 4KB        - Whiteboard data, medium images
    XL,  // 16KB       - Large files, complex diagrams
    XXL, // 64KB+      - Heap fallback for rare huge events
}

// Automatic size selection
let size = EventAllocator::estimate_size(payload.len());
let pool_id = PoolId::from_size(size);
```

## Reference Counting & Memory Management

Automatic slot lifecycle management with atomic reference counting:

```rust
// 1. Allocation
let ring_ptr = create_event(data, event_type); // ref_count = 1

// 2. Sharing
let ptr1 = ring_ptr.clone(); // ref_count = 2
let ptr2 = ring_ptr.clone(); // ref_count = 3

// 3. Distribution
send_to_consumer1(ptr1); 
send_to_consumer2(ptr2);
drop(ring_ptr); // ref_count = 2

// 4. Automatic cleanup
// When consumers finish: ref_count = 1, then 0
// Slot automatically marked reusable with new generation
```

## Memory Ordering & Safety

Careful memory ordering ensures data visibility without locks:

- **Writers**: Use `Release` ordering when updating cursors
- **Readers**: Use `Acquire` ordering when reading cursor positions
- **Reference counting**: Uses `AcqRel` ordering for atomicity
- **Slot reuse**: Protected by generation numbers (ABA prevention)

## Cache Optimization

All structures are cache-line aligned (64 bytes) to prevent false sharing:

```rust
#[repr(C, align(64))]
pub struct RingBuffer<...> { ... }

#[repr(C, align(64))]  
pub struct SlotMetadata { ... }

#[repr(C, align(64))]
pub struct PooledEvent<...> { ... }
```

## Performance Characteristics

- **Allocation**: ~10-50ns per event (pool allocation vs ~100-500ns malloc)
- **Reference counting**: ~1-3 CPU cycles (atomic increment/decrement)
- **Access latency**: Direct pointer dereference to ring buffer memory
- **Memory usage**: Predictable 2.5MB total across all pools
- **Cache performance**: Optimized for temporal locality in slot access

## Use Cases

Perfect for:

- **Event-driven architectures** with actor-based processing
- **Real-time systems** requiring predictable latency and memory usage
- **High-frequency event processing** (trading, gaming, IoT)
- **P2P systems** needing efficient local buffering
- **CRDT operations** and collaborative editing systems
- **Mobile applications** with strict memory constraints

## Example: Multi-Actor Event Processing

```rust
use std::sync::{Arc, LazyLock};
use rusted_ring::{EventAllocator, RingPtr};

// Global allocator
static ALLOCATOR: LazyLock<EventAllocator> = LazyLock::new(|| EventAllocator::new());

// Event wrapper for your application
pub struct XaeroEvent {
    pub evt: RingPtr<Event>,                    // Points to ring buffer
    pub vector_clock: Option<Arc<VectorClock>>, // Heap metadata
    pub author_id: Option<Arc<AuthorID>>,       // Heap metadata  
    pub latest_ts: u64,                         // Inline primitive
}

// FFI boundary - incoming events
#[no_mangle]
pub extern "C" fn handle_ffi_event(data: *const u8, len: usize, event_type: u8) {
    let data_slice = unsafe { std::slice::from_raw_parts(data, len) };
    
    // Allocate from ring buffer
    let ring_ptr = create_event(data_slice, event_type);
    
    // Create application event
    let xaero_event = Arc::new(XaeroEvent {
        evt: ring_ptr,
        vector_clock: Some(Arc::new(get_vector_clock())),
        author_id: Some(Arc::new(get_author())),
        latest_ts: current_timestamp(),
    });
    
    // Send to all actors
    mmr_actor_tx.send(xaero_event.clone()).unwrap();
    segment_actor_tx.send(xaero_event.clone()).unwrap();
    index_actor_tx.send(xaero_event.clone()).unwrap();
    // Ring buffer slot freed when all actors finish processing
}

// Actor processing
fn mmr_actor_main() {
    while let Ok(event) = rx.recv() {
        // Zero-copy access to ring buffer data
        let event_data = event.evt.deref();
        let payload = &event_data.data[..event_data.len as usize];
        
        // Process and emit new event
        let mmr_hash = calculate_mmr(payload);
        emit_system_event(MmrAppended { hash: mmr_hash });
        
        // event drops here, decrementing ring buffer slot ref count
    }
}

// Batch processing with automatic cleanup
fn fold_reduce_events(events: Vec<Arc<XaeroEvent>>) -> Result<Arc<XaeroEvent>> {
    // Extract ring buffer references  
    let input_slots: Vec<RingPtr<Event>> = events.iter()
        .map(|e| e.evt.clone()) // Clone increments ref counts
        .collect();
    
    // Process data from ring buffer slots
    let reduced_data = fold_operation(&input_slots);
    
    // Create result in new ring buffer slot
    let result_ring_ptr = create_event(&reduced_data, RESULT_EVENT_TYPE);
    let result_event = Arc::new(XaeroEvent {
        evt: result_ring_ptr,
        ..Default::default()
    });
    
    // Send result and release input slots
    send_result(result_event);
    drop(input_slots); // All input slots freed for reuse
    
    Ok(result_event)
}
```

## Memory Requirements

Approximate memory usage for default pool configurations:

- **XS Pool**: 64 × 2000 = 128KB (+ 14KB metadata)
- **S Pool**: 256 × 1000 = 256KB (+ 7KB metadata)
- **M Pool**: 1024 × 300 = 307KB (+ 2KB metadata)
- **L Pool**: 4096 × 60 = 245KB (+ 420 bytes metadata)
- **XL Pool**: 16384 × 15 = 245KB (+ 105 bytes metadata)

**Total**: ~2.5MB of predictable, pre-allocated memory

## Compile-time Safety

Built-in guards prevent stack overflow from oversized ring buffers:

```rust
const MAX_STACK_BYTES: usize = 1_048_576; // 1MB limit

// Compile-time check
const _STACK_GUARD: () = {
    let total_size = TSHIRT_SIZE * RING_CAPACITY;
    assert!(total_size <= MAX_STACK_BYTES, "Ring buffer too large for stack!");
};
```

## License

MPL-2.0