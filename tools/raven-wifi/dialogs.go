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

// layoutPasswordDialog renders the password entry dialog
func (s *AppState) layoutPasswordDialog(gtx C, th *Theme) D {
	// Modal overlay
	paint.FillShape(gtx.Ops, th.ModalOverlay, clip.Rect{Max: gtx.Constraints.Max}.Op())

	// Center the dialog
	return layout.Center.Layout(gtx, func(gtx C) D {
		// Constrain dialog width
		gtx.Constraints.Max.X = gtx.Dp(unit.Dp(350))
		gtx.Constraints.Min.X = gtx.Dp(unit.Dp(350))

		// Dialog card
		return layout.Stack{}.Layout(gtx,
			layout.Expanded(func(gtx C) D {
				// Rounded background
				rr := clip.UniformRRect(image.Rectangle{Max: gtx.Constraints.Min}, 12)
				paint.FillShape(gtx.Ops, th.DialogBg, rr.Op(gtx.Ops))
				return D{Size: gtx.Constraints.Min}
			}),
			layout.Stacked(func(gtx C) D {
				return layout.UniformInset(unit.Dp(24)).Layout(gtx, func(gtx C) D {
					return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
						// Title
						layout.Rigid(func(gtx C) D {
							s.mu.Lock()
							ssid := s.pendingSSID
							s.mu.Unlock()

							l := material.H6(th.Material(), "Connect to "+ssid)
							l.Color = th.TextColor
							return l.Layout(gtx)
						}),

						layout.Rigid(layout.Spacer{Height: unit.Dp(16)}.Layout),

						// Password editor
						layout.Rigid(func(gtx C) D {
							ed := material.Editor(th.Material(), &s.passwordEditor, "Password")
							ed.Editor.Mask = '•'
							ed.Color = th.TextColor
							ed.HintColor = th.TextColorSecondary
							return ed.Layout(gtx)
						}),

						layout.Rigid(layout.Spacer{Height: unit.Dp(24)}.Layout),

						// Buttons
						layout.Rigid(func(gtx C) D {
							return s.layoutDialogButtons(gtx, th, &s.passwordCancel, &s.passwordOK, "Cancel", "Connect", true)
						}),
					)
				})
			}),
		)
	})
}

// layoutErrorDialog renders an error message dialog
func (s *AppState) layoutErrorDialog(gtx C, th *Theme) D {
	// Handle OK button
	if s.errorOK.Clicked(gtx) {
		s.mu.Lock()
		s.showErrorDialog = false
		s.mu.Unlock()
	}

	// Modal overlay
	paint.FillShape(gtx.Ops, th.ModalOverlay, clip.Rect{Max: gtx.Constraints.Max}.Op())

	// Center the dialog
	return layout.Center.Layout(gtx, func(gtx C) D {
		gtx.Constraints.Max.X = gtx.Dp(unit.Dp(350))
		gtx.Constraints.Min.X = gtx.Dp(unit.Dp(350))

		return layout.Stack{}.Layout(gtx,
			layout.Expanded(func(gtx C) D {
				rr := clip.UniformRRect(image.Rectangle{Max: gtx.Constraints.Min}, 12)
				paint.FillShape(gtx.Ops, th.DialogBg, rr.Op(gtx.Ops))
				return D{Size: gtx.Constraints.Min}
			}),
			layout.Stacked(func(gtx C) D {
				return layout.UniformInset(unit.Dp(24)).Layout(gtx, func(gtx C) D {
					return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
						// Title
						layout.Rigid(func(gtx C) D {
							s.mu.Lock()
							title := s.errorTitle
							s.mu.Unlock()

							l := material.H6(th.Material(), title)
							l.Color = th.TextColor
							return l.Layout(gtx)
						}),

						layout.Rigid(layout.Spacer{Height: unit.Dp(16)}.Layout),

						// Message
						layout.Rigid(func(gtx C) D {
							s.mu.Lock()
							msg := s.errorMessage
							s.mu.Unlock()

							l := material.Body1(th.Material(), msg)
							l.Color = th.TextColor
							return l.Layout(gtx)
						}),

						layout.Rigid(layout.Spacer{Height: unit.Dp(24)}.Layout),

						// OK button
						layout.Rigid(func(gtx C) D {
							return layout.Flex{}.Layout(gtx,
								layout.Flexed(1, layout.Spacer{}.Layout),
								layout.Rigid(func(gtx C) D {
									btn := material.Button(th.Material(), &s.errorOK, "OK")
									btn.Background = th.TealAccent
									return btn.Layout(gtx)
								}),
							)
						}),
					)
				})
			}),
		)
	})
}

