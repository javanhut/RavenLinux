use gtk4::gdk::Display;
use gtk4::CssProvider;
use tracing::debug;

/// Unified CSS theme for all Raven Shell components
pub const PANEL_CSS: &str = r#"
/* ========== Global Styles ========== */

window {
    background-color: rgba(30, 30, 30, 0.75);
}

/* ========== Panel Styles ========== */

.panel-container {
    background: linear-gradient(
        to bottom,
        rgba(255, 255, 255, 0.12) 0%,
        rgba(255, 255, 255, 0.05) 50%,
        rgba(0, 0, 0, 0.1) 100%
    );
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    padding: 2px 8px;
}

.panel-section {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 3px 4px;
    margin: 2px 4px;
}

/* ========== Dock Styles ========== */

.dock-container {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 10px;
    padding: 3px 6px;
    margin: 2px 8px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    min-width: 50px;
}

.dock-item {
    background: rgba(255, 255, 255, 0.05);
    border-radius: 8px;
    min-width: 44px;
    padding: 4px 12px;
    margin: 0 2px;
}

.dock-item:hover {
    background: rgba(255, 255, 255, 0.18);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
}

.dock-item:active {
    background: rgba(255, 255, 255, 0.25);
}

.dock-item-running {
    border-bottom: 2px solid rgba(0, 150, 136, 0.9);
}

.dock-item-pinned {
    background: rgba(255, 255, 255, 0.08);
}

.dock-item-minimized {
    opacity: 0.6;
}

.dock-item-focused {
    background: rgba(0, 150, 136, 0.25);
    border-bottom: 2px solid rgba(0, 200, 180, 1.0);
}

/* ========== Button Styles ========== */

button {
    background: transparent;
    border: none;
    border-radius: 6px;
    padding: 4px 14px;
    color: rgba(255, 255, 255, 0.9);
    font-size: 13px;
    font-weight: 500;
    min-height: 26px;
    min-width: 40px;
    transition: all 150ms ease;
}

button:hover {
    background: rgba(255, 255, 255, 0.15);
}

button:active {
    background: rgba(255, 255, 255, 0.25);
    transform: scale(0.97);
}

.start-button {
    background: linear-gradient(
        to bottom,
        rgba(0, 150, 136, 0.9) 0%,
        rgba(0, 130, 118, 0.9) 100%
    );
    font-weight: 600;
    padding: 4px 12px;
    border-radius: 6px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3),
                inset 0 1px 0 rgba(255, 255, 255, 0.2);
}

.start-button:hover {
    background: linear-gradient(
        to bottom,
        rgba(0, 170, 150, 0.95) 0%,
        rgba(0, 150, 136, 0.95) 100%
    );
}

.start-button:active {
    background: linear-gradient(
        to bottom,
        rgba(0, 130, 118, 0.95) 0%,
        rgba(0, 110, 100, 0.95) 100%
    );
}

.power-button {
    color: rgba(255, 120, 120, 0.9);
}

.power-button:hover {
    background: rgba(255, 100, 100, 0.2);
    color: rgba(255, 140, 140, 1);
}

.settings-button {
    color: rgba(200, 200, 200, 0.9);
}

.settings-button:hover {
    background: rgba(255, 255, 255, 0.15);
    color: rgba(255, 255, 255, 1);
}

/* ========== Label Styles ========== */

label {
    color: rgba(255, 255, 255, 0.9);
    font-size: 13px;
    font-weight: 500;
}

.clock {
    font-weight: 600;
    padding: 4px 14px;
    color: rgba(255, 255, 255, 0.95);
    font-size: 13px;
    letter-spacing: 0.3px;
}

/* ========== Separator Styles ========== */

separator {
    background-color: rgba(255, 255, 255, 0.15);
    min-width: 1px;
    margin: 6px 4px;
}

/* ========== Context Menu Styles ========== */

