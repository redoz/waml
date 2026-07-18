use crate::multiplicity::Multiplicity;

/// `skip_serializing_if` helper: TS optionals omit a `false` flag rather than emit it.
#[cfg(feature = "serde")]
fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(into = "String", try_from = "String"))]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Package,
}

#[cfg(feature = "serde")]
impl From<Visibility> for String {
    fn from(v: Visibility) -> String {
        v.marker().to_string()
    }
}

#[cfg(feature = "serde")]
impl TryFrom<String> for Visibility {
    type Error = String;
    fn try_from(s: String) -> Result<Visibility, String> {
        s.chars()
            .next()
            .and_then(Visibility::from_marker)
            .ok_or_else(|| format!("invalid visibility '{s}'"))
    }
}

impl Visibility {
    pub fn from_marker(c: char) -> Option<Visibility> {
        match c {
            '+' => Some(Visibility::Public),
            '-' => Some(Visibility::Private),
            '#' => Some(Visibility::Protected),
            '~' => Some(Visibility::Package),
            _ => None,
        }
    }
    pub fn marker(self) -> char {
        match self {
            Visibility::Public => '+',
            Visibility::Private => '-',
            Visibility::Protected => '#',
            Visibility::Package => '~',
        }
    }
}

/// An attribute's type: a display token, optionally resolved to another classifier's slug.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TypeRef {
    pub name: String,
    #[cfg_attr(
        feature = "serde",
        serde(rename = "ref", default, skip_serializing_if = "Option::is_none")
    )]
    pub ref_: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Attribute {
    pub name: String,
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub ty: TypeRef,
    #[cfg_attr(feature = "wasm", tsify(type = "string"))]
    pub multiplicity: Multiplicity,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    #[cfg_attr(feature = "wasm", tsify(type = "\"+\" | \"-\" | \"#\" | \"~\""))]
    pub visibility: Option<Visibility>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub description: Option<String>,
}

/// A slot value on an `InstanceSpecification` (design spec §3.2): a named value
/// that stands in for a classifier attribute, rather than declaring one. Mirrors
/// `Attribute` for serde/tsify.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Slot {
    pub name: String,
    pub value: String,
    /// Set when the slot value resolves to another pool element (an
    /// instance-valued slot); a display token otherwise.
    #[cfg_attr(
        feature = "serde",
        serde(rename = "ref", default, skip_serializing_if = "Option::is_none")
    )]
    pub ref_: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum RelationshipKind {
    Associates,
    Aggregates,
    Composes,
    Specializes,
    Implements,
    Depends,
    Annotates,
    Includes,
    Extends,
    InstanceOf,
    Links,
}

