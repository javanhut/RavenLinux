use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Entry, Image, Label, ListBox, ListBoxRow,
    Orientation as GtkOrientation, Paned, ScrolledWindow, ApplicationWindow,
};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use raven_core::{ComponentId, ShellEvent};
use crate::common::{Component, ComponentContext};
use super::types::{self, FileEntry, Bookmark};
use super::navigation::History;
use super::clipboard::Clipboard;

const WINDOW_WIDTH: i32 = 1000;
const WINDOW_HEIGHT: i32 = 700;
const SIDEBAR_WIDTH: i32 = 200;
const PREVIEW_WIDTH: i32 = 280;

/// File manager state
struct FileManagerState {
    current_path: PathBuf,
    history: History,
    current_files: Vec<FileEntry>,
    selected_files: Vec<FileEntry>,
    clipboard: Clipboard,
    show_hidden: bool,
    show_preview: bool,
    bookmarks: Vec<Bookmark>,
}

impl Default for FileManagerState {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            current_path: home,
            history: History::new(),
            current_files: Vec::new(),
            selected_files: Vec::new(),
            clipboard: Clipboard::new(),
            show_hidden: false,
            show_preview: true,
            bookmarks: types::get_default_bookmarks(),
        }
    }
}

/// File manager component (regular window, not layer-shell)
pub struct FileManagerComponent {
    window: Option<ApplicationWindow>,
    ctx: Option<ComponentContext>,
    state: Option<Rc<RefCell<FileManagerState>>>,
    file_list: Option<Rc<RefCell<Option<ListBox>>>>,
    location_entry: Option<Rc<RefCell<Option<Entry>>>>,
    status_label: Option<Rc<RefCell<Option<Label>>>>,
    status_right: Option<Rc<RefCell<Option<Label>>>>,
    preview_pane: Option<Rc<RefCell<Option<GtkBox>>>>,
    back_btn: Option<Rc<RefCell<Option<Button>>>>,
    forward_btn: Option<Rc<RefCell<Option<Button>>>>,
    up_btn: Option<Rc<RefCell<Option<Button>>>>,
    initialized: bool,
}

impl FileManagerComponent {
    pub fn new() -> Self {
        Self {
            window: None,
            ctx: None,
            state: None,
            file_list: None,
            location_entry: None,
            status_label: None,
            status_right: None,
            preview_pane: None,
            back_btn: None,
            forward_btn: None,
            up_btn: None,
            initialized: false,
        }
    }

    fn build_ui(&self, window: &ApplicationWindow) -> GtkBox {
        let state = Rc::new(RefCell::new(FileManagerState::default()));
        let file_list = Rc::new(RefCell::new(None::<ListBox>));
        let location_entry = Rc::new(RefCell::new(None::<Entry>));
        let status_label = Rc::new(RefCell::new(None::<Label>));
        let status_right = Rc::new(RefCell::new(None::<Label>));
        let preview_pane = Rc::new(RefCell::new(None::<GtkBox>));
        let back_btn = Rc::new(RefCell::new(None::<Button>));
        let forward_btn = Rc::new(RefCell::new(None::<Button>));
        let up_btn = Rc::new(RefCell::new(None::<Button>));

        let main_box = GtkBox::new(GtkOrientation::Vertical, 0);
        main_box.add_css_class("file-manager");

        // Header bar
        let header = self.build_header(
            &state,
            &file_list,
            &location_entry,
            &back_btn,
            &forward_btn,
            &up_btn,
            &preview_pane,
        );
        main_box.append(&header);

        // Content area
        let content = GtkBox::new(GtkOrientation::Horizontal, 0);
        content.set_vexpand(true);

        // Sidebar
        let sidebar = self.build_sidebar(&state, &file_list, &location_entry, &status_label, &status_right, &back_btn, &forward_btn, &up_btn);
        content.append(&sidebar);

        // Main paned (file list + preview)
        let paned = Paned::new(GtkOrientation::Horizontal);
        paned.set_hexpand(true);

        // File area
        let file_area = self.build_file_area(&state, &file_list, &status_label, &status_right, &preview_pane);
        paned.set_start_child(Some(&file_area));

        // Preview pane
        let preview = self.build_preview_pane(&state, &preview_pane);
        paned.set_end_child(Some(&preview));
        paned.set_position(WINDOW_WIDTH - SIDEBAR_WIDTH - PREVIEW_WIDTH);
        paned.set_shrink_start_child(false);
        paned.set_shrink_end_child(false);

        content.append(&paned);
        main_box.append(&content);

        // Status bar
        let status = self.build_status_bar(&status_label, &status_right);
        main_box.append(&status);

        // Setup keyboard shortcuts
        self.setup_keyboard(window, &state, &file_list, &location_entry, &status_label, &status_right, &preview_pane, &back_btn, &forward_btn, &up_btn);

        // Initial load
        {
            let s = state.borrow();
            let path = s.current_path.clone();
            drop(s);
            self.load_directory(&state, &file_list, &status_label, &status_right, &path);
            state.borrow_mut().history.push(path);
        }

        main_box
    }

