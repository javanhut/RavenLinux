#!/bin/bash

write_hyprland_config() {
    local dest="$1"
    mkdir -p "$(dirname "$dest")"
    cat > "$dest" << 'EOF'
# Raven Linux - Hyprland Configuration
# This configuration integrates all Raven desktop components

# =====================
# Monitor Configuration
# =====================
monitor=,preferred,auto,1

# =====================
# Startup Applications
# =====================
# Raven unified desktop shell (includes panel, desktop, menu, settings, etc.)
exec-once = raven-shell

# Notification daemon
exec-once = mako || dunst || swaync

# PolicyKit agent (for privilege escalation)
exec-once = /usr/lib/polkit-gnome/polkit-gnome-authentication-agent-1 || /usr/lib/polkit-kde-authentication-agent-1

# Clipboard manager
exec-once = wl-paste --type text --watch cliphist store
exec-once = wl-paste --type image --watch cliphist store

# =====================
# Environment Variables
# =====================
env = XCURSOR_SIZE,24
env = QT_QPA_PLATFORMTHEME,qt5ct
env = QT_WAYLAND_DISABLE_WINDOWDECORATION,1

# XDG runtime directory (critical for Wayland apps)
env = XDG_RUNTIME_DIR,/run/user/0

# Wayland/XDG session settings
env = XDG_SESSION_TYPE,wayland
env = XDG_CURRENT_DESKTOP,Hyprland
env = XDG_SESSION_DESKTOP,Hyprland

# Gio UI settings (for vem and other Gio-based apps)
# Force OpenGL backend for better Hyprland compatibility
env = GIOUI_GPU_BACKEND,opengl

# =====================
# Input Configuration
# =====================
input {
    kb_layout = us
    kb_variant =
    kb_model =
    kb_options =
    kb_rules =

    follow_mouse = 1
    sensitivity = 0

    touchpad {
        natural_scroll = yes
        tap-to-click = yes
        disable_while_typing = yes
    }
}

# =====================
# General Settings
# =====================
general {
    gaps_in = 4
    gaps_out = 8
    border_size = 2
    col.active_border = rgba(009688ff) rgba(00bfa5ff) 45deg
    col.inactive_border = rgba(333333aa)

    layout = dwindle
    allow_tearing = false
}

# =====================
# Decorations
# =====================
decoration {
    rounding = 8

    blur {
        enabled = no
        size = 8
        passes = 2
        new_optimizations = yes
        xray = false
    }

    active_opacity = 1.0
    inactive_opacity = 1.0
    fullscreen_opacity = 1.0

    shadow {
        enabled = no
        range = 8
        render_power = 2
        color = rgba(00000055)
    }
}

# =====================
# Animations
# =====================
# Disabled for software rendering performance
animations {
    enabled = no
}

# =====================
# Layouts
# =====================
dwindle {
    pseudotile = yes
    preserve_split = yes
    force_split = 2
}

master {
    new_status = master
}

# =====================
# Miscellaneous
# =====================
misc {
    force_default_wallpaper = 0
    disable_hyprland_logo = true
    disable_splash_rendering = true
    mouse_move_enables_dpms = true
    key_press_enables_dpms = true
}

# =====================
# Window Rules
# =====================
# Raven components - layer shell handled internally
windowrulev2 = float,class:^(raven-menu)$
windowrulev2 = float,class:^(raven-settings)$
windowrulev2 = float,class:^(raven-wifi)$
windowrulev2 = float,class:^(raven-launcher)$
windowrulev2 = float,class:^(raven-installer)$

# Picture-in-Picture
windowrulev2 = float,title:^(Picture-in-Picture)$
windowrulev2 = pin,title:^(Picture-in-Picture)$

# File dialogs
windowrulev2 = float,title:^(Open File)$
windowrulev2 = float,title:^(Save File)$
windowrulev2 = float,title:^(Open Folder)$

# Confirmation dialogs
windowrulev2 = float,class:^(org.gnome.*)$,title:^(Confirm)$

# =====================
# Layer Rules
# =====================
# Raven shell uses layer-shell for panel, desktop, and popups
layerrule = blur,raven-shell
layerrule = ignorezero,raven-shell

# =====================
# Keybindings
# =====================
$mainMod = SUPER

# Application launchers
bind = $mainMod, T, exec, raven-terminal || foot || kitty || alacritty
bind = $mainMod, M, exec, raven-ctl toggle menu
bind = $mainMod, S, exec, raven-ctl toggle settings
bind = $mainMod, P, exec, raven-ctl toggle power
bind = $mainMod, K, exec, raven-ctl toggle keybindings
bind = $mainMod, Space, exec, raven-ctl toggle menu
bind = $mainMod, E, exec, raven-ctl toggle filemanager

# Window management
bind = $mainMod, Q, killactive,
bind = $mainMod SHIFT, Q, exit,
bind = $mainMod SHIFT, F, fullscreen, 0
bind = $mainMod CTRL, F, fullscreen, 1
bind = $mainMod, V, togglefloating,
bind = $mainMod SHIFT, P, pseudo,
bind = $mainMod, J, togglesplit,

# Focus movement
bind = $mainMod, left, movefocus, l
bind = $mainMod, right, movefocus, r
bind = $mainMod, up, movefocus, u
bind = $mainMod, down, movefocus, d
bind = $mainMod, H, movefocus, l
bind = $mainMod, L, movefocus, r
bind = $mainMod, K, movefocus, u
bind = Alt, J, movefocus, d

# Window movement
bind = $mainMod SHIFT, left, movewindow, l
bind = $mainMod SHIFT, right, movewindow, r
bind = $mainMod SHIFT, up, movewindow, u
bind = $mainMod SHIFT, down, movewindow, d
bind = $mainMod SHIFT, H, movewindow, l
bind = $mainMod SHIFT, L, movewindow, r
bind = $mainMod SHIFT, K, movewindow, u
bind = $mainMod SHIFT, J, movewindow, d

# Resize mode
bind = $mainMod, R, submap, resize

submap = resize
binde = , right, resizeactive, 10 0
binde = , left, resizeactive, -10 0
binde = , up, resizeactive, 0 -10
binde = , down, resizeactive, 0 10
binde = , L, resizeactive, 10 0
binde = , H, resizeactive, -10 0
binde = , K, resizeactive, 0 -10
binde = , J, resizeactive, 0 10
bind = , escape, submap, reset
bind = , Return, submap, reset
submap = reset

# Workspaces
bind = $mainMod, 1, workspace, 1
bind = $mainMod, 2, workspace, 2
bind = $mainMod, 3, workspace, 3
bind = $mainMod, 4, workspace, 4
bind = $mainMod, 5, workspace, 5
bind = $mainMod, 6, workspace, 6
bind = $mainMod, 7, workspace, 7
bind = $mainMod, 8, workspace, 8
bind = $mainMod, 9, workspace, 9
bind = $mainMod, 0, workspace, 10

# Move window to workspace
bind = $mainMod SHIFT, 1, movetoworkspace, 1
bind = $mainMod SHIFT, 2, movetoworkspace, 2
bind = $mainMod SHIFT, 3, movetoworkspace, 3
bind = $mainMod SHIFT, 4, movetoworkspace, 4
bind = $mainMod SHIFT, 5, movetoworkspace, 5
bind = $mainMod SHIFT, 6, movetoworkspace, 6
bind = $mainMod SHIFT, 7, movetoworkspace, 7
bind = $mainMod SHIFT, 8, movetoworkspace, 8
bind = $mainMod SHIFT, 9, movetoworkspace, 9
bind = $mainMod SHIFT, 0, movetoworkspace, 10

# Workspace navigation
bind = $mainMod, mouse_down, workspace, e+1
bind = $mainMod, mouse_up, workspace, e-1
bind = $mainMod, Tab, workspace, e+1
bind = $mainMod SHIFT, Tab, workspace, e-1

# Special workspace (scratchpad)
bind = $mainMod, grave, togglespecialworkspace, scratchpad
bind = $mainMod SHIFT, grave, movetoworkspace, special:scratchpad

# Move/resize windows with mouse
bindm = $mainMod, mouse:272, movewindow
bindm = $mainMod, mouse:273, resizewindow

# Media keys
bindel = , XF86AudioRaiseVolume, exec, wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%+
bindel = , XF86AudioLowerVolume, exec, wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-
bindl = , XF86AudioMute, exec, wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle
bindl = , XF86AudioMicMute, exec, wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle

# Brightness keys
bindel = , XF86MonBrightnessUp, exec, brightnessctl set 5%+
bindel = , XF86MonBrightnessDown, exec, brightnessctl set 5%-

# Media control
bindl = , XF86AudioPlay, exec, playerctl play-pause
bindl = , XF86AudioPrev, exec, playerctl previous
bindl = , XF86AudioNext, exec, playerctl next

# Screenshot
bind = , Print, exec, grim -g "$(slurp)" - | wl-copy
bind = SHIFT, Print, exec, grim - | wl-copy
bind = $mainMod, Print, exec, grim -g "$(slurp)" ~/Pictures/Screenshots/$(date +%Y%m%d_%H%M%S).png
bind = $mainMod SHIFT, Print, exec, grim ~/Pictures/Screenshots/$(date +%Y%m%d_%H%M%S).png

# Lock screen
bind = $mainMod, Escape, exec, hyprlock || swaylock || loginctl lock-session

# Settings (alternative binding)
bind = $mainMod, I, exec, raven-ctl toggle settings

# File manager
bind = $mainMod SHIFT, E, exec, raven-ctl toggle filemanager
EOF
}

install_hyprland_config() {
    local dest
    for dest in "$@"; do
        write_hyprland_config "$dest"
    done
}
