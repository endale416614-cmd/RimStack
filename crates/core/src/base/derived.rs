use std::sync::{Arc, Mutex};

use crate::base::locks::Held;

pub struct Derived<K, T> {
    now: Mutex<Option<(K, Arc<T>)>>,
}

impl<K, T> Default for Derived<K, T> {
    fn default() -> Self {
        Self {
            now: Mutex::new(None),
        }
    }
}

impl<K: PartialEq, T> Derived<K, T> {
    /// Made outside the lock, so a reader of the value it holds never waits on a making; two
    /// threads asking at once for a new key may both make it.
    pub fn get(&self, key: K, make: impl FnOnce() -> T) -> Arc<T> {
        if let Some((made_for, value)) = self.now.held().as_ref()
            && *made_for == key
        {
            return value.clone();
        }
        let value = Arc::new(make());
        *self.now.held() = Some((key, value.clone()));
        value
    }

    pub fn put(&self, key: K, value: T) {
        *self.now.held() = Some((key, Arc::new(value)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reader_of_the_value_held_does_not_wait_on_a_making() {
        use std::sync::mpsc::channel;
        let derived: Arc<Derived<u64, u64>> = Arc::default();
        derived.put(1, 10);
        let (started, making) = channel();
        let (release, held) = channel::<()>();
        let maker = {
            let derived = derived.clone();
            std::thread::spawn(move || {
                derived.get(2, || {
                    started.send(()).unwrap();
                    held.recv().unwrap();
                    20
                });
            })
        };
        making.recv().unwrap();
        assert_eq!(*derived.get(1, || unreachable!("made while held")), 10);
        release.send(()).unwrap();
        maker.join().unwrap();
        assert_eq!(*derived.get(2, || unreachable!("made twice")), 20);
    }

    #[test]
    fn a_value_is_made_again_only_when_its_key_changes() {
        let derived: Derived<u64, String> = Derived::default();
        assert_eq!(*derived.get(1, || "first".into()), "first");
        assert_eq!(*derived.get(1, || unreachable!("made twice")), "first");
        assert_eq!(*derived.get(2, || "second".into()), "second");
        derived.put(3, "given".into());
        assert_eq!(
            *derived.get(3, || unreachable!("made though given")),
            "given"
        );
    }
}
