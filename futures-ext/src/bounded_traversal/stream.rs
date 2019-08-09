// Copyright (c) 2019-present, Facebook, Inc.
// All Rights Reserved.
//
// This software may be used and distributed according to the terms of the
// GNU General Public License version 2 or any later version.

use futures::{
    stream::{self, FuturesUnordered},
    try_ready, Async, IntoFuture, Stream,
};
use std::collections::VecDeque;

/// `bounded_traversal_stream` traverses implicit asynchronous tree specified by `init`
/// and `unfold` arguments. All `unfold` operations are executed in parallel if they
/// do not depend on each other (not related by ancestor-descendant relation in implicit
/// tree) with amount of concurrency constrained by `scheduled_max`. Main difference
/// with `bounded_traversal` is that this one is not structure perserving, and returns
/// stream.
///
/// ## `init: In`
/// Is the root of the implicit tree to be traversed
///
/// ## `unfold: FnMut(In) -> impl IntoFuture<Item = (Out, impl IntoIterator<Item = In>)>`
/// Asynchronous function which given input value produces list of its children and output
/// value.
///
/// ## return value `impl Stream<Item = Out>`
/// Stream of all `Out` values
///
pub fn bounded_traversal_stream<In, Ins, Out, Unfold, UFut>(
    scheduled_max: usize,
    init: In,
    mut unfold: Unfold,
) -> impl Stream<Item = Out, Error = UFut::Error>
where
    Unfold: FnMut(In) -> UFut,
    UFut: IntoFuture<Item = (Out, Ins)>,
    Ins: IntoIterator<Item = In>,
{
    let mut unscheduled = VecDeque::new();
    unscheduled.push_front(init);
    let mut scheduled = FuturesUnordered::new();
    stream::poll_fn(move || loop {
        if scheduled.is_empty() && unscheduled.is_empty() {
            return Ok(Async::Ready(None));
        }

        for item in
            unscheduled.drain(..std::cmp::min(unscheduled.len(), scheduled_max - scheduled.len()))
        {
            scheduled.push(unfold(item).into_future())
        }

        if let Some((out, children)) = try_ready!(scheduled.poll()) {
            for child in children {
                unscheduled.push_front(child);
            }
            return Ok(Async::Ready(Some(out)));
        }
    })
}