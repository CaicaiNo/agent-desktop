use super::*;
use crate::ProcessId;
use crate::menu_origin::instance;

const APP: &str = "Fixture";
const PID: ProcessId = ProcessId::new(42);
const GENERATION: Option<&str> = Some("generation-1");
const NOW: u64 = 1_000_000;

fn menu_instance(name: &str) -> instance::MenuInstance {
    instance::of_tree(&crate::AccessibilityNode {
        ref_id: None,
        role: "menu".into(),
        identity: crate::NodeIdentity {
            name: Some(name.into()),
            ..Default::default()
        },
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children: Vec::new(),
    })
}

fn delivery(prior: PriorMenu, prior_instance: Option<&str>) -> MenuDelivery {
    MenuDelivery {
        sequence: 0,
        origin_ref: "@s1:e7".into(),
        app: APP.into(),
        pid: PID,
        process_instance: GENERATION.map(str::to_owned),
        window_id: Some("w-2".into()),
        delivered_at_ms: NOW,
        delivered: true,
        prior,
        prior_instance: prior_instance.map(str::to_owned),
    }
}

fn ledger_with(entry: MenuDelivery) -> MenuLedger {
    let mut ledger = MenuLedger::default();
    ledger.push(entry);
    ledger
}

fn attribute<'a>(
    ledger: &'a MenuLedger,
    observed: &instance::MenuInstance,
) -> Option<&'a MenuDelivery> {
    ledger.origin_for(observed, APP, PID, GENERATION, NOW)
}

#[test]
fn the_first_open_is_attributed_to_the_delivery_and_its_ref() {
    let observed = menu_instance("context");
    let ledger = ledger_with(delivery(PriorMenu::None, None));

    let proof = attribute(&ledger, &observed).expect("first open must be attributed");

    assert_eq!(proof.origin_ref, "@s1:e7");
    assert_eq!(proof.window_id.as_deref(), Some("w-2"));
}

#[test]
fn a_retry_that_left_the_old_menu_in_place_is_refused() {
    let observed = menu_instance("context");
    let ledger = ledger_with(delivery(PriorMenu::Open, Some(observed.id())));

    assert!(attribute(&ledger, &observed).is_none());
}

#[test]
fn a_retry_that_did_open_a_new_menu_is_attributed_to_the_retry() {
    let old = menu_instance("context");
    let fresh = menu_instance("context after retry");
    let ledger = ledger_with(delivery(PriorMenu::Open, Some(old.id())));

    let proof = attribute(&ledger, &fresh).expect("a renewed menu belongs to the retry");
    assert_eq!(proof.sequence, 1);
    assert!(attribute(&ledger, &old).is_none());
}

#[test]
fn an_unknown_prior_state_attributes_nothing() {
    let observed = menu_instance("context");
    let ledger = ledger_with(delivery(PriorMenu::Unknown, None));

    assert!(attribute(&ledger, &observed).is_none());
}

#[test]
fn a_delivery_that_never_landed_attributes_nothing() {
    let observed = menu_instance("context");
    let mut entry = delivery(PriorMenu::None, None);
    entry.delivered = false;

    assert!(attribute(&ledger_with(entry), &observed).is_none());
}

#[test]
fn a_later_delivery_supersedes_an_earlier_proof_for_the_same_menu() {
    let observed = menu_instance("context");
    let mut ledger = ledger_with(delivery(PriorMenu::None, None));
    ledger.push(delivery(PriorMenu::Open, Some(observed.id())));

    assert!(attribute(&ledger, &observed).is_none());
}

#[test]
fn a_restarted_application_generation_attributes_nothing() {
    let observed = menu_instance("context");
    let ledger = ledger_with(delivery(PriorMenu::None, None));

    assert!(
        ledger
            .origin_for(&observed, APP, PID, Some("generation-2"), NOW)
            .is_none()
    );
    assert!(ledger.origin_for(&observed, APP, PID, None, NOW).is_none());
    assert!(
        ledger
            .origin_for(&observed, "Other", PID, GENERATION, NOW)
            .is_none()
    );
    assert!(
        ledger
            .origin_for(&observed, APP, ProcessId::new(7), GENERATION, NOW)
            .is_none()
    );
}

#[test]
fn a_proof_older_than_the_ttl_attributes_nothing() {
    let observed = menu_instance("context");
    let ledger = ledger_with(delivery(PriorMenu::None, None));

    assert!(
        ledger
            .origin_for(&observed, APP, PID, GENERATION, NOW + PROOF_TTL_MS)
            .is_some()
    );
    assert!(
        ledger
            .origin_for(&observed, APP, PID, GENERATION, NOW + PROOF_TTL_MS + 1)
            .is_none()
    );
}

#[test]
fn an_inconsistent_prior_record_attributes_nothing() {
    let observed = menu_instance("context");
    let ledger = ledger_with(delivery(PriorMenu::None, Some("mnu1:deadbeef")));

    assert!(attribute(&ledger, &observed).is_none());
}

#[test]
fn the_ledger_orders_its_entries_and_keeps_itself_bounded() {
    let mut ledger = MenuLedger::default();
    for _ in 0..(MAX_ENTRIES + 8) {
        ledger.push(delivery(PriorMenu::None, None));
    }

    assert_eq!(ledger.entries.len(), MAX_ENTRIES);
    let sequences: Vec<u64> = ledger.entries.iter().map(|entry| entry.sequence).collect();
    let mut sorted = sequences.clone();
    sorted.sort_unstable();
    assert_eq!(sequences, sorted);
    assert_eq!(sequences.last().copied(), Some((MAX_ENTRIES + 8) as u64));
}