.context-menu {
    background: rgba(40, 40, 40, 0.95);
    border-radius: 8px;
    padding: 4px;
    border: 1px solid rgba(255, 255, 255, 0.1);
}

.context-menu button {
    border-radius: 4px;
    padding: 8px 16px;
    min-width: 120px;
}

.context-menu button:hover {
    background: rgba(255, 255, 255, 0.1);
}

.context-menu-close:hover {
    background: rgba(255, 80, 80, 0.3);
    color: rgba(255, 150, 150, 1);
}

/* ========== Settings Menu Styles ========== */

.settings-menu {
    background: rgba(30, 30, 35, 0.95);
    border-radius: 12px;
    padding: 8px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    min-width: 200px;
}

.settings-menu button {
    border-radius: 6px;
    padding: 10px 16px;
    min-width: 180px;
    margin: 2px 0;
}

.settings-menu button:hover {
    background: rgba(255, 255, 255, 0.1);
}

.settings-section-label {
    color: rgba(150, 150, 150, 0.8);
    font-size: 11px;
    font-weight: 600;
    padding: 8px 16px 4px 16px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
}

.settings-menu-separator {
    background-color: rgba(255, 255, 255, 0.1);
    min-height: 1px;
    margin: 6px 8px;
}

/* ========== Quick Toggle Styles ========== */

.quick-toggle {
    background: rgba(255, 255, 255, 0.05);
    border-radius: 8px;
    padding: 8px 12px;
    margin: 4px 0;
}

.quick-toggle:hover {
    background: rgba(255, 255, 255, 0.12);
}

.quick-toggle-active {
    background: rgba(0, 150, 136, 0.3);
    border: 1px solid rgba(0, 150, 136, 0.5);
}

.quick-toggle-active:hover {
    background: rgba(0, 150, 136, 0.4);
}

/* ========== Overlay/Modal Styles ========== */

.overlay-window {
    background: rgba(25, 25, 28, 0.95);
    border-radius: 16px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    padding: 16px;
}

.overlay-title {
    font-size: 18px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.95);
    margin-bottom: 12px;
}

/* ========== Power Menu Styles ========== */

.power-menu {
    background: rgba(25, 25, 28, 0.95);
    border-radius: 16px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    padding: 24px;
}

.power-menu-button {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    padding: 16px 24px;
    margin: 8px;
    min-width: 100px;
}

.power-menu-button:hover {
    background: rgba(255, 255, 255, 0.15);
}

.power-menu-button.danger {
    background: rgba(255, 80, 80, 0.15);
}

.power-menu-button.danger:hover {
    background: rgba(255, 80, 80, 0.25);
}

/* ========== Menu/Launcher Styles ========== */

.app-menu {
    background: rgba(25, 25, 28, 0.95);
    border-radius: 16px;
    border: 1px solid rgba(255, 255, 255, 0.1);
}

.app-menu-search {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 8px 12px;
    margin: 12px;
    border: none;
    color: rgba(255, 255, 255, 0.9);
}

.app-menu-search:focus {
    background: rgba(255, 255, 255, 0.12);
    border: 1px solid rgba(0, 150, 136, 0.5);
}

.app-category {
    padding: 8px 16px;
    border-radius: 8px;
    color: rgba(255, 255, 255, 0.7);
}

.app-category:hover {
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.9);
}

.app-category-active {
    background: rgba(0, 150, 136, 0.25);
    color: rgba(255, 255, 255, 0.95);
}

.app-item {
    padding: 10px 16px;
    border-radius: 8px;
}

.app-item:hover {
    background: rgba(255, 255, 255, 0.1);
}

/* ========== File Manager Styles ========== */

.file-manager {
    background: rgba(30, 30, 32, 0.98);
}

.file-manager-sidebar {
    background: rgba(25, 25, 28, 0.95);
    border-right: 1px solid rgba(255, 255, 255, 0.08);
    padding: 8px;
}

