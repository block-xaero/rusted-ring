use std::sync::Arc;
use std::sync::atomic::Ordering;
use crate::ring::{EventSize, RingBuffer, PooledEvent};

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

    pub fn capacity(&self) -> usize {
        match self {
            PoolId::XS => XS_CAPACITY,
            PoolId::S => S_CAPACITY,
            PoolId::M => M_CAPACITY,
            PoolId::L => L_CAPACITY,
            PoolId::XL => XL_CAPACITY,
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
    /// Get pointer to slot data (for RingPtr deref)
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

    /// Increment reference count (when RingPtr is cloned)
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

    /// Decrement reference count (when RingPtr is dropped)
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

    /// Mark slot as reusable (when ref count hits 0)
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

    /// Get generation number (for ABA protection)
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

    // NEW: Slot allocation methods

    /// Check if a slot is available (ref_count = 0, is_allocated = 0)
    pub fn is_slot_available(&self, pool_id: PoolId, slot_index: u32) -> bool {
        unsafe {
            match pool_id {
                PoolId::XS => {
                    let ptr = self.xs_pool.metadata.get();
                    let slot = &(*ptr)[slot_index as usize];
                    slot.ref_count.load(Ordering::Acquire) == 0 &&
                        slot.is_allocated.load(Ordering::Acquire) == 0
                }
                PoolId::S => {
                    let ptr = self.s_pool.metadata.get();
                    let slot = &(*ptr)[slot_index as usize];
                    slot.ref_count.load(Ordering::Acquire) == 0 &&
                        slot.is_allocated.load(Ordering::Acquire) == 0
                }
                PoolId::M => {
                    let ptr = self.m_pool.metadata.get();
                    let slot = &(*ptr)[slot_index as usize];
                    slot.ref_count.load(Ordering::Acquire) == 0 &&
                        slot.is_allocated.load(Ordering::Acquire) == 0
                }
                PoolId::L => {
                    let ptr = self.l_pool.metadata.get();
                    let slot = &(*ptr)[slot_index as usize];
                    slot.ref_count.load(Ordering::Acquire) == 0 &&
                        slot.is_allocated.load(Ordering::Acquire) == 0
                }
                PoolId::XL => {
                    let ptr = self.xl_pool.metadata.get();
                    let slot = &(*ptr)[slot_index as usize];
                    slot.ref_count.load(Ordering::Acquire) == 0 &&
                        slot.is_allocated.load(Ordering::Acquire) == 0
                }
            }
        }
    }

    /// Try to atomically claim a slot (CAS is_allocated: 0 -> 1)
    pub fn try_allocate_slot(&self, pool_id: PoolId, slot_index: u32) -> bool {
        unsafe {
            match pool_id {
                PoolId::XS => {
                    let ptr = self.xs_pool.metadata.get();
                    let slot = &(*ptr)[slot_index as usize];

                    // Only try to allocate if ref_count is 0
                    if slot.ref_count.load(Ordering::Acquire) != 0 {
                        return false;
                    }

                    // Atomically try to claim: is_allocated 0 -> 1
                    if slot.is_allocated.compare_exchange(
                        0, 1,
                        Ordering::AcqRel,
                        Ordering::Acquire
                    ).is_ok() {
                        // Successfully claimed, set initial ref count
                        slot.ref_count.store(1, Ordering::Release);
                        true
                    } else {
                        false // Someone else claimed it
                    }
                }
                PoolId::S => {
                    let ptr = self.s_pool.metadata.get();
                    let slot = &(*ptr)[slot_index as usize];

                    if slot.ref_count.load(Ordering::Acquire) != 0 {
                        return false;
                    }

                    if slot.is_allocated.compare_exchange(
                        0, 1,
                        Ordering::AcqRel,
                        Ordering::Acquire
                    ).is_ok() {
                        slot.ref_count.store(1, Ordering::Release);
                        true
                    } else {
                        false
                    }
                }
                PoolId::M => {
                    let ptr = self.m_pool.metadata.get();
                    let slot = &(*ptr)[slot_index as usize];

                    if slot.ref_count.load(Ordering::Acquire) != 0 {
                        return false;
                    }

                    if slot.is_allocated.compare_exchange(
                        0, 1,
                        Ordering::AcqRel,
                        Ordering::Acquire
                    ).is_ok() {
                        slot.ref_count.store(1, Ordering::Release);
                        true
                    } else {
                        false
                    }
                }
                PoolId::L => {
                    let ptr = self.l_pool.metadata.get();
                    let slot = &(*ptr)[slot_index as usize];

                    if slot.ref_count.load(Ordering::Acquire) != 0 {
                        return false;
                    }

                    if slot.is_allocated.compare_exchange(
                        0, 1,
                        Ordering::AcqRel,
                        Ordering::Acquire
                    ).is_ok() {
                        slot.ref_count.store(1, Ordering::Release);
                        true
                    } else {
                        false
                    }
                }
                PoolId::XL => {
                    let ptr = self.xl_pool.metadata.get();
                    let slot = &(*ptr)[slot_index as usize];

                    if slot.ref_count.load(Ordering::Acquire) != 0 {
                        return false;
                    }

                    if slot.is_allocated.compare_exchange(
                        0, 1,
                        Ordering::AcqRel,
                        Ordering::Acquire
                    ).is_ok() {
                        slot.ref_count.store(1, Ordering::Release);
                        true
                    } else {
                        false
                    }
                }
            }
        }
    }

    /// Place event data in allocated slot - separate methods for each size
    pub fn place_xs_event(&self, event: PooledEvent<64>, slot_index: u32) {
        unsafe {
            let ptr = self.xs_pool.data.get();
            (*ptr)[slot_index as usize] = event;
        }
    }

    pub fn place_s_event(&self, event: PooledEvent<256>, slot_index: u32) {
        unsafe {
            let ptr = self.s_pool.data.get();
            (*ptr)[slot_index as usize] = event;
        }
    }

    pub fn place_m_event(&self, event: PooledEvent<1024>, slot_index: u32) {
        unsafe {
            let ptr = self.m_pool.data.get();
            (*ptr)[slot_index as usize] = event;
        }
    }

    pub fn place_l_event(&self, event: PooledEvent<4096>, slot_index: u32) {
        unsafe {
            let ptr = self.l_pool.data.get();
            (*ptr)[slot_index as usize] = event;
        }
    }

    pub fn place_xl_event(&self, event: PooledEvent<16384>, slot_index: u32) {
        unsafe {
            let ptr = self.xl_pool.data.get();
            (*ptr)[slot_index as usize] = event;
        }
    }

    /// Get reference count for a slot (for testing)
    pub fn get_ref_count(&self, pool_id: PoolId, slot_index: u32) -> u8 {
        unsafe {
            match pool_id {
                PoolId::XS => {
                    let ptr = self.xs_pool.metadata.get();
                    (*ptr)[slot_index as usize].ref_count.load(Ordering::Acquire)
                }
                PoolId::S => {
                    let ptr = self.s_pool.metadata.get();
                    (*ptr)[slot_index as usize].ref_count.load(Ordering::Acquire)
                }
                PoolId::M => {
                    let ptr = self.m_pool.metadata.get();
                    (*ptr)[slot_index as usize].ref_count.load(Ordering::Acquire)
                }
                PoolId::L => {
                    let ptr = self.l_pool.metadata.get();
                    (*ptr)[slot_index as usize].ref_count.load(Ordering::Acquire)
                }
                PoolId::XL => {
                    let ptr = self.xl_pool.metadata.get();
                    (*ptr)[slot_index as usize].ref_count.load(Ordering::Acquire)
                }
            }
        }
    }
    pub fn get_pool_stats(&self, pool_id: PoolId) -> PoolStats {
        let capacity = pool_id.capacity();
        let mut allocated_slots = 0;
        let mut total_references = 0;

        for slot_index in 0..capacity {
            unsafe {
                let (ref_count, is_allocated) = match pool_id {
                    PoolId::XS => {
                        let ptr = self.xs_pool.metadata.get();
                        let slot = &(*ptr)[slot_index];
                        (
                            slot.ref_count.load(Ordering::Acquire),
                            slot.is_allocated.load(Ordering::Acquire)
                        )
                    }
                    PoolId::S => {
                        let ptr = self.s_pool.metadata.get();
                        let slot = &(*ptr)[slot_index];
                        (
                            slot.ref_count.load(Ordering::Acquire),
                            slot.is_allocated.load(Ordering::Acquire)
                        )
                    }
                    PoolId::M => {
                        let ptr = self.m_pool.metadata.get();
                        let slot = &(*ptr)[slot_index];
                        (
                            slot.ref_count.load(Ordering::Acquire),
                            slot.is_allocated.load(Ordering::Acquire)
                        )
                    }
                    PoolId::L => {
                        let ptr = self.l_pool.metadata.get();
                        let slot = &(*ptr)[slot_index];
                        (
                            slot.ref_count.load(Ordering::Acquire),
                            slot.is_allocated.load(Ordering::Acquire)
                        )
                    }
                    PoolId::XL => {
                        let ptr = self.xl_pool.metadata.get();
                        let slot = &(*ptr)[slot_index];
                        (
                            slot.ref_count.load(Ordering::Acquire),
                            slot.is_allocated.load(Ordering::Acquire)
                        )
                    }
                };

                if is_allocated != 0 {
                    allocated_slots += 1;
                }
                total_references += ref_count as usize;
            }
        }

        PoolStats {
            pool_id,
            total_slots: capacity,
            allocated_slots,
            available_slots: capacity - allocated_slots,
            total_references,
            utilization_percent: (allocated_slots as f32 / capacity as f32) * 100.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PoolStats {
    pub pool_id: PoolId,
    pub total_slots: usize,
    pub allocated_slots: usize,
    pub available_slots: usize,
    pub total_references: usize,
    pub utilization_percent: f32,
}

impl std::fmt::Display for PoolStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Pool {:?}: {}/{} slots ({:.1}%), {} total refs",
            self.pool_id,
            self.allocated_slots,
            self.total_slots,
            self.utilization_percent,
            self.total_references
        )
    }
}