// layoutSavedNetworksDialog renders the saved networks list dialog
func (s *AppState) layoutSavedNetworksDialog(gtx C, th *Theme) D {
	// Handle close button
	if s.savedClose.Clicked(gtx) {
		s.mu.Lock()
		s.showSavedDialog = false
		s.mu.Unlock()
	}

	// Check for delete button clicks
	s.mu.Lock()
	saved := make([]string, len(s.savedNetworks))
	copy(saved, s.savedNetworks)
	s.mu.Unlock()

	for _, ssid := range saved {
		deleteBtn := s.getSavedDeleteClickable(ssid)
		if deleteBtn.Clicked(gtx) {
			// Show forget confirmation
			s.mu.Lock()
			s.forgetSSID = ssid
			s.showForgetConfirm = true
			s.mu.Unlock()
		}
	}

	// Modal overlay
	paint.FillShape(gtx.Ops, th.ModalOverlay, clip.Rect{Max: gtx.Constraints.Max}.Op())

	// Center the dialog
	return layout.Center.Layout(gtx, func(gtx C) D {
		gtx.Constraints.Max.X = gtx.Dp(unit.Dp(400))
		gtx.Constraints.Min.X = gtx.Dp(unit.Dp(400))

		return layout.Stack{}.Layout(gtx,
			layout.Expanded(func(gtx C) D {
				rr := clip.UniformRRect(image.Rectangle{Max: gtx.Constraints.Min}, 12)
				paint.FillShape(gtx.Ops, th.DialogBg, rr.Op(gtx.Ops))
				return D{Size: gtx.Constraints.Min}
			}),
			layout.Stacked(func(gtx C) D {
				return layout.UniformInset(unit.Dp(24)).Layout(gtx, func(gtx C) D {
					return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
						// Title
						layout.Rigid(func(gtx C) D {
							l := material.H6(th.Material(), "Saved Networks")
							l.Color = th.TextColor
							return l.Layout(gtx)
						}),

						layout.Rigid(layout.Spacer{Height: unit.Dp(16)}.Layout),

						// List of saved networks
						layout.Rigid(func(gtx C) D {
							if len(saved) == 0 {
								l := material.Body1(th.Material(), "No saved networks found.")
								l.Color = th.TextColorSecondary
								return l.Layout(gtx)
							}

							return layout.Flex{Axis: layout.Vertical}.Layout(gtx, func() []layout.FlexChild {
								children := make([]layout.FlexChild, 0, len(saved)*2)
								for i, ssid := range saved {
									ssid := ssid // Capture
									children = append(children, layout.Rigid(func(gtx C) D {
										return s.layoutSavedNetworkItem(gtx, th, ssid)
									}))
									if i < len(saved)-1 {
										children = append(children, layout.Rigid(layout.Spacer{Height: unit.Dp(8)}.Layout))
									}
								}
								return children
							}()...)
						}),

						layout.Rigid(layout.Spacer{Height: unit.Dp(24)}.Layout),

						// Close button
						layout.Rigid(func(gtx C) D {
							return layout.Flex{}.Layout(gtx,
								layout.Flexed(1, layout.Spacer{}.Layout),
								layout.Rigid(func(gtx C) D {
									btn := material.Button(th.Material(), &s.savedClose, "Close")
									return btn.Layout(gtx)
								}),
							)
						}),
					)
				})
			}),
		)
	})
}

// layoutSavedNetworkItem renders a single saved network item with delete button
func (s *AppState) layoutSavedNetworkItem(gtx C, th *Theme, ssid string) D {
	deleteBtn := s.getSavedDeleteClickable(ssid)

	return layout.Flex{Alignment: layout.Middle}.Layout(gtx,
		// SSID label
		layout.Flexed(1, func(gtx C) D {
			l := material.Body1(th.Material(), ssid)
			l.Color = th.TextColor
			return l.Layout(gtx)
		}),

		// Delete button
		layout.Rigid(func(gtx C) D {
			icon, _ := widget.NewIcon(icons.ActionDelete)
			btn := material.IconButton(th.Material(), deleteBtn, icon, "Delete")
			btn.Color = th.SignalWeak // Red color for delete
			return btn.Layout(gtx)
		}),
	)
}