impl RelationshipKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RelationshipKind::Associates => "associates",
            RelationshipKind::Aggregates => "aggregates",
            RelationshipKind::Composes => "composes",
            RelationshipKind::Specializes => "specializes",
            RelationshipKind::Implements => "implements",
            RelationshipKind::Depends => "depends",
            RelationshipKind::Annotates => "annotates",
            RelationshipKind::Includes => "includes",
            RelationshipKind::Extends => "extends",
            RelationshipKind::InstanceOf => "instance of",
            RelationshipKind::Links => "links",
        }
    }
    pub fn parse(s: &str) -> Option<RelationshipKind> {
        match s {
            "associates" => Some(RelationshipKind::Associates),
            "aggregates" => Some(RelationshipKind::Aggregates),
            "composes" => Some(RelationshipKind::Composes),
            "specializes" => Some(RelationshipKind::Specializes),
            "implements" => Some(RelationshipKind::Implements),
            "depends" => Some(RelationshipKind::Depends),
            "annotates" => Some(RelationshipKind::Annotates),
            "includes" => Some(RelationshipKind::Includes),
            "extends" => Some(RelationshipKind::Extends),
            "instance of" => Some(RelationshipKind::InstanceOf),
            "links" => Some(RelationshipKind::Links),
            _ => None,
        }
    }
    /// associates/aggregates/composes require `: near to far` ends; the rest forbid them.
    pub fn is_ended(self) -> bool {
        matches!(
            self,
            RelationshipKind::Associates
                | RelationshipKind::Aggregates
                | RelationshipKind::Composes
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct RelEnd {
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    #[cfg_attr(feature = "wasm", tsify(type = "string"))]
    pub multiplicity: Option<Multiplicity>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub role: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub navigable: Option<bool>,
}

/// A relationship's optional `as …` name: a plain label, or a link to a
/// `uml.Association` document (an association class), stored by its resolved slug.
///
/// TS shape (`types.ts`): `string | { ref: string }`. `Label` → bare string,
/// `Assoc` → `{ ref }`. Two `String` newtypes can't disambiguate under
/// `#[serde(untagged)]`, so the impls are hand-written.
#[derive(Debug, Clone, PartialEq)]
pub enum AssocName {
    Label(String),
    Assoc(String),
}

#[cfg(feature = "serde")]
impl serde::Serialize for AssocName {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        match self {
            AssocName::Label(l) => s.serialize_str(l),
            AssocName::Assoc(r) => {
                let mut st = s.serialize_struct("AssocRef", 1)?;
                st.serialize_field("ref", r)?;
                st.end()
            }
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for AssocName {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<AssocName, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum Repr {
            // Struct arm first: a JSON string can't satisfy it, a JSON object can't satisfy the string arm.
            Ref {
                #[serde(rename = "ref")]
                r: String,
            },
            Label(String),
        }
        Ok(match Repr::deserialize(d)? {
            Repr::Ref { r } => AssocName::Assoc(r),
            Repr::Label(l) => AssocName::Label(l),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Edge {
    #[cfg_attr(feature = "serde", serde(rename = "from"))]
    pub source: String,
    #[cfg_attr(feature = "serde", serde(rename = "to"))]
    pub target: String,
    pub kind: RelationshipKind,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    #[cfg_attr(feature = "wasm", tsify(type = "string | { ref: string }"))]
    pub name: Option<AssocName>,
    #[cfg_attr(feature = "serde", serde(rename = "fromEnd"))]
    pub from_end: RelEnd,
    #[cfg_attr(feature = "serde", serde(rename = "toEnd"))]
    pub to_end: RelEnd,
    /// True when a reciprocal `associates` was declared from both ends; both
    /// ends are then navigable. Set during Model resolution (Plan 3).
    pub bidirectional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UmlMetaclass {
    Class,
    Interface,
    Enum,
    DataType,
    Package,
    Note,
    Association,
    Actor,
    UseCase,
    InstanceSpecification,
}

impl UmlMetaclass {
    fn parse(metaclass: &str) -> Option<UmlMetaclass> {
        match metaclass {
            "Class" => Some(UmlMetaclass::Class),
            "Interface" => Some(UmlMetaclass::Interface),
            "Enum" => Some(UmlMetaclass::Enum),
            "DataType" => Some(UmlMetaclass::DataType),
            "Package" => Some(UmlMetaclass::Package),
            "Note" => Some(UmlMetaclass::Note),
            "Association" => Some(UmlMetaclass::Association),
            "Actor" => Some(UmlMetaclass::Actor),
            "UseCase" => Some(UmlMetaclass::UseCase),
            "InstanceSpecification" => Some(UmlMetaclass::InstanceSpecification),
            _ => None,
        }
    }
    fn name(self) -> &'static str {
        match self {
            UmlMetaclass::Class => "Class",
            UmlMetaclass::Interface => "Interface",
            UmlMetaclass::Enum => "Enum",
            UmlMetaclass::DataType => "DataType",
            UmlMetaclass::Package => "Package",
            UmlMetaclass::Note => "Note",
            UmlMetaclass::Association => "Association",
            UmlMetaclass::Actor => "Actor",
            UmlMetaclass::UseCase => "UseCase",
            UmlMetaclass::InstanceSpecification => "InstanceSpecification",
        }
    }
}

/// A behavior document's kind: selects the substrate (flow vs interaction) and
/// the flow flavor. Behavior docs are the document — model AND view — and are
/// never classifier nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehaviorKind {
    Activity,
    StateMachine,
    Sequence,
}

impl BehaviorKind {
    pub fn parse(s: &str) -> Option<BehaviorKind> {
        match s {
            "Activity" => Some(BehaviorKind::Activity),
            "StateMachine" => Some(BehaviorKind::StateMachine),
            "Sequence" => Some(BehaviorKind::Sequence),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            BehaviorKind::Activity => "Activity",
            BehaviorKind::StateMachine => "StateMachine",
            BehaviorKind::Sequence => "Sequence",
        }
    }
}

/// A flow node's closed kind set (heading keyword). `Plain` = no keyword →
/// action (activity) or state (state machine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum FlowNodeKind {
    Initial,
    Final,
    Decision,
    Merge,
    Fork,
    Join,
    Object,
    Plain,
}

impl FlowNodeKind {
    pub fn keyword(self) -> Option<&'static str> {
        match self {
            FlowNodeKind::Initial => Some("initial"),
            FlowNodeKind::Final => Some("final"),
            FlowNodeKind::Decision => Some("decision"),
            FlowNodeKind::Merge => Some("merge"),
            FlowNodeKind::Fork => Some("fork"),
            FlowNodeKind::Join => Some("join"),
            FlowNodeKind::Object => Some("object"),
            FlowNodeKind::Plain => None,
        }
    }
    pub fn from_keyword(s: &str) -> Option<FlowNodeKind> {
        match s {
            "initial" => Some(FlowNodeKind::Initial),
            "final" => Some(FlowNodeKind::Final),
            "decision" => Some(FlowNodeKind::Decision),
            "merge" => Some(FlowNodeKind::Merge),
            "fork" => Some(FlowNodeKind::Fork),
            "join" => Some(FlowNodeKind::Join),
            "object" => Some(FlowNodeKind::Object),
            _ => None,
        }
    }
}

/// Flow flavor: tunes rendering only — one grammar for both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum FlowFlavor {
    Activity,
    StateMachine,
}

/// A behavior flow element in the shared model-level pool (design spec §3): an
/// `Element`, NOT a classifier. Each activity/state-machine node lives here and
/// is referenced from its owning behavior's view (`FlowDoc.nodes`) by `key` —
/// exactly as a class `Diagram` references pooled classifiers by `members`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ActivityNode {
    /// Global pool identity: `"{behavior}#{id}"` (unique across the model).
    pub key: String,
    /// Local heading identity (unique within the owning behavior): the display
    /// name and the name local transitions resolve against.
    pub id: String,
    /// Owning behavior document key.
    pub behavior: String,
    pub kind: FlowNodeKind,
    /// Resolved key of an `object` node's typing classifier.
    #[cfg_attr(
        feature = "serde",
        serde(rename = "objectRef", default, skip_serializing_if = "Option::is_none")
    )]
    pub object_ref: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub partition: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub entry: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(rename = "do", default, skip_serializing_if = "Option::is_none")
    )]
    pub do_: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub exit: Option<String>,
    /// Resolved key of the flow document this composite/call-behavior refines.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub refines: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub notes: Vec<String>,
}

