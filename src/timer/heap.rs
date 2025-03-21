//! A simple binary heap with support for removal of arbitrary elements
//!
//! This heap is used to manage timer state in the event loop. All timeouts go
//! into this heap and we also cancel timeouts from this heap. The crucial
//! feature of this heap over the standard library's `BinaryHeap` is the ability
//! to remove arbitrary elements. (e.g. when a timer is canceled)
//!
//! Note that this heap is not at all optimized right now, it should hopefully
//! just work.

use std::mem;

pub struct Heap<T> {
    // Binary heap of items, plus the slab index indicating what position in the
    // list they're in.
    items: Vec<(T, usize)>,

    // A map from a slab index (assigned to an item above) to the actual index
    // in the array the item appears at.
    index: Vec<SlabSlot<usize>>,
    next_index: usize,
}

enum SlabSlot<T> {
    Empty { next: usize },
    Full { value: T },
}

pub struct Slot {
    idx: usize,
}

impl<T: Ord> Heap<T> {
    pub fn new() -> Heap<T> {
        Heap {
            items: Vec::new(),
            index: Vec::new(),
            next_index: 0,
        }
    }

    /// Pushes an element onto this heap, returning a slot token indicating
    /// where it was pushed on to.
    ///
    /// The slot can later get passed to `remove` to remove the element from the
    /// heap, but only if the element was previously not removed from the heap.
    pub fn push(&mut self, t: T) -> Slot {
        self.assert_consistent();
        let len = self.items.len();
        let slot = SlabSlot::Full { value: len };
        let slot_idx = if self.next_index == self.index.len() {
            self.next_index += 1;
            self.index.push(slot);
            self.index.len() - 1
        } else {
            match mem::replace(&mut self.index[self.next_index], slot) {
                SlabSlot::Empty { next } => mem::replace(&mut self.next_index, next),
                SlabSlot::Full { .. } => panic!(),
            }
        };
        self.items.push((t, slot_idx));
        self.percolate_up(len);
        self.assert_consistent();
        Slot { idx: slot_idx }
    }

    pub fn peek(&self) -> Option<&T> {
        self.assert_consistent();
        self.items.first().map(|i| &i.0)
    }

    pub fn pop(&mut self) -> Option<T> {
        self.assert_consistent();
        if self.items.is_empty() {
            return None;
        }
        let slot = Slot {
            idx: self.items[0].1,
        };
        Some(self.remove(slot))
    }

    pub fn remove(&mut self, slot: Slot) -> T {
        self.assert_consistent();
        let empty = SlabSlot::Empty {
            next: self.next_index,
        };
        let idx = match mem::replace(&mut self.index[slot.idx], empty) {
            SlabSlot::Full { value } => value,
            SlabSlot::Empty { .. } => panic!(),
        };
        self.next_index = slot.idx;
        let (item, slot_idx) = self.items.swap_remove(idx);
        debug_assert_eq!(slot.idx, slot_idx);
        if idx < self.items.len() {
            set_index(&mut self.index, self.items[idx].1, idx);
            if self.items[idx].0 < item {
                self.percolate_up(idx);
            } else {
                self.percolate_down(idx);
            }
        }
        self.assert_consistent();
        item
    }

    fn percolate_up(&mut self, mut idx: usize) -> usize {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.items[idx].0 >= self.items[parent].0 {
                break;
            }
            let (a, b) = self.items.split_at_mut(idx);
            mem::swap(&mut a[parent], &mut b[0]);
            set_index(&mut self.index, a[parent].1, parent);
            set_index(&mut self.index, b[0].1, idx);
            idx = parent;
        }
        idx
    }

    fn percolate_down(&mut self, mut idx: usize) -> usize {
        loop {
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;

            let mut swap_left = true;
            match (self.items.get(left), self.items.get(right)) {
                (Some(left), None) => {
                    if left.0 >= self.items[idx].0 {
                        break;
                    }
                }
                (Some(left), Some(right)) => {
                    if left.0 < self.items[idx].0 {
                        if right.0 < left.0 {
                            swap_left = false;
                        }
                    } else if right.0 < self.items[idx].0 {
                        swap_left = false;
                    } else {
                        break;
                    }
                }

                (None, None) => break,
                (None, Some(_right)) => panic!("not possible"),
            }

            let (a, b) = if swap_left {
                self.items.split_at_mut(left)
            } else {
                self.items.split_at_mut(right)
            };
            mem::swap(&mut a[idx], &mut b[0]);
            set_index(&mut self.index, a[idx].1, idx);
            set_index(&mut self.index, b[0].1, a.len());
            idx = a.len();
        }
        idx
    }

    fn assert_consistent(&self) {
        #[allow(unexpected_cfgs)]
        if !cfg!(assert_timer_heap_consistent) {
            return;
        }

        assert_eq!(
            self.items.len(),
            self.index
                .iter()
                .filter(|slot| {
                    match **slot {
                        SlabSlot::Full { .. } => true,
                        SlabSlot::Empty { .. } => false,
                    }
                })
                .count()
        );

        for (i, &(_, j)) in self.items.iter().enumerate() {
            let index = match self.index[j] {
                SlabSlot::Full { value } => value,
                SlabSlot::Empty { .. } => panic!(),
            };
            if index != i {
                panic!(
                    "self.index[j] != i : i={} j={} self.index[j]={}",
                    i, j, index
                );
            }
        }

        for (i, (item, _)) in self.items.iter().enumerate() {
            if i > 0 {
                assert!(*item >= self.items[(i - 1) / 2].0, "bad at index: {i}");
            }
            if let Some(left) = self.items.get(2 * i + 1) {
                assert!(*item <= left.0, "bad left at index: {i}");
            }
            if let Some(right) = self.items.get(2 * i + 2) {
                assert!(*item <= right.0, "bad right at index: {i}");
            }
        }
    }
}

