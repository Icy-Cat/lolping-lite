//! Windows low-level keyboard/mouse hooks (`WH_KEYBOARD_LL` / `WH_MOUSE_LL`).
//!
//! Reports trigger state, cursor, the pause chord, and foreign chord keys (Alt+Tab …,
//! upstream c104b08). Injections follow upstream 9256530 / e2e26a1: hook callbacks never
//! call SendInput — they queue into a FIFO that the hook thread's message loop drains after
//! the callback has returned (kWmInject); our own events carry the "LPIN" marker; a real
//! button/key event that would overtake still-queued injects is swallowed and replayed behind
//! them; injected keys carry scan codes. lolping-lite's gestures don't swallow input today, so
//! the queue only fills if a future decision emits injects — the ordering guarantees are in place.

use super::{HookStatus, InputEvent, SharedConfig, SharedInput};
#[allow(unused_imports)]
use super::decision::{classify_vk, is_extended_vk, Btn, Inject, InjectQueue, KeyClass, SELF_MARKER};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering};
use std::sync::mpsc::Sender;

static TX: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());
static RUNNING: AtomicBool = AtomicBool::new(false);
static HOOK_THREAD: AtomicU32 = AtomicU32::new(0);

thread_local! {
    /// Injects waiting for the message loop. Hook callbacks and that loop share one thread.
    static QUEUE: std::cell::RefCell<InjectQueue> = std::cell::RefCell::new(InjectQueue::default());
}

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
const WM_INJECT: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 2;

/// Queue injects for the message loop (never SendInput inside a hook callback).
#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub(crate) fn queue_injects(injects: &[Inject]) {
    if QUEUE.with(|q| q.borrow_mut().queue(injects)) {
        post_drain();
    }
}

#[cfg(target_os = "windows")]
fn post_drain() {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW;
    unsafe {
        let _ = PostThreadMessageW(HOOK_THREAD.load(Ordering::SeqCst), WM_INJECT, WPARAM(0), LPARAM(0));
    }
}

#[cfg(target_os = "windows")]
fn drain_injects() {
    let batch = QUEUE.with(|q| q.borrow_mut().drain());
    send_injects(&batch);
}

#[cfg(target_os = "windows")]
fn send_injects(injects: &[Inject]) {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    let mut inputs: Vec<INPUT> = Vec::with_capacity(injects.len());
    for i in injects {
        let mut input = INPUT::default();
        match *i {
            Inject::ButtonDown(b) | Inject::ButtonUp(b) => {
                let down = matches!(i, Inject::ButtonDown(_));
                let (flags, data) = match b {
                    Btn::Left => (if down { MOUSEEVENTF_LEFTDOWN } else { MOUSEEVENTF_LEFTUP }, 0),
                    Btn::Right => (if down { MOUSEEVENTF_RIGHTDOWN } else { MOUSEEVENTF_RIGHTUP }, 0),
                    Btn::Middle => (if down { MOUSEEVENTF_MIDDLEDOWN } else { MOUSEEVENTF_MIDDLEUP }, 0),
                    Btn::X1 => (if down { MOUSEEVENTF_XDOWN } else { MOUSEEVENTF_XUP }, 1u32), // XBUTTON1
                    Btn::X2 => (if down { MOUSEEVENTF_XDOWN } else { MOUSEEVENTF_XUP }, 2u32), // XBUTTON2
                };
                input.r#type = INPUT_MOUSE;
                input.Anonymous.mi = MOUSEINPUT {
                    dwFlags: flags,
                    mouseData: data,
                    dwExtraInfo: SELF_MARKER,
                    ..Default::default()
                };
            }
            Inject::KeyDown(vk) | Inject::KeyUp(vk) => {
                let mut flags = KEYBD_EVENT_FLAGS(0);
                if matches!(i, Inject::KeyUp(_)) {
                    flags |= KEYEVENTF_KEYUP;
                }
                if is_extended_vk(vk) {
                    flags |= KEYEVENTF_EXTENDEDKEY;
                }
                input.r#type = INPUT_KEYBOARD;
                input.Anonymous.ki = KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk as u16),
                    // games read scan codes (e2e26a1)
                    wScan: unsafe { MapVirtualKeyW(vk, MAPVK_VK_TO_VSC) } as u16,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: SELF_MARKER,
                };
            }
        }
        inputs.push(input);
    }
    if !inputs.is_empty() {
        unsafe {
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }
}

/// e2e26a1 applyInOrder for a real button/key event: returns whether to swallow it.
#[cfg(target_os = "windows")]
fn apply_in_order(self_injected: bool, decided_swallow: bool, replay: Inject) -> bool {
    let (swallow, post) = QUEUE.with(|q| q.borrow_mut().apply_in_order(self_injected, decided_swallow, replay));
    if post {
        post_drain();
    }
    swallow
}

#[cfg(target_os = "windows")]
fn modifiers() -> (bool, bool, bool, bool) {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    unsafe {
        (
            GetAsyncKeyState(0x12) < 0,
            GetAsyncKeyState(0x11) < 0,
            GetAsyncKeyState(0x10) < 0,
            GetAsyncKeyState(0x5B) < 0 || GetAsyncKeyState(0x5C) < 0,
        )
    }
}

