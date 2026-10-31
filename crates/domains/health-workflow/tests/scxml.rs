//! The curated W3C SCXML conformance subset (ADR 0042): one test per assertion of SCXML 1.0's
//! core semantics (§3) that the workflow model has, named by its assertion id in the W3C
//! implementation report manifest (https://www.w3.org/Voice/2013/scxml-irp/manifest.xml).
//!
//! **Left out:**
//! - history states (387, 388, 579, 580), which the workflow model does not have;
//! - error events (401, 402): our executable content cannot fail;
//! - the ECMAScript data model assertions: guards are FHIRPath, through the `Guards` port.
//!
//! Executable content is observed through `log` actions, which surface as `Execute` effects in
//! order.

use std::collections::BTreeMap;

use health_time::SignedDuration;
use health_workflow::{
    Action, DataModel, DefinitionError, Effect, Event, Guards, Instance, Invoke, Kind, Machine,
    State, Statechart, Timer, Transition, Value, matches,
};

/// Guards for the tests: `true`, `false`, or `flag` (the data-model field is `true`).
struct TestGuards;

impl Guards for TestGuards {
    fn holds(&self, guard: &str, _event: Option<&Event>, data: &DataModel) -> bool {
        match guard {
            "true" => true,
            "false" => false,
            field => data.get(field) == Some(&Value::Bool(true)),
        }
    }
}

fn log(message: &str) -> Action {
    Action::Execute {
        action: "log".into(),
        params: BTreeMap::from([("msg".into(), Value::Text(message.into()))]),
    }
}

fn raise(event: &str) -> Action {
    Action::Raise {
        event: event.into(),
    }
}

fn atomic(id: &str) -> State {
    State::new(id, Kind::Atomic)
}

fn final_(id: &str) -> State {
    State::new(id, Kind::Final)
}

fn compound(id: &str, children: Vec<State>) -> State {
    State {
        children,
        ..State::new(id, Kind::Compound)
    }
}

fn parallel(id: &str, children: Vec<State>) -> State {
    State {
        children,
        ..State::new(id, Kind::Parallel)
    }
}

fn on(event: &str, target: &str) -> Transition {
    Transition {
        events: vec![event.into()],
        guard: None,
        targets: vec![target.into()],
        internal: false,
        actions: Vec::new(),
    }
}

fn always(target: &str) -> Transition {
    Transition {
        events: Vec::new(),
        ..on("", target)
    }
}

fn with(mut state: State, f: impl FnOnce(&mut State)) -> State {
    f(&mut state);
    state
}

fn chart(states: Vec<State>) -> Statechart {
    Statechart {
        states,
        initial: Vec::new(),
    }
}

fn machine(chart: &Statechart) -> Machine {
    Machine::new(chart).expect("valid statechart")
}

