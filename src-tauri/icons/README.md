# Icons

Place your app icons here before running `tauri build`.

Required files:
- `32x32.png`
- `128x128.png`
- `128x128@2x.png`
- `icon.icns` (macOS)
- `icon.ico` (Windows)

## Generating icons from a source image

If you have a 1024×1024 PNG source image, Tauri can generate all required sizes:

```bash
npm run tauri icon path/to/source-1024.png
```

This creates all required formats automatically.

## Quick placeholder (development)

For dev builds without real icons, create a minimal 32×32 PNG:

```bash
# PowerShell (Windows)
$bitmap = [System.Drawing.Bitmap]::new(32, 32)
$g = [System.Drawing.Graphics]::FromImage($bitmap)
$g.Clear([System.Drawing.Color]::FromArgb(124, 58, 237))
$bitmap.Save("32x32.png")
```
