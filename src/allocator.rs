use std::sync::Arc;

use bytemuck::Zeroable;

use crate::{
    pool::EventPools,
    ring::{EventSize, PooledEvent, Reader, RingBuffer, Writer},
};

pub struct EventAllocator {
    pools: Arc<EventPools>,
}

impl EventAllocator {
    pub fn new() -> Self {
        Self {
            pools: Arc::new(EventPools {
                xs_pool: Arc::new(RingBuffer::new()),
                s_pool: Arc::new(RingBuffer::new()),
                m_pool: Arc::new(RingBuffer::new()),
                l_pool: Arc::new(RingBuffer::new()),
                xl_pool: Arc::new(RingBuffer::new()),
            }),
        }
    }

    // Get writers for each size
    pub fn get_xs_writer(&self) -> Writer<64, 2000> {
        Writer {
            ringbuffer: self.pools.xs_pool.clone(),
            last_ts: 0,
        }
    }

    pub fn get_s_writer(&self) -> Writer<256, 1000> {
        Writer {
            ringbuffer: self.pools.s_pool.clone(),
            last_ts: 0,
        }
    }

    pub fn get_m_writer(&self) -> Writer<1024, 300> {
        Writer {
            ringbuffer: self.pools.m_pool.clone(),
            last_ts: 0,
        }
    }

    pub fn get_l_writer(&self) -> Writer<4096, 60> {
        Writer {
            ringbuffer: self.pools.l_pool.clone(),
            last_ts: 0,
        }
    }

    pub fn get_xl_writer(&self) -> Writer<16384, 15> {
        Writer {
            ringbuffer: self.pools.xl_pool.clone(),
            last_ts: 0,
        }
    }

    // Get readers for each size
    pub fn get_xs_reader(&self) -> Reader<64, 2000> {
        Reader {
            ringbuffer: self.pools.xs_pool.clone(),
            cursor: 0,
            last_ts: 0,
        }
    }

    pub fn get_s_reader(&self) -> Reader<256, 1000> {
        Reader {
            ringbuffer: self.pools.s_pool.clone(),
            cursor: 0,
            last_ts: 0,
        }
    }

    pub fn get_m_reader(&self) -> Reader<1024, 300> {
        Reader {
            ringbuffer: self.pools.m_pool.clone(),
            cursor: 0,
            last_ts: 0,
        }
    }

    pub fn get_l_reader(&self) -> Reader<4096, 60> {
        Reader {
            ringbuffer: self.pools.l_pool.clone(),
            cursor: 0,
            last_ts: 0,
        }
    }

    pub fn get_xl_reader(&self) -> Reader<16384, 15> {
        Reader {
            ringbuffer: self.pools.xl_pool.clone(),
            cursor: 0,
            last_ts: 0,
        }
    }

    // ... similar for other reader sizes

    // Size-based allocation (the smart interface)
    pub fn estimate_size(data_len: usize) -> EventSize {
        match data_len {
            0..=64 => EventSize::XS,
            65..=256 => EventSize::S,
            257..=1024 => EventSize::M,
            1025..=4096 => EventSize::L,
            4097..=16384 => EventSize::XL,
            _ => EventSize::XXL,
        }
    }

    // Create pooled event from raw data
    pub fn create_pooled_event<const SIZE: usize>(
        data: &[u8],
        event_type: u32,
    ) -> Result<PooledEvent<SIZE>, &'static str> {
        if data.len() > SIZE {
            return Err("Data too large for this pool size");
        }
        let mut pooled = PooledEvent::<SIZE>::zeroed();
        pooled.data[..data.len()].copy_from_slice(data);
        pooled.len = data.len() as u32;
        pooled.event_type = event_type;
        Ok(pooled)
    }
}