// layoutForgetConfirmDialog renders the forget network confirmation dialog
func (s *AppState) layoutForgetConfirmDialog(gtx C, th *Theme) D {
	// Handle button clicks
	if s.forgetConfirmNo.Clicked(gtx) {
		s.mu.Lock()
		s.showForgetConfirm = false
		s.mu.Unlock()
	}

	if s.forgetConfirmYes.Clicked(gtx) {
		s.mu.Lock()
		ssid := s.forgetSSID
		s.showForgetConfirm = false
		s.mu.Unlock()
		go s.forgetNetwork(ssid)
	}

	// Modal overlay (darker for stacked dialog)
	paint.FillShape(gtx.Ops, th.ModalOverlay, clip.Rect{Max: gtx.Constraints.Max}.Op())

	// Center the dialog
	return layout.Center.Layout(gtx, func(gtx C) D {
		gtx.Constraints.Max.X = gtx.Dp(unit.Dp(300))
		gtx.Constraints.Min.X = gtx.Dp(unit.Dp(300))

		return layout.Stack{}.Layout(gtx,
			layout.Expanded(func(gtx C) D {
				rr := clip.UniformRRect(image.Rectangle{Max: gtx.Constraints.Min}, 12)
				paint.FillShape(gtx.Ops, th.DialogBg, rr.Op(gtx.Ops))
				return D{Size: gtx.Constraints.Min}
			}),
			layout.Stacked(func(gtx C) D {
				return layout.UniformInset(unit.Dp(24)).Layout(gtx, func(gtx C) D {
					return layout.Flex{Axis: layout.Vertical}.Layout(gtx,
						// Title
						layout.Rigid(func(gtx C) D {
							l := material.H6(th.Material(), "Forget Network")
							l.Color = th.TextColor
							return l.Layout(gtx)
						}),

						layout.Rigid(layout.Spacer{Height: unit.Dp(16)}.Layout),

						// Message
						layout.Rigid(func(gtx C) D {
							s.mu.Lock()
							ssid := s.forgetSSID
							s.mu.Unlock()

							msg := fmt.Sprintf("Forget '%s'?", ssid)
							l := material.Body1(th.Material(), msg)
							l.Color = th.TextColor
							return l.Layout(gtx)
						}),

						layout.Rigid(layout.Spacer{Height: unit.Dp(24)}.Layout),

						// Buttons
						layout.Rigid(func(gtx C) D {
							return s.layoutDialogButtons(gtx, th, &s.forgetConfirmNo, &s.forgetConfirmYes, "Cancel", "Forget", false)
						}),
					)
				})
			}),
		)
	})
}

// layoutDialogButtons renders dialog button pair (cancel/action)
func (s *AppState) layoutDialogButtons(gtx C, th *Theme, cancelBtn, actionBtn *widget.Clickable, cancelText, actionText string, isAction bool) D {
	// Handle cancel button
	if cancelBtn.Clicked(gtx) {
		s.mu.Lock()
		s.showPasswordDialog = false
		s.showForgetConfirm = false
		s.passwordEditor.SetText("")
		s.mu.Unlock()
	}

	// Handle action button
	if actionBtn.Clicked(gtx) {
		if isAction {
			// Connect action
			password := s.passwordEditor.Text()
			s.mu.Lock()
			ssid := s.pendingSSID
			s.showPasswordDialog = false
			s.mu.Unlock()
			s.passwordEditor.SetText("")
			go s.connectToNetwork(ssid, password)
		}
		// Forget action is handled in layoutForgetConfirmDialog
	}

	return layout.Flex{Spacing: layout.SpaceBetween}.Layout(gtx,
		layout.Flexed(1, layout.Spacer{}.Layout),

		// Cancel button
		layout.Rigid(func(gtx C) D {
			btn := material.Button(th.Material(), cancelBtn, cancelText)
			return btn.Layout(gtx)
		}),

		layout.Rigid(layout.Spacer{Width: unit.Dp(8)}.Layout),

		// Action button
		layout.Rigid(func(gtx C) D {
			btn := material.Button(th.Material(), actionBtn, actionText)
			btn.Background = th.TealAccent
			return btn.Layout(gtx)
		}),
	)
}
