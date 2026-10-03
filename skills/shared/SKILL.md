# Shared

Lessons for the shared role. Numbered and dated, newest last.

1. (2026-10-03) Shell scripts that draw images (ImageMagick 6, `convert`): name the exact operators in the prompt (`gradient:`, `radial-gradient:`, `-gravity` + `-geometry` + `-composite`, `-resize WxH`). Left to itself the model invents options (`-offset`, `-draw "gradient(...)"`), puts `local` outside functions (breaks under `set -e`) and single-quotes `xc:'$var'` so the variable never expands. Say "logo at N% of the width" means size, not position. Run the script and look at the output before using it.
