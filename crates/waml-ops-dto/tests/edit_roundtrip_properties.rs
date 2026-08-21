//! The apply -> write -> reparse round-trip property for the semantic edit-op
//! layer.
//!
//! Everything else that covers `waml::edit` is example-based: a hand-written
//! bundle, one hand-written op, one hand-written expectation. That catches the
//! cases somebody thought of. It cannot catch the case where a *combination* of
//! ops writes text that the parser then reads back differently from what the
//! writer believed it had written — and that is the failure mode that matters,
//! because the CLI's `run_batch` writes the result to disk and every later
//! session reads it cold.
//!
//! So this file drives the real production pipeline, in production order:
//!
//! ```text
//! pairs -> SourceBundle::try_from_pairs
//!       -> prepare_candidate(.., None, 0)       // cold read, as `waml <op>` does
//!       -> Batch::lower(EditContext { .. })     // the edit-op layer under test
//!       -> prepare_candidate(.., Some(prev), 1) // the writer's own view
//!       -> to_pairs()                           // what lands on disk
//! ```
//!
//! and then re-reads what landed, cold, exactly as the next process would.
//!
//! # What is asserted, and why that is the right invariant
//!
//! 1. **Never panic, never half-write.** A rejected op is a fine outcome — most
//!    generated ops name something that does not exist. What is not fine is a
//!    panic, or a batch whose third step fails after its first two already
//!    mutated the bundle. `apply` is all-or-nothing by construction (it takes
//!    `&SourceBundle`), and this pins that.
//! 2. **The written text is a fixed point of write/read.** `to_pairs` ->
//!    `try_from_pairs` -> `to_pairs` must be the identity. If it is not, the
//!    bundle on disk is not the bundle the editor thinks it has.
//! 3. **The writer's analysis agrees with a cold reparse.** `run_batch` reuses
//!    the pre-edit analyses incrementally to validate what it just wrote; the
//!    next process reads the same bytes with no prior state at all. Those two
//!    must produce the same diagnostics. This is the invariant that actually
//!    protects users: a disagreement means the editor shows a clean document
//!    that reopens dirty (or vice versa).
//! 4. **No op may make a document unreadable.** Semantic diagnostics may
//!    legitimately appear — removing a node leaves references dangling, and
//!    that *should* be reported. A `document-quarantined` diagnostic may not:
//!    that means the edit layer emitted text its own parser refuses.
//! 5. **Re-application converges.** One op applied to its own output must fail
//!    cleanly or change nothing; a whole batch must reach a fixed point. The
//!    per-family reasoning, and why batches get the weaker claim, is written
//!    out at `_IDEMPOTENCE_REASONING`.
//!
//! The seeds under `proptest-regressions/` replay every failure this has ever
//! found.

use proptest::prelude::*;
use std::collections::BTreeSet;
use waml::analysis::{prepare_candidate, PreviousAnalyses};
use waml::edit::{EditBatch, EditContext};
use waml::layout::Direction;
use waml::source::SourceBundle;
use waml::uml::FieldEdit;
use waml_ops_dto::{to_batch, OpDto, TraceEditDto};

type Pairs = Vec<(String, String)>;

// ---------------------------------------------------------------------------
// The seed bundle
// ---------------------------------------------------------------------------

