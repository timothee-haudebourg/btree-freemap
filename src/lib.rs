#![feature(min_const_generics)]

use std::ops::Range;
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
pub(crate) enum AllocationBoundary<T> {
	/// Marks the begining of an allocated memory region.
	Begin(T),

	/// Marks the begining of a free memory region.
	End(T)
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
	len: T,
	strategy: AllocationStrategy,
	map: BTreeMap<T, Vec<T>, N>,
	boundaries: BTreeMap<T, AllocationBoundary<T>, N>
}

impl<T: Address, const N: usize> FreeMap<T, N> {
	#[inline]
	pub fn new(len: T, strategy: AllocationStrategy) -> FreeMap<T, N> {
		let mut map = BTreeMap::new();
		map.insert(len, vec![T::START]);

		let mut boundaries = BTreeMap::new();
		boundaries.insert(T::START, AllocationBoundary::End(len));
		boundaries.insert(len, AllocationBoundary::Begin(len));

		FreeMap {
			len,
			strategy,
			map,
			boundaries
		}
	}

	fn insert_free_range(&mut self, free_range: Range<T>) {
		self.map.update(free_range.len(), |offsets| {
			match offsets {
				Some(mut offsets) => {
					offsets.push(free_range.start);
					(Some(offsets), ())
				},
				None => {
					let offsets = vec![free_range.start];
					(Some(offsets), ())
				}
			}
		});
	}

	fn remove_free_range(&mut self, free_range: Range<T>) {
		self.map.update(free_range.len(), |offsets| {
			let mut offsets = offsets.expect("corrupted free map");
			let i = offsets.iter().position(|o| *o == free_range.start).expect("corrupted free map");
			offsets.swap_remove(i);
			if offsets.is_empty() {
				(None, ())
			} else {
				(Some(offsets), ())
			}
		});
	}

	/// ## Complexity
	///
	/// Best case (allocation failed): O(1 * log n)
	/// Best success case: O(4 * log n)
	/// Worst case: O(5 * log n)
	#[inline]
	pub fn allocate(&mut self, len: T) -> Result<T, AllocationFailed> {
		match self.map.find_free_range(len, self.strategy) {
			Some(range) => {
				self.remove_free_range(range.clone());

				let new_free_range = (range.start+len)..range.end;
				let new_free_range_len = new_free_range.len();
				if new_free_range_len > T::START {
					self.insert_free_range(new_free_range);
					self.boundaries.insert(range.end, AllocationBoundary::End(new_free_range_len));
				} else {
					self.boundaries.remove(&range.end);
				}

				self.boundaries.remove(&range.start);
				Ok(range.start)
			},
			None => Err(AllocationFailed)
		}
	}

	/// ## Complexity
	///
	/// Best case: O(5 * log n)
	/// Worst case: O(10 * log n)
	#[inline]
	pub fn free(&mut self, offset: T, len: T) {
		let end = offset + len;

		let mut final_free_region_offset = offset;
		let mut final_free_region_end = end;

		let left = match self.boundaries.get(&offset) {
			Some(AllocationBoundary::Begin(len)) => {
				final_free_region_offset = offset - *len;
				Some(final_free_region_offset..offset)
			},
			None => None,
			_ => panic!("corrupted free map")
		};

		let right = match self.boundaries.get(&end) {
			Some(AllocationBoundary::End(len)) => {
				final_free_region_end = end + *len;
				Some(end..final_free_region_end)
			},
			None => None,
			_ => panic!("corrupted free map")
		};

		let final_free_range = final_free_region_offset..final_free_region_end;
		let final_free_region_len = final_free_range.len();

		if let Some(left) = left {
			self.remove_free_range(left);
			self.boundaries.remove(&offset);
		}

		if let Some(right) = right {
			self.remove_free_range(right);
			self.boundaries.remove(&end);
		}

		self.boundaries.insert(final_free_range.start, AllocationBoundary::End(final_free_region_len));
		self.boundaries.insert(final_free_range.end, AllocationBoundary::Begin(final_free_region_len));

		self.insert_free_range(final_free_range);
	}
}
