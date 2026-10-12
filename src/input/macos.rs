//! macOS global input via `CGEventTap` (ListenOnly).
//!
//! Platform glue lives here (`#[cfg(target_os = "macos")]` from `mod.rs`).
//! Requires Accessibility (Privacy → Accessibility) for the hosting process.

use super::{HookStatus, InputEvent, SharedInput};
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventType,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

pub fn spawn(tx: Sender<InputEvent>, shared: SharedInput) -> HookStatus {
    if !accessibility_trusted(true) {
        let _ = tx.send(InputEvent::Status(HookStatus::NeedsAccessibility));
        log::warn!(
            "Accessibility not granted — open System Settings → Privacy & Security → Accessibility"
        );
        return HookStatus::NeedsAccessibility;
    }

    let tx_thread = tx.clone();
    std::thread::Builder::new()
        .name("lolping-cgeventtap".into())
        .spawn(move || {
            if let Err(status) = run_tap(tx_thread.clone(), shared) {
                let _ = tx_thread.send(InputEvent::Status(status));
            }
        })
        .ok();

    HookStatus::Starting
}

fn accessibility_trusted(prompt: bool) -> bool {
    if prompt {
        macos_accessibility_client::accessibility::application_is_trusted_with_prompt()
    } else {
        macos_accessibility_client::accessibility::application_is_trusted()
    }
}

fn run_tap(tx: Sender<InputEvent>, shared: SharedInput) -> Result<(), HookStatus> {
    let last_ping = Arc::new(AtomicBool::new(false));
    let last_emote = Arc::new(AtomicBool::new(false));
    let tx_cb = tx.clone();
    let last_ping_cb = last_ping.clone();
    let last_emote_cb = last_emote.clone();

    let events_of_interest = vec![
        CGEventType::FlagsChanged,
        CGEventType::MouseMoved,
        CGEventType::LeftMouseDragged,
        CGEventType::RightMouseDragged,
        CGEventType::OtherMouseDragged,
    ];

    let tap = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        // Conservative: listen only — do not swallow Option/Control.
        CGEventTapOptions::ListenOnly,
        events_of_interest,
        move |_proxy, ev_type, event: &CGEvent| {
            match ev_type {
                CGEventType::FlagsChanged => {
                    let flags = event.get_flags();
                    let ping = flags.contains(CGEventFlags::CGEventFlagAlternate);
                    let emote = flags.contains(CGEventFlags::CGEventFlagControl);
                    if last_ping_cb.swap(ping, Ordering::SeqCst) != ping {
                        let _ = tx_cb.send(InputEvent::PingModifier { down: ping });
                    }
                    if last_emote_cb.swap(emote, Ordering::SeqCst) != emote {
                        let _ = tx_cb.send(InputEvent::EmoteModifier { down: emote });
                    }
                }
                CGEventType::MouseMoved
                | CGEventType::LeftMouseDragged
                | CGEventType::RightMouseDragged
                | CGEventType::OtherMouseDragged => {
                    let loc = event.location();
                    shared.update(|g| {
                        g.mouse_x = loc.x;
                        g.mouse_y = loc.y;
                        g.move_count = g.move_count.saturating_add(1);
                    });
                    let _ = tx_cb.send(InputEvent::MouseMove {
                        x: loc.x,
                        y: loc.y,
                    });
                }
                CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
                    log::warn!("CGEventTap disabled by system ({ev_type:?}); re-enable required");
                }
                _ => {}
            }
            Some(event.clone())
        },
    )
    .map_err(|_| HookStatus::Failed("CGEventTap::new failed (permission or session)"))?;

    unsafe {
        let source = tap
            .mach_port
            .create_runloop_source(0)
            .map_err(|_| HookStatus::Failed("create_runloop_source failed"))?;
        CFRunLoop::get_current().add_source(&source, kCFRunLoopCommonModes);
        tap.enable();
    }

    let _ = tx.send(InputEvent::Status(HookStatus::Running));
    log::info!("CGEventTap running (ListenOnly; Option=ping, Control=emote)");
    CFRunLoop::run_current();
    Ok(())
}
