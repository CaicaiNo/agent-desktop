//! Where the menu ledger lives between driver invocations.
//!
//! The ledgers this module reads and writes are session state, not evidence a
//! caller asked for: a ledger that cannot be read or written costs provenance,
//! never the command that touched it. Reads therefore degrade to an empty
//! ledger, and a caller that needs the delivery recorded before it is useful
//! treats the missing entry the same way it treats a missing menu.

use std::io::ErrorKind;
use std::path::PathBuf;

use crate::storage::file_lock::FileLock;
use crate::{AppError, Deadline};

use super::ledger::{MenuDelivery, MenuLedger};

const LEDGER_FILE: &str = "menu_origin.json";
const LOCK_FILE: &str = "menu_origin.lock";
const MAX_LEDGER_BYTES: u64 = 262_144;
const LOCK_TIMEOUT_MS: u64 = 2_000;

/// The ledger of one state root or one session.
pub(crate) struct MenuLedgerStore {
    dir: PathBuf,
}

impl MenuLedgerStore {
    pub(crate) fn for_session(session_id: Option<&str>) -> Result<Self, AppError> {
        let dir = match session_id {
            Some(session_id) => crate::session::session_dir(session_id)?,
            None => crate::session::agent_desktop_dir()?,
        };
        Ok(Self { dir })
    }

    fn ledger_path(&self) -> PathBuf {
        self.dir.join(LEDGER_FILE)
    }

    /// Reads the ledger, treating an absent or unreadable file as empty.
    ///
    /// An empty ledger attributes nothing, which is the same answer a menu
    /// opened outside the driver gets, so a damaged file cannot manufacture
    /// provenance.
    pub(crate) fn load(&self) -> MenuLedger {
        let path = self.ledger_path();
        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.len() > MAX_LEDGER_BYTES => return MenuLedger::default(),
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => return MenuLedger::default(),
            Err(_) => return MenuLedger::default(),
        }
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(_) => MenuLedger::default(),
        }
    }

    /// Records `delivery`, keeping the ledger bounded.
    pub(crate) fn record(&self, delivery: MenuDelivery) -> Result<(), AppError> {
        crate::storage::private_file_parent::ensure_private(&self.dir).map_err(AppError::Io)?;
        let lock = FileLock::acquire(
            &self.dir.join(LOCK_FILE),
            Deadline::detached_after(LOCK_TIMEOUT_MS)?,
            "menu origin ledger lock",
        )?;
        let mut ledger = self.load();
        let _held = lock;
        ledger.push(delivery);
        let bytes = serde_json::to_vec(&ledger).map_err(AppError::Json)?;
        crate::refs::write_private_file(&self.ledger_path(), &bytes)
    }
}