.file-manager-content {
    background: rgba(35, 35, 38, 0.95);
}

.file-item {
    padding: 8px 12px;
    border-radius: 6px;
}

.file-item:hover {
    background: rgba(255, 255, 255, 0.08);
}

.file-item:selected {
    background: rgba(0, 150, 136, 0.3);
}

/* ========== Settings Styles ========== */

.settings-window {
    background: rgba(25, 25, 28, 0.98);
}

.settings-sidebar {
    background: rgba(20, 20, 22, 0.95);
    border-right: 1px solid rgba(255, 255, 255, 0.08);
    min-width: 200px;
}

.settings-content {
    background: rgba(30, 30, 32, 0.95);
    padding: 24px;
}

.settings-group {
    background: rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    padding: 16px;
    margin-bottom: 16px;
}

.settings-group-title {
    font-size: 14px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.9);
    margin-bottom: 12px;
}

.settings-row {
    padding: 12px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.settings-row:last-child {
    border-bottom: none;
}

/* ========== Icon Styles ========== */

.raven-icon {
    min-width: 24px;
    min-height: 24px;
}

/* ========== Keybindings Overlay ========== */

.keybindings-overlay {
    background: rgba(25, 25, 28, 0.95);
    border-radius: 16px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    padding: 24px;
}

.keybindings-category {
    font-size: 14px;
    font-weight: 600;
    color: rgba(0, 200, 180, 0.9);
    margin: 16px 0 8px 0;
}

.keybinding-row {
    padding: 6px 0;
}

.keybinding-key {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    padding: 4px 8px;
    font-family: monospace;
    color: rgba(255, 255, 255, 0.9);
}

.keybinding-desc {
    color: rgba(255, 255, 255, 0.7);
    margin-left: 12px;
}

/* ========== Desktop Styles ========== */

.desktop-container {
    background: #0b0f14;
}

.desktop-background-fallback {
    background: linear-gradient(
        135deg,
        #0b0f14 0%,
        #1a2332 50%,
        #0b0f14 100%
    );
}

.desktop-wallpaper {
    /* Wallpaper fills entire desktop */
}

.desktop-icon-grid {
    /* FlowBox container for icons */
}

.desktop-icon {
    background: transparent;
    border-radius: 8px;
    padding: 8px;
    min-width: 80px;
    min-height: 80px;
    transition: background 150ms ease;
}

.desktop-icon:hover {
    background: rgba(255, 255, 255, 0.1);
}

.desktop-icon:selected,
.desktop-icon:active {
    background: rgba(0, 150, 136, 0.3);
}

.desktop-icon-image {
    /* Icon image styling */
}

.desktop-icon-label {
    color: rgba(255, 255, 255, 0.95);
    font-size: 11px;
    font-weight: 500;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.8);
    margin-top: 4px;
}

.desktop-context-menu {
    background: rgba(30, 30, 35, 0.95);
    border-radius: 12px;
    padding: 8px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    min-width: 180px;
}

.desktop-context-menu button {
    border-radius: 6px;
    padding: 10px 16px;
    min-width: 160px;
    margin: 2px 0;
}

.desktop-context-menu button:hover {
    background: rgba(255, 255, 255, 0.1);
}

/* ========== Application Menu (Phase 3) ========== */

.menu-container {
    background: rgba(25, 25, 28, 0.98);
    border-radius: 0 0 12px 0;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-top: none;
}

.menu-header {
    font-size: 18px;
    font-weight: 700;
    color: rgba(0, 200, 180, 0.95);
    letter-spacing: 0.5px;
}

.menu-search {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    border: 1px solid transparent;
    color: rgba(255, 255, 255, 0.9);
    padding: 8px 12px;
}

.menu-search:focus {
    background: rgba(255, 255, 255, 0.12);
    border: 1px solid rgba(0, 150, 136, 0.5);
}

