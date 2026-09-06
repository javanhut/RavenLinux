//! The wizard pages: the same questions raven-install asks, in the same order.
//!
//! Each page writes straight into the shared Answers as it is edited, so there
//! is no "collect" step that can forget a field, and validation is whatever
//! answers::Answers::problems() says plus whatever is local to the page (a
//! password typed twice, for instance, which is not an answer -- it is a
//! check on one).

use std::rc::Rc;

use adw::prelude::*;
use crate::answers::Answers;
use crate::probe::{self, Disk, Probe};
use crate::{App, PageDef, Validator};

/// A window that only says why it cannot go on. Used for the two things that
/// are decided before the wizard exists: no route to root, and a machine the
/// installer refuses.
pub fn fatal(title: &str, detail: &str, p: Option<&Probe>) -> gtk::Widget {
    let status = adw::StatusPage::builder()
        .icon_name("dialog-error-symbolic")
        .title(title)
        .description(detail)
        .build();

    // The installer's reasons are prose with newlines in them; a StatusPage
    // description renders those, but the machine facts behind them are worth
    // showing too, and they belong in rows rather than in a paragraph.
    if let Some(p) = p {
        let group = adw::PreferencesGroup::builder().title("This machine").build();
        let mut rows: Vec<(&str, String)> = vec![
            ("Firmware", p.firmware.to_uppercase()),
            ("Secure Boot", p.secureboot.clone()),
        ];
        if !p.tools_missing.is_empty() {
            rows.push(("Missing tools", p.tools_missing.join(", ")));
        }
        if p.euid != 0 {
            rows.push(("Running as", format!("uid {}", p.euid)));
        }
        for (k, v) in rows {
            if v.is_empty() {
                continue;
            }
            group.add(&adw::ActionRow::builder().title(k).subtitle(v).build());
        }
        let clamp = adw::Clamp::builder().maximum_size(520).child(&group).build();
        status.set_child(Some(&clamp));
    }

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&status));
    toolbar.upcast()
}

/// A scrolling shell for a PreferencesPage, so long pages behave on a laptop
/// screen and short ones do not stretch.
fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(child)
        .build()
}

pub fn build_all(app: &Rc<App>) {
    welcome(app);
    disk(app);
    account(app);
    locale(app);
    profile(app);
    summary(app);
}

fn push(app: &Rc<App>, name: &'static str, title: &'static str, widget: &impl IsA<gtk::Widget>,
        validate: Validator) {
    app.stack.add_named(widget, Some(name));
    app.defs.borrow_mut().push(PageDef { name, title, validate });
}

// =============================================================================
// Welcome
// =============================================================================

fn welcome(app: &Rc<App>) {
    let p = &app.probe;

    let page = adw::PreferencesPage::new();

    let intro = adw::PreferencesGroup::builder()
        .title("Install RavenLinux")
        .description(
            "This installs RavenLinux onto a disk in this machine. The disk you \
             choose is erased completely -- installing alongside another \
             operating system is not supported yet.",
        )
        .build();
    page.add(&intro);

    let found = adw::PreferencesGroup::builder().title("What will be installed").build();
    let source_desc = match p.source_kind.as_str() {
        // The two cases locate_source distinguishes, in the words its own
        // comments use for them.
        "squashfs" => "The pristine system image on the boot media",
        _ => "This running system, copied onto the new disk",
    };
    found.add(
        &adw::ActionRow::builder()
            .title(source_desc)
            .subtitle(if p.source_size_mb > 0 {
                format!("about {:.1} GB to copy", p.source_size_mb as f64 / 1024.0)
            } else {
                "size unknown".to_string()
            })
            .build(),
    );
    found.add(
        &adw::ActionRow::builder()
            .title("Bootloader")
            .subtitle("RavenBoot, at \\EFI\\raven and the firmware's fallback path")
            .build(),
    );
    found.add(
        &adw::ActionRow::builder()
            .title("Firmware")
            .subtitle(format!(
                "{}, Secure Boot {}",
                p.firmware.to_uppercase(),
                p.secureboot
            ))
            .build(),
    );
    page.add(&found);

    let warnings = p.warnings();
    if !warnings.is_empty() {
        let group = adw::PreferencesGroup::builder()
            .title("Worth knowing before you start")
            .build();
        for w in warnings {
            let row = adw::ActionRow::builder().title(w).build();
            row.set_title_lines(0);
            let icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
            icon.add_css_class("warning");
            row.add_prefix(&icon);
            group.add(&row);
        }
        page.add(&group);
    }

    push(app, "welcome", "Welcome", &scrolled(&page), Box::new(|_, _| Vec::new()));
}