fn set_index<T>(slab: &mut [SlabSlot<T>], slab_slot: usize, val: T) {
    match slab[slab_slot] {
        SlabSlot::Full { ref mut value } => *value = val,
        SlabSlot::Empty { .. } => panic!(),
    }
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    use super::Heap;

    #[wasm_bindgen_test]
    fn simple() {
        let mut h = Heap::new();
        h.push(1);
        h.push(2);
        h.push(8);
        h.push(4);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(4));
        assert_eq!(h.pop(), Some(8));
        assert_eq!(h.pop(), None);
        assert_eq!(h.pop(), None);
    }

    #[wasm_bindgen_test]
    fn simple2() {
        let mut h = Heap::new();
        h.push(5);
        h.push(4);
        h.push(3);
        h.push(2);
        h.push(1);
        assert_eq!(h.pop(), Some(1));
        h.push(8);
        assert_eq!(h.pop(), Some(2));
        h.push(1);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(3));
        assert_eq!(h.pop(), Some(4));
        h.push(5);
        assert_eq!(h.pop(), Some(5));
        assert_eq!(h.pop(), Some(5));
        assert_eq!(h.pop(), Some(8));
    }

    #[wasm_bindgen_test]
    fn remove() {
        let mut h = Heap::new();
        h.push(5);
        h.push(4);
        h.push(3);
        let two = h.push(2);
        h.push(1);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.remove(two), 2);
        h.push(1);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(3));
    }

    fn vec2heap<T: Ord>(v: Vec<T>) -> Heap<T> {
        let mut h = Heap::new();
        for t in v {
            h.push(t);
        }
        return h;
    }

    #[wasm_bindgen_test]
    fn test_peek_and_pop() {
        let data = vec![2, 4, 6, 2, 1, 8, 10, 3, 5, 7, 0, 9, 1];
        let mut sorted = data.clone();
        sorted.sort();
        let mut heap = vec2heap(data);
        while heap.peek().is_some() {
            assert_eq!(heap.peek().unwrap(), sorted.first().unwrap());
            assert_eq!(heap.pop().unwrap(), sorted.remove(0));
        }
    }

    #[wasm_bindgen_test]
    fn test_push() {
        let mut heap = Heap::new();
        heap.push(-2);
        heap.push(-4);
        heap.push(-9);
        assert!(*heap.peek().unwrap() == -9);
        heap.push(-11);
        assert!(*heap.peek().unwrap() == -11);
        heap.push(-5);
        assert!(*heap.peek().unwrap() == -11);
        heap.push(-27);
        assert!(*heap.peek().unwrap() == -27);
        heap.push(-3);
        assert!(*heap.peek().unwrap() == -27);
        heap.push(-103);
        assert!(*heap.peek().unwrap() == -103);
    }

    fn check_to_vec(mut data: Vec<i32>) {
        let mut heap = Heap::new();
        for data in data.iter() {
            heap.push(*data);
        }
        data.sort();
        let mut v = Vec::new();
        while let Some(i) = heap.pop() {
            v.push(i);
        }
        assert_eq!(v, data);
    }

    #[wasm_bindgen_test]
    fn test_to_vec() {
        check_to_vec(vec![]);
        check_to_vec(vec![5]);
        check_to_vec(vec![3, 2]);
        check_to_vec(vec![2, 3]);
        check_to_vec(vec![5, 1, 2]);
        check_to_vec(vec![1, 100, 2, 3]);
        check_to_vec(vec![1, 3, 5, 7, 9, 2, 4, 6, 8, 0]);
        check_to_vec(vec![2, 4, 6, 2, 1, 8, 10, 3, 5, 7, 0, 9, 1]);
        check_to_vec(vec![9, 11, 9, 9, 9, 9, 11, 2, 3, 4, 11, 9, 0, 0, 0, 0]);
        check_to_vec(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        check_to_vec(vec![10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]);
        check_to_vec(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 0, 0, 1, 2]);
        check_to_vec(vec![5, 4, 3, 2, 1, 5, 4, 3, 2, 1, 5, 4, 3, 2, 1]);
    }

    #[wasm_bindgen_test]
    fn test_empty_pop() {
        let mut heap = Heap::<i32>::new();
        assert!(heap.pop().is_none());
    }

    #[wasm_bindgen_test]
    fn test_empty_peek() {
        let empty = Heap::<i32>::new();
        assert!(empty.peek().is_none());
    }
}
