use serde_json::{Value, json};

use super::*;

/// The bump the change from `before` to `after` needs.
fn bump(before: &Value, after: &Value) -> Bump {
    required(&changes(before, after))
}

/// Each change as `at nature bump`.
fn found(before: &Value, after: &Value) -> Vec<String> {
    changes(before, after)
        .into_iter()
        .map(|change| format!("{} {:?} {}", change.at, change.nature, change.bump))
        .collect()
}

fn form(items: Value) -> Value {
    json!({
        "resourceType": "Questionnaire",
        "url": "https://x/Questionnaire/anc",
        "version": "1.0.0",
        "title": "ANC visit",
        "item": items,
    })
}

fn bp() -> Value {
    json!({
        "linkId": "bp",
        "text": "Blood pressure",
        "type": "group",
        "item": [
            {"linkId": "bp-sys", "text": "Systolic", "type": "integer"},
            {"linkId": "bp-dia", "text": "Diastolic", "type": "integer", "required": true}
        ]
    })
}

/// `value` with `new` at `pointer`: replacing, inserting a key, or appending at the next index.
fn with(mut value: Value, pointer: &str, new: Value) -> Value {
    let (parent, name) = pointer.rsplit_once('/').expect("pointer");
    match value.pointer_mut(parent).expect(parent) {
        Value::Object(map) => {
            map.insert(name.to_owned(), new);
        }
        Value::Array(list) => {
            let index: usize = name.parse().expect("index");
            if index == list.len() {
                list.push(new);
            } else {
                list[index] = new;
            }
        }
        _ => panic!("{pointer}"),
    }
    value
}

fn without(mut value: Value, pointer: &str) -> Value {
    let (parent, name) = pointer.rsplit_once('/').expect("pointer");
    let parent = value.pointer_mut(parent).expect(parent);
    match parent {
        Value::Object(map) => {
            map.remove(name);
        }
        Value::Array(list) => {
            list.remove(name.parse().expect("index"));
        }
        _ => panic!("{pointer}"),
    }
    value
}

#[test]
fn release_metadata_needs_nothing_and_descriptive_metadata_a_patch() {
    let before = form(json!([bp()]));
    let after = with(before.clone(), "/version", json!("1.0.1"));
    let after = with(
        after,
        "/meta",
        json!({"lastUpdated": "2026-11-04T00:00:00Z"}),
    );
    assert_eq!(bump(&before, &after), Bump::None);

    let after = with(before.clone(), "/title", json!("Antenatal visit"));
    assert_eq!(found(&before, &after), ["title Reworded patch"]);

    let after = with(
        before.clone(),
        "/url",
        json!("https://x/Questionnaire/other"),
    );
    assert_eq!(bump(&before, &after), Bump::Major);

    let other = json!({"resourceType": "Library", "url": "https://x/Questionnaire/anc"});
    assert_eq!(found(&before, &other), ["resourceType Changed major"]);
}

#[test]
fn questionnaire_items_follow_the_bump_table() {
    let before = form(json!([bp()]));
    let cases = [
        // Display and help text.
        (
            with(
                before.clone(),
                "/item/0/item/0/text",
                json!("Systolic (mmHg)"),
            ),
            "item[bp].item[bp-sys].text Reworded patch",
        ),
        (
            with(before.clone(), "/item/0/item/0/prefix", json!("1a")),
            "item[bp].item[bp-sys].prefix Added patch",
        ),
        // New optional item, and a new required one.
        (
            with(
                before.clone(),
                "/item/0/item/2",
                json!({"linkId": "bp-arm", "type": "string"}),
            ),
            "item[bp].item[bp-arm] Added minor",
        ),
        (
            with(
                before.clone(),
                "/item/1",
                json!({"linkId": "pulse", "type": "integer", "required": true}),
            ),
            "item[pulse] Added major",
        ),
        // Removed item, changed answer type, changed extraction target.
        (
            without(before.clone(), "/item/0/item/1"),
            "item[bp].item[bp-dia] Removed major",
        ),
        (
            with(before.clone(), "/item/0/item/0/type", json!("decimal")),
            "item[bp].item[bp-sys].type Changed major",
        ),
        (
            with(
                before.clone(),
                "/item/0/item/0/definition",
                json!("http://hl7.org/fhir/StructureDefinition/Observation#Observation.value[x]"),
            ),
            "item[bp].item[bp-sys].definition Added major",
        ),
        // Tightened and relaxed constraints.
        (
            with(before.clone(), "/item/0/item/0/required", json!(true)),
            "item[bp].item[bp-sys].required Tightened major",
        ),
        (
            with(before.clone(), "/item/0/item/1/required", json!(false)),
            "item[bp].item[bp-dia].required Relaxed minor",
        ),
        (
            with(before.clone(), "/item/0/item/0/repeats", json!(true)),
            "item[bp].item[bp-sys].repeats Relaxed minor",
        ),
        (
            with(before.clone(), "/item/0/item/0/readOnly", json!(true)),
            "item[bp].item[bp-sys].readOnly Tightened major",
        ),
        (
            with(before.clone(), "/item/0/item/0/maxLength", json!(3)),
            "item[bp].item[bp-sys].maxLength Tightened major",
        ),
        (
            with(
                before.clone(),
                "/item/0/item/0/initial",
                json!([{"valueInteger": 120}]),
            ),
            "item[bp].item[bp-sys].initial Added minor",
        ),
        (
            with(
                before.clone(),
                "/item/0/item/0/enableWhen",
                json!([{"question": "x", "operator": "exists", "answerBoolean": true}]),
            ),
            "item[bp].item[bp-sys].enableWhen Added major",
        ),
    ];
    for (after, expected) in cases {
        assert_eq!(found(&before, &after), [expected]);
    }
}

