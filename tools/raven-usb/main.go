// RavenLinux USB Creator - GUI tool to create bootable USB drives
// Works with RavenLinux and any other Linux distribution ISO
package main

import (
	"fmt"
	"image"
	"image/color"
	"io"
	"log"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"syscall"
	"time"

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
)

// Theme colors (Blue and Black)
var (
	colorBackground = color.NRGBA{R: 10, G: 10, B: 15, A: 255}
	colorSurface    = color.NRGBA{R: 20, G: 25, B: 35, A: 255}
	colorPrimary    = color.NRGBA{R: 0, G: 150, B: 255, A: 255}
	colorAccent     = color.NRGBA{R: 80, G: 200, B: 255, A: 255}
	colorText       = color.NRGBA{R: 150, G: 180, B: 220, A: 255}
	colorTextBright = color.NRGBA{R: 220, G: 230, B: 245, A: 255}
	colorDanger     = color.NRGBA{R: 255, G: 80, B: 80, A: 255}
	colorSuccess    = color.NRGBA{R: 80, G: 200, B: 120, A: 255}
	colorWarning    = color.NRGBA{R: 255, G: 200, B: 80, A: 255}
	colorDisabled   = color.NRGBA{R: 60, G: 60, B: 70, A: 255}
)

// Steps/Pages
const (
	PageSelectUSB = iota
	PageFormat
	PageSelectISO
	PageConfirm
	PageWriting
	PageComplete
)

// USBDevice represents a USB storage device
type USBDevice struct {
	Path      string
	Name      string
	Size      uint64
	Model     string
	Vendor    string
	Removable bool
}

// AppState holds the application state
type AppState struct {
	currentPage  int
	devices      []USBDevice
	selectedUSB  int
	isoPath      string
	isoSize      uint64
	progress     float64
	progressTxt  string
	statusLog    []string
	writeError   string
	writeDone    bool
	isRoot       bool
	formatUSBOpt bool
	writeSpeed   float64 // bytes per second for time estimation
	startTime    time.Time
	etaText      string

	mu sync.Mutex

	// Widgets
	refreshBtn   widget.Clickable
	browseBtn    widget.Clickable
	formatCheck  widget.Bool
	nextBtn      widget.Clickable
	backBtn      widget.Clickable
	exitBtn      widget.Clickable
	startOverBtn widget.Clickable
	deviceClicks []widget.Clickable
	confirmCheck widget.Bool

	// Scroll states for pages
	confirmScroll widget.List
	writingScroll widget.List
	completeScroll widget.List
}

func main() {
	go func() {
		w := new(app.Window)
		w.Option(
			app.Title("Raven USB Creator"),
			app.Size(unit.Dp(700), unit.Dp(500)),
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

	state := &AppState{
		currentPage: PageSelectUSB,
		selectedUSB: -1,
		isRoot:      os.Geteuid() == 0,
		confirmScroll: widget.List{List: layout.List{Axis: layout.Vertical}},
		writingScroll: widget.List{List: layout.List{Axis: layout.Vertical}},
		completeScroll: widget.List{List: layout.List{Axis: layout.Vertical}},
	}

	// Check command line for ISO path
	if len(os.Args) > 1 {
		path := os.Args[1]
		if info, err := os.Stat(path); err == nil && strings.HasSuffix(strings.ToLower(path), ".iso") {
			state.isoPath = path
			state.isoSize = uint64(info.Size())
		}
	}

	// Initial device scan
	state.devices = detectUSBDevices()
	state.deviceClicks = make([]widget.Clickable, len(state.devices))

	go autoScan(state, w)

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

func drawUI(gtx layout.Context, th *material.Theme, state *AppState, w *app.Window) layout.Dimensions {
	state.mu.Lock()
	defer state.mu.Unlock()

	paint.FillShape(gtx.Ops, colorBackground, clip.Rect{Max: gtx.Constraints.Max}.Op())

	// Handle button clicks
	if state.refreshBtn.Clicked(gtx) {
		state.devices = detectUSBDevices()
		state.deviceClicks = make([]widget.Clickable, len(state.devices))
		state.selectedUSB = -1
	}

	// Handle device selection
	for i := range state.deviceClicks {
		if state.deviceClicks[i].Clicked(gtx) {
			state.selectedUSB = i
		}
	}

	if state.browseBtn.Clicked(gtx) {
		go func() {
			isoPath := browseForISO()
			if isoPath != "" {
				if info, err := os.Stat(isoPath); err == nil {
					state.mu.Lock()
					state.isoPath = isoPath
					state.isoSize = uint64(info.Size())
					state.mu.Unlock()
					w.Invalidate()
				}
			}
		}()
	}

	if state.backBtn.Clicked(gtx) {
		if state.currentPage > PageSelectUSB && state.currentPage < PageWriting {
			state.currentPage--
		}
	}

	if state.nextBtn.Clicked(gtx) {
		switch state.currentPage {
		case PageSelectUSB:
			if state.selectedUSB >= 0 {
				state.currentPage = PageFormat
			}
		case PageFormat:
			state.formatUSBOpt = state.formatCheck.Value
			state.currentPage = PageSelectISO
		case PageSelectISO:
			if state.isoPath != "" && state.selectedUSB >= 0 {
				dev := state.devices[state.selectedUSB]
				if dev.Size >= state.isoSize {
					state.currentPage = PageConfirm
					state.confirmCheck.Value = false
				}
			}
		case PageConfirm:
			if state.confirmCheck.Value {
				state.currentPage = PageWriting
				state.progress = 0
				state.statusLog = nil
				state.writeError = ""
				state.etaText = ""
				go writeToUSB(state, w)
			}
		}
	}

	if state.exitBtn.Clicked(gtx) {
		os.Exit(0)
	}

	if state.startOverBtn.Clicked(gtx) {
		state.currentPage = PageSelectUSB
		state.selectedUSB = -1
		state.isoPath = ""
		state.isoSize = 0
		state.progress = 0
		state.writeError = ""
		state.writeDone = false
		state.formatUSBOpt = false
		state.formatCheck.Value = false
		state.confirmCheck.Value = false
	}

	// Main layout
	return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			return drawHeader(gtx, th, state)
		}),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			return drawStepIndicator(gtx, th, state)
		}),
		layout.Flexed(1, func(gtx layout.Context) layout.Dimensions {
			return drawContent(gtx, th, state)
		}),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			return drawFooter(gtx, th, state)
		}),
	)
}

