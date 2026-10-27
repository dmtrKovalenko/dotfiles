use std::time::{Duration, Instant};

/// A press at the same cell within this window continues the
/// multi-click run. GUI convention is ~500ms; 400ms leaves margin for
/// SSH/PTY forwarding jitter. Raise it only if the manual TTY check
/// (a real triple click through herdr over SSH) fails to register.
const MULTI_CLICK_WINDOW: Duration = Duration::from_millis(400);

/// Counts consecutive left presses on the same screen cell so the
/// caller can map 1/2/3 to move / word / line selection. Pure logic:
/// the clock is injected into `press`, so tests never sleep. Strict
/// same-cell equality is safe because SGR 1006 mouse reporting only
/// emits an event when the cell changes.
pub struct ClickTracker {
    at: (u16, u16),
    n: u8,
    when: Instant,
}

impl Default for ClickTracker {
    fn default() -> Self {
        // The construction time is inert: `n == 0` forces the first
        // press to count 1 regardless of elapsed time.
        ClickTracker {
            at: (0, 0),
            n: 0,
            when: Instant::now(),
        }
    }
}

impl ClickTracker {
    /// Record a left press at screen cell (col, row) and return its
    /// click count: 1 single, 2 double, 3 triple; further rapid
    /// presses on the same cell stay 3.
    pub fn press(&mut self, col: u16, row: u16, now: Instant) -> u8 {
        let run_continues = self.n > 0
            && self.at == (col, row)
            && now.duration_since(self.when) < MULTI_CLICK_WINDOW;
        self.n = if run_continues {
            (self.n + 1).min(3)
        } else {
            1
        };
        self.at = (col, row);
        self.when = now;
        self.n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn ms(base: Instant, n: u64) -> Instant {
        base + Duration::from_millis(n)
    }

    #[test]
    fn rapid_same_cell_presses_count_up_and_clamp_at_three() {
        let base = Instant::now();
        let mut t = ClickTracker::default();
        assert_eq!(t.press(5, 2, ms(base, 0)), 1);
        assert_eq!(t.press(5, 2, ms(base, 100)), 2);
        assert_eq!(t.press(5, 2, ms(base, 200)), 3);
        assert_eq!(t.press(5, 2, ms(base, 300)), 3, "a 4th click stays 3");
    }

    #[test]
    fn window_measures_from_the_previous_press() {
        // Gaps of 350ms each stay in the run even though the total
        // exceeds the window.
        let base = Instant::now();
        let mut t = ClickTracker::default();
        assert_eq!(t.press(0, 0, ms(base, 0)), 1);
        assert_eq!(t.press(0, 0, ms(base, 350)), 2);
        assert_eq!(t.press(0, 0, ms(base, 700)), 3);
    }

    #[test]
    fn slow_press_resets_the_run() {
        let base = Instant::now();
        let mut t = ClickTracker::default();
        assert_eq!(t.press(5, 2, ms(base, 0)), 1);
        assert_eq!(t.press(5, 2, ms(base, 400)), 1, "window is strict <400ms");
    }

    #[test]
    fn different_cell_resets_the_run() {
        let base = Instant::now();
        let mut t = ClickTracker::default();
        assert_eq!(t.press(5, 2, ms(base, 0)), 1);
        assert_eq!(t.press(6, 2, ms(base, 100)), 1, "new cell starts over");
        assert_eq!(t.press(6, 2, ms(base, 200)), 2, "and owns its own run");
    }
}
