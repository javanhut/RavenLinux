// RavenLinux Installer - GUI installer for RavenLinux
// Uses Gio UI for cross-platform graphics
package main

import (
	"fmt"
	"image/color"
	"log"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"gioui.org/app"
	"gioui.org/font"
	"gioui.org/layout"
	"gioui.org/op"
	"gioui.org/op/clip"
	"gioui.org/op/paint"
	"gioui.org/text"
	"gioui.org/unit"
	"gioui.org/widget"
	"gioui.org/widget/material"
	"github.com/dustin/go-humanize"
)

// Theme colors (Blue and Black)
var (
	colorBackground = color.NRGBA{R: 10, G: 10, B: 15, A: 255}    // Deep black
	colorSurface    = color.NRGBA{R: 20, G: 25, B: 35, A: 255}    // Dark blue-black
	colorPrimary    = color.NRGBA{R: 0, G: 150, B: 255, A: 255}   // Bright blue
	colorAccent     = color.NRGBA{R: 80, G: 200, B: 255, A: 255}  // Cyan/light blue
	colorText       = color.NRGBA{R: 150, G: 180, B: 220, A: 255} // Soft blue-gray
	colorDanger     = color.NRGBA{R: 255, G: 80, B: 80, A: 255}   // Red
)

// Installation steps
const (
	StepWelcome = iota
	StepDiskSelection
	StepPartitioning
	StepConfiguration
	StepInstallation
	StepComplete
)

// Disk represents a storage device
type Disk struct {
	Path   string
	Name   string
	Size   uint64
	Model  string
	Vendor string
}

// InstallerState holds the current installer state
type InstallerState struct {
	currentStep   int
	disks         []Disk
	selectedDisk  int
	hostname      string
	username      string
	password      string
	rootPassword  string
	timezone      string
	locale        string
	installLog    []string
	installDone   bool
	installError  string

	// Widgets
	nextBtn       widget.Clickable
	backBtn       widget.Clickable
	installBtn    widget.Clickable
	refreshBtn    widget.Clickable
	diskList      widget.List
	diskClicks    []widget.Clickable
	hostnameEdit  widget.Editor
	usernameEdit  widget.Editor
	passwordEdit  widget.Editor
	rootPassEdit  widget.Editor
}

func main() {
	go func() {
		w := new(app.Window)
		w.Option(
			app.Title("RavenLinux Installer"),
			app.Size(unit.Dp(900), unit.Dp(600)),
		)

		if err := run(w); err != nil {
			log.Fatal(err)
		}
		os.Exit(0)
	}()
	app.Main()
}

func run(w *app.Window) error {
	th := material.NewTheme()
	th.Palette.Bg = colorBackground
	th.Palette.Fg = colorText
	th.Palette.ContrastBg = colorPrimary
	th.Palette.ContrastFg = colorBackground

	state := &InstallerState{
		currentStep:  StepWelcome,
		selectedDisk: -1,
		hostname:     "raven",
		username:     "raven",
		timezone:     "UTC",
		locale:       "en_US.UTF-8",
	}

	// Initialize editors
	state.hostnameEdit.SetText(state.hostname)
	state.usernameEdit.SetText(state.username)

	// Detect disks
	state.disks = detectDisks()
	state.diskClicks = make([]widget.Clickable, len(state.disks))

	var ops op.Ops
	for {
		switch e := w.Event().(type) {
		case app.DestroyEvent:
			return e.Err
		case app.FrameEvent:
			gtx := app.NewContext(&ops, e)
			drawUI(gtx, th, state, w)
			e.Frame(gtx.Ops)
		}
	}
}

func drawUI(gtx layout.Context, th *material.Theme, state *InstallerState, w *app.Window) layout.Dimensions {
	// Paint background
	paint.FillShape(gtx.Ops, colorBackground, clip.Rect{Max: gtx.Constraints.Max}.Op())

	// Handle button clicks
	if state.nextBtn.Clicked(gtx) {
		if state.currentStep < StepComplete {
			state.currentStep++
			if state.currentStep == StepInstallation {
				go runInstallation(state, w)
			}
		} else if state.currentStep == StepComplete {
			// Reboot button clicked
			exec.Command("reboot").Run()
		}
	}
	if state.backBtn.Clicked(gtx) {
		if state.currentStep > StepWelcome {
			state.currentStep--
		}
	}
	if state.refreshBtn.Clicked(gtx) {
		state.disks = detectDisks()
		state.diskClicks = make([]widget.Clickable, len(state.disks))
	}

	// Handle disk clicks
	for i := range state.diskClicks {
		if state.diskClicks[i].Clicked(gtx) {
			state.selectedDisk = i
		}
	}

	// Main layout
	return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
		// Header
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			return drawHeader(gtx, th, state)
		}),
		// Content
		layout.Flexed(1, func(gtx layout.Context) layout.Dimensions {
			return drawContent(gtx, th, state)
		}),
		// Footer with navigation
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			return drawFooter(gtx, th, state)
		}),
	)
}

