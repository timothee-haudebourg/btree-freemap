#![feature(min_const_generics)]

use linear_btree::{
	BTreeMap,
	Item
};

mod btree;
use btree::BTreeFreeMap;

pub struct AllocationFailed;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AllocationStrategy {
	/// Minimises allocation time.
	FirstFit,

	/// Minimises fragmentation.
	WorstFit,

	/// Minimises memory usage.
	BestFit
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AllocationBoundary {
	/// Marks the begining of an allocated memory region.
	Begin,

	/// Marks the begining of a free memory region.
	End
}

pub trait Address : Copy + Ord + std::ops::Add<Output = Self> + std::ops::Sub<Output = Self> {
	const START: Self;
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

pub struct FreeMap<T: Address, const N: usize> {
	map: BTreeMap<T, AllocationBoundary, N>,
	len: T,
	strategy: AllocationStrategy,
	start: AllocationBoundary,
	end: AllocationBoundary
}

impl<T: Address, const N: usize> FreeMap<T, N> {
	#[inline]
	pub fn new(len: T, strategy: AllocationStrategy) -> FreeMap<T, N> {
		FreeMap {
			map: BTreeMap::new(),
			len,
			strategy,
			start: AllocationBoundary::End,
			end: AllocationBoundary::Begin
		}
	}

	#[inline]
	pub fn allocate(&mut self, len: T) -> Result<T, AllocationFailed> {
		match self.map.find_free_range(len, self.strategy, Item::new(T::START, self.start), Item::new(self.len, self.end)) {
			Some(range) => {
				if range.start == T::START {
					self.start = AllocationBoundary::Begin
				} else {
					self.map.remove(&range.start);
				}

				if range.len() == len {
					if range.end < self.len {
						self.map.remove(&range.end);
					} else {
						self.end = AllocationBoundary::End
					}
				} else {
					self.map.insert(range.end, AllocationBoundary::End);
				}

				Ok(range.start)
			},
			None => Err(AllocationFailed)
		}
	}

	#[inline]
	pub fn free(&mut self, offset: T, len: T) {
		let end = offset + len;

		if offset == T::START {
			self.start = AllocationBoundary::End
		} else {
			self.map.update(offset, |boundary| {
				match boundary {
					Some(AllocationBoundary::Begin) => (None, ()),
					None => (Some(AllocationBoundary::End), ()),
					_ => panic!("corrupted free map")
				}
			})
		}

		if end < self.len {
			self.map.update(end, |boundary| {
				match boundary {
					Some(AllocationBoundary::End) => (None, ()),
					None => (Some(AllocationBoundary::Begin), ()),
					_ => panic!("corrupted free map")
				}
			})
		} else {
			self.start = AllocationBoundary::Begin
		}
	}
}
