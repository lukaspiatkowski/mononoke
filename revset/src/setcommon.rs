// Copyright (c) 2017-present, Facebook, Inc.
// All Rights Reserved.
//
// This software may be used and distributed according to the terms of the
// GNU General Public License version 2 or any later version.

use blobrepo::BlobRepo;
use futures::future::Future;
use futures::stream::Stream;
use mercurial_types::HgNodeHash;
use mercurial_types::nodehash::HgChangesetId;
use mononoke_types::Generation;
use std::boxed::Box;
use std::sync::Arc;

use NodeStream;
use errors::*;
use failure::{err_msg, Error};

use futures::{Async, Poll};

pub type InputStream = Box<Stream<Item = (HgNodeHash, Generation), Error = Error> + 'static + Send>;

pub fn add_generations(stream: Box<NodeStream>, repo: Arc<BlobRepo>) -> InputStream {
    let stream = stream.and_then(move |node_hash| {
        repo.get_generation_number(&HgChangesetId::new(node_hash))
            .and_then(move |genopt| {
                genopt.ok_or_else(|| err_msg(format!("{} not found", node_hash)))
            })
            .map(move |gen_id| (node_hash, gen_id))
            .map_err(|err| err.context(ErrorKind::GenerationFetchFailed).into())
    });
    Box::new(stream)
}

pub fn all_inputs_ready(
    inputs: &Vec<(InputStream, Poll<Option<(HgNodeHash, Generation)>, Error>)>,
) -> bool {
    inputs
        .iter()
        .map(|&(_, ref state)| match state {
            &Err(_) => false,
            &Ok(ref p) => p.is_ready(),
        })
        .all(|ready| ready)
}

pub fn poll_all_inputs(
    inputs: &mut Vec<(InputStream, Poll<Option<(HgNodeHash, Generation)>, Error>)>,
) {
    for &mut (ref mut input, ref mut state) in inputs.iter_mut() {
        if let Ok(Async::NotReady) = *state {
            *state = input.poll();
        }
    }
}

#[cfg(test)]
pub struct NotReadyEmptyStream {
    pub poll_count: usize,
}

#[cfg(test)]
impl Stream for NotReadyEmptyStream {
    type Item = HgNodeHash;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if self.poll_count == 0 {
            Ok(Async::Ready(None))
        } else {
            self.poll_count -= 1;
            Ok(Async::NotReady)
        }
    }
}

#[cfg(test)]
pub struct RepoErrorStream {
    pub hash: HgNodeHash,
}

#[cfg(test)]
impl Stream for RepoErrorStream {
    type Item = HgNodeHash;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        bail_err!(ErrorKind::RepoError(self.hash));
    }
}
