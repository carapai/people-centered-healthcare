use super::*;

const SIGNS: &str = "https://health.go.ug/fhir/CodeSystem/anc-signs";
const ICD: &str = "http://hl7.org/fhir/sid/icd-10";
const DANGER: &str = "https://health.go.ug/fhir/ValueSet/danger-signs";

fn concept(code: &str, display: &str, children: Vec<Concept>) -> Concept {
    Concept {
        code: code.into(),
        display: Some(display.into()),
        children,
        ..Concept::default()
    }
}

fn inactive(mut concept: Concept) -> Concept {
    concept.properties.push(Property {
        code: "inactive".into(),
        value: PropertyValue::Boolean(true),
    });
    concept
}

/// danger ⊃ {bleeding ⊃ {heavy-bleeding}, fits, fever (inactive)}; routine.
fn signs(version: &str) -> CodeSystem {
    let mut fits = concept("fits", "Convulsions", vec![]);
    fits.designations.push(Designation {
        language: Some("lg".into()),
        use_code: None,
        value: "Okukonziba".into(),
    });
    fits.properties.push(Property {
        code: "severity".into(),
        value: PropertyValue::Code("high".into()),
    });
    CodeSystem {
        url: SIGNS.into(),
        version: Some(version.into()),
        case_sensitive: true,
        content: Content::Complete,
        concepts: vec![
            concept(
                "danger",
                "Danger signs",
                vec![
                    concept(
                        "bleeding",
                        "Vaginal bleeding",
                        vec![concept("heavy-bleeding", "Heavy bleeding", vec![])],
                    ),
                    fits,
                    inactive(concept("fever", "Fever", vec![])),
                ],
            ),
            concept("routine", "Routine visit", vec![]),
        ],
    }
}

fn rule(system: &str) -> Rule {
    Rule {
        system: Some(system.into()),
        ..Rule::default()
    }
}

fn filter(property: &str, op: FilterOp, value: &str) -> Filter {
    Filter {
        property: property.into(),
        op,
        value: value.into(),
    }
}

fn value_set(url: &str, include: Vec<Rule>, exclude: Vec<Rule>) -> ValueSet {
    ValueSet {
        url: url.into(),
        version: Some("1.0.0".into()),
        include,
        exclude,
        inactive: None,
        expansion: None,
    }
}

fn terminology() -> Terminology {
    let mut terminology = Terminology::new();
    terminology.add_code_system(signs("1.0.0"));
    terminology.add_value_set(value_set(
        DANGER,
        vec![Rule {
            filters: vec![filter("concept", FilterOp::DescendentOf, "danger")],
            ..rule(SIGNS)
        }],
        vec![],
    ));
    terminology
}

fn codes(expansion: &Expansion) -> Vec<&str> {
    expansion.contains.iter().map(|c| c.code.as_str()).collect()
}

#[test]
fn lookup_gives_display_designations_properties_and_neighbours() {
    let found = terminology()
        .lookup(SIGNS, None, "bleeding")
        .expect("lookup");
    assert_eq!(found.concept.display.as_deref(), Some("Vaginal bleeding"));
    assert_eq!(found.parents, ["danger"]);
    assert_eq!(found.children, ["heavy-bleeding"]);
    assert!(
        found.concept.children.is_empty(),
        "the children are listed, not nested"
    );

    let fits = terminology()
        .lookup(SIGNS, Some("1.0.0"), "fits")
        .expect("lookup");
    assert_eq!(fits.concept.designations[0].value, "Okukonziba");
    assert_eq!(fits.concept.properties[0].code, "severity");

    assert_eq!(
        terminology().lookup(SIGNS, None, "nausea"),
        Err(Error::UnknownCode {
            system: SIGNS.into(),
            code: "nausea".into()
        })
    );
    assert_eq!(
        terminology().lookup(ICD, None, "A00"),
        Err(Error::UnknownCodeSystem(ICD.into()))
    );
    assert_eq!(
        terminology().lookup(SIGNS, Some("9.9.9"), "fits"),
        Err(Error::UnknownCodeSystem(format!("{SIGNS}|9.9.9")))
    );
}