func drawHeader(gtx layout.Context, th *material.Theme, state *InstallerState) layout.Dimensions {
	return layout.UniformInset(unit.Dp(20)).Layout(gtx, func(gtx layout.Context) layout.Dimensions {
		return layout.Flex{Axis: layout.Vertical, Alignment: layout.Middle}.Layout(gtx,
			layout.Rigid(func(gtx layout.Context) layout.Dimensions {
				title := material.H4(th, "RavenLinux Installer")
				title.Color = colorPrimary
				title.Alignment = text.Middle
				return title.Layout(gtx)
			}),
			layout.Rigid(layout.Spacer{Height: unit.Dp(10)}.Layout),
			layout.Rigid(func(gtx layout.Context) layout.Dimensions {
				steps := []string{"Welcome", "Disk", "Partitions", "Config", "Install", "Done"}
				return drawProgressBar(gtx, th, state.currentStep, steps)
			}),
		)
	})
}

func drawProgressBar(gtx layout.Context, th *material.Theme, current int, steps []string) layout.Dimensions {
	return layout.Flex{Axis: layout.Horizontal, Spacing: layout.SpaceEvenly}.Layout(gtx,
		func() []layout.FlexChild {
			children := make([]layout.FlexChild, len(steps))
			for i, step := range steps {
				idx := i
				stepName := step
				children[i] = layout.Rigid(func(gtx layout.Context) layout.Dimensions {
					lbl := material.Body2(th, stepName)
					if idx == current {
						lbl.Color = colorAccent
						lbl.Font.Weight = font.Bold
					} else if idx < current {
						lbl.Color = colorPrimary
					} else {
						lbl.Color = color.NRGBA{R: 100, G: 100, B: 100, A: 255}
					}
					return lbl.Layout(gtx)
				})
			}
			return children
		}()...,
	)
}

func drawContent(gtx layout.Context, th *material.Theme, state *InstallerState) layout.Dimensions {
	return layout.UniformInset(unit.Dp(30)).Layout(gtx, func(gtx layout.Context) layout.Dimensions {
		switch state.currentStep {
		case StepWelcome:
			return drawWelcome(gtx, th)
		case StepDiskSelection:
			return drawDiskSelection(gtx, th, state)
		case StepPartitioning:
			return drawPartitioning(gtx, th, state)
		case StepConfiguration:
			return drawConfiguration(gtx, th, state)
		case StepInstallation:
			return drawInstallation(gtx, th, state)
		case StepComplete:
			return drawComplete(gtx, th)
		default:
			return layout.Dimensions{}
		}
	})
}

func drawWelcome(gtx layout.Context, th *material.Theme) layout.Dimensions {
	return layout.Flex{Axis: layout.Vertical, Alignment: layout.Middle}.Layout(gtx,
		layout.Rigid(layout.Spacer{Height: unit.Dp(20)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			title := material.H2(th, "RAVEN LINUX")
			title.Color = colorAccent
			title.Alignment = text.Middle
			return title.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(10)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			subtitle := material.H6(th, "- INSTALLER -")
			subtitle.Color = colorPrimary
			subtitle.Alignment = text.Middle
			return subtitle.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(40)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			welcome := material.H5(th, "Welcome!")
			welcome.Color = colorText
			welcome.Alignment = text.Middle
			return welcome.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(20)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			desc := material.Body1(th, `This installer will guide you through installing RavenLinux.

A modern, developer-focused Linux distribution featuring:`)
			desc.Color = colorText
			desc.Alignment = text.Middle
			return desc.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(15)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
				features := material.Body1(th, `Vem - GPU-accelerated text editor
Carrion - Modern programming language
Ivaldi - Next-generation version control
rvn - Raven package manager
Bash with custom configuration`)
			features.Color = colorAccent
			features.Alignment = text.Middle
			return features.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(30)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			next := material.Body2(th, "Click \"Next\" to begin.")
			next.Color = colorText
			next.Alignment = text.Middle
			return next.Layout(gtx)
		}),
	)
}

