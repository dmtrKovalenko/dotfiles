This is the place where I store my custom QMK customized firmware for some of my mechanical keyboards.

    jev/          (retired) host-driven lighting for the Rev. 6; no longer
    jevlight/     built into iris-rev6 since 2026-11-01, kept for reference
    vim/          vim-aware RGB lighting, entirely on the board (vim.c, vim.h)
    snake/        snake for the Rev. 8's LEDs
    iris-rev6/    Keebio Iris Rev. 6  (ATmega32U4)  -- plain VIA firmware, same layers as the Rev. 8
    iris-rev8/    Keebio Iris Rev. 8  (RP2040)      -- vim.c/vim.h, snake.c/snake.h symlinks

Each board folder is a QMK keymap.  Install into the QMK tree (dereferencing
the symlinks) and build:

    cp -L iris-rev6/* ~/qmk_firmware/keyboards/keebio/iris/rev6/keymaps/via/
    cp -L iris-rev8/* ~/qmk_firmware/keyboards/keebio/iris/rev8/keymaps/via/
    qmk flash -kb keebio/iris/rev6 -km via     # .hex, DFU: hold reset; flash BOTH halves
    qmk flash -kb keebio/iris/rev8 -km via     # .uf2, double-tap reset; flash BOTH halves

`qmk flash` needs dfu-programmer for the Rev. 6; QMK Toolbox bundles one:

    DFU="/Applications/QMK Toolbox.app/Contents/Resources/dfu-programmer"
    "$DFU" atmega32u4 erase --force && "$DFU" atmega32u4 flash keebio_iris_rev6_via.hex && "$DFU" atmega32u4 reset

or, per half (press reset, then run; it flashes, waits for the half to come
back and pushes the macros + keymap over VIA, since a new build date wipes
VIA's EEPROM):

    tools/flash_rev6.sh

## Rev. 6: jevlight (retired)

Until 2026-11-01 the Rev. 6 ran host-driven lighting: `jev/jev.c` streamed
key presses to `jevlight` (Rust, in `jevlight/`), which asked Jev
(typesafe.ai) for the next key and painted probabilities onto the board.  It is
no longer compiled in; the Rev. 6 is plain VIA / RGB Matrix now and its layer
3 `"` slot is KC_NO.  The sources stay here if it is ever wanted back
(re-add the symlinks, `SRC += jev.c`, and the jev block in config.h).