#[test]
fn answer_options_and_extensions() {
    let item = |options: Value| {
        form(json!([{"linkId": "hiv", "type": "coding", "answerOption": options}]))
    };
    let pos = json!({"valueCoding": {"system": "s", "code": "pos"}});
    let neg = json!({"valueCoding": {"system": "s", "code": "neg"}});
    let before = item(json!([pos, neg]));
    assert_eq!(bump(&before, &item(json!([pos]))), Bump::Major);
    let ind = json!({"valueCoding": {"system": "s", "code": "ind"}});
    assert_eq!(bump(&before, &item(json!([pos, neg, ind]))), Bump::Minor);
    let selected = json!({"valueCoding": {"system": "s", "code": "neg"}, "initialSelected": true});
    assert_eq!(bump(&before, &item(json!([pos, selected]))), Bump::Patch);

    let control = json!([{
        "url": "http://hl7.org/fhir/StructureDefinition/questionnaire-itemControl",
        "valueCodeableConcept": {"text": "radio-button"}
    }]);
    let shown = with(before.clone(), "/item/0/extension", control);
    assert_eq!(bump(&before, &shown), Bump::Patch);
    let calculated = json!([{
        "url": "http://hl7.org/fhir/uv/sdc/StructureDefinition/sdc-questionnaire-calculatedExpression",
        "valueExpression": {"language": "text/fhirpath", "expression": "1"}
    }]);
    let computed = with(before.clone(), "/item/0/extension", calculated);
    assert_eq!(bump(&before, &computed), Bump::Major);

    // Items without linkIds cannot be matched, so any change is major.
    let loose = form(json!([{"text": "a", "type": "display"}]));
    let reworded = form(json!([{"text": "b", "type": "display"}]));
    assert_eq!(bump(&loose, &reworded), Bump::Major);
}

fn code_system(concepts: Value) -> Value {
    json!({
        "resourceType": "CodeSystem",
        "url": "https://x/CodeSystem/signs",
        "version": "1.0.0",
        "content": "complete",
        "count": 3,
        "property": [{"code": "inactive", "type": "boolean"}],
        "concept": concepts,
    })
}

#[test]
fn code_system_concepts() {
    let before = code_system(json!([
        {"code": "danger", "display": "Danger signs", "concept": [
            {"code": "bleeding", "display": "Vaginal bleeding"},
            {"code": "fits", "display": "Convulsions"}
        ]}
    ]));
    let cases = [
        (
            without(before.clone(), "/concept/0/concept/1"),
            "concept[fits] Removed major",
        ),
        (
            with(
                before.clone(),
                "/concept/0/concept/2",
                json!({"code": "fever", "display": "Fever"}),
            ),
            "concept[fever] Added minor",
        ),
        (
            with(
                before.clone(),
                "/concept/0/concept/0/display",
                json!("Bleeding"),
            ),
            "concept[bleeding].display Reworded patch",
        ),
        (
            with(
                before.clone(),
                "/concept/0/concept/0/property",
                json!([{"code": "inactive", "valueBoolean": true}]),
            ),
            "concept[bleeding].property Added minor",
        ),
        (with(before.clone(), "/count", json!(4)), ""),
        (
            with(
                before.clone(),
                "/property/1",
                json!({"code": "notSelectable", "type": "boolean"}),
            ),
            "property[notSelectable] Added minor",
        ),
        (
            with(before.clone(), "/property", json!([])),
            "property[inactive] Removed major",
        ),
        (
            with(before.clone(), "/content", json!("fragment")),
            "content Changed major",
        ),
    ];
    for (after, expected) in cases {
        let expected: Vec<&str> = if expected.is_empty() {
            vec![]
        } else {
            vec![expected]
        };
        assert_eq!(found(&before, &after), expected);
    }

    // Moving a code under another parent changes what it subsumes.
    let moved = code_system(json!([
        {"code": "danger", "display": "Danger signs", "concept": [{"code": "fits", "display": "Convulsions"}]},
        {"code": "bleeding", "display": "Vaginal bleeding"}
    ]));
    assert_eq!(
        found(&before, &moved),
        ["concept[bleeding].parent Changed major"]
    );
}