// =============================================================================
// Disk
// =============================================================================

fn disk(app: &Rc<App>) {
    let page = adw::PreferencesPage::new();
    let installable: Vec<Disk> = app.probe.installable_disks().into_iter().cloned().collect();

    let group = adw::PreferencesGroup::builder()
        .title("Target disk")
        .description(
            "Everything on the disk you pick is destroyed. The disk this machine \
             booted from is not offered.",
        )
        .build();

    if installable.is_empty() {
        // choose_disk's own message for the case, which is nearly always this.
        let row = adw::ActionRow::builder()
            .title("No disks found")
            .subtitle(
                "If this machine uses RAID or Intel RST, switch the SATA/NVMe \
                 controller to AHCI mode in the firmware.",
            )
            .build();
        row.set_subtitle_lines(0);
        group.add(&row);
    }

    let mut first: Option<gtk::CheckButton> = None;
    for d in &installable {
        let row = adw::ActionRow::builder()
            .title(&d.dev)
            .subtitle(d.subtitle())
            .build();

        let check = gtk::CheckButton::new();
        if let Some(f) = &first {
            check.set_group(Some(f));
        } else {
            first = Some(check.clone());
        }
        check.set_active(app.answers.borrow().disk == d.dev);
        row.add_prefix(&check);
        row.set_activatable_widget(Some(&check));

        if d.windows {
            // choose_disk prints this in red before it asks. The same fact, in
            // the place where the click happens.
            let icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
            icon.set_tooltip_text(Some(
                "This looks like a Windows installation. It will be erased.",
            ));
            icon.add_css_class("warning");
            row.add_suffix(&icon);
        }

        check.connect_toggled({
            let app = app.clone();
            let dev = d.dev.clone();
            move |c| {
                if c.is_active() {
                    app.answers.borrow_mut().disk = dev.clone();
                    app.revalidate();
                }
            }
        });
        group.add(&row);

        // What the disk currently holds, exactly as choose_disk lists it.
        // Vague warnings get clicked through; somebody's own partitions do not.
        if let Some(holds) = d.holds_summary() {
            let expander = adw::ExpanderRow::builder()
                .title("This disk currently holds")
                .build();
            let label = gtk::Label::builder()
                .label(holds)
                .xalign(0.0)
                .margin_top(6)
                .margin_bottom(12)
                .margin_start(18)
                .margin_end(12)
                .selectable(true)
                .build();
            label.add_css_class("monospace");
            label.add_css_class("dim-label");
            expander.add_row(&adw::ActionRow::builder().child(&label).build());
            group.add(&expander);
        }
    }
    page.add(&group);

    // ---- layout -------------------------------------------------------------
    let layout = adw::PreferencesGroup::builder().title("Layout").build();

    let fs_names: Vec<&str> = app.probe.filesystems.iter().map(String::as_str).collect();
    let fs_model = gtk::StringList::new(&fs_names);
    let fs_row = adw::ComboRow::builder()
        .title("Root filesystem")
        // Only the ones whose mkfs is on this image: offering btrfs where there
        // is no mkfs.btrfs offers an install that dies three steps after the
        // disk was erased.
        .subtitle("Only filesystems this image can create are listed")
        .model(&fs_model)
        .build();
    if let Some(i) = app.probe.filesystems.iter().position(|f| *f == app.answers.borrow().fs) {
        fs_row.set_selected(i as u32);
    }
    fs_row.connect_selected_notify({
        let app = app.clone();
        move |r| {
            if let Some(f) = app.probe.filesystems.get(r.selected() as usize) {
                app.answers.borrow_mut().fs = f.clone();
            }
        }
    });
    layout.add(&fs_row);

    let swap_default = if app.probe.swap_suggested.is_empty() {
        "2G".to_string()
    } else {
        app.probe.swap_suggested.clone()
    };
    let swap_row = adw::SwitchRow::builder()
        .title(format!("Create a {swap_default} swap partition"))
        .subtitle("Sized to this machine's memory, so hibernate has somewhere to go")
        .active(true)
        .build();
    if !app.probe.mkswap {
        // preflight would die on this. Say so where the switch is.
        swap_row.set_active(false);
        swap_row.set_sensitive(false);
        swap_row.set_subtitle("mkswap is not installed on this image");
        app.answers.borrow_mut().swap = "none".into();
    }
    swap_row.connect_active_notify({
        let app = app.clone();
        move |r| {
            app.answers.borrow_mut().swap = if r.is_active() { String::new() } else { "none".into() };
            app.revalidate();
        }
    });
    layout.add(&swap_row);
    page.add(&layout);

    // ---- advanced -----------------------------------------------------------
    let advanced = adw::PreferencesGroup::builder()
        .title("Advanced")
        .description("The defaults are right on almost every machine.")
        .build();

    let esp_row = adw::EntryRow::builder().title("EFI System Partition size").build();
    esp_row.set_text(&app.answers.borrow().esp_size);
    esp_row.connect_changed({
        let app = app.clone();
        move |e| {
            app.answers.borrow_mut().esp_size = e.text().to_string();
            app.revalidate();
        }
    });
    advanced.add(&esp_row);

    let nvram_row = adw::SwitchRow::builder()
        .title("Register a UEFI boot entry")
        .subtitle(
            "Off by default: the fallback bootloader path boots without one, and \
             writing NVRAM can reorder the firmware's existing entries",
        )
        .active(false)
        .build();
    if !app.probe.efibootmgr {
        nvram_row.set_sensitive(false);
        nvram_row.set_subtitle("efibootmgr is not installed; the fallback path is used");
    }
    nvram_row.connect_active_notify({
        let app = app.clone();
        move |r| app.answers.borrow_mut().efi_nvram = r.is_active()
    });
    advanced.add(&nvram_row);
    page.add(&advanced);

    push(
        app,
        "disk",
        "Disk",
        &scrolled(&page),
        Box::new(|a, p| {
            let mut v = Vec::new();
            if a.disk.is_empty() {
                v.push("Choose the disk to install onto.".to_string());
                return v;
            }
            let esp_mb = match probe::size_to_mb(&a.esp_size) {
                Some(mb) if mb >= 32 => mb,
                Some(_) => {
                    v.push("The EFI System Partition needs to be at least 32M.".into());
                    return v;
                }
                None => {
                    v.push("The EFI System Partition size must be a number followed by M or G.".into());
                    return v;
                }
            };
            let swap_mb = match a.swap.as_str() {
                "none" => 0,
                "" => probe::size_to_mb(&p.swap_suggested).unwrap_or(0),
                other => probe::size_to_mb(other).unwrap_or(0),
            };
            if let Some(d) = p.disks.iter().find(|d| d.dev == a.disk) {
                if p.disk_too_small(d, swap_mb, esp_mb) {
                    v.push(format!(
                        "{} is too small for this system ({:.1} GB) plus swap and the ESP.",
                        d.dev,
                        p.source_size_mb as f64 / 1024.0
                    ));
                }
            }
            v
        }),
    );
}

