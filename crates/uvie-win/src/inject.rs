//! Text injection via `SendInput` — the Windows counterpart of uvie-mac's
//! `SyntheticOutput`/CGEvent posting.
//!
//! Every injected event is marked with `UVIE_EXTRA_INFO` in `dwExtraInfo`
//! *and* arrives back through our hook with `LLKHF_INJECTED` set, so echoes
//! are reliably identified by the dispatcher.

use uvie_core::dispatcher::InputAction;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_BACK, VK_DELETE, VK_LEFT, VK_RIGHT,
    VK_SHIFT,
};

/// Magic `dwExtraInfo` tag for our own synthesized keystrokes. The
/// keyboard hook checks this so that only *our* echoes are filtered —
/// injected input from VNC/RDP/automation still composes normally.
pub(crate) const UVIE_EXTRA_INFO: usize = 0x5556_4945; // "UVIE"

/// Execute a dispatch plan.
///
/// `chromium_mode` enables the select-and-overwrite workaround uvie-mac
/// applies to Chromium browsers: instead of plain Backspaces (which some
/// Chromium text fields — e.g. the omnibox — swallow or reorder), each
/// `Backspace(n)` becomes `Shift+Left × n` to select the text, then the
/// following `Text` action overwrites the selection. When no text follows,
/// a single `Delete` removes the selection. `ForwardDelete(n)` mirrors it
/// with `Shift+Right × n` + `Delete`.
pub fn inject(plan: &[InputAction], chromium_mode: bool) {
    let inputs = build_inputs(plan, chromium_mode, shift_held());
    if inputs.is_empty() {
        return;
    }
    unsafe {
        // SendInput may insert fewer events than requested (input can be
        // blocked by another thread); retry the remainder so a truncated
        // plan can't silently drop output.
        let mut sent = 0;
        while sent < inputs.len() {
            let n = SendInput(&inputs[sent..], std::mem::size_of::<INPUT>() as i32);
            if n == 0 {
                break;
            }
            sent += n as usize;
        }
    }
}

/// Whether the user is physically holding Shift right now. Injected
/// events take their modifiers from the live key state: a `Delete` sent
/// while Shift is held reads as Shift+Delete (a *cut*), and a synthetic
/// Shift release would leave the user's held key reported as up —
/// turning the rest of an ALL-CAPS word lowercase.
fn shift_held() -> bool {
    unsafe { GetAsyncKeyState(VK_SHIFT.0 as i32) & 0x8000u16 as i16 != 0 }
}

/// Translate a dispatch plan into raw `INPUT` events (testable — the
/// SendInput call is the only un-testable shell around it).
///
/// `shift` tracks the Shift state the event stream has established so
/// far, seeded with the physical state: steps that need Shift emit it on
/// demand, steps that need it *absent* (a bare Delete) release it, and
/// the final `set_shift` restores whatever the user is holding.
fn build_inputs(plan: &[InputAction], chromium_mode: bool, physical_shift: bool) -> Vec<INPUT> {
    let mut inputs: Vec<INPUT> = Vec::new();
    let mut shift = physical_shift;
    for (i, action) in plan.iter().enumerate() {
        match action {
            InputAction::Backspace(n) => {
                if chromium_mode {
                    let overwrites = plan[i + 1..]
                        .iter()
                        .any(|a| matches!(a, InputAction::Text(s) if !s.is_empty()));
                    push_select(inputs.as_mut(), &mut shift, *n, VK_LEFT);
                    if !overwrites {
                        push_delete(inputs.as_mut(), &mut shift);
                    }
                } else {
                    // Shift+Backspace means forward-delete in some
                    // editors — unshift like a bare Delete does.
                    set_shift(inputs.as_mut(), &mut shift, false);
                    for _ in 0..*n {
                        push_key(inputs.as_mut(), VK_BACK);
                    }
                }
            }
            InputAction::ForwardDelete(n) => {
                if chromium_mode {
                    // A later Backspace selection deselects the rightward
                    // span, so the tail must be removed with a real
                    // Delete right away.
                    push_select(inputs.as_mut(), &mut shift, *n, VK_RIGHT);
                    push_delete(inputs.as_mut(), &mut shift);
                } else {
                    for _ in 0..*n {
                        push_delete(inputs.as_mut(), &mut shift);
                    }
                }
            }
            InputAction::Text(s) => push_text(inputs.as_mut(), s),
        }
    }
    set_shift(&mut inputs, &mut shift, physical_shift);
    inputs
}