func drawHeader(gtx layout.Context, th *material.Theme, state *AppState) layout.Dimensions {
	return layout.UniformInset(unit.Dp(15)).Layout(gtx, func(gtx layout.Context) layout.Dimensions {
		title := material.H5(th, "RAVEN USB CREATOR")
		title.Color = colorAccent
		title.Alignment = text.Middle
		return title.Layout(gtx)
	})
}

func drawStepIndicator(gtx layout.Context, th *material.Theme, state *AppState) layout.Dimensions {
	steps := []string{"USB", "Format", "ISO", "Confirm", "Write"}
	currentStep := state.currentPage
	if currentStep > 4 {
		currentStep = 4
	}

	return layout.Inset{Left: unit.Dp(20), Right: unit.Dp(20), Bottom: unit.Dp(15)}.Layout(gtx, func(gtx layout.Context) layout.Dimensions {
		return layout.Flex{Axis: layout.Horizontal, Spacing: layout.SpaceEvenly}.Layout(gtx,
			func() []layout.FlexChild {
				var children []layout.FlexChild
				for i, step := range steps {
					idx := i
					s := step
					children = append(children, layout.Rigid(func(gtx layout.Context) layout.Dimensions {
						col := colorDisabled
						if idx < currentStep {
							col = colorSuccess
						} else if idx == currentStep {
							col = colorPrimary
						}

						return layout.Flex{Axis: layout.Horizontal, Alignment: layout.Middle}.Layout(gtx,
							layout.Rigid(func(gtx layout.Context) layout.Dimensions {
								// Circle
								size := gtx.Dp(unit.Dp(24))
								defer clip.Ellipse{Max: image.Pt(size, size)}.Push(gtx.Ops).Pop()
								paint.ColorOp{Color: col}.Add(gtx.Ops)
								paint.PaintOp{}.Add(gtx.Ops)

								// Number
								lbl := material.Body2(th, fmt.Sprintf("%d", idx+1))
								lbl.Color = colorBackground
								lbl.Alignment = text.Middle
								layout.Center.Layout(gtx, func(gtx layout.Context) layout.Dimensions {
									gtx.Constraints.Min = image.Pt(size, size)
									gtx.Constraints.Max = gtx.Constraints.Min
									return layout.Center.Layout(gtx, lbl.Layout)
								})
								return layout.Dimensions{Size: image.Pt(size, size)}
							}),
							layout.Rigid(layout.Spacer{Width: unit.Dp(6)}.Layout),
							layout.Rigid(func(gtx layout.Context) layout.Dimensions {
								lbl := material.Caption(th, s)
								lbl.Color = col
								return lbl.Layout(gtx)
							}),
						)
					}))
				}
				return children
			}()...,
		)
	})
}

func drawContent(gtx layout.Context, th *material.Theme, state *AppState) layout.Dimensions {
	return layout.UniformInset(unit.Dp(20)).Layout(gtx, func(gtx layout.Context) layout.Dimensions {
		if !state.isRoot {
			return drawNotRoot(gtx, th)
		}

		switch state.currentPage {
		case PageSelectUSB:
			return drawPageSelectUSB(gtx, th, state)
		case PageFormat:
			return drawPageFormat(gtx, th, state)
		case PageSelectISO:
			return drawPageSelectISO(gtx, th, state)
		case PageConfirm:
			return drawPageConfirm(gtx, th, state)
		case PageWriting:
			return drawPageWriting(gtx, th, state)
		case PageComplete:
			return drawPageComplete(gtx, th, state)
		}
		return layout.Dimensions{}
	})
}

