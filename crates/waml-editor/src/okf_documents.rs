use crate::document::{
    DocumentCapabilities, DocumentDescriptor, DocumentPresentation, NavCategory, OpenDocument,
};
use crate::icons::Icon;
use crate::view_history::DocumentLocator;
use makepad_widgets::Vec4;
use waml::view::surface::SurfaceId;

thread_local! {
    /// All generic Markdown documents on the UI thread share the renderer
    /// cache. Each `ReadingView` still owns a separate lease and request set.
    static MARKDOWN_EXTENSIONS: crate::markdown_extensions::SharedMarkdownExtensionHost =
        crate::markdown_extensions::EditorMarkdownExtensionHost::shared();
}

fn markdown_extensions() -> crate::markdown_extensions::SharedMarkdownExtensionHost {
    MARKDOWN_EXTENSIONS.with(Clone::clone)
}

pub fn generic_okf_accent() -> Option<Vec4> {
    Some(crate::accent::bucket_color(
        crate::node_style::AccentBucket::None,
    ))
}

pub fn presentation(
    analysis: &waml::analysis::OkfAnalysis,
    concept_id: &str,
) -> Option<DocumentPresentation> {
    describe(analysis, concept_id).map(|descriptor| descriptor.presentation)
}

pub fn describe(
    analysis: &waml::analysis::OkfAnalysis,
    concept_id: &str,
) -> Option<DocumentDescriptor> {
    analysis.bundle.concept(concept_id)?;
    Some(DocumentDescriptor {
        presentation: DocumentPresentation {
            // The generic-document glyph the tree rows, doc tabs, and the
            // inspector's node lead all share; a describe/open split here
            // makes the tree row disagree with the tab for the same concept.
            icon: Icon::FileText,
            accent: generic_okf_accent(),
            category: NavCategory::OkfDocument,
        },
        capabilities: DocumentCapabilities::default(),
    })
}

pub fn open_with_asset_host(
    analysis: &waml::analysis::OkfAnalysis,
    concept_id: &str,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
    emphasis: waml_markdown_editor::EditorEmphasis,
) -> Option<OpenDocument> {
    let concept = analysis.bundle.concept(concept_id)?;
    let presentation = presentation(analysis, concept_id)?;
    let locator = DocumentLocator::concept(concept_id, SurfaceId::markdown());
    Some(OpenDocument {
        tab_id: crate::documents::tab_id_for(&locator),
        locator,
        title: concept.title.clone().unwrap_or_else(|| {
            concept_id
                .rsplit('/')
                .next()
                .unwrap_or(concept_id)
                .to_string()
        }),
        presentation,
        view: Box::new(
            crate::generic_okf_view::GenericOkfView::new_with_asset_host(
                concept_id.to_string(),
                assets.clone(),
                markdown_extensions(),
                emphasis,
            ),
        ),
    })
}

pub fn open_markdown_for_target(
    analysis: &waml::analysis::OkfAnalysis,
    target: &waml::view::row::RowTarget,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
    emphasis: waml_markdown_editor::EditorEmphasis,
) -> Option<OpenDocument> {
    use waml::view::row::RowTarget;
    match target {
        RowTarget::Concept(concept_id) => {
            open_with_asset_host(analysis, concept_id, assets, emphasis)
        }
        RowTarget::Folder(address) => {
            let key = source_key_for(target)?;
            if !source_document_exists(analysis, &key) {
                return None;
            }
            let title = analysis
                .bundle
                .index(address)
                .and_then(|index| index.title.clone())
                .unwrap_or_else(|| {
                    address
                        .rsplit('/')
                        .next()
                        .filter(|last| !last.is_empty())
                        .unwrap_or(address)
                        .to_string()
                });
            let locator = DocumentLocator::new(target.clone(), SurfaceId::markdown());
            Some(OpenDocument {
                tab_id: crate::documents::tab_id_for(&locator),
                locator,
                title,
                presentation: DocumentPresentation {
                    icon: Icon::FileText,
                    accent: generic_okf_accent(),
                    category: NavCategory::OkfDocument,
                },
                view: Box::new(
                    crate::generic_okf_view::GenericOkfView::new_with_asset_host(
                        key,
                        assets.clone(),
                        markdown_extensions(),
                        emphasis,
                    ),
                ),
            })
        }
        RowTarget::Virtual => None,
    }
}

