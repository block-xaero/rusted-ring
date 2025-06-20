# rusted-ring

A high-performance, cache-optimized ring buffer library for Rust, designed for lock-free, zero-copy event processing with dual patterns for different use cases: smart pointer semantics for multi-actor sharing and sequential processing for high-throughput pipelines.

## Features

- **Cache-line aligned** ring buffers for optimal CPU cache performance
- **Lock-free** operations using atomic memory ordering
- **T-shirt sized pools** for different event categories (XS, S, M, L, XL)
- **Zero-copy** operations with Pod/Zeroable support
- **Dual processing patterns**: RingPtr for sharing, Reader/Writer for sequential throughput
- **Reference counting** for safe slot reuse across multiple consumers
- **Mobile optimized** for ARM and x86 architectures with device-aware pool sizing
- **LMAX-inspired** sequential processing for ultra-low latency

## Core Architecture: Two Complementary Patterns

### Pattern A: RingPtr<T> - Multi-Actor Event Sharing

For scenarios where events need to be shared across multiple actors with variable processing speeds and lifetimes:

```rust
use rusted_ring::{RingPtr, EventAllocator, PooledEvent};

// Allocate event in ring buffer
let allocator = EventAllocator::new();
let ring_ptr: RingPtr<PooledEvent<1024>> = allocator.allocate_m_event(data, event_type)?;

// Share across multiple actors (reference counting)
let shared_ptr1 = ring_ptr.clone();  // Database writer
let shared_ptr2 = ring_ptr.clone();  // Search indexer  
let shared_ptr3 = ring_ptr.clone();  // Analytics processor
let shared_ptr4 = ring_ptr;          // Audit logger

// Send to different actors
database_tx.send(shared_ptr1)?;
indexing_tx.send(shared_ptr2)?;
analytics_tx.send(shared_ptr3)?;
audit_tx.send(shared_ptr4)?;

// Each actor processes independently, slot freed when all finish
```

**Ideal for:**
- Multi-actor systems with fan-out processing
- Variable processing speeds across consumers
- Cross-system boundaries (FFI, networking)
- Event persistence and replication
- Complex routing and error handling
- Systems requiring reference counting semantics

### Pattern B: Reader/Writer - Sequential High-Throughput

For scenarios requiring maximum throughput with predictable, sequential processing:

```rust
use rusted_ring::{RingBuffer, Reader, Writer, PooledEvent};
use std::sync::Arc;

// Create ring buffer for high-throughput pipeline
let ring = Arc::new(RingBuffer::<1024, 8192>::new());
let mut writer = Writer::new(ring.clone());
let mut reader = Reader::new(ring);

// Producer thread: Ultra-fast sequential writing
std::thread::spawn(move || {
    loop {
        // Write events without allocation overhead
        let event = PooledEvent::<1024>::new(event_data, event_type);
        writer.add(event);
    }
});

// Consumer thread: Sequential processing pipeline
std::thread::spawn(move || {
    while let Some(event) = reader.try_read() {
        // Process in strict order for maximum throughput
        process_pipeline_stage_1(event);
        process_pipeline_stage_2(event);
        process_pipeline_stage_3(event);
    }
});
```

**Ideal for:**
- Sequential operator pipelines
- Batch processing workflows (sort → fold → reduce)
- High-frequency data streaming
- Real-time systems with strict ordering requirements
- Single-producer, single-consumer scenarios
- Maximum throughput applications

## Core Types

### PooledEvent<const TSHIRT_SIZE: usize>

Fixed-size event structure optimized for zero-copy operations:

```rust
#[repr(C, align(64))]
#[derive(Debug, Copy, Clone)]
pub struct PooledEvent<const TSHIRT_SIZE: usize> {
    pub data: [u8; TSHIRT_SIZE],
    pub len: u32,
    pub event_type: u32,
}

// Example: Custom event type conversion
impl<const SIZE: usize> From<PooledEvent<SIZE>> for MyCustomEvent {
    fn from(value: PooledEvent<SIZE>) -> Self {
        // Use bytemuck for zero-copy deserialization
        *bytemuck::from_bytes::<MyCustomEvent>(&value.data[..size_of::<MyCustomEvent>()])
    }
}
```

### RingPtr<T> - Smart Pointer to Ring Buffer Slots

Acts like `Arc<T>` but points to ring buffer memory instead of heap:

```rust
use rusted_ring::{RingPtr, EventAllocator};

// Allocation
let ring_ptr: RingPtr<PooledEvent<256>> = allocator.allocate_s_event(data, event_type)?;

// Sharing (increments reference count)
let shared_ptr = ring_ptr.clone();

// Access (zero-copy deref to ring buffer slot)
let event_data = &ring_ptr.data;
let event_type = ring_ptr.event_type;

// Automatic cleanup when all RingPtrs drop
```

### Dual Ring Buffer Architecture

