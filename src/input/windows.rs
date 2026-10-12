//! Windows low-level hooks — **stub**.
//!
//! TODO(issue): implement `SetWindowsHookExW` with `WH_KEYBOARD_LL` + `WH_MOUSE_LL`
//! on a dedicated thread with a message pump; map VK_MENU / VK_CONTROL to ping/emote
//! modifiers; mirror macOS ListenOnly policy first.
//!
//! Until then, the GUI falls back to in-window Alt/Ctrl for local testing.

use super::{HookStatus, InputEvent, SharedInput};
use std::sync::mpsc::Sender;

pub fn spawn(tx: Sender<InputEvent>, _shared: SharedInput) -> HookStatus {
    let status = HookStatus::Unsupported(
        "Windows WH_KEYBOARD_LL / WH_MOUSE_LL not implemented — use Alt/Ctrl in-window",
    );
    let _ = tx.send(InputEvent::Status(status));
    status
}
