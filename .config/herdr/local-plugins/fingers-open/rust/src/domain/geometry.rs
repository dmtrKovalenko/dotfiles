//! Where the captured pane sits inside the overlay, so hints land exactly on
//! the text they describe even when the tab is split.

use thiserror::Error;

/// A rectangle in terminal cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> u16 {
        self.x.saturating_add(self.width)
    }

    pub fn bottom(&self) -> u16 {
        self.y.saturating_add(self.height)
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn contains(&self, other: &Rect) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }

    pub fn intersection(&self, other: Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        Rect::new(x, y, right.saturating_sub(x), bottom.saturating_sub(y))
    }
}

/// A pane and its rectangle as Herdr's `pane.layout` reports them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanePlacement {
    pub pane_id: String,
    pub rect: Rect,
}

/// The tab layout at the moment the hints were requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub area: Rect,
    pub zoomed: bool,
    pub panes: Vec<PanePlacement>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GeometryError {
    #[error("pane {0} is not in the current tab layout")]
    PaneNotInLayout(String),
    #[error("the tab layout has an empty area or pane")]
    EmptyRect,
    #[error("pane {0} lies outside the tab area")]
    OutsideArea(String),
    #[error("malformed overlay geometry `{0}`")]
    Malformed(String),
}

/// The source pane's rectangle relative to the overlay's top-left corner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayGeometry {
    pub pane_id: String,
    pub area: Rect,
    pub pane: Rect,
}

impl OverlayGeometry {
    /// Locates `pane_id` in the layout. A zoomed pane fills the whole tab.
    pub fn locate(layout: &Layout, pane_id: &str) -> Result<Self, GeometryError> {
        let placement = layout
            .panes
            .iter()
            .find(|p| p.pane_id == pane_id)
            .ok_or_else(|| GeometryError::PaneNotInLayout(pane_id.to_string()))?;
        let absolute = if layout.zoomed {
            layout.area
        } else {
            placement.rect
        };
        if layout.area.is_empty() || absolute.is_empty() {
            return Err(GeometryError::EmptyRect);
        }
        if !layout.area.contains(&absolute) {
            return Err(GeometryError::OutsideArea(pane_id.to_string()));
        }
        Ok(OverlayGeometry {
            pane_id: pane_id.to_string(),
            area: layout.area,
            pane: Rect::new(
                absolute.x - layout.area.x,
                absolute.y - layout.area.y,
                absolute.width,
                absolute.height,
            ),
        })
    }

    /// A compact form fit for an environment variable:
    /// `pane_id|x,y,width,height|x,y,width,height` (area, then pane).
    pub fn encode(&self) -> String {
        format!(
            "{}|{}|{}",
            self.pane_id,
            encode_rect(&self.area),
            encode_rect(&self.pane)
        )
    }

    pub fn decode(text: &str) -> Result<Self, GeometryError> {
        let malformed = || GeometryError::Malformed(text.to_string());
        let mut parts = text.split('|');
        let pane_id = parts
            .next()
            .filter(|id| !id.is_empty())
            .ok_or_else(malformed)?;
        let area = parts.next().and_then(decode_rect).ok_or_else(malformed)?;
        let pane = parts.next().and_then(decode_rect).ok_or_else(malformed)?;
        if parts.next().is_some() {
            return Err(malformed());
        }
        Ok(OverlayGeometry {
            pane_id: pane_id.to_string(),
            area,
            pane,
        })
    }

    /// The cells of `frame` the captured pane is redrawn into (clipped).
    pub fn content_rect(&self, frame: Rect) -> Rect {
        Rect::new(
            frame.x.saturating_add(self.pane.x),
            frame.y.saturating_add(self.pane.y),
            self.pane.width,
            self.pane.height,
        )
        .intersection(frame)
    }

    /// A one-row strip of `frame` left blank by the pane, for the status
    /// line; `None` when the pane fills the frame.
    pub fn status_rect(&self, frame: Rect) -> Option<Rect> {
        let content = self.content_rect(frame);
        let last_row = frame.bottom().saturating_sub(1);
        let strip = if content.bottom() < frame.bottom() {
            Rect::new(frame.x, last_row, frame.width, 1)
        } else if content.x > frame.x {
            Rect::new(frame.x, last_row, content.x - frame.x, 1)
        } else if content.right() < frame.right() {
            Rect::new(
                content.right(),
                last_row,
                frame.right() - content.right(),
                1,
            )
        } else if content.y > frame.y {
            Rect::new(frame.x, frame.y, frame.width, 1)
        } else {
            return None;
        };
        Some(strip.intersection(frame)).filter(|r| !r.is_empty())
    }
}

