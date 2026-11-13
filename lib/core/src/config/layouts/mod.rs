mod circular_array;
mod linear_array;
mod matrix_2d;

pub use circular_array::*;
pub use linear_array::*;
pub use matrix_2d::*;

pub use embedded_graphics::geometry::{Point, Size};

use serde::{Deserialize, Serialize};

/// The physical layout of a span of pixels on a single channel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[serde(rename_all = "snake_case")]
pub enum PixelLayout {
    /// Pixels arranged in a line
    LinearArray(LinearArray),

    /// Pixels arranged in a circle
    CircularArray(CircularArray),

    /// Pixels arranged in a 2D matrix
    Matrix2d(Matrix2d),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_linear_array_from_json() {
        let layout = crate::io::decode_json::<PixelLayout>(br#"{"linear_array":{}}"#).unwrap();

        assert_eq!(layout, PixelLayout::LinearArray(LinearArray::default()));
    }

    #[test]
    fn deserializes_circular_array_from_json() {
        let layout =
            crate::io::decode_json::<PixelLayout>(br#"{"circular_array":{"degrees":180.0}}"#)
                .unwrap();

        match layout {
            PixelLayout::CircularArray(array) => assert_eq!(*array.degrees(), 180.0),
            _ => panic!(),
        }
    }

    #[test]
    fn deserializes_matrix_2d_from_json() {
        let layout = crate::io::decode_json::<PixelLayout>(
            br#"{"matrix2d":{"width":2,"height":2,"origin":"TopLeft","layout":"RowMajor","wiring_order":"Serpentine"}}"#,
        )
        .unwrap();

        match layout {
            PixelLayout::Matrix2d(matrix) => {
                assert_eq!(matrix.size(), Size::new(2, 2));
                assert_eq!(matrix.pixel_index(Point::new(0, 1)), Some(3));
            }
            _ => panic!(),
        }
    }
}
