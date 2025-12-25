#pragma once

// Turn off RGB backlight by default
#define RGB_MATRIX_DEFAULT_VAL 0

// Caps Word - activate by double tapping left shift
#define DOUBLE_TAP_SHIFT_TURNS_ON_CAPS_WORD

// Fix for mod-tap typing delay
#define TAPPING_TERM 200        // Adjust this value (150-250ms typical)
#define PERMISSIVE_HOLD          // Makes tap-hold keys trigger hold if another key is pressed
#define QUICK_TAP_TERM 0         // Disables quick tap to prevent accidental holds

// Optional: Uncomment if you want even more responsive mod-taps
// #define HOLD_ON_OTHER_KEY_PRESS  // Activate hold immediately when another key is pressed