// =============================================================================
// Account
// =============================================================================

fn account(app: &Rc<App>) {
    let page = adw::PreferencesPage::new();

    let machine = adw::PreferencesGroup::builder().title("This machine").build();
    let hostname = adw::EntryRow::builder().title("Hostname").build();
    hostname.set_text(&app.answers.borrow().hostname);
    hostname.connect_changed({
        let app = app.clone();
        move |e| {
            app.answers.borrow_mut().hostname = e.text().to_string();
            app.revalidate();
        }
    });
    machine.add(&hostname);
    page.add(&machine);

    let user = adw::PreferencesGroup::builder()
        .title("Your account")
        .description("The account you will log in as. The root account is set up separately below.")
        .build();

    let fullname = adw::EntryRow::builder().title("Full name (optional)").build();
    let username = adw::EntryRow::builder().title("Username").build();
    username.set_text(&app.answers.borrow().username);

    // Typing a name fills the username in, until the username is typed into --
    // then it is somebody's choice and is left alone.
    let username_touched = Rc::new(std::cell::Cell::new(false));
    fullname.connect_changed({
        let app = app.clone();
        let username = username.clone();
        let touched = username_touched.clone();
        move |e| {
            let text = e.text().to_string();
            app.answers.borrow_mut().fullname = text.clone();
            if !touched.get() {
                let guess: String = text
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .collect::<String>()
                    .to_lowercase();
                if !guess.is_empty() {
                    username.set_text(&guess);
                    app.answers.borrow_mut().username = guess;
                }
            }
            app.revalidate();
        }
    });
    username.connect_changed({
        let app = app.clone();
        let touched = username_touched.clone();
        move |e| {
            touched.set(true);
            app.answers.borrow_mut().username = e.text().to_string();
            app.revalidate();
        }
    });
    user.add(&fullname);
    user.add(&username);

    let pw = adw::PasswordEntryRow::builder().title("Password").build();
    let pw2 = adw::PasswordEntryRow::builder().title("Confirm password").build();
    pw.connect_changed({
        let app = app.clone();
        move |e| {
            app.answers.borrow_mut().user_password = e.text().to_string();
            app.revalidate();
        }
    });
    pw2.connect_changed({
        let app = app.clone();
        move |_| app.revalidate()
    });
    user.add(&pw);
    user.add(&pw2);

    let sudo = adw::SwitchRow::builder()
        .title("Allow this account to run administrative commands")
        .subtitle("Adds it to the wheel group, and makes sure sudoers honours wheel")
        .active(true)
        .build();
    sudo.connect_active_notify({
        let app = app.clone();
        move |r| app.answers.borrow_mut().user_sudo = r.is_active()
    });
    user.add(&sudo);
    page.add(&user);

    let root = adw::PreferencesGroup::builder()
        .title("Root account")
        .description(
            "Leave both empty to lock root. With sudo enabled above, you will not \
             need it.",
        )
        .build();
    let rpw = adw::PasswordEntryRow::builder().title("Root password").build();
    let rpw2 = adw::PasswordEntryRow::builder().title("Confirm root password").build();
    rpw.connect_changed({
        let app = app.clone();
        move |e| {
            app.answers.borrow_mut().root_password = e.text().to_string();
            app.revalidate();
        }
    });
    rpw2.connect_changed({
        let app = app.clone();
        move |_| app.revalidate()
    });
    root.add(&rpw);
    root.add(&rpw2);
    page.add(&root);

    // The two confirmations are not answers, so they are checked here against
    // the widgets rather than against Answers -- which is also why they cannot
    // be forgotten in the file the installer reads.
    push(
        app,
        "account",
        "Account",
        &scrolled(&page),
        Box::new(move |a, _| {
            let mut v = Vec::new();
            if !crate::answers::valid_hostname(&a.hostname) {
                v.push(
                    "Hostnames are letters, digits and hyphens, and cannot start or end with a hyphen."
                        .to_string(),
                );
            }
            if a.username == "root" {
                v.push("Pick a name other than root; the root account is configured separately.".into());
            } else if !crate::answers::valid_username(&a.username) {
                v.push(
                    "Usernames are lowercase letters, digits, underscore and hyphen, starting with a letter or underscore."
                        .into(),
                );
            }
            if a.fullname.contains(':') {
                v.push("The full name cannot contain a colon.".into());
            }
            if pw.text() != pw2.text() {
                v.push("The passwords do not match.".into());
            }
            if rpw.text() != rpw2.text() {
                v.push("The root passwords do not match.".into());
            }
            v
        }),
    );
}

