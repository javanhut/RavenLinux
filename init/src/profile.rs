//! Power profiles -- how hard the CPU and the platform are allowed to run,
//! depending on whether the machine is on mains or on its battery.
//!
//! # Why this exists
//!
//! The kernel picks a cpufreq governor at boot and never revisits it, and the
//! one it picks is a build-time choice with no knowledge of whether there is a
//! battery in the machine. A laptop left on `performance` -- or worse, on
//! `userspace`, which pins every core at its maximum clock and waits for
//! someone to say otherwise -- drains its battery at desktop rates while doing
//! nothing. Nothing else in this tree touches the governor, so this does.
//!
//! On most distributions this job belongs to power-profiles-daemon or TLP.
//! Both are considerably larger than the problem: what a laptop needs is a
//! sensible frequency policy that gets a little more frugal when the cord is
//! pulled, and that is a few sysfs writes made at start and on every change of
//! supply.
//!
//! # What "balanced" means here
//!
//! There is no single knob. Depending on the CPU and the kernel the frequency
//! policy lives in one of two places, and the writes have to go to the right
//! one:
//!
//! * **A driver with an energy-performance preference** (`intel_pstate` and
//!   `amd-pstate` in their active modes) exposes `energy_performance_preference`
//!   per policy. There the firmware chooses the frequency and the governor is
//!   nearly a formality: `powersave` means "firmware decides", `performance`
//!   means "pin the top". The real dial is the preference, and `balance_performance`
//!   / `balance_power` are the two balanced settings it offers.
//!
//! * **Everything else** (`acpi-cpufreq`, and the p-state drivers in passive
//!   mode) picks the frequency in the kernel, and the governor is the whole
//!   policy. `schedutil` is the balanced one: it scales with the scheduler's
//!   own load estimate, ramps up in a tick and idles down as fast. `ondemand`
//!   is the fallback for a kernel built without it.
//!
//! On top of that, where the firmware offers an ACPI platform profile (the
//! thing a vendor's Fn key cycles through) it is set to match, and PCIe ASPM
//! is nudged to `powersave` on battery.
//!
//! # What it deliberately does not do
//!
//! It does not cap the maximum frequency, turn off turbo, or pull cores
//! offline. All of those buy battery by making the machine slower at the
//! moments you notice, and none of them is what "balanced" means. A
//! frequency policy that idles down properly is where nearly all the saving
//! is; the compile that runs on every core still gets every core.
//!
//! Nor does it touch runtime power management on individual PCI or USB
//! devices. That is where the wins get small and the bug reports get
//! strange -- a mouse that stalls, a NIC that drops -- and it is not this
//! daemon's job to be TLP.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// A named preset. These are the three power-profiles-daemon made familiar,
/// so a settings panel can show the same three words and mean the same thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Preset {
    /// Every core at its top clock, platform in its performance mode.
    Performance,
    /// Scale with load; the platform in its default mode.
    Balanced,
    /// Scale with load, leaning towards lower clocks; platform and PCIe links
    /// in their low-power modes.
    PowerSaver,
}

impl Preset {
    fn name(self) -> &'static str {
        match self {
            Preset::Performance => "performance",
            Preset::Balanced => "balanced",
            Preset::PowerSaver => "power-saver",
        }
    }
}

/// The `[profile]` table in `power.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Profile {
    /// Apply profiles at all. Off leaves the kernel's choice alone, for
    /// someone running a different tool for this.
    pub manage: bool,
    /// The preset while on mains -- and always, on a machine with no battery.
    pub ac: Preset,
    /// The preset while on battery.
    pub battery: Preset,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            manage: true,
            ac: Preset::Balanced,
            battery: Preset::PowerSaver,
        }
    }
}

// ---------------------------------------------------------------------------
// Supply
// ---------------------------------------------------------------------------

/// Where the power is coming from right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Supply {
    Mains,
    Battery,
}

/// Read the supply from sysfs.
///
/// A machine is on mains if any `Mains` (or `USB`, for a laptop charged over
/// USB-C) supply reports `online`. A machine with no such supply at all -- a
/// desktop, a VM -- is treated as on mains, because that is the only thing
/// it can be. A machine that has a battery but whose AC adapter does not
/// show up in sysfs (it happens, on some firmware) falls back to whether the
/// battery says it is discharging.
pub fn supply() -> Supply {
    supply_in(Path::new("/sys/class/power_supply"))
}

