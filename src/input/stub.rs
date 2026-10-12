use super::{HookStatus, InputEvent, SharedConfig, SharedInput};
use std::sync::mpsc::Sender;

pub fn spawn(tx: Sender<InputEvent>, _shared: SharedInput, _config: SharedConfig) -> HookStatus {
    let status = HookStatus::Unsupported(
        "global hooks only on macOS/Windows — use Alt/Ctrl in-window; pause: Ctrl+Alt+P in GUI",
    );
    let _ = tx.send(InputEvent::Status(status));
    status
}
