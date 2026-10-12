//! Non-macOS / non-Windows stub (e.g. Linux CI box).

use super::{HookStatus, InputEvent, SharedInput};
use std::sync::mpsc::Sender;

pub fn spawn(tx: Sender<InputEvent>, _shared: SharedInput) -> HookStatus {
    let status = HookStatus::Unsupported("global hooks only on macOS for now (Linux stub)");
    let _ = tx.send(InputEvent::Status(status));
    status
}
