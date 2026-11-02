#!/bin/sh
# Renders every badge icon in web/public/image/badges/by-id/ to a PNG of the same
# name in web/public/image/badges/png/, for email: Gmail and Outlook don't show SVG
# images. Run it (make badge-pngs) after adding or changing a badge's SVG, and
# commit the PNGs. Needs rsvg-convert (brew install librsvg).
set -eu
cd "$(dirname "$0")/../public/image/badges"
command -v rsvg-convert >/dev/null || {
  echo "rsvg-convert not found: brew install librsvg" >&2
  exit 1
}
mkdir -p png
rm -f png/*.png
for svg in by-id/*.svg; do
  # 192px: drawn at 96px in the emails, so sharp on high-density screens.
  rsvg-convert -w 192 -h 192 "$svg" -o "png/$(basename "$svg" .svg).png"
done
echo "Rendered $(ls png | wc -l | tr -d ' ') badge PNGs"
