# rusted-ring

A high-performance, cache-optimized ring buffer library for Rust, designed for lock-free, multi-reader, single-writer scenarios with zero-allocation event processing.

## Features

- **Cache-line aligned** ring buffers for optimal CPU cache performance
- **Lock-free** operations using atomic memory ordering
- **T-shirt sized pools** for different event categories
- **Zero-copy** operations with Pod/Zeroable support
- **Mobile optimized** for ARM and x86 architectures
- **Single-writer, multi-reader** design inspired by LMAX Disruptor

## Quick Start

```rust
use rusted_ring::{RingBuffer, Reader, Writer, PooledEvent, EventPools};

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
    // Process event
    println!("Event type: {}", event.event_type);
}
```

## Core Types

### RingBuffer<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize>

The main ring buffer structure. Cache-aligned and optimized for high-frequency access.

```rust
let ring = RingBuffer::<1024, 500>::new(); // 1KB events, 500 capacity
```

### PooledEvent<const TSHIRT_SIZE: usize>

Fixed-size event structure that implements Pod and Zeroable for zero-copy operations.

```rust
#[repr(C, align(64))]
pub struct PooledEvent<const TSHIRT_SIZE: usize> {
    data: [u8; TSHIRT_SIZE],
    len: u32,
    event_type: u32,
}
```

### Writer<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize>

Single-writer interface for adding events to the ring buffer.

```rust
let mut writer = Writer {
    ringbuffer: ring.clone(),
    last_ts: 0,
};

// Add events with proper memory ordering
writer.add(pooled_event);
```

### Reader<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize>

Multi-reader interface implementing Iterator for consuming events.

```rust
let mut reader = Reader {
    ringbuffer: ring.clone(),
    cursor: 0,
    last_ts: 0,
};

// Iterate over available events
for event in reader {
    // Process event
}
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
```

## EventPools

Manage multiple ring buffers by size category:

```rust
use rusted_ring::EventPools;

let pools = EventPools::new();

// Get appropriately sized writers
let xs_writer = pools.get_xs_writer(); // 64-byte events
let s_writer = pools.get_s_writer();   // 256-byte events
let m_writer = pools.get_m_writer();   // 1KB events
let l_writer = pools.get_l_writer();   // 4KB events
let xl_writer = pools.get_xl_writer(); // 16KB events

// Get readers for each size
let xs_reader = pools.get_xs_reader();
let s_reader = pools.get_s_reader();
// ... etc
```

## Memory Ordering & Safety

The library uses careful memory ordering to ensure data visibility:

- **Writers**: Use `Release` ordering when updating cursors
- **Readers**: Use `Acquire` ordering when reading cursor positions
- **Data access**: Protected by cursor coordination

This ensures readers never see stale data while maintaining lock-free performance.

## Cache Optimization

All structures are cache-line aligned (64 bytes) to prevent false sharing:

```rust
#[repr(C, align(64))]
pub struct RingBuffer<...> { ... }

#[repr(C, align(64))]  
pub struct Reader<...> { ... }

#[repr(C, align(64))]
pub struct Writer<...> { ... }
```

## Performance Characteristics

- **Allocation**: Zero allocations after initialization
- **Write latency**: ~10-50ns per event
- **Read latency**: ~5-20ns per event
- **Memory usage**: Predictable and bounded
- **Cache performance**: Optimized for sequential access patterns

## Use Cases

Perfect for:

- **Real-time systems** requiring predictable latency
- **High-frequency event processing** (trading, gaming, IoT)
- **Mobile applications** with strict memory constraints
- **P2P systems** needing efficient local buffering
- **CRDT operations** and collaborative editing

## Example: Event Processing Pipeline

```rust
use std::sync::Arc;
use rusted_ring::{EventPools, PooledEvent};

// Initialize pools
let pools = Arc::new(EventPools::new());

// Producer thread
let pools_producer = pools.clone();
std::thread::spawn(move || {
    let mut writer = pools_producer.get_s_writer();
    
    loop {
        let mut event = PooledEvent::<256>::zeroed();
        // Fill event with data...
        event.event_type = 42;
        
        writer.add(event);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
});

// Consumer thread  
let pools_consumer = pools.clone();
std::thread::spawn(move || {
    let mut reader = pools_consumer.get_s_reader();
    
    while let Some(event) = reader.next() {
        println!("Processed event type: {}", event.event_type);
    }
});
```

## Memory Requirements

Approximate memory usage for default pool configurations:

- **XS Pool**: 64 × 2000 = 128KB
- **S Pool**: 256 × 1000 = 256KB
- **M Pool**: 1024 × 500 = 512KB
- **L Pool**: 4096 × 200 = 819KB
- **XL Pool**: 16384 × 50 = 819KB

**Total**: ~2.5MB of pre-allocated ring buffer memory

## License

MPL-2.0

```
