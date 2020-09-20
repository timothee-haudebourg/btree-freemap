use std::ops::Range;
use linear_btree::{
	BTreeMap,
	BTreeExt,
	Item,
	Node
};
use crate::{
	Address,
	AddressRange,
	AllocationStrategy,
	AllocationBoundary
};

pub(crate) trait BTreeFreeMap<T> {
	fn find_free_range(&self, len: T, strategy: AllocationStrategy, left: Item<T, AllocationBoundary>, right: Item<T, AllocationBoundary>) -> Option<Range<T>>;

	fn find_free_range_in(&self, len: T, strategy: AllocationStrategy, left: Item<T, AllocationBoundary>, right: Item<T, AllocationBoundary>, id: usize) -> Option<Range<T>>;
}

impl<T: Address, const N: usize> BTreeFreeMap<T> for BTreeMap<T, AllocationBoundary, N> {
	#[inline]
	fn find_free_range(&self, len: T, strategy: AllocationStrategy, left: Item<T, AllocationBoundary>, right: Item<T, AllocationBoundary>) -> Option<Range<T>> {
		match self.root_id() {
			Some(id) => self.find_free_range_in(len, strategy, left, right, id),
			None => {
				if left.value == AllocationBoundary::End {
					Some(left.key..right.key)
				} else {
					None
				}
			}
		}
	}

	fn find_free_range_in(&self, len: T, strategy: AllocationStrategy, left: Item<T, AllocationBoundary>, right: Item<T, AllocationBoundary>, id: usize) -> Option<Range<T>> {
		let node = self.node(id);
		match node {
			Node::Internal(node) => {
				let mut result: Option<Range<T>> = None;

				for (child_left, child_id, child_right) in node.children_with_separators() {
					let child_left = child_left.cloned().unwrap_or(left);
					let child_right = child_right.cloned().unwrap_or(right);

					let child_span = child_right.key - child_left.key;
					if child_span >= len {
						match self.find_free_range_in(len, strategy, child_left, child_right, child_id) {
							Some(range) => {
								let range_len = range.len();
								match (strategy, result.clone()) {
									(AllocationStrategy::FirstFit, None) => return Some(range),
									(AllocationStrategy::WorstFit, Some(current_range)) if current_range.len() < range_len => result = Some(range),
									(AllocationStrategy::BestFit, Some(current_range)) if current_range.len() > range_len => result = Some(range),
									(_, None) => result = Some(range),
									_ => ()
								}
							},
							None => ()
						}
					}
				}

				result
			},
			Node::Leaf(leaf) => {
				let mut result: Option<Range<T>> = None;
				let mut left_item = left;
				for right_item in leaf.items().cloned().chain(Some(right)) {
					if left_item.value == AllocationBoundary::End {
						let range = left_item.key..right_item.key;
						let range_len = range.len();
						if range_len >= len {
							match (strategy, result.clone()) {
								(AllocationStrategy::FirstFit, None) => return Some(range),
								(AllocationStrategy::WorstFit, Some(current_range)) if current_range.len() < range_len => result = Some(range),
								(AllocationStrategy::BestFit, Some(current_range)) if current_range.len() > range_len => result = Some(range),
								(_, None) => result = Some(range),
								_ => ()
							}
						}
					}

					left_item = right_item;
				}

				result
			},
			_ => unreachable!()
		}
	}
}