/// Emit a Shift press/release only when the stream's tracked state
/// differs — never a synthetic tap that fights the user's held key.
fn set_shift(inputs: &mut Vec<INPUT>, cur: &mut bool, down: bool) {
    if *cur == down {
        return;
    }
    let flags = if down {
        KEYBD_EVENT_FLAGS(0)
    } else {
        KEYEVENTF_KEYUP
    };
    inputs.push(key_input(VK_SHIFT, 0, flags));
    *cur = down;
}

/// Select `n` characters to one side of the caret (`vk` = VK_LEFT or
/// VK_RIGHT). A subsequent text input replaces the selection — the
/// Chromium workaround.
fn push_select(inputs: &mut Vec<INPUT>, shift: &mut bool, n: usize, vk: VIRTUAL_KEY) {
    if n == 0 {
        return;
    }
    set_shift(inputs, shift, true);
    for _ in 0..n {
        push_key(inputs, vk);
    }
}

/// A forward-delete that can never read as Shift+Delete (a cut) — the
/// shift stream is released first and restored at the end of the plan.
fn push_delete(inputs: &mut Vec<INPUT>, shift: &mut bool) {
    set_shift(inputs, shift, false);
    push_key(inputs, VK_DELETE);
}

fn key_input(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: UVIE_EXTRA_INFO,
            },
        },
    }
}

fn push_key(inputs: &mut Vec<INPUT>, vk: VIRTUAL_KEY) {
    inputs.push(key_input(vk, 0, KEYBD_EVENT_FLAGS(0)));
    inputs.push(key_input(vk, 0, KEYEVENTF_KEYUP));
}

