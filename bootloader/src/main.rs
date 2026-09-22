//! RavenBoot - Custom UEFI Bootloader for RavenLinux
//!
//! A multi-boot capable bootloader that can coexist with other operating systems.
//! Supports booting Linux kernels directly via UEFI stub or traditional boot.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use core::fmt::Write;
use uefi::prelude::*;
use uefi::proto::console::text::{Color, Key, ScanCode};
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::media::file::{File, FileAttribute, FileMode};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::table::boot::{OpenProtocolAttributes, OpenProtocolParams, SearchType};
use uefi::table::runtime::{VariableAttributes, VariableVendor};
use uefi::CString16;

mod cmdline;
mod config;
mod font;
mod gfx;
mod linux;
mod mark;
mod menu;
mod screen;
mod theme;

use alloc::format;
use alloc::vec::Vec;

use config::{
    BootConfig, BootEntry, EntryType, CONFIG_PATHS, KNOWN_BOOTLOADERS, MAX_SUBMENU_DEPTH,
};
use linux::{boot_efi_stub, chainload_efi, KernelError};
use menu::{Row, RowKind, Status as MenuStatus, Tone, View};
use screen::Screen;

/// Menu navigation state
struct MenuNav<'a> {
    /// Stack of menu levels (indices into parent's children)
    stack: [usize; MAX_SUBMENU_DEPTH],
    /// Current depth (0 = root menu)
    depth: usize,
    /// Selected index at current level
    selected: usize,
    /// Reference to root config
    config: &'a BootConfig,
}

impl<'a> MenuNav<'a> {
    fn new(config: &'a BootConfig) -> Self {
        Self {
            stack: [0; MAX_SUBMENU_DEPTH],
            depth: 0,
            selected: config.default.min(config.entries.len().saturating_sub(1)),
            config,
        }
    }

    /// Get current menu entries
    fn current_entries(&self) -> &[BootEntry] {
        if self.depth == 0 {
            &self.config.entries
        } else {
            // Navigate to current submenu
            let mut entries = &self.config.entries;
            for i in 0..self.depth {
                let idx = self.stack[i];
                if idx < entries.len() && entries[idx].entry_type == EntryType::Submenu {
                    entries = &entries[idx].children;
                }
            }
            entries
        }
    }

    /// Get selected entry
    fn selected_entry(&self) -> Option<&BootEntry> {
        let entries = self.current_entries();
        entries.get(self.selected)
    }

    /// Enter a submenu
    fn enter_submenu(&mut self) {
        if self.depth < MAX_SUBMENU_DEPTH - 1 {
            self.stack[self.depth] = self.selected;
            self.depth += 1;
            self.selected = 0;
        }
    }

    /// Go back to parent menu
    fn go_back(&mut self) {
        if self.depth > 0 {
            self.depth -= 1;
            self.selected = self.stack[self.depth];
        }
    }

    /// Move selection up
    fn move_up(&mut self) {
        let len = self.current_entries().len();
        if self.selected > 0 {
            self.selected -= 1;
        } else if len > 0 {
            self.selected = len - 1;
        }
    }

    /// Move selection down
    fn move_down(&mut self) {
        let len = self.current_entries().len();
        if self.selected < len.saturating_sub(1) {
            self.selected += 1;
        } else {
            self.selected = 0;
        }
    }

    /// Get menu title for current level
    fn menu_title(&self) -> &str {
        if self.depth == 0 {
            "Select an operating system to boot"
        } else {
            // Get parent submenu name
            let mut entries = &self.config.entries;
            for i in 0..self.depth - 1 {
                let idx = self.stack[i];
                if idx < entries.len() {
                    entries = &entries[idx].children;
                }
            }
            let parent_idx = self.stack[self.depth - 1];
            if parent_idx < entries.len() {
                &entries[parent_idx].name
            } else {
                "Submenu"
            }
        }
    }
}

/// Bootloader version
const VERSION: &str = "0.1.0";

