use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use rusted_ring::pool::{EventPoolFactory, EventUtils};

// === REALISTIC XAEROFLUX USAGE BENCHMARKS - LMAX AWARE ===

/// Benchmark: FFI events coming into Subject (your real entry point)
fn bench_ffi_to_subject(c: &mut Criterion) {
    let mut group = c.benchmark_group("ffi_to_subject");
    group.throughput(Throughput::Elements(250));

    // Real FFI event types from your whiteboard
    let ffi_events = [
        // Mouse/touch events (bursty during drawing)
        "mouse:x=100,y=200,p=0.5,t=1234567890",
        "touch:id=1,x=150,y=250,p=0.8,t=1234567891",
        // Stroke events (continuous during drawing)
        "stroke_start:tool=pen,color=#FF0000,width=2,t=1234567892",
        "stroke_point:x=102,y=203,p=0.6,t=1234567893",
        "stroke_end:duration=150ms,points=25,t=1234567894",
        // Peer events (network bursts)
        "peer_event:user=alice,action=join,room=abc123,t=1234567895",
        "sync:vector_clock=[1,2,3],merkle=abc123def,t=1234567896",
    ];

    group.bench_function("ffi_burst_scenario", |b| {
        b.iter(|| {
            // Simulate FFI boundary: events arrive in bursts
            let mut subject_writer = EventPoolFactory::get_xs_writer();

            // Burst 1: Rapid drawing (mouse events) - REDUCED
            for i in 0..50 {
                // Reduced from 100
                let event_data = ffi_events[i % 2].as_bytes();
                let event = EventUtils::create_pooled_event::<64>(black_box(event_data), black_box(i as u32)).unwrap();

                // This is your actual bottleneck: FFI → Subject
                black_box(subject_writer.add(event));
            }

            // Burst 2: Stroke processing - REDUCED
            for i in 50..100 {
                // Reduced range
                let event_data = ffi_events[(i % 3) + 2].as_bytes();
                let event = EventUtils::create_pooled_event::<64>(black_box(event_data), black_box(i as u32)).unwrap();
                black_box(subject_writer.add(event));
            }

            // Burst 3: Network sync (larger events) - REDUCED
            for i in 100..150 {
                // Reduced from 200..250
                let event_data = ffi_events[(i % 2) + 5].as_bytes();
                if event_data.len() <= 64 {
                    let event =
                        EventUtils::create_pooled_event::<64>(black_box(event_data), black_box(i as u32)).unwrap();
                    black_box(subject_writer.add(event));
                }
            }
        });
    });

    group.finish();
}

/// Benchmark: Write-Only Performance (No Reader Issues)
fn bench_write_only_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_performance");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("xs_pool_allocation", |b| {
        b.iter(|| {
            let mut writer = EventPoolFactory::get_xs_writer();

            // Pure write performance test
            for i in 0..1000 {
                let data = format!("write_test_{}", i % 10); // Reuse data
                let event =
                    EventUtils::create_pooled_event::<64>(black_box(data.as_bytes()), black_box(i as u32)).unwrap();

                // Measure pure write speed
                black_box(writer.add(event));
            }
        });
    });

    group.bench_function("s_pool_allocation", |b| {
        b.iter(|| {
            let mut writer = EventPoolFactory::get_s_writer();

            for i in 0..1000 {
                let data = format!("s_write_test_larger_payload_{}", i % 10);
                let event =
                    EventUtils::create_pooled_event::<256>(black_box(data.as_bytes()), black_box(i as u32)).unwrap();

                black_box(writer.add(event));
            }
        });
    });

    group.finish();
}

/// Benchmark: Read Performance with Coordinated Reader
fn bench_coordinated_read_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_performance");
    group.throughput(Throughput::Elements(500));

    group.bench_function("coordinated_read_cycle", |b| {
        b.iter(|| {
            // Create coordinated writer/reader pair
            let mut writer = EventPoolFactory::get_xs_writer();
            let mut reader = EventPoolFactory::get_xs_reader();

            let events_count = 500;
            let base_event_type = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                % 1_000_000) as u32
                + 10_000_000;

            // Write phase
            for i in 0..events_count {
                let data = format!("read_test_{}", i % 5);
                let event = EventUtils::create_pooled_event::<64>(data.as_bytes(), base_event_type + i).unwrap();
                writer.add(event);
            }

            // Read phase - look for our events
            let mut found = 0;
            let mut total_reads = 0;
            let expected_min = base_event_type;
            let expected_max = base_event_type + events_count - 1;

            // Read with safety limit
            while total_reads < events_count * 2 && found < events_count {
                if let Some(event) = reader.next() {
                    total_reads += 1;
                    if event.event_type >= expected_min && event.event_type <= expected_max {
                        found += 1;
                    }
                } else {
                    break;
                }
            }

            black_box((found, total_reads));
        });
    });

    group.finish();
}

/// Benchmark: Multi-Pool Performance
fn bench_multi_pool_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_pool");
    group.throughput(Throughput::Elements(300));

    group.bench_function("size_progression", |b| {
        b.iter(|| {
            // Simulate real usage: events grow in size as they're processed
            let mut xs_writer = EventPoolFactory::get_xs_writer();
            let mut s_writer = EventPoolFactory::get_s_writer();
            let mut m_writer = EventPoolFactory::get_m_writer();

            // XS: Raw input events
            for i in 0..100 {
                let data = format!("input_{}", i);
                let event =
                    EventUtils::create_pooled_event::<64>(black_box(data.as_bytes()), black_box(i as u32)).unwrap();
                black_box(xs_writer.add(event));
            }

            // S: Processed events with metadata
            for i in 0..100 {
                let data = format!("processed:id={},metadata={{ts:{},user:alice}}", i, i * 1000);
                let event =
                    EventUtils::create_pooled_event::<256>(black_box(data.as_bytes()), black_box((i + 1000) as u32))
                        .unwrap();
                black_box(s_writer.add(event));
            }

            // M: Aggregated events
            for i in 0..100 {
                let data = format!(
                    "aggregated:batch_id={},events=[{},{},{}],summary={{count:3,avg_x:150,avg_y:200}}",
                    i,
                    i * 3,
                    i * 3 + 1,
                    i * 3 + 2
                );
                let event =
                    EventUtils::create_pooled_event::<1024>(black_box(data.as_bytes()), black_box((i + 2000) as u32))
                        .unwrap();
                black_box(m_writer.add(event));
            }
        });
    });

    group.finish();
}

