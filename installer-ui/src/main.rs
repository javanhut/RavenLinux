//! raven-installer-ui -- the graphical front-end for raven-install.
//!
//! It asks the questions raven-install's wizard asks, in the order it asks
//! them, and then runs raven-install. It does not partition, format, copy,
//! configure or write a bootloader; there is exactly one implementation of
//! each of those in RavenLinux and it is a shell script. What this program
//! adds is a window.
//!
//! The parity is structural rather than maintained by hand:
//!
//!   - what to ask about comes from `raven-install --probe`, so the disks,
//!     filesystems, timezones and profiles offered here are the ones that
//!     script would offer, including the ones it refuses;
//!   - the answers go back as a file it parses, validated by the same rules
//!     its own apply_answers uses;
//!   - progress comes back as the protocol documented at the top of that
//!     script, with its ordinary output shown underneath verbatim.
//!
//! Adding a question to the wizard means adding a key to that script and a row
//! here. Changing how Raven installs means changing neither.

mod answers;
mod engine;
mod install;
mod pages;
mod privesc;
mod probe;
mod theme;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use answers::Answers;
use privesc::Priv;
use probe::Probe;

pub const APP_ID: &str = "com.raveninstaller.Raven";

/// Where the installer is. Overridable so the front-end can be run against a
/// working copy without installing it first -- which is how it is developed,
/// since the alternative is rebuilding an ISO to test a button.
fn installer_path() -> String {
    std::env::var("RAVEN_INSTALL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "raven-install".to_string())
}

/// Why a page is not finished, or empty. Re-run on every change.
pub type Validator = Box<dyn Fn(&Answers, &Probe) -> Vec<String>>;

/// One step of the wizard.
pub struct PageDef {
    pub name: &'static str,
    pub title: &'static str,
    /// What decides whether Next is sensitive.
    pub validate: Validator,
}

/// The shared state every page writes into and reads from.
pub struct App {
    pub probe: Rc<Probe>,
    pub answers: Rc<RefCell<Answers>>,
    pub priv_level: Priv,
    pub installer: String,

    pub stack: gtk::Stack,
    pub banner: adw::Banner,
    pub back: gtk::Button,
    pub next: gtk::Button,
    pub nav: gtk::Widget,
    pub window: adw::ApplicationWindow,

    pub defs: RefCell<Vec<PageDef>>,
    pub current: Cell<usize>,
    /// Set once the installer reports it has written to the target disk. From
    /// then on closing the window is a question, not a click.
    pub disk_dirty: Cell<bool>,
    pub installing: Cell<bool>,
}

impl App {
    /// Re-check the page on screen and reflect it in the buttons and banner.
    pub fn revalidate(self: &Rc<Self>) {
        let i = self.current.get();
        let defs = self.defs.borrow();
        let Some(def) = defs.get(i) else { return };
        let problems = (def.validate)(&self.answers.borrow(), &self.probe);

        self.next.set_sensitive(problems.is_empty());
        if problems.is_empty() {
            self.banner.set_revealed(false);
        } else {
            // One at a time, in the order they would be fixed. A list of five
            // complaints about a form somebody has not finished filling in is
            // noise; the first one is the next thing to do.
            self.banner.set_title(&problems[0]);
            self.banner.set_revealed(true);
        }
    }

    pub fn goto(self: &Rc<Self>, index: usize) {
        let defs_len = self.defs.borrow().len();
        if index >= defs_len {
            return;
        }
        self.current.set(index);
        let name = self.defs.borrow()[index].name;
        self.stack.set_visible_child_name(name);

        self.back.set_sensitive(index > 0);
        // The last wizard page is the summary, and its button destroys a disk.
        let last = defs_len - 1;
        if index == last {
            self.next.set_label("Install");
            self.next.add_css_class("destructive-action");
            self.next.remove_css_class("suggested-action");
        } else {
            self.next.set_label("Next");
            self.next.remove_css_class("destructive-action");
            self.next.add_css_class("suggested-action");
        }
        self.revalidate();
    }

    pub fn next_page(self: &Rc<Self>) {
        let i = self.current.get();
        if i + 1 < self.defs.borrow().len() {
            self.goto(i + 1);
        } else {
            install::begin(self);
        }
    }
}

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build);
    // No arguments of its own: everything it needs it asks the installer for.
    app.run_with_args::<&str>(&[])
}