#[cfg(test)]
pub fn open(analysis: &waml::analysis::OkfAnalysis, concept_id: &str) -> Option<OpenDocument> {
    open_with_asset_host(
        analysis,
        concept_id,
        &crate::markdown_hosts::EditorMarkdownAssetHost::shared(
            crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
        ),
        waml_markdown_editor::EditorEmphasis::Code,
    )
}

pub fn open_source_with_asset_host(
    analysis: &waml::analysis::OkfAnalysis,
    concept_id: &str,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
    emphasis: waml_markdown_editor::EditorEmphasis,
) -> Option<OpenDocument> {
    let concept = analysis.bundle.concept(concept_id)?;
    let mut presentation = presentation(analysis, concept_id)?;
    presentation.icon = Icon::FileCode;
    let locator = DocumentLocator::source(concept_id);
    Some(OpenDocument {
        tab_id: crate::documents::tab_id_for(&locator),
        locator,
        title: concept.title.clone().unwrap_or_else(|| {
            concept_id
                .rsplit('/')
                .next()
                .unwrap_or(concept_id)
                .to_string()
        }),
        presentation,
        // The SAME view a concept tab opens, just opened on its source face --
        // so the rendered view stays one toggle away no matter which door you
        // came through.
        view: Box::new(
            crate::generic_okf_view::GenericOkfView::new_source_with_asset_host(
                concept_id.to_string(),
                assets.clone(),
                markdown_extensions(),
                emphasis,
            ),
        ),
    })
}

#[cfg(test)]
pub fn open_source(
    analysis: &waml::analysis::OkfAnalysis,
    concept_id: &str,
) -> Option<OpenDocument> {
    open_source_with_asset_host(
        analysis,
        concept_id,
        &crate::markdown_hosts::EditorMarkdownAssetHost::shared(
            crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
        ),
        waml_markdown_editor::EditorEmphasis::Code,
    )
}

/// The source-document key for a target: a concept is its own key; a
/// folder's key is its index document ("/shop" -> "shop/index", "/" ->
/// "index"); a Virtual target has no source.
pub fn source_key_for(target: &waml::view::row::RowTarget) -> Option<String> {
    use waml::view::row::RowTarget;
    match target {
        RowTarget::Concept(id) => Some(id.clone()),
        RowTarget::Folder(addr) => Some(if addr == "/" {
            "index".to_string()
        } else {
            format!("{}/index", addr.trim_start_matches('/'))
        }),
        RowTarget::Virtual => None,
    }
}

/// The render predicate: does a real markdown document exist at `key`?
/// The SAME resolution SourceView::resolve_document performs
/// (source_view.rs:144-155) minus the snapshot: path derivation ->
/// catalog -> markdown analysis. NOT Bundle::index, which synthesizes
/// (spike Q3 refutation).
pub fn source_document_exists(analysis: &waml::analysis::OkfAnalysis, key: &str) -> bool {
    let Ok(path) = waml::source::BundlePath::parse(format!("{key}.md")) else {
        return false;
    };
    let Some(document) = analysis.catalog.id_for_path(&path) else {
        return false;
    };
    analysis.markdown_snapshot(document).is_some()
}

/// Does this target have a source surface to open? The cheap half of
/// `open_source_for_target` -- same arms, same gates, no document built.
/// Kept beside it so the two cannot drift (pinned by
/// `the_source_probe_agrees_with_open_source_for_target`).
pub fn source_opens_for_target(
    analysis: &waml::analysis::OkfAnalysis,
    target: &waml::view::row::RowTarget,
) -> bool {
    use waml::view::row::RowTarget;
    match target {
        RowTarget::Concept(concept_id) => analysis.bundle.concept(concept_id).is_some(),
        RowTarget::Folder(_) => {
            source_key_for(target).is_some_and(|key| source_document_exists(analysis, &key))
        }
        RowTarget::Virtual => false,
    }
}