/// Main entry point
#[entry]
fn main(image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    // Initialize UEFI services (also sets up allocator)
    uefi::helpers::init(&mut system_table).unwrap();

    // Before anything reads a key or looks for a filesystem. See the function:
    // on a cold boot the keyboard is usually not connected to a driver yet,
    // and the menu is unusable until something asks the firmware to do it.
    connect_all_controllers(system_table.boot_services());

    // Now that the keyboard is behind ConIn, drop whatever is in its buffer.
    // A keystroke from the firmware's own boot-menu prompt would otherwise be
    // read here as the user's first answer to this one, and cancel the
    // countdown -- or select an entry -- before the menu has been drawn.
    // `false` skips the extended self-test, which on some firmware takes
    // seconds and proves nothing this needs.
    let _ = system_table.stdin().reset(false);

    // The text console is only ever the fallback now, but the config load below
    // reports on it, so it is cleared either way. `Screen::open` paints over
    // this the moment it succeeds.
    {
        let stdout = system_table.stdout();
        let _ = stdout.clear();
        let _ = stdout.set_color(Color::LightGray, Color::Black);
    }

    // Load configuration
    let config = {
        let boot_services = system_table.boot_services();
        match load_config(boot_services, image_handle) {
            Ok(cfg) => cfg,
            Err(_) => {
                let stdout = system_table.stdout();
                let _ = writeln!(stdout, "Warning: Could not load config, using defaults");
                BootConfig::default()
            }
        }
    };

    // If no keyboard input protocol is available (common in fully headless QEMU setups),
    // auto-boot the default entry rather than waiting in the menu forever.
    if !has_text_input(system_table.boot_services()) {
        if let Some(entry) = config.entries.get(config.default).cloned() {
            {
                let stdout = system_table.stdout();
                let _ = stdout.set_color(Color::Yellow, Color::Black);
                let _ = writeln!(
                    stdout,
                    "No keyboard input detected; auto-booting default entry: {}",
                    entry.name
                );
                let _ = stdout.set_color(Color::LightGray, Color::Black);
                let _ = writeln!(
                    stdout,
                    "Tip: use the 'Serial Console' entry for -nographic debugging."
                );
            }

            let result = boot_entry(system_table.boot_services(), image_handle, &entry);

            {
                let stdout = system_table.stdout();
                let _ = stdout.set_color(Color::Red, Color::Black);
                let _ = writeln!(stdout, "\nBoot failed: {:?}", result);
                let _ = writeln!(stdout, "Press any key to reboot...");
            }
            wait_for_key(&mut system_table);
            system_table.runtime_services().reset(
                uefi::table::runtime::ResetType::COLD,
                Status::SUCCESS,
                None,
            );
        }
    }

    // The panel, if the firmware has one to give. `None` means the text menu
    // below -- see `Screen::open`, which is the only place that decides.
    let mut screen = Screen::open(system_table.boot_services());
    let version = format!("RavenBoot {VERSION}");

    // A boot that failed, or a UEFI shell that was not there. Shown under the
    // card until the next keypress, in place of the countdown.
    let mut message: Option<String> = None;

    // Main menu loop with submenu support
    let mut nav = MenuNav::new(&config);

    loop {
        // Display boot menu and handle navigation
        let action = match &mut screen {
            Some(screen) => {
                display_menu_graphical(&mut system_table, &mut nav, screen, &version, &mut message)
            }
            None => display_menu_with_nav(&mut system_table, &mut nav),
        };

        match action {
            MenuAction::Boot(entry) => {
                // Leave the panel holding the backdrop before handing control
                // over. From here until `raven-greeter`'s first frame nothing
                // else draws, so this colour is the whole of the boot -- see
                // `Screen::hand_off`.
                if let Some(screen) = &mut screen {
                    screen.hand_off(system_table.boot_services());
                } else {
                    let stdout = system_table.stdout();
                    let _ = stdout.set_color(Color::White, Color::Black);
                    let _ = writeln!(stdout, "\nBooting: {}", entry.name);
                }

                // Boot the selected entry
                let result = {
                    let boot_services = system_table.boot_services();
                    boot_entry(boot_services, image_handle, &entry)
                };

                // If we get here, boot failed. The backdrop is still on the
                // panel, so the menu can simply be drawn over it again.
                // The error, not the `Result` wrapping it: `{result:?}` put a
                // literal "Err(...)" on the screen.
                let failure = match &result {
                    Err(error) => format!("Boot failed: {error:?}"),
                    // `boot_entry` does not return on a successful boot, so an
                    // `Ok` here means the image loaded, ran, and handed control
                    // back -- which for a kernel means it declined to start.
                    Ok(()) => format!("{} returned without booting", entry.name),
                };
                if screen.is_some() {
                    message = Some(failure);
                } else {
                    let stdout = system_table.stdout();
                    let _ = stdout.set_color(Color::Red, Color::Black);
                    let _ = writeln!(stdout, "\n{failure}");
                    let _ = writeln!(stdout, "Press any key to return to menu...");
                    wait_for_key(&mut system_table);
                }
            }
            MenuAction::Reboot => {
                let runtime_services = system_table.runtime_services();
                runtime_services.reset(
                    uefi::table::runtime::ResetType::COLD,
                    Status::SUCCESS,
                    None,
                );
            }
            MenuAction::Shutdown => {
                let runtime_services = system_table.runtime_services();
                runtime_services.reset(
                    uefi::table::runtime::ResetType::SHUTDOWN,
                    Status::SUCCESS,
                    None,
                );
            }
            MenuAction::UefiShell => {
                // The shell draws on the text console, so the panel is handed
                // over the same way a kernel would have it -- otherwise the
                // shell's first line lands on top of the menu.
                if let Some(screen) = &mut screen {
                    screen.hand_off(system_table.boot_services());
                }

                // Try to launch UEFI shell
                let result = {
                    let boot_services = system_table.boot_services();
                    launch_uefi_shell(boot_services, image_handle)
                };
                if result.is_err() {
                    if screen.is_some() {
                        message = Some(String::from("UEFI Shell not found"));
                    } else {
                        let stdout = system_table.stdout();
                        let _ = stdout.set_color(Color::Red, Color::Black);
                        let _ = writeln!(stdout, "\nUEFI Shell not found.");
                        let _ = writeln!(stdout, "Press any key to return to menu...");
                        wait_for_key(&mut system_table);
                    }
                }
            }
            MenuAction::FirmwareSetup => {
                let attrs = VariableAttributes::NON_VOLATILE
                    | VariableAttributes::BOOTSERVICE_ACCESS
                    | VariableAttributes::RUNTIME_ACCESS;
                let request = 1u64.to_le_bytes();
                let result = system_table.runtime_services().set_variable(
                    cstr16!("OsIndications"),
                    &VariableVendor::GLOBAL_VARIABLE,
                    attrs,
                    &request,
                );
                if result.is_ok() {
                    system_table.runtime_services().reset(
                        uefi::table::runtime::ResetType::COLD,
                        Status::SUCCESS,
                        None,
                    );
                }

                if screen.is_some() {
                    message = Some(String::from(
                        "This firmware cannot reboot into its settings; use its setup key",
                    ));
                } else {
                    {
                        let stdout = system_table.stdout();
                        let _ = stdout.set_color(Color::Red, Color::Black);
                        let _ = writeln!(
                            stdout,
                            "\nThis firmware does not support rebooting into its settings."
                        );
                        let _ =
                            writeln!(stdout, "Use the machine's setup key during reboot instead.");
                        let _ = writeln!(stdout, "Press any key to return to the menu...");
                    }
                    wait_for_key(&mut system_table);
                }
            }
            MenuAction::Continue => {
                // Navigation action, continue loop
            }
        }
    }
}