fn encode_rect(rect: &Rect) -> String {
    format!("{},{},{},{}", rect.x, rect.y, rect.width, rect.height)
}

fn decode_rect(text: &str) -> Option<Rect> {
    let mut numbers = text.split(',').map(|n| n.parse::<u16>().ok());
    let rect = Rect::new(
        numbers.next()??,
        numbers.next()??,
        numbers.next()??,
        numbers.next()??,
    );
    numbers.next().is_none().then_some(rect)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(zoomed: bool) -> Layout {
        Layout {
            area: Rect::new(20, 2, 100, 40),
            zoomed,
            panes: vec![
                PanePlacement {
                    pane_id: "left".into(),
                    rect: Rect::new(20, 2, 50, 40),
                },
                PanePlacement {
                    pane_id: "right".into(),
                    rect: Rect::new(71, 23, 49, 19),
                },
            ],
        }
    }

    #[test]
    fn the_pane_is_located_relative_to_the_tab_area() {
        let geometry = OverlayGeometry::locate(&layout(false), "right").unwrap();
        assert_eq!(geometry.pane, Rect::new(51, 21, 49, 19));
        assert_eq!(geometry.area.width, 100);
    }

    #[test]
    fn a_zoomed_pane_takes_the_whole_tab() {
        let geometry = OverlayGeometry::locate(&layout(true), "right").unwrap();
        assert_eq!(geometry.pane, Rect::new(0, 0, 100, 40));
    }

    #[test]
    fn unknown_or_misplaced_panes_are_rejected() {
        assert_eq!(
            OverlayGeometry::locate(&layout(false), "nope"),
            Err(GeometryError::PaneNotInLayout("nope".into()))
        );
        let mut outside = layout(false);
        outside.panes[1].rect.x = 19;
        assert_eq!(
            OverlayGeometry::locate(&outside, "right"),
            Err(GeometryError::OutsideArea("right".into()))
        );
    }

    #[test]
    fn content_is_clipped_to_the_frame_and_status_uses_free_space() {
        let geometry = OverlayGeometry::locate(&layout(false), "right").unwrap();
        let frame = Rect::new(0, 0, 100, 40);
        assert_eq!(geometry.content_rect(frame), Rect::new(51, 21, 49, 19));
        assert_eq!(geometry.status_rect(frame), Some(Rect::new(0, 39, 51, 1)));
        assert_eq!(
            geometry.content_rect(Rect::new(0, 0, 80, 30)),
            Rect::new(51, 21, 29, 9)
        );
    }

    #[test]
    fn a_full_frame_pane_leaves_no_status_strip() {
        let geometry = OverlayGeometry::locate(&layout(true), "left").unwrap();
        assert_eq!(geometry.status_rect(Rect::new(0, 0, 100, 40)), None);
    }

    #[test]
    fn geometry_survives_a_round_trip_through_its_text_form() {
        let geometry = OverlayGeometry::locate(&layout(false), "right").unwrap();
        let text = geometry.encode();
        assert_eq!(text, "right|20,2,100,40|51,21,49,19");
        assert_eq!(OverlayGeometry::decode(&text), Ok(geometry));
    }

    #[test]
    fn malformed_geometry_text_is_rejected() {
        for text in [
            "",
            "right",
            "right|1,2,3|4,5,6,7",
            "right|1,2,3,4|5,6,7",
            "right|1,2,3,4|5,6,7,8|extra",
            "|1,2,3,4|5,6,7,8",
        ] {
            assert!(
                matches!(
                    OverlayGeometry::decode(text),
                    Err(GeometryError::Malformed(_))
                ),
                "{text:?}"
            );
        }
    }

    #[test]
    fn a_left_pane_gets_a_status_strip_on_the_right() {
        let geometry = OverlayGeometry::locate(&layout(false), "left").unwrap();
        assert_eq!(
            geometry.status_rect(Rect::new(0, 0, 100, 40)),
            Some(Rect::new(50, 39, 50, 1))
        );
    }
}
