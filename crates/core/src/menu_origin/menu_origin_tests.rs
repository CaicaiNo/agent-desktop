use super::*;
use crate::refs::refs_test_support::HomeGuard;
use crate::{NodeIdentity, NodePresentation, Rect, SnapshotOwner};

const APP: &str = "Fixture";
const PID: ProcessId = ProcessId::new(42);
const GENERATION: &str = "generation-1";

fn menu(name: &str, items: &[&str], token: Option<&str>) -> crate::AccessibilityNode {
    let mut root = crate::AccessibilityNode {
        ref_id: None,
        role: "menu".into(),
        identity: NodeIdentity {
            retained_object: token.map(str::to_owned),
            name: Some(name.into()),
            ..Default::default()
        },
        presentation: NodePresentation {
            bounds: Some(Rect {
                x: 100.0,
                y: 200.0,
                width: 240.0,
                height: 120.0,
            }),
            ..Default::default()
        },
        children_count: Some(items.len() as u32),
        subtree_truncated: false,
        children: items
            .iter()
            .map(|item| crate::AccessibilityNode {
                ref_id: None,
                role: "menuitem".into(),
                identity: NodeIdentity {
                    name: Some((*item).into()),
                    ..Default::default()
                },
                presentation: Default::default(),
                children_count: None,
                subtree_truncated: false,
                children: Vec::new(),
            })
            .collect(),
    };
    root.identity.retained_object = token.map(str::to_owned);
    root
}

fn result(tree: crate::AccessibilityNode, complete: bool) -> SnapshotResult {
    SnapshotResult {
        tree,
        refmap: crate::refs::RefMap::new(),
        owner: SnapshotOwner::Application(crate::AppInfo {
            name: APP.into(),
            pid: PID,
            bundle_id: None,
            process_instance: Some(GENERATION.into()),
            presentation: None,
        }),
        snapshot_id: Some("@s3".into()),
        complete,
        nodes_observed: 4,
        identity_unproven: None,
    }
}

fn record(store: &MenuLedgerStore, prior: PriorMenu, prior_instance: Option<String>) {
    store
        .record(MenuDelivery {
            sequence: 0,
            origin_ref: "@s2:e9".into(),
            app: APP.into(),
            pid: PID,
            process_instance: Some(GENERATION.into()),
            window_id: Some("w-2".into()),
            delivered_at_ms: now_ms(),
            delivered: true,
            prior,
            prior_instance,
        })
        .expect("record the delivery");
}

#[test]
fn a_menu_the_driver_opened_reports_the_ref_and_the_instance_it_observed() {
    let _home = HomeGuard::new();
    let store = MenuLedgerStore::for_session(None).unwrap();
    let observed = instance::of_tree(&menu("context", &["Rename"], None));
    record(&store, PriorMenu::None, None);

    let mut body = json!({ "app": APP, "complete": true });
    annotate(
        &mut body,
        &result(menu("context", &["Rename"], None), true),
        SnapshotSurface::Menu,
        &CommandContext::default(),
    );

    assert_eq!(body["origin_ref"], json!("@s2:e9"));
    assert_eq!(body["origin"]["ref_id"], json!("@s2:e9"));
    assert_eq!(body["origin"]["instance"], json!(observed.id()));
    assert_eq!(body["origin"]["instance_basis"], json!("structure"));
    assert_eq!(body["origin"]["proof"], json!("fresh_after_delivery"));
    assert_eq!(body["origin"]["window_id"], json!("w-2"));
    assert_eq!(body["origin"]["sequence"], json!(1));
}

#[test]
fn a_menu_read_without_a_recorded_delivery_names_no_origin() {
    let _home = HomeGuard::new();

    let mut body = json!({ "app": APP, "complete": true });
    annotate(
        &mut body,
        &result(menu("context", &["Rename"], None), true),
        SnapshotSurface::Menu,
        &CommandContext::default(),
    );

    assert!(body.get("origin_ref").is_none());
    assert!(body.get("origin").is_none());
}