.menu-sidebar {
    background: rgba(0, 0, 0, 0.2);
    border-right: 1px solid rgba(255, 255, 255, 0.05);
}

.menu-category-list {
    background: transparent;
}

.menu-category-row {
    background: transparent;
    border-radius: 6px;
}

.menu-category-row:hover {
    background: rgba(255, 255, 255, 0.08);
}

.menu-category-row:selected {
    background: rgba(0, 150, 136, 0.3);
}

.menu-app-scroll {
    background: transparent;
}

.menu-app-list {
    background: transparent;
}

.menu-app-row {
    background: transparent;
    border-radius: 8px;
    margin: 2px 4px;
}

.menu-app-row:hover {
    background: rgba(255, 255, 255, 0.08);
}

.menu-app-row:selected {
    background: rgba(0, 150, 136, 0.25);
}

.menu-app-icon {
    /* App icon in list */
}

.menu-app-name {
    font-size: 13px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.95);
}

.menu-app-comment {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.5);
}

.menu-power-section {
    background: rgba(0, 0, 0, 0.2);
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    padding: 8px;
}

.menu-power-button {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    padding: 8px 16px;
    font-size: 12px;
}

.menu-power-button:hover {
    background: rgba(255, 255, 255, 0.15);
}

.menu-power-reboot {
    color: rgba(255, 180, 100, 0.9);
}

.menu-power-reboot:hover {
    background: rgba(255, 180, 100, 0.2);
}

.menu-power-shutdown {
    color: rgba(255, 100, 100, 0.9);
}

.menu-power-shutdown:hover {
    background: rgba(255, 100, 100, 0.2);
}

/* ========== Power Overlay (Phase 3) ========== */

.power-overlay {
    background: rgba(15, 15, 18, 0.95);
    padding: 48px;
}

.power-overlay-title {
    font-size: 32px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.95);
}

.power-overlay-subtitle {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.5);
    margin-top: 8px;
}

.power-overlay-hint {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.4);
}

.power-overlay-button {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    padding: 24px 32px;
    min-width: 120px;
    transition: all 200ms ease;
}

.power-overlay-button:hover {
    background: rgba(255, 255, 255, 0.15);
    transform: scale(1.05);
}

.power-overlay-button:active {
    transform: scale(0.98);
}

.power-overlay-icon {
    color: rgba(255, 255, 255, 0.9);
}

.power-overlay-button-name {
    font-size: 14px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.95);
    margin-top: 8px;
}

.power-overlay-button-desc {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.5);
}

.power-button-reboot {
    background: rgba(255, 180, 100, 0.15);
}

.power-button-reboot:hover {
    background: rgba(255, 180, 100, 0.25);
}

.power-button-reboot .power-overlay-icon {
    color: rgba(255, 180, 100, 0.9);
}

.power-button-shutdown {
    background: rgba(255, 100, 100, 0.15);
}

.power-button-shutdown:hover {
    background: rgba(255, 100, 100, 0.25);
}

.power-button-shutdown .power-overlay-icon {
    color: rgba(255, 100, 100, 0.9);
}

/* ========== Keybindings Overlay (Phase 3) ========== */

.keybindings-overlay {
    background: rgba(15, 15, 18, 0.95);
    padding: 48px;
}

.keybindings-title {
    font-size: 32px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.95);
}

.keybindings-subtitle {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.5);
    margin-top: 8px;
}

.keybindings-scroll {
    /* Scrollable area */
}

.keybindings-category {
    min-width: 320px;
}

.keybindings-category-header {
    margin-bottom: 12px;
}

.keybindings-category-icon {
    color: rgba(0, 200, 180, 0.9);
}

.keybindings-category-name {
    font-size: 16px;
    font-weight: 600;
    color: rgba(0, 200, 180, 0.9);
}

.keybindings-row {
    padding: 6px 0;
}

