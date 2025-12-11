//! raven-installer - RavenLinux System Installer
//!
//! A TUI-based installer for RavenLinux with support for
//! disk partitioning, encryption, and system configuration.

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use std::io::{self, stdout};

mod config;
mod disk;
mod install;
mod steps;
mod ui;

use config::InstallConfig;

/// Installation steps
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Welcome,
    Language,
    Keyboard,
    DiskSelection,
    DiskPartitioning,
    Encryption,
    UserSetup,
    PackageSelection,
    Summary,
    Installing,
    Complete,
}

impl Step {
    fn title(&self) -> &'static str {
        match self {
            Step::Welcome => "Welcome to RavenLinux",
            Step::Language => "Language Selection",
            Step::Keyboard => "Keyboard Layout",
            Step::DiskSelection => "Select Installation Disk",
            Step::DiskPartitioning => "Disk Partitioning",
            Step::Encryption => "Disk Encryption",
            Step::UserSetup => "User Account Setup",
            Step::PackageSelection => "Package Selection",
            Step::Summary => "Installation Summary",
            Step::Installing => "Installing RavenLinux",
            Step::Complete => "Installation Complete",
        }
    }

    fn next(&self) -> Option<Step> {
        match self {
            Step::Welcome => Some(Step::Language),
            Step::Language => Some(Step::Keyboard),
            Step::Keyboard => Some(Step::DiskSelection),
            Step::DiskSelection => Some(Step::DiskPartitioning),
            Step::DiskPartitioning => Some(Step::Encryption),
            Step::Encryption => Some(Step::UserSetup),
            Step::UserSetup => Some(Step::PackageSelection),
            Step::PackageSelection => Some(Step::Summary),
            Step::Summary => Some(Step::Installing),
            Step::Installing => Some(Step::Complete),
            Step::Complete => None,
        }
    }

    fn prev(&self) -> Option<Step> {
        match self {
            Step::Welcome => None,
            Step::Language => Some(Step::Welcome),
            Step::Keyboard => Some(Step::Language),
            Step::DiskSelection => Some(Step::Keyboard),
            Step::DiskPartitioning => Some(Step::DiskSelection),
            Step::Encryption => Some(Step::DiskPartitioning),
            Step::UserSetup => Some(Step::Encryption),
            Step::PackageSelection => Some(Step::UserSetup),
            Step::Summary => Some(Step::PackageSelection),
            Step::Installing => None, // Can't go back during installation
            Step::Complete => None,
        }
    }
}

/// Main installer state
pub struct Installer {
    current_step: Step,
    config: InstallConfig,
    progress: f64,
    status_message: String,
    running: bool,
}

impl Installer {
    fn new() -> Self {
        Self {
            current_step: Step::Welcome,
            config: InstallConfig::default(),
            progress: 0.0,
            status_message: String::new(),
            running: true,
        }
    }

    fn next_step(&mut self) {
        if let Some(next) = self.current_step.next() {
            self.current_step = next;
        }
    }

    fn prev_step(&mut self) {
        if let Some(prev) = self.current_step.prev() {
            self.current_step = prev;
        }
    }
}

