use crate::document::MarkdownDocumentSnapshot;
use unicode_segmentation::UnicodeSegmentation;
use waml_syntax::{TextError, TextSize};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Utf16Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionError {
    LineOutOfBounds { line: u32 },
    Utf16ColumnOutOfBounds { line: u32, character: u32 },
    SplitUtf16Scalar { line: u32, character: u32 },
    Text(TextError),
}

pub fn previous_grapheme_boundary(text: &str, offset: usize) -> usize {
    UnicodeSegmentation::grapheme_indices(text, true)
        .map(|(index, _)| index)
        .take_while(|index| *index < offset)
        .last()
        .unwrap_or(0)
}

pub fn next_grapheme_boundary(text: &str, offset: usize) -> usize {
    UnicodeSegmentation::grapheme_indices(text, true)
        .map(|(index, _)| index)
        .find(|index| *index > offset)
        .unwrap_or(text.len())
}

pub fn word_range_at(text: &str, offset: usize) -> Option<(usize, usize)> {
    UnicodeSegmentation::unicode_word_indices(text).find_map(|(start, word)| {
        let end = start + word.len();
        (start <= offset && offset < end).then_some((start, end))
    })
}

pub fn offset_to_utf16(
    snapshot: &MarkdownDocumentSnapshot,
    offset: TextSize,
) -> Result<Utf16Position, PositionError> {
    let line_col = snapshot
        .line_index()
        .line_col(snapshot.text(), offset)
        .map_err(PositionError::Text)?;
    let character = snapshot
        .line_index()
        .utf16_column(snapshot.text(), offset)
        .map_err(PositionError::Text)?;
    Ok(Utf16Position {
        line: line_col.line,
        character,
    })
}

pub fn utf16_to_offset(
    snapshot: &MarkdownDocumentSnapshot,
    position: Utf16Position,
) -> Result<TextSize, PositionError> {
    let text = snapshot.text().shared().as_str();
    let lines = logical_lines(text);
    let Some((start, end)) = lines.get(position.line as usize).copied() else {
        return Err(PositionError::LineOutOfBounds {
            line: position.line,
        });
    };
    let line = &text[start..end];
    let mut column = 0_u32;
    for (index, character) in line.char_indices() {
        if column == position.character {
            return Ok(TextSize::try_from_usize(start + index)
                .expect("a source offset always fits TextSize"));
        }
        let width = character.len_utf16() as u32;
        if column < position.character && position.character < column + width {
            return Err(PositionError::SplitUtf16Scalar {
                line: position.line,
                character: position.character,
            });
        }
        column += width;
    }
    if column == position.character {
        Ok(TextSize::try_from_usize(end).expect("a source offset always fits TextSize"))
    } else {
        Err(PositionError::Utf16ColumnOutOfBounds {
            line: position.line,
            character: position.character,
        })
    }
}

pub fn logical_lines(text: &str) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
            lines.push((start, index));
            index += 2;
            start = index;
        } else if bytes[index] == b'\n' {
            lines.push((start, index));
            index += 1;
            start = index;
        } else {
            index += 1;
        }
    }
    lines.push((start, text.len()));
    lines
}