/// A small but structurally complete bundle: an indexed directory, classes with
/// attributes and both ended and unended relationships, an enum with values, a
/// datatype, a class diagram with members, and a state machine with a trace.
/// Every op family in `OpDto` has something real to bite on here.
///
/// `shop/index.md` deliberately carries a **stale** member entry — `Archived`,
/// whose document now sits at the bundle root. That is not a typo and not a
/// broken fixture: it is the exact residue `pkg.move` leaves behind, since
/// `pkg.move` does not maintain either directory's index (see
/// `okf::Op::ConceptDelete`, which documents the same deliberate choice).
/// Reconciling that residue is the whole job of `pkg.reorder`/`pkg.sort`, so a
/// seed without one cannot exercise them honestly — and in fact a clean seed
/// hid a real defect behind a two-op batch until this entry was added.
fn seed() -> Pairs {
    [
        (
            "shop/index.md",
            "# Shop\n\n* [Order](./order.md)\n* [OrderLine](./order-line.md)\n* [Customer](./customer.md)\n* [OrderStatus](./order-status.md)\n* [Money](./money.md)\n* [Archived](./archived.md)\n",
        ),
        (
            "archived.md",
            "---\ntype: uml.Class\ntitle: Archived\n---\n# Archived\n",
        ),
        (
            "shop/order.md",
            "---\ntype: uml.Class\nstereotype: [aggregateRoot, entity]\ntitle: Order\ndescription: \"A customer's placed order.\"\n---\n# Order\n\n## Attributes\n- id: OrderId {1}\n- status: [OrderStatus](./order-status.md) {1}\n- total: [Money](./money.md) {1}\n\n## Relationships\n- associates [Customer](./customer.md): 1 order to 1 customer\n- composes [OrderLine](./order-line.md): 1 to 1..* lines\n",
        ),
        (
            "shop/order-line.md",
            "---\ntype: uml.Class\nstereotype: entity\ntitle: OrderLine\n---\n# OrderLine\n\n## Attributes\n- quantity: Int {1}\n- unitPrice: [Money](./money.md) {1}\n",
        ),
        (
            "shop/customer.md",
            "---\ntype: uml.Class\nstereotype: [aggregateRoot, entity]\ntitle: Customer\n---\n# Customer\n\n## Attributes\n- id: CustomerId {1}\n- name: String {1}\n",
        ),
        (
            "shop/order-status.md",
            "---\ntype: uml.Enum\ntitle: OrderStatus\n---\n# OrderStatus\n\n## Values\n- DRAFT\n- PLACED\n- SHIPPED\n",
        ),
        (
            "shop/money.md",
            "---\ntype: uml.DataType\nstereotype: valueObject\ntitle: Money\n---\n# Money\n\n## Attributes\n- amount: Decimal {1}\n- currency: CurrencyCode {1}\n",
        ),
        (
            "shop/orders-domain.md",
            "---\ntype: uml.ClassDiagram\ntitle: Orders Domain\n---\n# Orders Domain\n\n## Members\n- [Order](./order.md)\n- [OrderLine](./order-line.md)\n- [Customer](./customer.md)\n- [OrderStatus](./order-status.md)\n- [Money](./money.md)\n",
        ),
        (
            "shop/checkout-flow.md",
            "---\ntype: uml.StateMachineDiagram\ntitle: Checkout Flow\n---\n# Checkout Flow\n\n## Nodes\n### Idle\n- transitions to Paid\n  traces [Order](./order.md)\n### final Paid\n",
        ),
        ("notes.md", "# Notes\n\nPlain markdown the projection does not claim.\n"),
    ]
    .into_iter()
    .map(|(path, text)| (path.to_owned(), text.to_owned()))
    .collect()
}

// ---------------------------------------------------------------------------
// The production pipeline, exactly as `waml-cli`'s `run_batch` runs it
// ---------------------------------------------------------------------------

struct Applied {
    /// What would land on disk.
    written: Pairs,
    /// The diagnostics the writer itself reported, from the incremental
    /// validation pass it runs against the pre-edit analyses.
    writer_diagnostics: Vec<String>,
}

fn fingerprint(diagnostics: &[waml::diagnostic::Diagnostic]) -> Vec<String> {
    let mut out: Vec<String> = diagnostics
        .iter()
        .map(|diagnostic| {
            format!(
                "{:?}|{}|{}|{:?}",
                diagnostic.code, diagnostic.file, diagnostic.line, diagnostic.message
            )
        })
        .collect();
    out.sort();
    out
}

fn apply_write(pairs: &Pairs, ops: &[OpDto]) -> Result<Applied, String> {
    let batch = to_batch(ops)?;
    let source =
        SourceBundle::try_from_pairs(pairs.iter().cloned()).map_err(|error| error.to_string())?;
    let prepared = prepare_candidate(source, None, 0).map_err(|error| error.to_string())?;
    let changed = batch
        .lower(EditContext {
            source: prepared.source(),
            okf_analysis: prepared.okf(),
            session_revision: prepared.revision(),
            uml: prepared.uml(),
        })
        .map_err(|error| error.to_string())?;
    let validated = prepare_candidate(
        changed,
        Some(PreviousAnalyses {
            okf: prepared.okf(),
            uml: prepared.uml(),
        }),
        1,
    )
    .map_err(|error| error.to_string())?;
    Ok(Applied {
        written: validated.source().to_pairs(),
        writer_diagnostics: fingerprint(&validated.diagnostics()),
    })
}