#[cfg(target_os = "windows")]
fn run(
    tx: Sender<InputEvent>,
    shared: SharedInput,
    config: SharedConfig,
) -> Result<(), &'static str> {
    use std::mem;
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, PeekMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HC_ACTION,
        HHOOK, KBDLLHOOKSTRUCT, LLKHF_INJECTED, LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT, PM_NOREMOVE,
        WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOWS_HOOK_ID, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN,
        WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN,
        WM_SYSKEYUP, WM_USER, WM_XBUTTONDOWN, WM_XBUTTONUP,
    };

    type State = (Sender<InputEvent>, SharedInput, SharedConfig);
    let boxed: Box<State> = Box::new((tx.clone(), shared, config));
    TX.store(Box::into_raw(boxed) as *mut (), Ordering::SeqCst);
    RUNNING.store(true, Ordering::SeqCst);

    unsafe extern "system" fn keyboard_proc(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code as u32 == HC_ACTION {
            let kb = *(l.0 as *const KBDLLHOOKSTRUCT);
            let msg = w.0 as u32;
            let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
            let self_injected = (kb.flags.0 & LLKHF_INJECTED.0) != 0 && kb.dwExtraInfo == SELF_MARKER;
            let state = TX.load(Ordering::SeqCst) as *mut State;
            if !state.is_null() && !self_injected && down {
                let (tx, _shared, config) = &*state;
                if let Ok(cfg) = config.lock() {
                    let (alt, ctrl, shift, meta) = modifiers();
                    if kb.vkCode == 0x50 && cfg.pause_chord_matches(alt, ctrl, shift, meta) {
                        let _ = tx.send(InputEvent::ToggleEnabled);
                    }
                    // c104b08: a non-modifier key while a trigger is held starts another chord.
                    if cfg.enabled && classify_vk(kb.vkCode) == KeyClass::Other {
                        let (p, e) = cfg.map_modifiers(alt, ctrl, shift, meta);
                        if p || e {
                            let _ = tx.send(InputEvent::ForeignKey);
                        }
                    }
                }
            }
            if down || up {
                let replay = if down { Inject::KeyDown(kb.vkCode) } else { Inject::KeyUp(kb.vkCode) };
                if apply_in_order(self_injected, false, replay) {
                    return LRESULT(1);
                }
            }
        }
        CallNextHookEx(HHOOK(std::ptr::null_mut()), code, w, l)
    }

    unsafe extern "system" fn mouse_proc(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code as u32 == HC_ACTION {
            let ms = *(l.0 as *const MSLLHOOKSTRUCT);
            let msg = w.0 as u32;
            let self_injected = (ms.flags & LLMHF_INJECTED) != 0 && ms.dwExtraInfo == SELF_MARKER;
            let xbtn = if (ms.mouseData >> 16) & 0xFFFF == 1 { Btn::X1 } else { Btn::X2 };
            let button = match msg {
                WM_LBUTTONDOWN => Some(Inject::ButtonDown(Btn::Left)),
                WM_LBUTTONUP => Some(Inject::ButtonUp(Btn::Left)),
                WM_RBUTTONDOWN => Some(Inject::ButtonDown(Btn::Right)),
                WM_RBUTTONUP => Some(Inject::ButtonUp(Btn::Right)),
                WM_MBUTTONDOWN => Some(Inject::ButtonDown(Btn::Middle)),
                WM_MBUTTONUP => Some(Inject::ButtonUp(Btn::Middle)),
                WM_XBUTTONDOWN => Some(Inject::ButtonDown(xbtn)),
                WM_XBUTTONUP => Some(Inject::ButtonUp(xbtn)),
                _ => None,
            };
            // Moves may overtake queued injects (a replayed press just lands at the newer position);
            // buttons may not.
            if let Some(replay) = button {
                if apply_in_order(self_injected, false, replay) {
                    return LRESULT(1);
                }
            }
            let state = TX.load(Ordering::SeqCst) as *mut State;
            if !state.is_null() && !self_injected {
                let (tx, shared, config) = &*state;
                shared.update(|g| {
                    g.mouse_x = ms.pt.x as f64;
                    g.mouse_y = ms.pt.y as f64;
                    g.move_count = g.move_count.saturating_add(1);
                });
                let _ = tx.send(InputEvent::MouseMove { x: ms.pt.x as f64, y: ms.pt.y as f64 });
                // Key re-sync from the OS on every mouse event (covers lost key-ups: Win+L, UAC).
                if let Ok(cfg) = config.lock() {
                    let (alt, ctrl, shift, meta) = modifiers();
                    let (ping, emote) = if cfg.enabled { cfg.map_modifiers(alt, ctrl, shift, meta) } else { (false, false) };
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
        HOOK_THREAD.store(GetCurrentThreadId(), Ordering::SeqCst);
        let mut msg: MSG = mem::zeroed();
        // make sure this thread has a message queue before anyone posts to it
        let _ = PeekMessageW(&mut msg, None, WM_USER, WM_USER, PM_NOREMOVE);
        let kbd = SetWindowsHookExW(WINDOWS_HOOK_ID(WH_KEYBOARD_LL.0), Some(keyboard_proc), None, 0)
            .map_err(|_| "SetWindowsHookExW keyboard")?;
        let mouse = SetWindowsHookExW(WINDOWS_HOOK_ID(WH_MOUSE_LL.0), Some(mouse_proc), None, 0)
            .map_err(|_| "SetWindowsHookExW mouse")?;
        let _ = tx.send(InputEvent::Status(HookStatus::Running));
        while RUNNING.load(Ordering::SeqCst) && GetMessageW(&mut msg, None, 0, 0).into() {
            if msg.message == WM_INJECT {
                drain_injects(); // after the hook callback that queued them has returned
            }
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