fn build(app: &adw::Application) {
    // Before any widget exists: a provider added later restyles what is
    // already on screen, and the password page would visibly change colour
    // the moment the wizard was built.
    let appearance = theme::read_appearance();
    theme::load(&appearance);

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Install RavenLinux")
        .default_width(880)
        .default_height(660)
        .build();
    theme::apply_to_window(&window, &appearance);

    // The probe needs root to mount the squashfs and read the firmware's
    // Secure Boot variable, so privilege is settled before anything is asked
    // rather than between the summary and the partitioning.
    //
    // Asked about the installer by name, not in general: on the live image the
    // sudoers rule grants that one command without a password, and a question
    // about anything else would be answered "password required" on an image
    // built specifically so that no password is needed.
    let installer = installer_path();
    let level = privesc::detect(&installer);
    match level {
        Priv::Root | Priv::Passwordless => start_probe(&window, level),
        Priv::NeedsPassword => ask_for_password(&window, level),
        Priv::Unavailable => {
            window.set_content(Some(&pages::fatal(
                "This account cannot become root",
                "Installing RavenLinux writes a partition table, so it has to run \
                 as root, and sudo will not grant it to this account.\n\n\
                 Log in as root, or run raven-install in a terminal.",
                None,
            )));
            window.present();
        }
    }
}

/// The password prompt. Shown before the wizard rather than in front of the
/// install, because an install that stops to ask for a password is an install
/// that stopped.
fn ask_for_password(window: &adw::ApplicationWindow, level: Priv) {
    let entry = adw::PasswordEntryRow::builder().title("Password").build();
    let group = adw::PreferencesGroup::new();
    group.add(&entry);

    let error = gtk::Label::builder()
        .wrap(true)
        .visible(false)
        .halign(gtk::Align::Center)
        .build();
    error.add_css_class("error");

    let unlock = gtk::Button::with_label("Continue");
    unlock.add_css_class("suggested-action");
    unlock.add_css_class("pill");
    unlock.set_halign(gtk::Align::Center);

    let status = adw::StatusPage::builder()
        .icon_name("dialog-password-symbolic")
        .title("Administrator password")
        .description(
            "Installing RavenLinux needs root. Your password is used to unlock \
             sudo and is not stored.",
        )
        .build();

    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_width_request(360);
    column.append(&group);
    column.append(&error);
    column.append(&unlock);
    status.set_child(Some(&column));

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&status));
    window.set_content(Some(&toolbar));

    let go = {
        let window = window.clone();
        let entry = entry.clone();
        let error = error.clone();
        move || {
            match privesc::authenticate(&entry.text()) {
                Ok(()) => start_probe(&window, level),
                Err(e) => {
                    error.set_text(&e);
                    error.set_visible(true);
                    entry.set_text("");
                    entry.grab_focus();
                }
            }
        }
    };
    unlock.connect_clicked({
        let go = go.clone();
        move |_| go()
    });
    entry.connect_entry_activated(move |_| go());

    window.present();
    entry.grab_focus();
}

/// Run the probe on a worker thread, showing a spinner, then build the wizard.
/// It is not instant -- it measures the size of the tree it would copy, the
/// same way the installer does -- and a window that appeared empty for four
/// seconds would look broken.
fn start_probe(window: &adw::ApplicationWindow, level: Priv) {
    let status = adw::StatusPage::builder()
        .title("Checking this machine")
        .description("Looking at the disks, the firmware and the system to install.")
        .build();
    let spinner = gtk::Spinner::builder()
        .spinning(true)
        .width_request(48)
        .height_request(48)
        .build();
    status.set_child(Some(&spinner));

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&status));
    window.set_content(Some(&toolbar));
    window.present();

    let installer = installer_path();
    let argv = privesc::wrap(level, &[installer.as_str(), "--probe"]);

    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = tx.send_blocking(probe::run(&argv));
    });

    let window = window.clone();
    glib::spawn_future_local(async move {
        let Ok(result) = rx.recv().await else { return };
        match result {
            Ok(p) => build_wizard(&window, p, level, installer),
            Err(e) => window.set_content(Some(&pages::fatal(
                "Cannot read this machine",
                &e,
                None,
            ))),
        }
    });
}

