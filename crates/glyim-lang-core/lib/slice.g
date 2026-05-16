//! Slice-related types and operations for the Glyim core library.

/// Extension methods for slice types.
impl<T> [T] {
    /// Returns the number of elements in the slice.
    fn len(&self) -> usize {
        // compiler intrinsic
    }

    /// Returns `true` if the slice has a length of 0.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the first element of the slice, or `None` if it is empty.
    fn first(&self) -> Option<&T> {
        if self.is_empty() {
            Option::None
        } else {
            Option::Some(&self[0])
        }
    }

    /// Returns the first and all the rest of the elements of the slice.
    fn split_first(&self) -> Option<(&T, &[T])> {
        if self.is_empty() {
            Option::None
        } else {
            Option::Some((&self[0], &self[1..]))
        }
    }

    /// Returns the last element of the slice, or `None` if it is empty.
    fn last(&self) -> Option<&T> {
        if self.is_empty() {
            Option::None
        } else {
            Option::Some(&self[self.len() - 1])
        }
    }

    /// Returns an element or subslice depending on the type of index.
    fn get(&self, index: usize) -> Option<&T> {
        if index < self.len() {
            Option::Some(&self[index])
        } else {
            Option::None
        }
    }

    /// Swaps two elements in the slice.
    fn swap(&mut self, a: usize, b: usize) {
        let tmp = self[a];
        self[a] = self[b];
        self[b] = tmp;
    }

    /// Reverses the order of elements in the slice, in place.
    fn reverse(&mut self) {
        let mut i = 0;
        let mut j = self.len();
        if j == 0 { return; }
        j -= 1;
        while i < j {
            self.swap(i, j);
            i += 1;
            if j == 0 { break; }
            j -= 1;
        }
    }

    /// Returns `true` if the slice contains an element with the given value.
    fn contains(&self, x: &T) -> bool where T: PartialEq {
        for item in self {
            if item == x {
                return true;
            }
        }
        false
    }

    /// Fills the slice with an element.
    fn fill(&mut self, value: T) where T: Clone {
        for item in self {
            *item = value.clone();
        }
    }

    /// Sorts the slice.
    fn sort(&mut self) where T: Ord {
        // simple insertion sort for now
        let len = self.len();
        for i in 1..len {
            let mut j = i;
            while j > 0 && self[j - 1] > self[j] {
                self.swap(j - 1, j);
                j -= 1;
            }
        }
    }
}
