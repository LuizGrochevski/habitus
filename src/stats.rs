use chrono::NaiveDate;
use std::collections::HashSet;

/// Índice de Jaccard: mede o quanto dois conjuntos de dias se sobrepõem.
/// 0.0 = nenhum dia em comum; 1.0 = exatamente os mesmos dias.
/// Fórmula: tamanho da INTERSEÇÃO dividido pelo tamanho da UNIÃO.
pub fn jaccard(a: &[NaiveDate], b: &[NaiveDate]) -> f64 {
    let set_a: HashSet<NaiveDate> = a.iter().cloned().collect();
    let set_b: HashSet<NaiveDate> = b.iter().cloned().collect();

    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();

    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jaccard_um_quando_dias_identicos() {
        let d1 = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        let d2 = NaiveDate::from_ymd_opt(2026, 7, 2).unwrap();
        let a = vec![d1, d2];
        let b = vec![d1, d2];
        assert_eq!(jaccard(&a, &b), 1.0);
    }

    #[test]
    fn jaccard_zero_quando_nenhum_dia_em_comum() {
        let d1 = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        let d2 = NaiveDate::from_ymd_opt(2026, 7, 2).unwrap();
        let a = vec![d1];
        let b = vec![d2];
        assert_eq!(jaccard(&a, &b), 0.0);
    }

    #[test]
    fn jaccard_meio_quando_metade_sobrepoe() {
        let d1 = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        let d2 = NaiveDate::from_ymd_opt(2026, 7, 2).unwrap();
        // a = {d1, d2}, b = {d1} -> interseção=1, união=2 -> 0.5
        let a = vec![d1, d2];
        let b = vec![d1];
        assert_eq!(jaccard(&a, &b), 0.5);
    }
}
