//! Renders the boot menu to PNG files, on the host.
//!
//! The menu is drawn by firmware, at a resolution the firmware picked, on a
//! machine that has to be rebooted to show it again. That is a bad loop to
//! design in, and a worse one to review a colour change in. So `gfx`, `font`
//! and `menu` are all free of UEFI — see the note at the top of `gfx.rs` — and
//! this includes them directly and renders the same frames the firmware would.
//!
//! ```text
//! cargo run --release --manifest-path preview/Cargo.toml
//! ```
//!
//! writes one PNG per case into `preview/out/`. What it cannot check is
//! anything on the far side of `blt`: a firmware whose reported resolution
//! lies, or a panel whose gamma is nothing like this monitor's.

extern crate alloc;

#[path = "../../src/theme.rs"]
mod theme;

#[path = "../../src/gfx.rs"]
mod gfx;

#[path = "../../src/font.rs"]
mod font;

#[path = "../../src/menu.rs"]
mod menu;

mod grub;
mod grubsim;
mod png;

use menu::{Row, RowKind, Status, Tone, View};

/// The panel sizes worth looking at, and the scale `screen::scale_for` gives
/// each. Kept in step with that function by hand — the preview showing a scale
/// the firmware would never choose is a wrong preview.
const PANELS: &[(usize, usize, f32)] = &[
    (1024, 768, 1.0),  // the smallest thing firmware commonly reports
    (1920, 1080, 1.0), // the baseline every metric is written against
    (2560, 1440, 1.5),
    (3840, 2160, 2.0),
];

/// Filler for the overflow case. Real enough to judge the layout against.
const LABELS: &[&str] = &[
    "Raven Linux",
    "Windows",
    "Ubuntu",
    "Fedora",
    "Debian",
    "Arch Linux",
    "openSUSE",
    "Pop!_OS",
    "Linux Mint",
    "Rocky Linux",
    "CentOS",
    "Manjaro",
    "A boot entry with a name far longer than the card is wide",
];

