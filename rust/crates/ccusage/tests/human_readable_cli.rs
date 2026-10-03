use std::process::Command;

use ccusage_test_support::{Fixture, fs_fixture};
use serde_json::Value;

fn fixture() -> Fixture {
    fs_fixture!({
        "claude/projects/project-a/session-a.jsonl": r#"{"timestamp":"2026-09-15T12:00:00.000Z","sessionId":"session-a","requestId":"request-a","costUSD":1.25,"message":{"id":"message-a","model":"claude-sonnet-4-20250514","usage":{"input_tokens":1234000,"output_tokens":567000,"cache_creation_input_tokens":1000,"cache_read_input_tokens":89000}}}"#,
        "codex/sessions/2026/09/15/rollout-session.jsonl": r#"{"timestamp":"2026-09-15T12:00:00.000Z","type":"event_msg","payload":{"type":"token_count","info":{"model":"gpt-5","last_token_usage":{"input_tokens":1324000,"cached_input_tokens":89000,"cache_creation_input_tokens":1000,"output_tokens":567000,"reasoning_output_tokens":1000,"total_tokens":1891000}}}}"#,
        "amp/threads/thread-a.json": r#"{"id":"thread-a","usageLedger":{"events":[{"id":"event-a","timestamp":"2026-09-15T12:00:00.000Z","model":"gpt-5","tokens":{"input":1234000,"output":567000}}]}}"#,
    })
}

