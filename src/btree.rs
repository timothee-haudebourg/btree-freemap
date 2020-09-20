use std::ops::Range;
use linear_btree::{
	BTreeMap,
	BTreeExt,
	Balance,
	Item,
	Node,
	utils::binary_search_min
};
use crate::{
	Address,
	AddressRange,
	AllocationStrategy,
	AllocationBoundary
};

pub(crate) trait BTreeFreeMap<T> {
	fn find_free_range(&mut self, min_len: T, strategy: AllocationStrategy) -> Option<Range<T>>;

	fn find_free_range_in(&mut self, min_len: T, strategy: AllocationStrategy, id: usize) -> Option<(Range<T>, Balance)>;
}

impl<T: Address, const N: usize> BTreeFreeMap<T> for BTreeMap<T, Vec<T>, N> {
	#[inline]
	fn find_free_range(&mut self, len: T, strategy: AllocationStrategy) -> Option<Range<T>> {
		match self.root_id() {
			Some(id) => {
				match self.find_free_range_in(len, strategy, id) {
					Some((range, balance)) => {
						match balance {
							Balance::Underflow(true) => { // The root is empty.
								self.set_root_id(self.node(id).child_id_opt(0));
								self.release_node(id);
							},
							_ => ()
						};

						Some(range)
					},
					None => None
				}
			},
			None => None
		}
	}

	fn find_free_range_in(&mut self, len: T, strategy: AllocationStrategy, id: usize) -> Option<(Range<T>, Balance)> {
		match free_range_offset_in(self.node(id), len, strategy) {
			Ok(None) => None,
			Ok(Some(offset)) => {
				take_from(self, id, offset)
			},
			Err((child_index, child_id)) => {
				match self.find_free_range_in(len, strategy, child_id) {
					Some((value, child_balance)) => {
						let balance = self.rebalance_child(id, child_index, child_balance);
						Some((value, balance))
					},
					None if strategy == AllocationStrategy::BestFit => {
						let offset = child_index;
						take_from(self, id, offset)
					},
					None => None
				}
			}
		}
	}
}

#[inline]
fn take_from<T, const M: usize>(btree: &mut BTreeMap<T, Vec<T>, M>, id: usize, offset: usize) -> Option<(Range<T>, Balance)> where T: Address {
	let range;
	let mut item_removed = false;

	let taken = {
		let node = btree.node_mut(id);
		match node.item_at_mut_opt(offset) {
			Some(item) => {
				let len = item.key;
				let start = item.value.pop().unwrap();
				range = start..(start+len);

				if item.value.is_empty() {
					item_removed = true;
					Some(node.take(offset))
				} else {
					None
				}
			},
			None => return None
		}
	};

	if item_removed {
		btree.set_len(btree.len() - 1);
	}

	match taken {
		Some(Ok((item, balance))) => { // removed from a leaf.
			Some((range, balance))
		},
		Some(Err(left_child_id)) => { // removed from an internal node.
			let left_child_index = offset;
			let (separator, left_child_balance) = btree.remove_rightmost_leaf_of(left_child_id);
			btree.node_mut(id).replace(offset, separator);
			let balance = btree.rebalance_child(id, left_child_index, left_child_balance);
			Some((range, balance))
		},
		None => {
			Some((range, Balance::Balanced))
		}
	}
}

fn free_range_offset_in<T, const M: usize>(node: &Node<T, Vec<T>, M>, len: T, strategy: AllocationStrategy) -> Result<Option<usize>, (usize, usize)> where T: Address {
	match node {
		Node::Internal(node) => {
			let branches = node.branches();
			match strategy {
				AllocationStrategy::FirstFit => {
					match binary_search_min(branches, &len) {
						Some(i) => {
							let b = &branches[i];
							if b.item.key == len {
								Ok(Some(i))
							} else {
								let j = i+1;
								if j < branches.len() {
									let b = &branches[j];
									Ok(Some(j))
								} else {
									Err((i, b.child))
								}
							}
						},
						None => Ok(Some(0))
					}
				},
				AllocationStrategy::WorstFit => {
					let i = branches.len() - 1;
					Err((i, branches[i].child))
				},
				AllocationStrategy::BestFit => {
					match binary_search_min(branches, &len) {
						Some(i) => {
							let b = &branches[i];
							if b.item.key == len {
								Ok(Some(i))
							} else {
								Err((i, b.child))
							}
						},
						None => {
							Ok(Some(0))
						}
					}
				}
			}
		},
		Node::Leaf(leaf) => {
			panic!("TODO")
		},
		_ => unreachable!()
	}
}

