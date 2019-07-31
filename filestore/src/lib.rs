// Copyright (c) 2019-present, Facebook, Inc.
// All Rights Reserved.
//
// This software may be used and distributed according to the terms of the
// GNU General Public License version 2 or any later version.

#![feature(never_type)]
#![deny(warnings)]

use std::sync::Arc;

use bytes::Bytes;

use cloned::cloned;
use failure_ext::Error;
use futures::{future, prelude::*, stream};
use futures_ext::FutureExt;

use blobstore::Blobstore;
use context::CoreContext;
use mononoke_types::{
    hash, ContentAlias, ContentId, ContentMetadata, ContentMetadataId, MononokeId,
};

mod chunk;
mod errors;
mod expected_size;
mod fetch;
mod finalize;
mod incremental_hash;
mod prepare;
mod streamhash;

#[cfg(test)]
mod test;

/// File storage.
///
/// This is a specialized wrapper around a blobstore specifically for user data files (rather
/// rather than metadata, trees, etc). Its primary (initial) goals are:
/// - providing a streaming interface for file access
/// - maintain multiple aliases for each file using different key schemes
/// - maintain reverse mapping from primary key to aliases
///
/// Secondary:
/// - Implement chunking at this level
/// - Compression
/// - Range access (initially fetch, and later store)
///
/// Implementation notes:
/// This code takes over the management of file content in a backwards compatible way - it uses
/// the same blobstore key structure and the same encoding schemes for existing files.
/// Extensions (compression, chunking) will change this, but it will still allow backwards
/// compatibility.
#[derive(Debug, Clone)]
pub struct Filestore {
    blobstore: Arc<dyn Blobstore>,
    config: FilestoreConfig,
}

#[derive(Debug, Clone)]
pub struct FilestoreConfig {
    chunk_size: u64,
}

impl FilestoreConfig {
    fn chunk_size(&self) -> u64 {
        self.chunk_size
    }
}

impl Default for FilestoreConfig {
    fn default() -> Self {
        FilestoreConfig {
            // TODO: Don't use the default value (expose it through config instead).
            chunk_size: 256 * 1024,
        }
    }
}

/// Key for fetching - we can access with any of the supported key types
#[derive(Debug, Clone)]
pub enum FetchKey {
    Canonical(ContentId),
    Sha1(hash::Sha1),
    Sha256(hash::Sha256),
    GitSha1(hash::GitSha1),
}

impl FetchKey {
    fn blobstore_key(&self) -> String {
        use FetchKey::*;

        match self {
            Canonical(contentid) => contentid.blobstore_key(),
            GitSha1(gitkey) => format!("alias.gitsha1.{}", gitkey.to_hex()),
            Sha1(sha1) => format!("alias.sha1.{}", sha1.to_hex()),
            Sha256(sha256) => format!("alias.sha256.{}", sha256.to_hex()),
        }
    }
}

/// Key for storing. We'll compute any missing keys, but we must have the total size.
#[derive(Debug, Clone)]
pub struct StoreRequest {
    pub expected_size: expected_size::ExpectedSize,
    pub canonical: Option<ContentId>,
    pub sha1: Option<hash::Sha1>,
    pub sha256: Option<hash::Sha256>,
    pub git_sha1: Option<hash::GitSha1>,
}

impl StoreRequest {
    pub fn new(size: u64) -> Self {
        use expected_size::*;

        Self {
            expected_size: ExpectedSize::new(size),
            canonical: None,
            sha1: None,
            sha256: None,
            git_sha1: None,
        }
    }

    pub fn with_canonical(size: u64, canonical: ContentId) -> Self {
        use expected_size::*;

        Self {
            expected_size: ExpectedSize::new(size),
            canonical: Some(canonical),
            sha1: None,
            sha256: None,
            git_sha1: None,
        }
    }

    pub fn with_sha1(size: u64, sha1: hash::Sha1) -> Self {
        use expected_size::*;

        Self {
            expected_size: ExpectedSize::new(size),
            canonical: None,
            sha1: Some(sha1),
            sha256: None,
            git_sha1: None,
        }
    }

    pub fn with_sha256(size: u64, sha256: hash::Sha256) -> Self {
        use expected_size::*;

        Self {
            expected_size: ExpectedSize::new(size),
            canonical: None,
            sha1: None,
            sha256: Some(sha256),
            git_sha1: None,
        }
    }

    pub fn with_git_sha1(size: u64, git_sha1: hash::GitSha1) -> Self {
        use expected_size::*;

        Self {
            expected_size: ExpectedSize::new(size),
            canonical: None,
            sha1: None,
            sha256: None,
            git_sha1: Some(git_sha1),
        }
    }
}

impl Filestore {
    pub fn new(blobstore: Arc<dyn Blobstore>) -> Self {
        Self::with_config(blobstore, FilestoreConfig::default())
    }

    pub fn with_config(blobstore: Arc<dyn Blobstore>, config: FilestoreConfig) -> Self {
        Filestore { blobstore, config }
    }

