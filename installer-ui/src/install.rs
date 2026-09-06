//! The two pages that are not questions: the install running, and what it did.
//!
//! Everything shown here arrives from raven-install. The phase list and the
//! bar come from the progress protocol; the text under "Details" is the
//! installer's own output, unedited, which is what a person would have been
//! reading had they run it in a terminal.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::engine::{self, Event, PHASES};
use crate::{privesc, App};

struct Progress {
    bar: gtk::ProgressBar,
    status: gtk::Label,
    rows: Vec<(adw::ActionRow, gtk::Image, gtk::Spinner)>,
    log: gtk::TextBuffer,
    log_view: gtk::TextView,
    log_scroll: gtk::ScrolledWindow,
    /// Warnings raise it; a fail sets it. Read by the done page.
    warnings: RefCell<Vec<String>>,
    failure: RefCell<Option<String>>,
}

/// Built once and called when the install ends, with the exit code, so the
/// finished page can describe what actually happened rather than being a
/// second copy of the wizard's assumptions.
type DoneBuilder = Rc<dyn Fn(&Rc<App>, i32)>;

thread_local! {
    static PROGRESS: RefCell<Option<Rc<Progress>>> = const { RefCell::new(None) };
    static DONE_BUILDER: RefCell<Option<DoneBuilder>> = const { RefCell::new(None) };
}

pub fn build_pages(app: &Rc<App>) {
    build_progress(app);
    build_done(app);
}

// =============================================================================
// Running
// =============================================================================

fn build_progress(app: &Rc<App>) {
    let outer = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();

    let heading = gtk::Label::builder()
        .label("Installing RavenLinux")
        .halign(gtk::Align::Start)
        .build();
    heading.add_css_class("title-2");
    outer.append(&heading);

    let status = gtk::Label::builder()
        .label("Starting…")
        .halign(gtk::Align::Start)
        .wrap(true)
        .build();
    status.add_css_class("dim-label");
    outer.append(&status);

    let bar = gtk::ProgressBar::builder().show_text(false).build();
    outer.append(&bar);

    // One row per phase, so the thing being waited on is named rather than
    // implied by a bar that has not moved in two minutes.
    let list = adw::PreferencesGroup::new();
    let mut rows = Vec::new();
    for (_, title, _) in PHASES {
        let row = adw::ActionRow::builder().title(*title).build();
        let icon = gtk::Image::from_icon_name("emblem-ok-symbolic");
        icon.set_visible(false);
        let spinner = gtk::Spinner::new();
        spinner.set_visible(false);
        row.add_prefix(&spinner);
        row.add_suffix(&icon);
        row.set_sensitive(false);
        list.add(&row);
        rows.push((row, icon, spinner));
    }
    outer.append(&list);

    let log = gtk::TextBuffer::new(None);
    let log_view = gtk::TextView::builder()
        .buffer(&log)
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .left_margin(8)
        .right_margin(8)
        .top_margin(8)
        .bottom_margin(8)
        .wrap_mode(gtk::WrapMode::WordChar)
        .build();
    let log_scroll = gtk::ScrolledWindow::builder()
        .height_request(200)
        .child(&log_view)
        .build();
    log_scroll.add_css_class("log-view");

    let expander = gtk::Expander::builder()
        .label("Details")
        .child(&log_scroll)
        .vexpand(true)
        .build();
    outer.append(&expander);

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&outer)
        .build();
    app.stack.add_named(&scrolled, Some("installing"));

    PROGRESS.with(|p| {
        *p.borrow_mut() = Some(Rc::new(Progress {
            bar,
            status,
            rows,
            log,
            log_view,
            log_scroll,
            warnings: RefCell::new(Vec::new()),
            failure: RefCell::new(None),
        }))
    });
}

impl Progress {
    fn phase_index(id: &str) -> Option<usize> {
        PHASES.iter().position(|(pid, _, _)| *pid == id)
    }

    fn enter_phase(&self, id: &str, text: &str) {
        self.status.set_text(text);
        let Some(i) = Self::phase_index(id) else { return };
        for (n, (row, icon, spinner)) in self.rows.iter().enumerate() {
            match n.cmp(&i) {
                std::cmp::Ordering::Less => {
                    row.set_sensitive(true);
                    row.remove_css_class("phase-current");
                    spinner.set_visible(false);
                    spinner.set_spinning(false);
                    icon.set_visible(true);
                }
                std::cmp::Ordering::Equal => {
                    row.set_sensitive(true);
                    // The accent, so the phase being waited on is findable in
                    // the list without reading it.
                    row.add_css_class("phase-current");
                    icon.set_visible(false);
                    spinner.set_visible(true);
                    spinner.set_spinning(true);
                }
                std::cmp::Ordering::Greater => {
                    row.set_sensitive(false);
                    row.remove_css_class("phase-current");
                    icon.set_visible(false);
                    spinner.set_visible(false);
                    spinner.set_spinning(false);
                }
            }
        }
        self.bar.set_fraction(engine::overall_fraction(id, 0));
    }