.keybindings-key {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    padding: 4px 10px;
    font-family: monospace;
    font-size: 12px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.9);
}

.keybindings-description {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.7);
}

/* ========== Settings Component (Phase 4) ========== */

.settings-window {
    background: rgba(25, 25, 28, 0.98);
}

.settings-header {
    background: rgba(0, 0, 0, 0.2);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.settings-title {
    font-size: 20px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.95);
}

.settings-subtitle {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
}

.settings-close-button {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 8px;
    min-width: 36px;
    min-height: 36px;
}

.settings-close-button:hover {
    background: rgba(255, 100, 100, 0.3);
}

.settings-sidebar {
    background: rgba(0, 0, 0, 0.2);
    border-right: 1px solid rgba(255, 255, 255, 0.05);
}

.settings-category-list {
    background: transparent;
}

.settings-category-row {
    background: transparent;
    border-radius: 8px;
    margin: 2px 8px;
}

.settings-category-row:hover {
    background: rgba(255, 255, 255, 0.08);
}

.settings-category-row:selected {
    background: rgba(0, 150, 136, 0.3);
}

.settings-category-icon {
    color: rgba(255, 255, 255, 0.7);
}

.settings-category-row:selected .settings-category-icon {
    color: rgba(0, 200, 180, 0.9);
}

.settings-category-name {
    font-size: 13px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.9);
}

.settings-category-desc {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.5);
}

.settings-content {
    background: transparent;
    padding: 24px;
}

.settings-page {
    background: transparent;
}

.settings-page-title {
    font-size: 24px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.95);
    margin-bottom: 8px;
}

.settings-page-subtitle {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.5);
    margin-bottom: 24px;
}

.settings-section {
    background: rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    padding: 16px;
    margin-bottom: 16px;
}

.settings-section-title {
    font-size: 14px;
    font-weight: 600;
    color: rgba(0, 200, 180, 0.9);
    margin-bottom: 16px;
}

.settings-row {
    padding: 12px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.settings-row:last-child {
    border-bottom: none;
}

.settings-label {
    font-size: 13px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.9);
}

.settings-description {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.5);
}

.settings-control {
    min-width: 180px;
}

dropdown.settings-control {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    padding: 6px 12px;
}

dropdown.settings-control:hover {
    background: rgba(255, 255, 255, 0.12);
}

scale.settings-control {
    min-width: 200px;
}

switch.settings-control {
    /* Switch styling */
}

entry.settings-control {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    padding: 6px 12px;
    border: 1px solid transparent;
}

entry.settings-control:focus {
    background: rgba(255, 255, 255, 0.12);
    border: 1px solid rgba(0, 150, 136, 0.5);
}

.settings-button {
    background: rgba(0, 150, 136, 0.3);
    border-radius: 6px;
    padding: 6px 16px;
}

.settings-button:hover {
    background: rgba(0, 150, 136, 0.45);
}

.color-button {
    border-radius: 16px;
    min-width: 32px;
    min-height: 32px;
    margin: 0 4px;
    border: 2px solid transparent;
    transition: all 150ms ease;
}

.color-button:hover {
    transform: scale(1.1);
}

.color-button:active {
    transform: scale(0.95);
}

/* About page */
.about-logo {
    color: rgba(0, 200, 180, 0.9);
    margin-bottom: 16px;
}

.about-title {
    font-size: 32px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.95);
}

.about-version {
    font-size: 14px;
    color: rgba(0, 200, 180, 0.9);
    margin-top: 4px;
}

.about-description {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.7);
    margin-top: 16px;
}

.about-info-label {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
}

.about-info-value {
    font-size: 13px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.9);
}

/* ========== File Manager Component (Phase 4) ========== */

.file-manager {
    background: rgba(30, 30, 32, 0.98);
}

