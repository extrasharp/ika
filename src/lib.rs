use std::{
    slice,
    fmt::{
        self,
    },
};

// TODO
//   write tests
//   usage documentation
//   make a threadsafe version
//     think you can just wrap it in an Arc
//  handle ZSTs

// Saftey
// the main points of unsafety are:
//   making sure pool.offsets doesnt have two items that point to the same T in data
//   making sure pool.offsets contains valid offsets that dont go over data.size()
//     first solved by only ever swapping offsets
//     both solved by initing it with 0..size
//   detach and attach which change indices
//     attach just adds obj onto the end of data and pust that index wherever in offsets
//     detach has to move all the offsets above the one that was removed down

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
#[derive(Eq, Clone, Default)]
pub struct Pool<T> {
    data: Vec<T>,
    offsets: Vec<usize>,
    alive_ct: usize,
}

impl<T> Pool<T> {
    /// Instantiate an object.
    /// Undefined behavior if `self.is_empty()`.  
    /// Object may have weird, possibly uninitialized, data.  
    pub unsafe fn spawn_unchecked(&mut self) -> &mut T {
        let at = self.alive_ct;
        self.alive_ct += 1;
        self.get_at_offset_mut(at)
    }

    /// Instantiate an object.
    /// Will return None if `self.is_empty()`.  
    /// Object may have weird, possibly uninitialized, data.  
    #[inline]
    pub fn spawn(&mut self) -> Option<&mut T> {
        if self.is_empty() {
            None
        } else {
            Some(unsafe { self.spawn_unchecked() })
        }
    }

    //

    /// Instantiate exactly `count` objects.
    /// If `count > self.available()`, the resulting vector will be empty.  
    /// Object may have weird, possibly uninitialized, data.  
    pub fn spawn_exact(&mut self, count: usize) -> Vec<&mut T> {
        use std::mem;

        if count > self.available() {
            Vec::new()
        } else {
            let offsets = &self.offsets[self.alive_ct..self.alive_ct + count];
            self.alive_ct += count;

            let mut ret = Vec::with_capacity(count);
            for offset in offsets {
                ret.push(unsafe {
                    mem::transmute(self.data.get_unchecked_mut(*offset))
                });
            }
            ret
        }
    }

    /// Instantiate some objects.
    /// If `count > self.available()`, the resulting vector will have a length < count, and may be empty.  
    /// Object may have weird, possibly uninitialized, data.  
    pub fn spawn_some(&mut self, count: usize) -> Vec<&mut T> {
        use std::cmp;
        self.spawn_exact(cmp::min(count, self.available()))
    }

    //

    /// Kill objects in the pool based on `kill_fn`.
    /// If `kill_fn` returns true, the object will be recycled.  
    /// Preserves ordering of the pool.  
    pub fn reclaim<F: FnMut(&T) -> bool>(&mut self, mut kill_fn: F) {
        // safe because:
        //   i goes from 0 to alive_ct

        let len = self.alive_ct;
        let mut del = 0;
        for i in 0..len {
            if kill_fn(unsafe { self.get_at_offset(i) }) {
                del += 1;
            } else if del > 0 {
                self.offsets.swap(i, i - del);
            }
        }
        self.alive_ct -= del;
    }

    /// Kill objects in the pool based on `kill_fn`.
    /// If `kill_fn` returns true, the object will be recycled.  
    /// Doesn't necessarity preserve ordering of the pool.  
    pub fn reclaim_unstable<F: FnMut(&T) -> bool>(&mut self, mut kill_fn: F) {
        // safe because:
        //   alive_ct can never go below zero
        //   i can never go above alive_ct

        let mut len = self.alive_ct;
        let mut i = 0;
        while i < len {
            if kill_fn(unsafe { self.get_at_offset(i) }) {
                len -= 1;
                self.offsets.swap(i, len);
            }
            i += 1;
        }
        self.alive_ct = len;
    }

    //

    /// Move an object to the end of the pool.
    /// Will resize the pool.  
    pub fn attach(&mut self, at: usize, obj: T) {
        if at > self.alive_ct {
            panic!("index out of bounds");
        }

        self.offsets.insert(at, self.data.len());
        self.alive_ct += 1;
        self.data.push(obj);
    }