#[test]
fn subsumption_follows_the_hierarchy() {
    let terminology = terminology();
    let subsumes = |a, b| terminology.subsumes(SIGNS, None, a, b).expect("subsumes");
    assert_eq!(subsumes("danger", "heavy-bleeding"), Subsumption::Subsumes);
    assert_eq!(
        subsumes("heavy-bleeding", "danger"),
        Subsumption::SubsumedBy
    );
    assert_eq!(subsumes("fits", "fits"), Subsumption::Equivalent);
    assert_eq!(subsumes("fits", "bleeding"), Subsumption::NotSubsumed);
    assert!(terminology.subsumes(SIGNS, None, "fits", "nausea").is_err());
}

#[test]
fn a_parent_property_states_the_hierarchy_of_a_flat_code_system() {
    let mut flat = CodeSystem {
        url: "urn:flat".into(),
        version: None,
        case_sensitive: false,
        content: Content::Complete,
        concepts: vec![
            concept("Top", "Top", vec![]),
            concept("Leaf", "Leaf", vec![]),
        ],
    };
    flat.concepts[1].properties.push(Property {
        code: "parent".into(),
        value: PropertyValue::Code("top".into()),
    });
    let mut terminology = Terminology::new();
    terminology.add_code_system(flat);
    assert_eq!(
        terminology.subsumes("urn:flat", None, "TOP", "leaf"),
        Ok(Subsumption::Subsumes),
        "case-insensitive codes compare without case"
    );
}

#[test]
fn expansion_computes_the_compose_from_local_code_systems() {
    let terminology = terminology();
    let all = terminology
        .expand(DANGER, &ExpandRequest::default())
        .expect("expand");
    assert_eq!(all.value_set, format!("{DANGER}|1.0.0"));
    assert!(!all.pinned);
    assert_eq!(codes(&all), ["bleeding", "fever", "fits", "heavy-bleeding"]);
    assert!(all.contains.iter().any(|c| c.code == "fever" && c.inactive));

    let active = terminology
        .expand(
            DANGER,
            &ExpandRequest {
                active_only: true,
                ..ExpandRequest::default()
            },
        )
        .expect("expand");
    assert_eq!(codes(&active), ["bleeding", "fits", "heavy-bleeding"]);

    let page = terminology
        .expand(
            DANGER,
            &ExpandRequest {
                filter: Some("BLEED".into()),
                offset: 1,
                count: Some(5),
                ..ExpandRequest::default()
            },
        )
        .expect("expand");
    assert_eq!(page.total, 2);
    assert_eq!(codes(&page), ["heavy-bleeding"]);
}

#[test]
fn each_filter_selects_what_it_says() {
    let cases = [
        (
            filter("concept", FilterOp::IsA, "bleeding"),
            vec!["bleeding", "heavy-bleeding"],
        ),
        (
            filter("concept", FilterOp::ChildOf, "danger"),
            vec!["bleeding", "fever", "fits"],
        ),
        (
            filter("concept", FilterOp::IsNotA, "danger"),
            vec!["routine"],
        ),
        (
            filter("concept", FilterOp::Generalizes, "heavy-bleeding"),
            vec!["bleeding", "danger", "heavy-bleeding"],
        ),
        (
            filter("concept", FilterOp::In, "fits, routine"),
            vec!["fits", "routine"],
        ),
        (filter("severity", FilterOp::Equals, "high"), vec!["fits"]),
        (filter("inactive", FilterOp::Exists, "true"), vec!["fever"]),
        (
            filter("severity", FilterOp::NotIn, "high"),
            vec!["bleeding", "danger", "fever", "heavy-bleeding", "routine"],
        ),
    ];
    for (f, expected) in cases {
        let mut terminology = terminology();
        terminology.add_value_set(value_set(
            "urn:vs",
            vec![Rule {
                filters: vec![f.clone()],
                ..rule(SIGNS)
            }],
            vec![],
        ));
        let found = terminology
            .expand("urn:vs", &ExpandRequest::default())
            .expect("expand");
        assert_eq!(codes(&found), expected, "{f:?}");
    }

    let mut terminology = terminology();
    terminology.add_value_set(value_set(
        "urn:regex",
        vec![Rule {
            filters: vec![filter("code", FilterOp::Regex, "^f")],
            ..rule(SIGNS)
        }],
        vec![],
    ));
    assert!(matches!(
        terminology.expand("urn:regex", &ExpandRequest::default()),
        Err(Error::UnsupportedFilter { .. })
    ));
}

