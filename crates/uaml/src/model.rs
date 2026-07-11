use crate::multiplicity::Multiplicity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Package,
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
pub struct TypeRef {
    pub name: String,
    pub ref_: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub ty: TypeRef,
    pub multiplicity: Multiplicity,
    pub visibility: Option<Visibility>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub struct RelEnd {
    pub multiplicity: Option<Multiplicity>,
    pub role: Option<String>,
    pub navigable: Option<bool>,
}

/// A relationship's optional `as …` name: a plain label, or a link to a
/// `uml.Association` document (an association class), stored by its resolved slug.
#[derive(Debug, Clone, PartialEq)]
pub enum AssocName {
    Label(String),
    Assoc(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub kind: RelationshipKind,
    pub name: Option<AssocName>,
    pub from_end: RelEnd,
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
#[derive(Debug, Clone, PartialEq)]
pub enum ClassifierType {
    Uml(UmlMetaclass),
    Diagram,
    Unknown(String),
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
pub enum NoteAnchor {
    Classifier { target_key: String },
    NamedAssoc { source_key: String, name: String },
    EndpointAssoc {
        source_key: String,
        kind: RelationshipKind,
        target_key: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub key: String,
    pub ty: ClassifierType,
    pub title: String,
    pub stereotypes: Vec<String>,
    pub abstract_: bool,
    pub description: Option<String>,
    pub attributes: Vec<Attribute>,
    pub values: Vec<String>,
    pub body: Option<String>,
    pub annotates: Vec<NoteAnchor>,
}

/// A diagram member: a classifier slug drawn in a view.
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub key: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagram {
    pub key: String,
    pub title: String,
    pub profile: String,
    pub members: Vec<Member>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub diagrams: Vec<Diagram>,
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
            key: "order".to_string(),
            ty: ClassifierType::Uml(UmlMetaclass::Class),
            title: "Order".to_string(),
            stereotypes: vec![],
            abstract_: false,
            description: None,
            attributes: vec![],
            values: vec![],
            body: None,
            annotates: vec![],
        };
        let model = Model { nodes: vec![node], edges: vec![], diagrams: vec![] };
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
