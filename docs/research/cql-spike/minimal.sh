#!/bin/bash
# Minimal reproductions of octofhir-cql 0.2.5 (commit 848156b) translator gaps found
# on WHO SMART libraries. Each case is valid CQL 1.5 that WHO libraries use.
# Usage: CQL=/path/to/cql bash minimal.sh   (writes the cases to ./min/)
CQL=${CQL:?set CQL to the path of the cql binary}
mkdir -p min && cd min || exit 1
hdr="using FHIR version '4.0.1'
include FHIRHelpers version '4.0.1'
parameter \"Measurement Period\" Interval<DateTime>
context Patient
"
t() { printf 'library %s\n%s\n%s\n' "$1" "$hdr" "$2" > "$1.cql"; printf -- '--- %s: ' "$1"; "$CQL" validate --color never "$1.cql" 2>&1 | grep -E 'error|^ok' | head -2; }
q="'"
t A_three_part_type   'define fluent function only(doses List<FHIR.Dosage.DoseAndRate>): singleton from doses'
t A3_two_part_type    'define function f(x List<FHIR.Dosage>): x'
t B_exists_alias      'define "X": [Observation]
define "Y": exists "X" O where O.status = '"${q}final${q}"
t B4_exists_paren     'define "X": [Observation]
define "Y": exists ("X" O where O.status = '"${q}final${q})"
t C_duration_of       'define "X": duration in months of Interval[@2020-01-01, @2020-05-01]'
t C2_duration_between 'define "X": months between @2020-01-01 and @2020-05-01'
t D_param_Today       'parameter Today Date default Today()'
t H_less_than         'define "Y": Patient.birthDate less than 12 months before end of "Measurement Period"'
t I_anon_tuple        'define "Y": [Observation] O return { id: O.id }'
t I2_named_tuple      'define "Y": [Observation] O return Tuple { id: O.id }'
t J_typed_list        'define "Y": List<String>{ '"${q}a${q}"' }'
t K_sort_fluent       'define "Y": [Observation] O sort by start of effective.toInterval()'