.fm-header {
    background: rgba(0, 0, 0, 0.2);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.fm-nav-button {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    padding: 6px;
    min-width: 32px;
    min-height: 32px;
}

.fm-nav-button:hover {
    background: rgba(255, 255, 255, 0.15);
}

.fm-nav-button:disabled {
    opacity: 0.4;
}

.fm-location-bar {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    padding: 6px 12px;
    border: 1px solid transparent;
    color: rgba(255, 255, 255, 0.9);
}

.fm-location-bar:focus {
    background: rgba(255, 255, 255, 0.12);
    border: 1px solid rgba(0, 150, 136, 0.5);
}

.fm-search-entry {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    padding: 6px 12px;
    border: 1px solid transparent;
}

.fm-search-entry:focus {
    background: rgba(255, 255, 255, 0.12);
    border: 1px solid rgba(0, 150, 136, 0.5);
}

.fm-sidebar {
    background: rgba(0, 0, 0, 0.2);
    border-right: 1px solid rgba(255, 255, 255, 0.05);
}

.fm-sidebar-section {
    font-size: 11px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.5);
    text-transform: uppercase;
    letter-spacing: 0.5px;
}

.fm-sidebar-list {
    background: transparent;
}

.fm-sidebar-row {
    background: transparent;
    border-radius: 6px;
    margin: 1px 4px;
}

.fm-sidebar-row:hover {
    background: rgba(255, 255, 255, 0.08);
}

.fm-sidebar-row:selected {
    background: rgba(0, 150, 136, 0.3);
}

.fm-sidebar-icon {
    color: rgba(255, 255, 255, 0.7);
}

.fm-sidebar-row:selected .fm-sidebar-icon {
    color: rgba(0, 200, 180, 0.9);
}

.fm-sidebar-label {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.9);
}

.fm-file-area {
    background: transparent;
}

.fm-file-list {
    background: transparent;
}

.fm-file-row {
    background: transparent;
    border-radius: 6px;
    margin: 1px 4px;
}

.fm-file-row:hover {
    background: rgba(255, 255, 255, 0.08);
}

.fm-file-row:selected {
    background: rgba(0, 150, 136, 0.3);
}

.fm-file-icon-folder {
    color: rgba(255, 200, 100, 0.9);
}

.fm-file-name {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.9);
}

.fm-file-name-folder {
    font-weight: 600;
}

.fm-file-size {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
}

.fm-file-date {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
}

.fm-preview-pane {
    background: rgba(0, 0, 0, 0.15);
    border-left: 1px solid rgba(255, 255, 255, 0.05);
}

.fm-preview-header {
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.fm-preview-title {
    font-size: 14px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.9);
}

.fm-preview-content {
    /* Preview content area */
}

.fm-preview-placeholder {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.4);
}

.fm-preview-name {
    font-size: 14px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.95);
}

.fm-preview-detail {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.6);
}

.fm-status-bar {
    background: rgba(0, 0, 0, 0.2);
    border-top: 1px solid rgba(255, 255, 255, 0.05);
}

.fm-status-text {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.6);
}

.fm-status-text-right {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
}

/* ========== Phase 5: WiFi Manager ========== */

.wifi-manager {
    background: rgba(25, 25, 28, 0.98);
}

.wifi-header {
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.wifi-title {
    font-size: 18px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.95);
}

.wifi-backend {
    font-size: 11px;
    color: rgba(0, 200, 180, 0.8);
    background: rgba(0, 150, 136, 0.2);
    border-radius: 4px;
    padding: 2px 8px;
}

.wifi-status {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.7);
    padding: 8px 0;
}

.wifi-network-list {
    background: transparent;
}

.wifi-network-row {
    background: transparent;
    border-radius: 8px;
    margin: 2px 8px;
}

.wifi-network-row:hover {
    background: rgba(255, 255, 255, 0.08);
}

.wifi-network-row.connected {
    background: rgba(0, 150, 136, 0.2);
}

.wifi-network-row.connected:hover {
    background: rgba(0, 150, 136, 0.3);
}