fn build_wizard(window: &adw::ApplicationWindow, p: Probe, level: Priv, installer: String) {
    // A machine the installer would refuse is a machine this refuses, with the
    // installer's own words for why. Reaching the disk page and failing at the
    // partitioning would be the same answer, four pages later.
    if !p.ok {
        let detail = if p.errors.is_empty() {
            "raven-install will not install onto this machine.".to_string()
        } else {
            p.errors.join("\n")
        };
        window.set_content(Some(&pages::fatal(
            "This machine cannot be installed to",
            &detail,
            Some(&p),
        )));
        return;
    }

    let mut answers = Answers::default();
    // Defaults that depend on the machine rather than on taste.
    if !p.esp_size_default.is_empty() {
        answers.esp_size = p.esp_size_default.clone();
    }
    if !p.filesystems.is_empty() && !p.filesystems.contains(&answers.fs) {
        answers.fs = p.filesystems[0].clone();
    }
    if !p.profiles.is_empty() && !p.profiles.contains(&answers.profile) {
        answers.profile = p.profiles[0].clone();
    }
    // One installable disk and no ambiguity: preselect it. The summary page
    // still names it, and still needs a deliberate click to erase it.
    let installable = p.installable_disks();
    if installable.len() == 1 {
        answers.disk = installable[0].dev.clone();
    }

    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::SlideLeftRight)
        .vexpand(true)
        .build();

    let banner = adw::Banner::new("");
    let back = gtk::Button::with_label("Back");
    let next = gtk::Button::with_label("Next");
    next.add_css_class("suggested-action");

    let nav = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .css_classes(["nav-bar"])
        .spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    nav.append(&back);
    nav.append(&spacer);
    nav.append(&next);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&banner);
    content.append(&stack);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&content));
    toolbar.add_bottom_bar(&nav);
    window.set_content(Some(&toolbar));

    let app = Rc::new(App {
        probe: Rc::new(p),
        answers: Rc::new(RefCell::new(answers)),
        priv_level: level,
        installer,
        stack,
        banner,
        back: back.clone(),
        next: next.clone(),
        nav: nav.upcast(),
        window: window.clone(),
        defs: RefCell::new(Vec::new()),
        current: Cell::new(0),
        disk_dirty: Cell::new(false),
        installing: Cell::new(false),
    });

    pages::build_all(&app);
    install::build_pages(&app);

    back.connect_clicked({
        let app = app.clone();
        move |_| {
            let i = app.current.get();
            if i > 0 {
                app.goto(i - 1);
            }
        }
    });
    next.connect_clicked({
        let app = app.clone();
        move |_| app.next_page()
    });

    // Closing halfway through a partitioning leaves a machine that boots
    // nothing, so once the disk has been touched the button asks first.
    window.connect_close_request({
        let app = app.clone();
        move |w| {
            if !app.installing.get() {
                return glib::Propagation::Proceed;
            }
            let body = if app.disk_dirty.get() {
                "The target disk has already been partitioned. Stopping now leaves \
                 it unbootable, and the install will have to be started again."
            } else {
                "The install has not written to the disk yet, but it is running."
            };
            let dialog = adw::MessageDialog::new(Some(w), Some("Stop the installation?"), Some(body));
            dialog.add_responses(&[("cancel", "Keep Installing"), ("stop", "Stop")]);
            dialog.set_response_appearance("stop", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));
            dialog.connect_response(None, {
                let w = w.clone();
                move |d, r| {
                    d.close();
                    if r == "stop" {
                        w.destroy();
                    }
                }
            });
            dialog.present();
            glib::Propagation::Stop
        }
    });

    app.goto(0);
}