    /// Move an object out of the pool by index.
    /// Will resize the pool.  
    /// Panics if index is out of bounds...
    pub fn detach(&mut self, at: usize) -> T {
        if at >= self.alive_ct {
            panic!("index out of bounds");
        }

        let data_idx = unsafe {
            self.offset_to_data_idx(at)
        };

        let ret = self.data.remove(data_idx);
        self.offsets.remove(at);

        if at != self.alive_ct - 1 {
            for offset in &mut self.offsets {
                if *offset > data_idx {
                    *offset -= 1;
                }
            }
        }

        self.alive_ct -= 1;
        ret
    }

    //

    /// Turns an offset into an index into the data vec.  
    /// Safe as long as `at` is within bounds of `self.offsets`.  
    #[inline]
    unsafe fn offset_to_data_idx(&self, at: usize) -> usize {
        *self.offsets.get_unchecked(at)
    }

    /// Get a &T from the offset found at `offsets[at]`.
    /// Safe as long as `at` is within bounds of `self.offsets`.  
    #[inline]
    unsafe fn get_at_offset(&self, at: usize) -> &T {
        self.data.get_unchecked(self.offset_to_data_idx(at))
    }

    /// Get a &mut T from the offset found at `offsets[at]`.
    /// Safe as long as `at` is within bounds of `self.offsets`.  
    #[inline]
    unsafe fn get_at_offset_mut(&mut self, at: usize) -> &mut T {
        let idx = self.offset_to_data_idx(at);
        self.data.get_unchecked_mut(idx)
    }

    pub fn get(&self, at: usize) -> Option<&T> {
        if at < self.alive_ct {
            Some(unsafe { self.get_at_offset(at) })
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, at: usize) -> Option<&mut T> {
        if at < self.alive_ct {
            Some(unsafe { self.get_at_offset_mut(at) })
        } else {
            None
        }
    }

    //

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

    //

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

    /// Number of objects in use in the pool.
    #[inline]
    pub fn len(&self) -> usize {
        self.alive_ct
    }

    /// Number of free objects in the pool.
    #[inline]
    pub fn available(&self) -> usize {
        self.offsets.len() - self.alive_ct
    }

    /// Number of total objects in the pool.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.offsets.len()
    }
}

impl<T: Default> Pool<T> {
    /// Create a new pool with a starting size of `size`.
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

    //

    /// Please instantiate an object.
    /// Will resize the pool if `self.is_empty()`.  
    /// Intializes new object with `T::default()`.  
    #[inline]
    pub fn please_spawn(&mut self) -> &mut T {
        if self.is_empty() {
            self.offsets.push(self.data.len());
            self.data.push(T::default());
        }
        unsafe { self.spawn_unchecked() }
    }

    //

    /// Instantiate exactly `count` objects.
    /// If `count > self.available()`, the pool will be resized to account for the difference.  
    /// Intializes new objects with `T::default()`.  
    pub fn please_spawn_some(&mut self, count: usize) -> Vec<&mut T> {
        if count > self.available() {
            let to_add = count - self.available();
            self.offsets.reserve(to_add);
            self.data.reserve(to_add);
            for _ in 0..to_add {
                self.offsets.push(self.data.len());
                self.data.push(T::default());
            }
        }
        self.spawn_exact(count)
    }
}

impl<'a, T> IntoIterator for &'a Pool<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut Pool<T> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: fmt::Debug> fmt::Debug for Pool<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut lst = f.debug_list();
        for obj in self.iter() {
            lst.entry(obj);
        }
        lst.finish()
    }
}

impl<T: PartialEq> PartialEq for Pool<T> {
    fn eq(&self, other: &Self) -> bool {
        let are_alive_equal =
            self.iter().zip(other.iter())
                .all(| (s, o) | {
                    s == o
                });
        // TODO does capacity need to be equal for two pools to be equal?
        self.len() == other.len() && are_alive_equal
    }
}
