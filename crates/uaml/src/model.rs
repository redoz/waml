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
pub struct Attribute {
    pub name: String,
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub ty: TypeRef,
    pub multiplicity: Multiplicity,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub visibility: Option<Visibility>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum RelationshipKind {
    Associates,
    Aggregates,
    Composes,
    Specializes,
    Implements,
    Depends,
    Annotates,
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
pub struct RelEnd {
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
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
        }
    }
}

/// A classifier's `type`. Graceful degradation is a type-level guarantee: any
/// unrecognized token becomes `Unknown` and renders as a generic labelled box.
///
/// Serializes as the flat TS `type` string (`"uml.Class"` / `"Diagram"` / opaque);
/// `parse` is total, so `From<String>` (not `TryFrom`) drives Deserialize.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(into = "String", from = "String"))]
pub enum ClassifierType {
    Uml(UmlMetaclass),
    Diagram,
    Unknown(String),
}

#[cfg(feature = "serde")]
impl From<ClassifierType> for String {
    fn from(t: ClassifierType) -> String {
        t.as_str()
    }
}

#[cfg(feature = "serde")]
impl From<String> for ClassifierType {
    fn from(s: String) -> ClassifierType {
        ClassifierType::parse(&s)
    }
}

impl ClassifierType {
    pub fn parse(s: &str) -> ClassifierType {
        if s == "Diagram" {
            return ClassifierType::Diagram;
        }
        if let Some((family, metaclass)) = s.split_once('.') {
            if family == "uml" {
                if let Some(mc) = UmlMetaclass::parse(metaclass) {
                    return ClassifierType::Uml(mc);
                }
            }
        }
        ClassifierType::Unknown(s.to_string())
    }
    pub fn as_str(&self) -> String {
        match self {
            ClassifierType::Uml(mc) => format!("uml.{}", mc.name()),
            ClassifierType::Diagram => "Diagram".to_string(),
            ClassifierType::Unknown(s) => s.clone(),
        }
    }
}

/// A `uml.Note` anchor. Three forms, per the spec.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
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
pub struct Node {
    /// Lossless OKF projection of this node's source document (OKF tier). Nested
    /// additively beneath the flat UML fields below — every flat field mirrors a
    /// `concept.*` slot, but `concept` also carries the non-UML OKF fields
    /// (`tags`/`resource`/`timestamp`/`links`/`citations`/`role`/`extra`) the flat
    /// projection drops. Populated from `crate::okf::project` (single source).
    pub concept: crate::okf::Concept,
    pub key: String,
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub ty: ClassifierType,
    pub title: String,
    pub stereotypes: Vec<String>,
    #[cfg_attr(
        feature = "serde",
        serde(rename = "abstract", default, skip_serializing_if = "is_false")
    )]
    pub abstract_: bool,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub description: Option<String>,
    pub attributes: Vec<Attribute>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub values: Vec<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub body: Option<String>,
    /// A `uml.Note`'s markdown prose (from its `## Body` section), byte-identical
    /// to flat `body` during the migration. Distinct from the generic verbatim
    /// `concept.body`: this is the Note-specific rendered prose. Sole reader is
    /// the note node renderer.
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
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub members: Vec<String>,
}

/// A resolved membership group in a diagram (heading text + resolved keys).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiagramGroup {
    pub name: String,
    pub members: Vec<String>,
    pub children: Vec<DiagramGroup>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Diagram {
    pub key: String,
    pub title: String,
    pub profile: String,
    pub groups: Vec<DiagramGroup>,
    // `layout` carries the raw layout AST (`syntax::LayoutStatement`). Serialized
    // end to end (Phase 2) so the frontend can read the layout relations.
    pub layout: Vec<crate::syntax::LayoutStatement>,
}

#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Model {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub diagrams: Vec<Diagram>,
    /// Bundle/root name (root `index.md` H1); "" when absent. Export label + root crumb.
    #[cfg_attr(feature = "serde", serde(default))]
    pub path: String,
    /// Discovered `uml.Package` nodes (root + nested). Kept out of `nodes` so
    /// classifier consumers are unaffected.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub packages: Vec<Node>,
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
    fn relationship_kind_round_trips() {
        for k in [
            RelationshipKind::Associates,
            RelationshipKind::Aggregates,
            RelationshipKind::Composes,
            RelationshipKind::Specializes,
            RelationshipKind::Implements,
            RelationshipKind::Depends,
            RelationshipKind::Annotates,
        ] {
            assert_eq!(RelationshipKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(RelationshipKind::parse("nope"), None);
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
            ClassifierType::parse("uml.Class"),
            ClassifierType::Uml(UmlMetaclass::Class)
        );
        assert_eq!(ClassifierType::parse("Diagram"), ClassifierType::Diagram);
        assert_eq!(
            ClassifierType::parse("bpmn.Task"),
            ClassifierType::Unknown("bpmn.Task".to_string())
        );
        assert_eq!(
            ClassifierType::parse("LegacyToken"),
            ClassifierType::Unknown("LegacyToken".to_string())
        );
    }

    #[test]
    fn classifier_type_round_trips_to_string() {
        assert_eq!(ClassifierType::Uml(UmlMetaclass::Enum).as_str(), "uml.Enum");
        assert_eq!(ClassifierType::Diagram.as_str(), "Diagram");
        assert_eq!(
            ClassifierType::Unknown("x.Y".to_string()).as_str(),
            "x.Y"
        );
    }

    #[test]
    fn model_looks_up_nodes_by_key() {
        let node = Node {
            concept: crate::okf::project(
                "order.md",
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
            ),
            key: "order".to_string(),
            ty: ClassifierType::Uml(UmlMetaclass::Class),
            title: "Order".to_string(),
            stereotypes: vec![],
            abstract_: false,
            description: None,
            attributes: vec![],
            values: vec![],
            body: None,
            note_body: None,
            annotates: vec![],
            members: vec![],
        };
        let model = Model { nodes: vec![node], edges: vec![], diagrams: vec![], path: String::new(), packages: vec![] };
        assert_eq!(model.node("order").map(|n| n.title.as_str()), Some("Order"));
        assert!(model.node("missing").is_none());
    }

    #[test]
    fn attribute_defaults_multiplicity_to_one() {
        let a = Attribute {
            name: "id".to_string(),
            ty: TypeRef { name: "OrderId".to_string(), ref_: None },
            multiplicity: Multiplicity::default(),
            visibility: None,
            description: None,
        };
        assert_eq!(a.multiplicity.as_str(), "1");
    }
}
