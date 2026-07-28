//! Semantic value types for UML diagram layout.
//!
//! Lossless parsing and writing belong to `uml::syntax` and `waml-syntax`.

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LayoutStatement {
    Placement {
        operands: Vec<Operand>,
        directions: Vec<Direction>,
    },
    Alignment {
        left: Anchored,
        right: Anchored,
    },
    Standalone(Operand),
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Direction {
    LeftOf,
    RightOf,
    Above,
    Below,
    AboveLeft,
    AboveRight,
    BelowLeft,
    BelowRight,
}

impl Direction {
    pub fn opposite(self) -> Direction {
        use Direction::*;
        match self {
            LeftOf => RightOf,
            RightOf => LeftOf,
            Above => Below,
            Below => Above,
            AboveLeft => BelowRight,
            BelowRight => AboveLeft,
            AboveRight => BelowLeft,
            BelowLeft => AboveRight,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Anchored {
    pub edge: Option<Edge>,
    pub operand: Operand,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
    Center,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Operand {
    pub ref_: OperandRef,
    pub axis: Option<Axis>,
    pub hints: Vec<Hint>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Axis {
    Row,
    Column,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OperandRef {
    Name(NameRef),
    InlineGroup { axis: Axis, items: Vec<Operand> },
    Paren(Box<Operand>),
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NameRef {
    Link { title: String, slug: String },
    Bare(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Hint {
    Shape(Shape),
    Margin(Margin),
    Flag(Flag),
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Shape {
    Frame,
    Box,
    Shrink,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Margin {
    No,
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Flag {
    Emphasized,
    Collapsed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinkRef {
    pub title: String,
    pub slug: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FlowTargetRef {
    Local(String),
    Link(LinkRef),
}

#[cfg(test)]
mod tests {
    use super::Direction::*;

    #[test]
    fn opposite_is_an_involution() {
        for direction in [
            LeftOf, RightOf, Above, Below, AboveLeft, AboveRight, BelowLeft, BelowRight,
        ] {
            assert_eq!(direction.opposite().opposite(), direction);
        }
    }
}
