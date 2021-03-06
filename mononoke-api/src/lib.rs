// Copyright (c) 2018-present, Facebook, Inc.
// All Rights Reserved.
//
// This software may be used and distributed according to the terms of the
// GNU General Public License version 2 or any later version.

#![deny(warnings)]

extern crate blobrepo;
#[macro_use]
extern crate failure_ext as failure;
extern crate futures;
extern crate futures_ext;
extern crate mercurial_types;
extern crate mononoke_types;

pub mod errors;

use std::sync::Arc;

use failure::Error;
use futures::Future;

use blobrepo::BlobRepo;
use mercurial_types::{Changeset, HgChangesetId};
use mercurial_types::manifest::Content;
use mononoke_types::MPath;

use errors::ErrorKind;

pub fn get_content_by_path(
    repo: Arc<BlobRepo>,
    changesetid: HgChangesetId,
    path: MPath,
) -> impl Future<Item = Content, Error = Error> {
    repo.get_changeset_by_changesetid(&changesetid)
        .from_err()
        .map(|changeset| changeset.manifestid().clone().into_nodehash())
        .and_then({
            let path = path.clone();
            move |manifest| repo.find_path_in_manifest(Some(path), manifest)
        })
        .and_then(|content| {
            content.ok_or_else(move || ErrorKind::NotFound(path.to_string()).into())
        })
}
