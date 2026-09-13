// Iris Rev. 6b -- layers transcribed from the VIA layout export
// (~/dev/iris6b.json) so the compiled defaults are exactly what VIA had.
// VIA resets its EEPROM copy whenever the firmware build date changes, so
// the defaults have to be right.
//
// The only edit to the export: layer 3's one unused slot (the `"` key)
// becomes VIM_TOGG, which turns the vim-aware lighting (../vim/vim.c) off/on.
// It is on at boot.  In VIA it shows as CUSTOM(0).
#include QMK_KEYBOARD_H
#include "vim.h"

const uint16_t PROGMEM keymaps[][MATRIX_ROWS][MATRIX_COLS] = {
    [0] = LAYOUT(
        KC_MPLY, S(KC_1), S(KC_2)     , S(KC_3), KC_MINS, KC_PEQL, S(KC_6), S(KC_7), S(KC_8), KC_PGUP, KC_PGDN, MC_6,
        MO(2)  , KC_Q   , KC_W        , KC_E   , KC_R   , KC_T   , KC_Y   , KC_U   , KC_I   , KC_O   , KC_P   , S(KC_QUOT),
        KC_LCTL, KC_A   , LSFT_T(KC_S), KC_D   , KC_F   , KC_G   , KC_H   , KC_J   , KC_K   , KC_L   , KC_SCLN, KC_ENT,
        KC_BSLS, KC_Z   , KC_X        , KC_C   , KC_V   , KC_B   , S(KC_GRV), KC_MPLY, KC_N   , KC_M   , KC_COMM, KC_DOT    , KC_SLSH, KC_QUOT,
        KC_LALT, KC_LGUI, KC_SPC      , MO(3)  , KC_LSFT, KC_BSPC
    ),
    [1] = LAYOUT(
        KC_ESC , KC_1   , KC_2   , KC_3   , KC_4   , KC_5   , KC_6   , KC_7   , KC_8   , KC_9   , KC_0   , TG(1),
        KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS,
        KC_TRNS, KC_TRNS, KC_S   , KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS,
        KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, MO(2)  , KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS,
        KC_LALT, KC_LGUI, KC_SPC , MO(3)  , KC_LSFT, KC_BSPC
    ),
    [2] = LAYOUT(
        KC_MPLY, KC_F1  , KC_F2  , KC_F3  , KC_F4  , KC_F5  , KC_F6  , KC_F7, KC_F8, KC_F9, KC_F10 , TG(1),
        KC_TRNS, S(KC_1), S(KC_2), KC_ESC , S(KC_4), S(KC_5), KC_PDOT, KC_P7, KC_P8, KC_P9, KC_PEQL, S(KC_8),
        KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_0   , KC_P4, KC_P5, KC_P6, KC_PSLS, KC_BSLS,
        KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_MPLY, KC_COMM, KC_P1, KC_P2  , KC_P3  , KC_PMNS, KC_PPLS,
        KC_TRNS, KC_TRNS, KC_TRNS, KC_SPC , KC_LSFT, KC_BSPC
    ),
    [3] = LAYOUT(
        KC_LCTL, MS_LEFT, MS_DOWN, MS_UP  , MS_RGHT   , MS_BTN1   , KC_MRWD, KC_MPRV, KC_MPLY, KC_MNXT, KC_MFFD, KC_TRNS,
        TG(3)  , KC_TRNS, KC_TRNS, KC_ESC , KC_LBRC   , KC_RBRC   , MC_3   , KC_F8  , KC_F9  , MC_4   , KC_TRNS, VIM_TOGG,
        KC_LCTL, KC_TRNS, MC_0   , KC_TRNS, S(KC_LBRC), S(KC_RBRC), KC_LEFT, KC_DOWN, KC_UP  , KC_RGHT, KC_TRNS, KC_TRNS,
        KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, S(KC_9)   , S(KC_0)   , KC_GRV , KC_TRNS, S(KC_MINS), KC_PMNS, S(KC_COMM), S(KC_DOT), KC_SLSH, KC_PPLS,
        MC_5   , KC_PEQL, KC_TAB , KC_TRNS, KC_TRNS   , KC_TRNS
    )
};

#ifdef ENCODER_ENABLE
bool encoder_update_user(uint8_t index, bool clockwise) {
    // on the iris I only have the right indexed encoder
    if (index == 1) {
        // Layer 0 (base): Scrolling
        if (layer_state_is(0)) {
            if (clockwise) {
                tap_code(MS_WHLD);  // Scroll down
            } else {
                tap_code(MS_WHLU);  // Scroll up
            }
        }
        // Layer 2: Volume control
        else if (layer_state_is(2)) {
            if (clockwise) {
                tap_code(KC_VOLU);
            } else {
                tap_code(KC_VOLD);
            }
        }
        // Layer 3: Brightness control
        else if (layer_state_is(3)) {
            if (clockwise) {
                tap_code(KC_BRIU);
            } else {
                tap_code(KC_BRID);
            }
        }
    }

    return false;
}
#endif

void keyboard_post_init_user(void) {
    vim_init();
}

bool process_record_user(uint16_t keycode, keyrecord_t *record) {
    return vim_process_record(keycode, record);
}

void housekeeping_task_user(void) {
    vim_housekeeping();
}

bool rgb_matrix_indicators_advanced_user(uint8_t led_min, uint8_t led_max) {
    return vim_render(led_min, led_max);
}
