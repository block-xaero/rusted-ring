use std::sync::Arc;
use std::sync::atomic::Ordering;
use crate::ring::{EventSize, RingBuffer};

// Pool size constants - Adjusted to stay under 1MB per ring buffer
pub const XS_CAPACITY: usize = 2000; // 64 * 2000 = 128KB
pub const S_CAPACITY: usize = 1000; // 256 * 1000 = 256KB
pub const M_CAPACITY: usize = 300; // 1024 * 300 = 307KB
pub const L_CAPACITY: usize = 60; // 4096 * 60 = 245KB
pub const XL_CAPACITY: usize = 15; // 16384 * 15 = 245KB 

pub struct EventPools {
    pub xs_pool: Arc<RingBuffer<64, XS_CAPACITY>>,
    pub s_pool: Arc<RingBuffer<256, S_CAPACITY>>,
    pub m_pool: Arc<RingBuffer<1024, M_CAPACITY>>,
    pub l_pool: Arc<RingBuffer<4096, L_CAPACITY>>,
    pub xl_pool: Arc<RingBuffer<16384, XL_CAPACITY>>,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolId {
    XS = 0, // 64 bytes
    S = 1,  // 256 bytes
    M = 2,  // 1KB
    L = 3,  // 4KB
    XL = 4, // 16KB
}

impl PoolId {
    pub fn from_size(size: EventSize) -> Self {
        match size {
            EventSize::XS => PoolId::XS,
            EventSize::S => PoolId::S,
            EventSize::M => PoolId::M,
            EventSize::L => PoolId::L,
            EventSize::XL => PoolId::XL,
            EventSize::XXL => panic!("XXL not supported in pools"),
        }
    }

    pub fn max_size(&self) -> usize {
        match self {
            PoolId::XS => 64,
            PoolId::S => 256,
            PoolId::M => 1024,
            PoolId::L => 4096,
            PoolId::XL => 16384,
        }
    }
}

impl From<PoolId> for u8 {
    fn from(value: PoolId) -> Self {
        value as u8
    }
}

impl From<u8> for PoolId {
    fn from(value: u8) -> Self {
        match value {
            0 => PoolId::XS,
            1 => PoolId::S,
            2 => PoolId::M,
            3 => PoolId::L,
            4 => PoolId::XL,
            _ => panic!("Unknown pool id"),
        }
    }
}

impl EventPools {
    pub fn get_slot_data(&self, pool_id: PoolId, slot_index: u32) -> *const u8 {
        unsafe {
            match pool_id {
                PoolId::XS => {
                    let ptr = self.xs_pool.data.get();
                    &(*ptr)[slot_index as usize].data as *const u8
                }
                PoolId::S => {
                    let ptr = self.s_pool.data.get();
                    &(*ptr)[slot_index as usize].data as *const u8
                }
                PoolId::M => {
                    let ptr = self.m_pool.data.get();
                    &(*ptr)[slot_index as usize].data as *const u8
                }
                PoolId::L => {
                    let ptr = self.l_pool.data.get();
                    &(*ptr)[slot_index as usize].data as *const u8
                }
                PoolId::XL => {
                    let ptr = self.xl_pool.data.get();
                    &(*ptr)[slot_index as usize].data as *const u8
                }
            }
        }
    }

    pub fn inc_ref_count(&self, pool_id: PoolId, slot_index: u32) -> u8 {
        unsafe {
            match pool_id {
                PoolId::XS => {
                    let ptr = self.xs_pool.metadata.get();
                    (*ptr)[slot_index as usize].ref_count.fetch_add(1, Ordering::AcqRel)
                }
                PoolId::S => {
                    let ptr = self.s_pool.metadata.get();
                    (*ptr)[slot_index as usize].ref_count.fetch_add(1, Ordering::AcqRel)
                }
                PoolId::M => {
                    let ptr = self.m_pool.metadata.get();
                    (*ptr)[slot_index as usize].ref_count.fetch_add(1, Ordering::AcqRel)
                }
                PoolId::L => {
                    let ptr = self.l_pool.metadata.get();
                    (*ptr)[slot_index as usize].ref_count.fetch_add(1, Ordering::AcqRel)
                }
                PoolId::XL => {
                    let ptr = self.xl_pool.metadata.get();
                    (*ptr)[slot_index as usize].ref_count.fetch_add(1, Ordering::AcqRel)
                }
            }
        }
    }