/// A cold read of text on disk — what the next process sees.
fn reparse(pairs: &Pairs) -> Result<(Pairs, Vec<String>), String> {
    let source =
        SourceBundle::try_from_pairs(pairs.iter().cloned()).map_err(|error| error.to_string())?;
    let prepared = prepare_candidate(source, None, 0).map_err(|error| error.to_string())?;
    Ok((
        prepared.source().to_pairs(),
        fingerprint(&prepared.diagnostics()),
    ))
}

fn quarantined(diagnostics: &[String]) -> BTreeSet<String> {
    diagnostics
        .iter()
        .filter(|line| line.starts_with("DocumentQuarantined|"))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Idempotence classification
// ---------------------------------------------------------------------------

/// # Idempotence, per op family
///
/// Every op names its target by identity, never by position, so applying one
/// op to its own output is always well defined. The families:
///
/// * **Set-shaped** (`node.set`, `attr.set`, `rel.set`, `concept.set`,
///   `diagram.set`, `place.set`, `pkg.retitle`, `pkg.reorder`, `pkg.sort`) —
///   assign a value to a named slot. Assigning the same value twice must be a
///   no-op.
/// * **Create-shaped** (`node.new`, `concept.new`, `pkg.insert`) and
///   **remove-shaped** (`node.rm`, `attr.rm`, `value.rm`, `rel.rm`,
///   `place.rm`, `concept.delete`, `pkg.delete`, `node.rename`, `pkg.rename`,
///   `pkg.move`) — the second application no longer finds the world it needs:
///   the slug is taken, the target is gone, the source directory has moved.
///   It must therefore *fail cleanly*, which the property accepts. Where it
///   does succeed (moving a document to the directory it is already in) it
///   must not have changed anything.
/// * **Append-shaped** (`attr.add`, `value.add`, `rel.add`) — the only family
///   where a second success could plausibly mean "now there are two". It must
///   not: a classifier cannot hold two attributes of the same name, an enum
///   cannot hold the same literal twice, and the lowering layer treats an
///   identical `kind`+`target`+`name` triple as the same relationship. A
///   duplicating second application is a defect, not a design.
///
/// * **Positional** (`transition.trace.edit`) — the one genuine exception, and
///   the reason [`arb_positional_op`] is its own strategy. Its payload
///   addresses a trace by
///   *index into a list*, not by identity: `Insert { index }` puts a trace at a
///   position, `Remove { index }` takes whichever trace is there now, and
///   `Move { from, to }` is positional on both ends. Re-applying `Insert`
///   legitimately yields two traces, and re-applying `Remove` legitimately
///   takes a second, different one. That is what a list-editing wire op means,
///   and it is what the trace editor sends. Demanding idempotence here would
///   be demanding that the op be a different op. Excluded, deliberately.
///
/// Every other family is held to idempotence with no exemption. If a future op
/// family genuinely accumulates, this doc comment is where the carve-out and
/// its argument belong.
///
/// # Why a batch gets no claim at all once it relocates a document
///
/// A batch is a sequence, and a batch whose later step consumes a name its
/// earlier step created is a *translation* of the tree, not an assignment to
/// it. Re-running such a batch feeds it a different world, so it is under no
/// obligation to settle — and it should not.
///
/// The witness, minimised out of a 20 000-case run:
///
/// ```text
/// [ pkg.rename shop    -> archive,
///   pkg.rename archive -> shop/legacy ]
/// ```
///
/// Pass one leaves the tree at `shop/legacy/*`. Pass two renames that `shop`
/// (the one pass one just created) to `archive`, then puts `archive` back at
/// `shop/legacy` — reaching `shop/legacy/legacy/*`. Every pass nests one level
/// deeper, forever. That is exactly what the equivalent shell pair
/// `mv shop archive && mv archive shop/legacy` does, and it is correct. There
/// is no defect here and no fixed point to demand.
///
/// So [`reapplying_a_settled_batch_reaches_a_fixed_point`] excludes any batch
/// that adds, removes, or relocates a document — the [`arb_relocating_op`]
/// family. What is left ([`arb_settled_op`]: in-document edits and index
/// regeneration) genuinely must settle, and unbounded growth there still fails
/// the property.
///
/// The sharp property is the single-op one above. It does not need this
/// carve-out, because one op cannot chain through its own output.
///
/// The three classes are the three op strategies, so each property draws
/// exactly the ops it is entitled to assume things about. Keeping the taxonomy
/// in the generators rather than in `prop_assume!` filters is deliberate — see
/// [`arb_settled_op`].
const _IDEMPOTENCE_REASONING: () = ();

// ---------------------------------------------------------------------------
// "The op's effect is present" — read back out of the reparsed text
// ---------------------------------------------------------------------------

/// The document a slug names, looked up the way the op layer resolves it:
/// a fully qualified `dir/slug` first, then a unique basename match.
fn resolve<'a>(pairs: &'a Pairs, slug: &str) -> Option<&'a (String, String)> {
    let qualified = format!("{slug}.md");
    if let Some(found) = pairs.iter().find(|(path, _)| *path == qualified) {
        return Some(found);
    }
    let suffix = format!("/{slug}.md");
    let mut matches = pairs.iter().filter(|(path, _)| path.ends_with(&suffix));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

