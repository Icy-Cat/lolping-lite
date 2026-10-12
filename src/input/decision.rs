//! Platform-independent decision helpers ported from upstream lolPing's hook helper
//! (`native/hook-helper/src/decision.cpp`, `hooks_win.cpp`), adapted to lolping-lite's
//! hold-trigger gesture model.
//!
//! * [`ChordGuard`] — c104b08 "Pass clicks through while the trigger is part of another chord".
//! * [`covers_monitor`] — 6030e94 / 645cad9 / bd64923 / 684de87 fullscreen-app probe rule.
//! * [`InjectQueue`] — 9256530 / e2e26a1 FIFO inject queue drained from the message loop,
//!   with swallow-and-replay so real input never overtakes queued injects.

use std::collections::VecDeque;

/// How a key-down relates to the wheel triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyClass {
    /// Shift / Ctrl / Alt / Win / Cmd / CapsLock …: never marks a chord (Alt+Shift+click keeps working).
    Modifier,
    /// One of the configured wheel triggers.
    Trigger,
    /// Esc: lolping-lite's own wheel cancel key, handled by the gesture itself.
    Cancel,
    /// Anything else (Tab, F4, letters …).
    Other,
}

/// Windows VK classification.
pub fn classify_vk(vk: u32) -> KeyClass {
    match vk {
        0x10 | 0x11 | 0x12 | 0xA0..=0xA5 | 0x5B | 0x5C | 0x14 => KeyClass::Modifier,
        0x1B => KeyClass::Cancel,
        _ => KeyClass::Other,
    }
}

/// macOS virtual keycode classification (kVK_*).
pub fn classify_mac_keycode(code: i64) -> KeyClass {
    match code {
        0x37 | 0x36 | 0x38 | 0x3C | 0x3A | 0x3D | 0x3B | 0x3E | 0x39 | 0x3F => KeyClass::Modifier,
        0x35 => KeyClass::Cancel,
        _ => KeyClass::Other,
    }
}

/// Tracks "the held trigger belongs to another program's chord" (Alt+Tab, Win+Tab, Alt+F4 …).
///
/// Upstream marks the chord only while Idle (a key arriving mid-gesture, e.g. Esc, says
/// nothing about the next one). lolping-lite opens its wheel as soon as the trigger goes
/// down, so "Idle" here means *no wheel slot chosen yet* (cursor still in the dead zone).
#[derive(Debug, Default, Clone)]
pub struct ChordGuard {
    foreign: bool,
}

impl ChordGuard {
    /// A key went down. Returns true if this marks a foreign chord.
    pub fn on_key_down(&mut self, class: KeyClass, trigger_held: bool, idle: bool) -> bool {
        if idle && class == KeyClass::Other && trigger_held {
            self.foreign = true;
            return true;
        }
        false
    }

    /// Trigger state changed or was re-synced from the OS (Win+L, UAC, lost key-ups):
    /// once no trigger is held the chord is over.
    pub fn sync(&mut self, any_trigger_held: bool) {
        if !any_trigger_held {
            self.foreign = false;
        }
    }

