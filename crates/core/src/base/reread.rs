use std::sync::{Arc, Mutex, RwLock};

use crate::base::locks::{Held, Shared};

pub struct Reread<T> {
    now: RwLock<Option<Arc<T>>>,
    reading: Mutex<()>,
}

impl<T> Default for Reread<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Reread<T> {
    pub const fn new() -> Self {
        Self {
            now: RwLock::new(None),
            reading: Mutex::new(()),
        }
    }

    pub fn get(&self, read: impl FnOnce() -> T) -> Arc<T> {
        let known = self.now.reading().clone();
        if let Some(known) = known {
            return known;
        }
        crate::base::waits::slow(std::any::type_name::<T>());
        let _alone = self.reading.held();
        if let Some(known) = self.now.reading().clone() {
            return known;
        }
        let fresh = Arc::new(read());
        self.now.writing().get_or_insert(fresh).clone()
    }

    /// What was read, if anything has been, never waiting on a reading.
    pub fn now(&self) -> Option<Arc<T>> {
        self.now.reading().clone()
    }

    pub fn put(&self, fresh: T) {
        *self.now.writing() = Some(Arc::new(fresh));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threads_asking_at_once_read_once() {
        let counted: Arc<Reread<usize>> = Arc::new(Reread::new());
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let asking: Vec<_> = (0..8)
            .map(|_| {
                let (counted, reads) = (counted.clone(), reads.clone());
                std::thread::spawn(move || {
                    *counted.get(|| {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                        reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                    })
                })
            })
            .collect();
        let got: Vec<usize> = asking.into_iter().map(|t| t.join().unwrap()).collect();
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(got.iter().all(|&g| g == 0));
    }

    #[test]
    fn it_is_read_once_until_put_again_and_the_old_reading_is_freed() {
        let counted: Reread<usize> = Reread::new();
        let first = counted.get(|| 1);
        assert_eq!(*counted.get(|| unreachable!("read twice")), 1);
        let old = Arc::downgrade(&first);
        counted.put(2);
        assert_eq!(*counted.get(|| unreachable!("read after put")), 2);
        assert!(old.upgrade().is_some());
        drop(first);
        assert!(old.upgrade().is_none());
    }
}
