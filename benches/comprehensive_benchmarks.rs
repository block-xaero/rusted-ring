use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use rusted_ring::pool::{EventPoolFactory, EventUtils};

// === REALISTIC XAEROFLUX USAGE BENCHMARKS ===

/// Benchmark: FFI events coming into Subject (your real entry point)
fn bench_ffi_to_subject(c: &mut Criterion) {
    let mut group = c.benchmark_group("ffi_to_subject");
    group.throughput(Throughput::Elements(1000));

    // Real FFI event types from your whiteboard
    let ffi_events = vec![
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

            // Burst 1: Rapid drawing (mouse events)
            for i in 0..100 {
                let event_data = ffi_events[i % 2].as_bytes();
                let event = EventUtils::create_pooled_event::<64>(
                    black_box(event_data),
                    black_box(i as u32)
                ).unwrap();

                // This is your actual bottleneck: FFI → Subject
                black_box(subject_writer.add(event));
            }

            // Burst 2: Stroke processing
            for i in 100..200 {
                let event_data = ffi_events[(i % 3) + 2].as_bytes();
                let event = EventUtils::create_pooled_event::<64>(
                    black_box(event_data),
                    black_box(i as u32)
                ).unwrap();
                black_box(subject_writer.add(event));
            }

            // Burst 3: Network sync (larger events)
            for i in 200..250 {
                let event_data = ffi_events[(i % 2) + 5].as_bytes();
                if event_data.len() <= 64 {
                    let event = EventUtils::create_pooled_event::<64>(
                        black_box(event_data),
                        black_box(i as u32)
                    ).unwrap();
                    black_box(subject_writer.add(event));
                }
            }
        });
    });

    group.finish();
}

/// Benchmark: Subject → Pipeline processing (your operators)
fn bench_subject_to_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("subject_to_pipeline");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("real_operator_chain", |b| {
        b.iter(|| {
            // Stage 1: Subject receives events (from FFI)
            let mut subject_writer = EventPoolFactory::get_xs_writer();
            let mut subject_reader = EventPoolFactory::get_xs_reader();

            // Stage 2: Pipeline processes (your actual operators)
            let mut pipeline_writer = EventPoolFactory::get_s_writer();

            // Emit realistic events to subject
            for i in 0..1000 {
                let event_data = match i % 4 {
                    0 => "mouse_move:x=100,y=200",
                    1 => "stroke_point:x=105,y=205",
                    2 => "text_input:Hello",
                    _ => "peer_sync:user=bob",
                };

                let event = EventUtils::create_pooled_event::<64>(
                    black_box(event_data.as_bytes()),
                    black_box(i as u32)
                ).unwrap();
                subject_writer.add(event);
            }

            // Pipeline: Apply your real operators (map/filter/buffer/reduce)
            let mut processed_count = 0;
            while let Some(raw_event) = subject_reader.next() {
                // Map: Add metadata (like your vector clocks, timestamps)
                let event_str = std::str::from_utf8(&raw_event.data[..raw_event.len as usize])
                    .unwrap_or("");

                // Filter: Only process certain event types
                if raw_event.event_type % 3 != 0 {
                    // Skip every 3rd event
                    continue;
                }

                // Buffer: Accumulate events (your batching logic)
                let buffered_data = format!(
                    "buffered:seq={},data={},meta={{vc:[1,2,3],ts:{}}}",
                    processed_count, event_str, raw_event.event_type
                );

                // Reduce: Create aggregated event (your CRDT operations)
                if let Ok(processed_event) = EventUtils::create_pooled_event::<256>(
                    buffered_data.as_bytes(),
                    raw_event.event_type + 1000
                ) {
                    pipeline_writer.add(processed_event);
                    processed_count += 1;
                }
            }

            black_box(processed_count);
        });
    });

    group.finish();
}

