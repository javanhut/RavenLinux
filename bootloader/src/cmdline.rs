//! One-boot edits to a kernel command line.
//!
//! # Why this exists
//!
//! The initramfs reads four arguments whose entire purpose is to rescue a
//! machine that no longer boots — `noresume` abandons a hibernation image that
//! is killing the restore, `rootdelay=N` buys a slow disk more time to appear,
//! `crypttries=N` changes how many passphrase attempts you get before the
//! rescue shell, and `raven.live` boots the live image from a disk that has a
//! `root=` of its own. Every one of them is documented in
//! `scripts/build-initramfs.sh` as something you type "at the RavenBoot
//! prompt".
//!
//! There was no prompt. The menu took UP, DOWN, ENTER, ESC and BACKSPACE, and
//! nothing in this crate built or altered a command line at runtime, so the
//! only way to add an argument to an installed machine was to take the disk to
//! another computer and hand-edit `\EFI\raven\boot.cfg` on the ESP. That is the
//! difference between "boot with noresume to recover" and "reinstall", and it
//! is the reason this module is here: `e` on the selected entry opens a prompt,
//! what you type is appended to that entry's command line, and ENTER boots it.
//!
//! # Why it appends rather than replaces
//!
//! Appending is not a shortcut around writing a full line editor; it is the
//! behaviour that actually recovers a machine. Both parsers that read this
//! string — `raven_root_from_cmdline` in the initramfs and the kernel's own —
//! take the last setting of a key as the winner, and the base line already
//! carries the thing you are overriding. `raven-install` writes
//! `resume=UUID=…` into every entry it creates on a machine that has swap, so
//! `noresume` only wins by arriving after it. Putting the typed text in front would mean the broken hibernation
//! image is picked up again, which is the one boot where getting that wrong
//! costs the machine.
//!
//! Replacing the whole line, by contrast, is how you lose `root=` at three in
//! the morning. The full editor can be built on top of this later; nothing here
//! forecloses it.
//!
//! # Nothing is written anywhere
//!
//! The menu hands [`combine`] a *clone* of the entry, so the edit lives in that
//! clone until control leaves for the kernel. `\EFI\raven\boot.cfg` is not
//! reopened, not rewritten, and a failed boot returns to a menu that has
//! forgotten the text. One boot means one boot — the same contract GRUB's `e`
//! has, and the reason an operator can try `noresume` without committing to it.
//!
//! This module deliberately mentions no UEFI type. `main.rs` converts a
//! `Char16` to a `char` before it gets here, which is what lets these rules be
//! tested on the host rather than by rebooting a machine.
//!
//! # Running the tests
//!
//! `cargo test` in this crate cannot: the crate is `no_std` and builds for
//! `x86_64-unknown-uefi`, which has no test harness. The tests below are
//! written to be run the way `preview/` runs `menu.rs` and `gfx.rs` -- by a
//! host crate that pulls the file in by path. Adding
//!
//! ```ignore
//! #[path = "../../src/cmdline.rs"]
//! mod cmdline;
//! ```
//!
//! to `preview/src/main.rs` makes
//! `cargo test --manifest-path preview/Cargo.toml` run them; until somebody
//! does that, they are run by pointing a throwaway host crate at this file the
//! same way. They are worth running: they are the only executable statement of
//! why the typed text is appended rather than prepended.

use alloc::string::{String, ToString};

/// The longest command line worth assembling, in bytes.
///
/// x86_64 Linux copies the command line into a buffer of `COMMAND_LINE_SIZE`
/// bytes — 2048 since 2.6 — and truncates anything past it without saying so.
/// A line silently cut in half is a worse failure than a keystroke that does
/// nothing, because the argument that gets lost is the last one, which here is
/// exactly the one just typed. So the budget is enforced while typing instead.
pub const MAX_CMDLINE: usize = 2048;

/// The text being typed at the prompt, and how much room is left for it.
pub struct Editor {
    extra: String,
    /// Bytes still available after the base line and its separating space.
    budget: usize,
}

impl Editor {
    /// An empty prompt for an entry whose command line is `base`.
    pub fn for_base(base: &str) -> Self {
        // The `+ 1` is the space `combine` inserts. Charging for it here is
        // what keeps a line that is accepted by the prompt from being rejected
        // — or truncated — a moment later.
        let used = base.trim().len().saturating_add(1);
        Self {
            extra: String::new(),
            budget: MAX_CMDLINE.saturating_sub(used),
        }
    }

    /// Take one character. Returns whether it was taken, so a caller that
    /// wants to beep or blink at a rejected key can.
    ///
    /// Only printable ASCII is accepted, and that is a correctness rule rather
    /// than a simplification: `linux.rs` widens this string to UCS-2 one *byte*
    /// at a time (`for (i, byte) in cmdline.bytes().enumerate()`), so a
    /// multi-byte character typed on a non-US keyboard would reach the kernel
    /// as two pieces of mojibake in the middle of an argument. A control
    /// character would be worse: it is invisible at the prompt and splits the
    /// line in the parser.
    pub fn insert(&mut self, c: char) -> bool {
        if !(' '..='~').contains(&c) {
            return false;
        }
        if self.extra.len() >= self.budget {
            return false;
        }
        self.extra.push(c);
        true
    }

    /// Remove the last character. Returns whether there was one.
    pub fn backspace(&mut self) -> bool {
        self.extra.pop().is_some()
    }

