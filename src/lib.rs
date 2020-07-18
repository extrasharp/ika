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

// Saftey
//   iterators are safe
//     start and end always fit the data
//     any &T's taken from the iter point to valid data
//   pool is safe
//     see comments
//     turning the *mut into &mut in spawn and reclaim { kill_fn } is safe
//       because its always pointing to valid data
//       also, the pointers can only be in either dead or alive
//         wont ever have an 2 &mut to the same location
//       also, rust compiler is more strict than necessary,
//         because spawn and reclaim are &mut self

/// Immutable pool iterator.
pub struct PoolIter<'a, T: Default> {
    start: *const *mut T,
    end: *const *mut T,
    _phantom: PhantomData<&'a [T]>
}

impl<'a, T: Default> Iterator for PoolIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let ret = if ptr::eq(self.start, self.end) {
                None
            } else {
                Some(& **self.start)
            };
            self.start = self.start.add(1);
            ret
        }
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
        unsafe {
            let ret = if ptr::eq(self.start, self.end) {
                None
            } else {
                Some(&mut **self.start)
            };
            self.start = self.start.add(1);
            ret
        }
    }
}

/// Let's go swimming!
pub struct Pool<T: Default> {
    _data: Vec<T>,
    dead: Vec<*mut T>,
    alive: Vec<*mut T>,
}

impl<T: Default> Pool<T> {
    /// Create a new pool with a maximum capacity of `size`.
    pub fn new(size: usize) -> Self {
        let mut data: Vec<T> = (0..size).map(|_| Default::default())
                                        .collect();

        let start = data.as_mut_ptr();
        let mut dead = Vec::with_capacity(size);
        for i in 0..data.len() {
            // note: safe, see rust docs for ptr.add
            dead.push(unsafe { start.add(i) });
        }

        let alive = Vec::with_capacity(size);

        Self {
            _data: data,
            dead,
            alive,
        }
    }

    /// Instantiate an object.
    /// Will return None if `self.available() == 0`.
    pub fn spawn(&mut self) -> Option<&mut T> {
        let ptr = self.dead.pop()?;
        self.alive.push(ptr);
        Some(unsafe { &mut *ptr })
    }

    /// Kill objects in the pool based on `kill_fn`.
    /// If `kill_fn` returns true, the object will be recycled.
    pub fn reclaim<F: Fn(&T) -> bool>(&mut self, kill_fn: F) {
        let len = self.alive.len();
        let mut del = 0;
        for i in 0..len {
            // note: safe because just going up to alive.len()
            let ptr = *unsafe { self.alive.get_unchecked(i) };
            if kill_fn(unsafe { &mut *ptr }) {
                self.dead.push(ptr);
                del += 1;
            } else if del > 0 {
                self.alive.swap(i, i - del);
            }
        }
        if del > 0 {
            self.alive.truncate(len - del);
        }
    }

    /// Returns an iterator over the pool.
    pub fn iter(&self) -> PoolIter<T> {
        let start = self.alive.as_ptr();
        // note: safe, see rust docs for ptr.add
        let end = unsafe { start.add(self.alive.len()) };
        PoolIter {
            start,
            end,
            _phantom: PhantomData,
        }
    }

    /// Returns a mutable iterator over the pool.
    pub fn iter_mut(&mut self) -> PoolIterMut<T> {
        let start = self.alive.as_mut_ptr();
        // note: safe, see rust docs for ptr.add
        let end = unsafe { start.add(self.alive.len()) };
        PoolIterMut {
            start,
            end,
            _phantom: PhantomData,
        }
    }

    /// Sort pointers to free objects for better cache locality.
    pub fn sort_the_dead(&mut self) {
        self.dead.sort_unstable();
    }

    /// Number of free objects in the pool.
    pub fn available(&self) -> usize {
        self.dead.len()
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