func drawNotRoot(gtx layout.Context, th *material.Theme) layout.Dimensions {
	return layout.Flex{Axis: layout.Vertical, Alignment: layout.Middle}.Layout(gtx,
		layout.Rigid(layout.Spacer{Height: unit.Dp(40)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			icon := material.H1(th, "!")
			icon.Color = colorDanger
			return icon.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(20)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			msg := material.H5(th, "Root Access Required")
			msg.Color = colorDanger
			msg.Alignment = text.Middle
			return msg.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(15)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			msg := material.Body1(th, "Please run this tool with sudo:\n\nsudo raven-usb")
			msg.Color = colorText
			msg.Alignment = text.Middle
			return msg.Layout(gtx)
		}),
	)
}

// Page 1: Select USB Device
func drawPageSelectUSB(gtx layout.Context, th *material.Theme, state *AppState) layout.Dimensions {
	return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			title := material.H6(th, "Select USB Device")
			title.Color = colorTextBright
			return title.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(5)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			subtitle := material.Body2(th, "Choose the USB drive to make bootable")
			subtitle.Color = colorText
			return subtitle.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(15)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			return layout.Flex{Axis: layout.Horizontal}.Layout(gtx,
				layout.Flexed(1, func(gtx layout.Context) layout.Dimensions {
					return layout.Dimensions{}
				}),
				layout.Rigid(func(gtx layout.Context) layout.Dimensions {
					btn := material.Button(th, &state.refreshBtn, "Refresh")
					btn.Background = colorSurface
					btn.Color = colorText
					return btn.Layout(gtx)
				}),
			)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(10)}.Layout),
		layout.Flexed(1, func(gtx layout.Context) layout.Dimensions {
			return drawUSBList(gtx, th, state)
		}),
	)
}

func drawUSBList(gtx layout.Context, th *material.Theme, state *AppState) layout.Dimensions {
	return widget.Border{
		Color: colorSurface,
		Width: unit.Dp(2),
	}.Layout(gtx, func(gtx layout.Context) layout.Dimensions {
		paint.FillShape(gtx.Ops, colorSurface, clip.Rect{Max: gtx.Constraints.Max}.Op())

		if len(state.devices) == 0 {
			return layout.Center.Layout(gtx, func(gtx layout.Context) layout.Dimensions {
				return layout.Flex{Axis: layout.Vertical, Alignment: layout.Middle}.Layout(gtx,
					layout.Rigid(func(gtx layout.Context) layout.Dimensions {
						msg := material.Body1(th, "No USB devices found")
						msg.Color = colorDisabled
						return msg.Layout(gtx)
					}),
					layout.Rigid(layout.Spacer{Height: unit.Dp(10)}.Layout),
					layout.Rigid(func(gtx layout.Context) layout.Dimensions {
						hint := material.Caption(th, "Insert a USB drive and click Refresh")
						hint.Color = colorDisabled
						return hint.Layout(gtx)
					}),
				)
			})
		}

		list := layout.List{Axis: layout.Vertical}
		return list.Layout(gtx, len(state.devices), func(gtx layout.Context, i int) layout.Dimensions {
			dev := state.devices[i]

			bg := colorSurface
			textColor := colorText
			borderColor := colorSurface
			if i == state.selectedUSB {
				bg = color.NRGBA{R: 0, G: 80, B: 150, A: 255}
				textColor = colorTextBright
				borderColor = colorPrimary
			}

			btn := &state.deviceClicks[i]
			return btn.Layout(gtx, func(gtx layout.Context) layout.Dimensions {
				return widget.Border{
					Color: borderColor,
					Width: unit.Dp(1),
				}.Layout(gtx, func(gtx layout.Context) layout.Dimensions {
					macro := op.Record(gtx.Ops)
					dims := layout.UniformInset(unit.Dp(15)).Layout(gtx, func(gtx layout.Context) layout.Dimensions {
						return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
							layout.Rigid(func(gtx layout.Context) layout.Dimensions {
								name := material.Body1(th, fmt.Sprintf("%s %s", dev.Vendor, dev.Model))
								name.Color = textColor
								name.Font.Weight = font.Bold
								return name.Layout(gtx)
							}),
							layout.Rigid(layout.Spacer{Height: unit.Dp(4)}.Layout),
							layout.Rigid(func(gtx layout.Context) layout.Dimensions {
								info := material.Caption(th, fmt.Sprintf("%s  -  %s", dev.Path, formatSize(dev.Size)))
								info.Color = colorText
								return info.Layout(gtx)
							}),
						)
					})
					call := macro.Stop()

					defer clip.Rect{Max: dims.Size}.Push(gtx.Ops).Pop()
					paint.FillShape(gtx.Ops, bg, clip.Rect{Max: dims.Size}.Op())
					call.Add(gtx.Ops)
					return dims
				})
			})
		})
	})
}

