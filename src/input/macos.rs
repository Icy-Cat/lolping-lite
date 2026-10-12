//! macOS CGEventTap — respects SharedConfig triggers + pause (Ctrl+Alt+P).

use super::{HookStatus, InputConfig, InputEvent, SharedConfig, SharedInput};
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventType, EventField,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

/// ANSI P on US keyboard (kVK_ANSI_P).
const KEY_P: i64 = 0x23;

pub fn spawn(tx: Sender<InputEvent>, shared: SharedInput, config: SharedConfig) -> HookStatus {
    if !accessibility_trusted(true) {
        let _ = tx.send(InputEvent::Status(HookStatus::NeedsAccessibility));
        return HookStatus::NeedsAccessibility;
    }
    let tx_thread = tx.clone();
    std::thread::Builder::new()
        .name("lolping-cgeventtap".into())
        .spawn(move || {
            if let Err(status) = run_tap(tx_thread.clone(), shared, config) {
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

fn run_tap(
    tx: Sender<InputEvent>,
    shared: SharedInput,
    config: SharedConfig,
) -> Result<(), HookStatus> {
    let last_ping = Arc::new(AtomicBool::new(false));
    let last_emote = Arc::new(AtomicBool::new(false));
    let tx_cb = tx.clone();
    let last_ping_cb = last_ping.clone();
    let last_emote_cb = last_emote.clone();

    let events = vec![
        CGEventType::FlagsChanged,
        CGEventType::KeyDown,
        CGEventType::MouseMoved,
        CGEventType::LeftMouseDragged,
        CGEventType::RightMouseDragged,
        CGEventType::OtherMouseDragged,
    ];

    let tap = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::ListenOnly,
        events,
        move |_proxy, ev_type, event: &CGEvent| {
            let cfg = config.lock().ok().map(|c| c.clone());
            let Some(cfg) = cfg else {
                return Some(event.clone());
            };

            match ev_type {
                CGEventType::FlagsChanged => {
                    let flags = event.get_flags();
                    let alt = flags.contains(CGEventFlags::CGEventFlagAlternate);
                    let ctrl = flags.contains(CGEventFlags::CGEventFlagControl);
                    let shift = flags.contains(CGEventFlags::CGEventFlagShift);
                    let meta = flags.contains(CGEventFlags::CGEventFlagCommand);
                    let (ping, emote) = if cfg.enabled {
                        cfg.map_modifiers(alt, ctrl, shift, meta)
                    } else {
                        (false, false)
                    };
                    if last_ping_cb.swap(ping, Ordering::SeqCst) != ping {
                        let _ = tx_cb.send(InputEvent::PingModifier { down: ping });
                    }
                    if last_emote_cb.swap(emote, Ordering::SeqCst) != emote {
                        let _ = tx_cb.send(InputEvent::EmoteModifier { down: emote });
                    }
                }
                CGEventType::KeyDown => {
                    let key = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
                    let flags = event.get_flags();
                    let alt = flags.contains(CGEventFlags::CGEventFlagAlternate);
                    let ctrl = flags.contains(CGEventFlags::CGEventFlagControl);
                    let shift = flags.contains(CGEventFlags::CGEventFlagShift);
                    let meta = flags.contains(CGEventFlags::CGEventFlagCommand);
                    if key == KEY_P && cfg.pause_chord_matches(alt, ctrl, shift, meta) {
                        let _ = tx_cb.send(InputEvent::ToggleEnabled);
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
                _ => {}
            }
            Some(event.clone())
        },
    )
    .map_err(|_| HookStatus::Failed("CGEventTap::new failed"))?;

    unsafe {
        let source = tap
            .mach_port
            .create_runloop_source(0)
            .map_err(|_| HookStatus::Failed("create_runloop_source failed"))?;
        CFRunLoop::get_current().add_source(&source, kCFRunLoopCommonModes);
        tap.enable();
    }
    let _ = tx.send(InputEvent::Status(HookStatus::Running));
    CFRunLoop::run_current();
    Ok(())
}

#[allow(dead_code)]
fn _cfg_type(_: &InputConfig) {}