/// Benchmark: Pipeline → System Actors (your storage/network actors)
fn bench_pipeline_to_actors(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_to_actors");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("system_actors_consumption", |b| {
        b.iter(|| {
            // Setup: Pipeline has processed events
            let mut pipeline_writer = EventPoolFactory::get_s_writer();

            // Fill pipeline with processed events
            for i in 0..1000 {
                let processed_data = format!(
                    "processed_event:id={},type=stroke,data={{x:{},y:{},p:0.5}},vc:[{},2,3]",
                    i, 100 + i, 200 + i, i % 10
                );

                let event = EventUtils::create_pooled_event::<256>(
                    black_box(processed_data.as_bytes()),
                    black_box(i as u32)
                ).unwrap();
                pipeline_writer.add(event);
            }

            // System Actors: Read and process (your real bottleneck)
            let mut pipeline_reader = EventPoolFactory::get_s_reader();

            // Actor 1: Storage Actor (writes to disk/database)
            let mut storage_count = 0;
            while let Some(event) = pipeline_reader.next() {
                // Simulate storage processing
                let event_str = std::str::from_utf8(&event.data[..event.len as usize])
                    .unwrap_or("");

                // Your storage logic: serialize, compress, write
                black_box(event_str.len()); // Simulate serialization
                black_box(event.event_type); // Simulate indexing

                storage_count += 1;
            }

            black_box(storage_count);
        });
    });

    group.finish();
}

/// Benchmark: Peer handshake scenario (network bursts)
fn bench_peer_handshake(c: &mut Criterion) {
    let mut group = c.benchmark_group("peer_handshake");
    group.throughput(Throughput::Elements(100));

    group.bench_function("network_burst_processing", |b| {
        b.iter(|| {
            // Peer connects: burst of handshake events
            let mut subject_writer = EventPoolFactory::get_xs_writer();
            let mut subject_reader = EventPoolFactory::get_xs_reader();
            let mut pipeline_writer = EventPoolFactory::get_m_writer(); // Larger events

            // Phase 1: Handshake burst (real network scenario)
            let handshake_events = [
                "peer_connect:id=alice,version=1.0,caps=[sync,draw]",
                "auth_challenge:nonce=abc123,timestamp=1234567890",
                "auth_response:signature=def456,pubkey=ghi789",
                "sync_request:vector_clock=[0,0,0],room=room123",
                "sync_response:events=100,merkle_root=jkl012",
            ];

            // Burst: All handshake events arrive quickly
            for (i, &event_data) in handshake_events.iter().enumerate() {
                let event = EventUtils::create_pooled_event::<64>(
                    black_box(event_data.as_bytes()),
                    black_box(i as u32)
                ).unwrap();
                subject_writer.add(event);
            }

            // Phase 2: Process handshake events
            let mut handshake_processed = 0;
            while let Some(event) = subject_reader.next() {
                let _event_str = std::str::from_utf8(&event.data[..event.len as usize])
                    .unwrap_or("");

                // Your handshake processing: validate, update state, respond
                let response_data = format!(
                    "handshake_result:step={},peer=alice,status=ok,state={{connected:true,room:room123}}",
                    handshake_processed
                );

                if let Ok(response_event) = EventUtils::create_pooled_event::<1024>(
                    response_data.as_bytes(),
                    event.event_type + 2000
                ) {
                    pipeline_writer.add(response_event);
                    handshake_processed += 1;
                }
            }

            black_box(handshake_processed);
        });
    });

    group.finish();
}

