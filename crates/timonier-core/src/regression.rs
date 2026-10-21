//! Performance regressions, as the spec decides them (« Seuil de régression », ADR 0053): a
//! candidate's runs against a reference of the main branch; a regression when the one-sided
//! Mann-Whitney test gives p < 0.01 and the medians differ by at least max(5 %, 3 × MAD of the
//! reference). Pure.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The test's threshold (spec).
pub const P_VALUE: f64 = 0.01;
/// The smallest change that counts, as a part of the reference (spec: 5 %).
pub const MIN_CHANGE: f64 = 0.05;
/// …and as MADs of the reference (spec: 3 × MAD).
pub const MADS: f64 = 3.0;
/// The runs of a batch (spec: 5, up to 15 if the variance is high).
pub const RUNS: usize = 5;
pub const MAX_RUNS: usize = 15;
/// A batch is spread when its MAD exceeds this part of its median: it gets more runs.
pub const SPREAD: f64 = 0.05;

pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    Some(if sorted.len().is_multiple_of(2) {
        f64::midpoint(sorted[middle - 1], sorted[middle])
    } else {
        sorted[middle]
    })
}

/// The median absolute deviation, unscaled.
pub fn mad(values: &[f64]) -> Option<f64> {
    let center = median(values)?;
    let deviations: Vec<f64> = values.iter().map(|v| (v - center).abs()).collect();
    median(&deviations)
}

/// Whether a batch's values are spread enough to deserve more runs (spec: up to 15).
pub fn needs_more(values: &[f64]) -> bool {
    if values.len() >= MAX_RUNS {
        return false;
    }
    if values.len() < RUNS {
        return true;
    }
    match (median(values), mad(values)) {
        (Some(center), Some(spread)) if center > 0.0 => spread / center > SPREAD,
        _ => false,
    }
}

/// The ranks of `all`, ties averaged; and the tie groups' sizes.
fn ranks(all: &[f64]) -> (Vec<f64>, Vec<usize>) {
    let mut order: Vec<usize> = (0..all.len()).collect();
    order.sort_by(|a, b| all[*a].total_cmp(&all[*b]));
    let mut ranks = vec![0.0; all.len()];
    let mut ties = Vec::new();
    let mut i = 0;
    while i < order.len() {
        let mut j = i;
        while j + 1 < order.len() && all[order[j + 1]] == all[order[i]] {
            j += 1;
        }
        #[allow(clippy::cast_precision_loss)]
        let rank = (i + j) as f64 / 2.0 + 1.0;
        for at in &order[i..=j] {
            ranks[*at] = rank;
        }
        if j > i {
            ties.push(j - i + 1);
        }
        i = j + 1;
    }
    (ranks, ties)
}

/// How many ways `n` values of the candidate and `m` of the reference give each U, by the
/// counting recurrence: f(n, m, u) = f(n − 1, m, u − m) + f(n, m − 1, u).
fn u_counts(n: usize, m: usize) -> Vec<f64> {
    // counts[i][j] is the distribution for i candidates and j references.
    let mut table: Vec<Vec<Vec<f64>>> = vec![vec![Vec::new(); m + 1]; n + 1];
    for i in 0..=n {
        for j in 0..=m {
            let mut counts = vec![0.0; i * j + 1];
            if i == 0 || j == 0 {
                counts[0] = 1.0;
            } else {
                for (u, slot) in counts.iter_mut().enumerate() {
                    let take = if u >= j {
                        table[i - 1][j].get(u - j).copied().unwrap_or(0.0)
                    } else {
                        0.0
                    };
                    let skip = table[i][j - 1].get(u).copied().unwrap_or(0.0);
                    *slot = take + skip;
                }
            }
            table[i][j] = counts;
        }
    }
    std::mem::take(&mut table[n][m])
}

/// The standard normal's upper tail, from the complementary error function.
fn upper_tail(z: f64) -> f64 {
    0.5 * erfc(z / std::f64::consts::SQRT_2)
}

