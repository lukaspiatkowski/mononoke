// Copyright (c) 2004-present, Facebook, Inc.
// All Rights Reserved.
//
// This software may be used and distributed according to the terms of the
// GNU General Public License version 2 or any later version.

use clap::ArgMatches;
use failure_ext::Error;
use futures::prelude::*;
use futures_ext::{BoxFuture, FutureExt};
use std::str::FromStr;

use cmdlib::args;
use context::CoreContext;
use mercurial_types::HgChangesetId;
use mononoke_types::ChangesetId;
use slog::Logger;

pub fn subcommand_hash_convert(
    logger: Logger,
    matches: &ArgMatches<'_>,
    sub_m: &ArgMatches<'_>,
) -> BoxFuture<(), Error> {
    let source_hash = sub_m.value_of("HASH").unwrap().to_string();
    let source = sub_m.value_of("from").unwrap().to_string();
    let target = sub_m.value_of("to").unwrap();
    // Check that source and target are different types.
    assert_eq!(
        false,
        (source == "hg") ^ (target == "bonsai"),
        "source and target should be different"
    );
    args::init_cachelib(&matches);
    // TODO(T37478150, luk) This is not a test case, fix it up in future diffs
    let ctx = CoreContext::test_mock();
    args::open_repo(&logger, &matches)
        .and_then(move |repo| {
            if source == "hg" {
                repo.get_bonsai_from_hg(
                    ctx,
                    HgChangesetId::from_str(&source_hash)
                        .expect("source hash is not valid hg changeset id"),
                )
                .and_then(move |maybebonsai| {
                    match maybebonsai {
                        Some(bonsai) => {
                            println!("{}", bonsai);
                        }
                        None => {
                            panic!("no matching mononoke id found");
                        }
                    }
                    Ok(())
                })
                .left_future()
            } else {
                repo.get_hg_from_bonsai_changeset(
                    ctx,
                    ChangesetId::from_str(&source_hash)
                        .expect("source hash is not valid mononoke id"),
                )
                .and_then(move |mercurial| {
                    println!("{}", mercurial);
                    Ok(())
                })
                .right_future()
            }
        })
        .boxify()
}