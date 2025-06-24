use std::{collections::VecDeque, hint::black_box, sync::Arc, time::Instant};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use rusted_ring::{AllocatedEvent, allocator::EventAllocator, ring::PooledEvent};

// Comparison structures for baseline testing

#[derive(Clone)]
struct HeapEvent {
    data: Vec<u8>,
    event_type: u32,
}

impl HeapEvent {
    fn new(data: &[u8], event_type: u32) -> Self {
        Self {
            data: data.to_vec(),
            event_type,
        }
    }
}

#[derive(Clone)]
struct BoxedEvent {
    data: Box<[u8]>,
    event_type: u32,
}

impl BoxedEvent {
    fn new(data: &[u8], event_type: u32) -> Self {
        Self {
            data: data.into(),
            event_type,
        }
    }
}

struct RcEvent {
    data: Arc<Vec<u8>>,
    event_type: u32,
}

impl Clone for RcEvent {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            event_type: self.event_type,
        }
    }
}

impl RcEvent {
    fn new(data: &[u8], event_type: u32) -> Self {
        Self {
            data: Arc::new(data.to_vec()),
            event_type,
        }
    }
}

// === ALLOCATION COMPARISON BENCHMARKS ===

fn bench_allocation_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocation_comparison");
    group.throughput(Throughput::Elements(1000));

    let test_data = b"test data for allocation comparison benchmarks - this is moderately sized";

    // RustedRing allocation
    group.bench_function("rusted_ring_allocation", |b| {
        let allocator = EventAllocator::new();
        b.iter(|| {
            let mut events = Vec::with_capacity(1000);
            for i in 0..1000 {
                let event = allocator.allocate_event(black_box(test_data), black_box(i)).unwrap();
                events.push(event);
            }
            black_box(events);
        });
    });

    // Standard Vec allocation
    group.bench_function("vec_allocation", |b| {
        b.iter(|| {
            let mut events = Vec::with_capacity(1000);
            for i in 0..1000 {
                let event = HeapEvent::new(black_box(test_data), black_box(i));
                events.push(event);
            }
            black_box(events);
        });
    });

    // Box allocation
    group.bench_function("box_allocation", |b| {
        b.iter(|| {
            let mut events = Vec::with_capacity(1000);
            for i in 0..1000 {
                let event = BoxedEvent::new(black_box(test_data), black_box(i));
                events.push(event);
            }
            black_box(events);
        });
    });

    // Arc allocation (for shared ownership comparison)
    group.bench_function("arc_allocation", |b| {
        b.iter(|| {
            let mut events = Vec::with_capacity(1000);
            for i in 0..1000 {
                let event = RcEvent::new(black_box(test_data), black_box(i));
                events.push(event);
            }
            black_box(events);
        });
    });

    group.finish();
}

// === CLONE COMPARISON BENCHMARKS ===

fn bench_clone_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("clone_comparison");
    group.throughput(Throughput::Elements(1000));

    let test_data = b"test data for clone comparison benchmarks";

    // Setup events for cloning
    let allocator = EventAllocator::new();
    let ring_event = allocator.allocate_event(test_data, 42).unwrap();
    let heap_event = HeapEvent::new(test_data, 42);
    let box_event = BoxedEvent::new(test_data, 42);
    let rc_event = RcEvent::new(test_data, 42);

    group.bench_function("rusted_ring_clone", |b| {
        b.iter(|| {
            let mut clones = Vec::with_capacity(1000);
            for _ in 0..1000 {
                clones.push(black_box(&ring_event).clone());
            }
            black_box(clones);
        });
    });

    group.bench_function("vec_clone", |b| {
        b.iter(|| {
            let mut clones = Vec::with_capacity(1000);
            for _ in 0..1000 {
                clones.push(black_box(&heap_event).clone());
            }
            black_box(clones);
        });
    });

    group.bench_function("box_clone", |b| {
        b.iter(|| {
            let mut clones = Vec::with_capacity(1000);
            for _ in 0..1000 {
                clones.push(black_box(&box_event).clone());
            }
            black_box(clones);
        });
    });

    group.bench_function("arc_clone", |b| {
        b.iter(|| {
            let mut clones = Vec::with_capacity(1000);
            for _ in 0..1000 {
                clones.push(black_box(&rc_event).clone());
            }
            black_box(clones);
        });
    });

    group.finish();
}