/// The names declared by `- name: Type` member lines, with any visibility
/// marker stripped. Line-shaped on purpose: a bare `contains` would be
/// satisfied by an attribute's *type* mentioning the name.
fn member_names(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("- "))
        .filter_map(|rest| rest.split_once(':'))
        .map(|(name, _)| {
            name.trim()
                .trim_start_matches(['+', '-', '#', '~'])
                .trim()
                .to_owned()
        })
        .collect()
}

/// Bare `- LITERAL` lines: an enum's values.
fn literal_lines(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("- "))
        .filter(|rest| !rest.contains(':') && !rest.contains('['))
        .map(|rest| rest.trim().to_owned())
        .collect()
}

fn join(directory: &str, slug: &str) -> String {
    if directory.is_empty() {
        format!("{slug}.md")
    } else {
        format!("{directory}/{slug}.md")
    }
}

/// What a successful op must be able to show for itself in the reparsed text.
///
/// Only op families with an unambiguous textual witness are checked; the rest
/// return `Ok` and are covered by the round-trip and idempotence properties.
/// "It succeeded" is not evidence that anything happened — this is.
fn effect_is_visible(before: &Pairs, after: &Pairs, op: &OpDto) -> Result<(), String> {
    let has = |path: &str| after.iter().any(|(candidate, _)| candidate == path);
    let missing = |what: String| Err(what);
    match op {
        OpDto::NodeNew { slug, dir, .. } | OpDto::ConceptNew { slug, dir, .. } => {
            let path = join(dir.trim_matches('/'), slug);
            has(&path)
                .then_some(())
                .ok_or_else(|| format!("created document '{path}' is not in the result"))
        }
        OpDto::NodeRename { from, to, .. } => {
            // A rename keeps the directory and swaps the basename, so the
            // expected destination is derived from where the document actually
            // was. `from == to` is a legitimate no-op rename, not a leak.
            let Some((old, _)) = resolve(before, from) else {
                return Ok(());
            };
            let new = match old.rsplit_once('/') {
                Some((directory, _)) => format!("{directory}/{to}.md"),
                None => format!("{to}.md"),
            };
            if !has(&new) {
                return missing(format!("renamed document '{new}' is not in the result"));
            }
            if new != *old && has(old) {
                return missing(format!("rename left the old document '{old}' behind"));
            }
            Ok(())
        }
        OpDto::NodeRm { slug, .. } | OpDto::ConceptDelete { id: slug, .. } => {
            match resolve(before, slug) {
                Some((path, _)) if has(path) => {
                    missing(format!("removed document '{path}' is still in the result"))
                }
                _ => Ok(()),
            }
        }
        OpDto::AttrAdd { node, name, .. } => match resolve(after, node) {
            Some((_, text)) if member_names(text).iter().any(|declared| declared == name) => Ok(()),
            Some((path, _)) => {
                missing(format!("'{path}' has no attribute '{name}' after attr.add"))
            }
            None => Ok(()),
        },
        OpDto::AttrRm { node, name, .. } => match resolve(after, node) {
            Some((path, text)) if member_names(text).iter().any(|declared| declared == name) => {
                missing(format!("'{path}' still declares '{name}' after attr.rm"))
            }
            _ => Ok(()),
        },
        OpDto::ValueAdd { node, literal, .. } => match resolve(after, node) {
            Some((_, text)) if literal_lines(text).iter().any(|value| value == literal) => Ok(()),
            Some((path, _)) => {
                missing(format!("'{path}' has no value '{literal}' after value.add"))
            }
            None => Ok(()),
        },
        OpDto::ValueRm { node, literal, .. } => match resolve(after, node) {
            Some((path, text)) if literal_lines(text).iter().any(|value| value == literal) => {
                missing(format!("'{path}' still lists '{literal}' after value.rm"))
            }
            _ => Ok(()),
        },
        OpDto::PkgMove { slug, to_dir, .. } => {
            let stem = slug.rsplit('/').next().unwrap_or(slug);
            let path = join(to_dir.trim_matches('/'), stem);
            has(&path)
                .then_some(())
                .ok_or_else(|| format!("moved document '{path}' is not in the result"))
        }
        OpDto::NodeSet {
            slug,
            title: Some(title),
            ..
        }
        | OpDto::ConceptSet {
            id: slug,
            title: Some(title),
            ..
        } => match resolve(after, slug) {
            Some((_, text)) if text.contains(title.as_str()) => Ok(()),
            Some((path, _)) => missing(format!("'{path}' does not carry the new title '{title}'")),
            None => Ok(()),
        },
        _ => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

/// Slugs the seed defines, in both the qualified and the bare form the
/// resolver accepts, plus names it does not define so rejection paths are
/// exercised too.
const SLUGS: &[&str] = &[
    "shop/order",
    "shop/order-line",
    "shop/customer",
    "shop/order-status",
    "shop/money",
    "shop/orders-domain",
    "shop/checkout-flow",
    "order",
    "customer",
    "money",
    "notes",
    "shop/ghost",
    "ghost",
];

const NEW_SLUGS: &[&str] = &["invoice", "shipment", "order", "index"];

const ATTR_NAMES: &[&str] = &[
    "id", "status", "total", "quantity", "name", "alpha", "ghost",
];

const TYPE_TOKENS: &[&str] = &["String", "Int", "OrderId", "[Money](./money.md)"];

const MULTS: &[&str] = &["1", "0..1", "1..*", "*", "2..5"];

const VIS: &[&str] = &["+", "-", "#", "~"];

const KINDS: &[&str] = &[
    "associates",
    "composes",
    "aggregates",
    "specializes",
    "implements",
    "depends",
    "links",
];

const ENDS: &[&str] = &["1 to 1", "1 order to 1 customer", "0..1 to 1..* lines"];

const LITERALS: &[&str] = &["DRAFT", "PLACED", "REFUNDED"];

const DIRS: &[&str] = &["", "shop", "archive", "shop/legacy"];

const ELEMENT_TYPES: &[&str] = &[
    "uml.Class",
    "uml.Enum",
    "uml.DataType",
    "uml.Interface",
    "uml.ClassDiagram",
];

const TITLES: &[&str] = &["Invoice", "Renamed", "A B"];

const STEREOTYPES: &[&str] = &["entity", "valueObject", "aggregateRoot"];

const DESCRIPTIONS: &[&str] = &["Short.", "Has: a colon", "Quote \" and # hash"];

const REL_NAMES: &[&str] = &["owns", "placedBy"];

fn pick(values: &'static [&'static str]) -> impl Strategy<Value = String> {
    prop::sample::select(values).prop_map(str::to_owned)
}

fn maybe(values: &'static [&'static str]) -> impl Strategy<Value = Option<String>> {
    prop::option::of(pick(values))
}

