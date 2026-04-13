//! Suite runner — executes every registered `MatchupSpec` and emits a
//! structured JSON report.

use std::collections::{HashMap, HashSet};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use engine::database::CardDatabase;
use engine::game::deck_loading::{
    load_deck_into_state, resolve_deck_list, DeckList, DeckPayload, PlayerDeckList,
};
use engine::types::game_state::{GameState, WaitingFor};
use engine::types::player::PlayerId;
use serde::{Deserialize, Serialize};

use crate::auto_play::run_ai_actions;
use crate::config::{create_config_for_players, AiConfig, AiDifficulty, Platform};

use super::{all_matchups, resolve_deck_ref, Expected, FeatureKind, MatchupSpec};

/// Safety cap on total AI actions per game — matches the constant in
/// `bin/ai_duel.rs` so suite games and single-matchup games terminate
/// identically.
const MAX_TOTAL_ACTIONS: usize = 10_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SuiteStatus {
    Pass,
    Fail,
    Open,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchupResult {
    pub matchup_id: String,
    pub exercises: Vec<FeatureKind>,
    pub p0_label: String,
    pub p1_label: String,
    pub expected: Expected,
    pub p0_wins: usize,
    pub p1_wins: usize,
    pub draws: usize,
    pub total_turns: u64,
    pub total_duration_ms: u128,
    pub avg_turns: f64,
    pub avg_duration_ms: f64,
    pub status: SuiteStatus,
    pub fail_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteReport {
    pub schema_version: u32,
    pub unix_timestamp_secs: i64,
    pub difficulty: String,
    pub games_per_matchup: usize,
    pub base_seed: u64,
    pub results: Vec<MatchupResult>,
}

#[derive(Debug)]
pub struct SuiteOptions {
    pub difficulty: AiDifficulty,
    pub games_per_matchup: usize,
    pub base_seed: u64,
    pub output_path: PathBuf,
    pub filter: Option<String>,
}

impl SuiteOptions {
    pub fn new(difficulty: AiDifficulty, games_per_matchup: usize, base_seed: u64) -> Self {
        Self {
            difficulty,
            games_per_matchup,
            base_seed,
            output_path: PathBuf::from("target/duel-suite-results.json"),
            filter: None,
        }
    }
}

/// Run every registered matchup, write the report to `options.output_path`,
/// and return the in-memory report for the caller to print.
pub fn run_suite(db: &CardDatabase, options: &SuiteOptions) -> Result<SuiteReport, std::io::Error> {
    let matchups = all_matchups();
    let mut results: Vec<MatchupResult> = Vec::with_capacity(matchups.len());

    for (idx, spec) in matchups.iter().enumerate() {
        if let Some(filter) = &options.filter {
            if !spec.id.contains(filter) {
                continue;
            }
        }
        eprintln!(
            "[{idx:>2}/{total}] {id}  (games: {games})",
            idx = idx + 1,
            total = matchups.len(),
            id = spec.id,
            games = options.games_per_matchup,
        );
        let matchup_seed = options.base_seed.wrapping_add(idx as u64 * 1_000);
        let result = run_single_matchup(db, spec, options, matchup_seed);
        print_matchup_row(&result);
        results.push(result);
    }

    let report = SuiteReport {
        schema_version: 1,
        unix_timestamp_secs: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
        difficulty: format!("{:?}", options.difficulty),
        games_per_matchup: options.games_per_matchup,
        base_seed: options.base_seed,
        results,
    };

    write_report(&report, &options.output_path)?;
    print_markdown_table(&report);

    Ok(report)
}

fn run_single_matchup(
    db: &CardDatabase,
    spec: &MatchupSpec,
    options: &SuiteOptions,
    matchup_seed: u64,
) -> MatchupResult {
    let payload = match build_payload(db, spec) {
        Ok(p) => p,
        Err(reason) => return failed_result(spec, &reason),
    };

    let mut p0_wins = 0usize;
    let mut p1_wins = 0usize;
    let mut draws = 0usize;
    let mut total_turns: u64 = 0;
    let mut total_duration_ms: u128 = 0;

    for game_idx in 0..options.games_per_matchup {
        let seed = matchup_seed.wrapping_add(game_idx as u64);
        let start = Instant::now();
        let (winner, turns) = run_game(&payload, seed, options.difficulty);
        total_duration_ms += start.elapsed().as_millis();
        total_turns += turns as u64;
        match winner {
            Some(PlayerId(0)) => p0_wins += 1,
            Some(_) => p1_wins += 1,
            None => draws += 1,
        }
    }

    let n = options.games_per_matchup.max(1) as f64;
    let avg_turns = total_turns as f64 / n;
    let avg_duration_ms = total_duration_ms as f64 / n;
    let (status, fail_reason) = classify(&spec.expected, p0_wins, options.games_per_matchup);

    MatchupResult {
        matchup_id: spec.id.to_string(),
        exercises: spec.exercises.to_vec(),
        p0_label: spec.p0_label.to_string(),
        p1_label: spec.p1_label.to_string(),
        expected: spec.expected,
        p0_wins,
        p1_wins,
        draws,
        total_turns,
        total_duration_ms,
        avg_turns,
        avg_duration_ms,
        status,
        fail_reason,
    }
}

fn build_payload(db: &CardDatabase, spec: &MatchupSpec) -> Result<DeckPayload, String> {
    let p0 = resolve_deck_ref(&spec.p0).map_err(|e| format!("p0 load: {e}"))?;
    let p1 = resolve_deck_ref(&spec.p1).map_err(|e| format!("p1 load: {e}"))?;
    let deck_list = DeckList {
        player: PlayerDeckList {
            main_deck: p0,
            sideboard: Vec::new(),
            commander: Vec::new(),
        },
        opponent: PlayerDeckList {
            main_deck: p1,
            sideboard: Vec::new(),
            commander: Vec::new(),
        },
        ai_decks: Vec::new(),
    };
    Ok(resolve_deck_list(db, &deck_list))
}

fn failed_result(spec: &MatchupSpec, reason: &str) -> MatchupResult {
    MatchupResult {
        matchup_id: spec.id.to_string(),
        exercises: spec.exercises.to_vec(),
        p0_label: spec.p0_label.to_string(),
        p1_label: spec.p1_label.to_string(),
        expected: spec.expected,
        p0_wins: 0,
        p1_wins: 0,
        draws: 0,
        total_turns: 0,
        total_duration_ms: 0,
        avg_turns: 0.0,
        avg_duration_ms: 0.0,
        status: SuiteStatus::Fail,
        fail_reason: Some(format!("setup error: {reason}")),
    }
}

fn classify(expected: &Expected, p0_wins: usize, total: usize) -> (SuiteStatus, Option<String>) {
    if total == 0 {
        return (SuiteStatus::Open, None);
    }
    let p0_rate = p0_wins as f32 / total as f32;
    match expected {
        Expected::Open => (SuiteStatus::Open, None),
        Expected::Mirror { tolerance } => {
            if (p0_rate - 0.5).abs() <= *tolerance {
                (SuiteStatus::Pass, None)
            } else {
                (
                    SuiteStatus::Fail,
                    Some(format!(
                        "mirror imbalance: p0={p0_rate:.2}, tolerance=±{tolerance}"
                    )),
                )
            }
        }
        Expected::Triangle {
            p0_winrate_min,
            p0_winrate_max,
        } => {
            if p0_rate >= *p0_winrate_min && p0_rate <= *p0_winrate_max {
                (SuiteStatus::Pass, None)
            } else {
                (
                    SuiteStatus::Fail,
                    Some(format!(
                        "triangle out of range: p0={p0_rate:.2}, expected \
                         [{p0_winrate_min:.2}, {p0_winrate_max:.2}]"
                    )),
                )
            }
        }
    }
}

fn run_game(payload: &DeckPayload, seed: u64, difficulty: AiDifficulty) -> (Option<PlayerId>, u32) {
    let mut state = GameState::new_two_player(seed);
    load_deck_into_state(&mut state, payload);
    engine::game::engine::start_game(&mut state);

    let ai_players: HashSet<PlayerId> = [PlayerId(0), PlayerId(1)].into_iter().collect();
    let config = create_config_for_players(difficulty, Platform::Native, 2);
    let ai_configs: HashMap<PlayerId, AiConfig> =
        [(PlayerId(0), config.clone()), (PlayerId(1), config)]
            .into_iter()
            .collect();

    let mut total_actions: usize = 0;
    loop {
        let results = run_ai_actions(&mut state, &ai_players, &ai_configs);
        if results.is_empty() {
            break;
        }
        total_actions += results.len();
        if total_actions >= MAX_TOTAL_ACTIONS {
            break;
        }
    }

    let winner = match &state.waiting_for {
        WaitingFor::GameOver { winner } => *winner,
        _ => None,
    };
    (winner, state.turn_number)
}

fn write_report(report: &SuiteReport, path: &Path) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    serde_json::to_writer_pretty(BufWriter::new(file), report).map_err(std::io::Error::other)?;
    Ok(())
}

fn print_matchup_row(r: &MatchupResult) {
    let total = r.p0_wins + r.p1_wins + r.draws;
    let p0_pct = if total > 0 {
        r.p0_wins as f64 / total as f64 * 100.0
    } else {
        0.0
    };
    let status_str = match r.status {
        SuiteStatus::Pass => "PASS",
        SuiteStatus::Fail => "FAIL",
        SuiteStatus::Open => "OPEN",
    };
    eprintln!(
        "       {status_str}  p0={:>3}/{total} ({p0_pct:.0}%)  turns={:.1}",
        r.p0_wins, r.avg_turns
    );
    if let Some(reason) = &r.fail_reason {
        eprintln!("       reason: {reason}");
    }
}

fn print_markdown_table(report: &SuiteReport) {
    println!();
    println!("| matchup | exercises | p0% | avg turns | status |");
    println!("|---------|-----------|-----|-----------|--------|");
    for r in &report.results {
        let total = r.p0_wins + r.p1_wins + r.draws;
        let p0_pct = if total > 0 {
            r.p0_wins as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        let exercises: Vec<String> = r.exercises.iter().map(|f| format!("{f:?}")).collect();
        let status_str = match r.status {
            SuiteStatus::Pass => "PASS",
            SuiteStatus::Fail => "FAIL",
            SuiteStatus::Open => "OPEN",
        };
        println!(
            "| {} | {} | {:.0}% | {:.1} | {} |",
            r.matchup_id,
            exercises.join(", "),
            p0_pct,
            r.avg_turns,
            status_str,
        );
    }
}

/// Utility for external callers (e.g. the binary's `--matchup` single-matchup
/// path) to resolve a `DeckRef` to a `DeckPayload`. Returns the resolved
/// payload and labels on success.
pub fn resolve_matchup(
    db: &CardDatabase,
    spec: &MatchupSpec,
) -> Result<(DeckPayload, String, String), String> {
    let payload = build_payload(db, spec)?;
    Ok((
        payload,
        spec.p0_label.to_string(),
        spec.p1_label.to_string(),
    ))
}