fn value_set(compose: Value) -> Value {
    json!({
        "resourceType": "ValueSet",
        "url": "https://x/ValueSet/signs",
        "version": "1.0.0",
        "compose": compose,
    })
}

#[test]
fn value_set_compose() {
    let listed = |codes: &[&str]| {
        let concept: Vec<Value> = codes.iter().map(|code| json!({"code": code})).collect();
        json!({"system": "https://x/CodeSystem/signs", "concept": concept})
    };
    let before = value_set(json!({"include": [listed(&["bleeding", "fits"])]}));
    let cases = [
        (
            value_set(json!({"include": [listed(&["bleeding", "fits", "fever"])]})),
            Bump::Minor,
        ),
        (
            value_set(json!({"include": [listed(&["bleeding"])]})),
            Bump::Major,
        ),
        (
            value_set(json!({"include": [{"system": "https://x/CodeSystem/signs"}]})),
            Bump::Minor,
        ),
        (
            value_set(
                json!({"include": [listed(&["bleeding", "fits"]), {"system": "http://loinc.org"}]}),
            ),
            Bump::Minor,
        ),
        (value_set(json!({"include": []})), Bump::Major),
        (
            value_set(
                json!({"include": [listed(&["bleeding", "fits"])], "exclude": [listed(&["fits"])]}),
            ),
            Bump::Major,
        ),
        (
            value_set(
                json!({"include": [listed(&["bleeding", "fits"])], "lockedDate": "2026-01-01"}),
            ),
            Bump::Major,
        ),
        (
            with(
                before.clone(),
                "/compose/include/0/concept/0/display",
                json!("Bleeding"),
            ),
            Bump::Patch,
        ),
        (
            with(
                before.clone(),
                "/expansion",
                json!({"contains": [{"code": "x"}]}),
            ),
            Bump::None,
        ),
    ];
    for (after, expected) in cases {
        assert_eq!(bump(&before, &after), expected, "{after}");
    }

    let excluding = value_set(
        json!({"include": [listed(&["bleeding", "fits"])], "exclude": [listed(&["fits"])]}),
    );
    assert_eq!(bump(&excluding, &before), Bump::Minor);

    let inactive =
        |flag: bool| value_set(json!({"include": [listed(&["fits"])], "inactive": flag}));
    assert_eq!(bump(&inactive(false), &inactive(true)), Bump::Minor);
    assert_eq!(bump(&inactive(true), &inactive(false)), Bump::Major);
}

fn profile(elements: Value) -> Value {
    json!({
        "resourceType": "StructureDefinition",
        "url": "https://x/StructureDefinition/anc-patient",
        "version": "1.0.0",
        "type": "Patient",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Patient",
        "derivation": "constraint",
        "differential": {"element": elements},
    })
}

fn element(extra: Value) -> Value {
    let mut element = json!({"id": "Patient.gender", "path": "Patient.gender"});
    if let (Some(map), Some(extra)) = (element.as_object_mut(), extra.as_object()) {
        map.extend(extra.clone());
    }
    element
}