func drawDiskSelection(gtx layout.Context, th *material.Theme, state *InstallerState) layout.Dimensions {
	return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			title := material.H6(th, "Select Installation Disk")
			return title.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(10)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			warn := material.Body2(th, "⚠ Warning: The selected disk will be erased. Make sure to backup important data.")
			warn.Color = colorDanger
			return warn.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(20)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			btn := material.Button(th, &state.refreshBtn, "Refresh Disks")
			btn.Background = colorSurface
			return btn.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(10)}.Layout),
		layout.Flexed(1, func(gtx layout.Context) layout.Dimensions {
			return material.List(th, &state.diskList).Layout(gtx, len(state.disks), func(gtx layout.Context, i int) layout.Dimensions {
				disk := state.disks[i]
				return layout.UniformInset(unit.Dp(5)).Layout(gtx, func(gtx layout.Context) layout.Dimensions {
					selected := i == state.selectedDisk
					bg := colorSurface
					if selected {
						bg = colorPrimary
					}

					return material.Clickable(gtx, &state.diskClicks[i], func(gtx layout.Context) layout.Dimensions {
						return widget.Border{
							Color: bg,
							Width: unit.Dp(2),
						}.Layout(gtx, func(gtx layout.Context) layout.Dimensions {
							return layout.UniformInset(unit.Dp(15)).Layout(gtx, func(gtx layout.Context) layout.Dimensions {
								return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
									layout.Rigid(func(gtx layout.Context) layout.Dimensions {
										name := material.Body1(th, fmt.Sprintf("%s - %s", disk.Path, disk.Model))
										if selected {
											name.Font.Weight = font.Bold
										}
										return name.Layout(gtx)
									}),
									layout.Rigid(func(gtx layout.Context) layout.Dimensions {
										size := material.Body2(th, fmt.Sprintf("Size: %s | Vendor: %s", humanize.Bytes(disk.Size), disk.Vendor))
										size.Color = color.NRGBA{R: 150, G: 150, B: 150, A: 255}
										return size.Layout(gtx)
									}),
								)
							})
						})
					})
				})
			})
		}),
	)
}

func drawPartitioning(gtx layout.Context, th *material.Theme, state *InstallerState) layout.Dimensions {
	return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			title := material.H6(th, "Partition Layout")
			return title.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(20)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			desc := material.Body1(th, `The following partition layout will be created:

  /dev/sdX1 - EFI System Partition (512 MB, FAT32)
  /dev/sdX2 - Root Partition (Remaining space, ext4)

This uses a simple GPT layout suitable for UEFI systems.
For advanced partitioning, use manual installation.`)
			return desc.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(30)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			if state.selectedDisk >= 0 && state.selectedDisk < len(state.disks) {
				disk := state.disks[state.selectedDisk]
				info := material.Body1(th, fmt.Sprintf("Selected: %s (%s)", disk.Path, humanize.Bytes(disk.Size)))
				info.Color = colorAccent
				return info.Layout(gtx)
			}
			warn := material.Body1(th, "No disk selected!")
			warn.Color = colorDanger
			return warn.Layout(gtx)
		}),
	)
}

func drawConfiguration(gtx layout.Context, th *material.Theme, state *InstallerState) layout.Dimensions {
	// Update state from editors
	state.hostname = state.hostnameEdit.Text()
	state.username = state.usernameEdit.Text()
	state.password = state.passwordEdit.Text()
	state.rootPassword = state.rootPassEdit.Text()

	return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			title := material.H6(th, "System Configuration")
			return title.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(20)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			return drawFormField(gtx, th, "Hostname:", &state.hostnameEdit)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(15)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			return drawFormField(gtx, th, "Username:", &state.usernameEdit)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(15)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			return drawFormField(gtx, th, "Password:", &state.passwordEdit)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(15)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			return drawFormField(gtx, th, "Root Password:", &state.rootPassEdit)
		}),
	)
}

func drawFormField(gtx layout.Context, th *material.Theme, label string, editor *widget.Editor) layout.Dimensions {
	return layout.Flex{Axis: layout.Horizontal, Alignment: layout.Middle}.Layout(gtx,
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			gtx.Constraints.Min.X = gtx.Dp(unit.Dp(150))
			lbl := material.Body1(th, label)
			return lbl.Layout(gtx)
		}),
		layout.Flexed(1, func(gtx layout.Context) layout.Dimensions {
			ed := material.Editor(th, editor, "")
			ed.Color = colorText
			return widget.Border{
				Color: colorSurface,
				Width: unit.Dp(1),
			}.Layout(gtx, func(gtx layout.Context) layout.Dimensions {
				return layout.UniformInset(unit.Dp(10)).Layout(gtx, ed.Layout)
			})
		}),
	)
}

func drawInstallation(gtx layout.Context, th *material.Theme, state *InstallerState) layout.Dimensions {
	return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			title := material.H6(th, "Installing RavenLinux...")
			return title.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(20)}.Layout),
		layout.Flexed(1, func(gtx layout.Context) layout.Dimensions {
			logText := strings.Join(state.installLog, "\n")
			lbl := material.Body2(th, logText)
			lbl.Color = colorText
			return lbl.Layout(gtx)
		}),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			if state.installError != "" {
				err := material.Body1(th, "Error: "+state.installError)
				err.Color = colorDanger
				return err.Layout(gtx)
			}
			return layout.Dimensions{}
		}),
	)
}