// Page 2: Format Option
func drawPageFormat(gtx layout.Context, th *material.Theme, state *AppState) layout.Dimensions {
	dev := state.devices[state.selectedUSB]

	return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			title := material.H6(th, "Format USB (Optional)")
			title.Color = colorTextBright
			return title.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(5)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			subtitle := material.Body2(th, "Format the USB before writing the ISO")
			subtitle.Color = colorText
			return subtitle.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(25)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			return drawInfoBox(gtx, th, "Selected USB", fmt.Sprintf("%s %s\n%s - %s", dev.Vendor, dev.Model, dev.Path, formatSize(dev.Size)))
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(25)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			return widget.Border{
				Color: colorSurface,
				Width: unit.Dp(2),
			}.Layout(gtx, func(gtx layout.Context) layout.Dimensions {
				return layout.UniformInset(unit.Dp(20)).Layout(gtx, func(gtx layout.Context) layout.Dimensions {
					return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
						layout.Rigid(func(gtx layout.Context) layout.Dimensions {
							cb := material.CheckBox(th, &state.formatCheck, "Format USB before writing")
							cb.Color = colorText
							return cb.Layout(gtx)
						}),
						layout.Rigid(layout.Spacer{Height: unit.Dp(10)}.Layout),
						layout.Rigid(func(gtx layout.Context) layout.Dimensions {
							note := material.Caption(th, "Creates a new FAT32 partition. Only needed if USB has issues or wrong format.")
							note.Color = colorDisabled
							return note.Layout(gtx)
						}),
					)
				})
			})
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(20)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			warn := material.Caption(th, "Note: Formatting is optional. Most ISOs will work without formatting.")
			warn.Color = colorWarning
			return warn.Layout(gtx)
		}),
	)
}

// Page 3: Select ISO
func drawPageSelectISO(gtx layout.Context, th *material.Theme, state *AppState) layout.Dimensions {
	dev := state.devices[state.selectedUSB]

	return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			title := material.H6(th, "Select ISO Image")
			title.Color = colorTextBright
			return title.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(5)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			subtitle := material.Body2(th, "Choose the Linux ISO to write to the USB")
			subtitle.Color = colorText
			return subtitle.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(25)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			return layout.Flex{Axis: layout.Horizontal}.Layout(gtx,
				layout.Flexed(1, func(gtx layout.Context) layout.Dimensions {
					return widget.Border{
						Color: colorSurface,
						Width: unit.Dp(2),
					}.Layout(gtx, func(gtx layout.Context) layout.Dimensions {
						return layout.UniformInset(unit.Dp(12)).Layout(gtx, func(gtx layout.Context) layout.Dimensions {
							if state.isoPath == "" {
								lbl := material.Body1(th, "No ISO selected...")
								lbl.Color = colorDisabled
								return lbl.Layout(gtx)
							}
							lbl := material.Body1(th, filepath.Base(state.isoPath))
							lbl.Color = colorText
							return lbl.Layout(gtx)
						})
					})
				}),
				layout.Rigid(layout.Spacer{Width: unit.Dp(10)}.Layout),
				layout.Rigid(func(gtx layout.Context) layout.Dimensions {
					btn := material.Button(th, &state.browseBtn, "Browse...")
					btn.Background = colorPrimary
					return btn.Layout(gtx)
				}),
			)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(20)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			if state.isoPath == "" {
				return layout.Dimensions{}
			}
			return drawInfoBox(gtx, th, "ISO Details", fmt.Sprintf("File: %s\nSize: %s", filepath.Base(state.isoPath), formatSize(state.isoSize)))
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(15)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			if state.isoPath == "" {
				return layout.Dimensions{}
			}
			// Check size compatibility
			if dev.Size < state.isoSize {
				warn := material.Body1(th, fmt.Sprintf("USB too small! Need %s, have %s", formatSize(state.isoSize), formatSize(dev.Size)))
				warn.Color = colorDanger
				return warn.Layout(gtx)
			}
			// Estimate write time (assuming ~15 MB/s for USB 2.0, ~80 MB/s for USB 3.0)
			estimatedTime := estimateWriteTime(state.isoSize)
			info := material.Body2(th, fmt.Sprintf("Estimated write time: %s", estimatedTime))
			info.Color = colorSuccess
			return info.Layout(gtx)
		}),
	)
}

func drawInfoBox(gtx layout.Context, th *material.Theme, title, content string) layout.Dimensions {
	return widget.Border{
		Color: colorPrimary,
		Width: unit.Dp(1),
	}.Layout(gtx, func(gtx layout.Context) layout.Dimensions {
		return layout.UniformInset(unit.Dp(15)).Layout(gtx, func(gtx layout.Context) layout.Dimensions {
			return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
				layout.Rigid(func(gtx layout.Context) layout.Dimensions {
					lbl := material.Caption(th, title)
					lbl.Color = colorPrimary
					lbl.Font.Weight = font.Bold
					return lbl.Layout(gtx)
				}),
				layout.Rigid(layout.Spacer{Height: unit.Dp(5)}.Layout),
				layout.Rigid(func(gtx layout.Context) layout.Dimensions {
					lbl := material.Body2(th, content)
					lbl.Color = colorText
					return lbl.Layout(gtx)
				}),
			)
		})
	})
}

