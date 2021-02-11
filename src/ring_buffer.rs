pub struct FixedRingBuffer<T> {
    data: Vec<T>,
    item: usize,
}

impl<T: Clone> FixedRingBuffer<T> {
    pub fn new(size: usize, init: T) -> Self {
        let mut me = Self {
            data: Vec::with_capacity(size),
            item: 0,
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
            item: 0,
        };

        me.data.resize_with(size, init);
        me
    }

    pub fn add(&mut self, item: T) {
        self.data[self.item] = item;
        self.item += 1;
        if self.item >= self.data.len() {
            self.item = 0;
        }
    }

    pub fn last(&self) -> &T {
        &self.data[self.item]
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn item(&self, no: usize) -> &T {
        &self.data[(self.item + no) % self.data.len()]
    }
}

impl<'a, T> FixedRingBuffer<T> {
    pub fn iter(&'a self) -> FixedRingBufferIterator<'a, T> {
        FixedRingBufferIterator::new(self)
    }
}

pub struct FixedRingBufferIterator<'a, T> {
    buf: &'a FixedRingBuffer<T>,
    count: usize,
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