/// Unicode text via `KEYEVENTF_UNICODE` surrogate pairs — the reliable
/// SendInput path for arbitrary UTF-16 (including precomposed Vietnamese).
fn push_text(inputs: &mut Vec<INPUT>, text: &str) {
    for unit in text.encode_utf16() {
        inputs.push(key_input(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE));
        inputs.push(key_input(
            VIRTUAL_KEY(0),
            unit,
            KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flatten a built INPUT sequence to (vk, scan, flags) triples for
    /// assertions. VK 0 entries carry UTF-16 code units in `scan`.
    fn k(inputs: &[INPUT]) -> Vec<(u16, u16, u32)> {
        inputs
            .iter()
            .map(|i| unsafe {
                let ki = i.Anonymous.ki;
                (ki.wVk.0, ki.wScan, ki.dwFlags.0)
            })
            .collect()
    }

    const UP: u32 = KEYEVENTF_KEYUP.0;
    const UNI: u32 = KEYEVENTF_UNICODE.0;
    const SH: u16 = VK_SHIFT.0;
    const LF: u16 = VK_LEFT.0;
    const RT: u16 = VK_RIGHT.0;
    const DEL: u16 = VK_DELETE.0;

    #[test]
    fn normal_mode_uses_plain_backspaces() {
        let inputs = build_inputs(&[InputAction::Backspace(2)], false, false);
        assert_eq!(
            k(&inputs),
            vec![
                (VK_BACK.0, 0, 0),
                (VK_BACK.0, 0, UP),
                (VK_BACK.0, 0, 0),
                (VK_BACK.0, 0, UP),
            ]
        );
    }

    #[test]
    fn chromium_selects_then_deletes_when_nothing_follows() {
        let inputs = build_inputs(&[InputAction::Backspace(2)], true, false);
        assert_eq!(
            k(&inputs),
            vec![
                (SH, 0, 0), // Shift down
                (LF, 0, 0),
                (LF, 0, UP),
                (LF, 0, 0),
                (LF, 0, UP),
                (SH, 0, UP), // released so the Delete is no cut
                (DEL, 0, 0),
                (DEL, 0, UP),
            ]
        );
    }

    #[test]
    fn chromium_selects_then_overwrites_when_text_follows() {
        // uvie-mac's omnibox workaround: Shift+Left selects, the typed text
        // replaces the selection — no Delete key needed.
        let inputs = build_inputs(
            &[InputAction::Backspace(2), InputAction::Text("v".into())],
            true,
            false,
        );
        assert_eq!(
            k(&inputs),
            vec![
                (SH, 0, 0),
                (LF, 0, 0),
                (LF, 0, UP),
                (LF, 0, 0),
                (LF, 0, UP),
                (0, 'v' as u16, UNI),
                (0, 'v' as u16, UNI | UP),
                (SH, 0, UP), // restore: Shift is physically up
            ]
        );
    }

    #[test]
    fn forward_delete_and_unicode_text() {
        let inputs = build_inputs(
            &[InputAction::ForwardDelete(1), InputAction::Text("ệ".into())],
            false,
            false,
        );
        let ks = k(&inputs);
        assert_eq!(&ks[..2], &[(VK_DELETE.0, 0, 0), (VK_DELETE.0, 0, UP)]);
        // "ệ" is a single UTF-16 code unit (U+1EC7).
        assert_eq!(&ks[2..], &[(0, 0x1EC7, UNI), (0, 0x1EC7, UNI | UP)]);
    }

    #[test]
    fn chromium_forward_delete_selects_right_then_deletes() {
        // Mid-word edit plan: remove the tail right of the caret, erase
        // the prefix, re-type. The forward half selects Shift+Right then
        // deletes — same trick as Backspace.
        let inputs = build_inputs(
            &[
                InputAction::ForwardDelete(1),
                InputAction::Backspace(1),
                InputAction::Text("ệ".into()),
            ],
            true,
            false,
        );
        assert_eq!(
            k(&inputs),
            vec![
                (SH, 0, 0),
                (RT, 0, 0),
                (RT, 0, UP),
                (SH, 0, UP), // released for a real Delete
                (DEL, 0, 0),
                (DEL, 0, UP),
                (SH, 0, 0),
                (LF, 0, 0),
                (LF, 0, UP),
                (0, 0x1EC7, UNI),
                (0, 0x1EC7, UNI | UP),
                (SH, 0, UP), // restore
            ]
        );
    }

    #[test]
    fn held_shift_is_never_synthetically_released_for_selects() {
        // Typing a capitalized word keeps Shift physically held through a
        // rewrite: emitting Shift down+up would report the held key as
        // released and lowercase the rest of the word. The Lefts inherit
        // the physical state instead; only the Delete needs a release,
        // and the held state is restored afterwards.
        let inputs = build_inputs(&[InputAction::Backspace(2)], true, true);
        assert_eq!(
            k(&inputs),
            vec![
                (LF, 0, 0),
                (LF, 0, UP),
                (LF, 0, 0),
                (LF, 0, UP),
                (SH, 0, UP), // release for a real Delete...
                (DEL, 0, 0),
                (DEL, 0, UP),
                (SH, 0, 0), // ...then restore the held key
            ]
        );
    }

    #[test]
    fn held_shift_forward_delete_restores_state() {
        let inputs = build_inputs(&[InputAction::ForwardDelete(1)], false, true);
        assert_eq!(
            k(&inputs),
            vec![
                (SH, 0, UP), // unshift so Delete isn't a cut
                (DEL, 0, 0),
                (DEL, 0, UP),
                (SH, 0, 0), // restore the physically held key
            ]
        );
    }
}
