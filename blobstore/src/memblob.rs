// Copyright (c) 2004-present, Facebook, Inc.
// All Rights Reserved.
//
// This software may be used and distributed according to the terms of the
// GNU General Public License version 2 or any later version.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use failure::Error;
use futures::future::{lazy, IntoFuture};
use futures_ext::{BoxFuture, FutureExt};

use mononoke_types::BlobstoreBytes;

use Blobstore;

/// In-memory "blob store"
///
/// Pure in-memory implementation for testing.
#[derive(Clone)]
pub struct EagerMemblob {
    hash: Arc<Mutex<HashMap<String, BlobstoreBytes>>>,
}

/// As EagerMemblob, but methods are lazy - they wait until polled to do anything.
#[derive(Clone)]
pub struct LazyMemblob {
    hash: Arc<Mutex<HashMap<String, BlobstoreBytes>>>,
}

impl EagerMemblob {
    pub fn new() -> Self {
        Self {
            hash: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl LazyMemblob {
    pub fn new() -> Self {
        Self {
            hash: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Blobstore for EagerMemblob {
    fn put(&self, key: String, value: BlobstoreBytes) -> BoxFuture<(), Error> {
        let mut inner = self.hash.lock().expect("lock poison");

        inner.insert(key, value);
        Ok(()).into_future().boxify()
    }

    fn get(&self, key: String) -> BoxFuture<Option<BlobstoreBytes>, Error> {
        let inner = self.hash.lock().expect("lock poison");

        Ok(inner.get(&key).map(Clone::clone)).into_future().boxify()
    }
}

impl Blobstore for LazyMemblob {
    fn put(&self, key: String, value: BlobstoreBytes) -> BoxFuture<(), Error> {
        let hash = self.hash.clone();

        lazy(move || {
            let mut inner = hash.lock().expect("lock poison");

            inner.insert(key, value);
            Ok(()).into_future()
        }).boxify()
    }

    fn get(&self, key: String) -> BoxFuture<Option<BlobstoreBytes>, Error> {
        let hash = self.hash.clone();

        lazy(move || {
            let inner = hash.lock().expect("lock poison");
            Ok(inner.get(&key).map(Clone::clone)).into_future()
        }).boxify()
    }
}

impl fmt::Debug for EagerMemblob {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EagerMemblob")
            .field(
                "hash",
                &format!(
                    "({} entries)",
                    self.hash.lock().expect("lock poisoned").len()
                ),
            )
            .finish()
    }
}

impl fmt::Debug for LazyMemblob {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("LazyMemblob")
            .field(
                "hash",
                &format!(
                    "({} entries)",
                    self.hash.lock().expect("lock poisoned").len()
                ),
            )
            .finish()
    }
}