// Page 4: Confirm (scrollable for smaller windows)
func drawPageConfirm(gtx layout.Context, th *material.Theme, state *AppState) layout.Dimensions {
	dev := state.devices[state.selectedUSB]
	estimatedTime := estimateWriteTime(state.isoSize)

	// Define the content items for the scrollable list
	items := []layout.Widget{
		func(gtx layout.Context) layout.Dimensions {
			icon := material.H2(th, "⚠")
			icon.Color = colorWarning
			icon.Alignment = text.Middle
			return icon.Layout(gtx)
		},
		layout.Spacer{Height: unit.Dp(10)}.Layout,
		func(gtx layout.Context) layout.Dimensions {
			title := material.H6(th, "Confirm Write Operation")
			title.Color = colorWarning
			title.Alignment = text.Middle
			return title.Layout(gtx)
		},
		layout.Spacer{Height: unit.Dp(20)}.Layout,
		func(gtx layout.Context) layout.Dimensions {
			warn := material.Body1(th, fmt.Sprintf("All data on %s will be permanently erased!", dev.Path))
			warn.Color = colorDanger
			warn.Alignment = text.Middle
			return warn.Layout(gtx)
		},
		layout.Spacer{Height: unit.Dp(20)}.Layout,
		func(gtx layout.Context) layout.Dimensions {
			return drawInfoBox(gtx, th, "Summary", fmt.Sprintf(
				"USB: %s %s (%s)\nISO: %s (%s)\nFormat: %s\nEstimated time: %s",
				dev.Vendor, dev.Model, formatSize(dev.Size),
				filepath.Base(state.isoPath), formatSize(state.isoSize),
				yesNo(state.formatUSBOpt),
				estimatedTime,
			))
		},
		layout.Spacer{Height: unit.Dp(25)}.Layout,
		func(gtx layout.Context) layout.Dimensions {
			cb := material.CheckBox(th, &state.confirmCheck, "I understand and want to proceed")
			cb.Color = colorText
			return layout.Center.Layout(gtx, cb.Layout)
		},
		layout.Spacer{Height: unit.Dp(20)}.Layout,
	}

	// Use material.List for scrolling
	return material.List(th, &state.confirmScroll).Layout(gtx, len(items), func(gtx layout.Context, i int) layout.Dimensions {
		return layout.Center.Layout(gtx, items[i])
	})
}

// Page 5: Writing
func drawPageWriting(gtx layout.Context, th *material.Theme, state *AppState) layout.Dimensions {
	progress := state.progress
	progressTxt := state.progressTxt
	etaText := state.etaText
	logs := make([]string, len(state.statusLog))
	copy(logs, state.statusLog)

	return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			title := material.H6(th, "Writing ISO to USB...")
			title.Color = colorAccent
			title.Alignment = text.Middle
			return title.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(8)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			if etaText != "" {
				eta := material.Body2(th, etaText)
				eta.Color = colorText
				eta.Alignment = text.Middle
				return eta.Layout(gtx)
			}
			return layout.Dimensions{}
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(25)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			return drawProgressBar(gtx, progress)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(10)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			pct := material.Body1(th, progressTxt)
			pct.Color = colorTextBright
			pct.Alignment = text.Middle
			return pct.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(25)}.Layout),
		layout.Flexed(1, func(gtx layout.Context) layout.Dimensions {
			return widget.Border{
				Color: colorSurface,
				Width: unit.Dp(1),
			}.Layout(gtx, func(gtx layout.Context) layout.Dimensions {
				return layout.UniformInset(unit.Dp(10)).Layout(gtx, func(gtx layout.Context) layout.Dimensions {
					logText := strings.Join(logs, "\n")
					log := material.Caption(th, logText)
					log.Color = colorText
					return log.Layout(gtx)
				})
			})
		}),
	)
}

func drawProgressBar(gtx layout.Context, progress float64) layout.Dimensions {
	height := gtx.Dp(unit.Dp(28))
	width := gtx.Constraints.Max.X

	// Background
	bgRect := clip.Rect{Max: image.Pt(width, height)}.Op()
	paint.FillShape(gtx.Ops, colorSurface, bgRect)

	// Progress
	progressWidth := int(float64(width) * progress)
	if progressWidth > 0 {
		progRect := clip.Rect{Max: image.Pt(progressWidth, height)}.Op()
		paint.FillShape(gtx.Ops, colorPrimary, progRect)
	}

	return layout.Dimensions{Size: image.Pt(width, height)}
}