// =============================================================================
// Locale
// =============================================================================

fn locale(app: &Rc<App>) {
    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::builder().title("Time and language").build();

    let zones: Vec<&str> = app.probe.timezones.iter().map(String::as_str).collect();
    if zones.is_empty() {
        // zoneinfo_dir found nothing; the installer warns and runs on UTC.
        let row = adw::ActionRow::builder()
            .title("Timezone")
            .subtitle("No timezone database in the system being installed; the clock will run on UTC")
            .build();
        row.set_subtitle_lines(0);
        group.add(&row);
        app.answers.borrow_mut().timezone = "UTC".into();
    } else {
        let model = gtk::StringList::new(&zones);
        let tz = adw::ComboRow::builder()
            .title("Timezone")
            .model(&model)
            .build();
        // 600 entries is a list nobody scrolls. The search box is what makes
        // it usable, and it needs to be told which property to match on.
        tz.set_enable_search(true);
        tz.set_expression(Some(gtk::PropertyExpression::new(
            gtk::StringObject::static_type(),
            None::<gtk::Expression>,
            "string",
        )));
        let want = app.answers.borrow().timezone.clone();
        let start = app
            .probe
            .timezones
            .iter()
            .position(|z| *z == want)
            .or_else(|| app.probe.timezones.iter().position(|z| z == "UTC"))
            .unwrap_or(0);
        tz.set_selected(start as u32);
        app.answers.borrow_mut().timezone = app.probe.timezones[start].clone();
        tz.connect_selected_notify({
            let app = app.clone();
            move |r| {
                if let Some(z) = app.probe.timezones.get(r.selected() as usize) {
                    app.answers.borrow_mut().timezone = z.clone();
                    app.revalidate();
                }
            }
        });
        group.add(&tz);
    }

    let locale = adw::EntryRow::builder().title("Locale").build();
    locale.set_text(&app.answers.borrow().locale);
    locale.connect_changed({
        let app = app.clone();
        move |e| {
            app.answers.borrow_mut().locale = e.text().to_string();
            app.revalidate();
        }
    });
    group.add(&locale);

    let keymap = adw::EntryRow::builder().title("Console keymap").build();
    keymap.set_text(&app.answers.borrow().keymap);
    keymap.connect_changed({
        let app = app.clone();
        move |e| {
            app.answers.borrow_mut().keymap = e.text().to_string();
            app.revalidate();
        }
    });
    group.add(&keymap);
    page.add(&group);

    // set_locale_and_time warns about exactly this after the fact. Saying it
    // next to the field is the difference between a setting that works and a
    // setting that looks like it does.
    let note = adw::PreferencesGroup::new();
    let row = adw::ActionRow::builder()
        .title("A keymap other than \"us\" is recorded but not applied")
        .subtitle(
            "It is written to /etc/vconsole.conf. RavenLinux does not ship \
             loadkeys yet, so the console stays on us until it does.",
        )
        .build();
    row.set_title_lines(0);
    row.set_subtitle_lines(0);
    let icon = gtk::Image::from_icon_name("dialog-information-symbolic");
    icon.add_css_class("dim-label");
    row.add_prefix(&icon);
    note.add(&row);
    page.add(&note);

    push(
        app,
        "locale",
        "Locale",
        &scrolled(&page),
        Box::new(|a, _| {
            let mut v = Vec::new();
            if a.locale.trim().is_empty() {
                v.push("A locale is needed; en_US.UTF-8 if in doubt.".into());
            }
            if a.keymap.trim().is_empty() {
                v.push("A console keymap is needed; us if in doubt.".into());
            }
            v
        }),
    );
}

