//! The deliveries that may have opened the menu a caller is looking at.
//!
//! A menu surface on macOS names only the application that owns it, so nothing
//! in the answer says which element opened it. What does say so is the driver's
//! own history: a right click delivered to a ref, plus what menu was already on
//! screen when that delivery happened. A menu read later is attributed to that
//! delivery only while it is demonstrably not the menu that predates it.

use serde::{Deserialize, Serialize};

use crate::{ProcessId, menu_origin::instance::MenuInstance};

const MAX_ENTRIES: usize = 32;
const PROOF_TTL_MS: u64 = 120_000;

/// What was on screen immediately before a right click was delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PriorMenu {
    None,
    Open,
    Unknown,
}

/// One right click that the driver delivered to an element, with the menu state
/// it saw on the way in.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct MenuDelivery {
    pub sequence: u64,
    pub origin_ref: String,
    pub app: String,
    pub pid: ProcessId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_instance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    pub delivered_at_ms: u64,
    pub delivered: bool,
    pub prior: PriorMenu,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prior_instance: Option<String>,
}

/// Every delivery this state root has seen, oldest first.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct MenuLedger {
    #[serde(default)]
    pub entries: Vec<MenuDelivery>,
}

impl MenuLedger {
    /// Appends `delivery`, keeping the ledger bounded.
    ///
    /// The sequence is the ledger's own count, not a clock, so a state root
    /// whose clock moved backwards still orders its deliveries.
    pub(crate) fn push(&mut self, mut delivery: MenuDelivery) {
        delivery.sequence = self
            .entries
            .last()
            .map_or(1, |last| last.sequence.saturating_add(1));
        self.entries.push(delivery);
        if self.entries.len() > MAX_ENTRIES {
            let excess = self.entries.len() - MAX_ENTRIES;
            self.entries.drain(..excess);
        }
    }

    /// The delivery a menu read now may be attributed to, or `None`.
    ///
    /// Attribution needs all of: a delivery the driver actually performed, a
    /// conclusive answer about what was on screen before it, a menu that is not
    /// the one that was already there, the same application and process
    /// generation, and a proof young enough that the caller is still acting on
    /// this delivery. The newest delivery is the only candidate: a later
    /// delivery that opened nothing must not leave an older proof standing for
    /// a menu it never opened.
    pub(crate) fn origin_for(
        &self,
        observed: &MenuInstance,
        app: &str,
        pid: ProcessId,
        process_instance: Option<&str>,
        now_ms: u64,
    ) -> Option<&MenuDelivery> {
        let candidate = self
            .entries
            .iter()
            .rev()
            .find(|entry| entry.app == app && entry.pid == pid)?;
        if !candidate.delivered || candidate.prior == PriorMenu::Unknown {
            return None;
        }
        if candidate.process_instance.as_deref() != process_instance {
            return None;
        }
        if candidate.prior_instance.as_deref() == Some(observed.id()) {
            return None;
        }
        if candidate.prior == PriorMenu::None && candidate.prior_instance.is_some() {
            return None;
        }
        if now_ms.saturating_sub(candidate.delivered_at_ms) > PROOF_TTL_MS {
            return None;
        }
        Some(candidate)
    }
}

#[cfg(test)]
#[path = "ledger_tests.rs"]
mod tests;
