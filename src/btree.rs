use btree_slab::{
	BTreeMap,
	generic::{
		map::{
			BTreeExt
		},
		node::{
			Address as ItemAddr,
			Node
		},
	},
	utils::binary_search_min
};
use crate::{
	Address,
	AllocationStrategy,
};

pub(crate) trait BTreeFreeMap<T> {
	fn address_of_free_range(&self, len: T, strategy: AllocationStrategy) -> Option<(ItemAddr, T)>;

	fn address_of_free_range_in(&self, id: usize, len: T, strategy: AllocationStrategy) -> Option<(ItemAddr, T)>;
}

impl<T: Address> BTreeFreeMap<T> for BTreeMap<T, usize> {
	#[inline]
	fn address_of_free_range(&self, len: T, strategy: AllocationStrategy) -> Option<(ItemAddr, T)> {
		match self.root_id() {
			Some(id) => self.address_of_free_range_in(id, len, strategy),
			None => None
		}
	}

	#[inline]
	fn address_of_free_range_in(&self, mut id: usize, len: T, strategy: AllocationStrategy) -> Option<(ItemAddr, T)> {
		loop {
			match free_range_offset_in(self.node(id), len, strategy) {
				Ok((offset, actual_len)) => {
					return Some((ItemAddr::new(id, offset.into()), actual_len))
				},
				Err(None) => {
					return None
				},
				Err(Some(child_id)) => {
					id = child_id;
				}
			}
		}
	}
}

fn free_range_offset_in<T>(node: &Node<T, usize>, len: T, strategy: AllocationStrategy) -> Result<(usize, T), Option<usize>> where T: Address {
	match node {
		Node::Internal(node) => {
			let branches = node.branches();
			match strategy {
				AllocationStrategy::FirstFit => {
					panic!("TODO")
				},
				AllocationStrategy::WorstFit => {
					let i = branches.len() - 1;
					Err(Some(branches[i].child))
				},
				AllocationStrategy::BestFit => {
					match binary_search_min(branches, &len) {
						Some(i) => {
							let b = &branches[i];
							if b.item.key() == &len {
								Ok((i, *b.item.key()))
							} else {
								Err(Some(b.child))
							}
						},
						None => {
							Ok((0, *branches[0].item.key()))
						}
					}
				}
			}
		},
		Node::Leaf(leaf) => {
			let items = leaf.items();
			match strategy {
				AllocationStrategy::FirstFit => {
					match binary_search_min(items, &len) {
						Some(i) => {
							let item = &items[i];
							if item.key() == &len {
								Ok((i, *item.key()))
							} else {
								let j = i+1;
								if j < items.len() {
									Ok((j, *items[j].key()))
								} else {
									Err(None)
								}
							}
						},
						None => Ok((0, *items[0].key()))
					}
				},
				AllocationStrategy::WorstFit => {
					let i = items.len() - 1;
					match items.get(i) {
						Some(item) if *item.key() >= len => Ok((i, *item.key())),
						_ => Err(None)
					}
				},
				AllocationStrategy::BestFit => {
					match binary_search_min(items, &len) {
						Some(i) => {
							let item = &items[i];
							if item.key() == &len {
								Ok((i, *item.key()))
							} else {
								let j = i+1;
								if j < items.len() {
									Ok((j, *items[j].key()))
								} else {
									Err(None)
								}
							}
						},
						None => Ok((0, *items[0].key()))
					}
				}
			}
		}
	}
}