func drawComplete(gtx layout.Context, th *material.Theme) layout.Dimensions {
	return layout.Flex{Axis: layout.Vertical, Alignment: layout.Middle}.Layout(gtx,
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			check := material.H3(th, "✓")
			check.Color = colorAccent
			return check.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(20)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			title := material.H5(th, "Installation Complete!")
			title.Color = colorAccent
			title.Alignment = text.Middle
			return title.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(20)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			desc := material.Body1(th, `RavenLinux has been successfully installed!

Please remove the installation media and reboot your computer
to start using RavenLinux.

Thank you for choosing RavenLinux!`)
			desc.Alignment = text.Middle
			return desc.Layout(gtx)
		}),
	)
}

func drawFooter(gtx layout.Context, th *material.Theme, state *InstallerState) layout.Dimensions {
	return layout.UniformInset(unit.Dp(20)).Layout(gtx, func(gtx layout.Context) layout.Dimensions {
		return layout.Flex{Axis: layout.Horizontal, Spacing: layout.SpaceBetween}.Layout(gtx,
			layout.Rigid(func(gtx layout.Context) layout.Dimensions {
				if state.currentStep > StepWelcome && state.currentStep < StepInstallation {
					btn := material.Button(th, &state.backBtn, "Back")
					btn.Background = colorSurface
					return btn.Layout(gtx)
				}
				return layout.Dimensions{}
			}),
			layout.Rigid(func(gtx layout.Context) layout.Dimensions {
				if state.currentStep < StepInstallation {
					btn := material.Button(th, &state.nextBtn, "Next")
					btn.Background = colorPrimary
					return btn.Layout(gtx)
				} else if state.currentStep == StepComplete {
					btn := material.Button(th, &state.nextBtn, "Reboot")
					btn.Background = colorAccent
					return btn.Layout(gtx)
				}
				return layout.Dimensions{}
			}),
		)
	})
}

// detectDisks finds available storage devices
func detectDisks() []Disk {
	var disks []Disk

	// Read from /sys/block
	entries, err := os.ReadDir("/sys/block")
	if err != nil {
		return disks
	}

	for _, entry := range entries {
		name := entry.Name()
		// Skip loop, ram, and other virtual devices
		if strings.HasPrefix(name, "loop") || strings.HasPrefix(name, "ram") ||
			strings.HasPrefix(name, "sr") || strings.HasPrefix(name, "dm-") {
			continue
		}

		path := filepath.Join("/dev", name)
		sysPath := filepath.Join("/sys/block", name)

		// Read size (in 512-byte sectors)
		sizeBytes, _ := os.ReadFile(filepath.Join(sysPath, "size"))
		size := parseUint(strings.TrimSpace(string(sizeBytes))) * 512

		// Skip very small devices (< 1GB)
		if size < 1024*1024*1024 {
			continue
		}

		// Read model
		modelBytes, _ := os.ReadFile(filepath.Join(sysPath, "device/model"))
		model := strings.TrimSpace(string(modelBytes))
		if model == "" {
			model = "Unknown"
		}

		// Read vendor
		vendorBytes, _ := os.ReadFile(filepath.Join(sysPath, "device/vendor"))
		vendor := strings.TrimSpace(string(vendorBytes))
		if vendor == "" {
			vendor = "Unknown"
		}

		disks = append(disks, Disk{
			Path:   path,
			Name:   name,
			Size:   size,
			Model:  model,
			Vendor: vendor,
		})
	}

	return disks
}

func parseUint(s string) uint64 {
	var n uint64
	fmt.Sscanf(s, "%d", &n)
	return n
}

// runInstallation performs the actual installation
func runInstallation(state *InstallerState, w *app.Window) {
	addLog := func(msg string) {
		state.installLog = append(state.installLog, msg)
		w.Invalidate()
	}

	addLog("Starting installation...")

	if state.selectedDisk < 0 || state.selectedDisk >= len(state.disks) {
		state.installError = "No disk selected"
		w.Invalidate()
		return
	}

	disk := state.disks[state.selectedDisk]
	addLog(fmt.Sprintf("Target disk: %s", disk.Path))

	// In a real installer, these would perform actual operations
	// For now, we simulate the process
	steps := []string{
		"Creating partition table...",
		"Creating EFI partition...",
		"Creating root partition...",
		"Formatting EFI partition (FAT32)...",
		"Formatting root partition (ext4)...",
		"Mounting partitions...",
		"Copying system files...",
		"Installing bootloader...",
		"Configuring system...",
		"Setting up users...",
		"Finalizing installation...",
	}

	for _, step := range steps {
		addLog(step)
		// Simulate work
		exec.Command("sleep", "1").Run()
	}

	addLog("Installation complete!")
	state.installDone = true
	state.currentStep = StepComplete
	w.Invalidate()
}