// =============================================================================
// Profile
// =============================================================================

fn profile(app: &Rc<App>) {
    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::builder()
        .title("Package profile")
        .description(
            "Chosen now, applied later: the installed system runs raven-postinstall \
             once it has networking, and that is what installs the packages.",
        )
        .build();

    let describe = |name: &str| -> &'static str {
        match name {
            "minimal" => "The base system and nothing else. A console.",
            "desktop" => "The Huginn desktop, its terminal, file manager and settings.",
            "developer" => "The desktop, plus the toolchains and editors.",
            _ => "",
        }
    };

    let mut first: Option<gtk::CheckButton> = None;
    for name in &app.probe.profiles {
        let row = adw::ActionRow::builder()
            .title(name)
            .subtitle(describe(name))
            .build();
        let check = gtk::CheckButton::new();
        if let Some(f) = &first {
            check.set_group(Some(f));
        } else {
            first = Some(check.clone());
        }
        check.set_active(app.answers.borrow().profile == *name);
        row.add_prefix(&check);
        row.set_activatable_widget(Some(&check));
        check.connect_toggled({
            let app = app.clone();
            let name = name.clone();
            move |c| {
                if c.is_active() {
                    app.answers.borrow_mut().profile = name.clone();
                    app.revalidate();
                }
            }
        });
        group.add(&row);
    }
    page.add(&group);

    if !app.probe.has_desktop {
        // install_bootloader writes the Desktop boot entry only when the copied
        // root carries a compositor, and preselects it only for these two
        // profiles. Without huginn, choosing them changes what gets installed
        // later and nothing about how this machine boots.
        let note = adw::PreferencesGroup::new();
        let row = adw::ActionRow::builder()
            .title("This image carries no compositor")
            .subtitle(
                "There is no huginn in the system being installed, so the \
                 installed machine boots to a console whichever profile you \
                 pick. raven-postinstall can install the desktop afterwards.",
            )
            .build();
        row.set_subtitle_lines(0);
        let icon = gtk::Image::from_icon_name("dialog-information-symbolic");
        icon.add_css_class("dim-label");
        row.add_prefix(&icon);
        note.add(&row);
        page.add(&note);
    }

    push(
        app,
        "profile",
        "Profile",
        &scrolled(&page),
        Box::new(|a, _| {
            if a.profile.is_empty() {
                vec!["Choose a package profile.".into()]
            } else {
                Vec::new()
            }
        }),
    );
}

