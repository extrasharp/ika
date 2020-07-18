use std::{
    ops::{
        Deref,
        DerefMut,
    },
    cell::RefCell,
};

// TODO write tests

pub trait Recyclable: Default {
    fn reset(&mut self) {
        *self = Default::default();
    }
}

// TODO impl Recyclable on stdlib types

// impl<T: Default> Recyclable for T {}

pub struct PoolObject<'a, T: Recyclable> {
    pool: &'a Pool<T>,
    obj: *mut T,
}

impl<'a, T: Recyclable> Deref for PoolObject<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { & *self.obj }
    }
}

impl<'a, T: Recyclable> DerefMut for PoolObject<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.obj }
    }
}

impl<'a, T: Recyclable> Drop for PoolObject<'a, T> {
    fn drop(&mut self) {
        self.pool.dead.borrow_mut().push(self.obj);
    }
}

pub struct Pool<T: Recyclable> {
    _data: Vec<T>,
    dead: RefCell<Vec<*mut T>>,
}

impl<T: Recyclable> Pool<T> {
    pub fn new(size: usize) -> Self {
        let mut data: Vec<T> = (0..size).map(|_| Default::default())
                                        .collect();

        let start = data.as_mut_ptr();
        let mut dead = Vec::with_capacity(size);
        for i in 0..size {
            dead.push(unsafe { start.add(i) });
        }
        Self {
            _data: data,
            dead: RefCell::new(dead),
        }
    }

    pub fn take<'a>(&'a self) -> Option<PoolObject<'a, T>> {
        Some(PoolObject {
            pool: self,
            obj: self.dead.borrow_mut().pop()?,
        })
    }

    pub fn take_new<'a>(&'a self) -> Option<PoolObject<'a, T>> {
        let mut obj = self.take()?;
        obj.reset();
        Some(obj)
    }

    pub fn available(&self) -> usize {
        self.dead.borrow().len()
    }
}
