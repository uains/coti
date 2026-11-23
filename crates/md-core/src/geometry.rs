//! Basic `f32` geometry primitives shared by layout and rendering boundaries.

/// A point in layout coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: f32,
    /// Vertical coordinate.
    pub y: f32,
}

impl Point {
    /// The origin point `(0, 0)`.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Creates a point.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Returns the point translated by `(dx, dy)`.
    #[must_use]
    pub const fn translate(self, dx: f32, dy: f32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }
}

/// A width/height pair.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    /// Width in layout units.
    pub width: f32,
    /// Height in layout units.
    pub height: f32,
}

impl Size {
    /// The zero size.
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    /// Creates a size.
    #[must_use]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Returns `true` when either dimension is zero or negative.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

/// An axis-aligned rectangle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// Top-left origin.
    pub origin: Point,
    /// Rectangle size.
    pub size: Size,
}

impl Rect {
    /// Creates a rectangle from origin and size components.
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    /// Creates a rectangle from an origin and size.
    #[must_use]
    pub const fn from_origin_size(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    /// Returns the left edge.
    #[must_use]
    pub const fn left(self) -> f32 {
        self.origin.x
    }

    /// Returns the top edge.
    #[must_use]
    pub const fn top(self) -> f32 {
        self.origin.y
    }

    /// Returns the right edge.
    #[must_use]
    pub const fn right(self) -> f32 {
        self.origin.x + self.size.width
    }

    /// Returns the bottom edge.
    #[must_use]
    pub const fn bottom(self) -> f32 {
        self.origin.y + self.size.height
    }

    /// Returns the width.
    #[must_use]
    pub const fn width(self) -> f32 {
        self.size.width
    }

    /// Returns the height.
    #[must_use]
    pub const fn height(self) -> f32 {
        self.size.height
    }

    /// Returns `true` when the rectangle has zero or negative area.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.size.is_empty()
    }

    /// Returns `true` when `point` lies in the half-open rectangle.
    #[must_use]
    pub fn contains(self, point: Point) -> bool {
        point.x >= self.left()
            && point.x < self.right()
            && point.y >= self.top()
            && point.y < self.bottom()
    }

    /// Returns the overlap with `other`, or `None` when they do not overlap.
    #[must_use]
    pub fn intersect(self, other: Self) -> Option<Self> {
        let left = self.left().max(other.left());
        let top = self.top().max(other.top());
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());

        if right <= left || bottom <= top {
            None
        } else {
            Some(Self::new(left, top, right - left, bottom - top))
        }
    }

    /// Returns the smallest rectangle containing both rectangles.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        let left = self.left().min(other.left());
        let top = self.top().min(other.top());
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Self::new(left, top, right - left, bottom - top)
    }

    /// Returns the rectangle shrunk by `insets`.
    #[must_use]
    pub fn inset(self, insets: EdgeInsets) -> Self {
        Self::new(
            self.left() + insets.left,
            self.top() + insets.top,
            self.width() - insets.left - insets.right,
            self.height() - insets.top - insets.bottom,
        )
    }

    /// Returns the rectangle translated by `(dx, dy)`.
    #[must_use]
    pub const fn translate(self, dx: f32, dy: f32) -> Self {
        Self {
            origin: self.origin.translate(dx, dy),
            ..self
        }
    }
}

/// Edge insets, typically used for padding or viewport margins.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EdgeInsets {
    /// Left inset.
    pub left: f32,
    /// Top inset.
    pub top: f32,
    /// Right inset.
    pub right: f32,
    /// Bottom inset.
    pub bottom: f32,
}

impl EdgeInsets {
    /// Zero insets.
    pub const ZERO: Self = Self {
        left: 0.0,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
    };

    /// Creates insets from individual edges.
    #[must_use]
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// Creates equal insets on all edges.
    #[must_use]
    pub const fn all(value: f32) -> Self {
        Self::new(value, value, value, value)
    }

    /// Creates symmetric horizontal and vertical insets.
    #[must_use]
    pub const fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self::new(horizontal, vertical, horizontal, vertical)
    }

    /// Creates insets on the left and right edges only.
    #[must_use]
    pub const fn horizontal(value: f32) -> Self {
        Self::new(value, 0.0, value, 0.0)
    }

    /// Creates insets on the top and bottom edges only.
    #[must_use]
    pub const fn vertical(value: f32) -> Self {
        Self::new(0.0, value, 0.0, value)
    }

    /// Returns `true` when every edge is zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.left == 0.0 && self.top == 0.0 && self.right == 0.0 && self.bottom == 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::{EdgeInsets, Point, Rect, Size};

    #[test]
    fn point_translates() {
        let point = Point::new(2.0, 3.0).translate(4.0, -1.0);
        assert_eq!(point, Point::new(6.0, 2.0));
    }

    #[test]
    fn size_empty_for_zero_or_negative_dimensions() {
        assert!(Size::ZERO.is_empty());
        assert!(Size::new(0.0, 4.0).is_empty());
        assert!(Size::new(4.0, -1.0).is_empty());
        assert!(!Size::new(4.0, 1.0).is_empty());
    }

    #[test]
    fn rect_edges_and_contains() {
        let rect = Rect::new(10.0, 20.0, 30.0, 40.0);
        assert_eq!(rect.left(), 10.0);
        assert_eq!(rect.top(), 20.0);
        assert_eq!(rect.right(), 40.0);
        assert_eq!(rect.bottom(), 60.0);
        assert!(rect.contains(Point::new(10.0, 20.0)));
        assert!(rect.contains(Point::new(39.9, 59.9)));
        assert!(!rect.contains(Point::new(40.0, 60.0)));
        assert!(!rect.contains(Point::new(9.9, 20.0)));
    }

    #[test]
    fn rect_intersection_and_union() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert_eq!(a.intersect(b), Some(Rect::new(5.0, 5.0, 5.0, 5.0)));
        assert_eq!(a.union(b), Rect::new(0.0, 0.0, 15.0, 15.0));
        assert_eq!(a.intersect(Rect::new(20.0, 20.0, 1.0, 1.0)), None);
    }

    #[test]
    fn rect_inset_and_translate() {
        let rect = Rect::new(0.0, 0.0, 20.0, 10.0);
        assert_eq!(
            rect.inset(EdgeInsets::new(2.0, 1.0, 3.0, 4.0)),
            Rect::new(2.0, 1.0, 15.0, 5.0)
        );
        assert_eq!(rect.translate(5.0, 6.0), Rect::new(5.0, 6.0, 20.0, 10.0));
    }

    #[test]
    fn edge_insets_helpers() {
        assert!(EdgeInsets::ZERO.is_zero());
        assert_eq!(EdgeInsets::all(2.0), EdgeInsets::new(2.0, 2.0, 2.0, 2.0));
        assert_eq!(
            EdgeInsets::symmetric(2.0, 3.0),
            EdgeInsets::new(2.0, 3.0, 2.0, 3.0)
        );
        assert_eq!(
            EdgeInsets::horizontal(2.0),
            EdgeInsets::new(2.0, 0.0, 2.0, 0.0)
        );
        assert_eq!(
            EdgeInsets::vertical(3.0),
            EdgeInsets::new(0.0, 3.0, 0.0, 3.0)
        );
        assert!(!EdgeInsets::all(1.0).is_zero());
    }
}