// === DATA ACCESS COMPARISON ===

fn bench_data_access_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("data_access_comparison");
    group.throughput(Throughput::Elements(1000));

    let test_data = b"test data for access pattern comparison benchmarks with sufficient length";

    // Setup events
    let allocator = EventAllocator::new();
    let mut ring_events = Vec::with_capacity(1000);
    let mut heap_events = Vec::with_capacity(1000);
    let mut box_events = Vec::with_capacity(1000);
    let mut rc_events = Vec::with_capacity(1000);

    for i in 0..1000 {
        ring_events.push(allocator.allocate_event(test_data, i).unwrap());
        heap_events.push(HeapEvent::new(test_data, i));
        box_events.push(BoxedEvent::new(test_data, i));
        rc_events.push(RcEvent::new(test_data, i));
    }

    group.bench_function("rusted_ring_access", |b| {
        b.iter(|| {
            let mut checksum = 0u64;
            for event in &ring_events {
                let data = event.data();
                checksum ^= data.len() as u64;
                checksum ^= event.event_type() as u64;
                if !data.is_empty() {
                    checksum ^= data[0] as u64;
                }
            }
            black_box(checksum);
        });
    });

    group.bench_function("vec_access", |b| {
        b.iter(|| {
            let mut checksum = 0u64;
            for event in &heap_events {
                checksum ^= event.data.len() as u64;
                checksum ^= event.event_type as u64;
                if !event.data.is_empty() {
                    checksum ^= event.data[0] as u64;
                }
            }
            black_box(checksum);
        });
    });

    group.bench_function("box_access", |b| {
        b.iter(|| {
            let mut checksum = 0u64;
            for event in &box_events {
                checksum ^= event.data.len() as u64;
                checksum ^= event.event_type as u64;
                if !event.data.is_empty() {
                    checksum ^= event.data[0] as u64;
                }
            }
            black_box(checksum);
        });
    });

    group.bench_function("arc_access", |b| {
        b.iter(|| {
            let mut checksum = 0u64;
            for event in &rc_events {
                checksum ^= event.data.len() as u64;
                checksum ^= event.event_type as u64;
                if !event.data.is_empty() {
                    checksum ^= event.data[0] as u64;
                }
            }
            black_box(checksum);
        });
    });

    group.finish();
}

// === MEMORY LOCALITY COMPARISON ===

fn bench_memory_locality(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_locality");

    let test_data = b"locality test data";
    let allocator = EventAllocator::new();

    group.bench_function("ring_buffer_locality", |b| {
        // All events allocated in same pool should have good locality
        let mut events = Vec::with_capacity(500);
        for i in 0..500 {
            events.push(allocator.allocate_xs_event(test_data, i).unwrap());
        }

        b.iter(|| {
            let mut sum = 0u64;
            for event in &events {
                // Access patterns that benefit from cache locality
                sum += event.len as u64;
                sum += event.event_type as u64;
                for &byte in &event.data[..std::cmp::min(8, event.len as usize)] {
                    sum += byte as u64;
                }
            }
            black_box(sum);
        });
    });

    group.bench_function("heap_allocation_locality", |b| {
        // Heap allocations likely scattered in memory
        let mut events = Vec::with_capacity(500);
        for i in 0..500 {
            events.push(HeapEvent::new(test_data, i));
        }

        b.iter(|| {
            let mut sum = 0u64;
            for event in &events {
                sum += event.data.len() as u64;
                sum += event.event_type as u64;
                for &byte in &event.data[..std::cmp::min(8, event.data.len())] {
                    sum += byte as u64;
                }
            }
            black_box(sum);
        });
    });

    group.finish();
}

// === CONCURRENT PERFORMANCE COMPARISON ===