    fn build_header(
        &self,
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        location_entry: &Rc<RefCell<Option<Entry>>>,
        back_btn: &Rc<RefCell<Option<Button>>>,
        forward_btn: &Rc<RefCell<Option<Button>>>,
        up_btn: &Rc<RefCell<Option<Button>>>,
        preview_pane: &Rc<RefCell<Option<GtkBox>>>,
    ) -> GtkBox {
        let header = GtkBox::new(GtkOrientation::Horizontal, 8);
        header.add_css_class("fm-header");
        header.set_margin_start(8);
        header.set_margin_end(8);
        header.set_margin_top(8);
        header.set_margin_bottom(8);

        // Navigation buttons
        let nav_box = GtkBox::new(GtkOrientation::Horizontal, 4);

        let back = Button::new();
        back.set_icon_name("go-previous-symbolic");
        back.add_css_class("fm-nav-button");
        back.set_tooltip_text(Some("Back (Alt+Left)"));
        back.set_sensitive(false);
        {
            let state = state.clone();
            let file_list = file_list.clone();
            let location_entry = location_entry.clone();
            let back_btn = back_btn.clone();
            let forward_btn = forward_btn.clone();
            let up_btn = up_btn.clone();
            back.connect_clicked(move |_| {
                Self::go_back_static(&state, &file_list, &location_entry, &back_btn, &forward_btn, &up_btn);
            });
        }
        *back_btn.borrow_mut() = Some(back.clone());
        nav_box.append(&back);

        let forward = Button::new();
        forward.set_icon_name("go-next-symbolic");
        forward.add_css_class("fm-nav-button");
        forward.set_tooltip_text(Some("Forward (Alt+Right)"));
        forward.set_sensitive(false);
        {
            let state = state.clone();
            let file_list = file_list.clone();
            let location_entry = location_entry.clone();
            let back_btn = back_btn.clone();
            let forward_btn = forward_btn.clone();
            let up_btn = up_btn.clone();
            forward.connect_clicked(move |_| {
                Self::go_forward_static(&state, &file_list, &location_entry, &back_btn, &forward_btn, &up_btn);
            });
        }
        *forward_btn.borrow_mut() = Some(forward.clone());
        nav_box.append(&forward);

        let up = Button::new();
        up.set_icon_name("go-up-symbolic");
        up.add_css_class("fm-nav-button");
        up.set_tooltip_text(Some("Parent Directory (Alt+Up)"));
        {
            let state = state.clone();
            let file_list = file_list.clone();
            let location_entry = location_entry.clone();
            let back_btn = back_btn.clone();
            let forward_btn = forward_btn.clone();
            let up_btn = up_btn.clone();
            up.connect_clicked(move |_| {
                Self::go_up_static(&state, &file_list, &location_entry, &back_btn, &forward_btn, &up_btn);
            });
        }
        *up_btn.borrow_mut() = Some(up.clone());
        nav_box.append(&up);

        let home = Button::new();
        home.set_icon_name("go-home-symbolic");
        home.add_css_class("fm-nav-button");
        home.set_tooltip_text(Some("Home (Alt+Home)"));
        {
            let state = state.clone();
            let file_list = file_list.clone();
            let location_entry = location_entry.clone();
            let back_btn = back_btn.clone();
            let forward_btn = forward_btn.clone();
            let up_btn = up_btn.clone();
            home.connect_clicked(move |_| {
                Self::go_home_static(&state, &file_list, &location_entry, &back_btn, &forward_btn, &up_btn);
            });
        }
        nav_box.append(&home);

        header.append(&nav_box);

        // Location bar
        let loc_entry = Entry::new();
        loc_entry.add_css_class("fm-location-bar");
        loc_entry.set_hexpand(true);
        loc_entry.set_placeholder_text(Some("Enter path..."));
        {
            let state = state.clone();
            let file_list = file_list.clone();
            let location_entry_ref = location_entry.clone();
            let back_btn = back_btn.clone();
            let forward_btn = forward_btn.clone();
            let up_btn = up_btn.clone();
            loc_entry.connect_activate(move |entry| {
                let path = PathBuf::from(entry.text().as_str());
                if path.exists() && path.is_dir() {
                    Self::navigate_to_static(&state, &file_list, &location_entry_ref, &back_btn, &forward_btn, &up_btn, &path);
                }
            });
        }
        *location_entry.borrow_mut() = Some(loc_entry.clone());
        header.append(&loc_entry);

        // Search entry
        let search = Entry::new();
        search.add_css_class("fm-search-entry");
        search.set_placeholder_text(Some("Search files..."));
        search.set_width_request(200);
        let search_icon = Image::from_icon_name("system-search-symbolic");
        search.set_primary_icon_paintable(search_icon.paintable().as_ref());
        {
            let state = state.clone();
            let file_list = file_list.clone();
            search.connect_changed(move |entry| {
                let query = entry.text().to_string();
                Self::filter_files_static(&state, &file_list, &query);
            });
        }
        header.append(&search);

        // Action buttons
        let action_box = GtkBox::new(GtkOrientation::Horizontal, 4);

        let refresh_btn = Button::new();
        refresh_btn.set_icon_name("view-refresh-symbolic");
        refresh_btn.add_css_class("fm-nav-button");
        refresh_btn.set_tooltip_text(Some("Refresh (F5)"));
        {
            let state = state.clone();
            let file_list = file_list.clone();
            refresh_btn.connect_clicked(move |_| {
                let path = state.borrow().current_path.clone();
                Self::load_directory_static(&state, &file_list, &path);
            });
        }
        action_box.append(&refresh_btn);

        let hidden_btn = Button::new();
        hidden_btn.set_icon_name("view-more-symbolic");
        hidden_btn.add_css_class("fm-nav-button");
        hidden_btn.set_tooltip_text(Some("Toggle Hidden Files (Ctrl+H)"));
        {
            let state = state.clone();
            let file_list = file_list.clone();
            hidden_btn.connect_clicked(move |_| {
                {
                    let mut s = state.borrow_mut();
                    s.show_hidden = !s.show_hidden;
                }
                let path = state.borrow().current_path.clone();
                Self::load_directory_static(&state, &file_list, &path);
            });
        }
        action_box.append(&hidden_btn);

        let preview_btn = Button::new();
        preview_btn.set_icon_name("view-dual-symbolic");
        preview_btn.add_css_class("fm-nav-button");
        preview_btn.set_tooltip_text(Some("Toggle Preview (Ctrl+P)"));
        {
            let state = state.clone();
            let preview_pane = preview_pane.clone();
            preview_btn.connect_clicked(move |_| {
                let show = {
                    let mut s = state.borrow_mut();
                    s.show_preview = !s.show_preview;
                    s.show_preview
                };
                if let Some(pane) = preview_pane.borrow().as_ref() {
                    pane.set_visible(show);
                }
            });
        }
        action_box.append(&preview_btn);

        header.append(&action_box);

        header
    }

