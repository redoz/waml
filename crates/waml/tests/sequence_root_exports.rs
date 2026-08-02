use waml::uml::{BindingSyntax, GateSyntax, InteractionUseSyntax};

#[test]
fn sequence_syntax_types_are_available_from_the_uml_root() {
    fn accepts_root_types(
        _: Option<GateSyntax>,
        _: Option<InteractionUseSyntax>,
        _: Option<BindingSyntax>,
    ) {
    }

    accepts_root_types(None, None, None);
}