    /// What has been typed so far.
    pub fn text(&self) -> &str {
        &self.extra
    }

    /// Whether ENTER would boot the entry exactly as the config describes it.
    pub fn is_empty(&self) -> bool {
        self.extra.trim().is_empty()
    }
}

/// The command line to boot with: `base`, then what was typed.
///
/// Whitespace-only input is not an edit, and returning `base` unchanged for it
/// means an operator who opened the prompt and thought better of it boots the
/// entry the config describes, byte for byte.
pub fn combine(base: &str, extra: &str) -> String {
    let base = base.trim();
    let extra = extra.trim();

    if extra.is_empty() {
        return base.to_string();
    }
    if base.is_empty() {
        return extra.to_string();
    }

    let mut combined = String::with_capacity(base.len() + 1 + extra.len());
    combined.push_str(base);
    combined.push(' ');
    combined.push_str(extra);
    combined
}

/// The last `max_chars` characters of `text`, marked with a leading `<` when
/// something was dropped.
///
/// The status line this is drawn on is one centred row with no wrapping and no
/// elision of its own, so a long line would run off both edges of the panel.
/// The *end* is the part that has to stay: it is where the caret is and where
/// the argument being typed is. `<` rather than an ellipsis because the font
/// atlas is printable ASCII only — see `font.rs`.
pub fn tail(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }

    let mut out = String::from("<");
    out.extend(text.chars().skip(count - max_chars));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reason the prompt appends instead of replacing. `raven-install`
    /// puts `resume=UUID=…` in every entry it writes, and the initramfs's
    /// parser is last-wins, so `noresume` has to land to the right of it.
    #[test]
    fn typed_arguments_land_after_the_ones_the_installer_wrote() {
        let base = "root=UUID=abc rw resume=UUID=def quiet";
        let combined = combine(base, "noresume");

        assert_eq!(combined, "root=UUID=abc rw resume=UUID=def quiet noresume");
        assert!(
            combined.rfind("noresume").unwrap() > combined.rfind("resume=UUID=def").unwrap(),
            "noresume must be parsed after resume= or the broken image is picked up again"
        );
    }

    #[test]
    fn an_untouched_prompt_boots_the_entry_unchanged() {
        let base = "root=UUID=abc rw quiet";
        assert_eq!(combine(base, ""), base);
        assert_eq!(combine(base, "   "), base);
    }

    #[test]
    fn a_base_with_nothing_in_it_does_not_produce_a_leading_space() {
        assert_eq!(combine("", "raven.live"), "raven.live");
        assert_eq!(combine("  ", "raven.live"), "raven.live");
    }

    #[test]
    fn typing_builds_the_string_and_backspace_takes_it_apart() {
        let mut editor = Editor::for_base("root=UUID=abc rw");
        assert!(editor.is_empty());

        for c in "rootdelay=30".chars() {
            assert!(editor.insert(c));
        }
        assert_eq!(editor.text(), "rootdelay=30");
        assert!(!editor.is_empty());

        assert!(editor.backspace());
        assert!(editor.backspace());
        assert_eq!(editor.text(), "rootdelay=");

        while editor.backspace() {}
        assert_eq!(editor.text(), "");
        assert!(editor.is_empty());
        // A backspace at an empty prompt must be a no-op rather than an
        // underflow, because it is what a hesitant operator presses first.
        assert!(!editor.backspace());
    }

    /// `linux.rs` widens the line byte by byte, so anything outside printable
    /// ASCII would reach the kernel as garbage. Better to refuse the key.
    #[test]
    fn only_printable_ascii_is_accepted() {
        let mut editor = Editor::for_base("");

        assert!(!editor.insert('\r'));
        assert!(!editor.insert('\n'));
        assert!(!editor.insert('\t'));
        assert!(!editor.insert('\u{8}'));
        assert!(!editor.insert('é'));
        assert!(!editor.insert('\u{7f}'));
        assert_eq!(editor.text(), "");

        assert!(editor.insert(' '));
        assert!(editor.insert('~'));
        assert!(editor.insert('='));
        assert_eq!(editor.text(), " ~=");
    }

    /// The kernel truncates past COMMAND_LINE_SIZE without a word, and what it
    /// drops is the tail — which is precisely what was just typed. Refusing the
    /// keystroke is the only version of this the operator can see happening.
    #[test]
    fn the_prompt_stops_before_the_kernel_would_truncate() {
        let base = "x".repeat(MAX_CMDLINE - 11);
        let mut editor = Editor::for_base(&base);

        // MAX_CMDLINE less the base and the space that joins them.
        for _ in 0..10 {
            assert!(editor.insert('a'));
        }
        assert!(!editor.insert('a'));
        assert_eq!(editor.text().len(), 10);
        assert_eq!(combine(&base, editor.text()).len(), MAX_CMDLINE);
    }

    #[test]
    fn a_base_that_already_fills_the_line_accepts_nothing() {
        let base = "x".repeat(MAX_CMDLINE);
        let mut editor = Editor::for_base(&base);
        assert!(!editor.insert('a'));
    }

    #[test]
    fn the_prompt_shows_the_end_of_what_was_typed() {
        assert_eq!(tail("noresume", 16), "noresume");
        assert_eq!(tail("0123456789", 10), "0123456789");
        assert_eq!(tail("0123456789", 4), "<6789");
    }
}