#[test]
fn listed_codes_exclusions_and_imports() {
    let mut terminology = terminology();
    terminology.add_value_set(value_set(
        "urn:listed",
        vec![Rule {
            concepts: vec![
                Listed {
                    code: "fits".into(),
                    display: Some("Fits".into()),
                },
                Listed {
                    code: "routine".into(),
                    display: None,
                },
            ],
            ..rule(SIGNS)
        }],
        vec![],
    ));
    let listed = terminology
        .expand("urn:listed", &ExpandRequest::default())
        .expect("expand");
    assert_eq!(codes(&listed), ["fits", "routine"]);
    assert_eq!(
        listed.contains[0].display.as_deref(),
        Some("Fits"),
        "the value set's display"
    );

    // Danger signs, but not those in the listed set: imports intersect, exclusions remove.
    terminology.add_value_set(value_set(
        "urn:excluded",
        vec![Rule {
            value_sets: vec![DANGER.into()],
            ..Rule::default()
        }],
        vec![Rule {
            value_sets: vec!["urn:listed".into()],
            ..Rule::default()
        }],
    ));
    let excluded = terminology
        .expand("urn:excluded", &ExpandRequest::default())
        .expect("expand");
    assert_eq!(codes(&excluded), ["bleeding", "fever", "heavy-bleeding"]);

    terminology.add_value_set(value_set(
        "urn:both",
        vec![Rule {
            value_sets: vec![DANGER.into(), "urn:listed".into()],
            ..Rule::default()
        }],
        vec![],
    ));
    let both = terminology
        .expand("urn:both", &ExpandRequest::default())
        .expect("expand");
    assert_eq!(codes(&both), ["fits"]);

    // A listed code the code system does not have is an error, not a silent omission.
    terminology.add_value_set(value_set(
        "urn:typo",
        vec![Rule {
            concepts: vec![Listed {
                code: "fitz".into(),
                display: None,
            }],
            ..rule(SIGNS)
        }],
        vec![],
    ));
    assert!(matches!(
        terminology.expand("urn:typo", &ExpandRequest::default()),
        Err(Error::UnknownCode { .. })
    ));
}

#[test]
fn nothing_is_fetched_from_elsewhere() {
    let mut terminology = terminology();
    // A whole code system that is not held cannot be expanded.
    terminology.add_value_set(value_set("urn:icd", vec![rule(ICD)], vec![]));
    assert_eq!(
        terminology.expand("urn:icd", &ExpandRequest::default()),
        Err(Error::UnknownCodeSystem(ICD.into()))
    );
    // Listed codes of a code system that is not held are taken as listed: a pinned subset.
    terminology.add_value_set(value_set(
        "urn:icd-subset",
        vec![Rule {
            concepts: vec![Listed {
                code: "O15".into(),
                display: Some("Eclampsia".into()),
            }],
            ..rule(ICD)
        }],
        vec![],
    ));
    let subset = terminology
        .expand("urn:icd-subset", &ExpandRequest::default())
        .expect("expand");
    assert_eq!(codes(&subset), ["O15"]);
    // A fragment cannot be filtered: codes could be missing.
    let mut fragment = signs("2.0.0");
    fragment.content = Content::Fragment;
    terminology.add_code_system(fragment);
    terminology.add_value_set(value_set(
        "urn:fragment",
        vec![Rule {
            version: Some("2.0.0".into()),
            ..rule(SIGNS)
        }],
        vec![],
    ));
    assert_eq!(
        terminology.expand("urn:fragment", &ExpandRequest::default()),
        Err(Error::Incomplete(SIGNS.into()))
    );
    assert_eq!(
        terminology.expand("urn:none", &ExpandRequest::default()),
        Err(Error::UnknownValueSet("urn:none".into()))
    );
}

#[test]
fn a_pinned_expansion_is_used_as_shipped() {
    let mut terminology = Terminology::new();
    let mut pinned = value_set("urn:pinned", vec![rule(ICD)], vec![]);
    pinned.expansion = Some(vec![Contains {
        system: ICD.into(),
        version: Some("2019".into()),
        code: "O15".into(),
        display: Some("Eclampsia".into()),
        inactive: false,
    }]);
    terminology.add_value_set(pinned);
    let expansion = terminology
        .expand("urn:pinned|1.0.0", &ExpandRequest::default())
        .expect("expand without ICD-10 held");
    assert!(expansion.pinned);
    assert_eq!(codes(&expansion), ["O15"]);
    assert_eq!(
        terminology.validate_code("urn:pinned", ICD, "O15", Some("Eclampsia")),
        Ok(Validation::Valid {
            display: Some("Eclampsia".into())
        })
    );
}

