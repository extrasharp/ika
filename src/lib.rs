use std::{
    slice,
};

// TODO
//   write tests
//   usage documentation
//   impl Recyclable on stdlib types
//   make a threadsafe version
//     think you can just wrap it in an Arc
//   attach, detach
//     attach just spawns one and copies/moves a T into it
//     detach would have to be done like reclaim?
//       unless you do detach_first or something idk
//     guarantee order of alive objs? until you reclam at least
//       reclaim vs reclaim_unstable
//       no guarantees about order of the dead
//  into_iter, common derives, clone, eq and stuff
//  handle ZSTs

// Saftey
// the main points of unsafety are:
//   making sure pool.offsets doesnt have two items that point to the same T in data
//   making sure pool.offsets contains valid offsets that dont go over data.size()
//     first solved by only ever swapping offsets
//     both solved by initing it with 0..size

/// Immutable pool iterator.
pub struct Iter<'a, T> {
    data: &'a Vec<T>,
    iter: slice::Iter<'a, usize>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let offset = self.iter.next()?;
        unsafe {
            Some(self.data.get_unchecked(*offset))
        }
    }
}

/// Mutable pool iterator.
pub struct IterMut<'a, T> {
    data: &'a mut Vec<T>,
    iter: slice::Iter<'a, usize>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        use std::mem;

        let offset = self.iter.next()?;
        unsafe {
            Some(mem::transmute(self.data.get_unchecked_mut(*offset)))
        }
    }
}

/// Let's go swimming!
pub struct Pool<T: Default> {
    data: Vec<T>,
    offsets: Vec<usize>,
    alive_ct: usize,
}

impl<T: Default> Pool<T> {
    /// Create a new pool with a maximum capacity of `size`.
    pub fn new(size: usize) -> Self {
        let mut data: Vec<T> = Vec::with_capacity(size);
        for _ in 0..size {
            data.push(T::default());
        }

        let mut offsets = Vec::with_capacity(size);
        for i in 0..size {
            offsets.push(i);
        }

        Self {
            data,
            offsets,
            alive_ct: 0,
        }
    }

    /// Instantiate an object.
    /// Will return None if `self.is_empty()`.
    /// Object may have weird data, but it will have at least been initialized with `T::default()`.
    #[inline]
    pub fn spawn(&mut self) -> Option<&mut T> {
        if self.is_empty() {
            None
        } else {
            Some(unsafe { self.spawn_unchecked() })
        }
    }

    /// Please instantiate an object.
    /// May allocate and resize the pool.
    /// Object may have weird data, but it will have at least been initialized with `T::default()`.
    #[inline]
    pub fn please_spawn(&mut self) -> &mut T {
        if self.is_empty() {
            self.offsets.push(self.data.len());
            self.data.push(T::default());
        }
        unsafe { self.spawn_unchecked() }
    }

    /// Get an &mut T from the offset found at `offsets[at]`.
    /// Unchecked.
    /// Safe as long as `at` is within bounds of `self.offsets`.
    #[inline]
    unsafe fn get_at_offset(&mut self, at: usize) -> &mut T {
        self.data.get_unchecked_mut(self.offsets[at])
    }

    /// Instantiate an object.
    /// Undefined behavior if `self.is_empty()`.
    /// Object may have weird data, but it will have at least been initialized with `T::default()`.
    pub unsafe fn spawn_unchecked(&mut self) -> &mut T {
        let at = self.alive_ct;
        self.alive_ct += 1;
        self.get_at_offset(at)
    }

    /// Kill objects in the pool based on `kill_fn`.
    /// If `kill_fn` returns true, the object will be recycled.
    pub fn reclaim<F: FnMut(&T) -> bool>(&mut self, mut kill_fn: F) {
        // safe because:
        //   alive_ct can never go below zero
        //   i can never go above alive_ct
        //   alive_ct only ever goes down, i only ever goes up
        let mut alive_ct = self.alive_ct;
        let mut i = 0;
        loop {
            if i >= alive_ct {
                break;
            }
            if kill_fn(unsafe { self.get_at_offset(i) }) {
                alive_ct -= 1;
                self.offsets.swap(i, alive_ct);
            }
            i += 1;
        }
        self.alive_ct = alive_ct;
    }

    /// Returns an iterator over the pool.
    pub fn iter(&self) -> Iter<T> {
        Iter {
            data: &self.data,
            iter: (&self.offsets[..self.alive_ct]).iter(),
        }
    }

    /// Returns an iterator over the pool.
    pub fn iter_mut(&mut self) -> IterMut<T> {
        IterMut {
            data: &mut self.data,
            iter: (&self.offsets[..self.alive_ct]).iter(),
        }
    }

    /// Sort pointers to available objects for better cache locality.
    pub fn sort_the_dead(&mut self) {
        if self.available() >= 2 {
            self.offsets[self.alive_ct..].sort_unstable();
        }
    }

    /// Returns whether there are available objects in the pool or not.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.available() == 0
    }

    /// Number of free objects in the pool.
    #[inline]
    pub fn available(&self) -> usize {
        self.offsets.len() - self.alive_ct
    }
}

/// A trait to simplify initializing objects taken from the pool.
pub trait Recyclable: Default {
    /// Reset the object.
    fn reset(&mut self);
}

impl<T: Recyclable> Pool<T> {
    /// Instantiate an object.
    /// Will return None if `self.is_empty()`.
    /// Object will be reset based on its implementation of Recyclable.
    pub fn spawn_new(&mut self) -> Option<&mut T> {
        let obj = self.spawn()?;
        obj.reset();
        Some(obj)
    }

    /// Instantiate an object.
    /// Undefined behavior if `self.is_empty()`.
    /// Object will be reset based on its implementation of Recyclable.
    pub unsafe fn spawn_new_unchecked(&mut self) -> &mut T {
        let obj = self.spawn_unchecked();
        obj.reset();
        obj
    }
}
