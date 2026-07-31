use crate::{ChangeMap, SourceText, TextChange, TextRange, TextSize};

/// Returns true when a change touches a reference-definition line.
///
/// The check is deliberately conservative. A false positive only expands the
/// incremental work set; a false negative could leave a reference annotation
/// with an obsolete destination.
pub(crate) fn reference_definition_changed(
    old: &SourceText,
    new: &SourceText,
    changes: &[TextChange],
    map: &ChangeMap,
) -> bool {
    changes.iter().zip(map.segments()).any(|(change, segment)| {
        line_is_definition(old.shared(), change.old_range)
            || line_is_definition(new.shared(), segment.new)
    })
}

fn line_is_definition(source: &str, range: TextRange) -> bool {
    let start = range.start().to_usize().min(source.len());
    let end = range.end().to_usize().min(source.len());
    let line_start = source[..start].rfind('\n').map_or(0, |at| at + 1);
    let line_end = source[end..].find('\n').map_or(source.len(), |at| end + at);
    let line = source[line_start..line_end].trim();
    line.starts_with('[') && line.contains("]:")
}

/// Sort, deduplicate, and merge overlapping or touching non-empty ranges.
pub(crate) fn normalize_affected_ranges(mut ranges: Vec<TextRange>) -> Vec<TextRange> {
    ranges.retain(|range| range.start() < range.end());
    ranges.sort_by_key(|range| (range.start(), range.end()));
    let mut normalized: Vec<TextRange> = Vec::new();
    for range in ranges {
        if let Some(previous) = normalized.last_mut() {
            if range.start() <= previous.end() {
                *previous = previous.cover(range);
                continue;
            }
        }
        normalized.push(range);
    }
    normalized
}

pub(crate) fn full_range(text: &SourceText) -> TextRange {
    TextRange::new(TextSize::try_from_usize(0).unwrap(), text.len()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(start: usize, end: usize) -> TextRange {
        TextRange::new(
            TextSize::try_from_usize(start).unwrap(),
            TextSize::try_from_usize(end).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn normalization_merges_touching_ranges() {
        assert_eq!(
            normalize_affected_ranges(vec![range(8, 9), range(1, 4), range(4, 8)]),
            vec![range(1, 9)]
        );
    }
}
