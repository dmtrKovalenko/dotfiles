This is the place where I store my custom QMK customized firmware for some of my mechanical keyboards.

    jev/          host-driven lighting for the Rev. 6 (jev.c, jev.h): the board
                  streams key presses over raw HID and renders the frame it
                  gets back
    jevlight/     the host side (Rust): vim rules + Jev (typesafe.ai) predicting
                  the next key, painted onto the board
    vim/          vim-aware RGB lighting, entirely on the board (vim.c, vim.h)
    snake/        snake for the Rev. 8's LEDs
    iris-rev6/    Keebio Iris Rev. 6  (ATmega32U4)  -- jev.c/jev.h are symlinks into ../jev
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

## Rev. 6: jevlight

The Rev. 6 no longer runs the vim state machine itself (that freed the flash
from 97% to 81%).  Instead:

    board --(0xD0 key press: row, col, keycode, mods, layers)--> jevlight
    board <--(0xC1 frame: mode, flags, per-LED overlay, fx sequence numbers)-- jevlight
                                                   |
                                                   +--> Jev: "which key is next?"

`jevlight` (jevlight/src) reads the live keymap out of the board with VIA's
own commands, watches the frontmost app (and, when that is kitty, whether the
focused window runs nvim via `kitty @ ls`), keeps the old vim rules
(jevlight/src/vim.rs is a port of vim/vim.c) and, on every key press, asks Jev
one Choice question with one option per physical key.  In nvim each option is
described with what that key means right now ("motion: completes the pending
operator d"), so the rules and the prediction agree.  Probabilities become
brightness; rule colours (amber text objects, cyan marks, magenta registers,
red escape) stay.  The board still renders the `.` ripple, mark/macro flashes,
the count pulse, REC breathing and the WPM rainbow itself.

Run it:

    cd jevlight && cargo build --release
    ./target/release/jevlight            # logs predictions; -q for silence, -v for more
    # API key: $TYPESAFE_API_KEY or ~/.config/jevlight/api_key (chmod 600)

Or let launchd keep it running (see jevlight/launchd/):

    cp jevlight/launchd/ai.typesafe.jevlight.plist ~/Library/LaunchAgents/
    launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/ai.typesafe.jevlight.plist

With no host talking to it the board falls back to whatever VIA has saved
(after 2.5 s), and CUSTOM(0) on layer 3 (the `"` key) toggles the host
lighting off/on.  The other half needs the same firmware: the master forwards
the frame over the TRRS link.