#[test]
fn validate_code_checks_membership_and_display() {
    let terminology = terminology();
    assert_eq!(
        terminology.validate_code(DANGER, SIGNS, "fits", None),
        Ok(Validation::Valid {
            display: Some("Convulsions".into())
        })
    );
    assert_eq!(
        terminology.validate_code(DANGER, SIGNS, "fits", Some("Okukonziba")),
        Ok(Validation::Valid {
            display: Some("Convulsions".into())
        }),
        "a designation is a valid display"
    );
    assert_eq!(
        terminology.validate_code(DANGER, SIGNS, "fits", Some("Seizure")),
        Ok(Validation::WrongDisplay {
            expected: Some("Convulsions".into())
        })
    );
    assert_eq!(
        terminology.validate_code(DANGER, SIGNS, "routine", None),
        Ok(Validation::Invalid)
    );
    assert_eq!(
        terminology.validate_code(DANGER, ICD, "fits", None),
        Ok(Validation::Invalid)
    );

    assert_eq!(
        terminology.validate_in_code_system(SIGNS, None, "routine", None),
        Ok(Validation::Valid {
            display: Some("Routine visit".into())
        })
    );
    assert_eq!(
        terminology.validate_in_code_system(SIGNS, None, "nausea", None),
        Ok(Validation::Invalid)
    );
}

#[test]
fn the_latest_version_is_used_unless_one_is_pinned() {
    let mut terminology = terminology();
    let mut next = signs("1.10.0");
    next.concepts[1].display = Some("Routine ANC visit".into());
    terminology.add_code_system(next);
    terminology.add_code_system(signs("1.9.0"));
    let latest = terminology.lookup(SIGNS, None, "routine").expect("lookup");
    assert_eq!(
        latest.version.as_deref(),
        Some("1.10.0"),
        "semver, not text order"
    );
    assert_eq!(latest.concept.display.as_deref(), Some("Routine ANC visit"));
    let pinned = terminology
        .lookup(SIGNS, Some("1.0.0"), "routine")
        .expect("lookup");
    assert_eq!(pinned.concept.display.as_deref(), Some("Routine visit"));
}

#[test]
fn a_value_set_including_itself_is_refused() {
    let mut terminology = Terminology::new();
    terminology.add_value_set(value_set(
        "urn:a",
        vec![Rule {
            value_sets: vec!["urn:b".into()],
            ..Rule::default()
        }],
        vec![],
    ));
    terminology.add_value_set(value_set(
        "urn:b",
        vec![Rule {
            value_sets: vec!["urn:a".into()],
            ..Rule::default()
        }],
        vec![],
    ));
    assert!(matches!(
        terminology.expand("urn:a", &ExpandRequest::default()),
        Err(Error::Cycle(_))
    ));
}

#[test]
fn translate_follows_concept_maps() {
    let mut terminology = terminology();
    terminology.add_concept_map(ConceptMap {
        url: "urn:map".into(),
        version: Some("1.0.0".into()),
        groups: vec![Group {
            source: SIGNS.into(),
            target: ICD.into(),
            mappings: vec![
                Mapping {
                    code: "fits".into(),
                    targets: vec![Target {
                        code: Some("O15".into()),
                        display: Some("Eclampsia".into()),
                        relationship: Relationship::SourceIsBroaderThanTarget,
                    }],
                },
                Mapping {
                    code: "routine".into(),
                    targets: vec![Target {
                        code: None,
                        display: None,
                        relationship: Relationship::NotRelatedTo,
                    }],
                },
            ],
        }],
    });
    let found = terminology
        .translate(Some("urn:map"), SIGNS, "fits", None)
        .expect("translate");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].concept_map, "urn:map|1.0.0");
    assert_eq!(found[0].system, ICD);
    assert_eq!(found[0].target.code.as_deref(), Some("O15"));
    assert_eq!(
        terminology
            .translate(None, SIGNS, "fits", Some(ICD))
            .expect("every map")
            .len(),
        1
    );
    assert!(
        terminology
            .translate(None, SIGNS, "fits", Some("urn:other"))
            .expect("translate")
            .is_empty()
    );
    let unmapped = terminology
        .translate(None, SIGNS, "routine", None)
        .expect("translate");
    assert_eq!(unmapped[0].target.relationship, Relationship::NotRelatedTo);
    assert_eq!(
        terminology.translate(Some("urn:none"), SIGNS, "fits", None),
        Err(Error::UnknownConceptMap("urn:none".into()))
    );
}

