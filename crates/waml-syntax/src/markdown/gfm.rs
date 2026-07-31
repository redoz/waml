use std::{num::NonZeroU64, sync::Arc};
use crate::SyntaxAnnotation;
#[derive(Clone, Copy, Debug, Eq, PartialEq)] pub enum TableAlignment { None, Left, Center, Right }
#[derive(Clone, Copy, Debug, Eq, PartialEq)] pub enum TaskListState { Unchecked, Checked }
#[derive(Clone, Copy, Debug, Eq, PartialEq)] pub enum HtmlTagFilter { Allowed, Disallowed }
pub(crate) const HTML_TAG_FILTER: &str = "waml.markdown.gfm.html_tag_filter";
pub(crate) fn annotation(id: u64, kind: &'static str, data: &'static str) -> SyntaxAnnotation { SyntaxAnnotation::new(NonZeroU64::new(id).expect("non-zero"), kind, Some(Arc::from(data))) }
pub(crate) fn task_marker(source: &str, at: usize, end: usize) -> Option<(usize, TaskListState)> { let rest = source.get(at..end)?; let state = match rest.as_bytes().get(..3)? { b"[ ]" => TaskListState::Unchecked, b"[x]" | b"[X]" => TaskListState::Checked, _ => return None }; rest.as_bytes().get(3).is_some_and(u8::is_ascii_whitespace).then_some((at + 3, state)) }
pub(crate) fn filtered_tag(source: &str) -> Option<(usize, usize, HtmlTagFilter)> { let bytes = source.as_bytes(); let mut at = usize::from(bytes.first() == Some(&b'<')); if bytes.get(at) == Some(&b'/') { at += 1; } while bytes.get(at).is_some_and(u8::is_ascii_whitespace) { at += 1; } let start = at; while bytes.get(at).is_some_and(u8::is_ascii_alphanumeric) { at += 1; } (start < at).then(|| { let name = &source[start..at]; let blocked = ["title", "textarea", "style", "xmp", "iframe", "noembed", "noframes", "script", "plaintext"]; (start, at, if blocked.iter().any(|item| name.eq_ignore_ascii_case(item)) { HtmlTagFilter::Disallowed } else { HtmlTagFilter::Allowed }) }) }