/// The kind of a pooled activity edge (design spec §3). Not flattened into
/// `Association`; each kind keeps its own semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum FlowEdgeKind {
    /// Plain sequencing between activity nodes.
    ControlFlow,
    /// Carries an object token (an `object`-node endpoint, or a `carries` type).
    ObjectFlow,
}

/// A typed control/object flow edge (design spec §3): a model-level pool member,
/// referenced from its owning behavior's view (`FlowDoc.edges`) by `key`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct FlowEdge {
    /// Global pool identity: `"{behavior}#e{n}"`.
    pub key: String,
    pub kind: FlowEdgeKind,
    /// Owning behavior document key.
    pub behavior: String,
    /// Source activity-node pool key (always a node in `behavior`).
    pub from: String,
    /// Target activity-node pool key for a LOCAL target; the link title for a
    /// cross-document target (matches no local node key → not drawn, mirroring
    /// the class-diagram edge rule).
    pub to: String,
    /// Resolved key of the target *behavior document* when the target was a
    /// cross-document link.
    #[cfg_attr(
        feature = "serde",
        serde(rename = "toRef", default, skip_serializing_if = "Option::is_none")
    )]
    pub to_ref: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub trigger: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub guard: Option<String>,
    /// Decision default branch (`else transitions to …`).
    #[cfg_attr(
        feature = "serde",
        serde(rename = "else", default, skip_serializing_if = "is_false")
    )]
    pub is_else: bool,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub effect: Option<String>,
    /// Resolved key of the carried object type (`carries <link>` object flow).
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub carries: Option<String>,
}

/// One behavior document as a **view** (design spec §4): it no longer owns its
/// nodes/edges inline — it references pooled `ActivityNode`s and `FlowEdge`s by
/// key, exactly as a class `Diagram` references pooled classifiers by `members`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct FlowDoc {
    pub key: String,
    pub title: String,
    pub flavor: FlowFlavor,
    /// Resolved key of the entity this behavior describes (frontmatter link).
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub describes: Option<String>,
    /// Pool keys of this behavior's `ActivityNode`s (view → pool reference).
    pub nodes: Vec<String>,
    /// Pool keys of this behavior's `FlowEdge`s (view → pool reference).
    pub edges: Vec<String>,
}

/// The message kind: fixes line and arrowhead (interaction substrate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum MessageVerb {
    Calls,
    Sends,
    Replies,
    Creates,
    Destroys,
}

