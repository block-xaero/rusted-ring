use crate::ring::RingBuffer;
use std::sync::Arc;

// Pool size constants - Adjusted to stay under 1MB per ring buffer
pub const XS_CAPACITY: usize = 2000;  // 64 * 2000 = 128KB ✅
pub const S_CAPACITY: usize = 1000;   // 256 * 1000 = 256KB ✅
pub const M_CAPACITY: usize = 300;    // 1024 * 300 = 307KB ✅ (reduced from 500)
pub const L_CAPACITY: usize = 60;     // 4096 * 60 = 245KB ✅ (reduced from 200)
pub const XL_CAPACITY: usize = 15;    // 16384 * 15 = 245KB ✅ (reduced from 50)

pub struct EventPools {
    pub xs_pool: Arc<RingBuffer<64, XS_CAPACITY>>,
    pub s_pool: Arc<RingBuffer<256, S_CAPACITY>>,
    pub m_pool: Arc<RingBuffer<1024, M_CAPACITY>>,
    pub l_pool: Arc<RingBuffer<4096, L_CAPACITY>>,
    pub xl_pool: Arc<RingBuffer<16384, XL_CAPACITY>>,
}