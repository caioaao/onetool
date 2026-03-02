mod runner;
mod scenarios;
mod scoring;

use clap::Parser;
use onetool::Repl;
use onetool::genai::LuaRepl;
use onetool::runtime::sandbox::policy::DenyAllPolicy;
use serde::Serialize;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "onetool-evals",
    about = "Evaluation harness for onetool system prompt"
)]
struct Cli {
    /// Model(s) to evaluate (can be specified multiple times)
    #[arg(long, default_value = "gpt-4o-mini")]
    model: Vec<String>,

    /// Only run scenarios matching these tags (comma-separated or repeated)
    #[arg(long)]
    tags: Vec<String>,

    /// Path to system prompt file (default: evals/prompts/baseline.txt)
    #[arg(long)]
    prompt: Option<String>,

    /// Output format: "table" (default) or "json"
    #[arg(long, default_value = "table")]
    output: String,

    /// Maximum turns per scenario
    #[arg(long, default_value = "10")]
    max_turns: usize,

    /// Model to use for LLM-as-judge (default: same as --model)
    #[arg(long)]
    judge_model: Option<String>,

    /// Skip LLM-as-judge scoring
    #[arg(long)]
    no_judge: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ScenarioResult {
    id: String,
    deterministic: scoring::ScoreResult,
    judge: Option<scoring::JudgeResult>,
    turns: usize,
    duration_secs: f64,
}

#[derive(Debug, Clone, Serialize)]
struct EvalRun {
    model: String,
    prompt_file: String,
    results: Vec<ScenarioResult>,
    summary: Summary,
}

#[derive(Debug, Clone, Serialize)]
struct Summary {
    total: usize,
    passed: usize,
    det_avg: f64,
    judge_avg: Option<f64>,
    total_secs: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    // Load system prompt
    let prompt_path = cli
        .prompt
        .clone()
        .unwrap_or_else(|| "evals/prompts/baseline.txt".to_string());
    let system_prompt = std::fs::read_to_string(&prompt_path)
        .map_err(|e| format!("Failed to read prompt file '{}': {}", prompt_path, e))?;

    // Collect tag filters
    let tag_filters: Vec<String> = cli
        .tags
        .iter()
        .flat_map(|t| t.split(',').map(|s| s.trim().to_string()))
        .collect();

    // Get all scenarios, filter by tags
    let all_scenarios = scenarios::all();
    let scenarios: Vec<_> = if tag_filters.is_empty() {
        all_scenarios
    } else {
        all_scenarios
            .into_iter()
            .filter(|s| {
                s.tags()
                    .iter()
                    .any(|t| tag_filters.contains(&t.to_string()))
            })
            .collect()
    };

    if scenarios.is_empty() {
        eprintln!("No scenarios match the given tags: {:?}", tag_filters);
        return Ok(());
    }

    let client = genai::Client::default();
    let mut all_runs = Vec::new();

    for model in &cli.model {
        let run = run_eval(
            &client,
            model,
            &system_prompt,
            &prompt_path,
            &scenarios,
            &cli,
        )
        .await?;

        if cli.output == "table" {
            print_table(&run);
        }

        all_runs.push(run);
    }

    if cli.output == "json" {
        let json = if all_runs.len() == 1 {
            serde_json::to_string_pretty(&all_runs[0])?
        } else {
            serde_json::to_string_pretty(&all_runs)?
        };
        println!("{}", json);
    }

    Ok(())
}

async fn run_eval(
    client: &genai::Client,
    model: &str,
    system_prompt: &str,
    prompt_path: &str,
    scenarios: &[Box<dyn scenarios::Scenario>],
    cli: &Cli,
) -> Result<EvalRun, Box<dyn std::error::Error>> {
    let judge_model = cli.judge_model.as_deref().unwrap_or(model);

    let mut results = Vec::new();

    for scenario in scenarios {
        eprint!("  {} ... ", scenario.id());

        // Create a fresh REPL for each scenario
        let repl = Repl::new_with_policy(Arc::new(DenyAllPolicy))?;
        scenario.setup(&repl)?;

        let lua_repl = LuaRepl::new(&repl);

        let transcript = runner::run(
            client,
            model,
            system_prompt,
            scenario.user_prompt(),
            &lua_repl,
            cli.max_turns,
        )
        .await?;

        let det_score = scenario.score(&transcript);

        let judge_result = if !cli.no_judge {
            if let Some(judge_prompt) = scenario.judge_prompt() {
                match scoring::judge(client, judge_model, &transcript, judge_prompt).await {
                    Ok(jr) => Some(jr),
                    Err(e) => {
                        tracing::warn!("Judge failed for {}: {}", scenario.id(), e);
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        let result = ScenarioResult {
            id: scenario.id().to_string(),
            deterministic: det_score,
            judge: judge_result,
            turns: transcript.total_turns,
            duration_secs: transcript.duration.as_secs_f64(),
        };

        let status = if result.deterministic.passed {
            "PASS"
        } else {
            "FAIL"
        };
        eprintln!("{}", status);

        results.push(result);
    }

    let total = results.len();
    let passed = results.iter().filter(|r| r.deterministic.passed).count();
    let det_avg = results.iter().map(|r| r.deterministic.score).sum::<f64>() / total as f64;
    let judge_scores: Vec<f64> = results
        .iter()
        .filter_map(|r| r.judge.as_ref().map(|j| j.score))
        .collect();
    let judge_avg = if judge_scores.is_empty() {
        None
    } else {
        Some(judge_scores.iter().sum::<f64>() / judge_scores.len() as f64)
    };
    let total_secs: f64 = results.iter().map(|r| r.duration_secs).sum();

    Ok(EvalRun {
        model: model.to_string(),
        prompt_file: prompt_path.to_string(),
        results,
        summary: Summary {
            total,
            passed,
            det_avg,
            judge_avg,
            total_secs,
        },
    })
}

fn print_table(run: &EvalRun) {
    println!();
    println!(
        "onetool eval  |  model: {}  |  {} scenarios",
        run.model, run.summary.total
    );
    println!();
    println!(
        "{:<35} {:>15} {:>8} {:>8} {:>8}",
        "", "deterministic", "judge", "turns", "time"
    );
    println!("{}", "-".repeat(80));

    for r in &run.results {
        let det_str = if r.deterministic.passed {
            format!("PASS ({:.1})", r.deterministic.score)
        } else {
            format!("FAIL ({:.1})", r.deterministic.score)
        };

        let judge_str = match &r.judge {
            Some(j) => format!("{:.1}", j.score),
            None => "-".to_string(),
        };

        println!(
            "{:<35} {:>15} {:>8} {:>5} {:>7.1}s",
            r.id, det_str, judge_str, r.turns, r.duration_secs
        );
    }

    println!("{}", "-".repeat(80));
    println!(
        "Summary: {}/{} passed  |  det avg: {:.2}  |  judge avg: {}  |  total: {:.1}s",
        run.summary.passed,
        run.summary.total,
        run.summary.det_avg,
        run.summary
            .judge_avg
            .map(|j| format!("{:.2}", j))
            .unwrap_or_else(|| "-".to_string()),
        run.summary.total_secs
    );
    println!();
}
