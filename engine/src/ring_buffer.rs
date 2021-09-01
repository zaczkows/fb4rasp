#[derive(Default, Clone)]
pub struct FixedRingBuffer<T> {
    data: Vec<T>,
    next: usize,
    len: usize,
}

impl<T: Clone> FixedRingBuffer<T> {
    pub fn new(size: usize, init: T) -> Self {
        let mut me = Self {
            data: Vec::with_capacity(size),
            next: 0,
            len: 0,
        };

        me.data.resize(size, init);
        me
    }
}

impl<T> FixedRingBuffer<T> {
    pub fn new_with<F>(size: usize, init: F) -> Self
    where
        F: FnMut() -> T,
    {
        let mut me = Self {
            data: Vec::with_capacity(size),
            next: 0,
            len: 0,
        };

        me.data.resize_with(size, init);
        me
    }

    pub fn add(&mut self, item: T) {
        self.data[self.next] = item;
        self.next += 1;
        if self.next >= self.data.len() {
            self.next = 0;
        }
        if self.len < self.data.len() {
            self.len += 1;
        }
    }

    pub fn last(&self) -> &T {
        self.item(-1)
    }

    pub fn size(&self) -> isize {
        self.len as isize
    }

    pub fn item(&self, no: isize) -> &T {
        let mut c = (self.next as isize) + no;
        while c < 0 {
            c += self.data.len() as isize;
        }

        c %= self.data.len() as isize;
        &self.data[c as usize]
    }

    pub fn remove(&mut self) {
        if self.len > 0 {
            self.len -= 1;
        }
    }
}

impl<'a, T> FixedRingBuffer<T> {
    pub fn iter(&'a self) -> FixedRingBufferIterator<'a, T> {
        FixedRingBufferIterator::new(self)
    }
}

pub struct FixedRingBufferIterator<'a, T> {
    buf: &'a FixedRingBuffer<T>,
    count: isize,
}

impl<'a, T> FixedRingBufferIterator<'a, T> {
    pub fn new(buf: &'a FixedRingBuffer<T>) -> Self {
        Self { buf, count: 0 }
    }
}

impl<'a, T> Iterator for FixedRingBufferIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let cnt = self.count;
        self.count += 1;

        if cnt >= self.buf.size() {
            None
        } else {
            Some(self.buf.item(cnt))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frb_get_items() {
        let mut frb: FixedRingBuffer<i32> = FixedRingBuffer::new(10, 0);
        assert_eq!(0, *frb.last());
        assert_eq!(0, *frb.item(0));
        assert_eq!(0, *frb.item(10));
        assert_eq!(0, *frb.item(-1));
        assert_eq!(0, *frb.item(-2));
        assert_eq!(0, *frb.item(-256));
        assert_eq!(0, frb.size());
        frb.add(42);
        assert_eq!(1, frb.size());
        assert_eq!(42, *frb.last());
        assert_eq!(0, *frb.item(0));
        assert_eq!(42, *frb.item(9));
        assert_eq!(42, *frb.item(59));
        assert_eq!(42, *frb.item(-1));
        assert_eq!(0, *frb.item(-2));
        assert_eq!(0, *frb.item(-256));
        frb.add(666);
        assert_eq!(2, frb.size());
        assert_eq!(666, *frb.last());
        assert_eq!(0, *frb.item(0));
        assert_eq!(666, *frb.item(-1));
        assert_eq!(42, *frb.item(-2));
        assert_eq!(0, *frb.item(-256));
        frb.add(1337);
        assert_eq!(3, frb.size());
        frb.add(911321);
        assert_eq!(4, frb.size());
        assert_eq!(911321, *frb.last());
        assert_eq!(0, *frb.item(0));
        assert_eq!(911321, *frb.item(-1));
        assert_eq!(911321, *frb.item(-11));
        assert_eq!(911321, *frb.item(9));
        assert_eq!(1337, *frb.item(-2));
        assert_eq!(1337, *frb.item(668));
        assert_eq!(666, *frb.item(-3));
        assert_eq!(42, *frb.item(-4));
        assert_eq!(0, *frb.item(-5));
        assert_eq!(0, *frb.item(15));
        assert_eq!(0, *frb.item(-256));
        frb.remove();
        assert_eq!(3, frb.size());
        frb.remove();
        assert_eq!(2, frb.size());
        frb.remove();
        assert_eq!(1, frb.size());
        frb.remove();
        assert_eq!(0, frb.size());
        frb.remove();
        assert_eq!(0, frb.size());
    }
}