fn run_cli(fixture: &Fixture, args: &[&str], extra: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_ccusage"))
        .env_clear()
        .env("HOME", fixture.path("home"))
        .env("USERPROFILE", fixture.path("userprofile"))
        .env("XDG_CONFIG_HOME", fixture.path("xdg-config"))
        .env("CLAUDE_CONFIG_DIR", fixture.path("claude"))
        .env("CODEX_HOME", fixture.path("codex"))
        .env("AMP_DATA_DIR", fixture.path("amp"))
        .env("LOG_LEVEL", "0")
        .env("NO_COLOR", "1")
        .env("COLUMNS", "240")
        .args(args)
        .args(["--offline", "--no-color", "--timezone", "UTC"])
        .args(extra)
        .output()
        .expect("ccusage should run");
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn human_readable_token_columns_cover_report_modes_and_breakdowns() {
    let fixture = fixture();
    for args in [
        vec!["daily"],
        vec!["weekly"],
        vec!["monthly"],
        vec!["session"],
        vec!["claude", "daily"],
        vec!["claude", "weekly"],
        vec!["claude", "monthly"],
        vec!["claude", "session"],
        vec!["codex", "daily"],
        vec!["codex", "monthly"],
        vec!["codex", "session"],
        vec!["amp", "daily"],
        vec!["amp", "monthly"],
        vec!["amp", "session"],
    ] {
        let raw = run_cli(&fixture, &args, &["--breakdown"]);
        assert!(raw.contains("1,234,000"), "{args:?}: {raw}");
        let human = run_cli(&fixture, &args, &["--human-readable", "--breakdown"]);
        assert!(human.contains("1.23M"), "{args:?}: {human}");
        assert!(human.contains("567K"), "{args:?}: {human}");
        assert!(!human.contains("1,234,000"), "{args:?}: {human}");
        assert!(!human.contains("567,000"), "{args:?}: {human}");
        for line in human
            .lines()
            .filter(|line| line.contains("Total") || line.contains("└─"))
        {
            assert!(!line.contains("567,000"), "{args:?}: {line}");
        }
    }
}

#[test]
fn human_readable_token_counts_leave_json_unchanged() {
    let fixture = fixture();
    for args in [
        vec!["daily"],
        vec!["weekly"],
        vec!["monthly"],
        vec!["session"],
        vec!["claude", "daily"],
        vec!["claude", "monthly"],
        vec!["claude", "session"],
        vec!["claude", "session", "--id", "session-a"],
        vec!["codex", "daily"],
        vec!["codex", "monthly"],
        vec!["codex", "session"],
        vec!["amp", "daily"],
        vec!["amp", "monthly"],
        vec!["amp", "session"],
        vec!["blocks"],
    ] {
        let raw: Value = serde_json::from_str(&run_cli(&fixture, &args, &["--json"])).unwrap();
        let human: Value =
            serde_json::from_str(&run_cli(&fixture, &args, &["--json", "--human-readable"]))
                .unwrap();
        assert_eq!(human, raw, "{args:?}");
    }
}

#[test]
fn human_readable_tokens_work_with_compact_layout_and_hidden_costs() {
    let fixture = fixture();
    for args in [
        vec!["daily"],
        vec!["claude", "daily"],
        vec!["codex", "daily"],
        vec!["amp", "daily"],
    ] {
        let output = run_cli(
            &fixture,
            &args,
            &["--human-readable", "--compact", "--no-cost", "--breakdown"],
        );
        assert!(output.contains("1.23M"), "{args:?}: {output}");
        assert!(output.contains("567K"), "{args:?}: {output}");
        assert!(!output.contains("Cost (USD)"), "{args:?}: {output}");
    }
}

#[test]
fn human_readable_tokens_cover_blocks_and_selected_claude_sessions() {
    let fixture = fixture();
    let blocks = run_cli(&fixture, &["blocks"], &["--human-readable"]);
    assert!(blocks.contains("1.89M"), "{blocks}");
    assert!(!blocks.contains("1,891,000"), "{blocks}");
    let session = run_cli(
        &fixture,
        &["claude", "session", "--id", "session-a"],
        &["--human-readable"],
    );
    assert!(session.contains("Total Tokens: 1.89M"), "{session}");
}

#[test]
fn human_readable_tokens_cover_cache_reasoning_and_total_columns() {
    let fixture = fixture();
    for (args, expected) in [
        (
            vec!["claude", "daily"],
            vec!["1.23M", "567K", "1K", "89K", "1.89M"],
        ),
        (
            vec!["codex", "daily"],
            vec!["1.23M", "567K", "1K", "1K", "89K", "1.89M"],
        ),
    ] {
        let report = run_cli(&fixture, &args, &["--human-readable", "--breakdown"]);
        for line in report.lines().filter(|line| {
            line.starts_with('│')
                && (line.contains("2026-09-15")
                    || line
                        .split('│')
                        .nth(1)
                        .is_some_and(|cell| cell.trim() == "Total")
                    || line.contains("└─"))
        }) {
            let cells = line.split('│').skip(1).map(str::trim).collect::<Vec<_>>();
            assert_eq!(
                &cells[2..2 + expected.len()],
                expected.as_slice(),
                "{args:?}: {line}"
            );
        }
    }
}

#[test]
fn human_readable_tokens_leave_jq_numeric_counts_exact() {
    let fixture = fixture();
    let output = run_cli(
        &fixture,
        &["claude", "daily"],
        &["--human-readable", "--jq", ".totals.inputTokens"],
    );
    assert_eq!(output.trim(), "1234000");
}

#[test]
fn human_readable_active_blocks_format_limits_remaining_and_burn_rate() {
    use ccusage_core::{MILLIS_PER_MINUTE, format_rfc3339_millis, utc_now};
    use serde_json::json;

    let fixture = Fixture::new();
    let now = utc_now();
    let entries = [10, 5].into_iter().map(|minutes| {
        json!({
            "timestamp": format_rfc3339_millis(now.checked_sub_millis(minutes * MILLIS_PER_MINUTE).unwrap()),
            "sessionId": "active-session",
            "requestId": format!("request-{minutes}"),
            "costUSD": 1.25,
            "message": {
                "id": format!("message-{minutes}"), "model": "claude-sonnet-4-20250514",
                "usage": { "input_tokens": 1_000_000, "output_tokens": 500_000 },
            },
        }).to_string()
    }).collect::<Vec<_>>().join("\n");
    let _ = fixture.write_file("claude/projects/project-a/session.jsonl", entries);
    let output = run_cli(
        &fixture,
        &["blocks", "--token-limit", "5000000"],
        &["--human-readable"],
    );
    assert!(output.contains("(assuming 5M token limit)"), "{output}");
    assert!(output.contains("REMAINING"), "{output}");
    assert!(output.contains("2M"), "{output}");
    assert!(output.contains("PROJECTED"), "{output}");
    let detail = run_cli(
        &fixture,
        &["blocks", "--active", "--token-limit", "5000000"],
        &["--human-readable"],
    );
    assert!(detail.contains("Tokens/minute:    600K"), "{detail}");
    assert!(detail.contains("Limit:            5M tokens"), "{detail}");
    assert!(!output.contains("5,000,000"), "{output}");
}