    fn finish_all(&self, success: bool) {
        for (row, icon, spinner) in &self.rows {
            row.remove_css_class("phase-current");
            spinner.set_visible(false);
            spinner.set_spinning(false);
            if success {
                row.set_sensitive(true);
                icon.set_visible(true);
            }
        }
        if success {
            self.bar.set_fraction(1.0);
        }
    }

    fn append_log(&self, line: &str) {
        let mut end = self.log.end_iter();
        self.log.insert(&mut end, line);
        self.log.insert(&mut end, "\n");

        // Follow the tail only while the view is already at the bottom, so
        // scrolling back to read something is not undone a second later.
        let adj = self.log_scroll.vadjustment();
        let at_bottom = adj.value() + adj.page_size() >= adj.upper() - 32.0;
        if at_bottom {
            let mark = self.log.create_mark(None, &self.log.end_iter(), false);
            self.log_view.scroll_to_mark(&mark, 0.0, false, 0.0, 0.0);
            self.log.delete_mark(&mark);
        }
    }
}

/// Leave the wizard and start the installer. From here the Back button is
/// meaningless -- the disk is about to stop matching any earlier page -- so
/// the whole navigation bar goes.
pub fn begin(app: &Rc<App>) {
    let run = match engine::Run::create(&app.answers.borrow()) {
        Ok(r) => r,
        Err(e) => {
            app.banner.set_title(&e);
            app.banner.set_revealed(true);
            return;
        }
    };

    let prefix = privesc::wrap(app.priv_level, &[]);
    let rx = match engine::start(&prefix, &app.installer, &run) {
        Ok(rx) => rx,
        Err(e) => {
            app.banner.set_title(&e);
            app.banner.set_revealed(true);
            return;
        }
    };

    app.installing.set(true);
    app.nav.set_visible(false);
    app.banner.set_revealed(false);
    app.stack.set_visible_child_name("installing");

    let progress = PROGRESS.with(|p| p.borrow().clone()).expect("progress page");
    progress.enter_phase("preflight", "Checking this machine…");

    let app = app.clone();
    glib::spawn_future_local(async move {
        // Held for the whole install: dropping it unlinks the answers file,
        // and the installer is still reading it.
        let run = run;
        let mut reported: Option<i32> = None;

        while let Ok(event) = rx.recv().await {
            match event {
                Event::Phase { id, text } => progress.enter_phase(&id, &text),
                Event::Pct { id, pct } => {
                    progress.bar.set_fraction(engine::overall_fraction(&id, pct));
                }
                Event::Warn(t) => {
                    progress.warnings.borrow_mut().push(t.clone());
                    progress.status.set_text(&t);
                }
                Event::Fail(t) => {
                    let mut f = progress.failure.borrow_mut();
                    // The first failure is the cause; the ones after it are
                    // consequences, and "Installation aborted." is the last.
                    if f.is_none() {
                        *f = Some(t);
                    }
                }
                Event::Dirty(d) => app.disk_dirty.set(d),
                // The finest-grained thing the installer says. It moves fast
                // during a configure, which is the point: the line under the
                // heading is what shows an install is alive between the two
                // phases that take minutes.
                Event::Ok(t) | Event::Info(t) => progress.status.set_text(&t),
                Event::Log(line) => progress.append_log(&line),
                Event::Done(code) => reported = Some(code),
                Event::Ended(code) => {
                    drop(run);
                    let code = reported.unwrap_or(code);
                    progress.finish_all(code == 0);
                    app.installing.set(false);
                    show_done(&app, code);
                    return;
                }
            }
        }
    });
}

// =============================================================================
// Finished
// =============================================================================