impl MessageVerb {
    pub fn as_str(self) -> &'static str {
        match self {
            MessageVerb::Calls => "calls",
            MessageVerb::Sends => "sends",
            MessageVerb::Replies => "replies",
            MessageVerb::Creates => "creates",
            MessageVerb::Destroys => "destroys",
        }
    }
    pub fn parse(s: &str) -> Option<MessageVerb> {
        match s {
            "calls" => Some(MessageVerb::Calls),
            "sends" => Some(MessageVerb::Sends),
            "replies" => Some(MessageVerb::Replies),
            "creates" => Some(MessageVerb::Creates),
            "destroys" => Some(MessageVerb::Destroys),
            _ => None,
        }
    }
}

/// Combined-fragment keyword. `par` deferred (open question in spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum FragmentKind {
    Alt,
    Opt,
    Loop,
}

impl FragmentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FragmentKind::Alt => "alt",
            FragmentKind::Opt => "opt",
            FragmentKind::Loop => "loop",
        }
    }
    pub fn parse(s: &str) -> Option<FragmentKind> {
        match s {
            "alt" => Some(FragmentKind::Alt),
            "opt" => Some(FragmentKind::Opt),
            "loop" => Some(FragmentKind::Loop),
            _ => None,
        }
    }
}

/// A message reference or a nested-fragment reference inside an ordered
/// interaction stream (the interaction root, or a fragment operand). Document
/// order within the list is time order (design spec §6). `edge`/`node` are ids
/// into `SequenceDoc.edges` / `SequenceDoc.nodes`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "item", rename_all = "lowercase"))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SeqChild {
    Message { edge: String },
    Fragment { node: String },
}

/// A message: an interaction-LOCAL, ORDERED edge (design spec §6). It is NOT a
/// reusable pool edge and NOT an Association — its identity is bound to this
/// interaction's time axis. `from`/`to` are lifeline node ids (a lifeline's
/// handle: its alias, else its title).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SeqEdge {
    /// Doc-unique id (`m0`, `m1`, … in document/time order), referenced by a
    /// container's ordered `items`.
    pub id: String,
    pub from: String,
    pub verb: MessageVerb,
    pub to: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub signature: Option<String>,
}

/// A node of an interaction's flat model: a participant lifeline, a combined
/// fragment, or a fragment operand. These are interaction-LOCAL (design spec
/// §6) — not members of the shared Element pool. Containment is preserved by id
/// reference: a fragment lists its operand ids; an operand lists its ordered
/// items (message edges + nested fragment nodes).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "node", rename_all = "lowercase"))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SeqNode {
    /// A participant column. `ref_` types-by a pool CLASSIFIER (design spec §6);
    /// widening to `InstanceSpecification` is §7.4 (out of scope here). `id` is
    /// the lifeline handle (alias, else title) that messages reference.
    Lifeline {
        id: String,
        title: String,
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Option::is_none")
        )]
        alias: Option<String>,
        #[cfg_attr(
            feature = "serde",
            serde(rename = "ref", default, skip_serializing_if = "Option::is_none")
        )]
        ref_: Option<String>,
    },
    /// A combined fragment (`alt`/`opt`/`loop`). `operands` are its `Operand`
    /// node ids, in order.
    Fragment {
        id: String,
        kind: FragmentKind,
        operands: Vec<String>,
    },
    /// One operand of a combined fragment. `guard: None` = the `else` operand.
    /// `items` is the ordered message/fragment stream (time order).
    Operand {
        id: String,
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Option::is_none")
        )]
        guard: Option<String>,
        items: Vec<SeqChild>,
    },
}

/// One interaction (`uml.Sequence`): a flat, interaction-local model of
/// lifelines/fragments/operands (`nodes`) and ordered messages (`edges`), with
/// containment preserved via `items` (the root stream) plus each fragment's
/// operand ids and each operand's item stream. This is the RUNTIME view; the
/// on-disk `## Lifelines`/`## Messages` form (nested) is a separate storage
/// shape (design spec §9 — storage/runtime need not be 1:1).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SequenceDoc {
    pub key: String,
    pub title: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub describes: Option<String>,
    /// Lifelines + fragments + operands; resolve by `id`. Lifelines appear first,
    /// in declaration order (participant column order).
    pub nodes: Vec<SeqNode>,
    /// Messages, ORDERED (document order = time order); interaction-local.
    pub edges: Vec<SeqEdge>,
    /// The interaction root's ordered item stream (message/fragment refs).
    pub items: Vec<SeqChild>,
}

