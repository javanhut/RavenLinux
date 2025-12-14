//! Service management for RavenInit

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

use crate::config::ServiceConfig;

/// Service state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    /// Service is running
    Running,
    /// Service has exited normally
    Exited,
    /// Service was killed by a signal
    Signaled,
    /// Service is stopped
    Stopped,
    /// Service failed to start
    Failed,
}

/// A managed service
pub struct Service {
    /// Service configuration
    config: ServiceConfig,
    /// Current state
    state: ServiceState,
    /// Child process handle
    child: Option<Child>,
    /// Process ID
    pid: Option<Pid>,
    /// Exit status (if exited)
    exit_status: Option<i32>,
    /// Signal that killed the process (if signaled)
    exit_signal: Option<Signal>,
    /// Number of restart attempts
    restart_count: u32,
    /// Last restart time
    last_restart: Option<Instant>,
}

impl Service {
    /// Start a new service
    pub fn start(config: &ServiceConfig) -> Result<Self> {
        let mut service = Self {
            config: config.clone(),
            state: ServiceState::Stopped,
            child: None,
            pid: None,
            exit_status: None,
            exit_signal: None,
            restart_count: 0,
            last_restart: None,
        };

        service.do_start()?;
        Ok(service)
    }

    fn do_start(&mut self) -> Result<()> {
        let mut cmd = Command::new(&self.config.exec);

        // Add arguments
        cmd.args(&self.config.args);

        // Set environment
        for (key, value) in &self.config.environment {
            cmd.env(key, value);
        }

        // Set up stdio
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        // Spawn the process
        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to start {}", self.config.name))?;

        let pid = Pid::from_raw(child.id() as i32);

        self.child = Some(child);
        self.pid = Some(pid);
        self.state = ServiceState::Running;
        self.exit_status = None;
        self.exit_signal = None;

        log::debug!("Service {} started with PID {}", self.config.name, pid);

        Ok(())
    }

    /// Get service name
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Get current state
    pub fn state(&self) -> ServiceState {
        self.state
    }

    /// Get process ID
    pub fn pid(&self) -> Option<Pid> {
        self.pid
    }

    /// Check if service should be restarted
    pub fn should_restart(&self) -> bool {
        if !self.config.restart {
            return false;
        }

        // Rate limit restarts
        if let Some(last_restart) = self.last_restart {
            if last_restart.elapsed() < Duration::from_secs(5) {
                // Don't restart more than once every 5 seconds
                if self.restart_count > 5 {
                    log::warn!(
                        "Service {} is restarting too frequently, disabling restart",
                        self.config.name
                    );
                    return false;
                }
            } else {
                // Reset count after 30 seconds of stability
                if last_restart.elapsed() > Duration::from_secs(30) {
                    return true;
                }
            }
        }

        true
    }

    /// Mark service as exited
    pub fn mark_exited(&mut self, status: i32) {
        self.state = ServiceState::Exited;
        self.exit_status = Some(status);
        self.pid = None;
        self.child = None;

        log::info!("Service {} exited with status {}", self.config.name, status);
    }

    /// Mark service as killed by signal
    pub fn mark_signaled(&mut self, signal: Signal) {
        self.state = ServiceState::Signaled;
        self.exit_signal = Some(signal);
        self.pid = None;
        self.child = None;

        log::info!("Service {} killed by signal {:?}", self.config.name, signal);
    }

    /// Restart the service
    pub fn restart(&mut self) -> Result<()> {
        self.restart_count += 1;
        self.last_restart = Some(Instant::now());

        log::info!(
            "Restarting service {} (attempt {})",
            self.config.name,
            self.restart_count
        );

        self.do_start()
    }

    /// Stop the service (SIGTERM)
    pub fn stop(&mut self) {
        if let Some(pid) = self.pid {
            log::debug!("Sending SIGTERM to {} (PID {})", self.config.name, pid);
            let _ = signal::kill(pid, Signal::SIGTERM);
        }
    }

    /// Kill the service (SIGKILL)
    pub fn kill(&mut self) {
        if let Some(pid) = self.pid {
            log::debug!("Sending SIGKILL to {} (PID {})", self.config.name, pid);
            let _ = signal::kill(pid, Signal::SIGKILL);
        }
        self.state = ServiceState::Stopped;
        self.pid = None;
        self.child = None;
    }
}
