package main

import (
	"fmt"
	"image"

	"gioui.org/layout"
	"gioui.org/op/clip"
	"gioui.org/op/paint"
	"gioui.org/unit"
	"gioui.org/widget"
	"gioui.org/widget/material"
	"golang.org/x/exp/shiny/materialdesign/icons"
)

// Type aliases for convenience
type (
	C = layout.Context
	D = layout.Dimensions
)

// Layout is the main layout function that renders the entire UI
func (s *AppState) Layout(gtx C, th *Theme) D {
	// Check if any dialog is shown
	if s.showPasswordDialog {
		return s.layoutPasswordDialog(gtx, th)
	}
	if s.showSavedDialog {
		return s.layoutSavedNetworksDialog(gtx, th)
	}
	if s.showErrorDialog {
		return s.layoutErrorDialog(gtx, th)
	}
	if s.showForgetConfirm {
		return s.layoutForgetConfirmDialog(gtx, th)
	}

	// Normal main layout
	return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
		// Header bar
		layout.Rigid(func(gtx C) D {
			return s.layoutHeader(gtx, th)
		}),

		// Status card
		layout.Rigid(func(gtx C) D {
			return s.layoutStatusCard(gtx, th)
		}),

		// Network list (takes remaining space)
		layout.Flexed(1, func(gtx C) D {
			return s.layoutNetworkList(gtx, th)
		}),

		// Button bar
		layout.Rigid(func(gtx C) D {
			return s.layoutButtons(gtx, th)
		}),
	)
}

// layoutHeader renders the header with title and refresh button
func (s *AppState) layoutHeader(gtx C, th *Theme) D {
	// Handle refresh button click
	if s.refreshBtn.Clicked(gtx) {
		go s.refreshNetworks()
	}

	// Background
	paint.FillShape(gtx.Ops, th.CardBg, clip.Rect{Max: gtx.Constraints.Max}.Op())

	return layout.UniformInset(unit.Dp(12)).Layout(gtx, func(gtx C) D {
		return layout.Flex{Alignment: layout.Middle}.Layout(gtx,
			// Title
			layout.Flexed(1, func(gtx C) D {
				l := material.H6(th.Material(), "WiFi Networks")
				l.Color = th.TextColor
				return l.Layout(gtx)
			}),

			// Refresh button
			layout.Rigid(func(gtx C) D {
				icon, _ := widget.NewIcon(icons.NavigationRefresh)
				btn := material.IconButton(th.Material(), &s.refreshBtn, icon, "Refresh")
				btn.Color = th.TextColor
				btn.Background = th.TealAccent
				return btn.Layout(gtx)
			}),
		)
	})
}

// layoutStatusCard renders the connection status card
func (s *AppState) layoutStatusCard(gtx C, th *Theme) D {
	return layout.UniformInset(unit.Dp(12)).Layout(gtx, func(gtx C) D {
		// Card background
		return layout.Stack{}.Layout(gtx,
			layout.Expanded(func(gtx C) D {
				// Rounded rectangle background
				rr := clip.UniformRRect(image.Rectangle{Max: gtx.Constraints.Min}, 8)
				paint.FillShape(gtx.Ops, th.CardBg, rr.Op(gtx.Ops))
				return D{Size: gtx.Constraints.Min}
			}),
			layout.Stacked(func(gtx C) D {
				return layout.UniformInset(unit.Dp(12)).Layout(gtx, func(gtx C) D {
					s.mu.Lock()
					text := s.statusText
					s.mu.Unlock()

					l := material.Body1(th.Material(), text)
					l.Color = th.TextColor
					return l.Layout(gtx)
				})
			}),
		)
	})
}

// layoutNetworkList renders the scrollable list of networks
func (s *AppState) layoutNetworkList(gtx C, th *Theme) D {
	s.mu.Lock()
	networks := make([]Network, len(s.networks))
	copy(networks, s.networks)
	s.mu.Unlock()

	return layout.UniformInset(unit.Dp(12)).Layout(gtx, func(gtx C) D {
		return material.List(th.Material(), &s.list).Layout(gtx, len(networks), func(gtx C, index int) D {
			return s.layoutNetworkItem(gtx, th, index, networks[index])
		})
	})
}