fn has_text_input(boot_services: &BootServices) -> bool {
    boot_services
        .get_handle_for_protocol::<uefi::proto::console::text::Input>()
        .is_ok()
}

/// Bind a driver to every device the firmware has enumerated.
///
/// This is the fix for a menu that draws, counts down, and ignores the
/// keyboard -- until you reboot, after which the same menu takes arrow keys
/// perfectly well.
///
/// UEFI does not have to have connected anything. The firmware is required to
/// connect the console and the device it is booting from; everything else is
/// left for the loader to ask for, and every fast-boot implementation takes
/// that permission. A USB keyboard on a cold start is typically *enumerated*
/// -- the host controller saw it -- and not *connected*: no driver is bound to
/// it, so UsbKbDxe never produces a SimpleTextInput for it, so the ConSplitter
/// has nothing to aggregate and ConIn returns NOT_READY forever. The second
/// boot works because the firmware kept the device connected across a warm
/// reset, which is exactly the asymmetry that makes this look like a bug in
/// the menu rather than in what ran before it.
///
/// So do what a loader is supposed to do: walk every handle and call
/// ConnectController on it, recursively, which drives the driver-binding
/// protocol over the whole device tree and gets UsbKbDxe onto the keyboard.
/// GRUB has carried this as `grub_efi_connect_all` for over a decade for this
/// same reason; it is not a workaround for one vendor's firmware.
///
/// Failures are ignored on purpose and there is nothing useful to report. A
/// handle with no driver willing to bind to it -- most of them -- returns
/// NOT_FOUND, and that is the ordinary case rather than an error; logging it
/// would print a screenful of noise about the devices that are working.
///
/// Cost is a scan of the handle database and one call per handle, which is
/// tens of milliseconds on a machine with a hundred handles. It runs once,
/// before the menu.
fn connect_all_controllers(boot_services: &BootServices) {
    let Ok(handles) = boot_services.locate_handle_buffer(SearchType::AllHandles) else {
        return;
    };

    for handle in handles.iter() {
        let _ = boot_services.connect_controller(*handle, None, None, true);
    }
}

/// Result of menu interaction
enum MenuAction {
    Boot(BootEntry),
    Reboot,
    Shutdown,
    UefiShell,
    FirmwareSetup,
    Continue,
}

/// Try to launch UEFI shell from common locations
fn launch_uefi_shell(
    boot_services: &BootServices,
    image_handle: Handle,
) -> Result<(), KernelError> {
    // Common UEFI shell locations
    let shell_paths = [
        "\\EFI\\BOOT\\Shell.efi",
        "\\EFI\\Shell.efi",
        "\\Shell.efi",
        "\\EFI\\tools\\Shell.efi",
        "\\shellx64.efi",
    ];

    for path in shell_paths {
        if let Ok(()) = chainload_efi(boot_services, image_handle, path, None) {
            return Ok(());
        }
    }

    Err(KernelError::EfiAppNotFound)
}

fn print_banner(stdout: &mut uefi::proto::console::text::Output) {
    let _ = stdout.set_color(Color::LightCyan, Color::Black);
    let _ = writeln!(stdout, "");
    let _ = stdout.set_color(Color::White, Color::Black);
    let _ = writeln!(stdout, r"    ██████╗  █████╗ ██╗   ██╗███████╗███╗   ██╗");
    let _ = writeln!(stdout, r"    ██╔══██╗██╔══██╗██║   ██║██╔════╝████╗  ██║");
    let _ = writeln!(stdout, r"    ██████╔╝███████║██║   ██║█████╗  ██╔██╗ ██║");
    let _ = writeln!(stdout, r"    ██╔══██╗██╔══██║╚██╗ ██╔╝██╔══╝  ██║╚██╗██║");
    let _ = writeln!(stdout, r"    ██║  ██║██║  ██║ ╚████╔╝ ███████╗██║ ╚████║");
    let _ = writeln!(stdout, r"    ╚═╝  ╚═╝╚═╝  ╚═╝  ╚═══╝  ╚══════╝╚═╝  ╚═══╝");
    let _ = stdout.set_color(Color::Cyan, Color::Black);
    let _ = writeln!(
        stdout,
        r"    ╔══════════════════════════════════════════════════════╗"
    );
    let _ = writeln!(
        stdout,
        r"    ║              UEFI MENU SELECT v{}                 ║",
        VERSION
    );
    let _ = writeln!(
        stdout,
        r"    ╚══════════════════════════════════════════════════════╝"
    );
    let _ = writeln!(stdout, "");
}