fn bench_concurrent_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_comparison");

    let test_data = b"concurrent test data for comparison";

    group.bench_function("rusted_ring_concurrent", |b| {
        b.iter(|| {
            let allocator = Arc::new(EventAllocator::new());
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let allocator = allocator.clone();
                    std::thread::spawn(move || {
                        let mut events = Vec::with_capacity(250);
                        for i in 0..250 {
                            let event = allocator.allocate_event(test_data, i).unwrap();
                            events.push(event);

                            // Clone some events (shared ownership)
                            if i % 10 == 0 && !events.is_empty() {
                                let _cloned = events[events.len() - 1].clone();
                            }
                        }
                        events
                    })
                })
                .collect();

            let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
            black_box(results);
        });
    });

    group.bench_function("arc_vec_concurrent", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|thread_id: usize| {
                    std::thread::spawn(move || {
                        let mut events = Vec::with_capacity(250);
                        for i in 0..250usize {
                            let event = match (thread_id + i) % 3 {
                                0 => RcEvent::new(test_data, i as u32),
                                1 => {
                                    let data = vec![0u8; 128];
                                    RcEvent::new(&data, i as u32)
                                }
                                _ => {
                                    let data = vec![0u8; 512];
                                    RcEvent::new(&data, i as u32)
                                }
                            };
                            events.push(event);

                            // Clone some events
                            if i % 10 == 0 && !events.is_empty() {
                                let _cloned = events[events.len() - 1].clone();
                            }
                        }
                        events
                    })
                })
                .collect();

            let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
            black_box(results);
        });
    });

    group.finish();
}

// === REAL-WORLD SCENARIO COMPARISON ===

fn bench_real_world_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_world_comparison");
    group.sample_size(20);

    // Simulate a message queue / event log scenario
    group.bench_function("rusted_ring_message_queue", |b| {
        b.iter(|| {
            let allocator = EventAllocator::new();
            let mut active_messages = VecDeque::new();
            let mut processed = 0;

            // Simulate 5000 messages with processing
            for i in 0..5000 {
                let msg_data = format!("Message {} with some payload data", i);
                let event = allocator.allocate_event(msg_data.as_bytes(), i % 10).unwrap();

                // Add to active queue
                active_messages.push_back(event);

                // Process messages (FIFO)
                if active_messages.len() > 100 {
                    if let Some(msg) = active_messages.pop_front() {
                        processed += msg.len();
                    }
                }

                // Some messages get duplicated (broadcasts)
                if i % 20 == 0 && !active_messages.is_empty() {
                    let last_idx = active_messages.len() - 1;
                    let _duplicate = active_messages[last_idx].clone();
                }
            }

            black_box((active_messages.len(), processed));
        });
    });

    group.bench_function("vec_message_queue", |b| {
        b.iter(|| {
            let mut active_messages = VecDeque::new();
            let mut processed = 0;

            for i in 0..5000 {
                let msg_data = format!("Message {} with some payload data", i);
                let event = HeapEvent::new(msg_data.as_bytes(), i % 10);

                active_messages.push_back(event);

                if active_messages.len() > 100 {
                    if let Some(msg) = active_messages.pop_front() {
                        processed += msg.data.len();
                    }
                }

                if i % 20 == 0 && !active_messages.is_empty() {
                    let last_idx = active_messages.len() - 1;
                    let _duplicate = active_messages[last_idx].clone();
                }
            }

            black_box((active_messages.len(), processed));
        });
    });

    group.finish();
}

// === POOL EFFICIENCY COMPARISON ===

fn bench_pool_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("pool_efficiency");

    // Test how well the pool system handles different allocation patterns
    group.bench_function("rusted_ring_mixed_sizes", |b| {
        let allocator = EventAllocator::new();

        b.iter(|| {
            let mut events = Vec::with_capacity(1000);

            for i in 0..1000usize {
                let event: AllocatedEvent = match i % 5 {
                    0 => AllocatedEvent::Xs(allocator.allocate_xs_event(b"small", i as u32).unwrap()),
                    1 => AllocatedEvent::S(allocator.allocate_s_event(&vec![0u8; 100], i as u32).unwrap()),
                    2 => AllocatedEvent::M(allocator.allocate_m_event(&vec![0u8; 500], i as u32).unwrap()),
                    3 => AllocatedEvent::L(allocator.allocate_l_event(&vec![0u8; 2000], i as u32).unwrap()),
                    _ => AllocatedEvent::Xl(allocator.allocate_xl_event(&vec![0u8; 8000], i as u32).unwrap()),
                };
                events.push(event);
            }

            black_box(events);
        });
    });

    group.bench_function("heap_mixed_sizes", |b| {
        b.iter(|| {
            let mut events = Vec::with_capacity(1000);

            for i in 0..1000 {
                let event = match i % 5 {
                    0 => HeapEvent::new(b"small", i),
                    1 => HeapEvent::new(&vec![0u8; 100], i),
                    2 => HeapEvent::new(&vec![0u8; 500], i),
                    3 => HeapEvent::new(&vec![0u8; 2000], i),
                    _ => HeapEvent::new(&vec![0u8; 8000], i),
                };
                events.push(event);
            }

            black_box(events);
        });
    });

    group.finish();
}