fn main() -> Result<()> {
    // Initialize terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    // Create installer
    let mut installer = Installer::new();

    // Main loop
    while installer.running {
        terminal.draw(|f| ui::draw(f, &installer))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            if installer.current_step != Step::Installing {
                                installer.running = false;
                            }
                        }
                        KeyCode::Enter | KeyCode::Right => {
                            installer.next_step();
                        }
                        KeyCode::Left | KeyCode::Backspace => {
                            installer.prev_step();
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}

mod ui {
    use super::*;

    pub fn draw(frame: &mut Frame, installer: &Installer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(10),   // Content
                Constraint::Length(3), // Footer
            ])
            .split(frame.area());

        // Header
        let header = Paragraph::new(format!(
            "  RavenLinux Installer - {}",
            installer.current_step.title()
        ))
        .style(Style::default().fg(Color::Cyan).bold())
        .block(Block::default().borders(Borders::BOTTOM));
        frame.render_widget(header, chunks[0]);

        // Content based on current step
        let content = match installer.current_step {
            Step::Welcome => render_welcome(),
            Step::Language => render_language(),
            Step::Keyboard => render_keyboard(),
            Step::DiskSelection => render_disk_selection(),
            Step::DiskPartitioning => render_partitioning(),
            Step::Encryption => render_encryption(),
            Step::UserSetup => render_user_setup(),
            Step::PackageSelection => render_package_selection(),
            Step::Summary => render_summary(&installer.config),
            Step::Installing => render_installing(installer.progress, &installer.status_message),
            Step::Complete => render_complete(),
        };
        frame.render_widget(content, chunks[1]);

        // Footer
        let footer = Paragraph::new("  [←/→] Navigate  [Enter] Confirm  [q] Quit")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::TOP));
        frame.render_widget(footer, chunks[2]);
    }

    fn render_welcome() -> Paragraph<'static> {
        let text = r#"
   _____                         _      _
  |  __ \                       | |    (_)
  | |__) |__ ___   _____ _ __   | |     _ _ __  _   ___  __
  |  _  // _` \ \ / / _ \ '_ \  | |    | | '_ \| | | \ \/ /
  | | \ \ (_| |\ V /  __/ | | | | |____| | | | | |_| |>  <
  |_|  \_\__,_| \_/ \___|_| |_| |______|_|_| |_|\__,_/_/\_\

  Welcome to the RavenLinux installer!

  RavenLinux is a developer-focused Linux distribution
  designed for the best coding experience.

  This installer will guide you through:
    • Disk partitioning and optional encryption
    • User account creation
    • Package selection (minimal, standard, or full)
    • System configuration

  Press [Enter] or [→] to continue."#;

        Paragraph::new(text)
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::ALL).title(" Welcome "))
    }

    fn render_language() -> Paragraph<'static> {
        Paragraph::new("Select your language (placeholder)")
            .block(Block::default().borders(Borders::ALL).title(" Language "))
    }

    fn render_keyboard() -> Paragraph<'static> {
        Paragraph::new("Select keyboard layout (placeholder)")
            .block(Block::default().borders(Borders::ALL).title(" Keyboard "))
    }

    fn render_disk_selection() -> Paragraph<'static> {
        Paragraph::new("Select installation disk (placeholder)")
            .block(Block::default().borders(Borders::ALL).title(" Disk Selection "))
    }

    fn render_partitioning() -> Paragraph<'static> {
        Paragraph::new("Configure partitions (placeholder)")
            .block(Block::default().borders(Borders::ALL).title(" Partitioning "))
    }

    fn render_encryption() -> Paragraph<'static> {
        Paragraph::new("Configure disk encryption (placeholder)")
            .block(Block::default().borders(Borders::ALL).title(" Encryption "))
    }

    fn render_user_setup() -> Paragraph<'static> {
        Paragraph::new("Create user account (placeholder)")
            .block(Block::default().borders(Borders::ALL).title(" User Setup "))
    }

    fn render_package_selection() -> Paragraph<'static> {
        let text = r#"
  Select installation profile:

  [ ] Minimal
      Basic system with command-line only
      Size: ~500 MB

  [*] Standard (Recommended)
      Full desktop environment with developer tools
      Size: ~5 GB

  [ ] Full
      Everything including additional languages and tools
      Size: ~12 GB
"#;
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(" Package Selection "))
    }

    fn render_summary(config: &InstallConfig) -> Paragraph<'static> {
        Paragraph::new("Installation summary (placeholder)")
            .block(Block::default().borders(Borders::ALL).title(" Summary "))
    }

    fn render_installing(progress: f64, status: &str) -> Paragraph<'static> {
        let progress_bar = "█".repeat((progress * 40.0) as usize);
        let empty = "░".repeat(40 - (progress * 40.0) as usize);

        let text = format!(
            r#"
  Installing RavenLinux...

  [{progress_bar}{empty}] {:.0}%

  {status}
"#,
            progress * 100.0
        );

        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(" Installing "))
    }

    fn render_complete() -> Paragraph<'static> {
        let text = r#"
  ╔═══════════════════════════════════════════════════════════╗
  ║                                                           ║
  ║   Installation Complete!                                  ║
  ║                                                           ║
  ║   RavenLinux has been successfully installed.             ║
  ║                                                           ║
  ║   Remove the installation media and reboot to start       ║
  ║   using your new system.                                  ║
  ║                                                           ║
  ║   Press [Enter] to reboot or [q] to exit.                 ║
  ║                                                           ║
  ╚═══════════════════════════════════════════════════════════╝
"#;

        Paragraph::new(text)
            .style(Style::default().fg(Color::Green))
            .block(Block::default().borders(Borders::ALL).title(" Complete "))
    }
}
