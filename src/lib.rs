use std::{
    ptr,
    marker::PhantomData,
};

// TODO
//   write tests
//   usage documentation
//   impl Recyclable on stdlib types
//   make a threadsafe version
//     think you can just wrap it in an Arc
//   resizable ?
//     breaks everything
//     possible if you use handles, but that could make the iterators weird
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
//   see comments
//   BaseIter is safe
//     start and end should always fit the data
//     any &T's taken from the iter should point to valid data
//   Pool is safe
//     turning the *mut into &mut in .get_ptr_as_mut_ref is safe
//       because its always pointing to valid data
//       you can never have two &mut's to the same location
//         ptrs in pool.ptrs are only ever swapped
//       also, rust compiler is more strict than necessary,
//         because spawn and reclaim are &mut self
//           even if spawn returned a duplicate ref, rust wouldnt let you have two &mut to the pool

struct BaseIter<'a, T: Default> {
    start: *const *mut T,
    end: *const *mut T,
    _phantom: PhantomData<&'a [T]>
}

impl<'a, T: Default> BaseIter<'a, T> {
    /// Create a new BaseIter, will iterate over `ptrs` until `ptrs[alive_ct - 1]`
    /// `alive_ct > ptrs.len()` is undefined.
    unsafe fn new(ptrs: &[*mut T], alive_ct: usize) -> Self {
        let start = ptrs.as_ptr();
        let end = start.add(alive_ct);
        Self {
            start,
            end,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T: Default> Iterator for BaseIter<'a, T> {
    type Item = *mut T;

    fn next(&mut self) -> Option<Self::Item> {
        if ptr::eq(self.start, self.end) {
            None
        } else {
            let ret = unsafe { *self.start };
            self.start = unsafe { self.start.add(1) };
            Some(ret)
        }
    }
}

/// Immutable pool iterator.
pub struct Iter<'a, T: Default> {
    base: BaseIter<'a, T>,
}

impl<'a, T: Default> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let ptr = self.base.next()?;
        Some(unsafe { & *ptr })
    }
}

/// Mutable pool iterator.
pub struct IterMut<'a, T: Default> {
    base: BaseIter<'a, T>,
}

impl<'a, T: Default> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        let ptr = self.base.next()?;
        Some(unsafe { &mut *ptr })
    }
}

/// Let's go swimming!
pub struct Pool<T: Default> {
    _data: Vec<T>,
    ptrs: Vec<*mut T>,
    alive_ct: usize,
}

impl<T: Default> Pool<T> {
    /// Create a new pool with a maximum capacity of `size`.
    pub fn new(size: usize) -> Self {
        let mut data: Vec<T> = Vec::with_capacity(size);
        for _ in 0..size {
            data.push(T::default());
        }

        let start = data.as_mut_ptr();
        let mut ptrs = Vec::with_capacity(size);
        for i in 0..size {
            // note: safe, see rust docs for ptr.add
            ptrs.push(unsafe { start.add(i) });
        }

        Self {
            _data: data,
            ptrs,
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

    /// Get an &mut T from a *mut T found at `ptrs[at]`.
    /// Unchecked.
    /// Safe as long as `at` is within bounds of `self.ptr`.
    #[inline]
    unsafe fn get_ptr_as_mut_ref(&mut self, at: usize) -> &mut T {
        // safe as long as:
        //   at is within bounds
        //   all ptrs in self.ptrs point to valid data
        &mut **self.ptrs.get_unchecked(at)
    }

    /// Instantiate an object.
    /// Undefined behavior if `self.is_empty()`.
    /// Object may have weird data, but it will have at least been initialized with `T::default()`.
    pub unsafe fn spawn_unchecked(&mut self) -> &mut T {
        let at = self.alive_ct;
        self.alive_ct += 1;
        self.get_ptr_as_mut_ref(at)
    }

    /// Kill objects in the pool based on `kill_fn`.
    /// If `kill_fn` returns true, the object will be recycled.
    pub fn reclaim<F: FnMut(&T) -> bool>(&mut self, kill_fn: F) {
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
            if kill_fn(unsafe { self.get_ptr_as_mut_ref(i) }) {
                alive_ct -= 1;
                self.ptrs.swap(i, alive_ct);
            }
            i += 1;
        }
        self.alive_ct = alive_ct;
    }

    /// Returns an iterator over the pool.
    pub fn iter(&self) -> Iter<T> {
        let base = unsafe { BaseIter::new(&self.ptrs, self.alive_ct) };
        Iter {
            base,
        }
    }

    /// Returns a mutable iterator over the pool.
    pub fn iter_mut(&mut self) -> IterMut<T> {
        let base = unsafe { BaseIter::new(&self.ptrs, self.alive_ct) };
        IterMut {
            base,
        }
    }

    /// Sort pointers to available objects for better cache locality.
    pub fn sort_the_dead(&mut self) {
        if self.available() >= 2 {
            self.ptrs[self.alive_ct..].sort_unstable();
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
        self.ptrs.len() - self.alive_ct
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
