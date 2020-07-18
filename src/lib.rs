use std::{
    ops::{
        Deref,
        DerefMut,
    },
    cell::RefCell,
};

// TODO write tests
//      impl Recyclable on stdlib types
//      make a threadsafe version

pub trait Recyclable: Default {
    fn reset(&mut self) {
        *self = Default::default();
    }
}

//

// PoolObject.obj is completely safe
//   always points to data in the pool
//   all pointers from the pool are unique
//   PoolObject must live as long as Pool
//   needs to be a ptr to fit PoolObject::drop
//     the PoolObject behind '&mut self' passed into drop will die afterwards
//     rust complains self.obj pushed into the dead objects outlives the `&mut self` borrow
//     which it does, in fact, do but. ya know.
// RefCell accesses are completely safe
//   RefCell is necessary so PoolObject doesnt need an &mut to the pool
//     otherwise you could only do one pool.take()
//     in this case its fine to have multiple mut refs to the pool
//       they only ever modify one thing, pool.data
//           and only do so for one funciton call
//         its not thread safe, but otherwise its fine
//       any 'two &mut's to the same data' issues are solved by PoolObject being safe
//     Pool.take() being &self is a bonus
//       because a pool is a basically a special memory allocator
//         even if take() does mutate the pool, its not //really// mutating it
//   could put a RefCell<Vec<*mut T>> in the PoolObject, but thats wierd
//     doesnt solve the problem and also youd need a PhantomData<&'a Pool<T>> anyway

//

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

//

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