    /// Return the canonical ID for a key. It doesn't check if the corresponding content
    /// actually exists (its possible for an alias to exist before the ID if there was an
    /// interrupted store operation).
    pub fn get_canonical_id(
        &self,
        ctxt: CoreContext,
        key: &FetchKey,
    ) -> impl Future<Item = Option<ContentId>, Error = Error> {
        match key {
            FetchKey::Canonical(canonical) => future::ok(Some(*canonical)).left_future(),
            aliaskey => self
                .blobstore
                .get(ctxt, aliaskey.blobstore_key())
                .and_then(|maybe_alias| {
                    maybe_alias
                        .map(|blob| {
                            ContentAlias::from_bytes(blob.into_bytes().into())
                                .map(|alias| alias.content_id())
                        })
                        .transpose()
                })
                .right_future(),
        }
    }

    /// Fetch the alias ids for the underlying content.
    /// XXX Compute missing ones?
    /// XXX Allow caller to select which ones they're interested in?
    pub fn get_aliases(
        &self,
        ctxt: CoreContext,
        key: &FetchKey,
    ) -> impl Future<Item = Option<ContentMetadata>, Error = Error> {
        self.get_canonical_id(ctxt.clone(), key).and_then({
            cloned!(self.blobstore, ctxt);
            move |maybe_id| match maybe_id {
                None => Ok(None).into_future().left_future(),
                Some(id) => blobstore
                    .fetch(ctxt, ContentMetadataId::from(id))
                    .right_future(),
            }
        })
    }

    /// Return true if the given key exists. A successful return means the key definitely
    /// either exists or doesn't; an error means the existence could not be determined.
    pub fn exists(
        &self,
        ctxt: CoreContext,
        key: &FetchKey,
    ) -> impl Future<Item = bool, Error = Error> {
        self.get_canonical_id(ctxt.clone(), &key)
            .and_then({
                cloned!(self.blobstore, ctxt);
                move |maybe_id| maybe_id.map(|id| blobstore.is_present(ctxt, id.blobstore_key()))
            })
            .map(|exists: Option<bool>| exists.unwrap_or(false))
    }

    /// Fetch a file as a stream. This returns either success with a stream of data if the file
    ///  exists, success with None if it does not exist, or an Error if either existence can't
    /// be determined or if opening the file failed. File contents are returned in chunks
    /// configured by FilestoreConfig::read_chunk_size - this defines the max chunk size, but
    /// they may be shorter (not just the final chunks - any of them). Chunks are guaranteed to
    /// have non-zero size.
    pub fn fetch(
        &self,
        ctxt: CoreContext,
        key: &FetchKey,
    ) -> impl Future<Item = Option<impl Stream<Item = Bytes, Error = Error>>, Error = Error> {
        // First fetch either the content or the alias
        use fetch::*;

        self.get_canonical_id(ctxt.clone(), key).and_then({
            cloned!(self.blobstore, ctxt);
            move |content_id| match content_id {
                // If we found a ContentId, then return a Future that waits for the first element
                // to show up in the content stream. If we get a NotFound error waiting for this
                // element and it was the root, then resolve to None (i.e. "this content does not
                // exist"). Otherwise, return the content stream. Not found errors after the initial
                // bytes will NOT be captured: such an error would indicate that we're missing part
                // of our contents!
                Some(content_id) => fetch(blobstore, ctxt, content_id)
                    .into_future()
                    .then(|res| match res {
                        Err((FetchError::NotFound(_, Depth::ROOT), _)) => Ok(None),
                        Err((e, _)) => Err(e.into()),
                        Ok((bytes, rest)) => {
                            Ok(Some(stream::iter_ok(bytes).chain(rest.from_err())))
                        }
                    })
                    .left_future(),
                None => Ok(None).into_future().right_future(),
            }
        })
    }

    /// Store a file from a stream. This is guaranteed atomic - either the store will succeed
    /// for the entire file, or it will fail and the file will logically not exist (however
    /// there's no guarantee that any partially written parts will be cleaned up).
    pub fn store(
        &self,
        ctxt: CoreContext,
        req: &StoreRequest,
        data: impl Stream<Item = Bytes, Error = Error> + Send + 'static,
    ) -> impl Future<Item = (), Error = Error> + Send + 'static {
        use chunk::*;
        use finalize::*;
        use prepare::*;

        let prepared = match make_chunks(data, req.expected_size, self.config.chunk_size()) {
            Chunks::Inline(fut) => prepare_inline(fut).left_future(),
            Chunks::Chunked(expected_size, chunks) => {
                prepare_chunked(ctxt.clone(), self.blobstore.clone(), expected_size, chunks)
                    .right_future()
            }
        };

        prepared
            .and_then({
                cloned!(self.blobstore, ctxt, req);
                move |prepared| finalize(blobstore, ctxt, &req, prepared)
            })
            .map(|_| ())
    }
}