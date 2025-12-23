package css

const FileManagerCSS = `
	window {
		background-color: #0f1720;
	}

	.header-bar {
		background-color: #1a2332;
		border-bottom: 1px solid #333;
		padding: 6px 12px;
	}

	.nav-button {
		background: transparent;
		border: none;
		padding: 6px 10px;
		color: #e0e0e0;
		border-radius: 4px;
		min-width: 32px;
	}

	.nav-button:hover {
		background: rgba(255, 255, 255, 0.1);
	}

	.nav-button:disabled {
		color: #555;
	}

	.nav-button:active {
		background: rgba(255, 255, 255, 0.15);
	}

	.location-bar {
		background-color: #1a2332;
		border: 1px solid #333;
		border-radius: 6px;
		padding: 6px 12px;
		color: #e0e0e0;
		font-size: 13px;
	}

	.location-bar:focus {
		border-color: #009688;
		outline: none;
	}

	.search-entry {
		background-color: #1a2332;
		border: 1px solid #333;
		border-radius: 6px;
		padding: 6px 12px;
		color: #e0e0e0;
		font-size: 13px;
		min-width: 250px;
	}

	.search-entry:focus {
		border-color: #009688;
		outline: none;
	}

	.sidebar {
		background-color: #151d28;
		border-right: 1px solid #333;
	}

	.sidebar-section {
		padding: 12px 16px 6px 16px;
		color: #666;
		font-size: 11px;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.5px;
	}

	.sidebar-list {
		background-color: transparent;
	}

	.sidebar-list row {
		padding: 8px 16px;
		border-radius: 0;
		margin: 0;
	}

	.sidebar-list row:selected {
		background-color: rgba(0, 150, 136, 0.3);
	}

	.sidebar-list row:hover:not(:selected) {
		background-color: rgba(255, 255, 255, 0.05);
	}

	.sidebar-item {
		color: #e0e0e0;
		font-size: 13px;
	}

	.sidebar-item-icon {
		color: #888;
		margin-right: 8px;
	}

	.file-list {
		background-color: #0f1720;
	}

	.file-list row {
		padding: 6px 12px;
		border-radius: 4px;
		margin: 1px 4px;
	}

	.file-list row:selected {
		background-color: rgba(0, 150, 136, 0.4);
	}

	.file-list row:hover:not(:selected) {
		background-color: rgba(255, 255, 255, 0.05);
	}

	.file-row {
		padding: 4px 8px;
	}

	.file-icon {
		color: #888;
		min-width: 24px;
	}

	.file-icon-folder {
		color: #009688;
	}

	.file-name {
		color: #e0e0e0;
		font-size: 13px;
	}

	.file-name-folder {
		font-weight: 500;
	}

	.file-name-hidden {
		color: #888;
	}

	.file-size {
		color: #888;
		font-size: 12px;
		min-width: 80px;
	}

	.file-date {
		color: #888;
		font-size: 12px;
		min-width: 120px;
	}

	.file-grid {
		background-color: #0f1720;
	}

	.file-grid-item {
		padding: 12px;
		border-radius: 6px;
		margin: 4px;
	}

	.file-grid-item:selected {
		background-color: rgba(0, 150, 136, 0.4);
	}

	.file-grid-item:hover:not(:selected) {
		background-color: rgba(255, 255, 255, 0.05);
	}

	.file-grid-icon {
		font-size: 48px;
		color: #888;
		margin-bottom: 8px;
	}

	.file-grid-icon-folder {
		color: #009688;
	}

	.file-grid-name {
		color: #e0e0e0;
		font-size: 12px;
		text-align: center;
	}

	.preview-pane {
		background-color: #151d28;
		border-left: 1px solid #333;
	}

	.preview-header {
		background-color: #1a2332;
		padding: 12px 16px;
		border-bottom: 1px solid #333;
	}

	.preview-title {
		color: #e0e0e0;
		font-size: 14px;
		font-weight: 500;
	}

	.preview-content {
		padding: 16px;
	}

	.preview-image {
		background-color: #0f1720;
		border-radius: 6px;
	}

	.preview-text {
		font-family: "JetBrains Mono", "Fira Code", "Source Code Pro", monospace;
		font-size: 12px;
		color: #e0e0e0;
		background-color: #0f1720;
		padding: 12px;
		border-radius: 6px;
	}

	.preview-info {
		padding: 12px 0;
	}

	.preview-info-row {
		padding: 4px 0;
	}

	.preview-info-label {
		color: #888;
		font-size: 12px;
		min-width: 80px;
	}

	.preview-info-value {
		color: #e0e0e0;
		font-size: 12px;
	}

	.status-bar {
		background-color: #1a2332;
		border-top: 1px solid #333;
		padding: 4px 12px;
	}

	.status-text {
		color: #888;
		font-size: 12px;
	}

	.status-text-right {
		color: #888;
		font-size: 12px;
	}

	.filter-panel {
		background-color: #1a2332;
		padding: 8px 12px;
		border-bottom: 1px solid #333;
	}

	.filter-dropdown {
		background-color: #1a2332;
		border: 1px solid #333;
		border-radius: 4px;
		padding: 4px 8px;
		color: #e0e0e0;
		font-size: 12px;
	}

	.filter-dropdown:hover {
		border-color: #444;
	}

	.filter-label {
		color: #888;
		font-size: 12px;
		margin-right: 8px;
	}

	.filter-clear {
		background: transparent;
		border: none;
		color: #009688;
		font-size: 12px;
		padding: 4px 8px;
	}

	.filter-clear:hover {
		background: rgba(0, 150, 136, 0.2);
		border-radius: 4px;
	}

	.search-result {
		padding: 8px 12px;
	}

	.search-result-path {
		color: #888;
		font-size: 11px;
	}

	.search-result-name {
		color: #e0e0e0;
		font-size: 13px;
	}

	.search-result-match {
		color: #009688;
		font-weight: 500;
	}

	.search-result-score {
		color: #666;
		font-size: 10px;
		padding: 2px 6px;
		background: rgba(0, 150, 136, 0.2);
		border-radius: 10px;
	}

	.search-result-context {
		color: #888;
		font-size: 11px;
		font-family: monospace;
		background-color: #0f1720;
		padding: 4px 8px;
		border-radius: 4px;
		margin-top: 4px;
	}

	popover {
		background-color: #1a2332;
		border: 1px solid #333;
		border-radius: 8px;
	}

	popover contents {
		padding: 4px;
	}

	popover modelbutton {
		padding: 8px 16px;
		color: #e0e0e0;
		border-radius: 4px;
	}

	popover modelbutton:hover {
		background-color: rgba(255, 255, 255, 0.1);
	}

	popover separator {
		background-color: #333;
		margin: 4px 8px;
	}

	dialog {
		background-color: #1a2332;
	}

	dialog .dialog-content {
		padding: 16px;
	}

	dialog entry {
		background-color: #0f1720;
		border: 1px solid #333;
		border-radius: 6px;
		padding: 8px 12px;
		color: #e0e0e0;
	}

	dialog entry:focus {
		border-color: #009688;
	}

	dialog label {
		color: #e0e0e0;
	}

	dialog button {
		background-color: #009688;
		border: none;
		border-radius: 4px;
		padding: 8px 16px;
		color: #ffffff;
	}

	dialog button:hover {
		background-color: #00a896;
	}

	dialog button.cancel {
		background-color: transparent;
		border: 1px solid #333;
		color: #e0e0e0;
	}

	dialog button.cancel:hover {
		background-color: rgba(255, 255, 255, 0.05);
	}

	dialog button.destructive {
		background-color: #f44336;
	}

	dialog button.destructive:hover {
		background-color: #e53935;
	}

	scrollbar {
		background-color: transparent;
	}

	scrollbar slider {
		background-color: rgba(255, 255, 255, 0.2);
		border-radius: 4px;
		min-width: 8px;
		min-height: 8px;
	}

	scrollbar slider:hover {
		background-color: rgba(255, 255, 255, 0.3);
	}

	scrollbar slider:active {
		background-color: rgba(0, 150, 136, 0.5);
	}

	separator {
		background-color: #333;
		min-width: 1px;
		min-height: 1px;
	}

	dropdown button {
		background-color: #1a2332;
		border: 1px solid #333;
		border-radius: 4px;
		padding: 4px 8px;
		color: #e0e0e0;
	}

	dropdown button:hover {
		border-color: #444;
	}

	dropdown popover {
		background-color: #1a2332;
	}

	.syntax-keyword {
		color: #009688;
	}

	.syntax-string {
		color: #a5d6a7;
	}

	.syntax-comment {
		color: #666666;
		font-style: italic;
	}

	.syntax-number {
		color: #ff9800;
	}

	.syntax-function {
		color: #64b5f6;
	}

	.syntax-type {
		color: #ce93d8;
	}

	.syntax-operator {
		color: #e0e0e0;
	}

	.syntax-variable {
		color: #ef9a9a;
	}

	.empty-state {
		padding: 48px;
	}

	.empty-state-icon {
		font-size: 64px;
		color: #444;
		margin-bottom: 16px;
	}

	.empty-state-text {
		color: #888;
		font-size: 14px;
	}

	.loading {
		color: #009688;
	}

	.breadcrumb {
		background-color: transparent;
	}

	.breadcrumb-item {
		background: transparent;
		border: none;
		color: #888;
		padding: 4px 8px;
		font-size: 13px;
	}

	.breadcrumb-item:hover {
		color: #e0e0e0;
		background-color: rgba(255, 255, 255, 0.05);
		border-radius: 4px;
	}

	.breadcrumb-item-current {
		color: #e0e0e0;
		font-weight: 500;
	}

	.breadcrumb-separator {
		color: #555;
		padding: 0 2px;
	}

	.view-toggle {
		background: transparent;
		border: 1px solid #333;
		padding: 4px 8px;
		color: #888;
		border-radius: 0;
	}

	.view-toggle:first-child {
		border-radius: 4px 0 0 4px;
	}

	.view-toggle:last-child {
		border-radius: 0 4px 4px 0;
		border-left: none;
	}

	.view-toggle:hover {
		background: rgba(255, 255, 255, 0.05);
		color: #e0e0e0;
	}

	.view-toggle:checked {
		background: rgba(0, 150, 136, 0.3);
		color: #009688;
		border-color: #009688;
	}

	.drop-target {
		border: 2px dashed #009688;
		background-color: rgba(0, 150, 136, 0.1);
	}

	.dragging {
		opacity: 0.5;
	}
`