/// An element's `type`. Graceful degradation is a type-level guarantee: any
/// unrecognized token becomes `Unknown` and renders as a generic labelled box.
///
/// Serializes as the flat TS `type` string (`"uml.Class"` / `"Diagram"` / opaque);
/// `parse` is total, so `From<String>` (not `TryFrom`) drives Deserialize.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(into = "String", from = "String"))]
pub enum ElementType {
    Uml(UmlMetaclass),
    Behavior(BehaviorKind),
    Diagram,
    Unknown(String),
}

#[cfg(feature = "serde")]
impl From<ElementType> for String {
    fn from(t: ElementType) -> String {
        t.as_str()
    }
}

#[cfg(feature = "serde")]
impl From<String> for ElementType {
    fn from(s: String) -> ElementType {
        ElementType::parse(&s)
    }
}

impl ElementType {
    pub fn parse(s: &str) -> ElementType {
        if s == "Diagram" {
            return ElementType::Diagram;
        }
        if let Some((family, metaclass)) = s.split_once('.') {
            if family == "uml" {
                if let Some(mc) = UmlMetaclass::parse(metaclass) {
                    return ElementType::Uml(mc);
                }
                if let Some(bk) = BehaviorKind::parse(metaclass) {
                    return ElementType::Behavior(bk);
                }
            }
        }
        ElementType::Unknown(s.to_string())
    }
    pub fn as_str(&self) -> String {
        match self {
            ElementType::Uml(mc) => format!("uml.{}", mc.name()),
            ElementType::Behavior(bk) => format!("uml.{}", bk.name()),
            ElementType::Diagram => "Diagram".to_string(),
            ElementType::Unknown(s) => s.clone(),
        }
    }

    /// True only for pool members that are genuine UML **Classifiers** (design
    /// spec §3.1): `Class`, `Interface`, `Enum`, `DataType`, `Actor`, `UseCase`,
    /// `Association`, and the behavior classifiers (`Behavior ⊂ Class`).
    /// `Package`, `Note`/`Comment`, `Diagram`, and unrecognized tokens are not.
    ///
    /// The `UmlMetaclass` arm is written out explicitly (no `_ =>` catch-all) so
    /// adding a metaclass forces a classifier decision here at compile time.
    pub fn is_classifier(&self) -> bool {
        match self {
            ElementType::Uml(mc) => match mc {
                UmlMetaclass::Class
                | UmlMetaclass::Interface
                | UmlMetaclass::Enum
                | UmlMetaclass::DataType
                | UmlMetaclass::Actor
                | UmlMetaclass::UseCase
                | UmlMetaclass::Association => true,
                UmlMetaclass::Package
                | UmlMetaclass::Note
                | UmlMetaclass::InstanceSpecification => false,
            },
            // Behavior ⊂ Class: Activity / Interaction (Sequence) / StateMachine
            // are all Classifiers.
            ElementType::Behavior(_) => true,
            ElementType::Diagram => false,
            ElementType::Unknown(_) => false,
        }
    }

    /// True for element types that are **views / notation**, not pooled
    /// classifiers: `Diagram` (a class-diagram view) and every behavior kind
    /// (activity / state machine / interaction — each a view over pooled
    /// behavior elements, design spec §4). A view never contributes a
    /// classifier `Node` to `Model.nodes` and is never a relationship/link
    /// target. Distinct from `is_classifier()`, which is `true` for behaviors.
    pub fn is_view(&self) -> bool {
        matches!(self, ElementType::Diagram | ElementType::Behavior(_))
    }
}

