#!/bin/sh
# Exact, lossless PNG crops of the supplied 1536x1024 identity sheet.
set -eu
cd "$(dirname "$0")/.."
magick assets/identity.png -crop 1536x568+0+0 +repage assets/banner.png
magick assets/identity.png -crop 372x421+6+574 +repage assets/logo-ascii-hood.png
magick assets/identity.png -crop 377x430+388+574 +repage assets/logo-hackrf.png
magick assets/identity.png -crop 381x430+773+574 +repage assets/logo-round.png
magick assets/identity.png -crop 374x430+1162+574 +repage assets/logo-terminal.png