fn supply_in(dir: &Path) -> Supply {
    let Ok(entries) = fs::read_dir(dir) else {
        return Supply::Mains;
    };

    let mut saw_adapter = false;
    let mut battery_discharging = false;

    for entry in entries.filter_map(|e| e.ok()) {
        let p = entry.path();
        let kind = read_trimmed(&p.join("type")).unwrap_or_default();
        match kind.as_str() {
            "Mains" | "USB" => {
                saw_adapter = true;
                if read_trimmed(&p.join("online")).as_deref() == Some("1") {
                    return Supply::Mains;
                }
            }
            "Battery" => {
                // "Discharging" is the only status that means the wall is not
                // involved. "Charging", "Full", "Not charging" and "Unknown"
                // all happen on mains.
                if read_trimmed(&p.join("status")).as_deref() == Some("Discharging") {
                    battery_discharging = true;
                }
            }
            _ => {}
        }
    }

    if saw_adapter || battery_discharging {
        Supply::Battery
    } else {
        Supply::Mains
    }
}

// ---------------------------------------------------------------------------
// Applying a preset
// ---------------------------------------------------------------------------

const CPU_ROOT: &str = "/sys/devices/system/cpu";
const PLATFORM_PROFILE: &str = "/sys/firmware/acpi/platform_profile";
const PLATFORM_PROFILE_CHOICES: &str = "/sys/firmware/acpi/platform_profile_choices";
const ASPM_POLICY: &str = "/sys/module/pcie_aspm/parameters/policy";
const HDA_POWER_SAVE: &str = "/sys/module/snd_hda_intel/parameters/power_save";

/// Put the machine in `preset`. Logs what it did at `info` and what it could
/// not do at `debug`; a knob that is not there is normal, not an error.
///
/// Every write is idempotent, so calling this again with the same preset is
/// harmless -- which is what lets the caller apply it on a timer without
/// keeping track of state across a suspend the firmware may have undone.
pub fn apply(preset: Preset) {
    apply_in(Path::new(CPU_ROOT), preset);
    apply_platform(preset);
}

fn apply_in(cpu_root: &Path, preset: Preset) {
    let Some(policies) = policies(cpu_root) else {
        log::debug!("No cpufreq policies under {}; nothing to set", cpu_root.display());
        return;
    };

    let mut summary: Option<String> = None;
    let mut changed = false;
    for policy in &policies {
        let governors = read_trimmed(&policy.join("scaling_available_governors"))
            .unwrap_or_default();
        let governors: Vec<&str> = governors.split_whitespace().collect();
        let has_epp = policy.join("energy_performance_preference").exists();

        let (governor, epp) = choose(preset, &governors, has_epp);

        if let Some(governor) = governor {
            changed |= write_if_different(&policy.join("scaling_governor"), governor);
        }
        if let Some(epp) = epp {
            changed |= write_if_different(&policy.join("energy_performance_preference"), epp);
        }

        if summary.is_none() {
            summary = Some(match (governor, epp) {
                (Some(g), Some(e)) => format!("governor={} epp={}", g, e),
                (Some(g), None) => format!("governor={}", g),
                (None, Some(e)) => format!("epp={}", e),
                (None, None) => "no usable governor".to_string(),
            });
        }
    }

    // Said once when it changes, not every three seconds while it does not.
    let line = format!(
        "Profile {}: {} on {} polic{}",
        preset.name(),
        summary.unwrap_or_default(),
        policies.len(),
        if policies.len() == 1 { "y" } else { "ies" }
    );
    if changed {
        log::info!("{}", line);
    } else {
        log::debug!("{}", line);
    }
}

