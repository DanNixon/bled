use embedded_graphics::geometry::{Point, Size};
use getset::Getters;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct Matrix2d {
    /// Width of the matrix.
    width: u32,

    /// Height of the matrix.
    height: u32,

    /// The corner of the matrix that contains the first pixel.
    origin: MatrixOrigin,

    /// The layout of pixels in the matrix.
    layout: MatrixLayout,

    /// The order in which the matrix pixels are wired.
    wiring_order: MatrixWiringOrder,
}

impl Matrix2d {
    pub fn new(
        size: Size,
        origin: MatrixOrigin,
        layout: MatrixLayout,
        wiring_order: MatrixWiringOrder,
    ) -> Self {
        Self {
            width: size.width,
            height: size.height,
            origin,
            layout,
            wiring_order,
        }
    }

    /// Returns the size of the matrix.
    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    /// Returns the index of the pixel at the given point, relative to the top left corner of the matrix.
    /// Returns `None` if the point is outside the matrix bounds.
    pub fn pixel_index(&self, point: Point) -> Option<usize> {
        if point.x < 0 || point.y < 0 {
            return None;
        }

        let mut column = usize::try_from(point.x).ok()?;
        let mut row = usize::try_from(point.y).ok()?;
        let columns = usize::try_from(self.width).ok()?;
        let rows = usize::try_from(self.height).ok()?;

        if column >= columns || row >= rows {
            return None;
        }

        if matches!(
            self.origin,
            MatrixOrigin::TopRight | MatrixOrigin::BottomRight
        ) {
            column = columns - 1 - column;
        }
        if matches!(
            self.origin,
            MatrixOrigin::BottomLeft | MatrixOrigin::BottomRight
        ) {
            row = rows - 1 - row;
        }

        match self.layout {
            MatrixLayout::RowMajor => {
                if self.wiring_order == MatrixWiringOrder::Serpentine && row % 2 == 1 {
                    column = columns - 1 - column;
                }
                row.checked_mul(columns)?.checked_add(column)
            }
            MatrixLayout::ColumnMajor => {
                if self.wiring_order == MatrixWiringOrder::Serpentine && column % 2 == 1 {
                    row = rows - 1 - row;
                }
                column.checked_mul(rows)?.checked_add(row)
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum MatrixOrigin {
    #[default]
    TopLeft,

    BottomLeft,

    TopRight,

    BottomRight,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum MatrixLayout {
    #[default]
    RowMajor,

    ColumnMajor,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum MatrixWiringOrder {
    #[default]
    Linear,

    Serpentine,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_index_top_left_column_major_linear() {
        let matrix = Matrix2d::new(
            Size::new(3, 4),
            MatrixOrigin::TopLeft,
            MatrixLayout::ColumnMajor,
            MatrixWiringOrder::Linear,
        );
        // 0  4   8
        // 1  5   9
        // 2  6  10
        // 3  7  11
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), Some(0));
        assert_eq!(matrix.pixel_index(Point::new(2, 3)), Some(11));
        assert_eq!(matrix.pixel_index(Point::new(1, 2)), Some(6));
    }

    #[test]
    fn pixel_index_bottom_left_column_major_linear() {
        let matrix = Matrix2d::new(
            Size::new(3, 4),
            MatrixOrigin::BottomLeft,
            MatrixLayout::ColumnMajor,
            MatrixWiringOrder::Linear,
        );
        // 3  7  11
        // 2  6  10
        // 1  5   9
        // 0  4   8
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), Some(3));
        assert_eq!(matrix.pixel_index(Point::new(2, 3)), Some(8));
        assert_eq!(matrix.pixel_index(Point::new(1, 2)), Some(5));
    }

    #[test]
    fn pixel_index_top_right_column_major_linear() {
        let matrix = Matrix2d::new(
            Size::new(3, 4),
            MatrixOrigin::TopRight,
            MatrixLayout::ColumnMajor,
            MatrixWiringOrder::Linear,
        );
        //  8  4  0
        //  9  5  1
        // 10  6  2
        // 11  7  3
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), Some(8));
        assert_eq!(matrix.pixel_index(Point::new(2, 3)), Some(3));
        assert_eq!(matrix.pixel_index(Point::new(1, 2)), Some(6));
    }