/// Ops that edit documents in place or regenerate an index — they never add,
/// remove, or relocate a document. See `_IDEMPOTENCE_REASONING`.
///
/// This split is not cosmetic. `reapplying_a_settled_batch_reaches_a_fixed_point`
/// needs settled ops only, and filtering them out of a general strategy with
/// `prop_assume!` rejects ~86% of cases — enough to blow proptest's reject
/// ceiling and abort the run at high case counts instead of testing anything.
/// Generating exactly what each property needs rejects nothing.
fn arb_settled_op() -> impl Strategy<Value = OpDto> {
    prop_oneof![
        (
            pick(SLUGS),
            maybe(TITLES),
            prop::option::of(any::<bool>()),
            maybe(ELEMENT_TYPES)
        )
            .prop_map(|(slug, title, abstract_, ty)| OpDto::NodeSet {
                v: 1,
                slug,
                title,
                desc: None,
                stereotype: None,
                abstract_,
                ty,
            }),
        (
            pick(SLUGS),
            pick(ATTR_NAMES),
            pick(TYPE_TOKENS),
            maybe(MULTS),
            maybe(VIS)
        )
            .prop_map(|(node, name, ty, mult, vis)| OpDto::AttrAdd {
                v: 1,
                node,
                name,
                ty,
                mult,
                vis,
            }),
        (
            pick(SLUGS),
            pick(ATTR_NAMES),
            maybe(TYPE_TOKENS),
            arb_mult_edit(),
            maybe(VIS),
            prop::option::of(pick(ATTR_NAMES))
        )
            .prop_map(|(node, name, ty, mult, vis, rename)| OpDto::AttrSet {
                v: 1,
                node,
                name,
                ty,
                mult,
                vis,
                rename,
            }),
        (pick(SLUGS), pick(ATTR_NAMES)).prop_map(|(node, name)| OpDto::AttrRm { v: 1, node, name }),
        (pick(SLUGS), pick(LITERALS)).prop_map(|(node, literal)| OpDto::ValueAdd {
            v: 1,
            node,
            literal
        }),
        (pick(SLUGS), pick(LITERALS)).prop_map(|(node, literal)| OpDto::ValueRm {
            v: 1,
            node,
            literal
        }),
        (
            pick(SLUGS),
            pick(KINDS),
            pick(SLUGS),
            maybe(ENDS),
            maybe(REL_NAMES),
            prop::option::of(pick(SLUGS))
        )
            .prop_map(
                |(source, kind, target, ends, as_label, as_ref)| OpDto::RelAdd {
                    v: 1,
                    source,
                    kind,
                    target,
                    as_label,
                    as_ref,
                    ends,
                }
            ),
        (pick(SLUGS), pick(KINDS), pick(SLUGS), maybe(ENDS)).prop_map(
            |(source, kind, target, ends)| OpDto::RelSet {
                v: 1,
                source,
                kind: Some(kind),
                target: Some(target),
                as_sel: None,
                ends,
                set_label: None,
                set_as_ref: None,
            }
        ),
        (pick(SLUGS), pick(KINDS), pick(SLUGS)).prop_map(|(source, kind, target)| OpDto::RelRm {
            v: 1,
            source,
            kind: Some(kind),
            target: Some(target),
            as_sel: None,
        }),
        (pick(DIRS), prop::collection::vec(pick(NEW_SLUGS), 0..3))
            .prop_map(|(path, order)| OpDto::PkgReorder { v: 1, path, order }),
        pick(DIRS).prop_map(|path| OpDto::PkgSort { v: 1, path }),
        (pick(DIRS), pick(TITLES)).prop_map(|(path, title)| OpDto::PkgRetitle {
            v: 1,
            path,
            title
        }),
        (
            pick(SLUGS),
            pick(TITLES),
            pick(SLUGS),
            pick(TITLES),
            pick(SLUGS),
            prop::collection::vec(arb_direction(), 0..3)
        )
            .prop_map(
                |(
                    diagram,
                    subject_title,
                    subject_slug,
                    reference_title,
                    reference_slug,
                    directions,
                )| {
                    OpDto::PlaceSet {
                        v: 1,
                        diagram,
                        subject_title,
                        subject_slug,
                        reference_title,
                        reference_slug,
                        directions,
                    }
                }
            ),
        (pick(SLUGS), pick(SLUGS), pick(SLUGS)).prop_map(
            |(diagram, subject_slug, reference_slug)| OpDto::PlaceRm {
                v: 1,
                diagram,
                subject_slug,
                reference_slug,
            }
        ),
        (pick(SLUGS), maybe(TITLES)).prop_map(|(id, title)| OpDto::ConceptSet {
            v: 1,
            id,
            title,
            desc: None
        }),
        (pick(SLUGS), maybe(TITLES), any::<bool>()).prop_map(|(key, title, clear_desc)| {
            OpDto::DiagramSet {
                v: 1,
                key,
                title,
                desc: None,
                clear_desc,
                display: None,
            }
        }),
    ]
}