/// The governor and the energy-performance preference for a preset, given
/// what the policy offers. Either may be `None` when nothing fits.
///
/// With an EPP the firmware picks the frequency and `powersave` is the
/// governor that lets it; the preset goes into the preference. Without one
/// the governor *is* the policy.
fn choose(preset: Preset, governors: &[&str], has_epp: bool) -> (Option<&'static str>, Option<&'static str>) {
    let pick = |wanted: &[&'static str]| wanted.iter().copied().find(|g| governors.contains(g));

    if has_epp {
        let epp = match preset {
            Preset::Performance => "performance",
            Preset::Balanced => "balance_performance",
            Preset::PowerSaver => "balance_power",
        };
        let governor = match preset {
            Preset::Performance => pick(&["performance", "powersave"]),
            // `powersave` here is not "slow": with an EPP it is the governor
            // under which the preference has any effect at all.
            Preset::Balanced | Preset::PowerSaver => pick(&["powersave", "schedutil", "ondemand"]),
        };
        return (governor, Some(epp));
    }

    let governor = match preset {
        Preset::Performance => pick(&["performance", "schedutil", "ondemand"]),
        // `performance` last: on a kernel with nothing that scales, a pinned
        // top clock is at least honest, and it beats `userspace`, which is
        // the same clock waiting for a daemon that is not coming.
        Preset::Balanced | Preset::PowerSaver => {
            pick(&["schedutil", "ondemand", "conservative", "performance"])
        }
    };
    (governor, None)
}

fn apply_platform(preset: Preset) {
    // ACPI platform profile: the vendor's own performance/balanced/quiet
    // switch. Only where the firmware offers it, and only a choice it lists.
    if let Some(choices) = read_trimmed(Path::new(PLATFORM_PROFILE_CHOICES)) {
        let choices: Vec<&str> = choices.split_whitespace().collect();
        let wanted: &[&str] = match preset {
            Preset::Performance => &["performance", "balanced-performance", "balanced"],
            Preset::Balanced => &["balanced", "balanced-performance"],
            Preset::PowerSaver => &["low-power", "quiet", "balanced"],
        };
        if let Some(choice) = wanted.iter().find(|c| choices.contains(c)) {
            write_if_different(Path::new(PLATFORM_PROFILE), choice);
        }
    }

    // PCIe link power management. `powersave` lets idle links drop to L1,
    // which matters on battery and costs latency nobody notices; `default`
    // is whatever the firmware set up, which is the right thing on mains.
    let aspm = match preset {
        Preset::PowerSaver => "powersave",
        Preset::Performance | Preset::Balanced => "default",
    };
    write_bracketed_if_different(Path::new(ASPM_POLICY), aspm);

    // HDA codec power-down after idle, in seconds. Ten is what most
    // distributions ship on battery; zero keeps it awake, which avoids the
    // pop some codecs make coming back.
    let hda = match preset {
        Preset::PowerSaver => "10",
        Preset::Performance | Preset::Balanced => "0",
    };
    write_if_different(Path::new(HDA_POWER_SAVE), hda);
}

/// Every `policyN` directory under `cpufreq`, sorted. `None` if there is no
/// cpufreq at all (a VM without it, or a kernel built without it).
fn policies(cpu_root: &Path) -> Option<Vec<PathBuf>> {
    let dir = cpu_root.join("cpufreq");
    let entries = fs::read_dir(&dir).ok()?;
    let mut out: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("policy") && n[6..].chars().all(|c| c.is_ascii_digit()))
        })
        .collect();
    out.sort();
    Some(out)
}

// ---------------------------------------------------------------------------
// sysfs helpers
// ---------------------------------------------------------------------------

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// Write `value` unless the file already says it. Sysfs writes are not free
/// -- a governor switch tears down and rebuilds the policy -- and this is
/// called on a timer.
fn write_if_different(path: &Path, value: &str) -> bool {
    if read_trimmed(path).as_deref() == Some(value) {
        return false;
    }
    write(path, value)
}

/// Same, for a module parameter that reads back as `default [performance]
/// powersave` -- the current choice in brackets.
fn write_bracketed_if_different(path: &Path, value: &str) -> bool {
    let Some(current) = read_trimmed(path) else {
        return false;
    };
    if bracketed(&current).is_some_and(|c| c == value) {
        return false;
    }
    write(path, value)
}

fn bracketed(text: &str) -> Option<&str> {
    let start = text.find('[')? + 1;
    let end = text[start..].find(']')? + start;
    Some(&text[start..end])
}