// pub(crate) trait BTreeFreeMap<T> {
// 	fn find_free_range(&self, len: T, strategy: AllocationStrategy, left: Item<T, AllocationBoundary>, right: Item<T, AllocationBoundary>) -> Option<T>;
//
// 	fn find_free_range_in(&self, len: T, strategy: AllocationStrategy, left: Item<T, AllocationBoundary>, right: Item<T, AllocationBoundary>, id: usize) -> Option<T>;
// }
//
// impl<T: Address, const N: usize> BTreeFreeMap<T> for BTreeMap<T, AllocationBoundary, N> {
// 	#[inline]
// 	fn find_free_range(&self, len: T, strategy: AllocationStrategy, left: Item<T, AllocationBoundary>, right: Item<T, AllocationBoundary>) -> Option<T> {
// 		match self.root_id() {
// 			Some(id) => self.find_free_range_in(len, strategy, left, right, id),
// 			None => {
// 				if left.value == AllocationBoundary::End {
// 					Some(left.key..right.key)
// 				} else {
// 					None
// 				}
// 			}
// 		}
// 	}
//
// 	fn find_free_range_in(&self, len: T, strategy: AllocationStrategy, left: Item<T, AllocationBoundary>, right: Item<T, AllocationBoundary>, id: usize) -> Option<T> {
// 		let node = self.node(id);
// 		match node {
// 			Node::Internal(node) => {
// 				let mut result: Option<T> = None;
//
// 				for (child_left, child_id, child_right) in node.children_with_separators() {
// 					let child_left = child_left.cloned().unwrap_or(left);
// 					let child_right = child_right.cloned().unwrap_or(right);
//
// 					let child_span = child_right.key - child_left.key;
// 					if child_span >= len {
// 						match self.find_free_range_in(len, strategy, child_left, child_right, child_id) {
// 							Some(range) => {
// 								let range_len = range.len();
// 								match (strategy, result.clone()) {
// 									(AllocationStrategy::FirstFit, None) => return Some(range),
// 									(AllocationStrategy::WorstFit, Some(current_range)) if current_range.len() < range_len => result = Some(range),
// 									(AllocationStrategy::BestFit, Some(current_range)) if current_range.len() > range_len => result = Some(range),
// 									(_, None) => result = Some(range),
// 									_ => ()
// 								}
// 							},
// 							None => ()
// 						}
// 					}
// 				}
//
// 				result
// 			},
// 			Node::Leaf(leaf) => {
// 				let mut result: Option<T> = None;
// 				let mut left_item = left;
// 				for right_item in leaf.items().cloned().chain(Some(right)) {
// 					if left_item.value == AllocationBoundary::End {
// 						let range = left_item.key..right_item.key;
// 						let range_len = range.len();
// 						if range_len >= len {
// 							match (strategy, result.clone()) {
// 								(AllocationStrategy::FirstFit, None) => return Some(range),
// 								(AllocationStrategy::WorstFit, Some(current_range)) if current_range.len() < range_len => result = Some(range),
// 								(AllocationStrategy::BestFit, Some(current_range)) if current_range.len() > range_len => result = Some(range),
// 								(_, None) => result = Some(range),
// 								_ => ()
// 							}
// 						}
// 					}
//
// 					left_item = right_item;
// 				}
//
// 				result
// 			},
// 			_ => unreachable!()
// 		}
// 	}
// }
