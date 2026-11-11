use crate::customglyph::*;

pub const TOP_LEFT_ROUNDED_CORNER: &[Poly] = &[Poly {
    path: &[PolyCommand::Oval {
        center: (BlockCoord::One, BlockCoord::One),
        radiuses: (BlockCoord::One, BlockCoord::One),
    }],
    intensity: BlockAlpha::Full,
    style: PolyStyle::Fill,
}];

pub const BOTTOM_LEFT_ROUNDED_CORNER: &[Poly] = &[Poly {
    path: &[PolyCommand::Oval {
        center: (BlockCoord::One, BlockCoord::Zero),
        radiuses: (BlockCoord::One, BlockCoord::One),
    }],
    intensity: BlockAlpha::Full,
    style: PolyStyle::Fill,
}];

pub const TOP_RIGHT_ROUNDED_CORNER: &[Poly] = &[Poly {
    path: &[PolyCommand::Oval {
        center: (BlockCoord::Zero, BlockCoord::One),
        radiuses: (BlockCoord::One, BlockCoord::One),
    }],
    intensity: BlockAlpha::Full,
    style: PolyStyle::Fill,
}];

pub const BOTTOM_RIGHT_ROUNDED_CORNER: &[Poly] = &[Poly {
    path: &[PolyCommand::Oval {
        center: (BlockCoord::Zero, BlockCoord::Zero),
        radiuses: (BlockCoord::One, BlockCoord::One),
    }],
    intensity: BlockAlpha::Full,
    style: PolyStyle::Fill,
}];

// Superellipse corners (|x|^5 + |y|^5 = 1), as in Apple's continuous
// corners: the curve eases into the straight edges instead of meeting them
// at a sharp change of curvature, as a circular arc does.

/// Fills the inside of a top-left superellipse corner.
pub const TOP_LEFT_SUPERELLIPSE_CORNER: &[Poly] = &[Poly {
    path: &[
        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::One),
        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(56, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(1, 100), BlockCoord::Frac(42, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(3, 100), BlockCoord::Frac(32, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(6, 100), BlockCoord::Frac(24, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(9, 100), BlockCoord::Frac(18, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(13, 100), BlockCoord::Frac(13, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(18, 100), BlockCoord::Frac(9, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(24, 100), BlockCoord::Frac(6, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(32, 100), BlockCoord::Frac(3, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(42, 100), BlockCoord::Frac(1, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(56, 100), BlockCoord::Zero),
        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
        PolyCommand::Close,
    ],
    intensity: BlockAlpha::Full,
    style: PolyStyle::Fill,
}];

/// Fills the inside of a top-right superellipse corner.
pub const TOP_RIGHT_SUPERELLIPSE_CORNER: &[Poly] = &[Poly {
    path: &[
        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::One),
        PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(56, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(99, 100), BlockCoord::Frac(42, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(97, 100), BlockCoord::Frac(32, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(94, 100), BlockCoord::Frac(24, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(91, 100), BlockCoord::Frac(18, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(87, 100), BlockCoord::Frac(13, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(82, 100), BlockCoord::Frac(9, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(76, 100), BlockCoord::Frac(6, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(68, 100), BlockCoord::Frac(3, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(58, 100), BlockCoord::Frac(1, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(44, 100), BlockCoord::Zero),
        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Zero),
        PolyCommand::Close,
    ],
    intensity: BlockAlpha::Full,
    style: PolyStyle::Fill,
}];

/// Fills the inside of a bottom-left superellipse corner.
pub const BOTTOM_LEFT_SUPERELLIPSE_CORNER: &[Poly] = &[Poly {
    path: &[
        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Zero),
        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Zero),
        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(44, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(1, 100), BlockCoord::Frac(58, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(3, 100), BlockCoord::Frac(68, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(6, 100), BlockCoord::Frac(76, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(9, 100), BlockCoord::Frac(82, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(13, 100), BlockCoord::Frac(87, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(18, 100), BlockCoord::Frac(91, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(24, 100), BlockCoord::Frac(94, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(32, 100), BlockCoord::Frac(97, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(42, 100), BlockCoord::Frac(99, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(56, 100), BlockCoord::One),
        PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
        PolyCommand::Close,
    ],
    intensity: BlockAlpha::Full,
    style: PolyStyle::Fill,
}];

/// Fills the inside of a bottom-right superellipse corner.
pub const BOTTOM_RIGHT_SUPERELLIPSE_CORNER: &[Poly] = &[Poly {
    path: &[
        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(44, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(99, 100), BlockCoord::Frac(58, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(97, 100), BlockCoord::Frac(68, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(94, 100), BlockCoord::Frac(76, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(91, 100), BlockCoord::Frac(82, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(87, 100), BlockCoord::Frac(87, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(82, 100), BlockCoord::Frac(91, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(76, 100), BlockCoord::Frac(94, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(68, 100), BlockCoord::Frac(97, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(58, 100), BlockCoord::Frac(99, 100)),
        PolyCommand::LineTo(BlockCoord::Frac(44, 100), BlockCoord::One),
        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
        PolyCommand::Close,
    ],
    intensity: BlockAlpha::Full,
    style: PolyStyle::Fill,
}];
