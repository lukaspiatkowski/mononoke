// Copyright (c) 2017-present, Facebook, Inc.
// All Rights Reserved.
//
// This software may be used and distributed according to the terms of the
// GNU General Public License version 2 or any later version.

// Codecs related to infinitepush also known as Commit Cloud.

use bytes::{Bytes, BytesMut};
use tokio_io::codec::Decoder;

use utils::BytesExt;

use errors::*;

#[derive(Debug)]
pub struct InfinitepushBookmarksUnpacker {
    finished: bool,
    expected_len: Option<usize>,
}

impl InfinitepushBookmarksUnpacker {
    pub fn new() -> Self {
        Self {
            finished: false,
            expected_len: None,
        }
    }
}

impl Decoder for InfinitepushBookmarksUnpacker {
    type Item = Bytes;
    type Error = Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>> {
        if self.finished {
            return Ok(None);
        }
        match self.expected_len {
            Some(toread) => {
                if buf.len() < toread {
                    Ok(None)
                } else {
                    self.finished = true;
                    Ok(Some(buf.split_to(toread).freeze()))
                }
            }
            None => {
                if buf.len() >= 4 {
                    self.expected_len = Some(buf.drain_u32() as usize);
                }
                Ok(None)
            }
        }
    }
}
