package main

import (
	"image/color"

	"gioui.org/widget/material"
)

// Theme holds the custom Material Design dark theme with teal accent
type Theme struct {
	material *material.Theme

	// Custom colors
	TealAccent         color.NRGBA
	CardBg             color.NRGBA
	DialogBg           color.NRGBA
	ModalOverlay       color.NRGBA
	TextColor          color.NRGBA
	TextColorSecondary color.NRGBA
	DisabledBg         color.NRGBA

	// Signal strength colors
	SignalStrong color.NRGBA
	SignalMedium color.NRGBA
	SignalWeak   color.NRGBA

	// Feedback colors
	ConnectingBg color.NRGBA
}

// NewDarkTheme creates a new dark theme with teal accent
func NewDarkTheme() *Theme {
	th := material.NewTheme()

	// Set dark background and light foreground
	th.Palette.Bg = color.NRGBA{R: 0x1e, G: 0x1e, B: 0x1e, A: 0xff}
	th.Palette.Fg = color.NRGBA{R: 0xff, G: 0xff, B: 0xff, A: 0xff}
	th.Palette.ContrastBg = color.NRGBA{R: 0x00, G: 0x96, B: 0x88, A: 0xff}
	th.Palette.ContrastFg = color.NRGBA{R: 0xff, G: 0xff, B: 0xff, A: 0xff}

	// Shaper is already set by material.NewTheme() with default font collection

	return &Theme{
		material:           th,
		TealAccent:         color.NRGBA{R: 0x00, G: 0x96, B: 0x88, A: 0xff},
		CardBg:             color.NRGBA{R: 0x2a, G: 0x2a, B: 0x2a, A: 0xff},
		DialogBg:           color.NRGBA{R: 0x30, G: 0x30, B: 0x30, A: 0xff},
		ModalOverlay:       color.NRGBA{R: 0x00, G: 0x00, B: 0x00, A: 0xb4},
		TextColor:          color.NRGBA{R: 0xff, G: 0xff, B: 0xff, A: 0xff},
		TextColorSecondary: color.NRGBA{R: 0xaa, G: 0xaa, B: 0xaa, A: 0xff},
		DisabledBg:         color.NRGBA{R: 0x60, G: 0x60, B: 0x60, A: 0xff},
		SignalStrong:       color.NRGBA{R: 0x4c, G: 0xaf, B: 0x50, A: 0xff},
		SignalMedium:       color.NRGBA{R: 0xff, G: 0xc1, B: 0x07, A: 0xff},
		SignalWeak:         color.NRGBA{R: 0xf4, G: 0x43, B: 0x36, A: 0xff},
		ConnectingBg:       color.NRGBA{R: 0x00, G: 0x96, B: 0x88, A: 0x30},
	}
}

// Material returns the underlying material theme
func (t *Theme) Material() *material.Theme {
	return t.material
}