    fn build_sidebar(
        &self,
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        location_entry: &Rc<RefCell<Option<Entry>>>,
        status_label: &Rc<RefCell<Option<Label>>>,
        status_right: &Rc<RefCell<Option<Label>>>,
        back_btn: &Rc<RefCell<Option<Button>>>,
        forward_btn: &Rc<RefCell<Option<Button>>>,
        up_btn: &Rc<RefCell<Option<Button>>>,
    ) -> GtkBox {
        let sidebar = GtkBox::new(GtkOrientation::Vertical, 0);
        sidebar.add_css_class("fm-sidebar");
        sidebar.set_size_request(SIDEBAR_WIDTH, -1);

        let scroll = ScrolledWindow::new();
        scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        scroll.set_vexpand(true);

        let content = GtkBox::new(GtkOrientation::Vertical, 0);

        let places_label = Label::new(Some("Places"));
        places_label.add_css_class("fm-sidebar-section");
        places_label.set_halign(gtk4::Align::Start);
        places_label.set_margin_start(12);
        places_label.set_margin_top(8);
        places_label.set_margin_bottom(4);
        content.append(&places_label);

        let list = ListBox::new();
        list.add_css_class("fm-sidebar-list");
        list.set_selection_mode(gtk4::SelectionMode::Single);

        let bookmarks = state.borrow().bookmarks.clone();
        for bookmark in &bookmarks {
            let row = self.create_sidebar_row(&bookmark.name, &bookmark.icon);
            list.append(&row);
        }

        {
            let state = state.clone();
            let file_list = file_list.clone();
            let location_entry = location_entry.clone();
            let status_label = status_label.clone();
            let status_right = status_right.clone();
            let back_btn = back_btn.clone();
            let forward_btn = forward_btn.clone();
            let up_btn = up_btn.clone();
            let bookmarks = bookmarks.clone();
            list.connect_row_activated(move |_, row| {
                let idx = row.index() as usize;
                if idx < bookmarks.len() {
                    let path = bookmarks[idx].path.clone();
                    if path.exists() {
                        Self::navigate_to_static(&state, &file_list, &location_entry, &back_btn, &forward_btn, &up_btn, &path);
                        Self::update_status_bar_static(&state, &status_label, &status_right);
                    }
                }
            });
        }

        content.append(&list);
        scroll.set_child(Some(&content));
        sidebar.append(&scroll);

        sidebar
    }

    fn create_sidebar_row(&self, name: &str, icon_name: &str) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.add_css_class("fm-sidebar-row");

        let hbox = GtkBox::new(GtkOrientation::Horizontal, 8);
        hbox.set_margin_start(12);
        hbox.set_margin_end(12);
        hbox.set_margin_top(8);
        hbox.set_margin_bottom(8);

        let icon = Image::from_icon_name(icon_name);
        icon.set_pixel_size(16);
        icon.add_css_class("fm-sidebar-icon");
        hbox.append(&icon);

        let label = Label::new(Some(name));
        label.add_css_class("fm-sidebar-label");
        label.set_halign(gtk4::Align::Start);
        hbox.append(&label);