/// The graphical menu: draw a frame, wait for a key or a second, repeat.
///
/// Structurally the same loop as [`display_menu_with_nav`] below, and
/// deliberately so — the two share `handle_nav_key` and
/// `handle_entry_selection`, so a change to what a key does cannot apply to
/// only one of them. What differs is the drawing and, because a frame here is
/// a few megabytes rather than a few hundred bytes of console writes, that the
/// frame is composed once per visible change rather than once per poll.
fn display_menu_graphical(
    system_table: &mut SystemTable<Boot>,
    nav: &mut MenuNav,
    screen: &mut Screen,
    version: &str,
    message: &mut Option<String>,
) -> MenuAction {
    let mut timeout = nav.config.timeout;
    let scale = screen.scale();

    // A message means something just failed, and a countdown that carried on
    // ticking underneath it would boot the entry the user is reading about.
    let mut countdown_active = nav.depth == 0 && message.is_none();

    // `Some` while the argument prompt is open. See `cmdline.rs` for what it
    // is for; here it only changes what the frame says and where keys go.
    let mut edit: Option<cmdline::Editor> = None;

    loop {
        {
            let entries = nav.current_entries();
            let rows: Vec<Row> = entries
                .iter()
                .map(|entry| Row {
                    label: &entry.name,
                    kind: row_kind(entry.entry_type),
                })
                .collect();

            // Owns the formatted countdown for as long as `view` borrows it.
            let countdown;
            // And the same for the line of typed arguments.
            let typed;
            let status = if let Some(editor) = &edit {
                // An empty prompt spends its line on the four arguments this
                // exists for, because the operator most likely to need one is
                // the one least likely to remember its spelling -- and the
                // machine they would look it up on is the one in front of
                // them, which is not booting.
                //
                // Once there is something to show, the trailing underscore is
                // the caret. There is no blink and no cursor positioning: the
                // prompt only ever appends, so the insertion point is always
                // the end of the line and drawing it as a character costs
                // nothing.
                typed = if editor.is_empty() {
                    format!("> _    {RECOVERY_ARGUMENTS}")
                } else {
                    format!("> {}_", cmdline::tail(editor.text(), EDIT_VISIBLE_CHARS))
                };
                Some(MenuStatus {
                    text: &typed,
                    tone: Tone::Caution,
                })
            } else if let Some(text) = message.as_deref() {
                Some(MenuStatus {
                    text,
                    tone: Tone::Failure,
                })
            } else if countdown_active && timeout > 0 {
                let name = entries
                    .get(nav.selected)
                    .map_or("the default entry", |entry| entry.name.as_str());
                countdown = format!("Booting {name} in {timeout}s");
                Some(MenuStatus {
                    text: &countdown,
                    tone: Tone::Caution,
                })
            } else {
                None
            };

            // The prompt line carries the only advertisement of the `e` key.
            // The footer would be the conventional place, but its hints are
            // built from `View` alone and `View` cannot tell a Linux entry from
            // a Reboot one -- offering to edit the arguments of "Shut Down"
            // would be worse than saying nothing. An undiscoverable recovery
            // key is very nearly as useless as no key at all, so it is said
            // here, and only while the selection is something it applies to.
            let prompt;
            let prompt_text = if edit.is_some() {
                prompt = String::from(
                    "Arguments to add for this boot only -- [Enter] Boot  [Esc] Cancel",
                );
                prompt.as_str()
            } else if nav
                .selected_entry()
                .is_some_and(|entry| entry_takes_cmdline(entry.entry_type))
            {
                prompt = format!("{}    [e] Kernel arguments", nav.menu_title());
                prompt.as_str()
            } else {
                nav.menu_title()
            };

            let view = View {
                rows: &rows,
                selected: nav.selected,
                prompt: prompt_text,
                status,
                version,
                can_go_back: nav.depth > 0,
            };

            menu::draw(&mut screen.canvas, &view, scale);
        }
        let _ = screen.present(system_table.boot_services());

        // Wait for input or timeout
        if countdown_active && timeout > 0 {
            match wait_for_key_timeout(system_table, 1_000_000) {
                Some(key) => {
                    countdown_active = false;
                    *message = None;
                    if let Key::Printable(c) = key {
                        if c == uefi::Char16::try_from('\r').unwrap() {
                            if let Some(entry) = nav.selected_entry() {
                                return handle_entry_selection(nav, entry.clone());
                            }
                        }
                    }
                    // `e` during the countdown opens the prompt rather than
                    // merely stopping the clock. Requiring it to be pressed
                    // twice would be a trap: the countdown is the five seconds
                    // in which somebody watching a machine fail to boot is
                    // most likely to reach for it.
                    if edit_requested(key) {
                        edit = open_editor(nav);
                        continue;
                    }
                    handle_nav_key(key, nav);
                }
                None => {
                    timeout -= 1;
                    if timeout == 0 {
                        if let Some(entry) = nav.selected_entry() {
                            return handle_entry_selection(nav, entry.clone());
                        }
                    }
                }
            }
        } else {
            let key = wait_for_key(system_table);

            // Any key dismisses the message. It is a report on something the
            // user just asked for, not a state they have to clear.
            *message = None;

            // The prompt takes the whole keyboard while it is open, which is
            // why this comes first and ends in `continue`: inside it Esc
            // cancels the edit rather than leaving the submenu, Backspace
            // deletes a character rather than going back, and the arrow keys
            // do nothing -- the selection must not move out from under a line
            // that was typed for the entry on screen.
            if edit.is_some() {
                match key {
                    Key::Printable(c) if c == uefi::Char16::try_from('\r').unwrap() => {
                        if let Some(entry) = nav.selected_entry() {
                            let mut entry = entry.clone();
                            // The clone is the whole of "for one boot only":
                            // `boot.cfg` on the ESP is never reopened, so a
                            // failed boot comes back to a menu that has
                            // forgotten this.
                            entry.cmdline = cmdline::combine(
                                &entry.cmdline,
                                edit.as_ref().map_or("", cmdline::Editor::text),
                            );
                            return MenuAction::Boot(entry);
                        }
                        edit = None;
                    }
                    Key::Special(ScanCode::ESCAPE) => edit = None,
                    Key::Printable(c) if c == uefi::Char16::try_from('\x08').unwrap() => {
                        if let Some(editor) = edit.as_mut() {
                            editor.backspace();
                        }
                    }
                    Key::Printable(c) => {
                        if let Some(editor) = edit.as_mut() {
                            editor.insert(char::from(c));
                        }
                    }
                    _ => {}
                }
                continue;
            }

            match key {
                Key::Printable(c) if c == uefi::Char16::try_from('\r').unwrap() => {
                    if let Some(entry) = nav.selected_entry() {
                        return handle_entry_selection(nav, entry.clone());
                    }
                }
                Key::Special(ScanCode::ESCAPE) => {
                    if nav.depth > 0 {
                        nav.go_back();
                    }
                }
                Key::Printable(c) if c == uefi::Char16::try_from('\x08').unwrap() => {
                    if nav.depth > 0 {
                        nav.go_back();
                    }
                }
                key if edit_requested(key) => edit = open_editor(nav),
                _ => handle_nav_key(key, nav),
            }
        }
    }
}

