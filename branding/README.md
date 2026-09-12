# Raven Linux branding

The one raven mark, used everywhere Raven Linux shows a logo:

- RavenBoot bakes `raven-logo-light.png` into the `.efi` (`bootloader/build.rs`),
  and `bootloader/preview -- grub-theme` renders the GRUB copy from the same
  code into `configs/grub/theme/raven.png`.
- `scripts/lib/branding.sh` installs it into the sysroot's hicolor theme as
  `raven-logo` (light body) and `raven-logo-dark` (as drawn), plus
  `/usr/share/pixmaps`, which is what `LOGO=raven-logo` in `/etc/os-release`
  names. The Settings About page, the Store and Power mastheads, and the
  installer's welcome page all ask the icon theme for `raven-logo`.
- The greeter (RavenLogin), the terminal's window icon (RavenTerminal) and the
  file manager's folder icon each carry their own copy of the mark, because
  they are separate repositories; when the master changes, regenerate and
  copy those too (see "Copies elsewhere").

| File | What it is |
|------|------------|
| `raven-logo-source.png` | The original artwork, untouched (513x462, transparent). |
| `raven-logo.png` | Master, 512x512: trimmed, centred in a square with a 4% margin. Black body, purple wings. For light grounds. |
| `raven-logo-light.png` | Same geometry with the body recoloured to the Raven Glass text colour `#c0caf5`. For the dark backdrop `#16161f` (boot, login, desktop). |
| `raven-logo.svg` / `raven-logo-light.svg` | The 512px masters wrapped as SVG, for `hicolor/scalable`. Raster inside: the mark is not vector art. |

Every other size in the tree is derived from the masters. Regenerate with
`scripts/branding/render-logo.py` after replacing a master, which rewrites the
masters from `raven-logo-source.png` and the PNG ladder under `sizes/`. Do
not hand-edit a derived copy.

## Copies elsewhere

| Repository | File | From |
|------------|------|------|
| RavenLogin | `crates/raven-ui/assets/raven-logo-light-128.png` | `sizes/raven-logo-light-128.png` |
| RavenTerminal | `src/assets/raven-logo-{16,32,48,64,128,256}.png` | `sizes/raven-logo-light-*.png` |
| RavenTerminal | `src/assets/raven_terminal_icon.svg` | `raven-logo-light.svg` |
| RavenFileManager | `data/icons/hicolor/scalable/apps/com.ravenfilemanager.Raven.svg` | embeds `sizes/raven-logo-128.png` on the folder |