/// A `uml.Note` anchor. Three forms, per the spec.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum NoteAnchor {
    Classifier {
        #[cfg_attr(feature = "serde", serde(rename = "targetKey"))]
        target_key: String,
    },
    NamedAssoc {
        #[cfg_attr(feature = "serde", serde(rename = "sourceKey"))]
        source_key: String,
        name: String,
    },
    EndpointAssoc {
        #[cfg_attr(feature = "serde", serde(rename = "sourceKey"))]
        source_key: String,
        kind: RelationshipKind,
        #[cfg_attr(feature = "serde", serde(rename = "targetKey"))]
        target_key: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Node {
    /// Lossless OKF projection of this node's source document (OKF tier) and the
    /// single authoritative source for `title`/`description`/verbatim `body` (read
    /// via `concept.title`/`concept.description`/`concept.body`) plus the non-UML
    /// OKF fields (`tags`/`resource`/`timestamp`/`links`/`citations`/`role`/`extra`).
    /// Populated from `crate::okf::project` (single source).
    pub concept: crate::okf::Concept,
    pub key: String,
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    #[cfg_attr(feature = "wasm", tsify(type = "string"))]
    pub ty: ElementType,
    pub stereotypes: Vec<String>,
    #[cfg_attr(
        feature = "serde",
        serde(rename = "abstract", default, skip_serializing_if = "is_false")
    )]
    pub abstract_: bool,
    pub attributes: Vec<Attribute>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub values: Vec<String>,
    /// A `uml.Note`'s markdown prose (from its `## Body` section). Distinct from
    /// the generic verbatim `concept.body`: this is the Note-specific rendered
    /// prose. Sole reader is the note node renderer. Title/description/verbatim
    /// body now live only on `concept` (the single authoritative source).
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub note_body: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub annotates: Vec<NoteAnchor>,
    /// Owned member keys (classifiers, diagrams, sub-packages), in progressive-
    /// disclosure order. Meaningful only on `uml.Package` nodes; empty elsewhere.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub members: Vec<String>,
    /// Slot values on an `InstanceSpecification` node (design spec §3.3). Empty
    /// on every non-instance node.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub slots: Vec<Slot>,
}

/// A resolved membership group in a diagram (heading text + resolved keys).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct DiagramGroup {
    pub name: String,
    pub members: Vec<String>,
    pub children: Vec<DiagramGroup>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Diagram {
    pub key: String,
    pub title: String,
    pub profile: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub description: Option<String>,
    pub groups: Vec<DiagramGroup>,
    // `layout` carries the raw layout AST (`syntax::LayoutStatement`). Serialized
    // end to end (Phase 2) so the frontend can read the layout relations.
    #[cfg_attr(feature = "wasm", tsify(type = "unknown[]"))]
    pub layout: Vec<crate::syntax::LayoutStatement>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "DiagramDisplay::is_empty")
    )]
    pub display: DiagramDisplay,
}

/// A diagram's authored render settings — a PARTIAL. Only keys present in the
/// file are `Some`/non-empty; TS `resolveDisplay` fills the rest from
/// `DEFAULT_DISPLAY`. Serde `rename_all="camelCase"` matches the TS keys.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase", default))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct DiagramDisplay {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub show_attributes: Option<bool>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub show_type: Option<bool>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub show_attribute_visibility: Option<bool>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub show_attribute_multiplicity: Option<bool>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub max_attributes: Option<u32>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub show_roles: Option<bool>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub show_cardinality: Option<bool>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub show_labels: Option<bool>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub show_stereotype: Option<bool>,
    /// `None` ⇒ key absent ⇒ show all; `Some(vec)` ⇒ allowlist (empty ⇒ show none).
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub stereotype_filter: Option<Vec<String>>,
    /// Opaque `"name:#rrggbb"` pairs; empty ⇒ key absent.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
    pub stereotype_colors: Vec<String>,
}

impl DiagramDisplay {
    pub fn is_empty(&self) -> bool {
        *self == DiagramDisplay::default()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Model {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub diagrams: Vec<Diagram>,
    /// Bundle/root name (root `index.md` H1); "" when absent. Export label + root crumb.
    #[cfg_attr(feature = "serde", serde(default))]
    pub path: String,
    /// Discovered `uml.Package` nodes (root + nested). Kept out of `nodes` so
    /// classifier consumers are unaffected.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub packages: Vec<Node>,
    /// Flow-substrate behavior documents (uml.Activity / uml.StateMachine).
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub flows: Vec<FlowDoc>,
    /// Model-level pool of behavior flow elements (activity/state-machine nodes),
    /// referenced by `FlowDoc.nodes`. Design spec §3/§4.
    #[cfg_attr(
        feature = "serde",
        serde(
            rename = "activityNodes",
            default,
            skip_serializing_if = "Vec::is_empty"
        )
    )]
    pub activity_nodes: Vec<ActivityNode>,
    /// Model-level pool of typed control/object flow edges, referenced by
    /// `FlowDoc.edges`. Design spec §3/§4.
    #[cfg_attr(
        feature = "serde",
        serde(rename = "flowEdges", default, skip_serializing_if = "Vec::is_empty")
    )]
    pub flow_edges: Vec<FlowEdge>,
    /// Interaction-substrate behavior documents (uml.Sequence).
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub interactions: Vec<SequenceDoc>,
}

