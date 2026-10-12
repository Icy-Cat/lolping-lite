//! Windows low-level hooks — stub for this milestone.

use super::{HookStatus, InputEvent, SharedInput};
use std::sync::mpsc::Sender;

pub fn spawn(tx: Sender<InputEvent>, _shared: SharedInput) -> HookStatus {
    let _ = tx.send(InputEvent::Status(HookStatus::Unsupported(
        "Windows WH_KEYBOARD_LL / WH_MOUSE_LL not implemented yet",
    )));
    HookStatus::Unsupported("Windows hooks TBD")
}