fn main() {
    // `cargo run -- grub-theme [dir]` writes the BIOS menu's theme instead of
    // the previews. Same renderer, so the two menus cannot drift apart.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("grub-theme") {
        let dir = args.get(1).map_or_else(
            || std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../configs/grub/theme"),
            std::path::PathBuf::from,
        );
        grub::generate(&dir).expect("cannot write the GRUB theme");
        println!("{}", dir.display());

        // Render what the theme composes to, at the sizes a BIOS machine is
        // likely to hand GRUB. See `grubsim` for what this does and does not
        // prove.
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("out");
        std::fs::create_dir_all(&out).expect("cannot create preview/out");
        for (w, h) in [(1024usize, 768usize), (1920, 1080)] {
            let path = out.join(format!("grub-{w}x{h}.png"));
            grubsim::render(&dir, &path, w, h).expect("cannot render the GRUB theme");
            println!("{}", path.display());
        }
        return;
    }

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("out");
    std::fs::create_dir_all(&out).expect("cannot create preview/out");

    let root = [
        Row {
            label: "Raven Linux",
            kind: RowKind::Action,
        },
        Row {
            label: "Raven Linux (Serial) >",
            kind: RowKind::Submenu,
        },
        Row {
            label: "Raven Linux (Graphical) >",
            kind: RowKind::Submenu,
        },
        Row {
            label: "Recovery >",
            kind: RowKind::Submenu,
        },
        Row {
            label: "System >",
            kind: RowKind::Submenu,
        },
    ];

    // `cargo run -- 2560x1080 800x600 ...` renders the root menu at sizes that
    // are not in PANELS. The layout is computed from the canvas rather than
    // chosen from a table, so the useful check is an awkward shape nobody
    // designed against: an ultrawide, a portrait panel, something too short for
    // the rows to fit.
    if !args.is_empty() {
        for spec in &args {
            let (w, h) = spec
                .split_once(['x', 'X'])
                .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                .unwrap_or_else(|| panic!("bad size '{spec}' (expected WIDTHxHEIGHT)"));

            // The same rule `screen::scale_for` applies, kept in step by hand.
            let scale = match h {
                0..=1152 => 1.0,
                1153..=1800 => 1.5,
                _ => 2.0,
            };

            let view = View {
                rows: &root,
                selected: 0,
                prompt: "Select an operating system to boot",
                status: Some(Status {
                    text: "Booting Raven Linux in 5s",
                    tone: Tone::Caution,
                }),
                version: "RavenBoot 0.1.0",
                can_go_back: false,
            };

            let mut canvas = gfx::Canvas::new(w, h);
            menu::draw(&mut canvas, &view, scale);
            let path = out.join(format!("size-{w}x{h}.png"));
            png::write(&path, &canvas).expect("cannot write PNG");
            println!("{}   scale {scale}", path.display());
        }
        return;
    }

    let system = [
        Row {
            label: "System UEFI Settings",
            kind: RowKind::Action,
        },
        Row {
            label: "UEFI Shell",
            kind: RowKind::Action,
        },
        Row {
            label: "Reboot",
            kind: RowKind::Action,
        },
        Row {
            label: "Shutdown",
            kind: RowKind::Action,
        },
        Row {
            label: "< Back",
            kind: RowKind::Back,
        },
    ];

    // A machine that dual-boots, which is where the row count actually gets
    // interesting: `detect_other_os` appends one row per bootloader it finds.
    let crowded: Vec<Row> = {
        let mut rows: Vec<Row> = root
            .iter()
            .map(|r| Row {
                label: r.label,
                kind: r.kind,
            })
            .collect();
        for label in [
            "Windows",
            "Ubuntu",
            "Fedora",
            "Debian",
            "Arch Linux",
            "openSUSE",
            "Pop!_OS",
            "Linux Mint",
            "Rocky Linux",
            "CentOS",
            "Manjaro",
        ] {
            rows.push(Row {
                label,
                kind: RowKind::Action,
            });
        }
        rows
    };

    // More rows than fit on a 1080p panel, which is the only way to see the
    // scroll window and its chevrons. Reachable in practice: `MAX_ENTRIES` is
    // 16 and `detect_other_os` appends on top of that.
    let overflowing: Vec<Row> = (0..26)
        .map(|i| Row {
            label: LABELS[i % LABELS.len()],
            kind: RowKind::Action,
        })
        .collect();

    let cases: &[(&str, View)] = &[
        (
            "root",
            View {
                rows: &root,
                selected: 0,
                prompt: "Select an operating system to boot",
                status: Some(Status {
                    text: "Booting Raven Linux in 5s",
                    tone: Tone::Caution,
                }),
                version: "RavenBoot 0.1.0",
                can_go_back: false,
            },
        ),
        (
            "root-no-countdown",
            View {
                rows: &root,
                selected: 2,
                prompt: "Select an operating system to boot",
                status: None,
                version: "RavenBoot 0.1.0",
                can_go_back: false,
            },
        ),
        (
            "submenu",
            View {
                rows: &system,
                selected: 1,
                prompt: "System",
                status: None,
                version: "RavenBoot 0.1.0",
                can_go_back: true,
            },
        ),
        (
            "failure",
            View {
                rows: &root,
                selected: 0,
                prompt: "Select an operating system to boot",
                status: Some(Status {
                    text: "Boot failed: KernelNotFound. Press any key.",
                    tone: Tone::Failure,
                }),
                version: "RavenBoot 0.1.0",
                can_go_back: false,
            },
        ),
        (
            "scrolling",
            View {
                rows: &overflowing,
                selected: 18,
                prompt: "Select an operating system to boot",
                status: None,
                version: "RavenBoot 0.1.0",
                can_go_back: false,
            },
        ),
        (
            "crowded",
            View {
                rows: &crowded,
                selected: 9,
                prompt: "Select an operating system to boot",
                status: None,
                version: "RavenBoot 0.1.0",
                can_go_back: false,
            },
        ),
    ];

    // The mark on its own and very large, because it is the one element that is
    // a drawing rather than a layout, and 56 pixels is too small to judge a
    // silhouette in.
    {
        let mut canvas = gfx::Canvas::new(420, 420);
        canvas.gradient(theme::BACKDROP, theme::BACKDROP_EDGE);
        menu::draw_mark(&mut canvas, 210.0, 210.0, 340.0);
        let path = out.join("mark.png");
        png::write(&path, &canvas).expect("cannot write PNG");
        println!("{}", path.display());
    }

    for (name, view) in cases {
        for &(w, h, scale) in PANELS {
            // Only the baseline panel gets every case. The others are there to
            // check that the metrics scale, which one case shows as well as
            // five.
            if *name != "root" && (w, h) != (1920, 1080) {
                continue;
            }
            let mut canvas = gfx::Canvas::new(w, h);
            menu::draw(&mut canvas, view, scale);

            let path = out.join(format!("{name}-{w}x{h}.png"));
            png::write(&path, &canvas).expect("cannot write PNG");
            println!("{}", path.display());
        }
    }
}