/// How a boot entry is drawn, which is all the menu needs to know about it.
fn row_kind(entry_type: EntryType) -> RowKind {
    match entry_type {
        EntryType::Submenu => RowKind::Submenu,
        EntryType::Back => RowKind::Back,
        _ => RowKind::Action,
    }
}

fn display_menu_with_nav(system_table: &mut SystemTable<Boot>, nav: &mut MenuNav) -> MenuAction {
    let mut timeout = nav.config.timeout;
    let mut countdown_active = nav.depth == 0; // Only countdown on root menu

    // The argument prompt, open or not. This path is the one a headless or
    // pre-GOP machine gets, which is disproportionately the one being rescued,
    // so it has the same key and the same rules as the graphical menu -- the
    // two differ in how they draw and in nothing else.
    let mut edit: Option<cmdline::Editor> = None;

    loop {
        let entries = nav.current_entries();

        // Worked out before `stdout` is borrowed, because that borrow takes
        // the whole system table and `nav` cannot be consulted through it.
        let edit_takes_a_prompt = nav
            .selected_entry()
            .is_some_and(|entry| entry_takes_cmdline(entry.entry_type));

        // Clear and redraw menu (scoped borrow of stdout)
        {
            let stdout = system_table.stdout();
            let _ = stdout.clear();
            print_banner(stdout);

            let _ = stdout.set_color(Color::LightGray, Color::Black);
            let _ = writeln!(stdout, "    {}", nav.menu_title());
            let _ = writeln!(stdout, "");

            // Menu width for full-line highlight
            const MENU_WIDTH: usize = 56;

            // Draw entries with full-line highlight
            for (i, entry) in entries.iter().enumerate() {
                if i == nav.selected {
                    // Selected entry - full line highlighted
                    let _ = stdout.set_color(Color::White, Color::Cyan);
                    let _ = write!(stdout, "    ");
                    let _ = write!(stdout, " >> ");
                    let _ = write!(stdout, "{}", entry.name);
                    // Pad to fill the line
                    let padding = MENU_WIDTH.saturating_sub(8 + entry.name.len());
                    for _ in 0..padding {
                        let _ = write!(stdout, " ");
                    }
                    let _ = writeln!(stdout, "");
                } else {
                    // Unselected entry
                    let _ = stdout.set_color(Color::LightGray, Color::Black);
                    let _ = writeln!(stdout, "        {}", entry.name);
                }
            }

            let _ = stdout.set_color(Color::DarkGray, Color::Black);
            let _ = writeln!(stdout, "");
            let _ = writeln!(
                stdout,
                "    ──────────────────────────────────────────────────────"
            );

            if let Some(editor) = &edit {
                let _ = stdout.set_color(Color::Yellow, Color::Black);
                let _ = writeln!(
                    stdout,
                    "    Arguments to add for this boot only   [Enter] Boot   [Esc] Cancel"
                );
                // No elision here: the text console wraps, so a long line
                // costs a row rather than running off the panel.
                if editor.is_empty() {
                    let _ = writeln!(stdout, "    try:  {RECOVERY_ARGUMENTS}");
                }
                let _ = writeln!(stdout, "    > {}_", editor.text());
            } else if countdown_active && timeout > 0 {
                let _ = stdout.set_color(Color::Yellow, Color::Black);
                let _ = writeln!(
                    stdout,
                    "    Auto-boot in {} seconds...  Press any key to stop",
                    timeout
                );
            } else {
                let _ = stdout.set_color(Color::LightGray, Color::Black);
                let hint_edit = if edit_takes_a_prompt { "   [e] Kernel args" } else { "" };
                if nav.depth > 0 {
                    let _ = writeln!(
                        stdout,
                        "    [↑/↓] Select   [Enter] Select   [Esc/Backspace] Back{hint_edit}"
                    );
                } else {
                    let _ = writeln!(
                        stdout,
                        "    [↑/↓] Select   [Enter] Boot/Enter   [Esc] Exit{hint_edit}"
                    );
                }
            }
        }

        // Wait for input or timeout
        if countdown_active && timeout > 0 {
            let key_result = wait_for_key_timeout(system_table, 1_000_000);
            match key_result {
                Some(key) => {
                    countdown_active = false;
                    // Handle enter key during countdown
                    if let Key::Printable(c) = key {
                        if c == uefi::Char16::try_from('\r').unwrap() {
                            if let Some(entry) = nav.selected_entry() {
                                return handle_entry_selection(nav, entry.clone());
                            }
                        }
                    }
                    if edit_requested(key) {
                        edit = open_editor(nav);
                        continue;
                    }
                    handle_nav_key(key, nav);
                }
                None => {
                    timeout -= 1;
                    if timeout == 0 {
                        // Auto-boot default entry
                        if let Some(entry) = nav.selected_entry() {
                            return handle_entry_selection(nav, entry.clone());
                        }
                    }
                }
            }
        } else {
            let key = wait_for_key(system_table);

            // See the graphical menu: while the prompt is open it owns every
            // key, so that Esc and Backspace mean what they mean inside a text
            // field rather than what they mean in the menu behind it.
            if edit.is_some() {
                match key {
                    Key::Printable(c) if c == uefi::Char16::try_from('\r').unwrap() => {
                        if let Some(entry) = nav.selected_entry() {
                            let mut entry = entry.clone();
                            entry.cmdline = cmdline::combine(
                                &entry.cmdline,
                                edit.as_ref().map_or("", cmdline::Editor::text),
                            );
                            return MenuAction::Boot(entry);
                        }
                        edit = None;
                    }
                    Key::Special(ScanCode::ESCAPE) => edit = None,
                    Key::Printable(c) if c == uefi::Char16::try_from('\x08').unwrap() => {
                        if let Some(editor) = edit.as_mut() {
                            editor.backspace();
                        }
                    }
                    Key::Printable(c) => {
                        if let Some(editor) = edit.as_mut() {
                            editor.insert(char::from(c));
                        }
                    }
                    _ => {}
                }
                continue;
            }

            match key {
                Key::Printable(c) if c == uefi::Char16::try_from('\r').unwrap() => {
                    if let Some(entry) = nav.selected_entry() {
                        return handle_entry_selection(nav, entry.clone());
                    }
                }
                Key::Special(ScanCode::ESCAPE) => {
                    if nav.depth > 0 {
                        nav.go_back();
                    }
                }
                Key::Printable(c) if c == uefi::Char16::try_from('\x08').unwrap() => {
                    // Backspace
                    if nav.depth > 0 {
                        nav.go_back();
                    }
                }
                key if edit_requested(key) => {
                    edit = open_editor(nav);
                }
                _ => {
                    handle_nav_key(key, nav);
                }
            }
        }
    }
}