fn logs(effects: &[Effect]) -> Vec<String> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Execute { action, params, .. } if action == "log" => match params.get("msg") {
                Some(Value::Text(m)) => Some(m.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn send(machine: &Machine, instance: Instance, event: &str) -> (Instance, Vec<Effect>) {
    machine
        .step(instance, &Event::named(event), &TestGuards)
        .expect("step")
}

fn active(instance: &Instance) -> Vec<&str> {
    instance.configuration.iter().map(String::as_str).collect()
}

#[test]
fn a355_without_initial_the_first_state_in_document_order_is_entered() {
    let m = machine(&chart(vec![atomic("s0"), atomic("s1")]));
    let (instance, _) = m.start(&TestGuards);
    assert_eq!(active(&instance), ["s0"]);
}

#[test]
fn a576_a413_the_initial_attribute_names_the_start_configuration_even_deep_in_parallels() {
    let mut c = chart(vec![
        atomic("s0"),
        parallel(
            "p",
            vec![
                compound("r1", vec![atomic("r1a"), atomic("r1b")]),
                compound("r2", vec![atomic("r2a"), atomic("r2b")]),
            ],
        ),
    ]);
    c.initial = vec!["r1b".into(), "r2b".into()];
    let (instance, _) = machine(&c).start(&TestGuards);
    assert_eq!(active(&instance), ["p", "r1", "r1b", "r2", "r2b"]);
}

#[test]
fn a364_a412_a_compound_entered_as_a_target_enters_its_default_initial_states() {
    let c = chart(vec![
        with(atomic("s0"), |s| s.transitions.push(on("go", "c"))),
        compound("c", vec![atomic("c1"), atomic("c2")]),
        with(compound("d", vec![atomic("d1"), atomic("d2")]), |s| {
            s.initial = vec!["d2".into()];
        }),
    ]);
    let m = machine(&c);
    let (instance, _) = m.start(&TestGuards);
    let (instance, _) = send(&m, instance, "go");
    // No initial: the first child; with initial: that child.
    assert_eq!(active(&instance), ["c", "c1"]);
    let mut d = c.clone();
    d.states[0].transitions = vec![on("go", "d")];
    let m = machine(&d);
    let (instance, _) = m.start(&TestGuards);
    let (instance, _) = send(&m, instance, "go");
    assert_eq!(active(&instance), ["d", "d2"]);
}

#[test]
fn a372_a416_entering_a_final_child_raises_done_state_after_its_onentry() {
    let c = chart(vec![
        with(
            compound(
                "s0",
                vec![
                    with(atomic("s01"), |s| s.transitions.push(on("go", "done"))),
                    with(final_("done"), |s| {
                        s.on_entry.push(vec![log("final entered")])
                    }),
                ],
            ),
            |s| {
                s.transitions.push(Transition {
                    actions: vec![log("done.state.s0")],
                    ..on("done.state.s0", "pass")
                });
            },
        ),
        final_("pass"),
    ]);
    let m = machine(&c);
    let (instance, _) = m.start(&TestGuards);
    let (instance, effects) = send(&m, instance, "go");
    assert_eq!(logs(&effects), ["final entered", "done.state.s0"]);
    assert!(effects.contains(&Effect::Completed));
    assert!(!instance.running);
}

#[test]
fn a570_a417_a_parallel_is_done_once_every_region_is_done() {
    let region = |id: &str| {
        compound(
            id,
            vec![
                with(atomic(&format!("{id}a")), |s| {
                    s.transitions
                        .push(on(&format!("{id}.go"), &format!("{id}f")))
                }),
                final_(&format!("{id}f")),
            ],
        )
    };
    let c = chart(vec![
        with(parallel("p", vec![region("r1"), region("r2")]), |s| {
            s.transitions.push(on("done.state.p", "pass"));
        }),
        final_("pass"),
    ]);
    let m = machine(&c);
    let (instance, _) = m.start(&TestGuards);
    let (instance, _) = send(&m, instance, "r1.go");
    // One region done: done.state.r1 only, the parallel goes on.
    assert!(active(&instance).contains(&"p"));
    let (instance, effects) = send(&m, instance, "r2.go");
    assert!(effects.contains(&Effect::Completed));
    assert_eq!(active(&instance), ["pass"]);
}

#[test]
fn a375_a376_onentry_handlers_run_in_document_order_as_separate_blocks() {
    let c = chart(vec![with(atomic("s0"), |s| {
        s.on_entry = vec![vec![log("a1"), log("a2")], vec![log("b")]];
    })]);
    let (_, effects) = machine(&c).start(&TestGuards);
    assert_eq!(logs(&effects), ["a1", "a2", "b"]);
}

#[test]
fn a377_a378_a407_a409_onexit_handlers_run_in_document_order_and_the_state_is_removed() {
    let c = chart(vec![
        with(atomic("s0"), |s| {
            s.on_exit = vec![vec![log("x1")], vec![log("x2")]];
            s.transitions.push(on("go", "s1"));
        }),
        atomic("s1"),
    ]);
    let m = machine(&c);
    let (instance, _) = m.start(&TestGuards);
    let (instance, effects) = send(&m, instance, "go");
    assert_eq!(logs(&effects), ["x1", "x2"]);
    assert_eq!(active(&instance), ["s1"]);
}

#[test]
fn a396_a399_event_descriptors_match_by_name_token_prefix() {
    assert!(matches("foo", "foo"));
    assert!(matches("foo", "foo.bar.baz"));
    assert!(matches("foo.*", "foo.bar"));
    assert!(matches("foo.bar", "foo.bar.baz"));
    assert!(matches("*", "anything.at.all"));
    assert!(!matches("foo", "foobar"));
    assert!(!matches("foo.bar", "foo"));
    // A transition on `triage` is taken by `triage.completed`.
    let c = chart(vec![
        with(atomic("s0"), |s| s.transitions.push(on("triage", "s1"))),
        atomic("s1"),
    ]);
    let m = machine(&c);
    let (instance, _) = m.start(&TestGuards);
    let (instance, _) = send(&m, instance, "triage.completed");
    assert_eq!(active(&instance), ["s1"]);
}

#[test]
fn a403_the_optimal_transition_set_prefers_descendants_and_takes_every_parallel_region() {
    // A child's transition preempts its ancestor's for the same event.
    let c = chart(vec![
        with(
            compound(
                "s0",
                vec![with(atomic("s01"), |s| {
                    s.transitions.push(on("go", "child"))
                })],
            ),
            |s| s.transitions.push(on("go", "parent")),
        ),
        atomic("child"),
        atomic("parent"),
    ]);
    let m = machine(&c);
    let (instance, _) = m.start(&TestGuards);
    let (instance, _) = send(&m, instance, "go");
    assert_eq!(active(&instance), ["child"]);

    // In a parallel, each region takes its own transition, in document order.
    let region = |id: &str| {
        compound(
            id,
            vec![
                with(atomic(&format!("{id}a")), |s| {
                    s.transitions.push(Transition {
                        actions: vec![log(id)],
                        ..on("go", &format!("{id}b"))
                    });
                }),
                atomic(&format!("{id}b")),
            ],
        )
    };
    let c = chart(vec![parallel("p", vec![region("r1"), region("r2")])]);
    let m = machine(&c);
    let (instance, _) = m.start(&TestGuards);
    let (instance, effects) = send(&m, instance, "go");
    assert_eq!(logs(&effects), ["r1", "r2"]);
    assert_eq!(active(&instance), ["p", "r1", "r1b", "r2", "r2b"]);

    // Two regions whose transitions both leave the parallel: the first in document order wins.
    let c = chart(vec![
        parallel(
            "p",
            vec![
                with(atomic("a"), |s| s.transitions.push(on("go", "out1"))),
                with(atomic("b"), |s| s.transitions.push(on("go", "out2"))),
            ],
        ),
        atomic("out1"),
        atomic("out2"),
    ]);
    let m = machine(&c);
    let (instance, _) = m.start(&TestGuards);
    let (instance, _) = send(&m, instance, "go");
    assert_eq!(active(&instance), ["out1"]);
}

#[test]
fn a404_a405_a406_a411_exits_then_transition_content_then_entries() {
    let c = chart(vec![
        with(
            compound(
                "s0",
                vec![with(atomic("s01"), |s| {
                    s.on_exit.push(vec![log("exit s01")])
                })],
            ),
            |s| {
                s.on_exit.push(vec![log("exit s0")]);
                s.transitions.push(Transition {
                    actions: vec![log("t1"), log("t2")],
                    ..on("go", "s11")
                });
            },
        ),
        with(
            compound(
                "s1",
                vec![with(atomic("s11"), |s| {
                    s.on_entry.push(vec![log("enter s11")])
                })],
            ),
            |s| {
                s.on_entry.push(vec![log("enter s1")]);
            },
        ),
    ]);
    let m = machine(&c);
    let (instance, _) = m.start(&TestGuards);
    let (_, effects) = send(&m, instance, "go");
    assert_eq!(
        logs(&effects),
        ["exit s01", "exit s0", "t1", "t2", "enter s1", "enter s11"]
    );
}

#[test]
fn a419_a421_eventless_transitions_come_before_internal_events_which_are_drained() {
    let c = chart(vec![
        with(atomic("s0"), |s| {
            // Raised events wait while an eventless transition is enabled.
            s.on_entry.push(vec![raise("unused"), raise("next")]);
            s.transitions.push(always("s1"));
        }),
        with(atomic("s1"), |s| {
            s.on_entry.push(vec![log("s1")]);
            s.transitions.push(on("next", "s2"));
        }),
        with(atomic("s2"), |s| s.on_entry.push(vec![log("s2")])),
    ]);
    let (instance, effects) = machine(&c).start(&TestGuards);
    // `unused` enables nothing and is discarded; `next` then moves on.
    assert_eq!(logs(&effects), ["s1", "s2"]);
    assert_eq!(active(&instance), ["s2"]);
}

#[test]
fn a422_invocations_start_after_the_macrostep_only_for_states_still_active() {
    let invoking = |id: &str| {
        with(atomic(id), |s| {
            s.invoke.push(Invoke {
                id: format!("{id}-sub"),
                src: "https://example.org/wf|1".into(),
            });
        })
    };
    let c = chart(vec![
        with(invoking("passing"), |s| {
            s.transitions.push(always("staying"))
        }),
        with(invoking("staying"), |s| {
            s.transitions.push(on("go", "after"))
        }),
        atomic("after"),
    ]);
    let m = machine(&c);
    let (instance, effects) = m.start(&TestGuards);
    // `passing` was entered and left in the same macrostep: never invoked.
    assert_eq!(
        effects,
        [Effect::StartInvoke {
            id: "staying-sub".into(),
            src: "https://example.org/wf|1".into()
        }]
    );
    let (_, effects) = send(&m, instance, "go");
    assert_eq!(
        effects,
        [Effect::CancelInvoke {
            id: "staying-sub".into()
        }]
    );
}

#[test]
fn a503_a_targetless_transition_exits_nothing() {
    let c = chart(vec![with(atomic("s0"), |s| {
        s.on_exit.push(vec![log("exit")]);
        s.transitions.push(Transition {
            targets: Vec::new(),
            actions: vec![log("handled")],
            ..on("go", "s0")
        });
    })]);
    let m = machine(&c);
    let (instance, _) = m.start(&TestGuards);
    let (instance, effects) = send(&m, instance, "go");
    assert_eq!(logs(&effects), ["handled"]);
    assert_eq!(active(&instance), ["s0"]);
}

#[test]
fn a504_an_external_self_transition_exits_and_reenters_its_source() {
    let c = chart(vec![with(compound("s0", vec![atomic("s01")]), |s| {
        s.on_entry.push(vec![log("enter")]);
        s.on_exit.push(vec![log("exit")]);
        s.transitions.push(on("go", "s01"));
    })]);
    let m = machine(&c);
    let (instance, _) = m.start(&TestGuards);
    let (_, effects) = send(&m, instance, "go");
    assert_eq!(logs(&effects), ["exit", "enter"]);
}

#[test]
fn a505_an_internal_transition_to_a_descendant_does_not_exit_its_source() {
    let c = chart(vec![with(
        compound("s0", vec![atomic("s01"), atomic("s02")]),
        |s| {
            s.on_entry.push(vec![log("enter")]);
            s.on_exit.push(vec![log("exit")]);
            s.transitions.push(Transition {
                internal: true,
                ..on("go", "s02")
            });
        },
    )]);
    let m = machine(&c);
    let (instance, _) = m.start(&TestGuards);
    let (instance, effects) = send(&m, instance, "go");
    assert!(logs(&effects).is_empty());
    assert_eq!(active(&instance), ["s0", "s02"]);
}

#[test]
fn a506_a533_an_internal_transition_from_an_atomic_source_behaves_as_external() {
    let c = chart(vec![with(atomic("s0"), |s| {
        s.on_entry.push(vec![log("enter")]);
        s.on_exit.push(vec![log("exit")]);
        s.transitions.push(Transition {
            internal: true,
            ..on("go", "s0")
        });
    })]);
    let m = machine(&c);
    let (instance, _) = m.start(&TestGuards);
    let (_, effects) = send(&m, instance, "go");
    assert_eq!(logs(&effects), ["exit", "enter"]);
}

#[test]
fn guards_choose_the_first_enabled_transition_in_document_order() {
    let c = chart(vec![
        with(atomic("s0"), |s| {
            s.transitions.push(Transition {
                guard: Some("emergency".into()),
                ..on("next", "consultation")
            });
            s.transitions.push(on("next", "triage"));
        }),
        atomic("triage"),
        atomic("consultation"),
    ]);
    let m = machine(&c);
    let (instance, _) = m.start(&TestGuards);
    let (routine, _) = send(&m, instance.clone(), "next");
    assert_eq!(active(&routine), ["triage"]);
    let mut urgent = instance;
    urgent.data.insert("emergency".into(), Value::Bool(true));
    let (urgent, _) = send(&m, urgent, "next");
    assert_eq!(active(&urgent), ["consultation"]);
}

#[test]
fn timers_start_on_entry_and_are_cancelled_on_exit_and_action_keys_are_unique() {
    let c = chart(vec![
        with(atomic("waiting"), |s| {
            s.timers.push(Timer {
                after: SignedDuration::from_mins(15),
                event: "claim.timeout".into(),
            });
            s.on_entry.push(vec![log("waiting")]);
            s.transitions.push(on("claim.timeout", "escalated"));
        }),
        with(atomic("escalated"), |s| {
            s.on_entry.push(vec![log("escalated"), log("again")])
        }),
    ]);
    let m = machine(&c);
    let (instance, effects) = m.start(&TestGuards);
    assert!(effects.contains(&Effect::StartTimer {
        id: "waiting#0".into(),
        after: SignedDuration::from_mins(15),
        event: "claim.timeout".into()
    }));
    let (_, effects) = send(&m, instance, "claim.timeout");
    assert!(effects.contains(&Effect::CancelTimer {
        id: "waiting#0".into()
    }));
    let keys: Vec<(u64, u32)> = effects
        .iter()
        .filter_map(|e| match e {
            Effect::Execute { key, .. } => Some(*key),
            _ => None,
        })
        .collect();
    assert_eq!(keys, [(2, 0), (2, 1)]);
}

#[test]
fn the_same_instance_and_event_always_give_the_same_result_and_instances_persist() {
    let c = chart(vec![
        with(atomic("s0"), |s| {
            s.transitions.push(Transition {
                actions: vec![Action::Assign {
                    field: "count".into(),
                    value: Value::Int(1),
                }],
                ..on("go", "s1")
            });
        }),
        atomic("s1"),
    ]);
    let m = machine(&c);
    let (instance, _) = m.start(&TestGuards);
    let first = send(&m, instance.clone(), "go");
    let second = send(&m, instance, "go");
    assert_eq!(first, second);
    assert_eq!(first.0.data.get("count"), Some(&Value::Int(1)));
    // An instance round-trips through its persisted form.
    let json = serde_json::to_string(&first.0).expect("json");
    assert_eq!(
        serde_json::from_str::<Instance>(&json).expect("instance"),
        first.0
    );
    // A completed instance ignores further events.
    let done = Instance {
        running: false,
        ..first.0.clone()
    };
    assert_eq!(send(&m, done.clone(), "go").0, done);
}

#[test]
fn invalid_definitions_are_refused() {
    assert_eq!(
        Machine::new(&chart(vec![atomic("a"), atomic("a")])).err(),
        Some(DefinitionError::DuplicateState("a".into()))
    );
    assert_eq!(
        Machine::new(&chart(vec![with(atomic("a"), |s| s
            .transitions
            .push(on("go", "nowhere")))]))
        .err(),
        Some(DefinitionError::UnknownTarget("nowhere".into()))
    );
    assert_eq!(
        Machine::new(&chart(vec![State::new("c", Kind::Compound)])).err(),
        Some(DefinitionError::BadChildren("c".into()))
    );
    assert_eq!(
        Machine::new(&chart(vec![
            with(compound("c", vec![atomic("c1")]), |s| s.initial =
                vec!["other".into()]),
            atomic("other"),
        ]))
        .err(),
        Some(DefinitionError::BadInitial("c".into()))
    );
}
