use crate::models::{
    Battle, Confidence, DataQuality, FeedbackItem, FeedbackTone, SessionMetrics, stats_eligibility,
};

fn confidence(n: usize) -> Confidence {
    match n {
        0..=9 => Confidence::Low,
        10..=29 => Confidence::Medium,
        _ => Confidence::High,
    }
}
fn item(
    code: &str,
    tone: FeedbackTone,
    actual: Option<f64>,
    baseline: Option<f64>,
    n: usize,
) -> FeedbackItem {
    FeedbackItem {
        code: code.into(),
        tone,
        confidence: confidence(n),
        actual,
        baseline,
        sample_size: n,
    }
}
#[cfg(test)]
fn avg(v: impl Iterator<Item = Option<i64>>) -> Option<(f64, usize)> {
    let v = v.flatten().collect::<Vec<_>>();
    (!v.is_empty()).then(|| (v.iter().sum::<i64>() as f64 / v.len() as f64, v.len()))
}

/// What per-battle feedback needs from the reference battles (P4): the number of battles that
/// count for statistics and the averages (with sample sizes) of damage and assist over them.
/// The database computes it with SQL aggregates instead of loading full rows.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FeedbackBaseline {
    pub battles: usize,
    pub damage: Option<(f64, usize)>,
    pub assist: Option<(f64, usize)>,
}

impl FeedbackBaseline {
    /// Average from integer sums, so SQL and row-based results are bit-identical.
    pub fn from_sums(battles: usize, damage: (i64, usize), assist: (i64, usize)) -> Self {
        let average =
            |(sum, count): (i64, usize)| (count > 0).then(|| (sum as f64 / count as f64, count));
        Self {
            battles,
            damage: average(damage),
            assist: average(assist),
        }
    }

    #[cfg(test)]
    pub fn from_battles(baseline: &[Battle]) -> Self {
        let full = baseline
            .iter()
            .filter(|x| stats_eligibility(x))
            .collect::<Vec<_>>();
        Self {
            battles: full.len(),
            damage: avg(full.iter().map(|x| x.damage)),
            assist: avg(full.iter().map(|x| x.assist)),
        }
    }
}

pub fn battle_feedback(b: &Battle, baseline: &FeedbackBaseline) -> Vec<FeedbackItem> {
    if b.data_quality == DataQuality::Full && !stats_eligibility(b) {
        return vec![item(
            "battle_not_counted",
            FeedbackTone::Neutral,
            None,
            None,
            0,
        )];
    }
    if b.data_quality != DataQuality::Full {
        return vec![item(
            if b.data_quality == DataQuality::Ambiguous {
                "battle_ambiguous"
            } else {
                "battle_partial"
            },
            FeedbackTone::Neutral,
            None,
            None,
            0,
        )];
    }
    if baseline.battles < 5 {
        return vec![item(
            "baseline_building",
            FeedbackTone::Neutral,
            None,
            None,
            baseline.battles,
        )];
    }
    let mut pos = Vec::<(f64, FeedbackItem)>::new();
    let mut warn = Vec::<(f64, FeedbackItem)>::new();
    if let (Some(actual), Some((base, n))) = (b.damage, baseline.damage)
        && base > 0.0
        && n >= 5
    {
        let r = actual as f64 / base;
        if r >= 1.2 {
            pos.push((
                r,
                item(
                    "damage_above_baseline",
                    FeedbackTone::Positive,
                    Some(actual as f64),
                    Some(base),
                    n,
                ),
            ))
        } else if r <= 0.65 {
            warn.push((
                1.0 - r,
                item(
                    "damage_below_baseline",
                    FeedbackTone::Warning,
                    Some(actual as f64),
                    Some(base),
                    n,
                ),
            ))
        }
    }
    if let (Some(actual), Some((base, n))) = (b.assist, baseline.assist)
        && base >= 100.0
        && n >= 5
    {
        let r = actual as f64 / base;
        if r >= 1.25 {
            pos.push((
                r - 1.0,
                item(
                    "assist_above_baseline",
                    FeedbackTone::Positive,
                    Some(actual as f64),
                    Some(base),
                    n,
                ),
            ))
        } else if r <= 0.4 {
            warn.push((
                1.0 - r,
                item(
                    "assist_below_baseline",
                    FeedbackTone::Warning,
                    Some(actual as f64),
                    Some(base),
                    n,
                ),
            ))
        }
    }
    if let (Some(shots), Some(pens)) = (b.shots, b.penetrations)
        && shots >= 5
    {
        let r = pens as f64 / shots as f64;
        if r >= 0.7 {
            pos.push((
                r,
                item(
                    "penetration_strong",
                    FeedbackTone::Positive,
                    Some(r * 100.0),
                    None,
                    baseline.battles,
                ),
            ))
        } else if r <= 0.4 {
            warn.push((
                1.0 - r,
                item(
                    "penetration_low",
                    FeedbackTone::Warning,
                    Some(r * 100.0),
                    None,
                    baseline.battles,
                ),
            ))
        }
    }
    pos.sort_by(|a, b| b.0.total_cmp(&a.0));
    warn.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut out = Vec::new();
    if let Some((_, v)) = pos.into_iter().next() {
        out.push(v)
    }
    out.extend(warn.into_iter().take(2).map(|x| x.1));
    if out.is_empty() {
        out.push(item(
            "performance_near_baseline",
            FeedbackTone::Neutral,
            b.damage.map(|x| x as f64),
            baseline.damage.map(|x| x.0),
            baseline.battles,
        ))
    }
    out
}

