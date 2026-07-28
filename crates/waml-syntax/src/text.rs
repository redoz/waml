use std::{fmt, ops::Add, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TextSize(u32);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TextRange {
    start: TextSize,
    end: TextSize,
}

#[derive(Clone, Debug)]
pub struct SourceText {
    source: Arc<String>,
}

#[derive(Clone, Debug)]
pub struct LineIndex {
    line_starts: Arc<[TextSize]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineColumn {
    pub line: u32,
    pub byte_column: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkdownDialect {
    CommonMarkCurrent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextError {
    SourceTooLarge { bytes: usize },
    WidthOverflow { left: TextSize, right: TextSize },
    ReversedRange { start: TextSize, end: TextSize },
    OutOfBounds { range: TextRange, len: TextSize },
    NonUtf8Boundary { offset: TextSize },
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceTooLarge { bytes } => write!(f, "source is too large: {bytes} bytes"),
            Self::WidthOverflow { left, right } => {
                write!(f, "text width overflow: {} + {}", left.0, right.0)
            }
            Self::ReversedRange { start, end } => {
                write!(f, "reversed text range: {}..{}", start.0, end.0)
            }
            Self::OutOfBounds { range, len } => write!(
                f,
                "range {}..{} exceeds source length {}",
                range.start.0, range.end.0, len.0
            ),
            Self::NonUtf8Boundary { offset } => {
                write!(f, "offset {} is not a UTF-8 boundary", offset.0)
            }
        }
    }
}
impl std::error::Error for TextError {}

impl TextSize {
    pub fn try_from_usize(value: usize) -> Result<Self, TextError> {
        u32::try_from(value)
            .map(Self)
            .map_err(|_| TextError::SourceTooLarge { bytes: value })
    }
    pub fn to_usize(self) -> usize {
        self.0 as usize
    }
    pub fn checked_add(self, rhs: Self) -> Result<Self, TextError> {
        self.0
            .checked_add(rhs.0)
            .map(Self)
            .ok_or(TextError::WidthOverflow {
                left: self,
                right: rhs,
            })
    }
}
impl TryFrom<usize> for TextSize {
    type Error = TextError;
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::try_from_usize(value)
    }
}
impl Add for TextSize {
    type Output = Result<Self, TextError>;
    fn add(self, rhs: Self) -> Self::Output {
        self.checked_add(rhs)
    }
}

impl TextRange {
    pub fn new(start: TextSize, end: TextSize) -> Result<Self, TextError> {
        (start <= end)
            .then_some(Self { start, end })
            .ok_or(TextError::ReversedRange { start, end })
    }
    pub fn start(self) -> TextSize {
        self.start
    }
    pub fn end(self) -> TextSize {
        self.end
    }
    pub fn len(self) -> TextSize {
        TextSize(self.end.0 - self.start.0)
    }
    pub fn contains(self, offset: TextSize) -> bool {
        self.start <= offset && offset < self.end
    }
    pub fn cover(self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

impl SourceText {
    pub fn from_shared(source: Arc<String>) -> Result<Self, TextError> {
        TextSize::try_from_usize(source.len())?;
        Ok(Self { source })
    }
    pub fn len(&self) -> TextSize {
        TextSize(self.source.len() as u32)
    }
    pub fn shared(&self) -> &Arc<String> {
        &self.source
    }
    pub fn slice(&self, range: TextRange) -> Result<&str, TextError> {
        if range.end > self.len() {
            return Err(TextError::OutOfBounds {
                range,
                len: self.len(),
            });
        }
        if !self.source.is_char_boundary(range.start.to_usize()) {
            return Err(TextError::NonUtf8Boundary {
                offset: range.start,
            });
        }
        if !self.source.is_char_boundary(range.end.to_usize()) {
            return Err(TextError::NonUtf8Boundary { offset: range.end });
        }
        Ok(&self.source[range.start.to_usize()..range.end.to_usize()])
    }
}

impl LineIndex {
    pub fn new(source: &SourceText) -> Self {
        let mut starts = vec![TextSize(0)];
        let bytes = source.source.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                i += 2;
                starts.push(TextSize(i as u32));
            } else if bytes[i] == b'\n' {
                i += 1;
                starts.push(TextSize(i as u32));
            } else {
                i += 1;
            }
        }
        Self {
            line_starts: starts.into(),
        }
    }
    pub fn line_col(&self, source: &SourceText, offset: TextSize) -> Result<LineColumn, TextError> {
        source.slice(TextRange::new(TextSize(0), offset)?)?;
        let index = self
            .line_starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1);
        Ok(LineColumn {
            line: index as u32,
            byte_column: offset.0 - self.line_starts[index].0,
        })
    }
    pub fn utf16_column(&self, source: &SourceText, offset: TextSize) -> Result<u32, TextError> {
        let column = self.line_col(source, offset)?;
        let start = self.line_starts[column.line as usize];
        Ok(source
            .slice(TextRange::new(start, offset)?)?
            .encode_utf16()
            .count() as u32)
    }
}