fn erfc(x: f64) -> f64 {
    if x < 0.0 {
        return 2.0 - erfc(-x);
    }
    // Numerical Recipes' `erfcc` (a Chebyshev fit, fractional error below 1.2e-7): ample for
    // p-values compared with 0.01; the tests check it against SciPy.
    let t = 1.0 / (1.0 + 0.5 * x);
    let poly = -x * x - 1.265_512_23
        + t * (1.000_023_68
            + t * (0.374_091_96
                + t * (0.096_784_18
                    + t * (-0.186_288_06
                        + t * (0.278_868_07
                            + t * (-1.135_203_98
                                + t * (1.488_515_87 + t * (-0.822_152_23 + t * 0.170_872_77))))))));
    t * poly.exp()
}

/// The one-sided Mann-Whitney p-value that `candidate` is stochastically greater than
/// `reference`: exact when there is no tie and at most 40 values, else the normal
/// approximation with the tie correction and the continuity correction. `None` when a side is
/// empty.
pub fn mann_whitney_greater(candidate: &[f64], reference: &[f64]) -> Option<f64> {
    let (n, m) = (candidate.len(), reference.len());
    if n == 0 || m == 0 {
        return None;
    }
    let all: Vec<f64> = candidate.iter().chain(reference).copied().collect();
    let (ranks, ties) = ranks(&all);
    #[allow(clippy::cast_precision_loss)]
    let (nf, mf) = (n as f64, m as f64);
    let rank_sum: f64 = ranks[..n].iter().sum();
    let u = rank_sum - nf * (nf + 1.0) / 2.0;
    if ties.is_empty() && n + m <= 40 {
        let counts = u_counts(n, m);
        let total: f64 = counts.iter().sum();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let from = u.round() as usize;
        let tail: f64 = counts.iter().skip(from).sum();
        return Some(tail / total);
    }
    let mean = nf * mf / 2.0;
    #[allow(clippy::cast_precision_loss)]
    let tie_term: f64 = ties.iter().map(|t| (*t as f64).powi(3) - *t as f64).sum();
    let whole = nf + mf;
    let variance = nf * mf / 12.0 * ((whole + 1.0) - tie_term / (whole * (whole - 1.0)));
    if variance <= 0.0 {
        return Some(1.0);
    }
    let z = (u - mean - 0.5) / variance.sqrt();
    Some(upper_tail(z).clamp(0.0, 1.0))
}

/// What a comparison found.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    pub reference: f64,
    pub candidate: f64,
    /// `(candidate − reference) / reference`.
    pub change: f64,
    pub p_value: f64,
    /// The smallest change that counts here: max(5 %, 3 × MAD), in the values' unit.
    pub threshold: f64,
    pub regressed: bool,
}

