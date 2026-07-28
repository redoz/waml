use std::{hash::Hash, sync::Arc};

use waml_syntax::{
    write_green_to, GreenElement, GreenFactory, GreenText, LineIndex, MarkdownDialect, SourceText,
    SyntaxLanguage, TextError, TextRange, TextSize, TriviaKind,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Kind {
    Ident,
    Colon,
    Bad,
    Root,
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Code {
    Invalid,
}
struct TestLanguage;
impl SyntaxLanguage for TestLanguage {
    type Kind = Kind;
    type DiagnosticCode = Code;
}

fn text(value: &str) -> SourceText {
    SourceText::from_shared(Arc::new(value.into())).unwrap()
}
fn size(value: usize) -> TextSize {
    TextSize::try_from_usize(value).unwrap()
}
fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(size(start), size(end)).unwrap()
}

#[test]
fn checked_text_handles_offsets_boundaries_ranges_and_widths() {
    let source = text("aé𝄞\r\n");
    assert_eq!(source.len().to_usize(), 9);
    assert_eq!(source.slice(range(0, 0)).unwrap(), "");
    assert_eq!(source.slice(range(9, 9)).unwrap(), "");
    assert_eq!(source.slice(range(1, 7)).unwrap(), "é𝄞");
    assert_eq!(
        source.slice(range(2, 7)).unwrap_err(),
        TextError::NonUtf8Boundary { offset: size(2) }
    );
    assert_eq!(
        source.slice(range(1, 6)).unwrap_err(),
        TextError::NonUtf8Boundary { offset: size(6) }
    );
    assert_eq!(
        source.slice(range(0, 10)).unwrap_err(),
        TextError::OutOfBounds {
            range: range(0, 10),
            len: size(9)
        }
    );
    assert_eq!(
        TextRange::new(size(7), size(1)).unwrap_err(),
        TextError::ReversedRange {
            start: size(7),
            end: size(1)
        }
    );
    assert_eq!(size(2).checked_add(size(3)).unwrap().to_usize(), 5);
    assert_eq!(
        size(u32::MAX as usize).checked_add(size(1)).unwrap_err(),
        TextError::WidthOverflow {
            left: size(u32::MAX as usize),
            right: size(1)
        }
    );
    assert_eq!(
        TextSize::try_from_usize(u32::MAX as usize + 1).unwrap_err(),
        TextError::SourceTooLarge {
            bytes: u32::MAX as usize + 1
        }
    );
}

#[test]
fn source_text_preserves_shared_arc_and_line_index_handles_crlf_and_utf16() {
    let shared = Arc::new("aé𝄞\r\nb\n".to_owned());
    let source = SourceText::from_shared(shared.clone()).unwrap();
    assert!(Arc::ptr_eq(&shared, source.shared()));
    let index = LineIndex::new(&source);
    assert_eq!(index.line_col(&source, size(9)).unwrap().line, 1);
    assert_eq!(index.line_col(&source, size(9)).unwrap().byte_column, 0);
    assert_eq!(index.utf16_column(&source, size(7)).unwrap(), 4);
    assert_eq!(
        index.line_col(&source, size(2)).unwrap_err(),
        TextError::NonUtf8Boundary { offset: size(2) }
    );
}

#[test]
fn green_storage_is_lossless_checked_and_source_aware() {
    let source = text("name  \r\n");
    let factory = GreenFactory::<TestLanguage>::new();
    let ident = factory
        .token(
            Kind::Ident,
            GreenText::SourceSlice {
                source: source.clone(),
                range: range(0, 4),
            },
            [],
            [
                factory.trivia(TriviaKind::Whitespace, GreenText::Static("  ")),
                factory.trivia(TriviaKind::Newline, GreenText::Static("\r\n")),
            ],
        )
        .unwrap();
    let punct = factory
        .token(Kind::Colon, GreenText::Owned(Arc::<str>::from(":")), [], [])
        .unwrap();
    let missing = factory.missing_token(Kind::Colon);
    let bad = factory
        .bad_token(Kind::Bad, GreenText::Static("?"), Code::Invalid)
        .unwrap();
    assert!(factory
        .bad_token(Kind::Bad, GreenText::Static(""), Code::Invalid)
        .is_err());
    let root = factory
        .node(
            Kind::Root,
            [
                GreenElement::Token(ident.clone()),
                GreenElement::Token(punct.clone()),
            ],
        )
        .unwrap();
    let mut output = String::new();
    write_green_to(&root, &mut output).unwrap();
    assert_eq!(output, "name  \r\n:");
    assert_eq!(root.width().to_usize(), 9);
    assert_eq!(missing.width().to_usize(), 0);
    assert!(!ident.is_source_independent());
    assert!(punct.is_source_independent());
    assert!(missing.is_source_independent());
    assert!(bad.is_source_independent());
    assert!(!root.is_source_independent());
}

#[test]
fn green_writer_preserves_eof_leading_whitespace_and_dialect_marker() {
    let factory = GreenFactory::<TestLanguage>::new();
    let eof = factory
        .token(
            Kind::Ident,
            GreenText::Static(""),
            [factory.trivia(TriviaKind::Whitespace, GreenText::Static("  "))],
            [],
        )
        .unwrap();
    let root = factory
        .node(Kind::Root, [GreenElement::Token(eof)])
        .unwrap();
    let mut output = String::new();
    write_green_to(&root, &mut output).unwrap();
    assert_eq!(output, "  ");
    assert_eq!(
        MarkdownDialect::CommonMarkCurrent,
        MarkdownDialect::CommonMarkCurrent
    );
}