/// Ops that add, remove, or relocate a document, and so can chain through a
/// name an earlier step in the same batch created. See `_IDEMPOTENCE_REASONING`.
fn arb_relocating_op() -> impl Strategy<Value = OpDto> {
    prop_oneof![
        (
            pick(NEW_SLUGS),
            pick(DIRS),
            pick(ELEMENT_TYPES),
            pick(TITLES),
            any::<bool>(),
            prop::collection::vec(pick(STEREOTYPES), 0..3),
            maybe(DESCRIPTIONS)
        )
            .prop_map(|(slug, dir, ty, title, abstract_, stereotype, desc)| {
                OpDto::NodeNew {
                    v: 1,
                    slug,
                    dir,
                    ty,
                    title,
                    stereotype,
                    desc,
                    abstract_,
                }
            }),
        (pick(SLUGS), pick(NEW_SLUGS)).prop_map(|(from, to)| OpDto::NodeRename { v: 1, from, to }),
        (pick(SLUGS), any::<bool>()).prop_map(|(slug, cascade)| OpDto::NodeRm {
            v: 1,
            slug,
            cascade
        }),
        (pick(SLUGS), pick(DIRS)).prop_map(|(slug, to_dir)| OpDto::PkgMove { v: 1, slug, to_dir }),
        (pick(DIRS), pick(DIRS)).prop_map(|(from, to)| OpDto::PkgRename { v: 1, from, to }),
        (pick(DIRS), any::<bool>()).prop_map(|(path, cascade)| OpDto::PkgDelete {
            v: 1,
            path,
            cascade
        }),
        (
            pick(DIRS),
            pick(NEW_SLUGS),
            prop::collection::vec(arb_imported_doc(), 0..3)
        )
            .prop_map(|(parent_path, name, docs)| OpDto::PkgInsert {
                v: 1,
                parent_path,
                name,
                docs,
            }),
        (pick(DIRS), pick(NEW_SLUGS), pick(TITLES)).prop_map(|(dir, slug, title)| {
            OpDto::ConceptNew {
                v: 1,
                dir,
                slug,
                ty: String::new(),
                title,
                desc: None,
            }
        }),
        pick(SLUGS).prop_map(|id| OpDto::ConceptDelete { v: 1, id }),
    ]
}

