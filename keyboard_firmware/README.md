This is the place where I store my custom QMK customized firmware for some of my mechanical keyboards.

    vim/          vim-aware RGB lighting, shared by every board (vim.c, vim.h)
    iris-rev6/    Keebio Iris Rev. 6  (ATmega32U4)  -- vim.c/vim.h are symlinks into ../vim
    iris-rev8/    Keebio Iris Rev. 8  (RP2040)      -- vim.c/vim.h are symlinks into ../vim

Each board folder is a QMK keymap.  Install into the QMK tree (dereferencing
the symlinks) and build:

    cp -L iris-rev6/* ~/qmk_firmware/keyboards/keebio/iris/rev6/keymaps/via/
    cp -L iris-rev8/* ~/qmk_firmware/keyboards/keebio/iris/rev8/keymaps/via/
    qmk flash -kb keebio/iris/rev6 -km via     # .hex, DFU: hold reset
    qmk flash -kb keebio/iris/rev8 -km via     # .uf2, double-tap reset; flash BOTH halves

Rev. 6 is at 97% of flash: the stock RGB animations are #undef'd in its
config.h to make room, since the vim lighting paints every LED itself.