```rust
#[repr(C, align(64))]
pub struct RingBuffer<const SIZE: usize, const CAPACITY: usize> {
    pub(crate) data: UnsafeCell<[PooledEvent<SIZE>; CAPACITY]>,
    pub(crate) metadata: UnsafeCell<[SlotMetadata; CAPACITY]>,
    pub write_cursor: AtomicUsize,
}

#[repr(C, align(64))]
pub struct SlotMetadata {
    pub ref_count: AtomicU8,     // Multi-actor reference counting
    pub generation: AtomicU16,   // ABA protection for safe reuse  
    pub is_allocated: AtomicU8,  // Allocation state tracking
}
```

## T-Shirt Sizing for Event Categories

Pre-defined event sizes for optimal memory usage:

```rust
pub enum EventSize {
    XS,  // 64 bytes   - Heartbeats, simple state changes, control signals
    S,   // 256 bytes  - Chat messages, small data packets, user actions
    M,   // 1KB        - Document edits, API requests, structured data
    L,   // 4KB        - Images, complex objects, batch operations
    XL,  // 16KB       - Large files, documents, multimedia data
    XXL, // 64KB+      - Heap fallback for exceptionally large events
}

// Automatic size selection
let size = EventAllocator::estimate_size(payload.len());
match size {
    EventSize::XS => allocator.allocate_xs_event(payload, event_type)?,
    EventSize::S => allocator.allocate_s_event(payload, event_type)?,
    EventSize::M => allocator.allocate_m_event(payload, event_type)?,
    EventSize::L => allocator.allocate_l_event(payload, event_type)?,
    EventSize::XL => allocator.allocate_xl_event(payload, event_type)?,
    EventSize::XXL => fallback_to_heap(payload, event_type)?,
}
```

## Performance Characteristics by Pattern

### Multi-Actor Sharing (RingPtr)
- **Allocation**: ~10-50ns per event vs ~100-500ns malloc
- **Sharing**: ~1-3 CPU cycles per clone (atomic ref count)
- **Fan-out**: Zero-copy distribution to N actors
- **Memory**: Predictable pools, no heap fragmentation
- **Cleanup**: Automatic when all references drop

### Sequential Processing (Reader/Writer)
- **Throughput**: 10-50x faster than channel-based pipelines
- **Latency**: Microsecond-level event processing
- **Backpressure**: Natural ring buffer full detection
- **Cache**: Optimized sequential access patterns
- **Ordering**: Strict FIFO processing guarantees

## Usage Examples

### Example 1: Event Distribution System

```rust
use rusted_ring::{EventAllocator, RingPtr, PooledEvent};
use std::sync::Arc;

// Initialize global allocator
static ALLOCATOR: std::sync::LazyLock<EventAllocator> = 
    std::sync::LazyLock::new(|| EventAllocator::new());

// Event wrapper for your application
#[derive(Clone)]
pub struct ApplicationEvent {
    pub data: RingPtr<PooledEvent<1024>>,  // Ring buffer data
    pub metadata: Arc<EventMetadata>,       // Heap metadata when needed
    pub timestamp: u64,                     // Inline primitive
}

// Incoming event handler
fn handle_incoming_event(payload: &[u8], event_type: u8) -> Result<()> {
    // Allocate from ring buffer instead of heap
    let ring_ptr = ALLOCATOR.allocate_m_event(payload, event_type as u32)?;
    
    // Create application event
    let app_event = ApplicationEvent {
        data: ring_ptr,
        metadata: Arc::new(EventMetadata::new()),
        timestamp: current_timestamp(),
    };
    
    // Distribute to multiple processors
    persistence_tx.send(app_event.clone())?;  // Database writer
    search_tx.send(app_event.clone())?;       // Search indexer
    analytics_tx.send(app_event.clone())?;    // Analytics processor
    audit_tx.send(app_event)?;                // Audit logger
    
    Ok(())
}

// Actor processing
fn database_actor() {
    while let Ok(event) = persistence_rx.recv() {
        // Zero-copy access to ring buffer data
        let payload = &event.data.data[..event.data.len as usize];
        write_to_database(payload, event.timestamp);
        // Ring buffer slot reference count decremented when event drops
    }
}
```

### Example 2: High-Throughput Data Pipeline

```rust
use rusted_ring::{RingBuffer, Reader, Writer, PooledEvent};
use std::sync::Arc;

// Pipeline stages
fn create_processing_pipeline() -> Result<()> {
    // Stage 1: Input buffer
    let input_ring = Arc::new(RingBuffer::<512, 16384>::new());
    let mut input_writer = Writer::new(input_ring.clone());
    let mut input_reader = Reader::new(input_ring);
    
    // Stage 2: Processing buffer  
    let process_ring = Arc::new(RingBuffer::<1024, 8192>::new());
    let mut process_writer = Writer::new(process_ring.clone());
    let mut process_reader = Reader::new(process_ring);
    
    // Producer: Feed data into pipeline
    std::thread::spawn(move || {
        loop {
            let raw_data = receive_external_data();
            let event = PooledEvent::<512>::new(&raw_data, INPUT_EVENT_TYPE);
            input_writer.add(event);
        }
    });
    
    // Stage 1: Parse and validate
    std::thread::spawn(move || {
        while let Some(input_event) = input_reader.try_read() {
            if let Ok(parsed_data) = parse_and_validate(&input_event.data) {
                let processed_event = PooledEvent::<1024>::new(&parsed_data, PARSED_EVENT_TYPE);
                process_writer.add(processed_event);
            }
        }
    });
    
    // Stage 2: Final processing and output
    std::thread::spawn(move || {
        while let Some(process_event) = process_reader.try_read() {
            let result = final_processing(&process_event.data);
            emit_result(result);
        }
    });
    
    Ok(())
}
```

