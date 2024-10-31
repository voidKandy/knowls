use std::cmp::Ordering;

// Heap or Priority Queue
// Weak ordered tree
// Min Heap: every child / grandchild is smaller
// Max Heap: every child / grandchild is larger
// Adjust tree on every insert/delete
// No traversal
#[derive(Debug)]
pub enum HeapError {
    LengthIsZero,
}

#[derive(Debug)]
pub struct MinHeap<T> {
    data: Vec<T>,
    length: usize,
}

impl<T: Ord + PartialEq + Copy + std::fmt::Debug> MinHeap<T> {
    pub fn new() -> Self {
        Self {
            data: vec![],
            length: 0,
        }
    }

    pub fn insert(&mut self, val: T) {
        self.data.push(val);
        self.heapify_up(self.length);
        self.length += 1;
    }

    pub fn delete(&mut self) -> Result<T, HeapError> {
        if self.length == 0 {
            return Err(HeapError::LengthIsZero);
        }
        let out = self.data.remove(0);
        self.length -= 1;

        if self.length == 0 {
            self.data = vec![];
            self.length = 0;
            return Ok(out);
        }
        self.heapify_down(0);
        Ok(out)
    }

    fn heapify_down(&mut self, idx: usize) {
        let (l_index, r_index) = (Self::left_child_idx(idx), Self::right_child_idx(idx));
        if idx >= self.length || l_index >= self.length {
            return;
        }
        let rval = self.data[r_index].clone();
        let lval = self.data[l_index].clone();
        let val = self.data[idx].clone();
        println!("DOWN L: {:?} R: {:?} V: {:?}", lval, rval, val);

        if lval > rval && val > rval {
            self.data[idx] = rval;
            self.data[r_index] = val;
            self.heapify_down(r_index);
        } else if rval > lval && val > lval {
            self.data[idx] = lval;
            self.data[l_index] = val;
            self.heapify_down(l_index);
        }
    }

    fn heapify_up(&mut self, idx: usize) {
        if idx == 0 {
            return;
        }
        let parent_idx = Self::parent_idx(idx);
        let parent_val = self.data[parent_idx];
        let val = self.data[idx];

        if parent_val > val {
            self.data[idx] = parent_val;
            self.data[parent_idx] = val;
            self.heapify_up(parent_idx)
        }
    }

    fn parent_idx(idx: usize) -> usize {
        // Numbers that don't evenly go into 2 return the division just without a remainder, not
        // floating point numbers
        (idx - 1) / 2
    }

    fn left_child_idx(idx: usize) -> usize {
        idx * 2 + 1
    }

    fn right_child_idx(idx: usize) -> usize {
        idx * 2 + 2
    }
}

mod tests {
    use super::MinHeap;

    #[test]
    fn heap_works() {
        let mut heap: MinHeap<i32> = MinHeap::new();
        heap.insert(32);
        heap.insert(42);
        heap.insert(82);
        heap.insert(2);
        heap.insert(0);
        heap.insert(9);
        heap.insert(33);
        println!("{:?}", heap);
        assert_eq!(heap.length, 7);
        assert_eq!(heap.delete().unwrap(), 0);
        assert_eq!(heap.length, 6);
        assert_eq!(heap.delete().unwrap(), 2);
    }
}