#[test]
fn canonicals_read_and_print() {
    let canonical = Canonical::parse("urn:vs|1.0.0");
    assert_eq!(canonical.version.as_deref(), Some("1.0.0"));
    assert_eq!(canonical.to_string(), "urn:vs|1.0.0");
    assert_eq!(Canonical::parse("urn:vs|").version, None);
    assert_eq!(Canonical::parse("urn:vs").to_string(), "urn:vs");
}

// The Studio's terminology browser (#523).

const ICD11: &str = "http://id.who.int/icd/release/11/mms";

/// A slice of ICD-11 big enough to page: chapter 01 with blocks of numbered codes, and chapter 12.
fn icd11() -> CodeSystem {
    let mut cholera = concept("1A00", "Cholera", vec![]);
    cholera.designations.push(Designation {
        language: Some("en".into()),
        use_code: Some("synonym".into()),
        value: "Asiatic cholera".into(),
    });
    let mut intestinal: Vec<Concept> = vec![cholera];
    for n in 1..=40 {
        intestinal.push(concept(
            &format!("1A{n:02}"),
            &format!("Intestinal infection type {n}"),
            vec![],
        ));
    }
    CodeSystem {
        url: ICD11.into(),
        version: Some("2024-01".into()),
        case_sensitive: false,
        content: Content::Complete,
        concepts: vec![
            concept(
                "01",
                "Certain infectious or parasitic diseases",
                vec![concept(
                    "1A0",
                    "Bacterial intestinal infections",
                    intestinal,
                )],
            ),
            concept(
                "12",
                "Diseases of the respiratory system",
                vec![concept("CA40", "Pneumonia", vec![])],
            ),
        ],
    }
}

#[test]
fn search_by_words_matches_code_display_and_designations_a_page_at_a_time() {
    let mut terminology = Terminology::new();
    terminology.add_code_system(icd11());

    let found = terminology
        .search(ICD11, None, "intestinal TYPE", 0, Some(10))
        .expect("search");
    assert_eq!(found.code_system, format!("{ICD11}|2024-01"));
    assert_eq!(found.total, 40, "every word must match, ignoring case");
    assert_eq!(found.concepts.len(), 10, "only one page is returned");
    assert_eq!(found.concepts[0].code, "1A01");

    let next = terminology
        .search(ICD11, None, "intestinal type", 10, Some(10))
        .expect("search");
    assert_eq!(next.concepts[0].code, "1A11");

    let synonym = terminology
        .search(ICD11, None, "asiatic", 0, None)
        .expect("search");
    assert_eq!(synonym.concepts[0].code, "1A00");

    // An exact code comes first, then codes starting with it.
    let by_code = terminology
        .search(ICD11, None, "1a0", 0, Some(3))
        .expect("search");
    let codes: Vec<&str> = by_code.concepts.iter().map(|c| c.code.as_str()).collect();
    assert_eq!(codes, ["1A0", "1A00", "1A01"]);
    assert_eq!(by_code.concepts[0].children, 41);

    assert_eq!(
        terminology.search("http://example.org/none", None, "x", 0, None),
        Err(Error::UnknownCodeSystem("http://example.org/none".into()))
    );
}

#[test]
fn the_hierarchy_is_read_one_level_at_a_time() {
    let mut terminology = Terminology::new();
    terminology.add_code_system(icd11());
    let summaries = terminology.code_systems();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].concepts, 45);

    let roots = terminology
        .children(ICD11, None, None, 0, None)
        .expect("roots");
    let codes: Vec<&str> = roots.concepts.iter().map(|c| c.code.as_str()).collect();
    assert_eq!(codes, ["01", "12"]);
    assert_eq!(roots.concepts[0].children, 1);

    let block = terminology
        .children(ICD11, None, Some("1a0"), 5, Some(5))
        .expect("children");
    assert_eq!(block.total, 41);
    assert_eq!(block.concepts.len(), 5);
    assert_eq!(block.concepts[0].code, "1A05");

    assert!(matches!(
        terminology.children(ICD11, None, Some("ZZ"), 0, None),
        Err(Error::UnknownCode { .. })
    ));
}