/// Returns whether the write went through.
fn write(path: &Path, value: &str) -> bool {
    match fs::write(path, value) {
        Ok(()) => {
            log::info!("{} <- {}", path.display(), value);
            true
        }
        Err(e) if e.kind() == ErrorKind::NotFound => false,
        // EINVAL is a choice this kernel or firmware does not accept -- a
        // governor listed but not loadable, a preference the CPU lacks. Not
        // worth more than a debug line; the fallbacks above already ran.
        Err(e) => {
            log::debug!("{} <- {}: {}", path.display(), value, e);
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epp_driver_gets_powersave_governor_and_a_preference() {
        // intel_pstate active: only two governors, and the preference is
        // the dial that matters.
        let g = ["performance", "powersave"];
        assert_eq!(choose(Preset::Balanced, &g, true), (Some("powersave"), Some("balance_performance")));
        assert_eq!(choose(Preset::PowerSaver, &g, true), (Some("powersave"), Some("balance_power")));
        assert_eq!(choose(Preset::Performance, &g, true), (Some("performance"), Some("performance")));
    }

    #[test]
    fn kernel_governed_driver_prefers_schedutil() {
        // acpi-cpufreq on the Raven kernel: what this machine actually lists.
        let g = ["ondemand", "userspace", "performance", "schedutil"];
        assert_eq!(choose(Preset::Balanced, &g, false), (Some("schedutil"), None));
        assert_eq!(choose(Preset::PowerSaver, &g, false), (Some("schedutil"), None));
        assert_eq!(choose(Preset::Performance, &g, false), (Some("performance"), None));
    }

    #[test]
    fn a_kernel_without_schedutil_falls_back_to_ondemand() {
        let g = ["ondemand", "performance"];
        assert_eq!(choose(Preset::Balanced, &g, false), (Some("ondemand"), None));
    }

    #[test]
    fn userspace_is_never_chosen() {
        // The whole reason this module exists.
        let g = ["userspace", "performance"];
        assert_eq!(choose(Preset::Balanced, &g, false), (Some("performance"), None));
        let g = ["userspace"];
        assert_eq!(choose(Preset::Balanced, &g, false), (None, None));
    }

    #[test]
    fn bracketed_picks_the_current_choice() {
        assert_eq!(bracketed("default [performance] powersave powersupersave"), Some("performance"));
        assert_eq!(bracketed("[default] performance"), Some("default"));
        assert_eq!(bracketed("default performance"), None);
    }

    fn fake_supply(dir: &Path, name: &str, kind: &str, extra: &[(&str, &str)]) {
        let p = dir.join(name);
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("type"), kind).unwrap();
        for (k, v) in extra {
            fs::write(p.join(k), v).unwrap();
        }
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("raven-profile-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn no_supplies_at_all_is_mains() {
        let d = tmp("none");
        assert_eq!(supply_in(&d), Supply::Mains);
        assert_eq!(supply_in(&d.join("missing")), Supply::Mains);
    }

    #[test]
    fn adapter_online_is_mains() {
        let d = tmp("ac");
        fake_supply(&d, "AC0", "Mains", &[("online", "1\n")]);
        fake_supply(&d, "BAT0", "Battery", &[("status", "Charging\n")]);
        assert_eq!(supply_in(&d), Supply::Mains);
    }

    #[test]
    fn adapter_offline_is_battery() {
        let d = tmp("bat");
        fake_supply(&d, "AC0", "Mains", &[("online", "0\n")]);
        fake_supply(&d, "BAT0", "Battery", &[("status", "Discharging\n")]);
        assert_eq!(supply_in(&d), Supply::Battery);
    }

    #[test]
    fn battery_without_an_adapter_node_goes_by_status() {
        let d = tmp("nostat");
        fake_supply(&d, "BAT0", "Battery", &[("status", "Discharging\n")]);
        assert_eq!(supply_in(&d), Supply::Battery);
        let d = tmp("full");
        fake_supply(&d, "BAT0", "Battery", &[("status", "Full\n")]);
        assert_eq!(supply_in(&d), Supply::Mains);
    }

    #[test]
    fn apply_writes_the_governor_to_every_policy() {
        let d = tmp("apply");
        for n in 0..2 {
            let p = d.join("cpufreq").join(format!("policy{n}"));
            fs::create_dir_all(&p).unwrap();
            fs::write(p.join("scaling_available_governors"), "ondemand userspace performance schedutil \n").unwrap();
            fs::write(p.join("scaling_governor"), "userspace\n").unwrap();
        }
        apply_in(&d, Preset::Balanced);
        for n in 0..2 {
            let p = d.join("cpufreq").join(format!("policy{n}"));
            assert_eq!(fs::read_to_string(p.join("scaling_governor")).unwrap(), "schedutil");
        }
    }

    #[test]
    fn shipped_power_toml_has_the_expected_profile() {
        #[derive(Deserialize)]
        struct Top {
            profile: Profile,
        }
        let text = include_str!("../../etc/raven/power.toml");
        let top: Top = toml::from_str(text).expect("etc/raven/power.toml parses");
        assert!(top.profile.manage);
        assert_eq!(top.profile.ac, Preset::Balanced);
        assert_eq!(top.profile.battery, Preset::PowerSaver);
    }
}
