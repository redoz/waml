#![cfg(feature = "serde")]
//! Pins the retained Rust JSON shape of `Model`.
use waml::diagnostic::{DiagCode, Diagnostic, Severity};
use waml::model::{
    AssocName, BehaviorKind, ElementType, EndpointRef, FragmentKind, InteractionUseId, MessageId,
    MessageKind, Model, Node, OperandSpec, SeqBinding, SeqChild, SeqEdge, SeqInteractionUse,
    SeqNode, SequenceDoc, UmlMetaclass, Visibility,
};
use waml::multiplicity::Multiplicity;
fn projection(bundle: &[(String, String)]) -> Model {
    let source = waml::source::SourceBundle::try_from_pairs(bundle.iter().cloned()).unwrap();
    waml::analysis::prepare_candidate(source, None, 0)
        .unwrap()
        .uml()
        .projection
        .clone()
}

fn bundle() -> Vec<(String, String)> {
    vec![
        (
            "m/order.md".into(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId {1}\n\n## Relationships\n- composes [Line](./line.md): 1 to 1..*\n".into(),
        ),
        (
            "m/line.md".into(),
            "---\ntype: uml.Class\ntitle: Line\n---\n# Line\n".into(),
        ),
    ]
}

#[test]
fn okf_bundle_json_has_separate_semantic_collections_and_string_bodies() {
    let empty = serde_json::to_value(waml::okf::Bundle::default()).unwrap();
    assert_eq!(
        empty,
        serde_json::json!({
            "concepts": [],
            "indexes": [],
            "logs": [],
            "directories": []
        })
    );

    let source = waml::source::SourceBundle::try_from_pairs([
        ("index.md", "# Root\n"),
        ("note.md", "---\ntype: Note\n---\n# Note\n"),
        ("log.md", "# Log\n"),
    ])
    .unwrap();
    let parsed = waml::okf::Bundle::parse(&source).unwrap();
    let value = serde_json::to_value(parsed).unwrap();

    assert!(value["concepts"][0]["body"].is_string());
    assert!(value["indexes"][0]["body"].is_string());
    assert!(value["logs"][0]["body"].is_string());
    assert!(value["concepts"][0].get("role").is_none());
    let encoded = serde_json::to_string(&value).unwrap();
    assert!(!encoded.contains("\"source\""));
    assert!(!encoded.contains("\"range\""));
    assert!(value["directories"].is_array());
}

#[test]
fn directory_address_deserialization_enforces_rooted_invariants() {
    assert!(serde_json::from_str::<waml::okf::DirectoryAddress>(r#""/sales/orders""#).is_ok());
    for invalid in [
        r#""sales""#,
        r#""/sales/../orders""#,
        r#""/sales/index.md""#,
    ] {
        assert!(
            serde_json::from_str::<waml::okf::DirectoryAddress>(invalid).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn selective_uml_projection_omits_unknowns_and_structural_packages() {
    let source = waml::source::SourceBundle::try_from_pairs([
        ("index.md", "# Root\n"),
        ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
        ("vendor.md", "---\ntype: vendor.Custom\n---\n# Vendor\n"),
    ])
    .unwrap();
    let projection = waml::analysis::prepare_candidate(source, None, 0)
        .unwrap()
        .uml()
        .projection
        .clone();
    let value = serde_json::to_value(&projection).unwrap();

    assert_eq!(value["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(value["nodes"][0]["key"], "order");
    assert!(value["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|node| node["type"] != serde_json::json!("")));
    assert!(projection
        .nodes
        .iter()
        .all(|node| !matches!(node.ty, ElementType::Unknown(_))));
    assert!(value.get("packages").is_none());
    assert_eq!(value["path"], "");
}

#[test]
fn model_json_wire_field_names() {
    let model = projection(&bundle());
    let v = serde_json::to_value(&model).unwrap();

    let node = v["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["key"] == "m/order")
        .unwrap();
    // Wire contract: the node's type key serializes as `type` and its identity as `key` (not `ty`).
    assert_eq!(node["type"], "uml.Class");
    assert_eq!(node["key"], "m/order");
    // Flat title/description/body are DELETED — the concept is the single source.
    assert!(node.get("title").is_none(), "flat title deleted: {node}");
    assert!(
        node.get("description").is_none(),
        "flat description deleted: {node}"
    );
    assert!(node.get("body").is_none(), "flat body deleted: {node}");
    assert_eq!(node["concept"]["id"], "m/order");
    assert_eq!(node["concept"]["title"], "Order");
    // Attribute.type is a TypeRef ({ name, ref? }); multiplicity is a canonical string.
    assert_eq!(node["attributes"][0]["name"], "id");
    assert_eq!(node["attributes"][0]["type"]["name"], "OrderId");
    assert_eq!(node["attributes"][0]["multiplicity"], "1");

    let edge = &v["edges"][0];
    // Wire contract: an edge's endpoints serialize as `from`/`to` and its kind as a lowercase string.
    assert_eq!(edge["kind"], "composes");
    assert_eq!(edge["from"], "m/order");
    assert_eq!(edge["to"], "m/line");
}

#[test]
fn stringy_newtypes_serialize_as_their_canonical_string() {
    // Multiplicity ⇒ bare string.
    assert_eq!(
        serde_json::to_value(Multiplicity::parse("1..*").unwrap()).unwrap(),
        serde_json::json!("1..*")
    );
    // Visibility ⇒ single-char marker string.
    assert_eq!(
        serde_json::to_value(Visibility::Private).unwrap(),
        serde_json::json!("-")
    );
}

#[test]
fn assoc_name_matches_ts_union_shape() {
    // TS: name?: string | { ref: string }
    assert_eq!(
        serde_json::to_value(AssocName::Label("has".into())).unwrap(),
        serde_json::json!("has")
    );
    assert_eq!(
        serde_json::to_value(AssocName::Assoc("employment".into())).unwrap(),
        serde_json::json!({ "ref": "employment" })
    );
}

#[test]
fn package_node_and_model_path() {
    let pkg = Node {
        concept: waml::okf::project("sales/package.md", "# sales\n\nSales bounded context.\n"),
        key: "sales".into(),
        ty: ElementType::Uml(UmlMetaclass::Package),
        stereotypes: vec![],
        abstract_: false,
        attributes: vec![],
        values: vec![],
        note_body: None,
        annotates: vec![],
        members: vec!["order".into(), "customer".into()],
        slots: vec![],
    };
    let model = Model {
        nodes: vec![],
        edges: vec![],
        diagrams: vec![],
        path: "acme-model".into(),
        packages: vec![pkg],
        ..Default::default()
    };
    let json = serde_json::to_string(&model).unwrap();
    assert!(json.contains("\"path\":\"acme-model\""));
    assert!(json.contains("\"members\":[\"order\",\"customer\"]"));
    // classifier with no members must omit field entirely.
    let bare = Node {
        concept: waml::okf::project(
            "order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        ),
        key: "order".into(),
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
    let bj = serde_json::to_string(&bare).unwrap();
    assert!(
        !bj.contains("members"),
        "empty members must be omitted: {bj}"
    );
}

#[test]
fn slot_serializes_with_ref_key_and_skips_none() {
    use waml::model::Slot;
    let bare = Slot {
        name: "id".into(),
        value: "ORD-42".into(),
        ref_: None,
    };
    let v = serde_json::to_value(&bare).unwrap();
    assert_eq!(v["name"], "id");
    assert_eq!(v["value"], "ORD-42");
    assert!(v.get("ref").is_none(), "None ref must be omitted: {v}");

    let linked = Slot {
        name: "customer".into(),
        value: "Ann".into(),
        ref_: Some("m/ann".into()),
    };
    assert_eq!(serde_json::to_value(&linked).unwrap()["ref"], "m/ann");
}

#[test]
fn instance_edge_kinds_serialize_lowercase() {
    use waml::model::RelationshipKind;
    assert_eq!(
        serde_json::to_value(RelationshipKind::InstanceOf).unwrap(),
        serde_json::json!("instanceof")
    );
    assert_eq!(
        serde_json::to_value(RelationshipKind::Links).unwrap(),
        serde_json::json!("links")
    );
    // Markdown verb (as_str) keeps the authored spelling.
    assert_eq!(RelationshipKind::InstanceOf.as_str(), "instance of");
    assert_eq!(RelationshipKind::Links.as_str(), "links");
    assert!(!RelationshipKind::InstanceOf.is_ended());
    assert!(!RelationshipKind::Links.is_ended());
}

#[test]
fn classifier_node_omits_empty_slots() {
    // A plain class must omit `slots` entirely (skip-if-empty, mirrors values).
    let m = projection(&bundle());
    let v = serde_json::to_value(&m).unwrap();
    assert!(
        v["nodes"][0].get("slots").is_none(),
        "empty slots must be omitted: {}",
        v["nodes"][0]
    );
}

#[test]
fn flow_doc_json_matches_ts_field_names() {
    let b = vec![
        ("m/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
        ("m/lifecycle.md".to_string(),
         "---\ntype: uml.StateMachine\ntitle: Order Lifecycle\ndescribes: [Order](./order.md)\n---\n# Order Lifecycle\n\n## Nodes\n\n### initial\n- transitions to Draft\n\n### Draft\n- on `place` when `items > 0` transitions to Placed: `reserve`\n- else transitions to Cancelled\n\n### Placed\n- entry: `reserveStock`\n\n### Cancelled\n\n### final\n".to_string()),
    ];
    let m = projection(&b);
    let v = serde_json::to_value(&m).unwrap();
    let f = &v["flows"][0];
    assert_eq!(f["key"], "m/lifecycle");
    assert_eq!(f["flavor"], "stateMachine");
    assert_eq!(f["describes"], "m/order");
    // The view references pooled nodes/edges by key (no inline objects).
    assert_eq!(f["nodes"][0], "m/lifecycle#initial");
    assert_eq!(f["edges"][1], "m/lifecycle#e1");
    // Activity nodes live in the model-level `activityNodes` pool.
    assert_eq!(v["activityNodes"][0]["kind"], "initial");
    assert_eq!(v["activityNodes"][0]["behavior"], "m/lifecycle");
    assert_eq!(v["activityNodes"][2]["entry"], "reserveStock");
    // Flow edges live in the typed model-level `flowEdges` pool.
    let e = &v["flowEdges"][1];
    assert_eq!(e["from"], "m/lifecycle#Draft");
    assert_eq!(e["kind"], "controlFlow");
    assert_eq!(e["trigger"], "place");
    assert_eq!(e["guard"], "items > 0");
    assert_eq!(e["effect"], "reserve");
    assert_eq!(v["flowEdges"][2]["else"], true);
    // classifier-only models omit the fields entirely
    let m2 = projection(&[(
        "a.md".to_string(),
        "---\ntype: uml.Class\ntitle: A\n---\n# A\n".to_string(),
    )]);
    let v2 = serde_json::to_value(&m2).unwrap();
    assert!(v2.get("flows").is_none());
    assert!(v2.get("activityNodes").is_none());
    assert!(v2.get("flowEdges").is_none());
}

#[test]
fn sequence_doc_json_matches_ts_field_names() {
    let b = vec![
        ("s/buyer.md".to_string(), "---\ntype: uml.Class\ntitle: Buyer\n---\n# Buyer\n".to_string()),
        ("s/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
        ("s/use.md".to_string(), "---\ntype: uml.Sequence\ntitle: Use\n---\n# Use\n".to_string()),
        ("s/seq.md".to_string(),
         "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n## Lifelines\n- [Buyer](./buyer.md) as buyer\n- [Order](./order.md) as order\n\n## Gates\n- request\n\n## Messages\n- buyer calls order `submit()` as submission\n- order returns `accepted` for submission\n- par\n  - branch `left`\n    - buyer signals order `go`\n  - branch `right`\n    - order signals buyer `done`\n- ref [Use](./use.md) as auth\n".to_string()),
    ];
    let m = projection(&b);
    let v = serde_json::to_value(&m).unwrap();
    let s = &v["interactions"][0];
    assert_eq!(s["nodes"][0]["node"], "lifeline");
    assert_eq!(s["nodes"][0]["id"], "buyer");
    assert_eq!(s["nodes"][0]["ref"], "s/buyer");
    assert_eq!(s["nodes"][0]["alias"], "buyer");
    assert_eq!(s["edges"][0]["id"], "m0");
    assert_eq!(s["edges"][0]["kind"], "syncCall");
    assert_eq!(
        s["edges"][0]["from"],
        serde_json::json!({"endpoint":"lifeline","id":"buyer"})
    );
    assert_eq!(s["edges"][0]["callId"], "submission");
    assert_eq!(s["edges"][1]["kind"], "reply");
    assert_eq!(s["edges"][1]["returnsCall"], "m0");
    assert_eq!(s["gates"], serde_json::json!(["request"]));
    assert_eq!(s["interactionUses"][0]["alias"], "auth");

    let endpoints = [
        EndpointRef::Outside,
        EndpointRef::LocalGate {
            gate: "local".into(),
        },
        EndpointRef::UseGate {
            interaction_use: InteractionUseId("u0".into()),
            gate: "remote".into(),
        },
    ];
    assert_eq!(
        serde_json::from_value::<Vec<EndpointRef>>(serde_json::to_value(&endpoints).unwrap())
            .unwrap(),
        endpoints
    );
    for kind in [
        MessageKind::SyncCall,
        MessageKind::AsyncCall,
        MessageKind::AsyncSignal,
        MessageKind::Reply,
        MessageKind::Create,
        MessageKind::Delete,
    ] {
        assert_eq!(
            serde_json::from_value::<MessageKind>(serde_json::to_value(kind).unwrap()).unwrap(),
            kind
        );
    }
    for kind in [
        FragmentKind::Alt,
        FragmentKind::Opt,
        FragmentKind::Loop,
        FragmentKind::Par,
        FragmentKind::Break,
        FragmentKind::Critical,
        FragmentKind::Assert,
        FragmentKind::Neg,
    ] {
        assert_eq!(
            serde_json::from_value::<FragmentKind>(serde_json::to_value(kind).unwrap()).unwrap(),
            kind
        );
    }
    for spec in [
        OperandSpec::Guard("ready".into()),
        OperandSpec::Else,
        OperandSpec::Branch {
            label: Some("a".into()),
        },
    ] {
        assert_eq!(
            serde_json::from_value::<OperandSpec>(serde_json::to_value(&spec).unwrap()).unwrap(),
            spec
        );
    }

    let direct = SequenceDoc {
        key: "direct".into(),
        title: "Direct".into(),
        describes: None,
        nodes: vec![SeqNode::Operand {
            id: "o0".into(),
            spec: OperandSpec::Branch { label: None },
            items: vec![SeqChild::InteractionUse {
                interaction_use: InteractionUseId("u0".into()),
            }],
        }],
        edges: vec![SeqEdge {
            id: MessageId("m0".into()),
            from: EndpointRef::Outside,
            kind: MessageKind::AsyncSignal,
            to: Some(EndpointRef::LocalGate {
                gate: "local".into(),
            }),
            value: Some("payload".into()),
            call_id: None,
            returns_call: None,
        }],
        gates: vec!["local".into()],
        interaction_uses: vec![SeqInteractionUse {
            id: InteractionUseId("u0".into()),
            target: "target".into(),
            alias: "target_use".into(),
            bindings: vec![SeqBinding {
                local: "a".into(),
                target: "b".into(),
            }],
            gates: vec!["remote".into()],
        }],
        items: vec![],
    };
    assert_eq!(
        serde_json::from_value::<SequenceDoc>(serde_json::to_value(&direct).unwrap()).unwrap(),
        direct
    );
}

#[test]
fn diagnostic_serializes_with_kebab_code_and_lowercase_severity() {
    let d = Diagnostic::new(DiagCode::UnresolvedTarget, "gone", "a.md", 3);
    let v = serde_json::to_value(&d).unwrap();
    assert_eq!(v["severity"], "error");
    assert_eq!(v["code"], "unresolved-target");
    assert_eq!(v["message"], "gone");
    assert_eq!(v["file"], "a.md");
    assert_eq!(v["line"], 3);
    // Severity round-trips as its lowercase string.
    assert_eq!(
        serde_json::to_value(Severity::Warning).unwrap(),
        serde_json::json!("warning")
    );
}

#[test]
fn classifier_type_wire_strings_are_stable() {
    assert_eq!(
        serde_json::to_string(&ElementType::Uml(UmlMetaclass::Class)).unwrap(),
        "\"uml.Class\""
    );
    assert_eq!(
        serde_json::to_string(&ElementType::Behavior(BehaviorKind::Activity)).unwrap(),
        "\"uml.Activity\""
    );
    assert_eq!(
        serde_json::to_string(&ElementType::Diagram).unwrap(),
        "\"Diagram\""
    );
    assert_eq!(
        serde_json::to_string(&ElementType::Unknown("bpmn.Task".to_string())).unwrap(),
        "\"bpmn.Task\""
    );
    // Deserialize round-trips through `From<String>`.
    let ct: ElementType = serde_json::from_str("\"uml.Class\"").unwrap();
    assert_eq!(ct, ElementType::Uml(UmlMetaclass::Class));
}

#[test]
fn instance_doc_slots_shape_and_ref_resolution() {
    let b = vec![
        ("m/ann.md".into(), "---\ntype: uml.Class\ntitle: Ann\n---\n# Ann\n".into()),
        ("m/order42.md".into(),
         "---\ntype: uml.InstanceSpecification\ntitle: order42\n---\n# order42\n\n## Slots\n- id: \"ORD-42\"\n- status: PLACED\n- owner: [Ann](./ann.md)\n".into()),
    ];
    let m = projection(&b);
    let v = serde_json::to_value(&m).unwrap();
    let inst = v["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["key"] == "m/order42")
        .unwrap();
    assert_eq!(inst["type"], "uml.InstanceSpecification");
    assert_eq!(inst["slots"][0]["name"], "id");
    assert_eq!(inst["slots"][0]["value"], "ORD-42");
    assert!(inst["slots"][0].get("ref").is_none());
    assert_eq!(inst["slots"][2]["name"], "owner");
    assert_eq!(inst["slots"][2]["value"], "Ann");
    assert_eq!(
        inst["slots"][2]["ref"], "m/ann",
        "link-valued slot resolves to a pool key"
    );
}

#[test]
fn inline_instance_is_promoted_to_a_pool_node_with_edge_and_membership() {
    let b = vec![
        ("m/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
        ("m/objects.md".into(),
         "---\ntype: Diagram\ntitle: Objects\nprofile: uml-domain\n---\n# Objects\n\n## Members\n- [Order](./order.md)\n- instance of [Order](./order.md) as order42 with id set to \"ORD-42\" and status set to PLACED\n".into()),
    ];
    let m = projection(&b);
    let v = serde_json::to_value(&m).unwrap();
    // Promoted pool node keyed {diagram}#name.
    let inst = v["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["key"] == "m/objects#order42")
        .unwrap();
    assert_eq!(inst["type"], "uml.InstanceSpecification");
    assert_eq!(inst["slots"][0]["value"], "ORD-42");
    assert_eq!(inst["slots"][1]["value"], "PLACED");
    // InstanceOf edge to the classifier.
    let io = v["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "instanceof" && e["from"] == "m/objects#order42")
        .unwrap();
    assert_eq!(io["to"], "m/order");
    // Auto-added to the diagram's members.
    let members = &v["diagrams"][0]["groups"][0]["members"];
    assert!(members
        .as_array()
        .unwrap()
        .iter()
        .any(|k| k == "m/objects#order42"));
}