// layoutNetworkItem renders a single network list item
func (s *AppState) layoutNetworkItem(gtx C, th *Theme, index int, net Network) D {
	click := s.getNetworkClickable(index)

	// Handle click
	if click.Clicked(gtx) {
		go s.handleNetworkClick(net)
	}

	// Determine state
	s.mu.Lock()
	isConnecting := s.connecting && s.connectingSSID == net.SSID
	s.mu.Unlock()

	isConnected := net.Connected

	// Create clickable wrapper
	return click.Layout(gtx, func(gtx C) D {
		// Draw connecting overlay if connecting
		if isConnecting {
			paint.FillShape(gtx.Ops, th.ConnectingBg, clip.Rect{Max: gtx.Constraints.Max}.Op())
		}

		return layout.UniformInset(unit.Dp(12)).Layout(gtx, func(gtx C) D {
			return layout.Flex{Alignment: layout.Middle, Spacing: layout.SpaceBetween}.Layout(gtx,
				// Lock icon for secured networks
				layout.Rigid(func(gtx C) D {
					if net.Security != "" && net.Security != "Open" {
						icon, _ := widget.NewIcon(icons.ActionLock)
						ic := icon
						gtx.Constraints.Max = image.Pt(gtx.Dp(unit.Dp(20)), gtx.Dp(unit.Dp(20)))
						paint.ColorOp{Color: th.TextColorSecondary}.Add(gtx.Ops)
						ic.Layout(gtx, th.TextColorSecondary)
						return D{Size: gtx.Constraints.Max}
					}
					return D{}
				}),

				layout.Rigid(layout.Spacer{Width: unit.Dp(8)}.Layout),

				// Network info (SSID and details)
				layout.Flexed(1, func(gtx C) D {
					return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
						// Title (SSID + status)
						layout.Rigid(func(gtx C) D {
							title := net.SSID
							if isConnected {
								title += " ✓"
							} else if isConnecting {
								title += " ⋯"
							}
							l := material.Body1(th.Material(), title)
							l.Color = th.TextColor
							l.TextSize = unit.Sp(16)
							return l.Layout(gtx)
						}),

						// Subtitle (signal + security)
						layout.Rigid(func(gtx C) D {
							subtitle := fmt.Sprintf("Signal: %d%% - %s", net.Signal, net.Security)
							l := material.Caption(th.Material(), subtitle)
							l.Color = th.TextColorSecondary
							return l.Layout(gtx)
						}),
					)
				}),

				layout.Rigid(layout.Spacer{Width: unit.Dp(8)}.Layout),

				// Signal strength icon
				layout.Rigid(func(gtx C) D {
					return s.layoutSignalIcon(gtx, th, net.Signal)
				}),
			)
		})
	})
}

// layoutSignalIcon renders the WiFi signal strength icon
func (s *AppState) layoutSignalIcon(gtx C, th *Theme, signal int) D {
	var iconData *widget.Icon
	var err error

	// Choose icon based on signal strength
	switch {
	case signal >= 80:
		iconData, err = widget.NewIcon(icons.DeviceSignalWiFi4Bar)
	case signal >= 60:
		iconData, err = widget.NewIcon(icons.DeviceSignalWiFi3Bar)
	case signal >= 40:
		iconData, err = widget.NewIcon(icons.DeviceSignalWiFi2Bar)
	case signal >= 20:
		iconData, err = widget.NewIcon(icons.DeviceSignalWiFi1Bar)
	default:
		iconData, err = widget.NewIcon(icons.DeviceSignalWiFi0Bar)
	}

	if err != nil {
		return D{}
	}

	// Choose color based on signal strength
	color := th.SignalWeak
	if signal >= 70 {
		color = th.SignalStrong
	} else if signal >= 40 {
		color = th.SignalMedium
	}

	ic := iconData
	size := gtx.Dp(unit.Dp(24))
	gtx.Constraints.Max = image.Pt(size, size)
	ic.Layout(gtx, color)
	return D{Size: image.Pt(size, size)}
}

// layoutButtons renders the button bar (Saved + Disconnect)
func (s *AppState) layoutButtons(gtx C, th *Theme) D {
	// Handle button clicks
	if s.disconnectBtn.Clicked(gtx) {
		go s.onDisconnect()
	}
	if s.savedBtn.Clicked(gtx) {
		go s.showSavedNetworks()
	}

	return layout.UniformInset(unit.Dp(12)).Layout(gtx, func(gtx C) D {
		return layout.Flex{Alignment: layout.End, Spacing: layout.SpaceBetween}.Layout(gtx,
			layout.Flexed(1, layout.Spacer{}.Layout),

			// Saved button
			layout.Rigid(func(gtx C) D {
				btn := material.Button(th.Material(), &s.savedBtn, "Saved")
				return btn.Layout(gtx)
			}),

			layout.Rigid(layout.Spacer{Width: unit.Dp(8)}.Layout),

			// Disconnect button
			layout.Rigid(func(gtx C) D {
				btn := material.Button(th.Material(), &s.disconnectBtn, "Disconnect")

				// Disable if not connected
				s.mu.Lock()
				connected := s.currentSSID != ""
				s.mu.Unlock()

				if !connected {
					btn.Background = th.DisabledBg
				}

				return btn.Layout(gtx)
			}),
		)
	})
}