// Page 6: Complete
func drawPageComplete(gtx layout.Context, th *material.Theme, state *AppState) layout.Dimensions {
	writeError := state.writeError

	if writeError != "" {
		return layout.Flex{Axis: layout.Vertical, Alignment: layout.Middle}.Layout(gtx,
			layout.Rigid(layout.Spacer{Height: unit.Dp(30)}.Layout),
			layout.Rigid(func(gtx layout.Context) layout.Dimensions {
				icon := material.H1(th, "X")
				icon.Color = colorDanger
				return icon.Layout(gtx)
			}),
			layout.Rigid(layout.Spacer{Height: unit.Dp(15)}.Layout),
			layout.Rigid(func(gtx layout.Context) layout.Dimensions {
				title := material.H5(th, "Write Failed")
				title.Color = colorDanger
				title.Alignment = text.Middle
				return title.Layout(gtx)
			}),
			layout.Rigid(layout.Spacer{Height: unit.Dp(15)}.Layout),
			layout.Rigid(func(gtx layout.Context) layout.Dimensions {
				err := material.Body1(th, writeError)
				err.Color = colorText
				err.Alignment = text.Middle
				return err.Layout(gtx)
			}),
		)
	}

	dev := state.devices[state.selectedUSB]
	return layout.Flex{Axis: layout.Vertical, Alignment: layout.Middle}.Layout(gtx,
		layout.Rigid(layout.Spacer{Height: unit.Dp(30)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			icon := material.H1(th, "OK")
			icon.Color = colorSuccess
			return icon.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(15)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			title := material.H5(th, "USB Created Successfully!")
			title.Color = colorSuccess
			title.Alignment = text.Middle
			return title.Layout(gtx)
		}),
		layout.Rigid(layout.Spacer{Height: unit.Dp(20)}.Layout),
		layout.Rigid(func(gtx layout.Context) layout.Dimensions {
			msg := material.Body1(th, fmt.Sprintf("Bootable USB created on %s\n\nYou can now boot from this drive.", dev.Path))
			msg.Color = colorText
			msg.Alignment = text.Middle
			return msg.Layout(gtx)
		}),
	)
}

func drawFooter(gtx layout.Context, th *material.Theme, state *AppState) layout.Dimensions {
	return layout.UniformInset(unit.Dp(15)).Layout(gtx, func(gtx layout.Context) layout.Dimensions {
		return layout.Flex{Axis: layout.Horizontal, Spacing: layout.SpaceBetween}.Layout(gtx,
			layout.Rigid(func(gtx layout.Context) layout.Dimensions {
				// Back/Exit button
				if state.currentPage == PageComplete {
					btn := material.Button(th, &state.startOverBtn, "Start Over")
					btn.Background = colorSurface
					btn.Color = colorText
					return btn.Layout(gtx)
				}
				if state.currentPage > PageSelectUSB && state.currentPage < PageWriting {
					btn := material.Button(th, &state.backBtn, "Back")
					btn.Background = colorSurface
					btn.Color = colorText
					return btn.Layout(gtx)
				}
				if state.currentPage == PageSelectUSB {
					btn := material.Button(th, &state.exitBtn, "Exit")
					btn.Background = colorSurface
					btn.Color = colorText
					return btn.Layout(gtx)
				}
				return layout.Dimensions{}
			}),
			layout.Rigid(func(gtx layout.Context) layout.Dimensions {
				// Next/Confirm button
				if state.currentPage == PageWriting {
					return layout.Dimensions{}
				}
				if state.currentPage == PageComplete {
					btn := material.Button(th, &state.exitBtn, "Exit")
					btn.Background = colorPrimary
					return btn.Layout(gtx)
				}

				var label string
				var enabled bool
				var btnColor color.NRGBA

				switch state.currentPage {
				case PageSelectUSB:
					label = "Next"
					enabled = state.selectedUSB >= 0
					btnColor = colorPrimary
				case PageFormat:
					label = "Next"
					enabled = true
					btnColor = colorPrimary
				case PageSelectISO:
					label = "Next"
					enabled = state.isoPath != "" && state.selectedUSB >= 0 && state.devices[state.selectedUSB].Size >= state.isoSize
					btnColor = colorPrimary
				case PageConfirm:
					label = "Start Writing"
					enabled = state.confirmCheck.Value
					btnColor = colorDanger
				}

				btn := material.Button(th, &state.nextBtn, label)
				if enabled {
					btn.Background = btnColor
				} else {
					btn.Background = colorDisabled
				}
				return btn.Layout(gtx)
			}),
		)
	})
}

// ============ Helper Functions ============

func yesNo(b bool) string {
	if b {
		return "Yes"
	}
	return "No"
}

func estimateWriteTime(sizeBytes uint64) string {
	// Assume conservative USB 2.0 speed of ~15 MB/s average
	const avgSpeed = 15 * 1024 * 1024
	seconds := float64(sizeBytes) / avgSpeed
	if seconds < 60 {
		return fmt.Sprintf("~%d seconds", int(seconds))
	}
	minutes := seconds / 60
	if minutes < 2 {
		return "~1-2 minutes"
	}
	return fmt.Sprintf("~%d minutes", int(minutes))
}

