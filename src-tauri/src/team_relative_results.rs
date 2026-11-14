use thiserror::Error;

use crate::{
    error::AppError,
    models::DataQuality,
    storage::{Database, TeamRelativeResultsReadRow},
    team_relative::{
        TeamRelativeCommonReason, TeamRelativeDamageReason, TeamRelativeFragsReason,
        TeamRelativeXpReason,
    },
};

pub(crate) const TEAM_RELATIVE_RESULTS_CONTRACT_VERSION: &str = "team-relative-results-v1";
/// Parser versions whose team-relative facts follow fact schema 1 (contract amendment A1).
/// Version 8 (spec 20, E1) only changes how the battle mode is read; version 9 (E2) only adds the
/// v10 foundations stored next to the facts. The facts are the same as in version 7.
const CURRENT_PARSER_VERSIONS: [i64; 3] = [7, 8, 9];
const CURRENT_FEATURE_SCHEMA_VERSION: i64 = 3;
const CURRENT_FACT_SCHEMA_VERSION: i64 = 1;
const TEAM_SIZE: i64 = 15;
const MAX_DAMAGE: i64 = 200_000;
const MAX_TEAM_DAMAGE: i64 = 3_000_000;
const MAX_FRAGS: i64 = 15;
const MAX_XP: i64 = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TeamRelativeResultsScope {
    Session { session_id: String },
    Recent { window: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TeamRelativeReasonCount {
    pub(crate) code: &'static str,
    pub(crate) battles: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TeamRelativeRankBucket {
    pub(crate) rank: usize,
    pub(crate) battles: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TeamRelativeCommonContextReport {
    pub(crate) eligible_battles: usize,
    pub(crate) ineligible_battles: usize,
    pub(crate) rejection_reasons: Vec<TeamRelativeReasonCount>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TeamRelativeDamageReport {
    pub(crate) selected_battles: usize,
    pub(crate) common_eligible_battles: usize,
    pub(crate) metric_eligible_battles: usize,
    pub(crate) metric_ineligible_battles: usize,
    pub(crate) missing_battles: usize,
    pub(crate) invalid_battles: usize,
    pub(crate) contributing_battles: usize,
    pub(crate) zero_denominator_battles: usize,
    pub(crate) player_damage_sum: i64,
    pub(crate) team_damage_sum: i64,
    pub(crate) share: Option<f64>,
    pub(crate) tied_battles: usize,
    pub(crate) rank_distribution: Vec<TeamRelativeRankBucket>,
    pub(crate) rejection_reasons: Vec<TeamRelativeReasonCount>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TeamRelativeFragsReport {
    pub(crate) selected_battles: usize,
    pub(crate) common_eligible_battles: usize,
    pub(crate) metric_eligible_battles: usize,
    pub(crate) metric_ineligible_battles: usize,
    pub(crate) missing_battles: usize,
    pub(crate) invalid_battles: usize,
    pub(crate) contributing_battles: usize,
    pub(crate) zero_denominator_battles: usize,
    pub(crate) player_frags_sum: i64,
    pub(crate) team_frags_sum: i64,
    pub(crate) share: Option<f64>,
    pub(crate) rejection_reasons: Vec<TeamRelativeReasonCount>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TeamRelativeXpPositionReport {
    pub(crate) selected_battles: usize,
    pub(crate) common_eligible_battles: usize,
    pub(crate) metric_eligible_battles: usize,
    pub(crate) metric_ineligible_battles: usize,
    pub(crate) missing_battles: usize,
    pub(crate) invalid_battles: usize,
    pub(crate) tied_battles: usize,
    pub(crate) rank_distribution: Vec<TeamRelativeRankBucket>,
    pub(crate) rejection_reasons: Vec<TeamRelativeReasonCount>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TeamRelativeResultsReport {
    pub(crate) contract_version: &'static str,
    pub(crate) scope: TeamRelativeResultsScope,
    pub(crate) selected_battles: usize,
    pub(crate) common_context: TeamRelativeCommonContextReport,
    pub(crate) damage: TeamRelativeDamageReport,
    pub(crate) frags: TeamRelativeFragsReport,
    pub(crate) xp_position: TeamRelativeXpPositionReport,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TeamRelativeResultsError {
    #[error("session was not found")]
    SessionNotFound,
    #[error("recent window must be 10, 30, or 100")]
    InvalidWindow,
    #[error("database operation failed")]
    Database,
    #[error("team-relative report invariants were violated")]
    ReportInvariantViolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClassifiedState {
    CommonEligible,
    CommonIneligible(TeamRelativeCommonReason),
}

fn invariant<T>() -> Result<T, TeamRelativeResultsError> {
    Err(TeamRelativeResultsError::ReportInvariantViolation)
}

fn map_storage_error(error: AppError) -> TeamRelativeResultsError {
    match error {
        AppError::SessionNotFound => TeamRelativeResultsError::SessionNotFound,
        _ => TeamRelativeResultsError::Database,
    }
}

fn checked_increment(value: &mut usize) -> Result<(), TeamRelativeResultsError> {
    *value = value
        .checked_add(1)
        .ok_or(TeamRelativeResultsError::ReportInvariantViolation)?;
    Ok(())
}

fn checked_add(total: &mut i64, value: i64) -> Result<(), TeamRelativeResultsError> {
    *total = total
        .checked_add(value)
        .ok_or(TeamRelativeResultsError::ReportInvariantViolation)?;
    Ok(())
}

fn checked_count_sum(
    mut values: impl Iterator<Item = usize>,
) -> Result<usize, TeamRelativeResultsError> {
    values.try_fold(0usize, |total, value| {
        total
            .checked_add(value)
            .ok_or(TeamRelativeResultsError::ReportInvariantViolation)
    })
}

fn common_catalog() -> Vec<TeamRelativeReasonCount> {
    TeamRelativeCommonReason::ALL
        .into_iter()
        .map(|reason| TeamRelativeReasonCount {
            code: reason.as_str(),
            battles: 0,
        })
        .collect()
}

fn metric_catalog(codes: impl Iterator<Item = &'static str>) -> Vec<TeamRelativeReasonCount> {
    let mut catalog = common_catalog();
    catalog.extend(codes.map(|code| TeamRelativeReasonCount { code, battles: 0 }));
    catalog
}

fn rank_catalog() -> Vec<TeamRelativeRankBucket> {
    (1..=TEAM_SIZE as usize)
        .map(|rank| TeamRelativeRankBucket { rank, battles: 0 })
        .collect()
}

fn count_reason(
    catalog: &mut [TeamRelativeReasonCount],
    code: &'static str,
) -> Result<(), TeamRelativeResultsError> {
    let Some(reason) = catalog.iter_mut().find(|reason| reason.code == code) else {
        return invariant();
    };
    checked_increment(&mut reason.battles)
}

fn reason_count(catalog: &[TeamRelativeReasonCount], code: &'static str) -> Option<usize> {
    catalog
        .iter()
        .find(|reason| reason.code == code)
        .map(|reason| reason.battles)
}

fn parse_common_reason(
    value: Option<&str>,
) -> Result<Option<TeamRelativeCommonReason>, TeamRelativeResultsError> {
    value
        .map(|value| {
            TeamRelativeCommonReason::parse(value)
                .ok_or(TeamRelativeResultsError::ReportInvariantViolation)
        })
        .transpose()
}

fn parse_damage_reason(
    value: Option<&str>,
) -> Result<Option<TeamRelativeDamageReason>, TeamRelativeResultsError> {
    value
        .map(|value| {
            TeamRelativeDamageReason::parse(value)
                .ok_or(TeamRelativeResultsError::ReportInvariantViolation)
        })
        .transpose()
}

fn parse_frags_reason(
    value: Option<&str>,
) -> Result<Option<TeamRelativeFragsReason>, TeamRelativeResultsError> {
    value
        .map(|value| {
            TeamRelativeFragsReason::parse(value)
                .ok_or(TeamRelativeResultsError::ReportInvariantViolation)
        })
        .transpose()
}

fn parse_xp_reason(
    value: Option<&str>,
) -> Result<Option<TeamRelativeXpReason>, TeamRelativeResultsError> {
    value
        .map(|value| {
            TeamRelativeXpReason::parse(value)
                .ok_or(TeamRelativeResultsError::ReportInvariantViolation)
        })
        .transpose()
}

fn fact_columns_are_empty(row: &TeamRelativeResultsReadRow) -> bool {
    row.team_player_count.is_none()
        && row.team_damage_total.is_none()
        && row.damage_greater_count.is_none()
        && row.damage_equal_count.is_none()
        && row.damage_reason.is_none()
        && row.team_frags_total.is_none()
        && row.frags_reason.is_none()
        && row.xp_greater_count.is_none()
        && row.xp_equal_count.is_none()
        && row.xp_reason.is_none()
}

fn metric_shapes_are_complete(
    row: &TeamRelativeResultsReadRow,
) -> Result<bool, TeamRelativeResultsError> {
    let damage = match parse_damage_reason(row.damage_reason.as_deref())? {
        None => {
            row.team_damage_total.is_some()
                && row.damage_greater_count.is_some()
                && row.damage_equal_count.is_some()
        }
        Some(_) => {
            row.team_damage_total.is_none()
                && row.damage_greater_count.is_none()
                && row.damage_equal_count.is_none()
        }
    };
    let frags = match parse_frags_reason(row.frags_reason.as_deref())? {
        None => row.team_frags_total.is_some(),
        Some(_) => row.team_frags_total.is_none(),
    };
    let xp = match parse_xp_reason(row.xp_reason.as_deref())? {
        None => row.xp_greater_count.is_some() && row.xp_equal_count.is_some(),
        Some(_) => row.xp_greater_count.is_none() && row.xp_equal_count.is_none(),
    };
    Ok(damage && frags && xp)
}

fn no_row_state_is_empty(row: &TeamRelativeResultsReadRow) -> bool {
    row.fact_schema_version.is_none() && row.common_reason.is_none() && fact_columns_are_empty(row)
}

fn is_known_legacy_feature_schema(value: Option<i64>) -> bool {
    matches!(value, None | Some(1..=3))
}

fn classify_state(
    row: &TeamRelativeResultsReadRow,
) -> Result<ClassifiedState, TeamRelativeResultsError> {
    if !row.facts_row_present {
        if !no_row_state_is_empty(row) {
            return invariant();
        }
        let legacy_parser = matches!(row.parser_version, None | Some(1..=6));
        if legacy_parser && is_known_legacy_feature_schema(row.feature_schema_version) {
            return Ok(ClassifiedState::CommonIneligible(
                TeamRelativeCommonReason::ParserTooOld,
            ));
        }
        return invariant();
    }

    if row.fact_schema_version != Some(CURRENT_FACT_SCHEMA_VERSION)
        || !row
            .parser_version
            .is_some_and(|version| CURRENT_PARSER_VERSIONS.contains(&version))
    {
        return invariant();
    }

    match row.data_quality {
        DataQuality::Full if row.feature_schema_version == Some(CURRENT_FEATURE_SCHEMA_VERSION) => {
            match parse_common_reason(row.common_reason.as_deref())? {
                None => {
                    if row.team_player_count != Some(TEAM_SIZE) || !metric_shapes_are_complete(row)?
                    {
                        return invariant();
                    }
                    Ok(ClassifiedState::CommonEligible)
                }
                Some(TeamRelativeCommonReason::PartialReplay) => invariant(),
                Some(reason) => {
                    if !fact_columns_are_empty(row) {
                        return invariant();
                    }
                    Ok(ClassifiedState::CommonIneligible(reason))
                }
            }
        }
        DataQuality::Partial if row.feature_schema_version.is_none() => {
            if row.common_reason.as_deref()
                != Some(TeamRelativeCommonReason::PartialReplay.as_str())
                || !fact_columns_are_empty(row)
            {
                return invariant();
            }
            Ok(ClassifiedState::CommonIneligible(
                TeamRelativeCommonReason::PartialReplay,
            ))
        }
        DataQuality::Full | DataQuality::Partial | DataQuality::Ambiguous => invariant(),
    }
}

fn empty_common() -> TeamRelativeCommonContextReport {
    TeamRelativeCommonContextReport {
        eligible_battles: 0,
        ineligible_battles: 0,
        rejection_reasons: common_catalog(),
    }
}

fn empty_damage(selected_battles: usize) -> TeamRelativeDamageReport {
    TeamRelativeDamageReport {
        selected_battles,
        common_eligible_battles: 0,
        metric_eligible_battles: 0,
        metric_ineligible_battles: 0,
        missing_battles: 0,
        invalid_battles: 0,
        contributing_battles: 0,
        zero_denominator_battles: 0,
        player_damage_sum: 0,
        team_damage_sum: 0,
        share: None,
        tied_battles: 0,
        rank_distribution: rank_catalog(),
        rejection_reasons: metric_catalog(
            TeamRelativeDamageReason::ALL
                .into_iter()
                .map(TeamRelativeDamageReason::as_str),
        ),
    }
}

fn empty_frags(selected_battles: usize) -> TeamRelativeFragsReport {
    TeamRelativeFragsReport {
        selected_battles,
        common_eligible_battles: 0,
        metric_eligible_battles: 0,
        metric_ineligible_battles: 0,
        missing_battles: 0,
        invalid_battles: 0,
        contributing_battles: 0,
        zero_denominator_battles: 0,
        player_frags_sum: 0,
        team_frags_sum: 0,
        share: None,
        rejection_reasons: metric_catalog(
            TeamRelativeFragsReason::ALL
                .into_iter()
                .map(TeamRelativeFragsReason::as_str),
        ),
    }
}

fn empty_xp(selected_battles: usize) -> TeamRelativeXpPositionReport {
    TeamRelativeXpPositionReport {
        selected_battles,
        common_eligible_battles: 0,
        metric_eligible_battles: 0,
        metric_ineligible_battles: 0,
        missing_battles: 0,
        invalid_battles: 0,
        tied_battles: 0,
        rank_distribution: rank_catalog(),
        rejection_reasons: metric_catalog(
            TeamRelativeXpReason::ALL
                .into_iter()
                .map(TeamRelativeXpReason::as_str),
        ),
    }
}

fn reject_damage(
    report: &mut TeamRelativeDamageReport,
    code: &'static str,
    missing: bool,
    invalid: bool,
) -> Result<(), TeamRelativeResultsError> {
    checked_increment(&mut report.metric_ineligible_battles)?;
    if missing {
        checked_increment(&mut report.missing_battles)?;
    }
    if invalid {
        checked_increment(&mut report.invalid_battles)?;
    }
    count_reason(&mut report.rejection_reasons, code)
}

fn reject_frags(
    report: &mut TeamRelativeFragsReport,
    code: &'static str,
    missing: bool,
    invalid: bool,
) -> Result<(), TeamRelativeResultsError> {
    checked_increment(&mut report.metric_ineligible_battles)?;
    if missing {
        checked_increment(&mut report.missing_battles)?;
    }
    if invalid {
        checked_increment(&mut report.invalid_battles)?;
    }
    count_reason(&mut report.rejection_reasons, code)
}

fn reject_xp(
    report: &mut TeamRelativeXpPositionReport,
    code: &'static str,
    missing: bool,
    invalid: bool,
) -> Result<(), TeamRelativeResultsError> {
    checked_increment(&mut report.metric_ineligible_battles)?;
    if missing {
        checked_increment(&mut report.missing_battles)?;
    }
    if invalid {
        checked_increment(&mut report.invalid_battles)?;
    }
    count_reason(&mut report.rejection_reasons, code)
}

fn increment_rank(
    distribution: &mut [TeamRelativeRankBucket],
    rank: i64,
) -> Result<(), TeamRelativeResultsError> {
    let Ok(index) = usize::try_from(rank - 1) else {
        return invariant();
    };
    let Some(bucket) = distribution.get_mut(index) else {
        return invariant();
    };
    checked_increment(&mut bucket.battles)
}

fn aggregate_damage(
    report: &mut TeamRelativeDamageReport,
    row: &TeamRelativeResultsReadRow,
) -> Result<(), TeamRelativeResultsError> {
    if let Some(reason) = parse_damage_reason(row.damage_reason.as_deref())? {
        return reject_damage(
            report,
            reason.as_str(),
            reason == TeamRelativeDamageReason::Missing,
            reason != TeamRelativeDamageReason::Missing,
        );
    }
    let (Some(player), Some(team), Some(greater), Some(equal)) = (
        row.damage,
        row.team_damage_total,
        row.damage_greater_count,
        row.damage_equal_count,
    ) else {
        return invariant();
    };
    if !(0..=MAX_DAMAGE).contains(&player)
        || !(0..=MAX_TEAM_DAMAGE).contains(&team)
        || !(0..=14).contains(&greater)
        || !(1..=15).contains(&equal)
        || greater
            .checked_add(equal)
            .is_none_or(|value| value > TEAM_SIZE)
        || player > team
    {
        return invariant();
    }
    if team == 0 && (player != 0 || greater != 0 || equal != TEAM_SIZE) {
        return invariant();
    }

    checked_increment(&mut report.metric_eligible_battles)?;
    let rank = greater + 1;
    increment_rank(&mut report.rank_distribution, rank)?;
    if equal > 1 {
        checked_increment(&mut report.tied_battles)?;
    }
    if team == 0 {
        checked_increment(&mut report.zero_denominator_battles)?;
    } else {
        checked_increment(&mut report.contributing_battles)?;
        checked_add(&mut report.player_damage_sum, player)?;
        checked_add(&mut report.team_damage_sum, team)?;
    }
    Ok(())
}

fn aggregate_frags(
    report: &mut TeamRelativeFragsReport,
    row: &TeamRelativeResultsReadRow,
) -> Result<(), TeamRelativeResultsError> {
    if let Some(reason) = parse_frags_reason(row.frags_reason.as_deref())? {
        return reject_frags(
            report,
            reason.as_str(),
            reason == TeamRelativeFragsReason::Missing,
            reason != TeamRelativeFragsReason::Missing,
        );
    }
    let (Some(player), Some(team)) = (row.frags, row.team_frags_total) else {
        return invariant();
    };
    if !(0..=MAX_FRAGS).contains(&player) || !(0..=MAX_FRAGS).contains(&team) || player > team {
        return invariant();
    }

    checked_increment(&mut report.metric_eligible_battles)?;
    if team == 0 {
        if player != 0 {
            return invariant();
        }
        checked_increment(&mut report.zero_denominator_battles)?;
    } else {
        checked_increment(&mut report.contributing_battles)?;
        checked_add(&mut report.player_frags_sum, player)?;
        checked_add(&mut report.team_frags_sum, team)?;
    }
    Ok(())
}

fn aggregate_xp(
    report: &mut TeamRelativeXpPositionReport,
    row: &TeamRelativeResultsReadRow,
) -> Result<(), TeamRelativeResultsError> {
    if let Some(reason) = parse_xp_reason(row.xp_reason.as_deref())? {
        return reject_xp(
            report,
            reason.as_str(),
            reason == TeamRelativeXpReason::Missing,
            reason != TeamRelativeXpReason::Missing,
        );
    }
    let (Some(player), Some(greater), Some(equal)) =
        (row.original_xp, row.xp_greater_count, row.xp_equal_count)
    else {
        return invariant();
    };
    if !(0..=MAX_XP).contains(&player)
        || !(0..=14).contains(&greater)
        || !(1..=15).contains(&equal)
        || greater
            .checked_add(equal)
            .is_none_or(|value| value > TEAM_SIZE)
    {
        return invariant();
    }

    checked_increment(&mut report.metric_eligible_battles)?;
    increment_rank(&mut report.rank_distribution, greater + 1)?;
    if equal > 1 {
        checked_increment(&mut report.tied_battles)?;
    }
    Ok(())
}

fn ratio(numerator: i64, denominator: i64) -> Result<Option<f64>, TeamRelativeResultsError> {
    if denominator == 0 {
        if numerator != 0 {
            return invariant();
        }
        return Ok(None);
    }
    if numerator < 0 || denominator < 0 || numerator > denominator {
        return invariant();
    }
    let value = numerator as f64 / denominator as f64;
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return invariant();
    }
    Ok(Some(value))
}

fn catalog_is_exact(
    actual: &[TeamRelativeReasonCount],
    expected: impl Iterator<Item = &'static str>,
) -> bool {
    actual.iter().map(|reason| reason.code).eq(expected)
}

fn rank_catalog_is_exact(actual: &[TeamRelativeRankBucket]) -> bool {
    actual.len() == TEAM_SIZE as usize
        && actual
            .iter()
            .enumerate()
            .all(|(index, bucket)| bucket.rank == index + 1)
}

fn validate_report(report: &TeamRelativeResultsReport) -> Result<(), TeamRelativeResultsError> {
    if report.contract_version != TEAM_RELATIVE_RESULTS_CONTRACT_VERSION
        || report
            .common_context
            .eligible_battles
            .checked_add(report.common_context.ineligible_battles)
            != Some(report.selected_battles)
        || checked_count_sum(
            report
                .common_context
                .rejection_reasons
                .iter()
                .map(|reason| reason.battles),
        )? != report.common_context.ineligible_battles
        || !catalog_is_exact(
            &report.common_context.rejection_reasons,
            TeamRelativeCommonReason::ALL
                .into_iter()
                .map(TeamRelativeCommonReason::as_str),
        )
    {
        return invariant();
    }

    let common_codes = TeamRelativeCommonReason::ALL
        .into_iter()
        .map(TeamRelativeCommonReason::as_str);
    if report.damage.selected_battles != report.selected_battles
        || report.damage.common_eligible_battles != report.common_context.eligible_battles
        || report
            .damage
            .metric_eligible_battles
            .checked_add(report.damage.metric_ineligible_battles)
            != Some(report.selected_battles)
        || report
            .damage
            .contributing_battles
            .checked_add(report.damage.zero_denominator_battles)
            != Some(report.damage.metric_eligible_battles)
        || checked_count_sum(
            report
                .damage
                .rejection_reasons
                .iter()
                .map(|reason| reason.battles),
        )? != report.damage.metric_ineligible_battles
        || report.damage.missing_battles
            != reason_count(
                &report.damage.rejection_reasons,
                TeamRelativeDamageReason::Missing.as_str(),
            )
            .ok_or(TeamRelativeResultsError::ReportInvariantViolation)?
        || report.damage.invalid_battles
            != reason_count(
                &report.damage.rejection_reasons,
                TeamRelativeDamageReason::OutOfRange.as_str(),
            )
            .and_then(|value| {
                reason_count(
                    &report.damage.rejection_reasons,
                    TeamRelativeDamageReason::Conflict.as_str(),
                )
                .and_then(|conflict| value.checked_add(conflict))
            })
            .ok_or(TeamRelativeResultsError::ReportInvariantViolation)?
        || checked_count_sum(
            report
                .damage
                .rank_distribution
                .iter()
                .map(|bucket| bucket.battles),
        )? != report.damage.metric_eligible_battles
        || report.damage.tied_battles > report.damage.metric_eligible_battles
        || !rank_catalog_is_exact(&report.damage.rank_distribution)
        || !catalog_is_exact(
            &report.damage.rejection_reasons,
            common_codes.chain(
                TeamRelativeDamageReason::ALL
                    .into_iter()
                    .map(TeamRelativeDamageReason::as_str),
            ),
        )
        || report.damage.player_damage_sum > report.damage.team_damage_sum
        || report.damage.share
            != ratio(
                report.damage.player_damage_sum,
                report.damage.team_damage_sum,
            )?
    {
        return invariant();
    }

    let common_codes = TeamRelativeCommonReason::ALL
        .into_iter()
        .map(TeamRelativeCommonReason::as_str);
    if report.frags.selected_battles != report.selected_battles
        || report.frags.common_eligible_battles != report.common_context.eligible_battles
        || report
            .frags
            .metric_eligible_battles
            .checked_add(report.frags.metric_ineligible_battles)
            != Some(report.selected_battles)
        || report
            .frags
            .contributing_battles
            .checked_add(report.frags.zero_denominator_battles)
            != Some(report.frags.metric_eligible_battles)
        || checked_count_sum(
            report
                .frags
                .rejection_reasons
                .iter()
                .map(|reason| reason.battles),
        )? != report.frags.metric_ineligible_battles
        || report.frags.missing_battles
            != reason_count(
                &report.frags.rejection_reasons,
                TeamRelativeFragsReason::Missing.as_str(),
            )
            .ok_or(TeamRelativeResultsError::ReportInvariantViolation)?
        || report.frags.invalid_battles
            != reason_count(
                &report.frags.rejection_reasons,
                TeamRelativeFragsReason::OutOfRange.as_str(),
            )
            .and_then(|value| {
                reason_count(
                    &report.frags.rejection_reasons,
                    TeamRelativeFragsReason::Conflict.as_str(),
                )
                .and_then(|conflict| value.checked_add(conflict))
            })
            .ok_or(TeamRelativeResultsError::ReportInvariantViolation)?
        || !catalog_is_exact(
            &report.frags.rejection_reasons,
            common_codes.chain(
                TeamRelativeFragsReason::ALL
                    .into_iter()
                    .map(TeamRelativeFragsReason::as_str),
            ),
        )
        || report.frags.player_frags_sum > report.frags.team_frags_sum
        || report.frags.share != ratio(report.frags.player_frags_sum, report.frags.team_frags_sum)?
    {
        return invariant();
    }

    let common_codes = TeamRelativeCommonReason::ALL
        .into_iter()
        .map(TeamRelativeCommonReason::as_str);
    if report.xp_position.selected_battles != report.selected_battles
        || report.xp_position.common_eligible_battles != report.common_context.eligible_battles
        || report
            .xp_position
            .metric_eligible_battles
            .checked_add(report.xp_position.metric_ineligible_battles)
            != Some(report.selected_battles)
        || checked_count_sum(
            report
                .xp_position
                .rejection_reasons
                .iter()
                .map(|reason| reason.battles),
        )? != report.xp_position.metric_ineligible_battles
        || report.xp_position.missing_battles
            != reason_count(
                &report.xp_position.rejection_reasons,
                TeamRelativeXpReason::Missing.as_str(),
            )
            .ok_or(TeamRelativeResultsError::ReportInvariantViolation)?
        || report.xp_position.invalid_battles
            != reason_count(
                &report.xp_position.rejection_reasons,
                TeamRelativeXpReason::OutOfRange.as_str(),
            )
            .and_then(|value| {
                reason_count(
                    &report.xp_position.rejection_reasons,
                    TeamRelativeXpReason::Conflict.as_str(),
                )
                .and_then(|conflict| value.checked_add(conflict))
            })
            .ok_or(TeamRelativeResultsError::ReportInvariantViolation)?
        || checked_count_sum(
            report
                .xp_position
                .rank_distribution
                .iter()
                .map(|bucket| bucket.battles),
        )? != report.xp_position.metric_eligible_battles
        || report.xp_position.tied_battles > report.xp_position.metric_eligible_battles
        || !rank_catalog_is_exact(&report.xp_position.rank_distribution)
        || !catalog_is_exact(
            &report.xp_position.rejection_reasons,
            common_codes.chain(
                TeamRelativeXpReason::ALL
                    .into_iter()
                    .map(TeamRelativeXpReason::as_str),
            ),
        )
    {
        return invariant();
    }
    Ok(())
}

pub(crate) fn analyze_team_relative_results(
    scope: TeamRelativeResultsScope,
    rows: &[TeamRelativeResultsReadRow],
) -> Result<TeamRelativeResultsReport, TeamRelativeResultsError> {
    let selected_battles = rows.len();
    let mut report = TeamRelativeResultsReport {
        contract_version: TEAM_RELATIVE_RESULTS_CONTRACT_VERSION,
        scope,
        selected_battles,
        common_context: empty_common(),
        damage: empty_damage(selected_battles),
        frags: empty_frags(selected_battles),
        xp_position: empty_xp(selected_battles),
    };

    for row in rows {
        match classify_state(row)? {
            ClassifiedState::CommonEligible => {
                checked_increment(&mut report.common_context.eligible_battles)?;
                checked_increment(&mut report.damage.common_eligible_battles)?;
                checked_increment(&mut report.frags.common_eligible_battles)?;
                checked_increment(&mut report.xp_position.common_eligible_battles)?;
                aggregate_damage(&mut report.damage, row)?;
                aggregate_frags(&mut report.frags, row)?;
                aggregate_xp(&mut report.xp_position, row)?;
            }
            ClassifiedState::CommonIneligible(reason) => {
                checked_increment(&mut report.common_context.ineligible_battles)?;
                count_reason(
                    &mut report.common_context.rejection_reasons,
                    reason.as_str(),
                )?;
                reject_damage(&mut report.damage, reason.as_str(), false, false)?;
                reject_frags(&mut report.frags, reason.as_str(), false, false)?;
                reject_xp(&mut report.xp_position, reason.as_str(), false, false)?;
            }
        }
    }

    report.damage.share = ratio(
        report.damage.player_damage_sum,
        report.damage.team_damage_sum,
    )?;
    report.frags.share = ratio(report.frags.player_frags_sum, report.frags.team_frags_sum)?;
    validate_report(&report)?;
    Ok(report)
}

pub(crate) fn team_relative_session_report(
    database: &Database,
    session_id: &str,
) -> Result<TeamRelativeResultsReport, TeamRelativeResultsError> {
    let rows = database
        .team_relative_results_session_rows(session_id)
        .map_err(map_storage_error)?;
    analyze_team_relative_results(
        TeamRelativeResultsScope::Session {
            session_id: session_id.to_owned(),
        },
        &rows,
    )
}

pub(crate) fn team_relative_recent_report(
    database: &Database,
    window: usize,
) -> Result<TeamRelativeResultsReport, TeamRelativeResultsError> {
    if !matches!(window, 10 | 30 | 100) {
        return Err(TeamRelativeResultsError::InvalidWindow);
    }
    let rows = database
        .team_relative_results_recent_rows(window)
        .map_err(map_storage_error)?;
    analyze_team_relative_results(TeamRelativeResultsScope::Recent { window }, &rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        models::{Battle, BattleOutcome, SourceKind},
        storage::ReplayFileState,
        team_relative::TeamRelativeFactsV1,
    };
    use tempfile::tempdir;

    fn full_row(id: &str) -> TeamRelativeResultsReadRow {
        TeamRelativeResultsReadRow {
            battle_key: id.into(),
            battle_time: 1,
            data_quality: DataQuality::Full,
            parser_version: Some(7),
            feature_schema_version: Some(3),
            facts_row_present: true,
            fact_schema_version: Some(1),
            damage: Some(3_000),
            frags: Some(2),
            original_xp: Some(1_000),
            common_reason: None,
            team_player_count: Some(15),
            team_damage_total: Some(15_000),
            damage_greater_count: Some(1),
            damage_equal_count: Some(1),
            damage_reason: None,
            team_frags_total: Some(5),
            frags_reason: None,
            xp_greater_count: Some(2),
            xp_equal_count: Some(1),
            xp_reason: None,
        }
    }

    fn partial_row(id: &str) -> TeamRelativeResultsReadRow {
        TeamRelativeResultsReadRow {
            battle_key: id.into(),
            battle_time: 1,
            data_quality: DataQuality::Partial,
            parser_version: Some(7),
            feature_schema_version: None,
            facts_row_present: true,
            fact_schema_version: Some(1),
            damage: None,
            frags: None,
            original_xp: None,
            common_reason: Some("partial_replay".into()),
            team_player_count: None,
            team_damage_total: None,
            damage_greater_count: None,
            damage_equal_count: None,
            damage_reason: None,
            team_frags_total: None,
            frags_reason: None,
            xp_greater_count: None,
            xp_equal_count: None,
            xp_reason: None,
        }
    }

    fn legacy_row(id: &str, quality: DataQuality) -> TeamRelativeResultsReadRow {
        let mut row = partial_row(id);
        row.data_quality = quality;
        row.parser_version = Some(6);
        row.feature_schema_version = Some(2);
        row.facts_row_present = false;
        row.fact_schema_version = None;
        row.common_reason = None;
        row
    }

    fn report(
        rows: &[TeamRelativeResultsReadRow],
    ) -> Result<TeamRelativeResultsReport, TeamRelativeResultsError> {
        analyze_team_relative_results(TeamRelativeResultsScope::Recent { window: 10 }, rows)
    }

    fn count(catalog: &[TeamRelativeReasonCount], code: &'static str) -> usize {
        reason_count(catalog, code).unwrap()
    }

    fn stored_battle(
        id: &str,
        battle_time: i64,
        quality: DataQuality,
        session_id: Option<String>,
    ) -> Battle {
        Battle {
            arena_id: id.into(),
            arena_unique_id: None,
            battle_time,
            battle_time_source: Some("metadata".into()),
            vehicle_key: "synthetic:tank".into(),
            wg_tank_id: Some(1),
            tank_name: Some("Synthetic tank".into()),
            map_name: Some("Synthetic map".into()),
            map_key: Some("synthetic_map".into()),
            battle_type: Some("random".into()),
            gameplay_id: Some("ctf".into()),
            source_kind: SourceKind::Replay,
            data_quality: quality,
            parser_version: Some(7),
            feature_schema_version: Some(3),
            game_version: Some("test".into()),
            data_completeness: Some("results".into()),
            replay_source: Some("replay_results".into()),
            parser_error_code: None,
            outcome: BattleOutcome::Victory,
            battle_pace: crate::models::strict_random_pace(),
            player_team: Some(1),
            winner_team_raw: Some(1),
            damage: Some(3_000),
            assist: None,
            assist_spotting: None,
            assist_tracking: None,
            assist_stun: None,
            blocked: None,
            damage_received: None,
            frags: Some(2),
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
            xp: Some(1_000),
            original_xp: Some(1_000),
            duration_seconds: None,
            session_id,
        }
    }

    fn replay_state(id: &str) -> ReplayFileState {
        ReplayFileState {
            source_path_key: format!("source-{id}"),
            file_size: 1,
            file_mtime_ms: 1,
            content_fingerprint: format!("fingerprint-{id}"),
            block_count: 2,
        }
    }

    fn eligible_facts() -> TeamRelativeFactsV1 {
        TeamRelativeFactsV1 {
            common_reason: None,
            team_player_count: Some(15),
            team_damage_total: Some(15_000),
            damage_greater_count: Some(1),
            damage_equal_count: Some(1),
            damage_reason: None,
            team_frags_total: Some(5),
            frags_reason: None,
            xp_greater_count: Some(2),
            xp_equal_count: Some(1),
            xp_reason: None,
        }
    }

    fn insert(database: &Database, mut battle: Battle, facts: Option<&TeamRelativeFactsV1>) {
        let state = replay_state(&battle.arena_id);
        database
            .upsert_replay_with_team_relative_facts(&mut battle, &state, false, facts)
            .unwrap();
    }

    #[test]
    fn legal_full_partial_and_legacy_states_have_exact_independent_coverage() {
        let actual = report(&[
            full_row("full"),
            partial_row("partial"),
            legacy_row("legacy", DataQuality::Full),
        ])
        .unwrap();
        assert_eq!(actual.selected_battles, 3);
        assert_eq!(actual.common_context.eligible_battles, 1);
        assert_eq!(actual.common_context.ineligible_battles, 2);
        assert_eq!(
            count(&actual.common_context.rejection_reasons, "partial_replay"),
            1
        );
        assert_eq!(
            count(&actual.common_context.rejection_reasons, "parser_too_old"),
            1
        );
        for catalog in [
            &actual.damage.rejection_reasons,
            &actual.frags.rejection_reasons,
            &actual.xp_position.rejection_reasons,
        ] {
            assert_eq!(count(catalog, "partial_replay"), 1);
            assert_eq!(count(catalog, "parser_too_old"), 1);
        }
        assert_eq!(actual.damage.missing_battles, 0);
        assert_eq!(actual.damage.invalid_battles, 0);
        assert_eq!(actual.frags.missing_battles, 0);
        assert_eq!(actual.frags.invalid_battles, 0);
        assert_eq!(actual.xp_position.missing_battles, 0);
        assert_eq!(actual.xp_position.invalid_battles, 0);
    }

    #[test]
    fn contract_amendment_a1_accepts_parsers_seven_eight_and_nine_as_current() {
        for version in [7, 8, 9] {
            let mut full = full_row("full");
            full.parser_version = Some(version);
            let mut partial = partial_row("partial");
            partial.parser_version = Some(version);
            let actual = report(&[full, partial]).unwrap();
            assert_eq!(actual.common_context.eligible_battles, 1, "{version}");
            assert_eq!(
                count(&actual.common_context.rejection_reasons, "partial_replay"),
                1,
                "{version}"
            );
        }
    }

    #[test]
    fn full_common_ineligible_is_legal_but_partial_reason_is_not() {
        let mut unsupported = full_row("unsupported");
        unsupported.common_reason = Some("unsupported_mode".into());
        unsupported.team_player_count = None;
        unsupported.team_damage_total = None;
        unsupported.damage_greater_count = None;
        unsupported.damage_equal_count = None;
        unsupported.team_frags_total = None;
        unsupported.xp_greater_count = None;
        unsupported.xp_equal_count = None;
        let actual = report(&[unsupported]).unwrap();
        assert_eq!(actual.common_context.ineligible_battles, 1);
        assert_eq!(
            count(&actual.common_context.rejection_reasons, "unsupported_mode"),
            1
        );

        let mut invalid = full_row("full-partial");
        invalid.common_reason = Some("partial_replay".into());
        invalid.team_player_count = None;
        invalid.team_damage_total = None;
        invalid.damage_greater_count = None;
        invalid.damage_equal_count = None;
        invalid.team_frags_total = None;
        invalid.xp_greater_count = None;
        invalid.xp_equal_count = None;
        assert_eq!(
            report(&[invalid]),
            Err(TeamRelativeResultsError::ReportInvariantViolation)
        );
    }

    #[test]
    fn every_illegal_partial_or_version_state_fails_closed() {
        let mut cases = Vec::new();

        let mut missing_row = partial_row("missing-row");
        missing_row.facts_row_present = false;
        missing_row.fact_schema_version = None;
        missing_row.common_reason = None;
        cases.push(missing_row);

        let mut schema_three = partial_row("schema-three");
        schema_three.feature_schema_version = Some(3);
        cases.push(schema_three);

        let mut metric_fact = partial_row("metric-fact");
        metric_fact.team_damage_total = Some(0);
        cases.push(metric_fact);

        let mut metric_reason = partial_row("metric-reason");
        metric_reason.damage_reason = Some("damage_missing".into());
        cases.push(metric_reason);

        let mut wrong_reason = partial_row("wrong-reason");
        wrong_reason.common_reason = Some("unsupported_mode".into());
        cases.push(wrong_reason);

        let mut team_count = partial_row("team-count");
        team_count.team_player_count = Some(15);
        cases.push(team_count);

        let mut parser_six = partial_row("parser-six");
        parser_six.parser_version = Some(6);
        cases.push(parser_six);

        let mut fact_two = partial_row("fact-two");
        fact_two.fact_schema_version = Some(2);
        cases.push(fact_two);

        let mut parser_ten = partial_row("parser-ten");
        parser_ten.parser_version = Some(10);
        cases.push(parser_ten);

        for case in cases {
            let id = case.battle_key.clone();
            assert_eq!(
                report(&[case]),
                Err(TeamRelativeResultsError::ReportInvariantViolation),
                "{id}"
            );
        }
    }

    #[test]
    fn every_illegal_full_or_missing_row_state_fails_closed() {
        let mut cases = Vec::new();

        let mut missing_row = full_row("missing-row");
        missing_row.facts_row_present = false;
        missing_row.fact_schema_version = None;
        missing_row.common_reason = None;
        missing_row.team_player_count = None;
        missing_row.team_damage_total = None;
        missing_row.damage_greater_count = None;
        missing_row.damage_equal_count = None;
        missing_row.team_frags_total = None;
        missing_row.xp_greater_count = None;
        missing_row.xp_equal_count = None;
        cases.push(missing_row);

        let mut null_schema = full_row("null-schema");
        null_schema.feature_schema_version = None;
        cases.push(null_schema);

        let mut schema_four = full_row("schema-four");
        schema_four.feature_schema_version = Some(4);
        cases.push(schema_four);

        let mut parser_six_with_row = full_row("parser-six");
        parser_six_with_row.parser_version = Some(6);
        cases.push(parser_six_with_row);

        let mut fact_two = full_row("fact-two");
        fact_two.fact_schema_version = Some(2);
        cases.push(fact_two);

        let mut ambiguous = full_row("ambiguous");
        ambiguous.data_quality = DataQuality::Ambiguous;
        cases.push(ambiguous);

        for case in cases {
            let id = case.battle_key.clone();
            assert_eq!(
                report(&[case]),
                Err(TeamRelativeResultsError::ReportInvariantViolation),
                "{id}"
            );
        }
    }

    #[test]
    fn legacy_full_and_partial_without_rows_are_parser_too_old() {
        for quality in [DataQuality::Full, DataQuality::Partial] {
            let actual = report(&[legacy_row("legacy", quality)]).unwrap();
            assert_eq!(actual.common_context.ineligible_battles, 1);
            assert_eq!(
                count(&actual.common_context.rejection_reasons, "parser_too_old"),
                1
            );
            assert_eq!(actual.damage.missing_battles, 0);
            assert_eq!(actual.frags.missing_battles, 0);
            assert_eq!(actual.xp_position.missing_battles, 0);
        }
    }

    #[test]
    fn damage_frags_and_xp_golden_values_use_exact_ranks_and_ties() {
        let actual = report(&[full_row("golden")]).unwrap();
        assert_eq!(actual.damage.share, Some(0.2));
        assert_eq!(actual.damage.rank_distribution[1].battles, 1);
        assert_eq!(actual.damage.tied_battles, 0);
        assert_eq!(actual.frags.share, Some(0.4));
        assert_eq!(actual.xp_position.rank_distribution[2].battles, 1);

        let mut tied = full_row("ties");
        tied.damage_greater_count = Some(1);
        tied.damage_equal_count = Some(2);
        tied.xp_greater_count = Some(2);
        tied.xp_equal_count = Some(3);
        let actual = report(&[tied]).unwrap();
        assert_eq!(actual.damage.rank_distribution[1].battles, 1);
        assert_eq!(actual.damage.tied_battles, 1);
        assert_eq!(actual.xp_position.rank_distribution[2].battles, 1);
        assert_eq!(actual.xp_position.tied_battles, 1);
    }

    #[test]
    fn zero_numerators_and_denominators_remain_distinct() {
        let mut zero_percent = full_row("zero-percent");
        zero_percent.damage = Some(0);
        zero_percent.frags = Some(0);
        let actual = report(&[zero_percent]).unwrap();
        assert_eq!(actual.damage.share, Some(0.0));
        assert_eq!(actual.frags.share, Some(0.0));

        let mut zero_denominator = full_row("zero-denominator");
        zero_denominator.damage = Some(0);
        zero_denominator.team_damage_total = Some(0);
        zero_denominator.damage_greater_count = Some(0);
        zero_denominator.damage_equal_count = Some(15);
        zero_denominator.frags = Some(0);
        zero_denominator.team_frags_total = Some(0);
        let actual = report(&[zero_denominator]).unwrap();
        assert_eq!(actual.damage.share, None);
        assert_eq!(actual.damage.zero_denominator_battles, 1);
        assert_eq!(actual.damage.rank_distribution[0].battles, 1);
        assert_eq!(actual.damage.tied_battles, 1);
        assert_eq!(actual.frags.share, None);
        assert_eq!(actual.frags.zero_denominator_battles, 1);
    }

    #[test]
    fn metric_failures_are_independent_and_have_exact_missing_invalid_counts() {
        let mut value = full_row("metric-failures");
        value.damage = None;
        value.team_damage_total = None;
        value.damage_greater_count = None;
        value.damage_equal_count = None;
        value.damage_reason = Some("damage_missing".into());
        value.team_frags_total = None;
        value.frags_reason = Some("frags_conflict".into());
        value.xp_greater_count = None;
        value.xp_equal_count = None;
        value.xp_reason = Some("xp_out_of_range".into());
        let actual = report(&[value]).unwrap();
        assert_eq!(actual.damage.missing_battles, 1);
        assert_eq!(actual.damage.invalid_battles, 0);
        assert_eq!(actual.frags.missing_battles, 0);
        assert_eq!(actual.frags.invalid_battles, 1);
        assert_eq!(actual.xp_position.missing_battles, 0);
        assert_eq!(actual.xp_position.invalid_battles, 1);
    }

    #[test]
    fn malformed_success_and_failure_shapes_are_invariant_violations() {
        let mut missing_success_fact = full_row("missing-success");
        missing_success_fact.team_damage_total = None;
        assert_eq!(
            report(&[missing_success_fact]),
            Err(TeamRelativeResultsError::ReportInvariantViolation)
        );

        let mut retained_failure_fact = full_row("retained-failure");
        retained_failure_fact.damage_reason = Some("damage_missing".into());
        assert_eq!(
            report(&[retained_failure_fact]),
            Err(TeamRelativeResultsError::ReportInvariantViolation)
        );

        let mut rank_conflict = full_row("rank-conflict");
        rank_conflict.damage_greater_count = Some(14);
        rank_conflict.damage_equal_count = Some(2);
        assert_eq!(
            report(&[rank_conflict]),
            Err(TeamRelativeResultsError::ReportInvariantViolation)
        );
    }

    #[test]
    fn player_totals_cannot_exceed_team_totals() {
        let mut damage = full_row("damage");
        damage.damage = Some(15_001);
        damage.team_damage_total = Some(15_000);
        assert_eq!(
            report(&[damage]),
            Err(TeamRelativeResultsError::ReportInvariantViolation)
        );

        let mut frags = full_row("frags");
        frags.frags = Some(6);
        frags.team_frags_total = Some(5);
        assert_eq!(
            report(&[frags]),
            Err(TeamRelativeResultsError::ReportInvariantViolation)
        );
    }

    #[test]
    fn aggregate_share_is_ratio_of_sums_and_order_independent() {
        let mut first = full_row("first");
        first.damage = Some(1);
        first.team_damage_total = Some(2);
        first.frags = Some(1);
        first.team_frags_total = Some(2);
        let mut second = full_row("second");
        second.damage = Some(1);
        second.team_damage_total = Some(8);
        second.frags = Some(1);
        second.team_frags_total = Some(8);
        let forward = report(&[first.clone(), second.clone()]).unwrap();
        let reverse = report(&[second, first]).unwrap();
        assert_eq!(forward, reverse);
        assert_eq!(forward.damage.share, Some(0.2));
        assert_eq!(forward.frags.share, Some(0.2));
        assert_ne!(forward.damage.share, Some(0.3125));
    }

    #[test]
    fn empty_report_has_complete_zeroed_catalogs() {
        let actual = report(&[]).unwrap();
        assert_eq!(actual.selected_battles, 0);
        assert_eq!(actual.common_context.rejection_reasons.len(), 15);
        assert_eq!(actual.damage.rejection_reasons.len(), 18);
        assert_eq!(actual.frags.rejection_reasons.len(), 18);
        assert_eq!(actual.xp_position.rejection_reasons.len(), 18);
        assert_eq!(actual.damage.rank_distribution, rank_catalog());
        assert_eq!(actual.xp_position.rank_distribution, rank_catalog());
        assert_eq!(actual.damage.share, None);
        assert_eq!(actual.frags.share, None);
    }

    #[test]
    fn reason_catalogs_and_rank_buckets_are_exact_and_report_mutation_fails_closed() {
        let actual = report(&[]).unwrap();
        assert_eq!(
            actual
                .common_context
                .rejection_reasons
                .iter()
                .map(|reason| reason.code)
                .collect::<Vec<_>>(),
            TeamRelativeCommonReason::ALL
                .into_iter()
                .map(TeamRelativeCommonReason::as_str)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            actual
                .damage
                .rank_distribution
                .iter()
                .map(|bucket| bucket.rank)
                .collect::<Vec<_>>(),
            (1..=15).collect::<Vec<_>>()
        );

        let mut malformed = actual.clone();
        malformed.damage.metric_ineligible_battles = 1;
        assert_eq!(
            validate_report(&malformed),
            Err(TeamRelativeResultsError::ReportInvariantViolation)
        );
    }

    #[test]
    fn checked_aggregation_overflow_fails_closed() {
        let mut total = i64::MAX;
        assert_eq!(
            checked_add(&mut total, 1),
            Err(TeamRelativeResultsError::ReportInvariantViolation)
        );
    }

    #[test]
    fn sqlite_session_and_recent_reports_preserve_membership_and_are_deterministic() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("reports.db")).unwrap();
        let session = database.start_session().unwrap();

        let full = stored_battle("full", 30, DataQuality::Full, Some(session.id.clone()));
        insert(&database, full, Some(&eligible_facts()));

        let mut partial = stored_battle(
            "partial",
            20,
            DataQuality::Partial,
            Some(session.id.clone()),
        );
        partial.feature_schema_version = None;
        partial.data_completeness = Some("metadata_only".into());
        partial.replay_source = Some("replay_metadata".into());
        partial.damage = None;
        partial.frags = None;
        partial.original_xp = None;
        insert(
            &database,
            partial,
            Some(&TeamRelativeFactsV1::common_ineligible(
                TeamRelativeCommonReason::PartialReplay,
            )),
        );

        let mut legacy = stored_battle("legacy", 10, DataQuality::Full, Some(session.id.clone()));
        legacy.parser_version = Some(6);
        legacy.feature_schema_version = Some(2);
        insert(&database, legacy, None);

        let diagnostics_before = database.diagnostics_read_only().unwrap();
        let membership_before = database
            .session_battles(&session.id)
            .unwrap()
            .into_iter()
            .map(|battle| battle.arena_id)
            .collect::<Vec<_>>();

        let first = team_relative_session_report(&database, &session.id).unwrap();
        let second = team_relative_session_report(&database, &session.id).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.selected_battles, 3);
        assert_eq!(first.common_context.eligible_battles, 1);
        assert_eq!(
            count(&first.common_context.rejection_reasons, "partial_replay"),
            1
        );
        assert_eq!(
            count(&first.common_context.rejection_reasons, "parser_too_old"),
            1
        );

        let recent = team_relative_recent_report(&database, 10).unwrap();
        assert_eq!(recent.selected_battles, 3);
        assert_eq!(
            database.diagnostics_read_only().unwrap(),
            diagnostics_before
        );
        assert_eq!(
            database
                .session_battles(&session.id)
                .unwrap()
                .into_iter()
                .map(|battle| battle.arena_id)
                .collect::<Vec<_>>(),
            membership_before
        );
        assert!(database.integrity_report().unwrap().is_healthy());
    }

    #[test]
    fn sqlite_empty_missing_and_current_missing_fact_rows_are_exact() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("states.db")).unwrap();
        let session = database.start_session().unwrap();

        let empty = team_relative_session_report(&database, &session.id).unwrap();
        assert_eq!(empty.selected_battles, 0);
        assert_eq!(
            team_relative_session_report(&database, "missing"),
            Err(TeamRelativeResultsError::SessionNotFound)
        );

        let full = stored_battle(
            "current-full-without-row",
            1,
            DataQuality::Full,
            Some(session.id.clone()),
        );
        insert(&database, full, None);
        assert_eq!(
            team_relative_session_report(&database, &session.id),
            Err(TeamRelativeResultsError::ReportInvariantViolation)
        );

        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("partial-state.db")).unwrap();
        let session = database.start_session().unwrap();
        let mut partial = stored_battle(
            "current-partial-without-row",
            1,
            DataQuality::Partial,
            Some(session.id.clone()),
        );
        partial.feature_schema_version = None;
        partial.data_completeness = Some("metadata_only".into());
        partial.replay_source = Some("replay_metadata".into());
        insert(&database, partial, None);
        assert_eq!(
            team_relative_session_report(&database, &session.id),
            Err(TeamRelativeResultsError::ReportInvariantViolation)
        );
    }

    #[test]
    fn sqlite_recent_applies_limit_before_eligibility_without_backfill() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("window.db")).unwrap();
        for index in 0..12 {
            let mut battle = stored_battle(
                &format!("battle-{index:02}"),
                100 - index,
                if index < 10 {
                    DataQuality::Partial
                } else {
                    DataQuality::Full
                },
                None,
            );
            if index < 10 {
                battle.feature_schema_version = None;
                battle.data_completeness = Some("metadata_only".into());
                battle.replay_source = Some("replay_metadata".into());
                battle.damage = None;
                battle.frags = None;
                battle.original_xp = None;
                insert(
                    &database,
                    battle,
                    Some(&TeamRelativeFactsV1::common_ineligible(
                        TeamRelativeCommonReason::PartialReplay,
                    )),
                );
            } else {
                insert(&database, battle, Some(&eligible_facts()));
            }
        }
        let actual = team_relative_recent_report(&database, 10).unwrap();
        assert_eq!(actual.selected_battles, 10);
        assert_eq!(actual.common_context.eligible_battles, 0);
        assert_eq!(
            count(&actual.common_context.rejection_reasons, "partial_replay"),
            10
        );
        assert_eq!(
            database
                .team_relative_results_recent_rows(10)
                .unwrap()
                .into_iter()
                .map(|row| row.battle_key)
                .collect::<Vec<_>>(),
            (0..10)
                .map(|index| format!("battle-{index:02}"))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn invalid_recent_window_is_rejected_before_database_open() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("removed.db");
        let database = Database::initialize(path.clone()).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(
            team_relative_recent_report(&database, 11),
            Err(TeamRelativeResultsError::InvalidWindow)
        );
    }
}
