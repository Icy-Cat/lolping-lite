//! Windows low-level keyboard/mouse hooks (`WH_KEYBOARD_LL` / `WH_MOUSE_LL`).
//!
//! Compiles on Windows targets. Listen-only (no swallow). Maps settings triggers
//! and Ctrl+Alt+P pause via SharedConfig.

use super::{HookStatus, InputEvent, SharedConfig, SharedInput};
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

static TX: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());
static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn spawn(tx: Sender<InputEvent>, shared: SharedInput, config: SharedConfig) -> HookStatus {
    let tx_thread = tx.clone();
    std::thread::Builder::new()
        .name("lolping-winhooks".into())
        .spawn(move || {
            if let Err(e) = run(tx_thread.clone(), shared, config) {
                let _ = tx_thread.send(InputEvent::Status(HookStatus::Failed(e)));
            }
        })
        .ok();
    HookStatus::Starting
}

#[cfg(target_os = "windows")]
fn run(
    tx: Sender<InputEvent>,
    shared: SharedInput,
    config: SharedConfig,
) -> Result<(), &'static str> {
    use std::mem;
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HC_ACTION,
        HHOOK, KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN,
        WM_SYSKEYDOWN, WINDOWS_HOOK_ID,
    };

    // Store boxed state for hooks — simplified: use channel only + config Arc.
    let boxed = Box::new((tx.clone(), shared, config));
    let ptr = Box::into_raw(boxed) as *mut ();
    TX.store(ptr, Ordering::SeqCst);
    RUNNING.store(true, Ordering::SeqCst);

    unsafe extern "system" fn keyboard_proc(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code as u32 == HC_ACTION {
            let kb = *(l.0 as *const KBDLLHOOKSTRUCT);
            let state = TX.load(Ordering::SeqCst) as *mut (Sender<InputEvent>, SharedInput, SharedConfig);
            if !state.is_null() {
                let (tx, _shared, config) = &*state;
                if let Ok(cfg) = config.lock() {
                    // VK_MENU=0x12 VK_CONTROL=0x11 VK_SHIFT=0x10 VK_LWIN=0x5B
                    // We approximate with GetAsyncKeyState in a fuller impl; here
                    // track via injected flagsChanged-equivalent using async state.
                    let _ = (tx, kb, w);
                    let msg = w.0 as u32;
                    if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                        if kb.vkCode == 0x50 && cfg.pause_requires_ctrl && cfg.pause_requires_alt {
                            // Check modifiers via GetKeyState
                            use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
                            let ctrl = GetAsyncKeyState(0x11) < 0;
                            let alt = GetAsyncKeyState(0x12) < 0;
                            let shift = GetAsyncKeyState(0x10) < 0;
                            let meta = GetAsyncKeyState(0x5B) < 0 || GetAsyncKeyState(0x5C) < 0;
                            if cfg.pause_chord_matches(alt, ctrl, shift, meta) {
                                let _ = tx.send(InputEvent::ToggleEnabled);
                            }
                        }
                    }
                }
            }
        }
        CallNextHookEx(HHOOK(std::ptr::null_mut()), code, w, l)
    }

    unsafe extern "system" fn mouse_proc(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code as u32 == HC_ACTION {
            let ms = *(l.0 as *const MSLLHOOKSTRUCT);
            let state = TX.load(Ordering::SeqCst) as *mut (Sender<InputEvent>, SharedInput, SharedConfig);
            if !state.is_null() {
                let (tx, shared, config) = &*state;
                shared.update(|g| {
                    g.mouse_x = ms.pt.x as f64;
                    g.mouse_y = ms.pt.y as f64;
                    g.move_count = g.move_count.saturating_add(1);
                });
                let _ = tx.send(InputEvent::MouseMove {
                    x: ms.pt.x as f64,
                    y: ms.pt.y as f64,
                });
                // Poll modifiers for hold detection
                if let Ok(cfg) = config.lock() {
                    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
                    let ctrl = GetAsyncKeyState(0x11) < 0;
                    let alt = GetAsyncKeyState(0x12) < 0;
                    let shift = GetAsyncKeyState(0x10) < 0;
                    let meta = GetAsyncKeyState(0x5B) < 0 || GetAsyncKeyState(0x5C) < 0;
                    let (ping, emote) = if cfg.enabled {
                        cfg.map_modifiers(alt, ctrl, shift, meta)
                    } else {
                        (false, false)
                    };
                    static LAST_PING: AtomicBool = AtomicBool::new(false);
                    static LAST_EMOTE: AtomicBool = AtomicBool::new(false);
                    if LAST_PING.swap(ping, Ordering::SeqCst) != ping {
                        let _ = tx.send(InputEvent::PingModifier { down: ping });
                    }
                    if LAST_EMOTE.swap(emote, Ordering::SeqCst) != emote {
                        let _ = tx.send(InputEvent::EmoteModifier { down: emote });
                    }
                }
            }
        }
        CallNextHookEx(HHOOK(std::ptr::null_mut()), code, w, l)
    }

    unsafe {
        let kbd = SetWindowsHookExW(
            WINDOWS_HOOK_ID(WH_KEYBOARD_LL.0),
            Some(keyboard_proc),
            None,
            0,
        )
        .map_err(|_| "SetWindowsHookExW keyboard")?;
        let mouse = SetWindowsHookExW(
            WINDOWS_HOOK_ID(WH_MOUSE_LL.0),
            Some(mouse_proc),
            None,
            0,
        )
        .map_err(|_| "SetWindowsHookExW mouse")?;
        let _ = tx.send(InputEvent::Status(HookStatus::Running));
        let mut msg = mem::zeroed();
        while RUNNING.load(Ordering::SeqCst) && GetMessageW(&mut msg, None, 0, 0).into() {
            // message pump required for LL hooks
        }
        let _ = UnhookWindowsHookEx(kbd);
        let _ = UnhookWindowsHookEx(mouse);
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn run(
    _tx: Sender<InputEvent>,
    _shared: SharedInput,
    _config: SharedConfig,
) -> Result<(), &'static str> {
    Err("windows hooks only on windows")
}
