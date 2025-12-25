#include QMK_KEYBOARD_H

const uint16_t PROGMEM keymaps[][MATRIX_ROWS][MATRIX_COLS] = {
    [0] = LAYOUT(
        KC_MPLY, S(KC_1),        S(KC_2),    S(KC_3),       KC_MINS,     KC_PEQL,                      S(KC_TILD), KC_PGDN,    KC_PGUP,    S(KC_8),      S(KC_7),      S(KC_6),
        MO(2),   KC_Q,           KC_W,       KC_E,          KC_R,        KC_T,                         KC_Y,       KC_U,       KC_I,       KC_O,         KC_P,         S(KC_QUOT),
        KC_LCTL, KC_A,           LSFT_T(KC_S), KC_D,        KC_F,        KC_G,                         KC_H,       KC_J,       KC_K,       KC_L,         KC_SCLN,      KC_ENT,
        KC_BSLS, KC_Z,           KC_X,       KC_C,          KC_V,        KC_B,        KC_NO,  KC_NO,  KC_N,       KC_M,       KC_COMM,    KC_DOT,       KC_SLSH,      KC_QUOT,
                                                            KC_LALT,     KC_LGUI,     KC_SPC,  KC_BSPC, KC_LSFT,   MO(3)
    ),
    [1] = LAYOUT(
        KC_ESC,  KC_1,    KC_2,    KC_3,    KC_4,    KC_5,                         MO(2),   TG(1),   KC_0,    KC_9,    KC_8,    KC_7,
        KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS,                     KC_6,    KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS,
        KC_TRNS, KC_TRNS, KC_S,    KC_TRNS, KC_TRNS, KC_TRNS,                     KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS,
        KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_NO,     KC_NO,  KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS,
                                             KC_LALT, KC_LGUI, KC_SPC,    KC_BSPC, KC_LSFT, MO(3)
    ),
    [2] = LAYOUT(
        KC_MPLY, KC_F1,   KC_F2,   KC_F3,   KC_F4,   KC_F5,                       KC_TRNS, TG(1),   KC_F10,  KC_F9,   KC_F8,   KC_F7,
        KC_TRNS, S(KC_1), S(KC_2), KC_ESC,  S(KC_4), S(KC_5),                     KC_F6,   S(KC_8), KC_PEQL, KC_P9,   KC_P8,   KC_P7,
        KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS,                     KC_PDOT, KC_BSLS, KC_PSLS, KC_P6,   KC_P5,   KC_P4,
        KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS, KC_NO,     KC_NO,  KC_0,    KC_PPLS, KC_PMNS, KC_P3,   KC_P2,   KC_P1,
                                             KC_TRNS, KC_TRNS, KC_TRNS,   KC_BSPC, KC_LSFT, KC_SPC
    ),
    [3] = LAYOUT(
        KC_LCTL, MS_LEFT,    MS_DOWN,    MS_UP,    MS_RGHT,     MS_BTN1,                   KC_GRV,  KC_MFFD, KC_MNXT, KC_MPLY, KC_MPRV, KC_MRWD,
        TG(3),   KC_TRNS,    KC_TRNS,    KC_ESC,   KC_LBRC,     KC_RBRC,                   KC_NO,   KC_TRNS, KC_F9,   KC_F8,   KC_TRNS, KC_TRNS,
        KC_LCTL, KC_TRNS,    KC_TRNS,    KC_TRNS,  S(KC_LBRC),  S(KC_RBRC),                KC_TRNS, KC_TRNS, KC_RGHT, KC_UP,   KC_DOWN, KC_LEFT,
        KC_TRNS, KC_TRNS,    KC_TRNS,    KC_TRNS,  S(KC_9),     S(KC_0),    KC_NO, KC_NO,  KC_PPLS, KC_SLSH, S(KC_DOT), S(KC_COMM), KC_PMNS, S(KC_MINS),
                                                    KC_TRNS,     KC_PEQL,    KC_TAB, KC_TRNS, KC_TRNS, KC_TRNS
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