/// The one op family addressed by list index rather than identity, and so the
/// one exempt from idempotence. See `_IDEMPOTENCE_REASONING`.
fn arb_positional_op() -> impl Strategy<Value = OpDto> {
    prop_oneof![
        (pick(SLUGS), arb_trace_edit()).prop_map(|(behavior, edit)| OpDto::TransitionTraceEdit {
            v: 1,
            behavior,
            source_node: "Idle".to_owned(),
            occurrence: 0,
            edit,
        }),
    ]
}

/// Every op family, for the properties that hold universally.
fn arb_op() -> impl Strategy<Value = OpDto> {
    prop_oneof![arb_idempotent_op(), arb_positional_op()]
}

/// Every op family that is required to be idempotent: everything but the
/// positional trace edits.
fn arb_idempotent_op() -> impl Strategy<Value = OpDto> {
    prop_oneof![arb_settled_op(), arb_relocating_op()]
}

fn arb_mult_edit() -> impl Strategy<Value = FieldEdit<String>> {
    prop_oneof![
        Just(FieldEdit::Unchanged),
        Just(FieldEdit::Clear),
        pick(MULTS).prop_map(FieldEdit::Set),
    ]
}

/// One document in a `pkg.insert` payload. The importer re-roots these under
/// the target package, so the baked-in folder segment is deliberately present.
fn arb_imported_doc() -> impl Strategy<Value = (String, String)> {
    (
        prop::sample::select(
            [
                "template/one.md",
                "template/index.md",
                "template/sub/two.md",
            ]
            .as_slice(),
        ),
        prop::sample::select(
            [
                "---\ntype: uml.Class\ntitle: One\n---\n# One\n",
                "# Template\n\nIntro.\n",
                "",
            ]
            .as_slice(),
        ),
    )
        .prop_map(|(path, text)| (path.to_owned(), text.to_owned()))
}

fn arb_direction() -> impl Strategy<Value = Direction> {
    prop::sample::select(
        [
            Direction::LeftOf,
            Direction::RightOf,
            Direction::Above,
            Direction::Below,
        ]
        .as_slice(),
    )
}

fn arb_trace_edit() -> impl Strategy<Value = TraceEditDto> {
    prop_oneof![
        (0usize..3).prop_map(|index| TraceEditDto::Insert {
            index,
            label: "Money".to_owned(),
            href: "./money.md".to_owned(),
        }),
        (0usize..3).prop_map(|index| TraceEditDto::Update {
            index,
            label: "Customer".to_owned(),
            href: "./customer.md".to_owned(),
        }),
        (0usize..3).prop_map(|index| TraceEditDto::Remove { index }),
        (0usize..3, 0usize..3).prop_map(|(from, to)| TraceEditDto::Move { from, to }),
    ]
}