    /// While set, gestures must not start (clicks / trigger holds belong to the other chord).
    pub fn blocked(&self) -> bool {
        self.foreign
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Upstream 684de87 rule: the foreground window counts as fullscreen on a monitor when its
/// rect *contains* the monitor rect (borderless fullscreen equals it; CS2's maximized SDL
/// window overflows it while keeping WS_CAPTION — bd64923: so no caption check in general).
/// A maximized window normally stops at the work area; only when the monitor reserves no
/// work area (auto-hide taskbar / menu bar) could a maximized window cover it, and there only
/// caption-less windows count.
pub fn covers_monitor(win: Rect, monitor: Rect, work: Rect, has_caption: bool) -> bool {
    if win.left > monitor.left || win.top > monitor.top || win.right < monitor.right || win.bottom < monitor.bottom {
        return false;
    }
    if work == monitor {
        return !has_caption;
    }
    true
}

/// Shell surfaces that also cover the monitor but are not apps.
pub fn is_shell_window_class(class: &str) -> bool {
    ["Progman", "WorkerW", "Shell_TrayWnd"].iter().any(|c| c.eq_ignore_ascii_case(class))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Btn {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inject {
    ButtonDown(Btn),
    ButtonUp(Btn),
    KeyDown(u32),
    KeyUp(u32),
}

/// FIFO of injections decided inside a hook callback, sent later from the message loop.
///
/// SendInput from inside a WH_*_LL callback deadlocks against that very hook (the injected
/// events must pass through the hook thread that is blocked in SendInput), so callbacks only
/// queue; the loop drains after the callback has returned. Callbacks and the loop share one
/// thread, so no lock is needed in the real hook — this type is plain data for testability.
#[derive(Debug, Default)]
pub struct InjectQueue {
    pending: VecDeque<Inject>,
}

impl InjectQueue {
    /// Queue injects. Returns true when the caller must post the "drain" message.
    pub fn queue(&mut self, injects: &[Inject]) -> bool {
        if injects.is_empty() {
            return false;
        }
        self.pending.extend(injects.iter().copied());
        true
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Take everything queued so far, in order.
    pub fn drain(&mut self) -> Vec<Inject> {
        self.pending.drain(..).collect()
    }

    /// e2e26a1 `applyInOrder`: given a real (not self-injected) button/key event and whether the
    /// decision already swallows it, decide whether to swallow it so it can't overtake injects
    /// still in the queue — in which case it is replayed behind them. Returns `(swallow, post)`.
    pub fn apply_in_order(&mut self, self_injected: bool, decided_swallow: bool, replay: Inject) -> (bool, bool) {
        let behind = !self_injected && !self.pending.is_empty();
        if decided_swallow {
            return (true, false);
        }
        if !behind {
            return (false, false);
        }
        let post = self.queue(&[replay]);
        (true, post)
    }
}

/// VKs that need KEYEVENTF_EXTENDEDKEY when injected (upstream `isExtendedVk`).
pub fn is_extended_vk(vk: u32) -> bool {
    matches!(vk, 0xA5 | 0xA3 | 0x5B | 0x5C | 0x2D | 0x2E | 0x24 | 0x23 | 0x21 | 0x22 | 0x25..=0x28)
}

/// `dwExtraInfo` marker on our own SendInput events ("LPIN").
pub const SELF_MARKER: usize = 0x4C50_494E;

#[cfg(test)]
mod tests {
    //! Mirrors upstream `helper_tests.cpp` cases, mapped onto the hold-trigger model.
    use super::*;

    const TAB: u32 = 0x09;
    const ALT: u32 = 0xA4;
    const LWIN: u32 = 0x5B;

    // test_alt_tab_clicks_pass_through
    #[test]
    fn alt_tab_blocks_until_trigger_released() {
        let mut g = ChordGuard::default();
        assert_eq!(classify_vk(ALT), KeyClass::Modifier);
        assert!(!g.on_key_down(classify_vk(ALT), true, true), "the trigger itself is no chord");
        assert!(g.on_key_down(classify_vk(TAB), true, true), "the switcher's key marks the chord");
        assert!(g.blocked());
        g.sync(true); // Alt still held
        assert!(g.blocked());
        g.sync(false); // Alt up: chord over
        assert!(!g.blocked(), "Alt+gesture works again");
    }

    // test_win_tab_clicks_pass_through_and_sync_clears_flag
    #[test]
    fn win_tab_and_resync_clears() {
        let mut g = ChordGuard::default();
        assert_eq!(classify_vk(LWIN), KeyClass::Modifier);
        g.on_key_down(classify_vk(TAB), true, true);
        assert!(g.blocked());
        g.sync(false); // OS reports every key up (Win+L, UAC …)
        assert!(!g.blocked());
    }

    #[test]
    fn modifiers_esc_and_mid_gesture_keys_do_not_mark() {
        let mut g = ChordGuard::default();
        g.on_key_down(classify_vk(0xA0), true, true); // Shift: Alt+Shift+gesture keeps working
        g.on_key_down(classify_vk(0x1B), true, true); // Esc cancels the wheel itself
        g.on_key_down(classify_vk(TAB), true, false); // mid-gesture: says nothing about the next
        g.on_key_down(classify_vk(TAB), false, true); // no trigger held
        assert!(!g.blocked());
        assert_eq!(classify_mac_keycode(0x30), KeyClass::Other); // kVK_Tab
        assert_eq!(classify_mac_keycode(0x3A), KeyClass::Modifier); // kVK_Option
        assert_eq!(classify_mac_keycode(0x35), KeyClass::Cancel);
    }

    fn r(l: i32, t: i32, rr: i32, b: i32) -> Rect {
        Rect { left: l, top: t, right: rr, bottom: b }
    }

    #[test]
    fn fullscreen_rule() {
        let mon = r(0, 0, 1920, 1080);
        let work = r(0, 0, 1920, 1040); // taskbar reserves 40px
        // borderless fullscreen equals the monitor
        assert!(covers_monitor(mon, mon, work, false));
        // CS2: maximized SDL window keeps WS_CAPTION and overflows the monitor (bd64923)
        assert!(covers_monitor(r(-8, -8, 1928, 1088), mon, work, true));
        // a maximized normal window stops at the work area
        assert!(!covers_monitor(r(-8, -8, 1928, 1048), mon, work, true));
        // auto-hide taskbar (work == monitor): only caption-less windows count (684de87)
        assert!(!covers_monitor(r(-8, -8, 1928, 1088), mon, mon, true));
        assert!(covers_monitor(mon, mon, mon, false));
        // a window on another monitor
        assert!(!covers_monitor(r(1920, 0, 3840, 1080), mon, work, false));
        assert!(is_shell_window_class("progman") && is_shell_window_class("Shell_TrayWnd"));
        assert!(!is_shell_window_class("SDL_app"));
    }

    #[test]
    fn inject_queue_fifo_and_in_order_replay() {
        let mut q = InjectQueue::default();
        assert!(!q.queue(&[]));
        assert!(q.queue(&[Inject::ButtonDown(Btn::Left), Inject::ButtonUp(Btn::Left)]));
        // a real key-up arriving while injects wait would overtake them: swallow + replay behind
        assert_eq!(q.apply_in_order(false, false, Inject::KeyUp(ALT)), (true, true));
        // our own injected events never get re-queued
        assert_eq!(q.apply_in_order(true, false, Inject::KeyUp(ALT)), (false, false));
        // already swallowed by the decision: nothing to replay
        assert_eq!(q.apply_in_order(false, true, Inject::KeyDown(TAB)), (true, false));
        assert_eq!(
            q.drain(),
            vec![Inject::ButtonDown(Btn::Left), Inject::ButtonUp(Btn::Left), Inject::KeyUp(ALT)]
        );
        // empty queue: real input passes straight through
        assert_eq!(q.apply_in_order(false, false, Inject::ButtonDown(Btn::Right)), (false, false));
        assert!(q.is_empty());
    }

    #[test]
    fn extended_vks() {
        assert!(is_extended_vk(0xA5) && is_extended_vk(0x26) && is_extended_vk(0x5B));
        assert!(!is_extended_vk(0xA4) && !is_extended_vk(0x41));
    }
}