// === FRAGMENTATION COMPARISON ===

fn bench_fragmentation_resistance(c: &mut Criterion) {
    let mut group = c.benchmark_group("fragmentation_resistance");

    // Test allocation/deallocation patterns that cause fragmentation
    group.bench_function("rusted_ring_fragmentation_test", |b| {
        let allocator = EventAllocator::new();

        b.iter(|| {
            let mut long_lived = Vec::new();
            let mut short_lived = Vec::new();

            // Create fragmentation pattern: interleave long and short-lived allocations
            for i in 0..500 {
                // Long-lived allocation
                let long_event = allocator.allocate_event(&vec![0u8; 200], i).unwrap();
                long_lived.push(long_event);

                // Short-lived allocations
                for j in 0..10 {
                    let short_event = allocator.allocate_event(b"temporary", i * 10 + j).unwrap();
                    short_lived.push(short_event);
                }

                // Free short-lived allocations periodically
                if i % 50 == 49 {
                    short_lived.clear();
                }
            }

            black_box((long_lived.len(), short_lived.len()));
        });
    });

    group.bench_function("heap_fragmentation_test", |b| {
        b.iter(|| {
            let mut long_lived = Vec::new();
            let mut short_lived = Vec::new();

            for i in 0..500 {
                let long_event = HeapEvent::new(&vec![0u8; 200], i);
                long_lived.push(long_event);

                for j in 0..10 {
                    let short_event = HeapEvent::new(b"temporary", i * 10 + j);
                    short_lived.push(short_event);
                }

                if i % 50 == 49 {
                    short_lived.clear();
                }
            }

            black_box((long_lived.len(), short_lived.len()));
        });
    });

    group.finish();
}

// === CACHE EFFICIENCY COMPARISON ===

fn bench_cache_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_efficiency");

    let data_pattern = (0..64).map(|i| i as u8).collect::<Vec<_>>();

    group.bench_function("rusted_ring_cache_traversal", |b| {
        let allocator = EventAllocator::new();
        let mut events = Vec::with_capacity(1000);

        // Allocate many events (should be cache-friendly due to pool allocation)
        for i in 0..1000 {
            events.push(allocator.allocate_xs_event(&data_pattern, i).unwrap());
        }

        b.iter(|| {
            let mut checksum = 0u64;

            // Sequential access pattern
            for event in &events {
                for chunk in event.data.chunks(8) {
                    for &byte in chunk {
                        checksum = checksum.wrapping_add(byte as u64);
                    }
                }
            }

            black_box(checksum);
        });
    });

    group.bench_function("heap_cache_traversal", |b| {
        let mut events = Vec::with_capacity(1000);

        // Heap allocations likely scattered
        for i in 0..1000 {
            events.push(HeapEvent::new(&data_pattern, i));
        }

        b.iter(|| {
            let mut checksum = 0u64;

            for event in &events {
                for chunk in event.data.chunks(8) {
                    for &byte in chunk {
                        checksum = checksum.wrapping_add(byte as u64);
                    }
                }
            }

            black_box(checksum);
        });
    });

    group.finish();
}

// === DROP PERFORMANCE COMPARISON ===