fn handle_entry_selection(nav: &mut MenuNav, entry: BootEntry) -> MenuAction {
    match entry.entry_type {
        EntryType::Submenu => {
            nav.enter_submenu();
            MenuAction::Continue
        }
        EntryType::Back => {
            nav.go_back();
            MenuAction::Continue
        }
        EntryType::Reboot => MenuAction::Reboot,
        EntryType::Shutdown => MenuAction::Shutdown,
        EntryType::UefiShell => MenuAction::UefiShell,
        EntryType::FirmwareSetup => MenuAction::FirmwareSetup,
        _ => MenuAction::Boot(entry),
    }
}

fn handle_nav_key(key: Key, nav: &mut MenuNav) {
    match key {
        Key::Special(ScanCode::UP) => nav.move_up(),
        Key::Special(ScanCode::DOWN) => nav.move_down(),
        _ => {}
    }
}

/// What an empty prompt suggests.
///
/// These are exactly the four arguments `scripts/build-initramfs.sh` parses as
/// recovery options, in the order somebody is likely to want them: `noresume`
/// abandons a hibernation image that is killing the restore, `rootdelay=`
/// gives a slow disk longer to appear, `crypttries=` changes how many
/// passphrase attempts come before the rescue shell, and `raven.live` boots
/// the live image from a disk that has a root= of its own. If that list
/// changes there, it has to change here -- a prompt that suggests an argument
/// nothing reads is worse than a prompt that suggests nothing.
const RECOVERY_ARGUMENTS: &str = "noresume   rootdelay=30   crypttries=10   raven.live";

/// How many characters of the typed line the status row shows.
///
/// The status is one centred, unwrapped row drawn in the small face, and the
/// narrowest panel firmware commonly reports is 1024 logical pixels wide. At
/// that width this many characters plus the `> ` and the caret still sit
/// inside the margins, and the recovery arguments it exists for -- `noresume`,
/// `rootdelay=30`, `crypttries=10`, `raven.live` -- are all far shorter than
/// it. A longer line is not refused, only scrolled: see `cmdline::tail`.
const EDIT_VISIBLE_CHARS: usize = 72;

/// Whether this key asks for the argument prompt.
///
/// `e` is GRUB's key for the same thing. Somebody who has ever edited a boot
/// entry on another distribution will try it here first, and somebody who has
/// not is no worse off. The case is ignored because Caps Lock on a machine
/// that will not boot is not a thing anyone should have to notice.
fn edit_requested(key: Key) -> bool {
    matches!(key, Key::Printable(c) if char::from(c).eq_ignore_ascii_case(&'e'))
}

/// The prompt, for the current selection, or `None` if it would do nothing.
///
/// Returning `None` rather than opening an empty prompt is the honest
/// behaviour: a chainloaded Windows entry is handed to the firmware's
/// `LoadImage` with no load options at all, and a Reboot row is not an image,
/// so a prompt on either would take a line of typing and then discard it.
fn open_editor(nav: &MenuNav) -> Option<cmdline::Editor> {
    let entry = nav.selected_entry()?;
    entry_takes_cmdline(entry.entry_type).then(|| cmdline::Editor::for_base(&entry.cmdline))
}

/// Whether an entry of this kind is booted with a command line that the kernel
/// will read.
///
/// `boot_entry` is the authority: `LinuxEfi` passes `entry.cmdline` through to
/// `boot_efi_stub`, and `LinuxLegacy` is where a command line would go if that
/// path were implemented. Every other kind either ignores the field or is not
/// a boot at all.
fn entry_takes_cmdline(entry_type: EntryType) -> bool {
    matches!(entry_type, EntryType::LinuxEfi | EntryType::LinuxLegacy)
}

fn wait_for_key(system_table: &mut SystemTable<Boot>) -> Key {
    loop {
        if let Some(key) = try_get_key(system_table) {
            return key;
        }
        system_table.boot_services().stall(10_000); // 10ms
    }
}

fn wait_for_key_timeout(system_table: &mut SystemTable<Boot>, timeout_us: u64) -> Option<Key> {
    let iterations = timeout_us / 10_000;
    for _ in 0..iterations {
        if let Some(key) = try_get_key(system_table) {
            return Some(key);
        }
        system_table.boot_services().stall(10_000);
    }
    None
}