#[test]
fn profile_elements() {
    let base = element(json!({
        "short": "Gender",
        "min": 0,
        "max": "1",
        "binding": {"strength": "extensible", "valueSet": "https://x/ValueSet/gender"},
        "constraint": [{"key": "anc-1", "severity": "error", "human": "Known", "expression": "exists()"}]
    }));
    let before = profile(json!([base]));
    let change = |pointer: &str, value: Value| {
        with(
            before.clone(),
            &format!("/differential/element/0{pointer}"),
            value,
        )
    };
    let drop =
        |pointer: &str| without(before.clone(), &format!("/differential/element/0{pointer}"));
    let at = "element[Patient.gender]";
    let cases = [
        (
            change("/short", json!("Administrative gender")),
            format!("{at}.short Reworded patch"),
        ),
        (
            change("/min", json!(1)),
            format!("{at}.min Tightened major"),
        ),
        (
            change("/max", json!("*")),
            format!("{at}.max Relaxed minor"),
        ),
        (
            change("/max", json!("0")),
            format!("{at}.max Tightened major"),
        ),
        (
            change("/binding/strength", json!("required")),
            format!("{at}.binding.strength Tightened major"),
        ),
        (
            change("/binding/strength", json!("preferred")),
            format!("{at}.binding.strength Relaxed minor"),
        ),
        (
            change("/binding/valueSet", json!("https://x/ValueSet/other")),
            format!("{at}.binding.valueSet Changed major"),
        ),
        (drop("/binding"), format!("{at}.binding Relaxed minor")),
        (
            change("/mustSupport", json!(true)),
            format!("{at}.mustSupport Tightened major"),
        ),
        (
            change("/fixedCode", json!("female")),
            format!("{at}.fixedCode Tightened major"),
        ),
        (
            change("/constraint/0/human", json!("Must be known")),
            format!("{at}.constraint[anc-1].human Reworded patch"),
        ),
        (
            change(
                "/constraint/0/expression",
                json!("exists() and $this != 'unknown'"),
            ),
            format!("{at}.constraint[anc-1].expression Changed major"),
        ),
        (
            change("/constraint", json!([])),
            format!("{at}.constraint[anc-1] Removed minor"),
        ),
    ];
    for (after, expected) in cases {
        assert_eq!(found(&before, &after), [expected.as_str()]);
    }

    // Types and target profiles list what is allowed.
    let typed = |targets: Value| {
        profile(json!([{
            "id": "Patient.generalPractitioner",
            "path": "Patient.generalPractitioner",
            "type": [{"code": "Reference", "targetProfile": targets}]
        }]))
    };
    let practitioner = "http://hl7.org/fhir/StructureDefinition/Practitioner";
    let organization = "http://hl7.org/fhir/StructureDefinition/Organization";
    assert_eq!(
        bump(
            &typed(json!([practitioner])),
            &typed(json!([practitioner, organization]))
        ),
        Bump::Minor
    );
    assert_eq!(
        bump(
            &typed(json!([practitioner, organization])),
            &typed(json!([practitioner]))
        ),
        Bump::Major
    );

    // An element added to the differential constrains; one removed from it relaxes.
    let other = element(json!({"id": "Patient.birthDate", "path": "Patient.birthDate", "min": 1}));
    let added = with(before.clone(), "/differential/element/1", other);
    assert_eq!(
        found(&before, &added),
        ["element[Patient.birthDate].min Tightened major"]
    );
    assert_eq!(bump(&added, &before), Bump::Minor);
    let documented =
        element(json!({"id": "Patient.name", "path": "Patient.name", "short": "Name"}));
    let documented = with(before.clone(), "/differential/element/1", documented);
    assert_eq!(bump(&before, &documented), Bump::Patch);

    // Without differentials, snapshots are compared, and a removed element is major.
    let snapshot = |elements: Value| json!({"resourceType": "StructureDefinition", "snapshot": {"element": elements}});
    let two = snapshot(
        json!([{"id": "Patient", "path": "Patient"}, {"id": "Patient.gender", "path": "Patient.gender"}]),
    );
    let one = snapshot(json!([{"id": "Patient", "path": "Patient"}]));
    assert_eq!(found(&two, &one), ["element[Patient.gender] Removed major"]);

    let rebased = with(
        before.clone(),
        "/baseDefinition",
        json!("https://x/StructureDefinition/other"),
    );
    assert_eq!(found(&before, &rebased), ["baseDefinition Changed major"]);
}

#[test]
fn other_resource_types_change_majorly() {
    let library = |data: &str| {
        json!({"resourceType": "Library", "url": "https://x/Library/map", "version": "1.0.0",
               "description": "Mapping", "content": [{"contentType": "application/json", "data": data}]})
    };
    assert_eq!(bump(&library("e30="), &library("e30=")), Bump::None);
    assert_eq!(
        found(&library("e30="), &library("e3g=")),
        ["content Changed major"]
    );
    let described = with(library("e30="), "/description", json!("The mapping"));
    assert_eq!(bump(&library("e30="), &described), Bump::Patch);
    assert_eq!(bump(&json!(1), &json!(2)), Bump::Major);
}

#[test]
fn changes_serialise() {
    let before = form(json!([bp()]));
    let after = without(before.clone(), "/item/0");
    let json = serde_json::to_value(changes(&before, &after)).expect("json");
    assert_eq!(
        json,
        json!([{"at": "item[bp]", "nature": "removed", "bump": "major"}])
    );
}
