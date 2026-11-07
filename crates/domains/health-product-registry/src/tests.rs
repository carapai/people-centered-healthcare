use super::*;

fn coding(code: &str) -> Coding {
    Coding {
        system: "http://www.whocc.no/atc".into(),
        code: code.into(),
    }
}

fn product(code: &str, name: &str, medicines: &[&str], synonyms: &[&str]) -> Product {
    Product {
        code: code.into(),
        name: name.into(),
        gtin: None,
        base_unit: "tablet".into(),
        pack_sizes: vec![24],
        medicines: medicines.iter().map(|m| coding(m)).collect(),
        synonyms: synonyms.iter().map(|s| (*s).to_owned()).collect(),
        active: true,
    }
}

fn medicine(text: &str, code: Option<&str>) -> Medicine {
    Medicine {
        text: text.into(),
        coding: code.map(coding),
    }
}

fn catalogue() -> Vec<Product> {
    let mut retired = product(
        "al-old",
        "Artemether/lumefantrine 20/120 mg (old pack)",
        &["P01BF01"],
        &[],
    );
    retired.active = false;
    vec![
        product(
            "al-20-120",
            "Artemether/lumefantrine 20/120 mg",
            &["P01BF01"],
            &["Coartem", "AL 20/120"],
        ),
        product("amoxicillin-500", "Amoxicillin 500 mg", &["J01CA04"], &[]),
        product(
            "amoxicillin-500-dt",
            "Amoxicillin 500 mg dispersible",
            &["J01CA04"],
            &[],
        ),
        retired,
    ]
}

#[test]
fn a_coded_medicine_resolves_by_its_code_first() {
    assert_eq!(
        resolve(&catalogue(), &medicine("anything at all", Some("P01BF01"))),
        Resolution::ByCode {
            product: "al-20-120".into()
        },
        "the retired product shares the code but is never chosen"
    );
    assert_eq!(
        resolve(
            &catalogue(),
            &medicine("Amoxicillin 500 mg", Some("J01CA04"))
        ),
        Resolution::Ambiguous {
            products: vec!["amoxicillin-500".into(), "amoxicillin-500-dt".into()]
        },
        "two products fill the code: the pharmacist chooses"
    );
}

#[test]
fn free_text_resolves_by_name_or_synonym_ignoring_case_spacing_and_punctuation() {
    for written in [
        "Artemether/lumefantrine 20/120 mg",
        "artemether / LUMEFANTRINE 20/120mg",
        "Coartem",
        "al 20/120",
    ] {
        assert_eq!(
            resolve(&catalogue(), &medicine(written, None)).product(),
            Some("al-20-120"),
            "{written}"
        );
    }
    assert_eq!(
        resolve(&catalogue(), &medicine("Paracetamol 500 mg", None)),
        Resolution::Unknown
    );
    assert_eq!(
        resolve(&catalogue(), &medicine("  ", None)),
        Resolution::Unknown
    );
    // An unknown code falls back to the name.
    assert_eq!(
        resolve(
            &catalogue(),
            &medicine("Amoxicillin 500 mg", Some("UNKNOWN"))
        ),
        Resolution::ByName {
            product: "amoxicillin-500".into()
        }
    );
}

#[test]
fn products_are_validated() {
    let good = product("al-20-120", "AL", &[], &[]);
    assert_eq!(good.validate(), Ok(()));
    let mut bad = good.clone();
    bad.code = "AL 20".into();
    assert_eq!(bad.validate(), Err(ProductRefused::Code));
    let mut bad = good.clone();
    bad.base_unit = " ".into();
    assert_eq!(bad.validate(), Err(ProductRefused::Missing("base unit")));
    let mut bad = good.clone();
    bad.pack_sizes = vec![0];
    assert_eq!(bad.validate(), Err(ProductRefused::PackSize));
    let mut gtin = good.clone();
    gtin.gtin = Some("09501101530003".into());
    assert_eq!(
        gtin.validate(),
        Ok(()),
        "the GTIN of the stock e2e's barcode"
    );
    gtin.gtin = Some("09501101530004".into());
    assert_eq!(
        gtin.validate(),
        Err(ProductRefused::Gtin("09501101530004".into()))
    );
    assert!(valid_gtin("4006381333931"), "a GTIN-13");
    assert!(!valid_gtin("40063813339a1"));
}
