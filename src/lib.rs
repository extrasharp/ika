use std::{
    ptr,
    marker::PhantomData,
};

// TODO write tests
//      usage documentation
//      impl Recyclable on stdlib types
//      make a threadsafe version
//      resizable ?
//        breaks everything
//        possible if you use handles, but that could make the iterators weird

// Saftey
//   iterators are safe
//     start and end always fit the data
//     any &T's taken from the iter point to valid data
//   pool is safe
//     see comments
//     turning the *mut into &mut in spawn and reclaim { kill_fn } is safe
//       because its always pointing to valid data
//       you can never have two &mut's to the same location
//         ptrs in pool.ptrs are only ever swapped
//       also, rust compiler is more strict than necessary,
//         because spawn and reclaim are &mut self
//           even if spawn returned a duplicate ref, rust wouldnt let you have two &mut to the pool

/// Immutable pool iterator.
pub struct PoolIter<'a, T: Default> {
    start: *const *mut T,
    end: *const *mut T,
    _phantom: PhantomData<&'a [T]>
}

impl<'a, T: Default> Iterator for PoolIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = if ptr::eq(self.start, self.end) {
            return None;
        } else {
            unsafe {
                Some(& **self.start)
            }
        };
        self.start = unsafe { self.start.add(1) };
        ret
    }
}

/// Mutable pool iterator.
pub struct PoolIterMut<'a, T: Default> {
    start: *const *mut T,
    end: *const *mut T,
    _phantom: PhantomData<&'a [T]>
}

impl<'a, T: Default> Iterator for PoolIterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = if ptr::eq(self.start, self.end) {
            return None;
        } else {
            unsafe {
                Some(&mut **self.start)
            }
        };
        self.start = unsafe { self.start.add(1) };
        ret
    }
}

/// Let's go swimming!
pub struct Pool<T: Default> {
    _data: Vec<T>,
    ptrs: Vec<*mut T>,
    ptrs_split: usize,
}

impl<T: Default> Pool<T> {
    /// Create a new pool with a maximum capacity of `size`.
    pub fn new(size: usize) -> Self {
        let mut data: Vec<T> = (0..size).map(|_| Default::default())
                                        .collect();

        let start = data.as_mut_ptr();
        let mut ptrs = Vec::with_capacity(size);
        for i in 0..data.len() {
            // note: safe, see rust docs for ptr.add
            ptrs.push(unsafe { start.add(i) });
        }

        Self {
            _data: data,
            ptrs,
            ptrs_split: 0,
        }
    }

    /// Instantiate an object.
    /// Will return None if `self.available() == 0`.
    pub fn spawn(&mut self) -> Option<&mut T> {
        if self.available() == 0 {
            None
        } else {
            let ptr = *unsafe { self.ptrs.get_unchecked(self.ptrs_split) };
            self.ptrs_split += 1;
            Some(unsafe { &mut *ptr })
        }
    }

    /// Kill objects in the pool based on `kill_fn`.
    /// If `kill_fn` returns true, the object will be recycled.
    pub fn reclaim<F: Fn(&T) -> bool>(&mut self, kill_fn: F) {
        // safe because len can never go below zero
        //      and i can never go above self.ptrs_split
        //      len only ever goes down, i only ever goes up
        let mut len = self.ptrs_split;
        let mut i = 0;
        loop {
            if i >= len {
                break;
            }
            let ptr = *unsafe { self.ptrs.get_unchecked(i) };
            if kill_fn(unsafe { &mut *ptr }) {
                len -= 1;
                self.ptrs.swap(i, len);
            }
            i += 1;
        }
        self.ptrs_split = len;
    }

    /// Returns an iterator over the pool.
    pub fn iter(&self) -> PoolIter<T> {
        let start = self.ptrs.as_ptr();
        // note: safe, see rust docs for ptr.add
        let end = unsafe { start.add(self.ptrs_split) };
        PoolIter {
            start,
            end,
            _phantom: PhantomData,
        }
    }

    /// Returns a mutable iterator over the pool.
    pub fn iter_mut(&mut self) -> PoolIterMut<T> {
        let start = self.ptrs.as_ptr();
        // note: safe, see rust docs for ptr.add
        let end = unsafe { start.add(self.ptrs_split) };
        PoolIterMut {
            start,
            end,
            _phantom: PhantomData,
        }
    }

    /// Sort pointers to free objects for better cache locality.
    pub fn sort_the_dead(&mut self) {
        if self.available() >= 2 {
            self.ptrs[self.ptrs_split..].sort_unstable();
        }
    }

    /// Number of free objects in the pool.
    #[inline]
    pub fn available(&self) -> usize {
        self.ptrs.len() - self.ptrs_split
    }
}

/// A trait to simplify initializing objects taken from the pool.
pub trait Recyclable: Default {
    /// Reset the object.
    /// Defaults to `*self = Default::default()`.
    fn reset(&mut self) {
        *self = Default::default();
    }
}

impl<T: Recyclable> Pool<T> {
    /// Spawn an object.
    /// Object will be reset based on its implementation of Recyclable.
    pub fn spawn_new(&mut self) -> Option<&mut T> {
        let obj = self.spawn()?;
        obj.reset();
        Some(obj)
    }
}