pub fn session_feedback(c: &SessionMetrics, b: Option<&SessionMetrics>) -> Vec<FeedbackItem> {
    if c.analyzed_battles < 3 {
        return vec![item(
            "session_sample_small",
            FeedbackTone::Neutral,
            Some(c.analyzed_battles as f64),
            None,
            c.analyzed_battles,
        )];
    }
    let Some(b) = b else {
        return vec![item(
            "baseline_building",
            FeedbackTone::Neutral,
            None,
            None,
            c.analyzed_battles,
        )];
    };
    let mut out = Vec::new();
    let damage_ready = c.damage_sample_size >= 3 && b.damage_sample_size >= 5;
    if damage_ready
        && let (Some(a), Some(base)) = (c.average_damage, b.average_damage)
        && base > 0.0
    {
        let r = a / base;
        if r >= 1.1 {
            out.push(item(
                "session_damage_up",
                FeedbackTone::Positive,
                Some(a),
                Some(base),
                c.damage_sample_size,
            ))
        } else if r <= 0.8 {
            out.push(item(
                "session_damage_down",
                FeedbackTone::Warning,
                Some(a),
                Some(base),
                c.damage_sample_size,
            ))
        }
    }
    let survival_ready = c.survival_sample_size >= 3 && b.survival_sample_size >= 5;
    if survival_ready && let (Some(a), Some(base)) = (c.survival_rate, b.survival_rate) {
        if a - base >= 10.0 {
            out.push(item(
                "session_survival_up",
                FeedbackTone::Positive,
                Some(a),
                Some(base),
                c.survival_sample_size,
            ))
        } else if a - base <= -15.0 {
            out.push(item(
                "session_survival_down",
                FeedbackTone::Warning,
                Some(a),
                Some(base),
                c.survival_sample_size,
            ))
        }
    }
    if c.analyzed_battles < 30 {
        out.push(item(
            "winrate_context_only",
            FeedbackTone::Neutral,
            c.win_rate,
            None,
            c.analyzed_battles,
        ))
    }
    if out.is_empty() {
        let evidence = c.damage_sample_size.max(c.survival_sample_size);
        if damage_ready || survival_ready {
            out.push(item(
                "session_stable",
                FeedbackTone::Neutral,
                c.average_damage,
                b.average_damage,
                evidence,
            ))
        } else {
            out.push(item(
                "session_sample_small",
                FeedbackTone::Neutral,
                Some(evidence as f64),
                None,
                evidence,
            ))
        }
    }
    out.truncate(3);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BattleOutcome, SourceKind};
    fn battle(d: Option<i64>, q: DataQuality) -> Battle {
        Battle {
            arena_id: "x".into(),
            arena_unique_id: None,
            battle_time: 0,
            battle_time_source: None,
            vehicle_key: "t".into(),
            wg_tank_id: None,
            tank_name: None,
            map_name: None,
            map_key: None,
            battle_type: None,
            gameplay_id: None,
            source_kind: SourceKind::Replay,
            data_quality: q,
            parser_version: None,
            feature_schema_version: None,
            game_version: None,
            data_completeness: None,
            replay_source: None,
            parser_error_code: None,
            outcome: BattleOutcome::Unknown,
            battle_pace: crate::models::strict_random_pace(),
            player_team: None,
            winner_team_raw: None,
            damage: d,
            assist: Some(0),
            assist_spotting: None,
            assist_tracking: None,
            assist_stun: None,
            blocked: None,
            damage_received: None,
            frags: None,
            spots: None,
            shots: None,
            hits: None,
            direct_enemy_hits: None,
            direct_team_hits: None,
            penetrations: None,
            piercing_enemy_hits: None,
            explosion_hits: None,
            defense_points: None,
            moving_avg_damage: None,
            damage_rating: None,
            survived: None,
            xp: None,
            original_xp: None,
            duration_seconds: None,
            session_id: None,
        }
    }
    #[test]
    fn partial_is_never_judged() {
        assert_eq!(
            battle_feedback(
                &battle(None, DataQuality::Partial),
                &FeedbackBaseline::default()
            )[0]
            .code,
            "battle_partial"
        )
    }
    #[test]
    fn missing_metric_is_not_zero() {
        let base = (0..10)
            .map(|_| battle(Some(1000), DataQuality::Full))
            .collect::<Vec<_>>();
        let r = battle_feedback(
            &battle(None, DataQuality::Full),
            &FeedbackBaseline::from_battles(&base),
        );
        assert!(!r.iter().any(|i| i.tone == FeedbackTone::Warning));
    }
}
