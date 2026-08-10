pub mod completion;
pub mod document;
pub mod edit;
pub mod gutter;
pub mod history;
pub mod ime;
pub mod input;
pub mod layout;
pub mod motion;
pub mod presentation;
pub use presentation::EditorEmphasis;
pub mod reading;
pub mod selection;
pub mod session;
pub mod squiggle;
pub mod syntax {
    pub use waml_syntax::{
        parse_markdown, reparse_markdown, ChangeMap, DocumentRevision, FullReparseReason,
        MarkdownDialect, MarkdownSyntaxSnapshot, MarkdownSyntaxUpdate, SourceText, SyntaxSeverity,
        TextChange, TextRange, TextSize,
    };
}
pub mod unicode;
pub mod widget;

pub fn script_mod(vm: &mut makepad_widgets::ScriptVm) -> makepad_widgets::ScriptValue {
    widget::register_script_mod(vm)
}

pub fn live_design(cx: &mut makepad_widgets::Cx) {
    cx.with_vm(makepad_widgets::script_mod);
    cx.with_vm(script_mod);
}

#[cfg(test)]
mod syntax_facade_tests {
    use super::syntax::{
        parse_markdown, reparse_markdown, ChangeMap, DocumentRevision, FullReparseReason,
        MarkdownDialect, MarkdownSyntaxSnapshot, MarkdownSyntaxUpdate, SourceText, SyntaxSeverity,
        TextChange, TextRange, TextSize,
    };

    #[test]
    fn syntax_facade_exposes_the_harness_incremental_parse_surface() {
        let _ = parse_markdown;
        let _ = reparse_markdown;
        let _: Option<ChangeMap> = None;
        let _: Option<DocumentRevision> = None;
        let _: Option<FullReparseReason> = None;
        let _: Option<MarkdownDialect> = None;
        let _: Option<MarkdownSyntaxSnapshot> = None;
        let _: Option<MarkdownSyntaxUpdate> = None;
        let _: Option<SourceText> = None;
        let _: Option<SyntaxSeverity> = None;
        let _: Option<TextChange> = None;
        let _: Option<TextRange> = None;
        let _: Option<TextSize> = None;
    }
}