        row.set_child(Some(&hbox));
        row
    }

    fn build_file_area(
        &self,
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        status_label: &Rc<RefCell<Option<Label>>>,
        status_right: &Rc<RefCell<Option<Label>>>,
        preview_pane: &Rc<RefCell<Option<GtkBox>>>,
    ) -> GtkBox {
        let file_area = GtkBox::new(GtkOrientation::Vertical, 0);
        file_area.add_css_class("fm-file-area");
        file_area.set_hexpand(true);
        file_area.set_vexpand(true);

        let scroll = ScrolledWindow::new();
        scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Automatic);
        scroll.set_vexpand(true);

        let list = ListBox::new();
        list.add_css_class("fm-file-list");
        list.set_selection_mode(gtk4::SelectionMode::Multiple);
        list.set_activate_on_single_click(false);

        {
            let state_clone = state.clone();
            let file_list_clone = file_list.clone();
            let status_label = status_label.clone();
            let status_right = status_right.clone();
            list.connect_row_activated(move |_, row| {
                let idx = row.index() as usize;
                let s = state_clone.borrow();
                if idx < s.current_files.len() {
                    let entry = s.current_files[idx].clone();
                    drop(s);
                    if entry.is_dir {
                        Self::navigate_to_static_simple(&state_clone, &file_list_clone, &status_label, &status_right, &entry.path);
                    } else {
                        Self::open_file(&entry);
                    }
                }
            });
        }

        {
            let state = state.clone();
            let status_label = status_label.clone();
            let preview_pane = preview_pane.clone();
            list.connect_selected_rows_changed(move |list| {
                let mut selected = Vec::new();
                for row in list.selected_rows() {
                    let idx = row.index() as usize;
                    let s = state.borrow();
                    if idx < s.current_files.len() {
                        selected.push(s.current_files[idx].clone());
                    }
                }
                {
                    let mut s = state.borrow_mut();
                    s.selected_files = selected.clone();
                }
                // Update status with selection info
                if !selected.is_empty() {
                    let total_size: u64 = selected.iter().map(|f| f.size).sum();
                    let text = format!(
                        "{} selected ({})",
                        types::pluralize(selected.len(), "item", "items"),
                        types::humanize_size(total_size)
                    );
                    if let Some(label) = status_label.borrow().as_ref() {
                        label.set_text(&text);
                    }
                }
                // Update preview for single selection
                if selected.len() == 1 {
                    Self::update_preview_static(&preview_pane, &selected[0]);
                }
            });
        }

        *file_list.borrow_mut() = Some(list.clone());
        scroll.set_child(Some(&list));
        file_area.append(&scroll);

        file_area
    }

    fn build_preview_pane(
        &self,
        state: &Rc<RefCell<FileManagerState>>,
        preview_pane: &Rc<RefCell<Option<GtkBox>>>,
    ) -> GtkBox {
        let pane = GtkBox::new(GtkOrientation::Vertical, 0);
        pane.add_css_class("fm-preview-pane");
        pane.set_size_request(PREVIEW_WIDTH, -1);

        let header = GtkBox::new(GtkOrientation::Horizontal, 8);
        header.add_css_class("fm-preview-header");
        header.set_margin_start(12);
        header.set_margin_end(12);
        header.set_margin_top(8);
        header.set_margin_bottom(8);

        let title = Label::new(Some("Preview"));
        title.add_css_class("fm-preview-title");
        title.set_halign(gtk4::Align::Start);
        header.append(&title);

        pane.append(&header);

        let scroll = ScrolledWindow::new();
        scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Automatic);
        scroll.set_vexpand(true);

        let content = GtkBox::new(GtkOrientation::Vertical, 12);
        content.add_css_class("fm-preview-content");
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);

        let placeholder = Label::new(Some("Select a file to preview"));
        placeholder.add_css_class("fm-preview-placeholder");
        content.append(&placeholder);

        scroll.set_child(Some(&content));
        pane.append(&scroll);

        let show = state.borrow().show_preview;
        pane.set_visible(show);
        *preview_pane.borrow_mut() = Some(pane.clone());

        pane
    }

    fn build_status_bar(
        &self,
        status_label: &Rc<RefCell<Option<Label>>>,
        status_right: &Rc<RefCell<Option<Label>>>,
    ) -> GtkBox {
        let status = GtkBox::new(GtkOrientation::Horizontal, 8);
        status.add_css_class("fm-status-bar");
        status.set_margin_start(12);
        status.set_margin_end(12);
        status.set_margin_top(4);
        status.set_margin_bottom(4);

        let left = Label::new(Some(""));
        left.add_css_class("fm-status-text");
        left.set_halign(gtk4::Align::Start);
        *status_label.borrow_mut() = Some(left.clone());
        status.append(&left);

        let spacer = GtkBox::new(GtkOrientation::Horizontal, 0);
        spacer.set_hexpand(true);
        status.append(&spacer);

        let right = Label::new(Some(""));
        right.add_css_class("fm-status-text-right");
        right.set_halign(gtk4::Align::End);
        *status_right.borrow_mut() = Some(right.clone());
        status.append(&right);

        status
    }

    fn setup_keyboard(
        &self,
        window: &ApplicationWindow,
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        location_entry: &Rc<RefCell<Option<Entry>>>,
        status_label: &Rc<RefCell<Option<Label>>>,
        status_right: &Rc<RefCell<Option<Label>>>,
        preview_pane: &Rc<RefCell<Option<GtkBox>>>,
        back_btn: &Rc<RefCell<Option<Button>>>,
        forward_btn: &Rc<RefCell<Option<Button>>>,
        up_btn: &Rc<RefCell<Option<Button>>>,
    ) {
        let key_controller = gtk4::EventControllerKey::new();

        let state = state.clone();
        let file_list = file_list.clone();
        let location_entry = location_entry.clone();
        let status_label = status_label.clone();
        let status_right = status_right.clone();
        let preview_pane = preview_pane.clone();
        let back_btn = back_btn.clone();
        let forward_btn = forward_btn.clone();
        let up_btn = up_btn.clone();

        key_controller.connect_key_pressed(move |_, key, _, modifier| {
            let ctrl = modifier.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
            let alt = modifier.contains(gtk4::gdk::ModifierType::ALT_MASK);
            let shift = modifier.contains(gtk4::gdk::ModifierType::SHIFT_MASK);

            match key {
                gtk4::gdk::Key::h if ctrl => {
                    {
                        let mut s = state.borrow_mut();
                        s.show_hidden = !s.show_hidden;
                    }
                    let path = state.borrow().current_path.clone();
                    Self::load_directory_static(&state, &file_list, &path);
                    glib::Propagation::Stop
                }
                gtk4::gdk::Key::l if ctrl => {
                    if let Some(entry) = location_entry.borrow().as_ref() {
                        entry.grab_focus();
                        entry.select_region(0, -1);
                    }
                    glib::Propagation::Stop
                }
                gtk4::gdk::Key::p if ctrl => {
                    let show = {
                        let mut s = state.borrow_mut();
                        s.show_preview = !s.show_preview;
                        s.show_preview
                    };
                    if let Some(pane) = preview_pane.borrow().as_ref() {
                        pane.set_visible(show);
                    }
                    glib::Propagation::Stop
                }
                gtk4::gdk::Key::c if ctrl => {
                    Self::copy_selected_static(&state);
                    glib::Propagation::Stop
                }
                gtk4::gdk::Key::x if ctrl => {
                    Self::cut_selected_static(&state);
                    glib::Propagation::Stop
                }
                gtk4::gdk::Key::v if ctrl => {
                    Self::paste_static(&state, &file_list, &status_label, &status_right);
                    glib::Propagation::Stop
                }
                gtk4::gdk::Key::n if ctrl && shift => {
                    Self::show_new_folder_dialog_static(&state, &file_list, &status_label, &status_right);
                    glib::Propagation::Stop
                }
                gtk4::gdk::Key::a if ctrl => {
                    if let Some(list) = file_list.borrow().as_ref() {
                        list.select_all();
                    }
                    glib::Propagation::Stop
                }
                gtk4::gdk::Key::Delete => {
                    if shift {
                        Self::permanent_delete_static(&state, &file_list, &status_label, &status_right);
                    } else {
                        Self::trash_selected_static(&state, &file_list, &status_label, &status_right);
                    }
                    glib::Propagation::Stop
                }
                gtk4::gdk::Key::F2 => {
                    Self::show_rename_dialog_static(&state, &file_list, &status_label, &status_right);
                    glib::Propagation::Stop
                }
                gtk4::gdk::Key::F5 => {
                    let path = state.borrow().current_path.clone();
                    Self::load_directory_static(&state, &file_list, &path);
                    glib::Propagation::Stop
                }
                gtk4::gdk::Key::BackSpace => {
                    Self::go_back_static(&state, &file_list, &location_entry, &back_btn, &forward_btn, &up_btn);
                    glib::Propagation::Stop
                }
                gtk4::gdk::Key::Left if alt => {
                    Self::go_back_static(&state, &file_list, &location_entry, &back_btn, &forward_btn, &up_btn);
                    glib::Propagation::Stop
                }
                gtk4::gdk::Key::Right if alt => {
                    Self::go_forward_static(&state, &file_list, &location_entry, &back_btn, &forward_btn, &up_btn);
                    glib::Propagation::Stop
                }
                gtk4::gdk::Key::Up if alt => {
                    Self::go_up_static(&state, &file_list, &location_entry, &back_btn, &forward_btn, &up_btn);
                    glib::Propagation::Stop
                }
                gtk4::gdk::Key::Home if alt => {
                    Self::go_home_static(&state, &file_list, &location_entry, &back_btn, &forward_btn, &up_btn);
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });

        window.add_controller(key_controller);
    }

    fn load_directory(
        &self,
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        status_label: &Rc<RefCell<Option<Label>>>,
        status_right: &Rc<RefCell<Option<Label>>>,
        path: &PathBuf,
    ) {
        Self::load_directory_static(state, file_list, path);
        Self::update_status_bar_static(state, status_label, status_right);
    }

    fn load_directory_static(
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        path: &PathBuf,
    ) {
        let show_hidden = state.borrow().show_hidden;
        let entries = types::read_directory(path, show_hidden);

        {
            let mut s = state.borrow_mut();
            s.current_files = entries.clone();
            s.selected_files.clear();
        }

        if let Some(list) = file_list.borrow().as_ref() {
            // Clear existing rows
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }

            // Add new rows
            for entry in &entries {
                let row = Self::create_file_row(entry);
                list.append(&row);
            }
        }
    }

    fn create_file_row(entry: &FileEntry) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.add_css_class("fm-file-row");
        row.set_activatable(true);

        let hbox = GtkBox::new(GtkOrientation::Horizontal, 8);
        hbox.set_margin_start(8);
        hbox.set_margin_end(8);
        hbox.set_margin_top(6);
        hbox.set_margin_bottom(6);

        let icon_name = types::get_file_icon(entry);
        let icon = Image::from_icon_name(icon_name);
        icon.set_pixel_size(20);
        if entry.is_dir {
            icon.add_css_class("fm-file-icon-folder");
        }
        hbox.append(&icon);

        let name = Label::new(Some(&entry.name));
        name.add_css_class("fm-file-name");
        if entry.is_dir {
            name.add_css_class("fm-file-name-folder");
        }
        if entry.is_hidden {
            name.set_opacity(0.6);
        }
        name.set_halign(gtk4::Align::Start);
        name.set_hexpand(true);
        name.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        name.set_max_width_chars(50);
        hbox.append(&name);

        if !entry.is_dir {
            let size = Label::new(Some(&types::humanize_size(entry.size)));
            size.add_css_class("fm-file-size");
            size.set_width_chars(10);
            hbox.append(&size);
        }

        let date = Label::new(Some(&types::format_date(entry.mod_time)));
        date.add_css_class("fm-file-date");
        date.set_width_chars(16);
        hbox.append(&date);

        row.set_child(Some(&hbox));
        row
    }

    fn update_status_bar_static(
        state: &Rc<RefCell<FileManagerState>>,
        status_label: &Rc<RefCell<Option<Label>>>,
        status_right: &Rc<RefCell<Option<Label>>>,
    ) {
        let s = state.borrow();
        let mut dir_count = 0;
        let mut file_count = 0;

        for entry in &s.current_files {
            if entry.is_dir {
                dir_count += 1;
            } else {
                file_count += 1;
            }
        }

        let status_text = if dir_count > 0 && file_count > 0 {
            format!(
                "{}, {}",
                types::pluralize(dir_count, "folder", "folders"),
                types::pluralize(file_count, "file", "files")
            )
        } else if dir_count > 0 {
            types::pluralize(dir_count, "folder", "folders")
        } else if file_count > 0 {
            types::pluralize(file_count, "file", "files")
        } else {
            "Empty folder".to_string()
        };

        if let Some(label) = status_label.borrow().as_ref() {
            label.set_text(&status_text);
        }

        // Disk space
        let (free, total) = types::get_disk_space(&s.current_path);
        if total > 0 {
            let space_text = format!(
                "{} free of {}",
                types::humanize_size(free),
                types::humanize_size(total)
            );
            if let Some(label) = status_right.borrow().as_ref() {
                label.set_text(&space_text);
            }
        }
    }

    fn navigate_to_static(
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        location_entry: &Rc<RefCell<Option<Entry>>>,
        back_btn: &Rc<RefCell<Option<Button>>>,
        forward_btn: &Rc<RefCell<Option<Button>>>,
        up_btn: &Rc<RefCell<Option<Button>>>,
        path: &PathBuf,
    ) {
        {
            let mut s = state.borrow_mut();
            s.history.push(path.clone());
            s.current_path = path.clone();
        }
        Self::load_directory_static(state, file_list, path);
        Self::update_location_bar_static(state, location_entry);
        Self::update_nav_buttons_static(state, back_btn, forward_btn, up_btn);
    }

    fn navigate_to_static_simple(
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        status_label: &Rc<RefCell<Option<Label>>>,
        status_right: &Rc<RefCell<Option<Label>>>,
        path: &PathBuf,
    ) {
        {
            let mut s = state.borrow_mut();
            s.history.push(path.clone());
            s.current_path = path.clone();
        }
        Self::load_directory_static(state, file_list, path);
        Self::update_status_bar_static(state, status_label, status_right);
    }

    fn update_location_bar_static(
        state: &Rc<RefCell<FileManagerState>>,
        location_entry: &Rc<RefCell<Option<Entry>>>,
    ) {
        let path = state.borrow().current_path.display().to_string();
        if let Some(entry) = location_entry.borrow().as_ref() {
            entry.set_text(&path);
        }
    }

    fn update_nav_buttons_static(
        state: &Rc<RefCell<FileManagerState>>,
        back_btn: &Rc<RefCell<Option<Button>>>,
        forward_btn: &Rc<RefCell<Option<Button>>>,
        up_btn: &Rc<RefCell<Option<Button>>>,
    ) {
        let s = state.borrow();
        if let Some(btn) = back_btn.borrow().as_ref() {
            btn.set_sensitive(s.history.can_go_back());
        }
        if let Some(btn) = forward_btn.borrow().as_ref() {
            btn.set_sensitive(s.history.can_go_forward());
        }
        if let Some(btn) = up_btn.borrow().as_ref() {
            btn.set_sensitive(s.current_path.parent().is_some());
        }
    }

    fn go_back_static(
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        location_entry: &Rc<RefCell<Option<Entry>>>,
        back_btn: &Rc<RefCell<Option<Button>>>,
        forward_btn: &Rc<RefCell<Option<Button>>>,
        up_btn: &Rc<RefCell<Option<Button>>>,
    ) {
        let path = {
            let mut s = state.borrow_mut();
            s.history.back()
        };
        if let Some(path) = path {
            state.borrow_mut().current_path = path.clone();
            Self::load_directory_static(state, file_list, &path);
            Self::update_location_bar_static(state, location_entry);
            Self::update_nav_buttons_static(state, back_btn, forward_btn, up_btn);
        }
    }

    fn go_forward_static(
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        location_entry: &Rc<RefCell<Option<Entry>>>,
        back_btn: &Rc<RefCell<Option<Button>>>,
        forward_btn: &Rc<RefCell<Option<Button>>>,
        up_btn: &Rc<RefCell<Option<Button>>>,
    ) {
        let path = {
            let mut s = state.borrow_mut();
            s.history.forward()
        };
        if let Some(path) = path {
            state.borrow_mut().current_path = path.clone();
            Self::load_directory_static(state, file_list, &path);
            Self::update_location_bar_static(state, location_entry);
            Self::update_nav_buttons_static(state, back_btn, forward_btn, up_btn);
        }
    }

    fn go_up_static(
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        location_entry: &Rc<RefCell<Option<Entry>>>,
        back_btn: &Rc<RefCell<Option<Button>>>,
        forward_btn: &Rc<RefCell<Option<Button>>>,
        up_btn: &Rc<RefCell<Option<Button>>>,
    ) {
        let parent = {
            let s = state.borrow();
            types::get_parent_path(&s.current_path)
        };
        if parent != state.borrow().current_path {
            Self::navigate_to_static(state, file_list, location_entry, back_btn, forward_btn, up_btn, &parent);
        }
    }

    fn go_home_static(
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        location_entry: &Rc<RefCell<Option<Entry>>>,
        back_btn: &Rc<RefCell<Option<Button>>>,
        forward_btn: &Rc<RefCell<Option<Button>>>,
        up_btn: &Rc<RefCell<Option<Button>>>,
    ) {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self::navigate_to_static(state, file_list, location_entry, back_btn, forward_btn, up_btn, &home);
    }

    fn filter_files_static(
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        query: &str,
    ) {
        if query.is_empty() {
            let path = state.borrow().current_path.clone();
            Self::load_directory_static(state, file_list, &path);
            return;
        }

        let query_lower = query.to_lowercase();
        let show_hidden = state.borrow().show_hidden;
        let path = state.borrow().current_path.clone();
        let all_entries = types::read_directory(&path, show_hidden);

        let filtered: Vec<FileEntry> = all_entries
            .into_iter()
            .filter(|e| e.name.to_lowercase().contains(&query_lower))
            .collect();

        {
            let mut s = state.borrow_mut();
            s.current_files = filtered.clone();
        }

        if let Some(list) = file_list.borrow().as_ref() {
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            for entry in &filtered {
                let row = Self::create_file_row(entry);
                list.append(&row);
            }
        }
    }

    fn open_file(entry: &FileEntry) {
        let path = entry.path.display().to_string();
        let _ = std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn();
    }

    fn copy_selected_static(state: &Rc<RefCell<FileManagerState>>) {
        let mut s = state.borrow_mut();
        let files: Vec<PathBuf> = s.selected_files.iter().map(|f| f.path.clone()).collect();
        if !files.is_empty() {
            s.clipboard.copy(files);
        }
    }

    fn cut_selected_static(state: &Rc<RefCell<FileManagerState>>) {
        let mut s = state.borrow_mut();
        let files: Vec<PathBuf> = s.selected_files.iter().map(|f| f.path.clone()).collect();
        if !files.is_empty() {
            s.clipboard.cut(files);
        }
    }

    fn paste_static(
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        status_label: &Rc<RefCell<Option<Label>>>,
        status_right: &Rc<RefCell<Option<Label>>>,
    ) {
        let dest = state.borrow().current_path.clone();
        {
            let mut s = state.borrow_mut();
            if s.clipboard.has_files() {
                let _ = s.clipboard.paste(&dest);
            }
        }
        Self::load_directory_static(state, file_list, &dest);
        Self::update_status_bar_static(state, status_label, status_right);
    }

    fn trash_selected_static(
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        status_label: &Rc<RefCell<Option<Label>>>,
        status_right: &Rc<RefCell<Option<Label>>>,
    ) {
        let files: Vec<PathBuf> = state.borrow().selected_files.iter().map(|f| f.path.clone()).collect();
        if !files.is_empty() {
            let _ = super::clipboard::trash_files(&files);
            let path = state.borrow().current_path.clone();
            Self::load_directory_static(state, file_list, &path);
            Self::update_status_bar_static(state, status_label, status_right);
        }
    }

    fn permanent_delete_static(
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        status_label: &Rc<RefCell<Option<Label>>>,
        status_right: &Rc<RefCell<Option<Label>>>,
    ) {
        let files: Vec<PathBuf> = state.borrow().selected_files.iter().map(|f| f.path.clone()).collect();
        if !files.is_empty() {
            let _ = super::clipboard::delete_files(&files);
            let path = state.borrow().current_path.clone();
            Self::load_directory_static(state, file_list, &path);
            Self::update_status_bar_static(state, status_label, status_right);
        }
    }

    fn update_preview_static(preview_pane: &Rc<RefCell<Option<GtkBox>>>, entry: &FileEntry) {
        if let Some(pane) = preview_pane.borrow().as_ref() {
            // Find the scroll window child and its content
            if let Some(scroll) = pane.last_child() {
                if let Some(scroll_win) = scroll.downcast_ref::<ScrolledWindow>() {
                    if let Some(content) = scroll_win.child() {
                        if let Some(content_box) = content.downcast_ref::<GtkBox>() {
                            // Clear existing content
                            while let Some(child) = content_box.first_child() {
                                content_box.remove(&child);
                            }

                            // Add icon
                            let icon_name = types::get_file_icon(entry);
                            let icon = Image::from_icon_name(icon_name);
                            icon.set_pixel_size(64);
                            icon.set_margin_bottom(12);
                            content_box.append(&icon);

                            // Add name
                            let name = Label::new(Some(&entry.name));
                            name.add_css_class("fm-preview-name");
                            name.set_wrap(true);
                            name.set_margin_bottom(12);
                            content_box.append(&name);

                            // Add details
                            let details_box = GtkBox::new(GtkOrientation::Vertical, 4);

                            let type_label = Label::new(Some(&format!(
                                "Type: {}",
                                if entry.is_dir { "Folder" } else { "File" }
                            )));
                            type_label.add_css_class("fm-preview-detail");
                            type_label.set_halign(gtk4::Align::Start);
                            details_box.append(&type_label);

                            if !entry.is_dir {
                                let size_label = Label::new(Some(&format!(
                                    "Size: {}",
                                    types::humanize_size(entry.size)
                                )));
                                size_label.add_css_class("fm-preview-detail");
                                size_label.set_halign(gtk4::Align::Start);
                                details_box.append(&size_label);
                            }

                            let date_label = Label::new(Some(&format!(
                                "Modified: {}",
                                types::format_date(entry.mod_time)
                            )));
                            date_label.add_css_class("fm-preview-detail");
                            date_label.set_halign(gtk4::Align::Start);
                            details_box.append(&date_label);

                            let path_label = Label::new(Some(&format!(
                                "Location: {}",
                                entry.path.parent().map(|p| p.display().to_string()).unwrap_or_default()
                            )));
                            path_label.add_css_class("fm-preview-detail");
                            path_label.set_halign(gtk4::Align::Start);
                            path_label.set_wrap(true);
                            details_box.append(&path_label);

                            content_box.append(&details_box);
                        }
                    }
                }
            }
        }
    }

    fn show_new_folder_dialog_static(
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        status_label: &Rc<RefCell<Option<Label>>>,
        status_right: &Rc<RefCell<Option<Label>>>,
    ) {
        let current_path = state.borrow().current_path.clone();
        let state = state.clone();
        let file_list = file_list.clone();
        let status_label = status_label.clone();
        let status_right = status_right.clone();

        glib::idle_add_local_once(move || {
            let dialog = gtk4::Dialog::new();
            dialog.set_title(Some("New Folder"));
            dialog.set_modal(true);
            dialog.set_default_width(400);

            let content = dialog.content_area();
            content.set_margin_top(16);
            content.set_margin_bottom(16);
            content.set_margin_start(16);
            content.set_margin_end(16);
            content.set_spacing(12);

            let label = Label::new(Some("Folder name:"));
            label.set_halign(gtk4::Align::Start);
            content.append(&label);

            let entry = Entry::new();
            entry.set_placeholder_text(Some("Enter folder name..."));
            entry.set_text("New Folder");
            entry.select_region(0, -1);
            content.append(&entry);

            let button_box = GtkBox::new(GtkOrientation::Horizontal, 8);
            button_box.set_halign(gtk4::Align::End);
            button_box.set_margin_top(16);

            let cancel_btn = Button::with_label("Cancel");
            cancel_btn.add_css_class("cancel");
            {
                let dialog = dialog.clone();
                cancel_btn.connect_clicked(move |_| {
                    dialog.close();
                });
            }
            button_box.append(&cancel_btn);

            let create_btn = Button::with_label("Create");
            {
                let dialog = dialog.clone();
                let entry = entry.clone();
                let current_path = current_path.clone();
                let state = state.clone();
                let file_list = file_list.clone();
                let status_label = status_label.clone();
                let status_right = status_right.clone();
                create_btn.connect_clicked(move |_| {
                    let name = entry.text().to_string();
                    if !name.is_empty() {
                        let path = current_path.join(&name);
                        let _ = std::fs::create_dir(&path);
                        Self::load_directory_static(&state, &file_list, &current_path);
                        Self::update_status_bar_static(&state, &status_label, &status_right);
                    }
                    dialog.close();
                });
            }
            button_box.append(&create_btn);

            content.append(&button_box);

            {
                let dialog = dialog.clone();
                let current_path = current_path.clone();
                let state = state.clone();
                let file_list = file_list.clone();
                let status_label = status_label.clone();
                let status_right = status_right.clone();
                entry.connect_activate(move |entry| {
                    let name = entry.text().to_string();
                    if !name.is_empty() {
                        let path = current_path.join(&name);
                        let _ = std::fs::create_dir(&path);
                        Self::load_directory_static(&state, &file_list, &current_path);
                        Self::update_status_bar_static(&state, &status_label, &status_right);
                    }
                    dialog.close();
                });
            }

            dialog.present();
            entry.grab_focus();
        });
    }

    fn show_rename_dialog_static(
        state: &Rc<RefCell<FileManagerState>>,
        file_list: &Rc<RefCell<Option<ListBox>>>,
        status_label: &Rc<RefCell<Option<Label>>>,
        status_right: &Rc<RefCell<Option<Label>>>,
    ) {
        let selected = state.borrow().selected_files.clone();
        if selected.len() != 1 {
            return;
        }
        let file = selected[0].clone();
        let current_path = state.borrow().current_path.clone();
        let state = state.clone();
        let file_list = file_list.clone();
        let status_label = status_label.clone();
        let status_right = status_right.clone();

        glib::idle_add_local_once(move || {
            let dialog = gtk4::Dialog::new();
            dialog.set_title(Some("Rename"));
            dialog.set_modal(true);
            dialog.set_default_width(400);

            let content = dialog.content_area();
            content.set_margin_top(16);
            content.set_margin_bottom(16);
            content.set_margin_start(16);
            content.set_margin_end(16);
            content.set_spacing(12);

            let label = Label::new(Some("New name:"));
            label.set_halign(gtk4::Align::Start);
            content.append(&label);

            let entry = Entry::new();
            entry.set_text(&file.name);
            // Select name without extension for files
            if !file.is_dir {
                if let Some(stem) = file.path.file_stem().and_then(|s| s.to_str()) {
                    entry.select_region(0, stem.len() as i32);
                }
            } else {
                entry.select_region(0, -1);
            }
            content.append(&entry);

            let button_box = GtkBox::new(GtkOrientation::Horizontal, 8);
            button_box.set_halign(gtk4::Align::End);
            button_box.set_margin_top(16);

            let cancel_btn = Button::with_label("Cancel");
            cancel_btn.add_css_class("cancel");
            {
                let dialog = dialog.clone();
                cancel_btn.connect_clicked(move |_| {
                    dialog.close();
                });
            }
            button_box.append(&cancel_btn);

            let rename_btn = Button::with_label("Rename");
            {
                let dialog = dialog.clone();
                let entry = entry.clone();
                let file = file.clone();
                let current_path = current_path.clone();
                let state = state.clone();
                let file_list = file_list.clone();
                let status_label = status_label.clone();
                let status_right = status_right.clone();
                rename_btn.connect_clicked(move |_| {
                    let new_name = entry.text().to_string();
                    if !new_name.is_empty() && new_name != file.name {
                        let new_path = file.path.parent()
                            .map(|p| p.join(&new_name))
                            .unwrap_or_else(|| PathBuf::from(&new_name));
                        let _ = std::fs::rename(&file.path, &new_path);
                        Self::load_directory_static(&state, &file_list, &current_path);
                        Self::update_status_bar_static(&state, &status_label, &status_right);
                    }
                    dialog.close();
                });
            }
            button_box.append(&rename_btn);

            content.append(&button_box);

            {
                let dialog = dialog.clone();
                let file = file.clone();
                let current_path = current_path.clone();
                let state = state.clone();
                let file_list = file_list.clone();
                let status_label = status_label.clone();
                let status_right = status_right.clone();
                entry.connect_activate(move |entry| {
                    let new_name = entry.text().to_string();
                    if !new_name.is_empty() && new_name != file.name {
                        let new_path = file.path.parent()
                            .map(|p| p.join(&new_name))
                            .unwrap_or_else(|| PathBuf::from(&new_name));
                        let _ = std::fs::rename(&file.path, &new_path);
                        Self::load_directory_static(&state, &file_list, &current_path);
                        Self::update_status_bar_static(&state, &status_label, &status_right);
                    }
                    dialog.close();
                });
            }

            dialog.present();
            entry.grab_focus();
        });
    }
}

impl Default for FileManagerComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for FileManagerComponent {
    fn id(&self) -> ComponentId {
        ComponentId::FileManager
    }

    fn init(&mut self, ctx: ComponentContext) {
        if self.initialized {
            return;
        }

        // File manager uses a regular window, not layer-shell
        let window = ApplicationWindow::builder()
            .application(&ctx.app)
            .title("Raven Files")
            .default_width(WINDOW_WIDTH)
            .default_height(WINDOW_HEIGHT)
            .build();

        let content = self.build_ui(&window);
        window.set_child(Some(&content));

        self.window = Some(window);
        self.ctx = Some(ctx);
        self.initialized = true;
    }

    fn show(&self) {
        if let Some(window) = &self.window {
            window.present();
        }
    }

    fn hide(&self) {
        if let Some(window) = &self.window {
            window.set_visible(false);
        }
    }

    fn is_visible(&self) -> bool {
        self.window.as_ref().map(|w| w.is_visible()).unwrap_or(false)
    }

    fn handle_event(&self, _event: &ShellEvent) {
        // Events handled by daemon
    }

    fn window(&self) -> Option<&gtk4::Window> {
        // ApplicationWindow is not a layer-shell window
        None
    }
}
