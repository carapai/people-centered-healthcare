//! String comparators for Fellegi–Sunter fields.

/// Jaro–Winkler similarity in `[0, 1]` (prefix scale 0.1, up to 4 characters).
pub fn jaro_winkler(a: &str, b: &str) -> f64 {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let window = (a.len().max(b.len()) / 2).saturating_sub(1);
    let mut a_matched = vec![false; a.len()];
    let mut b_matched = vec![false; b.len()];
    let mut matches = 0usize;
    for (i, ca) in a.iter().enumerate() {
        let start = i.saturating_sub(window);
        let end = (i + window + 1).min(b.len());
        for j in start..end {
            if !b_matched[j] && b[j] == *ca {
                a_matched[i] = true;
                b_matched[j] = true;
                matches += 1;
                break;
            }
        }
    }
    if matches == 0 {
        return 0.0;
    }
    let a_seq = a
        .iter()
        .zip(&a_matched)
        .filter(|(_, m)| **m)
        .map(|(c, _)| c);
    let b_seq = b
        .iter()
        .zip(&b_matched)
        .filter(|(_, m)| **m)
        .map(|(c, _)| c);
    let transpositions = a_seq.zip(b_seq).filter(|(x, y)| x != y).count() / 2;
    #[allow(clippy::cast_precision_loss)]
    let (m, t, la, lb) = (
        matches as f64,
        transpositions as f64,
        a.len() as f64,
        b.len() as f64,
    );
    let jaro = (m / la + m / lb + (m - t) / m) / 3.0;
    let prefix = a.iter().zip(&b).take(4).take_while(|(x, y)| x == y).count();
    #[allow(clippy::cast_precision_loss)]
    let boost = prefix as f64 * 0.1 * (1.0 - jaro);
    jaro + boost
}

/// Optimal-string-alignment Damerau–Levenshtein distance: insertions, deletions, substitutions
/// and adjacent transpositions.
pub fn damerau_levenshtein(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = best;
        }
    }
    d[a.len()][b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values() {
        // The classic examples.
        assert!((jaro_winkler("MARTHA", "MARHTA") - 0.9611).abs() < 1e-3);
        assert!((jaro_winkler("DIXON", "DICKSONX") - 0.8133).abs() < 1e-3);
        assert_eq!(jaro_winkler("", ""), 1.0);
        assert_eq!(jaro_winkler("A", ""), 0.0);
        assert_eq!(damerau_levenshtein("OKELLO", "OKELLO"), 0);
        assert_eq!(
            damerau_levenshtein("OKELLO", "OKLELO"),
            1,
            "adjacent transposition"
        );
        assert_eq!(damerau_levenshtein("OKELLO", "AKELLO"), 1);
        assert_eq!(damerau_levenshtein("KATO", "NAKATO"), 2);
    }
}