func detectUSBDevices() []USBDevice {
	var devices []USBDevice

	entries, err := os.ReadDir("/sys/block")
	if err != nil {
		return devices
	}

	for _, entry := range entries {
		name := entry.Name()

		// Skip non-disk devices
		if strings.HasPrefix(name, "loop") || strings.HasPrefix(name, "ram") ||
			strings.HasPrefix(name, "sr") || strings.HasPrefix(name, "dm-") ||
			strings.HasPrefix(name, "md") || strings.HasPrefix(name, "zram") ||
			strings.HasPrefix(name, "nvme") {
			continue
		}

		sysPath := filepath.Join("/sys/block", name)

		// Check if removable
		removableBytes, err := os.ReadFile(filepath.Join(sysPath, "removable"))
		if err != nil {
			continue
		}
		removable := strings.TrimSpace(string(removableBytes)) == "1"

		// Check if USB by reading the raw symlink
		rawLink, err := os.Readlink(sysPath)
		if err != nil {
			continue
		}
		isUSB := strings.Contains(rawLink, "/usb") || strings.Contains(rawLink, "usb")

		// If it's USB or removable, include it
		if !removable && !isUSB {
			continue
		}

		// Read size
		sizeBytes, _ := os.ReadFile(filepath.Join(sysPath, "size"))
		size := parseUint(strings.TrimSpace(string(sizeBytes))) * 512

		// Skip tiny devices (less than 10MB)
		if size < 10*1024*1024 {
			continue
		}

		// Read model
		modelBytes, _ := os.ReadFile(filepath.Join(sysPath, "device/model"))
		model := strings.TrimSpace(string(modelBytes))
		if model == "" {
			model = "USB Drive"
		}

		// Read vendor
		vendorBytes, _ := os.ReadFile(filepath.Join(sysPath, "device/vendor"))
		vendor := strings.TrimSpace(string(vendorBytes))
		if vendor == "" {
			vendor = "USB"
		}

		devices = append(devices, USBDevice{
			Path:      filepath.Join("/dev", name),
			Name:      name,
			Size:      size,
			Model:     model,
			Vendor:    vendor,
			Removable: removable,
		})
	}

	// Sort by path
	sort.Slice(devices, func(i, j int) bool {
		return devices[i].Path < devices[j].Path
	})

	return devices
}

func parseUint(s string) uint64 {
	var n uint64
	fmt.Sscanf(s, "%d", &n)
	return n
}

func formatSize(bytes uint64) string {
	const unit = 1024
	if bytes < unit {
		return fmt.Sprintf("%d B", bytes)
	}
	div, exp := uint64(unit), 0
	for n := bytes / unit; n >= unit; n /= unit {
		div *= unit
		exp++
	}
	return fmt.Sprintf("%.1f %cB", float64(bytes)/float64(div), "KMGTPE"[exp])
}

func browseForISO() string {
	// Try zenity first
	cmd := exec.Command("zenity", "--file-selection", "--title=Select ISO Image",
		"--file-filter=ISO files (*.iso)|*.iso", "--file-filter=All files|*")
	out, err := cmd.Output()
	if err == nil {
		return strings.TrimSpace(string(out))
	}

	// Try kdialog
	cmd = exec.Command("kdialog", "--getopenfilename", ".", "*.iso|ISO Images")
	out, err = cmd.Output()
	if err == nil {
		return strings.TrimSpace(string(out))
	}

	// Try yad
	cmd = exec.Command("yad", "--file", "--title=Select ISO Image",
		"--file-filter=*.iso")
	out, err = cmd.Output()
	if err == nil {
		return strings.TrimSpace(string(out))
	}

	return ""
}

func formatUSBDevice(state *AppState, w *app.Window) error {
	dev := state.devices[state.selectedUSB]

	// Unmount
	partitions, _ := filepath.Glob(dev.Path + "*")
	for _, part := range partitions {
		exec.Command("umount", "-f", part).Run()
	}
	time.Sleep(500 * time.Millisecond)

	// Create partition table
	if err := exec.Command("parted", "-s", dev.Path, "mklabel", "msdos").Run(); err != nil {
		return fmt.Errorf("failed to create partition table: %v", err)
	}
	if err := exec.Command("parted", "-s", dev.Path, "mkpart", "primary", "fat32", "1MiB", "100%").Run(); err != nil {
		return fmt.Errorf("failed to create partition: %v", err)
	}
	exec.Command("partprobe", dev.Path).Run()
	time.Sleep(2 * time.Second)

	// Format as FAT32
	part := dev.Path + "1"
	if err := exec.Command("mkfs.vfat", "-F", "32", "-n", "RAVENUSB", part).Run(); err != nil {
		return fmt.Errorf("failed to format: %v", err)
	}

	return nil
}