/// Target-resolving source open (spec §5). Concept targets delegate to
/// `open_source_with_asset_host` unchanged -- its own concept gate and title
/// stand, so a concept whose markdown snapshot is missing keeps opening its
/// source view exactly as it did before folder targets existed. Only Folder
/// targets consult `source_document_exists`, the render predicate for a
/// folder's index document.
pub fn open_source_for_target(
    analysis: &waml::analysis::OkfAnalysis,
    target: &waml::view::row::RowTarget,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
    emphasis: waml_markdown_editor::EditorEmphasis,
) -> Option<OpenDocument> {
    use waml::view::row::RowTarget;
    match target {
        RowTarget::Concept(concept_id) => {
            open_source_with_asset_host(analysis, concept_id, assets, emphasis)
        }
        RowTarget::Folder(address) => {
            let key = source_key_for(target)?;
            if !source_document_exists(analysis, &key) {
                return None;
            }
            let title = analysis
                .bundle
                .index(address)
                .and_then(|index| index.title.clone())
                .unwrap_or_else(|| {
                    address
                        .rsplit('/')
                        .next()
                        .filter(|last| !last.is_empty())
                        .unwrap_or(address)
                        .to_string()
                });
            let locator = DocumentLocator::new(
                waml::view::row::RowTarget::Folder(address.clone()),
                SurfaceId::source(),
            );
            Some(OpenDocument {
                tab_id: crate::documents::tab_id_for(&locator),
                locator,
                title,
                presentation: DocumentPresentation {
                    icon: Icon::FileCode,
                    accent: generic_okf_accent(),
                    category: NavCategory::OkfDocument,
                },
                view: Box::new(
                    crate::generic_okf_view::GenericOkfView::new_source_with_asset_host(
                        key.clone(),
                        assets.clone(),
                        markdown_extensions(),
                        emphasis,
                    ),
                ),
            })
        }
        RowTarget::Virtual => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::source::SourceBundle;

    fn assets() -> crate::markdown_hosts::SharedMarkdownAssetHost {
        crate::markdown_hosts::EditorMarkdownAssetHost::shared(
            crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
        )
    }

    fn test_emphasis() -> waml_markdown_editor::EditorEmphasis {
        waml_markdown_editor::EditorEmphasis::Code
    }

    #[test]
    fn generic_provider_excludes_reserved_index_and_log_documents() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n"),
            ("log.md", "# Log\n"),
            ("runbook.md", "---\ntype: Runbook\n---\n# Runbook\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        assert_eq!(
            open_with_asset_host(prepared.okf(), "runbook", &assets(), test_emphasis())
                .unwrap()
                .presentation
                .icon,
            Icon::FileText
        );
        assert!(
            open_with_asset_host(prepared.okf(), "index", &assets(), test_emphasis()).is_none()
        );
        assert!(open_with_asset_host(prepared.okf(), "log", &assets(), test_emphasis()).is_none());
    }

    #[test]
    fn tab_id_for_is_stable_and_distinct_across_targets_and_surfaces() {
        use crate::documents::tab_id_for;
        use waml::view::row::RowTarget;

        let markdown = tab_id_for(&DocumentLocator::concept("order", SurfaceId::markdown()));
        let source = tab_id_for(&DocumentLocator::concept("order", SurfaceId::source()));
        let canvas = tab_id_for(&DocumentLocator::concept("order", SurfaceId::canvas()));
        assert_ne!(markdown, source);
        assert_ne!(markdown, canvas);
        assert_ne!(source, canvas);
        // Stable across two calls.
        assert_eq!(
            markdown,
            tab_id_for(&DocumentLocator::concept("order", SurfaceId::markdown()))
        );

        // A folder tab never collides with a concept tab on the same surface.
        let folder_order = tab_id_for(&DocumentLocator::new(
            RowTarget::Folder("/order".into()),
            SurfaceId::folder(),
        ));
        assert_ne!(folder_order, markdown);

        // The folder's own source tab (its index document's key) is a
        // different tab from a direct concept-open of that same index --
        // they have different targets (Folder vs Concept), so the folder's
        // source tab belongs to the folder's own history entry, not to the
        // concept's.
        let folder_shop_source = tab_id_for(&DocumentLocator::new(
            RowTarget::Folder("/shop".into()),
            SurfaceId::source(),
        ));
        let concept_shop_index_source = tab_id_for(&DocumentLocator::source("shop/index"));
        assert_ne!(folder_shop_source, concept_shop_index_source);
    }

    #[test]
    fn source_documents_use_the_source_file_icon() {
        let source =
            SourceBundle::try_from_pairs([("runbook.md", "---\ntype: Runbook\n---\n# Runbook\n")])
                .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();

        let source_document =
            open_source_with_asset_host(prepared.okf(), "runbook", &assets(), test_emphasis())
                .unwrap();

        assert_eq!(source_document.presentation.icon, Icon::FileCode);
    }

    #[test]
    fn folder_source_resolves_through_the_index_key_and_the_root_works() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Shop](shop/)\n"),
            ("shop/index.md", "# Shop\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 31).unwrap();
        use waml::view::row::RowTarget;
        assert_eq!(
            source_key_for(&RowTarget::Folder("/shop".into())).as_deref(),
            Some("shop/index")
        );
        assert_eq!(
            source_key_for(&RowTarget::Folder("/".into())).as_deref(),
            Some("index")
        );
        assert_eq!(
            source_key_for(&RowTarget::Concept("shop/thing".into())).as_deref(),
            Some("shop/thing")
        );
        assert_eq!(source_key_for(&RowTarget::Virtual), None);
        let doc = open_source_for_target(
            prepared.okf(),
            &RowTarget::Folder("/shop".into()),
            &assets(),
            test_emphasis(),
        )
        .expect("a folder with an index.md resolves the source surface");
        assert_eq!(
            doc.tab_id,
            crate::documents::tab_id_for(&DocumentLocator::new(
                RowTarget::Folder("/shop".into()),
                SurfaceId::source(),
            ))
        );
        assert!(open_source_for_target(
            prepared.okf(),
            &RowTarget::Folder("/".into()),
            &assets(),
            test_emphasis(),
        )
        .is_some());
    }

    /// The test the spec's own last Testing bullet demands, and the one that
    /// FAILS if the gate is Bundle::index (spike Q3): /loose has concepts but
    /// no index.md on disk; Bundle::index still answers Some (synthesized),
    /// while the source surface must not resolve.
    #[test]
    fn a_folder_without_an_index_md_does_not_resolve_source_even_though_bundle_index_answers() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Loose](loose/)\n"),
            ("loose/thing.md", "---\ntype: Runbook\n---\n# Thing\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 33).unwrap();
        assert!(
            prepared.okf().bundle.index("/loose").is_some(),
            "the wrong gate would say yes here"
        );
        assert!(!source_document_exists(prepared.okf(), "loose/index"));
        assert!(open_source_for_target(
            prepared.okf(),
            &waml::view::row::RowTarget::Folder("/loose".into()),
            &assets(),
            test_emphasis(),
        )
        .is_none());
    }

    /// The index-document gate is the FOLDER arm's gate only: a Concept
    /// target must delegate to `open_source_with_asset_host` unchanged, so
    /// View Source on a concept keeps opening exactly what it opened before
    /// folder targets existed.
    #[test]
    fn a_concept_source_delegates_unchanged_and_the_probe_agrees_everywhere() {
        use waml::view::row::RowTarget;
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Shop](shop/)\n* [Loose](loose/)\n"),
            ("shop/index.md", "# Shop\n"),
            ("loose/thing.md", "---\ntype: Runbook\n---\n# Thing\n"),
            ("runbook.md", "---\ntype: Runbook\n---\n# Runbook\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 35).unwrap();

        for concept_id in ["runbook", "loose/thing", "no-such-concept"] {
            let target = RowTarget::Concept(concept_id.to_string());
            assert_eq!(
                open_source_for_target(prepared.okf(), &target, &assets(), test_emphasis())
                    .map(|doc| doc.tab_id),
                open_source_with_asset_host(
                    prepared.okf(),
                    concept_id,
                    &assets(),
                    test_emphasis(),
                )
                    .map(|doc| doc.tab_id),
                "concept `{concept_id}` must delegate unchanged"
            );
        }

        for target in [
            RowTarget::Concept("runbook".into()),
            RowTarget::Concept("loose/thing".into()),
            RowTarget::Concept("no-such-concept".into()),
            RowTarget::Folder("/".into()),
            RowTarget::Folder("/shop".into()),
            RowTarget::Folder("/loose".into()),
            RowTarget::Folder("/nowhere".into()),
            RowTarget::Virtual,
        ] {
            assert_eq!(
                source_opens_for_target(prepared.okf(), &target),
                open_source_for_target(prepared.okf(), &target, &assets(), test_emphasis())
                    .is_some(),
                "probe disagrees with the open for {target:?}"
            );
        }
    }
}