#[test]
fn a_menu_that_outlived_the_delivery_under_it_names_no_origin() {
    let _home = HomeGuard::new();
    let store = MenuLedgerStore::for_session(None).unwrap();
    let observed = instance::of_tree(&menu("context", &["Rename"], None));
    record(&store, PriorMenu::Open, Some(observed.id().to_owned()));

    let mut body = json!({ "app": APP, "complete": true });
    annotate(
        &mut body,
        &result(menu("context", &["Rename"], None), true),
        SnapshotSurface::Menu,
        &CommandContext::default(),
    );

    assert!(body.get("origin_ref").is_none());
}

#[test]
fn only_a_menu_surface_carries_an_origin() {
    let _home = HomeGuard::new();
    let store = MenuLedgerStore::for_session(None).unwrap();
    record(&store, PriorMenu::None, None);

    let mut body = json!({ "app": APP, "complete": true });
    annotate(
        &mut body,
        &result(menu("context", &["Rename"], None), true),
        SnapshotSurface::Window,
        &CommandContext::default(),
    );

    assert!(body.get("origin").is_none());
}

#[test]
fn an_incomplete_menu_read_carries_no_origin() {
    let _home = HomeGuard::new();
    let store = MenuLedgerStore::for_session(None).unwrap();
    record(&store, PriorMenu::None, None);

    let mut body = json!({ "app": APP, "complete": false });
    annotate(
        &mut body,
        &result(menu("context", &["Rename"], None), false),
        SnapshotSurface::Menu,
        &CommandContext::default(),
    );

    assert!(body.get("origin").is_none());
}

#[test]
fn a_platform_element_token_identifies_the_menu_appearance() {
    let first = instance::of_tree(&menu("context", &["Rename"], Some("axtok1:aa")));
    let second = instance::of_tree(&menu("context", &["Rename"], Some("axtok1:aa")));
    let reopened = instance::of_tree(&menu("context", &["Rename"], Some("axtok1:bb")));

    assert_eq!(first.basis(), instance::InstanceBasis::RemoteToken);
    assert_eq!(first.id(), second.id());
    assert_ne!(first.id(), reopened.id());
}

#[test]
fn a_menu_without_a_token_is_identified_by_its_own_structure() {
    let first = instance::of_tree(&menu("context", &["Rename", "Duplicate"], None));
    let same = instance::of_tree(&menu("context", &["Rename", "Duplicate"], None));
    let other_items = instance::of_tree(&menu("context", &["Rename", "Move to Trash"], None));

    assert_eq!(first.basis(), instance::InstanceBasis::Structure);
    assert_eq!(first.id(), same.id());
    assert_ne!(first.id(), other_items.id());
    assert!(first.id().starts_with("mnu1:"));
}

#[test]
fn a_structural_identity_ignores_geometry_and_read_budget() {
    let with_bounds = menu("context", &["Rename"], None);
    let mut sparse = menu("context", &["Rename"], None);
    sparse.presentation.bounds = None;
    sparse.children_count = None;
    sparse.subtree_truncated = true;

    assert_eq!(
        instance::of_tree(&with_bounds).id(),
        instance::of_tree(&sparse).id()
    );
}

#[test]
fn the_ledger_survives_the_process_that_recorded_it() {
    let _home = HomeGuard::new();
    let store = MenuLedgerStore::for_session(None).unwrap();
    record(&store, PriorMenu::None, None);
    record(
        &store,
        PriorMenu::Open,
        Some("mnu1:0000000000000000".into()),
    );

    let reopened = MenuLedgerStore::for_session(None).unwrap().load();

    assert_eq!(reopened.entries.len(), 2);
    assert_eq!(reopened.entries[1].prior, PriorMenu::Open);
    assert_eq!(reopened.entries[1].sequence, 2);
}

#[test]
fn a_damaged_ledger_attributes_nothing_and_is_replaced() {
    let _home = HomeGuard::new();
    let store = MenuLedgerStore::for_session(None).unwrap();
    let path = crate::session::agent_desktop_dir()
        .unwrap()
        .join("menu_origin.json");
    crate::storage::private_file_parent::ensure_private(path.parent().unwrap()).unwrap();
    crate::refs::write_private_file(&path, b"{not json").unwrap();

    assert!(store.load().entries.is_empty());

    record(&store, PriorMenu::None, None);
    assert_eq!(store.load().entries.len(), 1);
}
