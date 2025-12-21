//! Service management for RavenInit

use std::ffi::CString;
use std::os::unix::io::RawFd;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use nix::fcntl::{open, OFlag};
use nix::sys::signal::{self, Signal};
use nix::sys::stat::Mode;
use nix::unistd::{self, dup2, execvp, fork, setsid, ForkResult, Pid};

use crate::config::ServiceConfig;

// TIOCSCTTY ioctl to set controlling terminal
nix::ioctl_write_int_bad!(tiocsctty, libc::TIOCSCTTY);

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
        // Check if this service needs TTY handling
        if let Some(tty_path) = self.config.tty.clone() {
            return self.do_start_with_tty(&tty_path);
        }

        // Standard service spawning (no TTY)
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

    /// Start a service with proper TTY session and job control setup
    fn do_start_with_tty(&mut self, tty_path: &str) -> Result<()> {
        log::debug!(
            "Starting service {} with TTY {}",
            self.config.name,
            tty_path
        );

        // Prepare command and arguments as CStrings for execvp
        let exec_cstr = CString::new(self.config.exec.as_str())
            .with_context(|| format!("Invalid exec path: {}", self.config.exec))?;

        let mut args_cstr: Vec<CString> = Vec::with_capacity(self.config.args.len() + 1);
        args_cstr.push(exec_cstr.clone());
        for arg in &self.config.args {
            args_cstr.push(
                CString::new(arg.as_str())
                    .with_context(|| format!("Invalid argument: {}", arg))?,
            );
        }

        // Prepare environment
        let env_vars: Vec<(String, String)> = self.config.environment.clone().into_iter().collect();

        let tty_path_owned = tty_path.to_string();

        // Fork the process
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => {
                // Parent process - just record the child PID
                self.pid = Some(child);
                self.child = None; // We don't have a Child handle when using fork directly
                self.state = ServiceState::Running;
                self.exit_status = None;
                self.exit_signal = None;

                log::info!(
                    "Service {} started with PID {} on TTY {}",
                    self.config.name,
                    child,
                    tty_path_owned
                );

                Ok(())
            }
            Ok(ForkResult::Child) => {
                // Child process - set up TTY and exec

                // 1. Create a new session (become session leader)
                if let Err(e) = setsid() {
                    log::error!("setsid() failed: {}", e);
                    std::process::exit(1);
                }

                // 2. Open the TTY device
                let tty_fd: RawFd = match open(
                    tty_path_owned.as_str(),
                    OFlag::O_RDWR | OFlag::O_NOCTTY,
                    Mode::empty(),
                ) {
                    Ok(fd) => fd,
                    Err(e) => {
                        log::error!("Failed to open TTY {}: {}", tty_path_owned, e);
                        std::process::exit(1);
                    }
                };

                // 3. Set this TTY as the controlling terminal
                // TIOCSCTTY with arg 0 means "don't steal if already controlled"
                if let Err(e) = unsafe { tiocsctty(tty_fd, 0) } {
                    log::error!("TIOCSCTTY failed: {}", e);
                    // Continue anyway - some systems may not require this
                }

                // 4. Duplicate TTY fd to stdin/stdout/stderr
                if let Err(e) = dup2(tty_fd, 0) {
                    log::error!("dup2 stdin failed: {}", e);
                }
                if let Err(e) = dup2(tty_fd, 1) {
                    log::error!("dup2 stdout failed: {}", e);
                }
                if let Err(e) = dup2(tty_fd, 2) {
                    log::error!("dup2 stderr failed: {}", e);
                }

                // Close the original fd if it's not 0, 1, or 2
                if tty_fd > 2 {
                    let _ = unistd::close(tty_fd);
                }

                // 5. Set the foreground process group to our process group
                let our_pid = unistd::getpid();
                let ret = unsafe { libc::tcsetpgrp(0, our_pid.as_raw()) };
                if ret < 0 {
                    log::error!("tcsetpgrp failed: {}", std::io::Error::last_os_error());
                    // Continue anyway
                }

                // 6. Set environment variables
                for (key, value) in env_vars {
                    std::env::set_var(&key, &value);
                }

                // 7. Exec the service
                let _ = execvp(&exec_cstr, &args_cstr);

                // If we get here, exec failed
                log::error!("execvp failed for {}", self.config.exec);
                std::process::exit(1);
            }
            Err(e) => {
                anyhow::bail!("fork() failed: {}", e);
            }
        }
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