.wifi-signal {
    font-family: monospace;
    font-weight: 700;
    color: rgba(0, 200, 180, 0.9);
    min-width: 40px;
}

.wifi-network-name {
    font-size: 14px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.95);
}

.wifi-network-detail {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.5);
}

.wifi-signal-percent {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.6);
}

.wifi-button {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 8px 16px;
}

.wifi-button:hover {
    background: rgba(255, 255, 255, 0.18);
}

.wifi-saved-list {
    background: transparent;
}

/* ========== Phase 5: USB Creator ========== */

.usb-creator {
    background: rgba(25, 25, 28, 0.98);
}

.usb-title {
    font-size: 28px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.95);
}

.usb-description {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.7);
    line-height: 1.6;
}

.usb-page-title {
    font-size: 20px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.95);
    margin-bottom: 8px;
}

.usb-page-desc {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.6);
}

.usb-combo {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    padding: 8px 12px;
    min-height: 36px;
}

.usb-combo:hover {
    background: rgba(255, 255, 255, 0.12);
}

.usb-entry {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    padding: 8px 12px;
    border: 1px solid transparent;
}

.usb-entry:focus {
    background: rgba(255, 255, 255, 0.12);
    border: 1px solid rgba(0, 150, 136, 0.5);
}

.usb-warning {
    font-size: 12px;
    color: rgba(255, 150, 100, 0.9);
    background: rgba(255, 150, 100, 0.1);
    border-radius: 8px;
    padding: 12px;
    margin-top: 16px;
}

.usb-summary {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.8);
    background: rgba(255, 255, 255, 0.05);
    border-radius: 8px;
    padding: 16px;
}

.usb-button {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 8px 20px;
}

.usb-button:hover {
    background: rgba(255, 255, 255, 0.18);
}

.usb-progress {
    min-height: 24px;
    border-radius: 12px;
}

.usb-progress trough {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 12px;
}

.usb-progress progress {
    background: linear-gradient(90deg, rgba(0, 150, 136, 0.8), rgba(0, 200, 180, 0.9));
    border-radius: 12px;
}

.usb-status {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.7);
}

.usb-info {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
}

/* ========== Phase 5: System Installer ========== */

.installer {
    background: rgba(20, 20, 24, 0.98);
}

.installer-header {
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.installer-logo {
    font-size: 24px;
    font-weight: 700;
    color: rgba(0, 200, 180, 0.95);
    letter-spacing: 1px;
}

.installer-title {
    font-size: 32px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.95);
}

.installer-description {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.7);
    line-height: 1.7;
}

.installer-page-title {
    font-size: 22px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.95);
}

.installer-page-desc {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.6);
}

.installer-combo {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    padding: 8px 12px;
    min-height: 40px;
}

.installer-combo:hover {
    background: rgba(255, 255, 255, 0.12);
}

.installer-warning {
    font-size: 12px;
    color: rgba(255, 100, 100, 0.9);
    background: rgba(255, 100, 100, 0.1);
    border-radius: 8px;
    padding: 12px;
    margin-top: 16px;
}

.installer-option-desc {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
    line-height: 1.5;
}

.installer-button {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 10px 24px;
    font-weight: 500;
}

.installer-button:hover {
    background: rgba(255, 255, 255, 0.18);
}

.installer-progress {
    min-height: 28px;
    border-radius: 14px;
}

.installer-progress trough {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 14px;
}

.installer-progress progress {
    background: linear-gradient(90deg, rgba(0, 150, 136, 0.8), rgba(0, 200, 180, 0.9));
    border-radius: 14px;
}

.installer-status {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.8);
}

.installer-info {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
}
"#;

/// Load and apply the panel CSS theme
pub fn load_css() {
    let provider = CssProvider::new();
    provider.load_from_data(PANEL_CSS);

    if let Some(display) = Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        debug!("CSS theme loaded");
    }
}