func writeToUSB(state *AppState, w *app.Window) {
	addLog := func(msg string) {
		state.mu.Lock()
		state.statusLog = append(state.statusLog, msg)
		state.mu.Unlock()
		w.Invalidate()
	}

	setProgress := func(p float64, text string) {
		state.mu.Lock()
		state.progress = p
		state.progressTxt = text
		state.mu.Unlock()
		w.Invalidate()
	}

	setETA := func(text string) {
		state.mu.Lock()
		state.etaText = text
		state.mu.Unlock()
		w.Invalidate()
	}

	setError := func(err string) {
		state.mu.Lock()
		state.writeError = err
		state.currentPage = PageComplete
		state.mu.Unlock()
		w.Invalidate()
	}

	state.mu.Lock()
	dev := state.devices[state.selectedUSB]
	isoPath := state.isoPath
	doFormat := state.formatUSBOpt
	state.mu.Unlock()

	addLog("Starting write process...")
	setProgress(0.02, "Preparing...")

	// Format if requested
	if doFormat {
		addLog("Formatting USB...")
		setProgress(0.05, "Formatting...")
		if err := formatUSBDevice(state, w); err != nil {
			setError("Format failed: " + err.Error())
			return
		}
		addLog("Format complete")
	}

	// Unmount
	addLog("Unmounting device...")
	setProgress(0.08, "Unmounting...")
	partitions, _ := filepath.Glob(dev.Path + "*")
	for _, part := range partitions {
		exec.Command("umount", "-f", part).Run()
	}
	exec.Command("sync").Run()
	time.Sleep(500 * time.Millisecond)

	setProgress(0.1, "Opening files...")

	// Open ISO
	info, err := os.Stat(isoPath)
	if err != nil {
		setError("Cannot read ISO: " + err.Error())
		return
	}
	totalSize := info.Size()

	isoFile, err := os.Open(isoPath)
	if err != nil {
		setError("Cannot open ISO: " + err.Error())
		return
	}
	defer isoFile.Close()

	// Open device
	device, err := os.OpenFile(dev.Path, os.O_WRONLY|os.O_SYNC, 0)
	if err != nil {
		setError("Cannot open device: " + err.Error())
		return
	}
	defer device.Close()

	addLog("Writing ISO to USB...")
	setProgress(0.1, "Writing...")

	buffer := make([]byte, 4*1024*1024) // 4MB buffer
	var written int64
	startTime := time.Now()
	lastUpdate := startTime

	for {
		n, err := isoFile.Read(buffer)
		if n > 0 {
			_, werr := device.Write(buffer[:n])
			if werr != nil {
				setError("Write error: " + werr.Error())
				return
			}
			written += int64(n)

			// Update progress every 500ms or so
			now := time.Now()
			if now.Sub(lastUpdate) > 500*time.Millisecond {
				lastUpdate = now

				progress := 0.1 + (float64(written)/float64(totalSize))*0.85
				progressText := fmt.Sprintf("%s / %s (%.1f%%)", formatSize(uint64(written)), formatSize(uint64(totalSize)), progress*100)
				setProgress(progress, progressText)

				// Calculate ETA
				elapsed := now.Sub(startTime).Seconds()
				if elapsed > 0 && written > 0 {
					speed := float64(written) / elapsed
					remaining := float64(totalSize - written)
					etaSeconds := remaining / speed
					speedMB := speed / (1024 * 1024)

					if etaSeconds < 60 {
						setETA(fmt.Sprintf("Speed: %.1f MB/s  -  ETA: %d seconds", speedMB, int(etaSeconds)))
					} else {
						etaMinutes := etaSeconds / 60
						setETA(fmt.Sprintf("Speed: %.1f MB/s  -  ETA: %.1f minutes", speedMB, etaMinutes))
					}
				}
			}
		}
		if err == io.EOF {
			break
		}
		if err != nil {
			setError("Read error: " + err.Error())
			return
		}
	}

	addLog("Syncing data...")
	setProgress(0.96, "Syncing...")
	setETA("Syncing to disk...")
	device.Sync()
	syscall.Sync()

	elapsed := time.Since(startTime)
	setProgress(1.0, "Complete!")
	setETA(fmt.Sprintf("Completed in %s", formatDuration(elapsed)))
	addLog(fmt.Sprintf("Write completed in %s", formatDuration(elapsed)))

	state.mu.Lock()
	state.writeDone = true
	state.currentPage = PageComplete
	state.mu.Unlock()
	w.Invalidate()
}

func formatDuration(d time.Duration) string {
	if d < time.Minute {
		return fmt.Sprintf("%d seconds", int(d.Seconds()))
	}
	minutes := int(d.Minutes())
	seconds := int(d.Seconds()) % 60
	return fmt.Sprintf("%d min %d sec", minutes, seconds)
}

func autoScan(state *AppState, w *app.Window) {
	ticker := time.NewTicker(3 * time.Second)
	for range ticker.C {
		state.mu.Lock()
		currentPage := state.currentPage
		state.mu.Unlock()

		// Don't scan during write or complete
		if currentPage >= PageWriting {
			continue
		}

		newDevs := detectUSBDevices()

		state.mu.Lock()
		changed := len(newDevs) != len(state.devices)
		if !changed {
			for i, d := range newDevs {
				if d.Path != state.devices[i].Path {
					changed = true
					break
				}
			}
		}

		if changed {
			state.devices = newDevs
			state.deviceClicks = make([]widget.Clickable, len(state.devices))
			if state.selectedUSB >= len(state.devices) {
				state.selectedUSB = -1
			}
		}
		state.mu.Unlock()

		if changed {
			w.Invalidate()
		}
	}
}