// =============================================================================
// Summary
// =============================================================================

/// nvme0n1 -> nvme0n1p1, sda -> sda1. The same rule as partdev() in
/// raven-install, and the only piece of that script's logic restated here --
/// it is restated because the summary names the partitions it is about to
/// create, and a plan that does not name them is not the plan the terminal
/// shows.
fn partdev(disk: &str, n: u32) -> String {
    if disk.chars().last().is_some_and(|c| c.is_ascii_digit()) {
        format!("{disk}p{n}")
    } else {
        format!("{disk}{n}")
    }
}

/// The text `confirm()` prints, as a table.
fn plan_rows(a: &Answers, p: &Probe) -> Vec<(String, String)> {
    let mut n = 1;
    let esp = partdev(&a.disk, n);
    n += 1;
    let swap_size = match a.swap.as_str() {
        "none" => String::new(),
        "" => p.swap_suggested.clone(),
        other => other.to_string(),
    };
    let swap = if swap_size.is_empty() {
        None
    } else {
        let d = partdev(&a.disk, n);
        n += 1;
        Some(d)
    };
    let root = partdev(&a.disk, n);

    let disk_info = p
        .disks
        .iter()
        .find(|d| d.dev == a.disk)
        .map(|d| format!("{} ({})", d.dev, d.subtitle()))
        .unwrap_or_else(|| a.disk.clone());

    let mut layout = format!("{esp}   {}   EFI System Partition (FAT32) → /boot/efi", a.esp_size);
    if let Some(sd) = &swap {
        layout.push_str(&format!("\n{sd}   {swap_size}   swap"));
    }
    layout.push_str(&format!("\n{root}   rest   {} → /", a.fs));

    vec![
        ("Disk".into(), disk_info),
        ("Partitioning".into(), format!("GPT, erasing everything on the disk\n{layout}")),
        ("Hostname".into(), a.hostname.clone()),
        (
            "User".into(),
            format!(
                "{}{}{}",
                a.username,
                if a.fullname.is_empty() { String::new() } else { format!(" ({})", a.fullname) },
                if a.user_sudo { ", may use sudo via wheel" } else { "" }
            ),
        ),
        (
            "Passwords".into(),
            format!(
                "{}, root {}",
                if a.user_password.is_empty() { format!("{} not set", a.username) } else { format!("{} set", a.username) },
                if a.root_password.is_empty() { "locked" } else { "set" }
            ),
        ),
        ("Timezone".into(), a.timezone.clone()),
        ("Locale".into(), format!("{}, keymap {}", a.locale, a.keymap)),
        (
            "Package profile".into(),
            format!("{} (applied later with raven-postinstall)", a.profile),
        ),
        (
            "Bootloader".into(),
            format!(
                "RavenBoot at \\EFI\\raven and \\EFI\\BOOT\\BOOTX64.EFI{}",
                if a.efi_nvram { ", plus an NVRAM entry" } else { "" }
            ),
        ),
    ]
}

