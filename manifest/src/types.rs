// Copyright (c) 2019-present, Facebook, Inc.
// All Rights Reserved.
//
// This software may be used and distributed according to the terms of the
// GNU General Public License version 2 or any later version.

use blobstore::{Blobstore, Loadable, Storable};
use context::CoreContext;
use failure::Error;
use futures::Future;
use futures_ext::{BoxFuture, FutureExt};
use mononoke_types::{MPath, MPathElement};
use serde_derive::{Deserialize, Serialize};
use std::{collections::BTreeMap, iter::FromIterator};

pub trait Manifest: Sized + 'static {
    type TreeId: Loadable<Value = Self>;
    type LeafId;

    fn list(&self) -> Box<dyn Iterator<Item = (MPathElement, Entry<Self::TreeId, Self::LeafId>)>>;
    fn lookup(&self, name: &MPathElement) -> Option<Entry<Self::TreeId, Self::LeafId>>;
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Entry<T, L> {
    Tree(T),
    Leaf(L),
}

impl<T, L> Entry<T, L> {
    pub fn into_tree(self) -> Option<T> {
        match self {
            Entry::Tree(tree) => Some(tree),
            _ => None,
        }
    }

    pub fn into_leaf(self) -> Option<L> {
        match self {
            Entry::Leaf(leaf) => Some(leaf),
            _ => None,
        }
    }
}

impl<T, L> Loadable for Entry<T, L>
where
    T: Loadable,
    L: Loadable,
{
    type Value = Entry<T::Value, L::Value>;

    fn load(
        &self,
        ctx: CoreContext,
        blobstore: impl Blobstore + Clone,
    ) -> BoxFuture<Self::Value, Error> {
        match self {
            Entry::Tree(tree_id) => tree_id.load(ctx, blobstore).map(Entry::Tree).boxify(),
            Entry::Leaf(leaf_id) => leaf_id.load(ctx, blobstore).map(Entry::Leaf).boxify(),
        }
    }
}

impl<T, L> Storable for Entry<T, L>
where
    T: Storable,
    L: Storable,
{
    type Key = Entry<T::Key, L::Key>;

    fn store(
        &self,
        ctx: CoreContext,
        blobstore: impl Blobstore + Clone,
    ) -> BoxFuture<Self::Key, Error> {
        match self {
            Entry::Tree(tree) => tree.store(ctx, blobstore).map(Entry::Tree).boxify(),
            Entry::Leaf(leaf) => leaf.store(ctx, blobstore).map(Entry::Leaf).boxify(),
        }
    }
}

pub struct PathTree<V> {
    pub value: V,
    pub subentries: BTreeMap<MPathElement, Self>,
}

impl<V> PathTree<V>
where
    V: Default,
{
    pub fn insert(&mut self, path: MPath, value: V) {
        let mut node = path.into_iter().fold(self, |node, element| {
            node.subentries
                .entry(element)
                .or_insert_with(Default::default)
        });
        node.value = value;
    }
}

impl<V> Default for PathTree<V>
where
    V: Default,
{
    fn default() -> Self {
        Self {
            value: Default::default(),
            subentries: Default::default(),
        }
    }
}

impl<V> FromIterator<(MPath, V)> for PathTree<V>
where
    V: Default,
{
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (MPath, V)>,
    {
        let mut tree: Self = Default::default();
        for (path, value) in iter {
            tree.insert(path, value);
        }
        tree
    }
}

pub struct PathTreeIter<V> {
    frames: Vec<(Option<MPath>, PathTree<V>)>,
}

impl<V> Iterator for PathTreeIter<V> {
    type Item = (Option<MPath>, V);

    fn next(&mut self) -> Option<Self::Item> {
        let (path, PathTree { value, subentries }) = self.frames.pop()?;
        for (name, subentry) in subentries {
            self.frames.push((
                Some(MPath::join_opt_element(path.as_ref(), &name)),
                subentry,
            ));
        }
        Some((path, value))
    }
}

impl<V> IntoIterator for PathTree<V> {
    type Item = (Option<MPath>, V);
    type IntoIter = PathTreeIter<V>;

    fn into_iter(self) -> Self::IntoIter {
        PathTreeIter {
            frames: vec![(None, self)],
        }
    }
}