#[test]
fn an_is_a_rule_takes_in_a_child_code_added_later() {
    let rule = Rule {
        filters: vec![filter("concept", FilterOp::IsA, "bleeding")],
        ..rule(SIGNS)
    };
    let mut terminology = Terminology::new();
    terminology.add_code_system(signs("1.0.0"));
    terminology.add_value_set(value_set(DANGER, vec![rule], vec![]));
    let before = terminology
        .expand(DANGER, &ExpandRequest::default())
        .expect("expand");
    assert_eq!(codes(&before), ["bleeding", "heavy-bleeding"]);

    // The code system's next version adds a code under bleeding.
    let mut next = signs("1.1.0");
    next.concepts[0].children[0]
        .children
        .push(concept("spotting", "Spotting", vec![]));
    terminology.add_code_system(next);
    let after = terminology
        .expand(DANGER, &ExpandRequest::default())
        .expect("expand");
    assert_eq!(codes(&after), ["bleeding", "heavy-bleeding", "spotting"]);
}

#[test]
fn the_change_against_published_lists_added_and_removed_codes() {
    let mut terminology = terminology();
    let published = terminology
        .expand(DANGER, &ExpandRequest::default())
        .expect("expand")
        .contains;
    terminology.add_value_set(ValueSet {
        version: Some("1.1.0".into()),
        ..value_set(
            DANGER,
            vec![Rule {
                filters: vec![filter("concept", FilterOp::DescendentOf, "danger")],
                ..rule(SIGNS)
            }],
            vec![Rule {
                concepts: vec![Listed {
                    code: "fits".into(),
                    display: None,
                }],
                ..rule(SIGNS)
            }],
        )
    });
    terminology.add_value_set(value_set(
        "https://health.go.ug/fhir/ValueSet/other",
        vec![Rule {
            concepts: vec![Listed {
                code: "routine".into(),
                display: None,
            }],
            ..rule(SIGNS)
        }],
        vec![],
    ));
    let draft = terminology
        .expand(&format!("{DANGER}|1.1.0"), &ExpandRequest::default())
        .expect("expand")
        .contains;
    let change = compare(&published, &draft);
    assert!(change.added.is_empty());
    let removed: Vec<&str> = change.removed.iter().map(|c| c.code.as_str()).collect();
    assert_eq!(removed, ["fits"]);

    let routine = vec![Contains {
        system: SIGNS.into(),
        version: None,
        code: "routine".into(),
        display: None,
        inactive: false,
    }];
    let change = compare(&published, &[draft.clone(), routine].concat());
    assert_eq!(change.added[0].code, "routine");
    assert_eq!(terminology.value_set_references().len(), 3);

    let mut merged = Terminology::new();
    merged.extend(&terminology);
    assert_eq!(merged.value_set_references().len(), 3);
    assert_eq!(merged.code_systems().len(), 1);
}

// Editing a local CodeSystem (#524).

#[test]
fn a_retired_code_leaves_new_form_expansions_and_still_resolves() {
    let mut terminology = terminology();
    let mut next = signs("1.1.0");
    next.concepts[0].children[1].properties.push(Property {
        code: "status".into(),
        value: PropertyValue::Code("retired".into()),
    });
    terminology.add_code_system(next);
    let forms = ExpandRequest {
        active_only: true,
        ..ExpandRequest::default()
    };
    let offered = terminology.expand(DANGER, &forms).expect("expand");
    assert!(!codes(&offered).contains(&"fits"), "{:?}", codes(&offered));
    // Records that hold it still resolve against the code system.
    let found = terminology.lookup(SIGNS, None, "fits").expect("lookup");
    assert!(found.concept.is_inactive());
    assert_eq!(
        terminology
            .validate_in_code_system(SIGNS, None, "fits", None)
            .expect("validate"),
        Validation::Valid {
            display: Some("Convulsions".into())
        }
    );
}

#[test]
fn a_draft_code_system_is_swapped_for_its_published_one_and_value_sets_on_it_are_found() {
    let mut terminology = terminology();
    terminology.add_value_set(value_set(
        "https://health.go.ug/fhir/ValueSet/icd",
        vec![rule(ICD)],
        vec![],
    ));
    assert_eq!(
        terminology.value_sets_on(SIGNS),
        [format!("{DANGER}|1.0.0")]
    );
    let mut draft = signs("1.1.0");
    draft.concepts[1].code = "review".into();
    terminology.add_code_system(draft);
    assert!(terminology.lookup(SIGNS, None, "review").is_ok());
    terminology.remove_code_system(SIGNS, Some("1.1.0"));
    assert!(terminology.lookup(SIGNS, None, "routine").is_ok());
    terminology.remove_code_system(SIGNS, Some("1.0.0"));
    assert!(terminology.code_systems().is_empty());
}
