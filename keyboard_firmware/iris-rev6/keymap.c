// Iris Rev. 6b -- the same layers as the Rev. 8 (../iris-rev8/keymap.c), so
// both boards feel identical.  VIA resets its EEPROM copy whenever the
// firmware build date changes, so the defaults have to be right; the macros
// live only in EEPROM, so after a flash push them back with
// ../tools/via_push.py (it writes the keymap too, no reflash needed):
//
//     tools/via_push.py iris-rev6/keymap.c iris-rev8/via.json
//
// The only difference from the Rev. 8: layer 3's `"` slot is KC_NO here.
// On the Rev. 8 it starts the snake game, which does not fit on the
// ATmega32U4.  (The host-driven jev lighting that used to live in that slot
// is gone; lighting is plain VIA / RGB Matrix now.)
#include QMK_KEYBOARD_H

const uint16_t PROGMEM keymaps[][MATRIX_ROWS][MATRIX_COLS] = {
    [0] = LAYOUT(
        KC_MPLY, S(KC_1), S(KC_2),      S(KC_3), KC_HOME, KC_END,                          S(KC_6),   S(KC_7), S(KC_8), KC_PGUP, KC_PGDN, MC_6,
        MO(2),   KC_Q,    KC_W,         KC_E,    KC_R,    KC_T,                             KC_Y,      KC_U,    KC_I,    KC_O,    KC_P,    S(KC_QUOT),
        KC_LCTL, KC_A,    LSFT_T(KC_S), KC_D,    KC_F,    KC_G,                             KC_H,      KC_J,    KC_K,    KC_L,    KC_SCLN, KC_ENT,
        KC_BSLS, KC_Z,    KC_X,         KC_C,    KC_V,    KC_B,    S(KC_GRV),      KC_MPLY, KC_N,      KC_M,    KC_COMM, KC_DOT,  KC_SLSH, KC_QUOT,
                                                 KC_LALT, KC_LGUI, KC_SPC,         MO(3),   KC_LSFT,   KC_BSPC
    ),
    [1] = LAYOUT(
        KC_ESC,  KC_1,    KC_2,    KC_3,    KC_4,    KC_5,                                  KC_6,    KC_7,    KC_8,    KC_9,    KC_0,    TG(1),
        MO(2),   KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS,                                KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS,
        KC_LCTL, KC_TRNS, KC_S,    KC_TRNS, KC_TRNS, KC_TRNS,                                KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS,
        KC_LSFT, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, MO(2),          KC_TRNS,       KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS,
                                            KC_LALT, KC_LGUI, KC_SPC,         MO(3),         KC_LSFT, KC_BSPC
    ),
    [2] = LAYOUT(
        KC_MPLY, KC_F1,   KC_F2,   KC_F3,   KC_F4,   KC_F5,                                  KC_F6,   KC_F7,   KC_F8,   KC_F9,   KC_F10,  TG(1),
        KC_TRNS, S(KC_1), S(KC_2), KC_ESC,  S(KC_4), S(KC_5),                                KC_DOT, KC_7,   KC_8,   KC_9,   KC_EQL, S(KC_8),
        KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS,                                KC_0,    KC_4,   KC_5,   KC_6,   KC_SLSH, KC_BSLS,
        KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS,        KC_MPLY,       KC_COMM, KC_1,   KC_2,   KC_3,   KC_MINS, S(KC_EQL),
                                            KC_TRNS, KC_TRNS, KC_TRNS,        KC_SPC,        KC_LSFT, KC_BSPC
    ),
    [3] = LAYOUT(
        KC_LCTL, MS_LEFT, MS_DOWN, MS_UP,   MS_RGHT,    MC_7,                                KC_MRWD,    KC_MPRV, KC_MPLY,    KC_MNXT,   KC_MFFD, KC_TRNS,
        TG(3),   MS_BTN1, MS_BTN2, KC_ESC,  KC_LBRC,    KC_RBRC,                             MC_3,       KC_F8,   KC_F9,      MC_4,      KC_TRNS, KC_NO,
        KC_LCTL, KC_TRNS, KC_F12,  KC_TRNS, S(KC_LBRC), S(KC_RBRC),                          KC_LEFT,    KC_DOWN, KC_UP,      KC_RGHT,   KC_TRNS, KC_GRV,
        KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, S(KC_9),    S(KC_0),   KC_GRV,       KC_TRNS,    S(KC_MINS), KC_MINS, S(KC_COMM), S(KC_DOT), KC_SLSH, S(KC_EQL),
                                            MC_5,       KC_EQL,   KC_TAB,       KC_TRNS,    KC_TRNS,    KC_TRNS
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
