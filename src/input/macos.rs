//! macOS CGEventTap — respects SharedConfig triggers + pause (Ctrl+Alt+P).
//!
//! Keeps the status channel alive: if Accessibility is missing or the tap dies,
//! we retry instead of dropping the Sender (which used to surface as
//! `Failed("input channel disconnected")` and left the UI looking dead).

use super::{HookStatus, InputConfig, InputEvent, SharedConfig, SharedInput};
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventType, EventField,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

/// ANSI P on US keyboard (kVK_ANSI_P).
const KEY_P: i64 = 0x23;

pub fn spawn(tx: Sender<InputEvent>, shared: SharedInput, config: SharedConfig) -> HookStatus {
    let tx_thread = tx.clone();
    match std::thread::Builder::new()
        .name("lolping-cgeventtap".into())
        .spawn(move || supervisor(tx_thread, shared, config))
    {
        Ok(_) => HookStatus::Starting,
        Err(_) => {
            let _ = tx.send(InputEvent::Status(HookStatus::Failed(
                "failed to spawn CGEventTap thread",
            )));
            HookStatus::Failed("failed to spawn CGEventTap thread")
        }
    }
}

fn supervisor(tx: Sender<InputEvent>, shared: SharedInput, config: SharedConfig) {
    let mut prompted = false;
    loop {
        // Prompt once; then poll quietly so we pick up a grant from System Settings
        // without spamming the OS dialog.
        let trusted = if prompted {
            accessibility_trusted(false)
        } else {
            prompted = true;
            accessibility_trusted(true)
        };
        if !trusted {
            let _ = tx.send(InputEvent::Status(HookStatus::NeedsAccessibility));
            std::thread::sleep(Duration::from_secs(2));
            continue;
        }

        match run_tap(tx.clone(), shared.clone(), config.clone()) {
            Ok(()) => {
                // Run loop ended unexpectedly — retry.
                let _ = tx.send(InputEvent::Status(HookStatus::Failed(
                    "CGEventTap run loop exited",
                )));
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(status) => {
                let _ = tx.send(InputEvent::Status(status));
                // Accessibility can be revoked at runtime; re-check next iteration.
                if matches!(status, HookStatus::NeedsAccessibility) {
                    prompted = false;
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        }
    }
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
    // Re-check right before creating the tap — the prompt may have been denied.
    if !accessibility_trusted(false) {
        return Err(HookStatus::NeedsAccessibility);
    }

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
                    // c104b08: a non-modifier key while a trigger is held (Cmd+Tab, Option+Tab …)
                    // belongs to another chord. Autorepeat doesn't matter: the flag is idempotent.
                    if cfg.enabled
                        && super::decision::classify_mac_keycode(key) == super::decision::KeyClass::Other
                    {
                        let (p, e) = cfg.map_modifiers(alt, ctrl, shift, meta);
                        if p || e {
                            let _ = tx_cb.send(InputEvent::ForeignKey);
                        }
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
    .map_err(|_| HookStatus::Failed("CGEventTap::new failed (check Accessibility)"))?;

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