/// One keystroke from the console, if there is one waiting.
///
/// This reads ConIn -- `SystemTable::stdin` -- and not a handle located by
/// protocol, which is what it used to do:
///
/// ```ignore
/// let handle = boot_services.get_handle_for_protocol::<Input>()?;
/// let mut input = boot_services.open_protocol_exclusive::<Input>(handle)?;
/// ```
///
/// Two things are wrong with that, and both of them are why the menu ignored
/// the keyboard until the machine had been rebooted once.
///
/// `get_handle_for_protocol` returns the *first* handle carrying the protocol,
/// and on a machine with a keyboard there is more than one: the ConSplitter's
/// virtual handle, which aggregates every input device the firmware knows
/// about, and one handle per physical keyboard behind it. Which of those comes
/// first is whatever order the firmware happens to have installed them in. Pick
/// a physical handle and only that one keyboard works; pick the built-in
/// PS/2-emulation handle on a laptop whose keyboard is USB-attached and none of
/// them do.
///
/// `open_protocol_exclusive` then makes it worse rather than better. EXCLUSIVE
/// tells the firmware to disconnect any driver holding the protocol BY_DRIVER
/// first -- and on a physical keyboard handle the driver holding it is the
/// ConSplitter. So the poll that was meant to read a key could instead detach
/// that keyboard from the console it feeds, at a hundred opens and closes a
/// second, for as long as the menu was on screen.
///
/// ConIn is the aggregate and it is already open. Reading it needs no handle
/// search and no protocol open at all.
fn try_get_key(system_table: &mut SystemTable<Boot>) -> Option<Key> {
    match system_table.stdin().read_key() {
        Ok(key) => key,
        Err(_) => None,
    }
}

fn load_config(boot_services: &BootServices, image_handle: Handle) -> Result<BootConfig, ()> {
    // Get device handle from our loaded image
    let loaded_image = boot_services
        .open_protocol_exclusive::<LoadedImage>(image_handle)
        .map_err(|_| ())?;

    let device_handle = loaded_image.device().ok_or(())?;

    // Open filesystem
    let mut fs = boot_services
        .open_protocol_exclusive::<SimpleFileSystem>(device_handle)
        .map_err(|_| ())?;

    let mut root = fs.open_volume().map_err(|_| ())?;

    // Try to load config from known paths, falling back to defaults.
    let mut config: BootConfig = BootConfig::default();
    for config_path in CONFIG_PATHS {
        if let Ok(config_data) = read_file_from_root(&mut root, config_path) {
            if let Ok(parsed) = BootConfig::parse(&config_data) {
                config = parsed;
                break;
            }
        }
    }

    // Snapshots, if this machine takes any. Read while the ESP is still open,
    // and before detect_other_os, because both want top-level slots and
    // MAX_ENTRIES is not generous: a way back into yesterday's system belongs
    // in the menu ahead of a Windows that the firmware's own boot menu can
    // still reach.
    for snapshot_path in config::SNAPSHOT_CONFIG_PATHS {
        let Ok(snapshot_data) = read_file_from_root(&mut root, snapshot_path) else {
            continue;
        };
        if let Some(menu) = config::parse_snapshot_menu(&snapshot_data) {
            insert_snapshot_menu(&mut config, menu);
        }
        // The first of these paths that exists is the answer, even if it held
        // nothing usable. Falling through to the next one after reading an
        // empty list would let a stale `snapshots.cfg` reappear the moment
        // `raven-snapshot` emptied `snaps.cfg`, which is exactly when the menu
        // must stop offering snapshots that are no longer on the disk.
        break;
    }

    // Our own volume is scanned by the sweep below like any other, so the
    // handle is what is needed here, not the open filesystem.
    drop(root);
    drop(fs);
    drop(loaded_image);

    // Try to detect other operating systems and add them to the config
    detect_other_os(boot_services, device_handle, &mut config);

    Ok(config)
}

/// Put the "Snapshots >" submenu in the menu, after the last Linux entry.
///
/// Position is a decision and not tidiness. Appending would put it below
/// Shutdown, at the bottom of a list whose bottom is where the machine-control
/// entries live; a snapshot is another way to boot this machine's Linux, so it
/// belongs with the other ways to boot this machine's Linux. `raven-install`
/// writes those first and the firmware/reboot/shutdown entries last, so "after
/// the last Linux entry" puts it exactly on that seam without this code having
/// to know anything about what the entries are called.
///
/// THE PART THAT MUST NOT BE GOT WRONG is the two lines at the bottom.
/// `config.default` is an *index*, chosen by `raven-install` -- 0 for a console
/// profile and 1 for a desktop one -- and inserting an entry above it moves the
/// entry it points at. Forgetting that would mean a desktop machine that took
/// its first snapshot then started booting to a text console instead, five
/// seconds later, with nothing anywhere saying why.
fn insert_snapshot_menu(config: &mut BootConfig, menu: BootEntry) {
    // One top-level slot is all this needs, but if there is not even one then
    // the menu is already at the cap and something would have to be dropped to
    // make room. Nothing here is worth dropping a boot entry for.
    if config.entries.len() >= config::MAX_ENTRIES {
        return;
    }

    let position = config
        .entries
        .iter()
        .rposition(|entry| {
            matches!(
                entry.entry_type,
                EntryType::LinuxEfi | EntryType::LinuxLegacy
            )
        })
        .map(|last| last + 1)
        .unwrap_or(config.entries.len());

    config.entries.insert(position, menu);

    if config.default >= position {
        config.default += 1;
    }
}

