use super::scan::ScanAlignment;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TableAlignment {
    None,
    Left,
    Center,
    Right,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskListState {
    Unchecked,
    Checked,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HtmlTagFilter {
    Allowed,
    Disallowed,
}
pub(crate) const TABLE_ALIGNMENT: &str = "waml.markdown.gfm.table_alignment";
pub(crate) const TASK_STATE: &str = "waml.markdown.gfm.task_state";
pub(crate) const HTML_TAG_FILTER: &str = "waml.markdown.gfm.html_tag_filter";
impl TableAlignment {
    pub(crate) fn from_scan(value: ScanAlignment) -> Self {
        match value {
            ScanAlignment::None => Self::None,
            ScanAlignment::Left => Self::Left,
            ScanAlignment::Center => Self::Center,
            ScanAlignment::Right => Self::Right,
        }
    }
    pub(crate) const fn data(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
}
impl TaskListState {
    pub(crate) const fn data(self) -> &'static str {
        match self {
            Self::Unchecked => "unchecked",
            Self::Checked => "checked",
        }
    }
}
impl HtmlTagFilter {
    pub(crate) const fn data(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::Disallowed => "disallowed",
        }
    }
}
pub(crate) fn task_marker(source: &str, at: usize, end: usize) -> Option<(usize, TaskListState)> {
    let rest = source.get(at..end)?;
    let state = match rest.as_bytes().get(..3)? {
        b"[ ]" => TaskListState::Unchecked,
        b"[x]" | b"[X]" => TaskListState::Checked,
        _ => return None,
    };
    rest.as_bytes()
        .get(3)
        .is_some_and(u8::is_ascii_whitespace)
        .then_some((at + 3, state))
}
pub(crate) fn classify_html(source: &str) -> (Option<(usize, usize)>, HtmlTagFilter) {
    let Some((_, start, end, blocked)) = html_tags(source).next() else {
        return (None, HtmlTagFilter::Allowed);
    };
    (
        Some((start, end)),
        if blocked {
            HtmlTagFilter::Disallowed
        } else {
            HtmlTagFilter::Allowed
        },
    )
}

pub(crate) fn disallowed_html_tag_ranges(source: &str) -> Vec<(usize, usize)> {
    html_tags(source)
        .filter_map(|(marker, _, end, blocked)| blocked.then_some((marker, end)))
        .collect()
}

pub(crate) fn disallowed_html_tag_name_ranges(source: &str) -> Vec<(usize, usize)> {
    html_tags(source)
        .filter_map(|(_, start, end, blocked)| blocked.then_some((start, end)))
        .collect()
}

fn html_tags(source: &str) -> impl Iterator<Item = (usize, usize, usize, bool)> + '_ {
    source.match_indices('<').filter_map(|(marker_start, _)| {
        let bytes = source.as_bytes();
        let mut at = marker_start + 1;
        if bytes.get(at) == Some(&b'/') {
            at += 1;
        }
        while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
            at += 1;
        }
        let name_start = at;
        while bytes.get(at).is_some_and(u8::is_ascii_alphanumeric) {
            at += 1;
        }
        if name_start == at {
            return None;
        }
        let name = &source[name_start..at];
        let blocked = [
            "title",
            "textarea",
            "style",
            "xmp",
            "iframe",
            "noembed",
            "noframes",
            "script",
            "plaintext",
        ]
        .iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate));
        Some((marker_start, name_start, at, blocked))
    })
}
