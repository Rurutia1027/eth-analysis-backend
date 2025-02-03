use crate::kv_store::KvStore;
///! Job Progress
///! This module is designed for tracking the progress of long executed jobs.
///! Each tracked long executed job is stored to cache by its DB key and its progress value.
use serde::{de::DeserializeOwned, Serialize};

pub struct JobProgress<'a, A: Serialize + DeserializeOwned> {
    key_value_store: &'a dyn KvStore,
    key: &'static str,
    phantom: std::marker::PhantomData<A>,
}

impl<A: Serialize + DeserializeOwned> JobProgress<'_, A> {
    pub fn new<'a>(
        key: &'static str,
        kv_store: &'a impl KvStore,
    ) -> JobProgress<'a, A> {
        JobProgress {
            key,
            key_value_store,
            phantom: std::marker::PhantomData,
        }
    }

    pub async fn get(&self) -> Option<A> {
        self.key_value_store
            .get(self.key)
            .await
            .map(|value| serde_json::from_value(value).unwrap())
    }

    pub async fn set(&self, value: &A) {
        self.key_value_store
            .set_value(self.key, &serde_json::to_value(value).unwrap())
            .await
    }
}