fn build_done(app: &Rc<App>) {
    let holder = adw::Bin::new();
    app.stack.add_named(&holder, Some("done"));

    let builder = move |app: &Rc<App>, code: i32| {
        let progress = PROGRESS.with(|p| p.borrow().clone()).expect("progress page");
        let a = app.answers.borrow();
        let p = &app.probe;

        let status = adw::StatusPage::new();
        let group = adw::PreferencesGroup::new();

        let mut notes: Vec<(&str, String, &str)> = Vec::new();

        if code == 0 {
            status.set_icon_name(Some("emblem-ok-symbolic"));
            status.set_title("RavenLinux is installed");
            status.set_description(Some(&format!(
                "On {}. Log in as {} on tty1.",
                a.disk, a.username
            )));

            notes.push((
                "Remove the installation media",
                "Before rebooting, so the firmware boots the disk rather than the stick.".into(),
                "media-removable-symbolic",
            ));
            if p.secureboot == "on" {
                notes.push((
                    "Disable Secure Boot",
                    "RavenBoot is unsigned and the firmware will not load it until you do."
                        .into(),
                    "dialog-warning-symbolic",
                ));
            }
            if !a.efi_nvram {
                notes.push((
                    "The firmware boots this disk through the removable-media path",
                    "If the disk does not appear, choose it directly from the firmware's \
                     boot menu (F8 on ASUS), or reinstall with a UEFI boot entry."
                        .into(),
                    "dialog-information-symbolic",
                ));
            }
            if a.user_password.is_empty() && a.root_password.is_empty() {
                notes.push((
                    "No password was set",
                    "Boot the \"RavenLinux (rescue shell)\" entry and run passwd.".into(),
                    "dialog-warning-symbolic",
                ));
            }
            notes.push((
                "If it does not boot",
                "Pick \"RavenLinux (rescue shell)\" from the RavenBoot menu -- it drops \
                 straight to a root shell on the installed root."
                    .into(),
                "dialog-information-symbolic",
            ));
            for w in progress.warnings.borrow().iter() {
                notes.push(("During the install", w.clone(), "dialog-warning-symbolic"));
            }
        } else {
            status.set_icon_name(Some("dialog-error-symbolic"));
            status.set_title("The installation did not finish");
            let reason = progress
                .failure
                .borrow()
                .clone()
                .unwrap_or_else(|| format!("raven-install exited with status {code}."));
            status.set_description(Some(&reason));

            if app.disk_dirty.get() {
                notes.push((
                    "The target disk has been modified and is not bootable",
                    format!("{} was partitioned before this failed. Start the install again.", a.disk),
                    "dialog-warning-symbolic",
                ));
            } else {
                notes.push((
                    "No changes were made to any disk",
                    "Nothing was partitioned, formatted or copied.".into(),
                    "dialog-information-symbolic",
                ));
            }
            notes.push((
                "The full log is below",
                "Everything raven-install printed, in order.".into(),
                "dialog-information-symbolic",
            ));
        }

        for (title, body, icon_name) in notes {
            let row = adw::ActionRow::builder().title(title).subtitle(body).build();
            row.set_title_lines(0);
            row.set_subtitle_lines(0);
            let icon = gtk::Image::from_icon_name(icon_name);
            if icon_name == "dialog-warning-symbolic" {
                icon.add_css_class("warning");
            } else {
                icon.add_css_class("dim-label");
            }
            row.add_prefix(&icon);
            group.add(&row);
        }

        let column = gtk::Box::new(gtk::Orientation::Vertical, 18);
        column.append(&group);

        // The log moves here from the progress page, so a failure is read
        // where it happened rather than behind a Back button that is gone.
        if let Some(parent) = progress.log_scroll.parent() {
            if let Ok(expander) = parent.downcast::<gtk::Expander>() {
                expander.set_child(None::<&gtk::Widget>);
            }
        }
        let log_expander = gtk::Expander::builder()
            .label("Installation log")
            .expanded(code != 0)
            .child(&progress.log_scroll)
            .build();
        column.append(&log_expander);

        let buttons = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .halign(gtk::Align::Center)
            .build();

        let close = gtk::Button::with_label("Close");
        close.add_css_class("pill");
        close.connect_clicked({
            let window = app.window.clone();
            move |_| window.destroy()
        });

        if code == 0 {
            let reboot = gtk::Button::with_label("Restart Now");
            reboot.add_css_class("suggested-action");
            reboot.add_css_class("pill");
            reboot.connect_clicked({
                let app = app.clone();
                move |_| confirm_reboot(&app)
            });
            buttons.append(&close);
            buttons.append(&reboot);
        } else {
            buttons.append(&close);
        }
        column.append(&buttons);

        status.set_child(Some(&column));

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&status)
            .build();
        holder.set_child(Some(&scrolled));
    };

    DONE_BUILDER.with(|b| *b.borrow_mut() = Some(Rc::new(builder)));
}

fn show_done(app: &Rc<App>, code: i32) {
    let builder = DONE_BUILDER.with(|b| b.borrow().clone());
    if let Some(builder) = builder {
        builder(app, code);
    }
    app.stack.set_visible_child_name("done");
}

/// The wizard's last question, which the terminal asks as "Reboot now? [y/N]".
fn confirm_reboot(app: &Rc<App>) {
    let dialog = adw::MessageDialog::new(
        Some(&app.window),
        Some("Restart now?"),
        Some("Remove the installation media first, or the machine will boot it again."),
    );
    dialog.add_responses(&[("cancel", "Cancel"), ("restart", "Restart")]);
    dialog.set_response_appearance("restart", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.connect_response(None, {
        let app = app.clone();
        move |d, r| {
            d.close();
            if r != "restart" {
                return;
            }
            let argv = privesc::wrap(app.priv_level, &["reboot"]);
            if let Some((head, tail)) = argv.split_first() {
                let _ = std::process::Command::new(head).args(tail).spawn();
            }
        }
    });
    dialog.present();
}