/// `candidate`'s runs against the reference (spec): the reference's value is the median of its
/// snapshots' medians, its spread the MAD of all its values.
pub fn judge(candidate: &[f64], snapshots: &[Vec<f64>]) -> Option<Verdict> {
    let medians: Vec<f64> = snapshots.iter().filter_map(|s| median(s)).collect();
    let pooled: Vec<f64> = snapshots.iter().flatten().copied().collect();
    let reference = median(&medians)?;
    let candidate_median = median(candidate)?;
    let p_value = mann_whitney_greater(candidate, &pooled)?;
    let threshold = (MIN_CHANGE * reference.abs()).max(MADS * mad(&pooled).unwrap_or(0.0));
    let change = if reference == 0.0 {
        0.0
    } else {
        (candidate_median - reference) / reference
    };
    Some(Verdict {
        reference,
        candidate: candidate_median,
        change,
        p_value,
        threshold,
        regressed: p_value < P_VALUE && candidate_median - reference >= threshold,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance * b.abs().max(1e-12)
    }

    /// Against SciPy 1.18.1, `mannwhitneyu(x, y, alternative="greater")`, computed on
    /// 2026-10-21 (ADR 0053).
    #[test]
    fn the_mann_whitney_test_gives_scipy_s_values_on_known_datasets() {
        let ten = |from: usize| (from..from + 5).map(|v| v as f64).collect::<Vec<_>>();
        // Exact: complete separation, 1 / C(10, 5).
        let p = mann_whitney_greater(&ten(6), &ten(1)).unwrap();
        assert!(close(p, 0.003_968_253_968_253_968, 1e-12), "{p}");
        let x = [10.2, 10.5, 10.15, 10.8, 10.4];
        let y = [10.0, 9.9, 10.1, 10.25, 9.8, 10.05, 10.12, 9.95];
        let p = mann_whitney_greater(&x, &y).unwrap();
        assert!(close(p, 0.003_108_003_108_003_108, 1e-12), "{p}");
        let a = [1.1, 2.2, 3.3, 4.4, 5.5, 6.6, 7.7];
        let p = mann_whitney_greater(&a, &[2.5, 3.5, 4.5]).unwrap();
        assert!(close(p, 0.416_666_666_666_666_63, 1e-12), "{p}");
        // Asymptotic, ties corrected, continuity corrected.
        let p = mann_whitney_greater(
            &[3.0, 3.0, 4.0, 5.0, 5.0, 6.0],
            &[1.0, 2.0, 2.0, 3.0, 4.0, 4.0],
        )
        .unwrap();
        assert!(close(p, 0.030_428_225_023_950_876, 1e-5), "{p}");
        let big: Vec<f64> = (30..60).map(f64::from).collect();
        let small: Vec<f64> = (0..40).map(f64::from).collect();
        let p = mann_whitney_greater(&big, &small).unwrap();
        assert!(close(p, 3.470_286_552_579_936_6e-11, 1e-4), "{p}");
        assert_eq!(mann_whitney_greater(&[5.0; 5], &[5.0; 5]), Some(1.0));
        assert_eq!(mann_whitney_greater(&[], &[1.0]), None);
    }

    #[test]
    fn median_and_mad() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), Some(2.5));
        // |1−3|, |2−3|, |3−3|, |4−3|, |9−3| → 2, 1, 0, 1, 6 → 1.
        assert_eq!(mad(&[1.0, 2.0, 3.0, 4.0, 9.0]), Some(1.0));
        assert_eq!(mad(&[]), None);
    }

    #[test]
    fn a_spread_batch_gets_more_runs_up_to_fifteen() {
        assert!(needs_more(&[1.0, 1.0, 1.0]), "fewer than five");
        assert!(!needs_more(&[100.0, 101.0, 99.0, 100.0, 100.5]), "tight");
        assert!(needs_more(&[100.0, 130.0, 70.0, 100.0, 140.0]), "spread");
        assert!(
            !needs_more(&[100.0, 130.0, 70.0, 100.0, 140.0].repeat(3)),
            "fifteen at most"
        );
    }

    #[test]
    fn a_regression_needs_the_test_and_a_change_beyond_five_percent_and_three_mads() {
        let reference = vec![vec![100.0, 101.0, 99.0, 100.0, 100.5]];
        // Slower beyond doubt: regressed.
        let slow = judge(&[120.0, 121.0, 119.0, 122.0, 120.5], &reference).unwrap();
        assert!(slow.regressed, "{slow:?}");
        assert!(close(slow.change, 0.205, 1e-9), "{slow:?}");
        // Significant but under 5 %: not a regression.
        let slight = judge(&[102.0, 103.0, 102.5, 103.5, 102.2], &reference).unwrap();
        assert!(slight.p_value < P_VALUE && !slight.regressed, "{slight:?}");
        // A noisy reference: 3 MADs beyond the change, not a regression.
        let noisy = vec![vec![100.0, 60.0, 140.0, 80.0, 120.0]];
        let within = judge(&[150.0, 151.0, 152.0, 153.0, 154.0], &noisy).unwrap();
        assert!(within.threshold >= 60.0 && !within.regressed, "{within:?}");
        // Faster: never a regression.
        assert!(
            !judge(&[80.0, 81.0, 79.0, 80.0, 80.5], &reference)
                .unwrap()
                .regressed
        );
        // The reference is the median of the snapshots' medians.
        let snapshots = vec![vec![100.0; 5], vec![110.0; 5], vec![90.0; 5]];
        assert_eq!(judge(&[100.0; 5], &snapshots).unwrap().reference, 100.0);
        assert!(judge(&[1.0], &[]).is_none());
    }
}