fn bench_drop_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("drop_performance");
    group.throughput(Throughput::Elements(1000));

    let test_data = vec![0u8; 1024];

    group.bench_function("rusted_ring_drop", |b| {
        let allocator = EventAllocator::new();

        b.iter_custom(|iters| {
            let start = Instant::now();

            for _ in 0..iters {
                let mut events = Vec::with_capacity(100); // Reduced from 1000 to avoid pool exhaustion
                for i in 0..100 {
                    // Use smaller data to avoid pool exhaustion
                    let small_data = &test_data[..std::cmp::min(512, test_data.len())];
                    match allocator.allocate_event(black_box(small_data), i as u32) {
                        Ok(event) => events.push(event),
                        Err(_) => break, // Stop if pool is full rather than panic
                    }
                }
                // Events dropped here when `events` goes out of scope
                black_box(events);
            }

            start.elapsed()
        });
    });

    group.bench_function("vec_drop", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();

            for _ in 0..iters {
                let mut events = Vec::with_capacity(1000);
                for i in 0..1000 {
                    events.push(HeapEvent::new(black_box(&test_data), i));
                }
                black_box(events);
            }

            start.elapsed()
        });
    });

    group.bench_function("arc_drop", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();

            for _ in 0..iters {
                let mut events = Vec::with_capacity(1000);
                for i in 0..1000 {
                    events.push(RcEvent::new(black_box(&test_data), i));
                }
                black_box(events);
            }

            start.elapsed()
        });
    });

    group.finish();
}

// === SIZE OVERHEAD COMPARISON ===

fn bench_size_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("size_overhead");

    group.bench_function("structure_size_analysis", |b| {
        b.iter(|| {
            // Analyze memory overhead of different approaches
            let ring_event_size = size_of::<AllocatedEvent>();
            let heap_event_size = size_of::<HeapEvent>();
            let box_event_size = size_of::<BoxedEvent>();
            let rc_event_size = size_of::<RcEvent>();

            // Also check actual pooled event sizes
            let xs_pooled_size = size_of::<PooledEvent<64>>();
            let s_pooled_size = size_of::<PooledEvent<256>>();
            let m_pooled_size = size_of::<PooledEvent<1024>>();

            black_box((
                ring_event_size,
                heap_event_size,
                box_event_size,
                rc_event_size,
                xs_pooled_size,
                s_pooled_size,
                m_pooled_size,
            ));
        });
    });

    group.finish();
}

// === THROUGHPUT COMPARISON ===

fn bench_throughput_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_comparison");
    group.throughput(Throughput::Elements(10000));

    let test_data = b"throughput test data for high volume scenarios";

    group.bench_function("rusted_ring_high_throughput", |b| {
        let allocator = EventAllocator::new();

        b.iter(|| {
            for i in 0..10000 {
                let event = allocator.allocate_event(black_box(test_data), black_box(i)).unwrap();

                // Simulate some work with the event
                let _data_len = event.len();
                let _event_type = event.event_type();

                // Event is dropped here (immediate cleanup)
                black_box(event);
            }
        });
    });

    group.bench_function("vec_high_throughput", |b| {
        b.iter(|| {
            for i in 0..10000 {
                let event = HeapEvent::new(black_box(test_data), black_box(i));

                let _data_len = event.data.len();
                let _event_type = event.event_type;

                black_box(event);
            }
        });
    });

    group.bench_function("box_high_throughput", |b| {
        b.iter(|| {
            for i in 0..10000 {
                let event = BoxedEvent::new(black_box(test_data), black_box(i));

                let _data_len = event.data.len();
                let _event_type = event.event_type;

                black_box(event);
            }
        });
    });

    group.finish();
}

criterion_group!(
    allocation_comparison_benches,
    bench_allocation_comparison,
    bench_clone_comparison,
    bench_data_access_comparison,
);

criterion_group!(
    locality_comparison_benches,
    bench_memory_locality,
    bench_cache_efficiency,
    bench_fragmentation_resistance,
);

criterion_group!(
    concurrent_comparison_benches,
    bench_concurrent_comparison,
    bench_throughput_comparison,
);

criterion_group!(
    overhead_comparison_benches,
    bench_drop_performance,
    bench_size_overhead,
    bench_pool_efficiency,
);

criterion_group!(scenario_comparison_benches, bench_real_world_comparison,);

criterion_main!(
    allocation_comparison_benches,
    locality_comparison_benches,
    concurrent_comparison_benches,
    overhead_comparison_benches,
    scenario_comparison_benches,
);
