use std::collections::BTreeMap;

use super::page;
use crate::workspace::records::{Favorite, PageId, PageRecord, Placement, Trash};
use crate::workspace::tree::{TrashedPage, Tree, TreePage, tree};

fn rec(parent: Option<&str>, order: u64) -> PageRecord {
    PageRecord {
        title: String::new(),
        placement: Placement {
            parent: parent.map(page),
            order,
        },
        favorite: None,
        trash: None,
        created_at: 1,
        updated_at: 2,
    }
}

fn trashed(mut record: PageRecord, batch: &str, at: u64) -> PageRecord {
    record.trash = Some(Trash {
        original_parent: record.placement.parent.clone(),
        original_order: record.placement.order,
        batch_id: batch.into(),
        trashed_at: at,
    });
    record
}

fn fav(mut record: PageRecord, order: u64) -> PageRecord {
    record.favorite = Some(Favorite { order });
    record
}

fn ids(tree: &Tree) -> Vec<(&str, Option<&str>)> {
    tree.pages
        .iter()
        .map(|p| (p.id.as_str(), p.parent.as_ref().map(PageId::as_str)))
        .collect()
}

#[test]
fn pages_come_depth_first_in_sibling_order() {
    let records = BTreeMap::from([
        (page("b"), rec(None, 1)),
        (page("a"), rec(None, 2)),
        (page("a1"), rec(Some("a"), 0)),
        (page("b2"), rec(Some("b"), 5)),
        (page("b1"), rec(Some("b"), 5)),
        (page("b1x"), rec(Some("b1"), 0)),
    ]);
    let tree = tree(&records);
    assert_eq!(
        ids(&tree),
        [
            ("b", None),
            ("b1", Some("b")),
            ("b1x", Some("b1")),
            ("b2", Some("b")),
            ("a", None),
            ("a1", Some("a")),
        ]
    );
    assert_eq!(
        tree.pages[0],
        TreePage {
            id: page("b"),
            parent: None,
            title: String::new(),
            created_at: 1,
            updated_at: 2,
        }
    );
}

#[test]
fn trash_hides_the_page_and_every_descendant_and_lists_only_batch_roots() {
    let records = BTreeMap::from([
        (page("keep"), fav(rec(None, 0), 2)),
        (page("gone"), trashed(rec(None, 1), "batch-1", 10)),
        (
            page("gone-child"),
            fav(trashed(rec(Some("gone"), 0), "batch-1", 10), 1),
        ),
        (page("left-active"), rec(Some("gone-child"), 0)),
        (page("later"), trashed(rec(Some("keep"), 0), "batch-2", 20)),
        (page("inner"), trashed(rec(Some("gone"), 1), "batch-0", 5)),
    ]);
    let tree = tree(&records);
    assert_eq!(ids(&tree), [("keep", None)]);
    assert_eq!(tree.favorites, [page("keep")]);
    assert_eq!(
        tree.trash,
        [
            TrashedPage {
                id: page("later"),
                title: String::new(),
                batch_id: "batch-2".into(),
                trashed_at: 20,
            },
            TrashedPage {
                id: page("gone"),
                title: String::new(),
                batch_id: "batch-1".into(),
                trashed_at: 10,
            },
            TrashedPage {
                id: page("inner"),
                title: String::new(),
                batch_id: "batch-0".into(),
                trashed_at: 5,
            },
        ]
    );
}

#[test]
fn favorites_sort_by_favorite_order_then_id() {
    let records = BTreeMap::from([
        (page("c"), fav(rec(None, 0), 1)),
        (page("a"), fav(rec(None, 1), 1)),
        (page("b"), fav(rec(Some("c"), 0), 0)),
    ]);
    assert_eq!(tree(&records).favorites, [page("b"), page("a"), page("c")]);
}

#[test]
fn a_missing_parent_or_a_cycle_never_hides_a_page() {
    let records = BTreeMap::from([
        (page("orphan"), rec(Some("erased"), 0)),
        (page("x"), rec(Some("z"), 0)),
        (page("y"), rec(Some("x"), 0)),
        (page("z"), rec(Some("y"), 0)),
        (page("self"), rec(Some("self"), 1)),
    ]);
    assert_eq!(
        ids(&tree(&records)),
        [
            ("orphan", None),
            ("x", None),
            ("y", Some("x")),
            ("z", Some("y")),
            ("self", None),
        ]
    );
}