/// Try to read a file from the ESP root
fn read_file_from_root(
    root: &mut uefi::proto::media::file::Directory,
    path: &str,
) -> Result<alloc::vec::Vec<u8>, ()> {
    use alloc::vec::Vec;

    // Convert path to UCS-2
    let path_cstr = CString16::try_from(path).map_err(|_| ())?;

    // Try to open the file
    let file_handle = root
        .open(&path_cstr, FileMode::Read, FileAttribute::empty())
        .map_err(|_| ())?;

    let mut file = match file_handle.into_type().map_err(|_| ())? {
        uefi::proto::media::file::FileType::Regular(f) => f,
        _ => return Err(()),
    };

    // Get file size
    let mut info_buf = [0u8; 256];
    let info: &uefi::proto::media::file::FileInfo = file.get_info(&mut info_buf).map_err(|_| ())?;

    let file_size = info.file_size() as usize;

    // Read file
    let mut buffer = Vec::with_capacity(file_size);
    buffer.resize(file_size, 0);

    file.read(&mut buffer).map_err(|_| ())?;

    Ok(buffer)
}

/// Check if a file exists on the ESP
fn file_exists(root: &mut uefi::proto::media::file::Directory, path: &str) -> bool {
    let path_cstr = match CString16::try_from(path) {
        Ok(p) => p,
        Err(_) => return false,
    };

    if let Ok(handle) = root.open(&path_cstr, FileMode::Read, FileAttribute::empty()) {
        // Close the file handle by dropping it
        drop(handle);
        true
    } else {
        false
    }
}

/// Detect other operating systems, on every disk rather than only on ours.
///
/// This used to scan the volume RavenBoot was loaded from and nothing else,
/// which found a Windows sharing our EFI System Partition and missed the
/// commonest dual-boot layout there is: Windows on the machine's first disk,
/// RavenLinux on its second. That machine has two ESPs, ours holds no trace of
/// the other OS, and the menu offered no way to reach it -- the firmware's own
/// boot menu was the only route back to Windows.
///
/// `find_handles` returns every filesystem the firmware has a driver for,
/// which on a UEFI machine is every FAT partition on every disk it can see.
/// Each is opened read-only (`GetProtocol`, never `Exclusive`: these are other
/// people's volumes and disconnecting the driver holding one is not ours to
/// do) and checked for the loaders in `KNOWN_BOOTLOADERS`.
///
/// `own_device` is the handle our own volume is behind. Entries found there
/// keep `volume: None` so they boot exactly as they did before; everything
/// else carries the handle it was found on.
fn detect_other_os(boot_services: &BootServices, own_device: Handle, config: &mut BootConfig) {
    let Ok(handles) = boot_services.find_handles::<SimpleFileSystem>() else {
        return;
    };

    for handle in handles {
        if config.entries.len() >= config::MAX_ENTRIES {
            break;
        }

        // SAFETY: GetProtocol takes a reference to an already-installed
        // protocol without disturbing whatever driver owns it. The
        // ScopedProtocol closes it again at the end of the iteration.
        let params = OpenProtocolParams {
            handle,
            agent: boot_services.image_handle(),
            controller: None,
        };
        let Ok(mut fs) = (unsafe {
            boot_services.open_protocol::<SimpleFileSystem>(
                params,
                OpenProtocolAttributes::GetProtocol,
            )
        }) else {
            continue;
        };
        let Ok(mut root) = fs.open_volume() else {
            continue;
        };

        // One entry per operating system per volume, not one per file. An
        // Ubuntu ESP carries both shimx64.efi and grubx64.efi and the table
        // lists both, because either can be the one that is there; finding
        // both used to put "Ubuntu" in the menu twice.
        let mut found_here: Vec<&str> = Vec::new();

        for bootloader in KNOWN_BOOTLOADERS {
            if config.entries.len() >= config::MAX_ENTRIES {
                break;
            }
            if found_here.contains(&bootloader.name) {
                continue;
            }
            if !file_exists(&mut root, bootloader.path) {
                continue;
            }
            // A hand-written boot.cfg may already name this loader. Skipping
            // it stops the same Windows appearing in the menu twice -- but
            // only on our own volume, because the same path on another disk
            // is a different installation and both belong there.
            if handle == own_device
                && config
                    .entries
                    .iter()
                    .any(|e| e.volume.is_none() && e.kernel.eq_ignore_ascii_case(bootloader.path))
            {
                continue;
            }
            found_here.push(bootloader.name);

            config.entries.push(BootEntry {
                name: bootloader.name.into(),
                kernel: bootloader.path.into(),
                initrd: None,
                cmdline: String::new(),
                entry_type: bootloader.entry_type,
                // Ours stays None: an entry on our own volume is one boot.cfg
                // could have described, and it is booted the way it always was.
                volume: (handle != own_device).then_some(handle),
                children: Vec::new(),
            });
        }
    }
}

fn boot_entry(
    boot_services: &BootServices,
    image_handle: Handle,
    entry: &BootEntry,
) -> Result<(), KernelError> {
    match entry.entry_type {
        EntryType::LinuxEfi => {
            // Boot Linux kernel with EFI stub
            boot_efi_stub(
                boot_services,
                image_handle,
                entry.kernel.as_str(),
                entry.initrd.as_deref(),
                entry.cmdline.as_str(),
            )
        }
        EntryType::Windows | EntryType::Chainload | EntryType::EfiApp => {
            // Chainload another EFI application
            chainload_efi(
                boot_services,
                image_handle,
                entry.kernel.as_str(),
                entry.volume,
            )
        }
        EntryType::LinuxLegacy => {
            // Traditional Linux boot - not implemented
            Err(KernelError::NotImplemented)
        }
        // These entry types are handled by the menu system, not boot_entry
        EntryType::Submenu
        | EntryType::Back
        | EntryType::Reboot
        | EntryType::Shutdown
        | EntryType::UefiShell
        | EntryType::FirmwareSetup => Err(KernelError::NotImplemented),
    }
}