### Example 3: Batch Processing with Ring Buffers

```rust
use rusted_ring::{RingBuffer, Reader, Writer, PooledEvent};

// Efficient batch processing using sequential ring buffer access
fn process_data_batch(input_data: Vec<DataItem>) -> Result<ProcessedBatch> {
    // Create temporary ring buffer for batch processing
    let ring = RingBuffer::<256, 4096>::new();
    let mut writer = Writer::new(&ring);
    let mut reader = Reader::new(&ring);
    
    // Stage 1: Load data into ring buffer
    for item in input_data {
        let serialized = serialize_data_item(&item);
        let event = PooledEvent::<256>::new(&serialized, DATA_ITEM_TYPE);
        writer.add(event);
    }
    
    // Stage 2: Sequential processing (optimized for cache)
    let mut processed_items = Vec::new();
    while let Some(event) = reader.try_read() {
        let item = deserialize_data_item(&event.data);
        let processed = apply_transformation(item);
        processed_items.push(processed);
    }
    
    // Stage 3: Aggregate results
    let batch_result = processed_items.into_iter()
        .fold(ProcessedBatch::empty(), |acc, item| acc.merge(item));
    
    Ok(batch_result)
}
```

## Memory Requirements by Configuration

### Default Configuration (~2.5MB total)
```rust
XS: 64B × 2000   = 128KB  (+ 14KB metadata)
S:  256B × 1000  = 256KB  (+ 7KB metadata)  
M:  1KB × 300    = 307KB  (+ 2KB metadata)
L:  4KB × 60     = 245KB  (+ 420B metadata)
XL: 16KB × 15    = 245KB  (+ 105B metadata)
```

### Mobile Optimized (~600KB total)
```rust
XS: 64B × 500    = 32KB   (+ 3.5KB metadata)
S:  256B × 250   = 64KB   (+ 1.8KB metadata)  
M:  1KB × 100    = 100KB  (+ 700B metadata)
L:  4KB × 20     = 80KB   (+ 140B metadata)
XL: 16KB × 5     = 80KB   (+ 35B metadata)
```

### High-Throughput Server (~8MB total)
```rust
XS: 64B × 4000   = 256KB  (+ 28KB metadata)
S:  256B × 2000  = 512KB  (+ 14KB metadata)  
M:  1KB × 600    = 614KB  (+ 4.2KB metadata)
L:  4KB × 120    = 491KB  (+ 840B metadata)
XL: 16KB × 30    = 491KB  (+ 210B metadata)
```

## Device-Aware Initialization

```rust
use rusted_ring::{EventAllocator, DeviceConfig};

// Automatic device detection and optimization
pub fn initialize_allocator() -> EventAllocator {
    let config = match detect_device_class() {
        DeviceClass::Mobile => DeviceConfig::mobile_optimized(),
        DeviceClass::Desktop => DeviceConfig::default(),
        DeviceClass::Server => DeviceConfig::high_throughput(),
    };
    
    EventAllocator::new_with_config(config)
}

// Custom configuration
let custom_config = DeviceConfig {
    xs_capacity: 1000,
    s_capacity: 500,
    m_capacity: 200,
    l_capacity: 40,
    xl_capacity: 10,
};

let allocator = EventAllocator::new_with_config(custom_config);
```

## When to Use Each Pattern

### Use RingPtr<T> when:
- Events need to be shared across multiple actors
- Processing speeds vary significantly between consumers
- You need reference counting semantics
- Events cross system boundaries (FFI, networking)
- Complex routing and error handling is required
- Fan-out distribution patterns

### Use Reader/Writer when:
- Maximum throughput is the primary goal
- Processing follows strict sequential ordering
- Single-producer, single-consumer scenarios
- Batch processing workflows
- Real-time systems with predictable latency requirements
- Pipeline architectures with multiple stages

## Compile-time Safety

Built-in guards prevent stack overflow from oversized ring buffers:

```rust
const MAX_STACK_BYTES: usize = 1_048_576; // 1MB stack limit

// Compile-time size validation
const _STACK_GUARD: () = {
    let total_size = TSHIRT_SIZE * RING_CAPACITY;
    assert!(total_size <= MAX_STACK_BYTES, "Ring buffer too large for stack allocation!");
};
```

## Memory Ordering & Safety

Careful memory ordering ensures lock-free safety:

- **Writers**: Use `Release` ordering when updating cursors
- **Readers**: Use `Acquire` ordering when reading positions
- **Reference counting**: Uses `AcqRel` ordering for atomicity
- **Slot reuse**: Protected by generation numbers (ABA prevention)
- **Cache optimization**: All structures are 64-byte aligned

## License

MPL-2.0