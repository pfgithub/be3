use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

use crate::computation::create_effect;
use crate::memo::{create_memo, Memo};
use crate::runtime::tracking;
use crate::scope::on_cleanup;
use crate::signal::Source;

struct Watched {
    source: Rc<Source>,
    watchers: usize,
}

type Keys<K> = Rc<RefCell<HashMap<K, Watched>>>;

pub struct Selector<K> {
    value: Memo<K>,
    keys: Keys<K>,
}

pub fn create_selector<K: Clone + Eq + Hash + 'static>(
    mut source: impl FnMut() -> K + 'static,
) -> Selector<K> {
    let keys: Keys<K> = Rc::new(RefCell::new(HashMap::new()));
    let value = {
        let keys = keys.clone();
        let mut previous: Option<K> = None;
        create_memo(move || {
            let next = source();
            if let Some(previous) = previous.replace(next.clone()) {
                if previous != next {
                    notify(&keys, &previous);
                    notify(&keys, &next);
                }
            }
            next
        })
    };
    let pull = value.clone();
    create_effect(move || pull.with(|_| ()));
    Selector { value, keys }
}

fn notify<K: Eq + Hash>(keys: &Keys<K>, key: &K) {
    let source = keys.borrow().get(key).map(|watched| watched.source.clone());
    if let Some(source) = source {
        source.changed();
    }
}

impl<K> Clone for Selector<K> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            keys: self.keys.clone(),
        }
    }
}

impl<K: Clone + Eq + Hash + 'static> Selector<K> {
    pub fn is_selected(&self, key: &K) -> bool {
        let selected = self.value.with_untracked(|current| current == key);
        self.watch(key);
        selected
    }

    pub fn memo(&self, key: K) -> Memo<bool> {
        let selector = self.clone();
        create_memo(move || selector.is_selected(&key))
    }

    fn watch(&self, key: &K) {
        if !tracking() {
            return;
        }
        let source = {
            let mut keys = self.keys.borrow_mut();
            let watched = keys.entry(key.clone()).or_insert_with(|| Watched {
                source: Rc::new(Source::default()),
                watchers: 0,
            });
            watched.watchers += 1;
            watched.source.clone()
        };
        source.track();
        let keys = self.keys.clone();
        let key = key.clone();
        on_cleanup(move || {
            let mut keys = keys.borrow_mut();
            let Some(watched) = keys.get_mut(&key) else {
                return;
            };
            watched.watchers -= 1;
            if watched.watchers == 0 {
                keys.remove(&key);
            }
        });
    }

    #[cfg(test)]
    pub(crate) fn watched_keys(&self) -> usize {
        self.keys.borrow().len()
    }
}