/// Benchmark: Realistic Pipeline (LMAX-Aware)
fn bench_realistic_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_pipeline");
    group.throughput(Throughput::Elements(50));

    group.bench_function("controlled_pipeline", |b| {
        b.iter(|| {
            // Stage 1: Input events
            let mut input_writer = EventPoolFactory::get_xs_writer();

            let events_count = 50; // Small count to avoid overwrite issues
            let base_event_type = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                % 1_000_000) as u32
                + 20_000_000;

            // Write input events
            for i in 0..events_count {
                let event_data = match i % 4 {
                    0 => "mouse_move:x=100,y=200",
                    1 => "stroke_point:x=105,y=205",
                    2 => "text_input:Hello",
                    _ => "peer_sync:user=bob",
                };

                let event = EventUtils::create_pooled_event::<64>(
                    black_box(event_data.as_bytes()),
                    black_box(base_event_type + i),
                )
                .unwrap();
                input_writer.add(event);
            }

            // Stage 2: Processing
            let mut input_reader = EventPoolFactory::get_xs_reader();
            let mut process_writer = EventPoolFactory::get_s_writer();

            let mut processed_count = 0;
            let mut events_read = 0;
            let expected_min = base_event_type;
            let expected_max = base_event_type + events_count - 1;

            // Process events with safety limits
            while events_read < events_count * 2 && processed_count < events_count {
                if let Some(raw_event) = input_reader.next() {
                    events_read += 1;

                    // Only process our events
                    if raw_event.event_type >= expected_min && raw_event.event_type <= expected_max {
                        // Simulate processing
                        let event_str = std::str::from_utf8(&raw_event.data[..raw_event.len as usize]).unwrap_or("");

                        // Filter: Only process certain event types
                        if raw_event.event_type % 3 != 0 {
                            continue;
                        }

                        // Create processed event
                        let processed_data = format!(
                            "processed:seq={},data={},ts={}",
                            processed_count, event_str, raw_event.event_type
                        );

                        if let Ok(processed_event) = EventUtils::create_pooled_event::<256>(
                            processed_data.as_bytes(),
                            raw_event.event_type + 10000,
                        ) {
                            process_writer.add(processed_event);
                            processed_count += 1;
                        }
                    }
                } else {
                    break;
                }
            }

            black_box((processed_count, events_read));
        });
    });

    group.finish();
}

/// Benchmark: Backpressure Simulation
fn bench_backpressure_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("backpressure");
    group.throughput(Throughput::Elements(100));

    group.bench_function("fast_writer_slow_reader", |b| {
        b.iter(|| {
            let mut writer = EventPoolFactory::get_xs_writer();
            let reader = EventPoolFactory::get_xs_reader();

            // Fast writer: write many events
            for i in 0..100 {
                let data = format!("fast_event_{}", i);
                let event =
                    EventUtils::create_pooled_event::<64>(black_box(data.as_bytes()), black_box(i as u32)).unwrap();
                writer.add(event);
            }

            // Measure backpressure
            let backpressure_before = reader.backpressure_ratio();

            // Slow reader: only read a few events
            let mut reader = reader;
            let mut consumed = 0;
            for _ in 0..20 {
                // Only consume 20% of events
                if let Some(_event) = reader.next() {
                    consumed += 1;
                } else {
                    break;
                }
            }

            let backpressure_after = reader.backpressure_ratio();

            black_box((consumed, backpressure_before, backpressure_after));
        });
    });

    group.finish();
}

/// Benchmark: Memory Access Patterns
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_patterns");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("sequential_access", |b| {
        b.iter(|| {
            let mut writer = EventPoolFactory::get_xs_writer();

            // Sequential memory access pattern (LMAX strength)
            for i in 0..1000 {
                let data = b"sequential_data_pattern_test"; // Fixed size data
                let event = EventUtils::create_pooled_event::<64>(black_box(data), black_box(i as u32)).unwrap();

                black_box(writer.add(event));
            }
        });
    });

    group.bench_function("cache_aligned_writes", |b| {
        b.iter(|| {
            // Test cache-aligned 64-byte writes
            let mut writer = EventPoolFactory::get_xs_writer();

            for i in 0..1000 {
                // Create exactly 64-byte aligned event
                let data = format!("cache_test_{:0>40}", i); // Pad to consistent size
                let event =
                    EventUtils::create_pooled_event::<64>(black_box(data.as_bytes()), black_box(i as u32)).unwrap();

                black_box(writer.add(event));
            }
        });
    });

    group.finish();
}

criterion_group!(
    core_benches,
    bench_ffi_to_subject,
    bench_write_only_performance,
    bench_coordinated_read_performance,
);

criterion_group!(
    lmax_pipeline_benches,
    bench_multi_pool_performance,
    bench_realistic_pipeline,
    bench_backpressure_simulation,
);

criterion_group!(memory_benches, bench_memory_patterns,);

criterion_main!(core_benches, lmax_pipeline_benches, memory_benches);
