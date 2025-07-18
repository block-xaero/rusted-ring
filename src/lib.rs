pub mod pool;
pub mod ring;

pub use pool::*;
pub use ring::*;
pub const XS_CAPACITY: usize = 2000; // 64 * 2000 = 128KB
pub const S_CAPACITY: usize = 1000; // 256 * 1000 = 256KB
pub const M_CAPACITY: usize = 300; // 1024 * 300 = 307KB
pub const L_CAPACITY: usize = 60; // 4096 * 60 = 245KB
pub const XL_CAPACITY: usize = 15; // 16384 * 15 = 245KB

pub const XS_TSHIRT_SIZE : usize = 64;
pub const S_TSHIRT_SIZE : usize = 256;
pub const M_TSHIRT_SIZE : usize = 1024;
pub const L_TSHIRT_SIZE : usize = 4096;
pub const XL_TSHIRT_SIZE : usize = 16384;

/// A Ring Source is the emitter bound to ring buffer
/// Essentially has a writer that is used by underlying `user` - lets say an actor
/// This can be leveraged to write to source which then can be read by others.
/// RingBuffer is a shared entity, expected to be static,
/// However it is expected that we follow Single Writer and many readers principle,
/// Source emits events (See `PooledEvent<TSHIRT_SIZE>` for more details).
#[derive(Debug)]
pub struct RingSource<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> {
    pub writer: Writer<TSHIRT_SIZE, RING_CAPACITY>,
}
impl<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> RingSource<TSHIRT_SIZE, RING_CAPACITY> {
    pub fn new(out_buffer: &'static RingBuffer<TSHIRT_SIZE, RING_CAPACITY>) -> Self {
        RingSource {
            writer: Writer::new(out_buffer),
        }
    }
}

/// A Ring Sink is the just that: a Sink that is  bound to ring buffer
/// Why a reader? Sink carries an in_buffer which `outsider` writes to.
/// Reader then used by receiving actor to process messages.
#[derive(Debug, Clone)]
pub struct RingSink<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> {
    pub reader: Reader<TSHIRT_SIZE, RING_CAPACITY>,
}

impl<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> RingSink<TSHIRT_SIZE, RING_CAPACITY> {
    pub fn new(in_buffer: &'static RingBuffer<TSHIRT_SIZE, RING_CAPACITY>) -> Self {
        RingSink {
            reader: Reader::new(in_buffer),
        }
    }
}

/// A conduit meant as a channel replacement in a stack friendly way
/// This enables actors to communicate following - Single Write and Many reads principle.
/// Source and Sink both which are underlying entities of a RingPipe
/// are essentially mechanisms for external parties communicating with anyone
/// using a RingPipe to leverage reader and writers.
/// NOTE: the IN_BUFFER is ringbuffer that writer writes to and is read by entity
/// using the RingPipe using `RingSink`'s `Reader`.
/// Like wise Writer is used in a manner similar.
#[derive(Debug)]
pub struct RingPipe<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> {
    pub source: RingSource<TSHIRT_SIZE, RING_CAPACITY>,
    pub sink: RingSink<TSHIRT_SIZE, RING_CAPACITY>,
}

impl<const TSHIRT_SIZE: usize, const RING_CAPACITY: usize> RingPipe<TSHIRT_SIZE, RING_CAPACITY> {
    pub fn new(
        in_buffer: &'static RingBuffer<TSHIRT_SIZE, RING_CAPACITY>,
        out_buffer: &'static RingBuffer<TSHIRT_SIZE, RING_CAPACITY>,
    ) -> Self {
        RingPipe {
            source: RingSource::new(out_buffer),
            sink: RingSink::new(in_buffer),
        }
    }
}