    #[test]
    fn pixel_index_bottom_right_column_major_linear() {
        let matrix = Matrix2d::new(
            Size::new(3, 4),
            MatrixOrigin::BottomRight,
            MatrixLayout::ColumnMajor,
            MatrixWiringOrder::Linear,
        );
        // 11  7  3
        // 10  6  2
        //  9  5  1
        //  8  4  0
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), Some(11));
        assert_eq!(matrix.pixel_index(Point::new(2, 3)), Some(0));
        assert_eq!(matrix.pixel_index(Point::new(1, 2)), Some(5));
    }

    #[test]
    fn pixel_index_top_left_row_major_linear() {
        let matrix = Matrix2d::new(
            Size::new(3, 4),
            MatrixOrigin::TopLeft,
            MatrixLayout::RowMajor,
            MatrixWiringOrder::Linear,
        );
        // 0   1   2
        // 3   4   5
        // 6   7   8
        // 9  10  11
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), Some(0));
        assert_eq!(matrix.pixel_index(Point::new(2, 3)), Some(11));
        assert_eq!(matrix.pixel_index(Point::new(1, 2)), Some(7));
    }

    #[test]
    fn pixel_index_bottom_left_row_major_linear() {
        let matrix = Matrix2d::new(
            Size::new(3, 4),
            MatrixOrigin::BottomLeft,
            MatrixLayout::RowMajor,
            MatrixWiringOrder::Linear,
        );
        // 9  10  11
        // 6   7   8
        // 3   4   5
        // 0   1   2
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), Some(9));
        assert_eq!(matrix.pixel_index(Point::new(2, 3)), Some(2));
        assert_eq!(matrix.pixel_index(Point::new(1, 2)), Some(4));
    }

    #[test]
    fn pixel_index_top_right_row_major_linear() {
        let matrix = Matrix2d::new(
            Size::new(3, 4),
            MatrixOrigin::TopRight,
            MatrixLayout::RowMajor,
            MatrixWiringOrder::Linear,
        );
        //  2   1  0
        //  5   4  3
        //  8   7  6
        // 11  10  9
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), Some(2));
        assert_eq!(matrix.pixel_index(Point::new(2, 3)), Some(9));
        assert_eq!(matrix.pixel_index(Point::new(1, 2)), Some(7));
    }

    #[test]
    fn pixel_index_bottom_right_row_major_linear() {
        let matrix = Matrix2d::new(
            Size::new(3, 4),
            MatrixOrigin::BottomRight,
            MatrixLayout::RowMajor,
            MatrixWiringOrder::Linear,
        );
        // 11  10  9
        //  8   7  6
        //  5   4  3
        //  2   1  0
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), Some(11));
        assert_eq!(matrix.pixel_index(Point::new(2, 3)), Some(0));
        assert_eq!(matrix.pixel_index(Point::new(1, 2)), Some(4));
    }

    #[test]
    fn pixel_index_top_left_column_major_serpentine() {
        let matrix = Matrix2d::new(
            Size::new(3, 4),
            MatrixOrigin::TopLeft,
            MatrixLayout::ColumnMajor,
            MatrixWiringOrder::Serpentine,
        );
        // 0  7  8
        // 1  6  9
        // 2  5 10
        // 3  4 11
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), Some(0));
        assert_eq!(matrix.pixel_index(Point::new(2, 3)), Some(11));
        assert_eq!(matrix.pixel_index(Point::new(1, 2)), Some(5));
    }

    #[test]
    fn pixel_index_bottom_left_column_major_serpentine() {
        let matrix = Matrix2d::new(
            Size::new(3, 4),
            MatrixOrigin::BottomLeft,
            MatrixLayout::ColumnMajor,
            MatrixWiringOrder::Serpentine,
        );
        // 3  4 11
        // 2  5 10
        // 1  6  9
        // 0  7  8
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), Some(3));
        assert_eq!(matrix.pixel_index(Point::new(2, 3)), Some(8));
        assert_eq!(matrix.pixel_index(Point::new(1, 2)), Some(6));
    }

    #[test]
    fn pixel_index_top_right_column_major_serpentine() {
        let matrix = Matrix2d::new(
            Size::new(3, 4),
            MatrixOrigin::TopRight,
            MatrixLayout::ColumnMajor,
            MatrixWiringOrder::Serpentine,
        );
        //  8  7  0
        //  9  6  1
        // 10  5  2
        // 11  4  3
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), Some(8));
        assert_eq!(matrix.pixel_index(Point::new(2, 3)), Some(3));
        assert_eq!(matrix.pixel_index(Point::new(1, 2)), Some(5));
    }

    #[test]
    fn pixel_index_bottom_right_column_major_serpentine() {
        let matrix = Matrix2d::new(
            Size::new(3, 4),
            MatrixOrigin::BottomRight,
            MatrixLayout::ColumnMajor,
            MatrixWiringOrder::Serpentine,
        );
        // 11  4  3
        // 10  5  2
        //  9  6  1
        //  8  7  0
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), Some(11));
        assert_eq!(matrix.pixel_index(Point::new(2, 3)), Some(0));
        assert_eq!(matrix.pixel_index(Point::new(1, 2)), Some(6));
    }

    #[test]
    fn pixel_index_top_left_row_major_serpentine() {
        let matrix = Matrix2d::new(
            Size::new(3, 4),
            MatrixOrigin::TopLeft,
            MatrixLayout::RowMajor,
            MatrixWiringOrder::Serpentine,
        );
        //  0  1  2
        //  5  4  3
        //  6  7  8
        // 11 10  9
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), Some(0));
        assert_eq!(matrix.pixel_index(Point::new(2, 3)), Some(9));
        assert_eq!(matrix.pixel_index(Point::new(1, 2)), Some(7));
    }

    #[test]
    fn pixel_index_bottom_left_row_major_serpentine() {
        let matrix = Matrix2d::new(
            Size::new(3, 4),
            MatrixOrigin::BottomLeft,
            MatrixLayout::RowMajor,
            MatrixWiringOrder::Serpentine,
        );
        // 11 10  9
        //  6  7  8
        //  5  4  3
        //  0  1  2
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), Some(11));
        assert_eq!(matrix.pixel_index(Point::new(2, 3)), Some(2));
        assert_eq!(matrix.pixel_index(Point::new(1, 2)), Some(4));
    }

    #[test]
    fn pixel_index_top_right_row_major_serpentine() {
        let matrix = Matrix2d::new(
            Size::new(3, 4),
            MatrixOrigin::TopRight,
            MatrixLayout::RowMajor,
            MatrixWiringOrder::Serpentine,
        );
        // 2  1  0
        // 3  4  5
        // 8  7  6
        // 9 10 11
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), Some(2));
        assert_eq!(matrix.pixel_index(Point::new(2, 3)), Some(11));
        assert_eq!(matrix.pixel_index(Point::new(1, 2)), Some(7));
    }

    #[test]
    fn pixel_index_bottom_right_row_major_serpentine() {
        let matrix = Matrix2d::new(
            Size::new(3, 4),
            MatrixOrigin::BottomRight,
            MatrixLayout::RowMajor,
            MatrixWiringOrder::Serpentine,
        );
        // 9 10 11
        // 8  7  6
        // 3  4  5
        // 2  1  0
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), Some(9));
        assert_eq!(matrix.pixel_index(Point::new(2, 3)), Some(0));
        assert_eq!(matrix.pixel_index(Point::new(1, 2)), Some(4));
    }

    #[test]
    fn pixel_index_points_outside_the_matrix() {
        let matrix = Matrix2d::new(
            Size::new(3, 4),
            MatrixOrigin::TopLeft,
            MatrixLayout::RowMajor,
            MatrixWiringOrder::Linear,
        );
        assert_eq!(matrix.pixel_index(Point::new(-1, 0)), None);
        assert_eq!(matrix.pixel_index(Point::new(0, -1)), None);
        assert_eq!(matrix.pixel_index(Point::new(4, 0)), None);
        assert_eq!(matrix.pixel_index(Point::new(0, 5)), None);
    }

    #[test]
    fn pixel_index_empty_matrix() {
        let matrix = Matrix2d::new(
            Size::new(0, 1),
            MatrixOrigin::TopLeft,
            MatrixLayout::RowMajor,
            MatrixWiringOrder::Linear,
        );
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), None);

        let matrix = Matrix2d::new(
            Size::new(1, 0),
            MatrixOrigin::TopLeft,
            MatrixLayout::RowMajor,
            MatrixWiringOrder::Linear,
        );
        assert_eq!(matrix.pixel_index(Point::new(0, 0)), None);
    }
}