/// Batches of settled ops only — see `arb_settled_op` for why this is a
/// separate strategy rather than a filter.
fn arb_settled_ops() -> impl Strategy<Value = Vec<OpDto>> {
    prop::collection::vec(arb_settled_op(), 1..4)
}

fn arb_ops() -> impl Strategy<Value = Vec<OpDto>> {
    prop::collection::vec(arb_op(), 1..4)
}

// ---------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------

proptest! {
    // Five properties, each running the whole analyse -> lower -> re-analyse
    // pipeline two or three times over a nine-document bundle. 128 keeps the
    // file at a few seconds in `cargo test --workspace`, which is where it has
    // to stay. Raise it with `PROPTEST_CASES` when hunting, not here.
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Apply -> write -> reparse. A rejected batch is a valid outcome; a batch
    /// that succeeds must have written text that reads back identically and
    /// that its own validation pass and a cold reparse agree about.
    #[test]
    fn apply_write_reparse_agrees_with_the_writer(ops in arb_ops()) {
        let before = seed();
        let baseline_quarantine = {
            let (_, diagnostics) = reparse(&before).expect("seed bundle parses");
            quarantined(&diagnostics)
        };

        let Ok(applied) = apply_write(&before, &ops) else {
            // A clean rejection. Nothing was written, so there is nothing to
            // round-trip; the absence of a panic is the assertion.
            return Ok(());
        };

        let (reread, cold_diagnostics) = reparse(&applied.written)
            .map_err(|error| TestCaseError::fail(format!("written bundle does not reparse: {error}")))?;

        prop_assert_eq!(
            &reread,
            &applied.written,
            "write -> read -> write is not the identity"
        );

        prop_assert_eq!(
            quarantined(&cold_diagnostics),
            baseline_quarantine,
            "the edit layer wrote a document its own parser quarantines"
        );

        prop_assert_eq!(
            &cold_diagnostics,
            &applied.writer_diagnostics,
            "the writer's incremental validation disagrees with a cold reparse"
        );
    }

    /// A batch that fails publishes nothing — not even the steps that ran
    /// before the failing one.
    #[test]
    fn a_rejected_batch_writes_nothing(ops in arb_ops()) {
        let before = seed();
        if apply_write(&before, &ops).is_ok() {
            return Ok(());
        }
        // `apply` borrows the bundle, so a partial write could only escape by
        // way of a mutated input. Pin that it cannot.
        prop_assert_eq!(before, seed());
    }

    /// A single op that reports success must be able to point at its own
    /// effect in the text that came back off the round-trip.
    #[test]
    fn a_successful_op_is_visible_after_reparse(op in arb_op()) {
        let before = seed();
        let Ok(applied) = apply_write(&before, std::slice::from_ref(&op)) else {
            return Ok(());
        };
        let (reread, _) = reparse(&applied.written)
            .map_err(|error| TestCaseError::fail(format!("written bundle does not reparse: {error}")))?;
        if let Err(reason) = effect_is_visible(&before, &reread, &op) {
            return Err(TestCaseError::fail(reason));
        }
    }

    /// One op applied to its own output either fails cleanly or changes
    /// nothing. See `_IDEMPOTENCE_REASONING` for the per-family argument.
    #[test]
    fn reapplying_one_op_is_idempotent(op in arb_idempotent_op()) {
        let ops = [op];
        let Ok(once) = apply_write(&seed(), &ops) else {
            return Ok(());
        };
        let Ok(twice) = apply_write(&once.written, &ops) else {
            return Ok(());
        };
        prop_assert_eq!(
            &twice.written,
            &once.written,
            "re-applying one op changed its own output"
        );
    }

    /// A batch that only edits documents in place — no creation, deletion, or
    /// relocation — must settle. The second pass may still differ from the
    /// first (an index op may be reconciling residue an earlier step left), but
    /// the third may not. See `_IDEMPOTENCE_REASONING` for why a batch that
    /// *does* relocate documents is excluded rather than exempted.
    #[test]
    fn reapplying_a_settled_batch_reaches_a_fixed_point(ops in arb_settled_ops()) {
        let Ok(once) = apply_write(&seed(), &ops) else {
            return Ok(());
        };
        let Ok(twice) = apply_write(&once.written, &ops) else {
            return Ok(());
        };
        let Ok(thrice) = apply_write(&twice.written, &ops) else {
            return Ok(());
        };
        prop_assert_eq!(
            &thrice.written,
            &twice.written,
            "the batch never settles: it kept changing its own output"
        );
    }
}