fn summary(app: &Rc<App>) {
    let page = adw::PreferencesPage::new();

    let danger = adw::PreferencesGroup::new();
    let danger_row = adw::ActionRow::builder().title("").build();
    danger_row.set_title_lines(0);
    danger_row.add_css_class("danger-row");
    let danger_icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
    danger_icon.add_css_class("error");
    danger_row.add_prefix(&danger_icon);
    danger.add(&danger_row);
    page.add(&danger);

    let plan = adw::PreferencesGroup::builder().title("Installation plan").build();
    page.add(&plan);

    let nopw = adw::PreferencesGroup::new();
    let nopw_row = adw::ActionRow::builder()
        .title("Neither account has a password")
        .subtitle("Anyone with the machine can log in.")
        .build();
    let nopw_icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
    nopw_icon.add_css_class("warning");
    nopw_row.add_prefix(&nopw_icon);
    nopw.add(&nopw_row);
    nopw.set_visible(false);
    page.add(&nopw);

    // Rebuilt on entry rather than on every keystroke: it summarises five
    // pages, and it is only ever looked at from this one.
    let rows: Rc<std::cell::RefCell<Vec<adw::ActionRow>>> = Rc::new(std::cell::RefCell::new(Vec::new()));
    let refresh = {
        let app = app.clone();
        let plan = plan.clone();
        let rows = rows.clone();
        let danger_row = danger_row.clone();
        let nopw = nopw.clone();
        move || {
            let a = app.answers.borrow();
            for r in rows.borrow_mut().drain(..) {
                plan.remove(&r);
            }
            for (k, v) in plan_rows(&a, &app.probe) {
                let row = adw::ActionRow::builder().title(k).subtitle(v).build();
                row.set_subtitle_lines(0);
                row.add_css_class("property");
                plan.add(&row);
                rows.borrow_mut().push(row);
            }
            danger_row.set_title(&format!("Everything on {} will be destroyed", a.disk));
            nopw.set_visible(a.no_password_anywhere());
        }
    };
    refresh();

    app.stack.connect_visible_child_name_notify({
        let refresh = refresh.clone();
        move |s| {
            if s.visible_child_name().as_deref() == Some("summary") {
                refresh();
            }
        }
    });

    push(
        app,
        "summary",
        "Summary",
        &scrolled(&page),
        Box::new(|a, _| a.problems()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_names_follow_the_device_naming_rule() {
        assert_eq!(partdev("/dev/sda", 1), "/dev/sda1");
        assert_eq!(partdev("/dev/nvme0n1", 3), "/dev/nvme0n1p3");
        assert_eq!(partdev("/dev/mmcblk0", 2), "/dev/mmcblk0p2");
    }

    #[test]
    fn the_plan_numbers_root_around_the_swap_partition() {
        let p = Probe {
            swap_suggested: "8G".into(),
            ..Default::default()
        };
        let with_swap = Answers { disk: "/dev/sda".into(), ..Default::default() };
        let text = plan_rows(&with_swap, &p)[1].1.clone();
        assert!(text.contains("/dev/sda1"), "esp");
        assert!(text.contains("/dev/sda2   8G   swap"), "swap: {text}");
        assert!(text.contains("/dev/sda3   rest"), "root: {text}");

        let no_swap = Answers { disk: "/dev/sda".into(), swap: "none".into(), ..Default::default() };
        let text = plan_rows(&no_swap, &p)[1].1.clone();
        assert!(!text.contains("swap"));
        // Root moves up to 2 when there is no swap partition before it.
        assert!(text.contains("/dev/sda2   rest"), "root: {text}");
    }

    #[test]
    fn a_locked_root_is_described_as_locked() {
        let a = Answers { disk: "/dev/sda".into(), ..Default::default() };
        let rows = plan_rows(&a, &Probe::default());
        let pw = &rows.iter().find(|(k, _)| k == "Passwords").unwrap().1;
        assert!(pw.contains("root locked"));
        assert!(pw.contains("raven not set"));
    }
}
