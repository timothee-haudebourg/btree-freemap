use std::ops::Range;
use slab::Slab;
use btree_slab::{
	BTreeMap,
	generic::map::BTreeExtMut
};

mod btree;
use btree::BTreeFreeMap;

pub struct AllocationFailed;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AllocationStrategy {
	/// Minimises allocation time.
	///
	/// # Complexity
	/// - Allocation: O(1 log n)
	/// - Free: O(log n)
	FirstFit,

	/// Minimises external fragmentation.
	///
	/// # Complexity
	/// - Allocation: O(2 log n)
	/// - Free: O(log n)
	WorstFit,

	/// Minimises memory waste.
	///
	/// # Complexity
	/// - Allocation: O(2 log n)
	/// - Free: O(log n)
	BestFit
}

pub trait Address : Copy + Ord + std::ops::Add<Output = Self> + std::ops::Sub<Output = Self> {
	const ZERO: Self;
}

pub(crate) trait AddressRange<T> {
	fn len(&self) -> T;
}

impl<T: Address> AddressRange<T> for std::ops::Range<T> {
	#[inline]
	fn len(&self) -> T {
		self.end - self.start
	}
}

fn index(i: usize) -> Option<usize> {
	if i == std::usize::MAX {
		Some(i)
	} else {
		None
	}
}

struct Page<T> {
	len: T
}

/// Free region of unknown size.
#[derive(Copy, Clone)]
struct FreeRegion<T> {
	page: usize,
	offset: T,
	previous_allocated_region: usize
}

/// Free regions uniquely indexed and organised in linked-lists of same-size regions.
struct FreeRegions<T> {
	regions: Slab<(FreeRegion<T>, usize)>
}

impl<T> FreeRegions<T> {
	fn new() -> FreeRegions<T> {
		FreeRegions {
			regions: Slab::new()
		}
	}

	fn pop(&mut self, i: usize) -> (Option<usize>, FreeRegion<T>) {
		panic!("TODO")
	}

	fn push(&mut self, i: Option<usize>) -> (Option<usize>, ()) {
		panic!("TODO")
	}
}

#[derive(Copy, Clone)]
struct AllocatedRegion<T> {
	offset: T,
	len: T
}

struct AllocatedRegions<T> {
	regions: Slab<(usize, AllocatedRegion<T>, usize)>,
	first: usize
}

impl<T> AllocatedRegions<T> {
	fn new() -> AllocatedRegions<T> {
		AllocatedRegions {
			regions: Slab::new(),
			first: std::usize::MAX
		}
	}

	/// Insert a first allocated region.
	///
	/// Return the second allocated region.
	fn push_front(&mut self, region: AllocatedRegion<T>) -> Option<AllocatedRegion<T>> where T: Copy {
		let second = self.first;
		let n = self.regions.insert((std::usize::MAX, region, second));
		self.first = n;
		match index(second) {
			Some(i) => {
				self.regions[i].0 = n;
				Some(self.regions[i].1)
			},
			None => None
		}
	}

	/// Insert an allocated region after the given index.
	///
	/// Return the next allocated region.
	fn insert_after(&mut self, i: usize, region: AllocatedRegion<T>) -> Option<AllocatedRegion<T>> where T: Copy {
		let next = self.regions[i].2;
		let n = self.regions.insert((i, region, next));
		self.regions[i].2 = n;
		match index(next) {
			Some(j) => {
				self.regions[j].0 = n;
				Some(self.regions[j].1)
			},
			None => None
		}
	}

	/// Remove the region with the given index and return the allocated region before and after it.
	fn remove(&mut self, i: usize) -> (Option<AllocatedRegion<T>>, Option<AllocatedRegion<T>>) where T: Copy {
		let node = self.regions.remove(i);

		let prev = match index(node.0) {
			Some(j) => {
				let prev = &mut self.regions[j];
				prev.2 = node.2;
				Some(prev.1)
			},
			None => None
		};

		let next = match index(node.2) {
			Some(j) => {
				let next = &mut self.regions[j];
				next.0 = node.0;
				Some(next.1)
			},
			None => None
		};

		(prev, next)
	}

	/// Free region starting at the given offset index.
	fn free(&mut self, page_len: T, i: usize) -> Range<T> where T: Address {
		let (prev, next) = self.remove(i);

		let free_region_start = match prev {
			Some(prev) => prev.offset + prev.len,
			None => T::ZERO
		};

		let free_region_end = match next {
			Some(next) => next.offset,
			None => page_len
		};

		free_region_start..free_region_end
	}

	/// Allocate a new free region.
	///
	/// Returns the range of the next free region.
	fn allocate(&mut self, page_len: T, i: Option<usize>, offset: T, len: T) -> Range<T> where T: Address {
		let region = AllocatedRegion {
			offset,
			len
		};

		let next_region = match i {
			Some(i) => self.insert_after(i, region),
			None => self.push_front(region)
		};

		let next_free_region_start = offset + len;
		let next_free_region_end = match next_region {
			Some(region) => region.offset,
			None => page_len
		};

		next_free_region_start..next_free_region_end
	}
}

/// Alloated region informations.
pub struct Allocation<T> {
	/// Page where the allocated region is.
	pub page: usize,

	/// Offset of the allocated region in the page.
	pub offset: T,

	/// Size of the allocated region.
	pub len: T
}

pub struct FreeMap<T: Address> {
	/// Prefered allocation strategy.
	strategy: AllocationStrategy,

	/// Memory pages.
	pages: Slab<Page<T>>,

	/// Allocated regions.
	allocated_regions: AllocatedRegions<T>,

	/// Free regions.
	free_regions: FreeRegions<T>,

	/// Maps a size to a free region of the given size.
	map: BTreeMap<T, usize>
}

impl<T: Address> FreeMap<T> {
	#[inline]
	pub fn new(strategy: AllocationStrategy) -> FreeMap<T> {
		FreeMap {
			strategy,
			pages: Slab::new(),
			allocated_regions: AllocatedRegions::new(),
			free_regions: FreeRegions::new(),
			map: BTreeMap::new()
		}
	}

	/// Add a new empty page.
	///
	/// Returns the index used to uniquely identify the page.
	pub fn new_page(&mut self, len: T) -> usize {
		self.pages.insert(Page {
			len
		})
	}

	#[inline]
	pub fn allocate(&mut self, len: T) -> Result<Allocation<T>, AllocationFailed> {
		if len > T::ZERO {
			match self.map.address_of_free_range(len, self.strategy) {
				Some((addr, region_len)) => {
					let free_regions = &mut self.free_regions;
					let free_region = self.map.update_at(addr, move |i| free_regions.pop(i));

					let page_len = self.pages[free_region.page].len;
					let new_free_region_range = self.allocated_regions.allocate(page_len, index(free_region.previous_allocated_region), free_region.offset, len);

					let free_regions = &mut self.free_regions;
					self.map.update(len, move |i| free_regions.push(i)); // O(log n)

					Ok(Allocation {
						page: free_region.page,
						offset: free_region.offset,
						len
					})
				},
				None => Err(AllocationFailed)
			}
		} else {
			Ok(Allocation {
				page: 0,
				offset: T::ZERO,
				len: T::ZERO
			})
		}
	}

	#[inline]
	pub fn free(&mut self, offset: T, len: T) {
		panic!("TODO")
	}
}