    pub fn dec_ref_count(&self, pool_id: PoolId, slot_index: u32) -> u8 {
        unsafe {
            match pool_id {
                PoolId::XS => {
                    let ptr = self.xs_pool.metadata.get();
                    (*ptr)[slot_index as usize].ref_count.fetch_sub(1, Ordering::AcqRel)
                }
                PoolId::S => {
                    let ptr = self.s_pool.metadata.get();
                    (*ptr)[slot_index as usize].ref_count.fetch_sub(1, Ordering::AcqRel)
                }
                PoolId::M => {
                    let ptr = self.m_pool.metadata.get();
                    (*ptr)[slot_index as usize].ref_count.fetch_sub(1, Ordering::AcqRel)
                }
                PoolId::L => {
                    let ptr = self.l_pool.metadata.get();
                    (*ptr)[slot_index as usize].ref_count.fetch_sub(1, Ordering::AcqRel)
                }
                PoolId::XL => {
                    let ptr = self.xl_pool.metadata.get();
                    (*ptr)[slot_index as usize].ref_count.fetch_sub(1, Ordering::AcqRel)
                }
            }
        }
    }

    pub fn mark_slot_reusable(&self, pool_id: PoolId, slot_index: u32) {
        unsafe {
            match pool_id {
                PoolId::XS => {
                    let ptr = self.xs_pool.metadata.get();
                    (*ptr)[slot_index as usize].generation.fetch_add(1, Ordering::Release);
                    (*ptr)[slot_index as usize].is_allocated.store(0, Ordering::Release);
                }
                PoolId::S => {
                    let ptr = self.s_pool.metadata.get();
                    (*ptr)[slot_index as usize].generation.fetch_add(1, Ordering::Release);
                    (*ptr)[slot_index as usize].is_allocated.store(0, Ordering::Release);
                }
                PoolId::M => {
                    let ptr = self.m_pool.metadata.get();
                    (*ptr)[slot_index as usize].generation.fetch_add(1, Ordering::Release);
                    (*ptr)[slot_index as usize].is_allocated.store(0, Ordering::Release);
                }
                PoolId::L => {
                    let ptr = self.l_pool.metadata.get();
                    (*ptr)[slot_index as usize].generation.fetch_add(1, Ordering::Release);
                    (*ptr)[slot_index as usize].is_allocated.store(0, Ordering::Release);
                }
                PoolId::XL => {
                    let ptr = self.xl_pool.metadata.get();
                    (*ptr)[slot_index as usize].generation.fetch_add(1, Ordering::Release);
                    (*ptr)[slot_index as usize].is_allocated.store(0, Ordering::Release);
                }
            }
        }
    }

    pub fn get_generation(&self, pool_id: PoolId, slot_index: u32) -> u32 {
        unsafe {
            match pool_id {
                PoolId::XS => {
                    let ptr = self.xs_pool.metadata.get();
                    (*ptr)[slot_index as usize].generation.load(Ordering::Acquire) as u32
                }
                PoolId::S => {
                    let ptr = self.s_pool.metadata.get();
                    (*ptr)[slot_index as usize].generation.load(Ordering::Acquire) as u32
                }
                PoolId::M => {
                    let ptr = self.m_pool.metadata.get();
                    (*ptr)[slot_index as usize].generation.load(Ordering::Acquire) as u32
                }
                PoolId::L => {
                    let ptr = self.l_pool.metadata.get();
                    (*ptr)[slot_index as usize].generation.load(Ordering::Acquire) as u32
                }
                PoolId::XL => {
                    let ptr = self.xl_pool.metadata.get();
                    (*ptr)[slot_index as usize].generation.load(Ordering::Acquire) as u32
                }
            }
        }
    }
}