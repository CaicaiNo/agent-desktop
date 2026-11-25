//! Provenance for the application-owned `menu` surface.
//!
//! A menu answer names the application that owns it and nothing about how it
//! came to be open, so a caller that opened a menu by acting on a row cannot
//! tell its own menu from one that was already there. This module closes that
//! gap with evidence the driver alone holds: every right click it delivers is
//! recorded together with what menu, if any, was on screen immediately before,
//! and a later menu read is attributed to that delivery only when the menu it
//! observes is not the one that predates it.
//!
//! Nothing here is inferred from where the menu is drawn. Two menus can share a
//! position and a set of items, so geometry and item lists never stand in for
//! the delivery that opened one.

mod instance;
mod ledger;
mod probe;
mod store;

pub(crate) use ledger::PriorMenu;

use crate::{
    CommandContext, PlatformAdapter, ProcessId, RefEntry, SnapshotSurface, snapshot::SnapshotResult,
};
use serde_json::{Value, json};

use instance::MenuInstance;
use ledger::MenuDelivery;
use probe::MenuRead;
use store::MenuLedgerStore;

const PRE_READ_MS: u64 = 400;
const DELIVERY: &str = "right-click";

/// A right click the driver is about to deliver, with the menu evidence taken
/// on the way in.
pub(crate) struct PendingDelivery {
    origin_ref: String,
    app: String,
    pid: ProcessId,
    process_instance: Option<String>,
    window_id: Option<String>,
    prior: PriorMenu,
    prior_instance: Option<String>,
}

/// Reads the menu state a right click on `ref_id` is about to change.
///
/// Returns `None` when the platform serves no menu surface or the ref does not
/// name the application it belongs to: without either, no delivery can ever be
/// attributed, so nothing is recorded. A read that fails is not treated as
/// "no menu open" — it records that the prior state is unknown, and an unknown
/// prior state attributes nothing later.
pub(crate) fn prepare(
    adapter: &dyn PlatformAdapter,
    ref_id: &str,
    entry: &RefEntry,
) -> Option<PendingDelivery> {
    if adapter
        .check_surface_support(SnapshotSurface::Menu)
        .is_err()
    {
        return None;
    }
    let app = entry.source.source_app.clone()?;
    let deadline = crate::Deadline::after(PRE_READ_MS).ok()?;
    let read = probe::observe(adapter, &app, deadline);
    let (prior, prior_instance) = match read {
        MenuRead::Absent => (PriorMenu::None, None),
        MenuRead::Observed(instance) => (PriorMenu::Open, Some(instance.id().to_owned())),
        MenuRead::Unknown => (PriorMenu::Unknown, None),
    };
    Some(PendingDelivery {
        origin_ref: ref_id.to_owned(),
        app,
        pid: entry.process.pid,
        process_instance: entry.process.process_instance.clone(),
        window_id: entry.source.source_window_id.clone(),
        prior,
        prior_instance,
    })
}

/// Records the delivery `prepare` saw, whatever it delivered.
///
/// A failed or unproven delivery is recorded too: it supersedes the proof of an
/// earlier one, so a menu that survived a delivery nobody could attribute is
/// never attributed to the delivery before it.
pub(crate) fn finish(pending: Option<PendingDelivery>, delivered: bool, context: &CommandContext) {
    let Some(pending) = pending else {
        return;
    };
    let Ok(store) = MenuLedgerStore::for_session(context.session_id()) else {
        return;
    };
    let _ = store.record(MenuDelivery {
        sequence: 0,
        origin_ref: pending.origin_ref,
        app: pending.app,
        pid: pending.pid,
        process_instance: pending.process_instance,
        window_id: pending.window_id,
        delivered_at_ms: now_ms(),
        delivered,
        prior: pending.prior,
        prior_instance: pending.prior_instance,
    });
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Adds the menu's provenance to a menu-surface snapshot answer.
///
/// The answer keeps its shape when nothing can be attributed: an absent
/// `origin` is the caller's signal that the menu it read has no recorded
/// delivery behind it, and a caller that needs one refuses to act.
pub(crate) fn annotate(
    body: &mut Value,
    result: &SnapshotResult,
    surface: SnapshotSurface,
    context: &CommandContext,
) {
    if surface != SnapshotSurface::Menu || !result.complete {
        return;
    }
    let Some((app, pid, process_instance)) = owner_identity(result) else {
        return;
    };
    let Ok(store) = MenuLedgerStore::for_session(context.session_id()) else {
        return;
    };
    let observed = instance::of_tree(&result.tree);
    let ledger = store.load();
    let Some(delivery) =
        ledger.origin_for(&observed, &app, pid, process_instance.as_deref(), now_ms())
    else {
        return;
    };
    body["origin_ref"] = json!(delivery.origin_ref);
    body["origin"] = origin_json(delivery, &observed);
}

fn owner_identity(result: &SnapshotResult) -> Option<(String, ProcessId, Option<String>)> {
    match &result.owner {
        crate::SnapshotOwner::Application(app) => {
            Some((app.name.clone(), app.pid, app.process_instance.clone()))
        }
        crate::SnapshotOwner::Window(window) => Some((
            window.app.clone(),
            window.pid,
            window.process_instance.clone(),
        )),
    }
}

fn origin_json(delivery: &MenuDelivery, observed: &MenuInstance) -> Value {
    let mut origin = json!({
        "ref_id": delivery.origin_ref,
        "app": delivery.app,
        "pid": delivery.pid.get(),
        "surface": SnapshotSurface::Menu.as_str(),
        "delivery": DELIVERY,
        "sequence": delivery.sequence,
        "delivered_at_ms": delivery.delivered_at_ms,
        "instance": observed.id(),
        "instance_basis": observed.basis().as_str(),
        "prior": delivery.prior,
        "proof": "fresh_after_delivery",
    });
    if let Some(process_instance) = delivery.process_instance.as_deref() {
        origin["process_instance"] = json!(process_instance);
    }
    if let Some(window_id) = delivery.window_id.as_deref() {
        origin["window_id"] = json!(window_id);
    }
    origin
}

#[cfg(test)]
#[path = "menu_origin_tests.rs"]
mod tests;