/// Benchmark: Mixed workload (realistic usage pattern)
fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("realistic_xaeroflux_usage", |b| {
        b.iter(|| {
            // This simulates your real application load

            // Multiple event sources (FFI, network, timers)
            let mut xs_writer = EventPoolFactory::get_xs_writer(); // FFI events
            let mut xs_reader = EventPoolFactory::get_xs_reader();

            let mut s_writer = EventPoolFactory::get_s_writer(); // Processed events
            let mut s_reader = EventPoolFactory::get_s_reader();

            let mut m_writer = EventPoolFactory::get_m_writer(); // Aggregated events
            let mut m_reader = EventPoolFactory::get_m_reader();

            // Phase 1: Burst of user interaction (drawing)
            for i in 0..200 {
                let event_data = format!("draw:x={},y={},p=0.{}", 100 + i, 200 + i, i % 10);
                let event = EventUtils::create_pooled_event::<64>(
                    black_box(event_data.as_bytes()),
                    black_box(i as u32)
                ).unwrap();
                xs_writer.add(event);
            }

            // Phase 2: Process drawing events
            let mut processed = 0;
            while let Some(draw_event) = xs_reader.next() {
                if processed % 10 == 0 {
                    // Buffer every 10 drawing events
                    let stroke_data = format!(
                        "stroke:id={},points=10,bbox={{x:100,y:200,w:50,h:30}}",
                        processed / 10
                    );

                    if let Ok(stroke_event) = EventUtils::create_pooled_event::<256>(
                        stroke_data.as_bytes(),
                        draw_event.event_type + 1000
                    ) {
                        s_writer.add(stroke_event);
                    }
                }
                processed += 1;
            }

            // Phase 3: Network sync events
            for i in 0..50 {
                let sync_data = format!("peer_event:from=bob,data=stroke_{}", i);
                let event = EventUtils::create_pooled_event::<64>(
                    black_box(sync_data.as_bytes()),
                    black_box((i + 1000) as u32),
                ).unwrap();
                xs_writer.add(event);
            }

            // Phase 4: Aggregate and store
            let mut strokes_processed = 0;
            while let Some(stroke_event) = s_reader.next() {
                let document_data = format!(
                    "document_update:strokes={},peers=3,version={},merkle=abc123",
                    strokes_processed + 1,
                    strokes_processed
                );

                if let Ok(doc_event) = EventUtils::create_pooled_event::<1024>(
                    document_data.as_bytes(),
                    stroke_event.event_type + 2000
                ) {
                    m_writer.add(doc_event);
                    strokes_processed += 1;
                }
            }

            // Phase 5: System actors consume final events
            let mut stored = 0;
            while let Some(doc_event) = m_reader.next() {
                // Storage actor: serialize and persist
                black_box(&doc_event.data[..doc_event.len as usize]);
                stored += 1;
            }

            black_box((processed, strokes_processed, stored));
        });
    });

    group.finish();
}

/// Benchmark: Backpressure in real scenarios
fn bench_realistic_backpressure(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_backpressure");

    group.bench_function("fast_drawing_slow_storage", |b| {
        b.iter(|| {
            // Real scenario: User draws fast, storage/network is slow
            let mut xs_writer = EventPoolFactory::get_xs_writer();
            let mut xs_reader = EventPoolFactory::get_xs_reader();

            let mut events_emitted = 0;
            let mut events_processed = 0;
            let mut backpressure_triggered = 0;

            // Fast drawing: 1000 events
            for i in 0..1000 {
                let draw_data = format!("fast_draw:x={},y={}", i, i);
                if let Ok(event) = EventUtils::create_pooled_event::<64>(
                    draw_data.as_bytes(),
                    i as u32
                ) {
                    if xs_writer.add(event) {
                        events_emitted += 1;
                    } else {
                        backpressure_triggered += 1;
                    }
                }

                // Slow processing: only process every 3rd event
                if i % 3 == 0 {
                    if let Some(event) = xs_reader.next() {
                        // Simulate slow storage
                        black_box(&event.data[..event.len as usize]);
                        events_processed += 1;
                    }
                }

                // Check backpressure
                if xs_reader.is_under_pressure() {
                    backpressure_triggered += 1;
                }
            }

            black_box((events_emitted, events_processed, backpressure_triggered));
        });
    });

    group.finish();
}

criterion_group!(
    xaero_core_benches,
    bench_ffi_to_subject,
    bench_subject_to_pipeline,
    bench_pipeline_to_actors,
);

criterion_group!(
    xaero_scenario_benches,
    bench_peer_handshake,
    bench_mixed_workload,
    bench_realistic_backpressure,
);

criterion_main!(xaero_core_benches, xaero_scenario_benches);