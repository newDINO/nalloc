use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
    ops::Range,
};

#[derive(Debug)]
pub struct BinnedAllocation {
    pub allocation: Allocation,
    pub bin_id: usize,
}

#[derive(Debug)]
pub struct Allocator {
    // Free regions sorted first by size than by addr.
    size_addr_set: BTreeSet<u64>,
    // Free regions with addr as key and size as value.
    addr_size_map: BTreeMap<u32, u32>,
    size: u32,
}

impl Allocator {
    pub fn new(size: u32) -> Self {
        Self {
            size_addr_set: BTreeSet::from_iter([new_size_addr(size, 0)]),
            addr_size_map: BTreeMap::from_iter([(0, size)]),
            size,
        }
    }
    pub fn is_empty(&self) -> bool {
        if self.addr_size_map.len() > 1 {
            false
        } else {
            let size = *self.addr_size_map.first_key_value().unwrap().1;
            size == self.size
        }
    }
    pub fn alloc(&mut self, size: u32) -> Option<Allocation> {
        if size == 0 {
            const ZERO_ALLOCATION: Allocation =
                Allocation(NonZeroU64::new(new_size_addr(0, 1)).unwrap());
            return Some(ZERO_ALLOCATION);
        }
        let region_size_addr = *self.size_addr_set.range(new_size_addr(size, 0)..).next()?;

        let region_size = (region_size_addr >> 32) as u32;
        let addr = region_size_addr as u32;

        self.addr_size_map.remove(&addr);
        self.size_addr_set.remove(&region_size_addr);

        let left_size = region_size - size;
        if left_size > 0 {
            let left_addr = addr + size;

            self.size_addr_set
                .insert(new_size_addr(left_size, left_addr));
            self.addr_size_map.insert(left_addr, left_size);
        }

        // size > 0, compiler is able to optimize this.
        Some(Allocation(
            NonZeroU64::new(new_size_addr(size, addr)).unwrap(),
        ))
    }
    pub fn free(&mut self, allocation: Allocation) {
        let size_addr = allocation.0.get();

        let size = (size_addr >> 32) as u32;
        if size == 0 {
            // The reason to do this is to prevent adding zero sized free regions.
            return;
        }
        let addr = size_addr as u32;

        let end_addr = addr + size;

        let added_size = if let Some(next_size) = self.addr_size_map.remove(&end_addr) {
            self.size_addr_set
                .remove(&new_size_addr(next_size, end_addr));

            size + next_size
        } else {
            size
        };

        if let Some((last_addr, last_size)) = self.addr_size_map.range_mut(..addr).next_back()
            && *last_addr + *last_size == addr
        {
            self.size_addr_set
                .remove(&new_size_addr(*last_size, *last_addr));

            *last_size += added_size;

            self.size_addr_set
                .insert(new_size_addr(*last_size, *last_addr));
        } else {
            self.addr_size_map.insert(addr, added_size);
            self.size_addr_set.insert(new_size_addr(added_size, addr));
        }
    }
    pub fn increase_size(&mut self, to_add_size: u32) {
        if let Some(mut last) = self.addr_size_map.last_entry()
            && last.key() + last.get() == self.size
        {
            let last_addr = *last.key();
            let last_size = last.get_mut();
            self.size_addr_set
                .remove(&new_size_addr(*last_size, last_addr));

            *last_size += to_add_size;

            self.size_addr_set
                .insert(new_size_addr(*last_size, last_addr));
        } else {
            let addr = self.size;
            let size = to_add_size;
            self.addr_size_map.insert(addr, size);
            self.size_addr_set.insert(new_size_addr(size, addr));
        }

        self.size += to_add_size;
    }
}

const fn new_size_addr(size: u32, addr: u32) -> u64 {
    ((size as u64) << 32) | (addr as u64)
}

pub struct Allocation(NonZeroU64);
impl std::fmt::Debug for Allocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Allocation {{ size: {} , addr: {} }}",
            self.size(),
            self.addr()
        )
    }
}

impl Allocation {
    pub fn to_range(&self) -> Range<u32> {
        let size = (self.0.get() >> 32) as u32;
        let addr = self.0.get() as u32;
        addr..addr + size
    }
    pub fn addr(&self) -> u32 {
        self.0.get() as u32
    }
    pub fn size(&self) -> u32 {
        (self.0.get() >> 32) as u32
    }
}