impl Model {
    pub fn node(&self, key: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.key == key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagram_display_default_is_empty() {
        let d = DiagramDisplay::default();
        assert!(d.is_empty(), "an all-None/empty display must report empty");
    }

    #[test]
    fn diagram_display_with_a_set_field_is_not_empty() {
        let d = DiagramDisplay {
            show_attributes: Some(false),
            ..Default::default()
        };
        assert!(!d.is_empty(), "any set field makes the display non-empty");
    }

    #[test]
    fn relationship_kind_round_trips() {
        for k in [
            RelationshipKind::Associates,
            RelationshipKind::Aggregates,
            RelationshipKind::Composes,
            RelationshipKind::Specializes,
            RelationshipKind::Implements,
            RelationshipKind::Depends,
            RelationshipKind::Annotates,
            RelationshipKind::Includes,
            RelationshipKind::Extends,
            RelationshipKind::InstanceOf,
            RelationshipKind::Links,
        ] {
            assert_eq!(RelationshipKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(RelationshipKind::parse("nope"), None);
    }

    #[test]
    fn actor_and_usecase_metaclasses_parse_and_round_trip() {
        assert_eq!(
            ElementType::parse("uml.Actor"),
            ElementType::Uml(UmlMetaclass::Actor)
        );
        assert_eq!(
            ElementType::parse("uml.UseCase"),
            ElementType::Uml(UmlMetaclass::UseCase)
        );
        assert_eq!(ElementType::Uml(UmlMetaclass::Actor).as_str(), "uml.Actor");
        assert_eq!(
            ElementType::Uml(UmlMetaclass::UseCase).as_str(),
            "uml.UseCase"
        );
    }

    #[test]
    fn behavior_types_parse_and_round_trip() {
        assert_eq!(
            ElementType::parse("uml.Activity"),
            ElementType::Behavior(BehaviorKind::Activity)
        );
        assert_eq!(
            ElementType::parse("uml.StateMachine"),
            ElementType::Behavior(BehaviorKind::StateMachine)
        );
        assert_eq!(
            ElementType::parse("uml.Sequence"),
            ElementType::Behavior(BehaviorKind::Sequence)
        );
        assert_eq!(
            ElementType::Behavior(BehaviorKind::StateMachine).as_str(),
            "uml.StateMachine"
        );
    }

    #[test]
    fn includes_and_extends_are_endless_dependency_verbs() {
        assert_eq!(
            RelationshipKind::parse("includes"),
            Some(RelationshipKind::Includes)
        );
        assert_eq!(
            RelationshipKind::parse("extends"),
            Some(RelationshipKind::Extends)
        );
        assert_eq!(RelationshipKind::Includes.as_str(), "includes");
        assert_eq!(RelationshipKind::Extends.as_str(), "extends");
        assert!(!RelationshipKind::Includes.is_ended());
        assert!(!RelationshipKind::Extends.is_ended());
    }

    #[test]
    fn only_association_family_takes_ends() {
        assert!(RelationshipKind::Associates.is_ended());
        assert!(RelationshipKind::Aggregates.is_ended());
        assert!(RelationshipKind::Composes.is_ended());
        assert!(!RelationshipKind::Specializes.is_ended());
        assert!(!RelationshipKind::Implements.is_ended());
        assert!(!RelationshipKind::Depends.is_ended());
        assert!(!RelationshipKind::Annotates.is_ended());
    }

    #[test]
    fn classifier_type_parses_known_and_unknown() {
        assert_eq!(
            ElementType::parse("uml.Class"),
            ElementType::Uml(UmlMetaclass::Class)
        );
        assert_eq!(ElementType::parse("Diagram"), ElementType::Diagram);
        assert_eq!(
            ElementType::parse("bpmn.Task"),
            ElementType::Unknown("bpmn.Task".to_string())
        );
        assert_eq!(
            ElementType::parse("LegacyToken"),
            ElementType::Unknown("LegacyToken".to_string())
        );
    }

    #[test]
    fn classifier_type_round_trips_to_string() {
        assert_eq!(ElementType::Uml(UmlMetaclass::Enum).as_str(), "uml.Enum");
        assert_eq!(ElementType::Diagram.as_str(), "Diagram");
        assert_eq!(ElementType::Unknown("x.Y".to_string()).as_str(), "x.Y");
    }

    #[test]
    fn is_classifier_matches_spec_table() {
        // Genuine UML classifiers (design spec §3.1).
        assert!(ElementType::Uml(UmlMetaclass::Class).is_classifier());
        assert!(ElementType::Uml(UmlMetaclass::Interface).is_classifier());
        assert!(ElementType::Uml(UmlMetaclass::Enum).is_classifier());
        assert!(ElementType::Uml(UmlMetaclass::DataType).is_classifier());
        assert!(ElementType::Uml(UmlMetaclass::Actor).is_classifier());
        assert!(ElementType::Uml(UmlMetaclass::UseCase).is_classifier());
        assert!(ElementType::Uml(UmlMetaclass::Association).is_classifier());
        // Behavior ⊂ Class: all behavior classifiers qualify.
        assert!(ElementType::Behavior(BehaviorKind::Activity).is_classifier());
        assert!(ElementType::Behavior(BehaviorKind::StateMachine).is_classifier());
        assert!(ElementType::Behavior(BehaviorKind::Sequence).is_classifier());
        // Not classifiers.
        assert!(!ElementType::Uml(UmlMetaclass::Package).is_classifier());
        assert!(!ElementType::Uml(UmlMetaclass::Note).is_classifier());
        assert!(!ElementType::Uml(UmlMetaclass::InstanceSpecification).is_classifier());
        assert!(!ElementType::Diagram.is_classifier());
        assert!(!ElementType::Unknown("bpmn.Task".to_string()).is_classifier());
    }

    #[test]
    fn is_view_flags_diagrams_and_behaviors() {
        // Views / notation — never pooled classifiers, never link targets.
        assert!(ElementType::Diagram.is_view());
        assert!(ElementType::Behavior(BehaviorKind::Activity).is_view());
        assert!(ElementType::Behavior(BehaviorKind::StateMachine).is_view());
        assert!(ElementType::Behavior(BehaviorKind::Sequence).is_view());
        // Pool members (classifiers, notes, unknowns) are not views.
        assert!(!ElementType::Uml(UmlMetaclass::Class).is_view());
        assert!(!ElementType::Uml(UmlMetaclass::Note).is_view());
        assert!(!ElementType::Uml(UmlMetaclass::Package).is_view());
        assert!(!ElementType::Uml(UmlMetaclass::InstanceSpecification).is_view());
        assert!(!ElementType::Unknown("bpmn.Task".to_string()).is_view());
    }

    #[test]
    fn instance_specification_metaclass_round_trips_and_is_not_a_classifier() {
        assert_eq!(
            ElementType::parse("uml.InstanceSpecification"),
            ElementType::Uml(UmlMetaclass::InstanceSpecification)
        );
        assert_eq!(
            ElementType::Uml(UmlMetaclass::InstanceSpecification).as_str(),
            "uml.InstanceSpecification"
        );
        // An instance is NOT a classifier (spec §3.1) and NOT a view (it is a
        // pool member).
        assert!(!ElementType::Uml(UmlMetaclass::InstanceSpecification).is_classifier());
        assert!(!ElementType::Uml(UmlMetaclass::InstanceSpecification).is_view());
    }

    #[test]
    fn model_looks_up_nodes_by_key() {
        let node = Node {
            concept: crate::okf::project(
                "order.md",
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
            ),
            key: "order".to_string(),
            ty: ElementType::Uml(UmlMetaclass::Class),
            stereotypes: vec![],
            abstract_: false,
            attributes: vec![],
            values: vec![],
            note_body: None,
            annotates: vec![],
            members: vec![],
            slots: vec![],
        };
        let model = Model {
            nodes: vec![node],
            ..Default::default()
        };
        assert_eq!(
            model.node("order").and_then(|n| n.concept.title.as_deref()),
            Some("Order")
        );
        assert!(model.node("missing").is_none());
    }

    #[test]
    fn message_verbs_and_fragment_kinds_round_trip() {
        for v in [
            MessageVerb::Calls,
            MessageVerb::Sends,
            MessageVerb::Replies,
            MessageVerb::Creates,
            MessageVerb::Destroys,
        ] {
            assert_eq!(MessageVerb::parse(v.as_str()), Some(v));
        }
        assert_eq!(MessageVerb::parse("shouts"), None);
        for k in [FragmentKind::Alt, FragmentKind::Opt, FragmentKind::Loop] {
            assert_eq!(FragmentKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(
            FragmentKind::parse("par"),
            None,
            "par operands are deferred"
        );
    }

    #[test]
    fn attribute_defaults_multiplicity_to_one() {
        let a = Attribute {
            name: "id".to_string(),
            ty: TypeRef {
                name: "OrderId".to_string(),
                ref_: None,
            },
            multiplicity: Multiplicity::default(),
            visibility: None,
            description: None,
        };
        assert_eq!(a.multiplicity.as_str(), "1");
    }
}
