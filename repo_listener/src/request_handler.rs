// Copyright (c) 2004-present, Facebook, Inc.
// All Rights Reserved.
//
// This software may be used and distributed according to the terms of the
// GNU General Public License version 2 or any later version.

use std::mem;
use std::net::SocketAddr;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dns_lookup::getnameinfo;
use failure::{SlogKVError, prelude::*};
use futures::{Future, Sink, Stream};
use futures_stats::Timed;
use slog::{self, Drain, Level, Logger};
use slog_kvfilter::KVFilter;
use slog_term;
use tokio::util::FutureExt as TokioFutureExt;
use tracing::TraceContext;
use uuid::Uuid;

use hgproto::{sshproto, HgProtoHandler};
use repo_client::RepoClient;
use scuba_ext::ScubaSampleBuilderExt;
use sshrelay::{SenderBytesWrite, Stdio};

use repo_handlers::RepoHandler;

pub fn request_handler(
    (logger, mut scuba_logger, repo): RepoHandler,
    stdio: Stdio,
    addr: SocketAddr,
) -> impl Future<Item = (), Error = ()> {
    let Stdio {
        stdin,
        stdout,
        stderr,
        mut preamble,
    } = stdio;

    let session_uuid = match preamble
        .misc
        .get("session_uuid")
        .and_then(|v| Uuid::parse_str(v).ok())
    {
        Some(session_uuid) => session_uuid,
        None => {
            let session_uuid = Uuid::new_v4();
            preamble
                .misc
                .insert("session_uuid".to_owned(), format!("{}", session_uuid));
            session_uuid
        }
    };

    let wireproto_calls = Arc::new(Mutex::new(Vec::new()));
    let trace = TraceContext::new(session_uuid, Instant::now());

    let conn_log = {
        let stderr_write = SenderBytesWrite {
            chan: stderr.wait(),
        };
        let client_drain = slog_term::PlainSyncDecorator::new(stderr_write);
        let client_drain = slog_term::FullFormat::new(client_drain).build();
        let client_drain =
            KVFilter::new(client_drain, Level::Critical).only_pass_any_on_all_keys(hashmap! {
                "remote".into() => hashset!["true".into(), "remote_only".into()],
            });

        let server_drain = KVFilter::new(logger, Level::Critical).always_suppress_any(hashmap! {
            "remote".into() => hashset!["remote_only".into()],
        });

        let drain = slog::Duplicate::new(client_drain, server_drain).ignore_res();
        Logger::root(drain, o!("session_uuid" => format!("{}", session_uuid)))
    };

    let mut scuba_logger = {
        let client_hostname = match getnameinfo(&addr, 0) {
            Ok((hostname, _)) => hostname,
            Err(err) => {
                warn!(
                    conn_log,
                    "failed to lookup hostname for address {}, reason: {:?}", addr, err
                );
                "".to_owned()
            }
        };
        scuba_logger
            .add_preamble(&preamble)
            .add("client_hostname", client_hostname);
        scuba_logger
    };

    scuba_logger.log_with_msg("Connection established", None);

    // Construct a hg protocol handler
    let proto_handler = HgProtoHandler::new(
        stdin,
        RepoClient::new(repo.clone(), conn_log.clone(), scuba_logger.clone(), trace),
        sshproto::HgSshCommandDecode,
        sshproto::HgSshCommandEncode,
        &conn_log,
        wireproto_calls.clone(),
    );

    // send responses back
    let endres = proto_handler
        .map_err(Error::from)
        .forward(stdout)
        .map(|_| ());

    // If we got an error at this point, then catch it and print a message
    endres
        // Don't wait for more that 15 mins for a request
        .deadline(Instant::now() + Duration::from_secs(15 * 60))
        .timed(move |stats, result| {
            let mut wireproto_calls = wireproto_calls.lock().expect("lock poisoned");
            let wireproto_calls = mem::replace(wireproto_calls.deref_mut(), Vec::new());

            scuba_logger
                .add_stats(&stats)
                .add("wireproto_commands", wireproto_calls);

            match result {
                Ok(_) => scuba_logger.log_with_msg("Request finished - Success", None),
                Err(err) => if err.is_inner() {
                    scuba_logger.log_with_msg("Request finished - Failure", format!("{:#?}", err));
                } else if err.is_elapsed() {
                    scuba_logger.log_with_msg("Request finished - Timeout", None);
                } else {
                    scuba_logger.log_with_msg(
                        "Request finished - Unexpected timer error",
                        format!("{:#?}", err),
                    );
                },
            }
            Ok(())
        })
        .map_err(move |err| {
            if err.is_inner() {
                error!(conn_log, "Command failed";
                SlogKVError(err.into_inner().unwrap()),
                "remote" => "true");
            } else if err.is_elapsed() {
                error!(conn_log, "Timeout while handling request";
                "remote" => "true");
            } else {
                crit!(conn_log, "Unexpected error";
                SlogKVError(err.into_timer().unwrap().into()),
                "remote" => "true");
            }
        })
}
