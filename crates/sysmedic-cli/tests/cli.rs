//! End-to-end tests for the `sysmedic` binary.
//!
//! Everything else in this workspace is a unit test over a pure function. What
//! nothing covered was the contract people actually build on: the shape of
//! `--format json`, the exit codes, and the refusal to act on an unknown fix
//! id. Those are the promises a script or a monitoring check depends on, and
//! they can only be tested by running the binary.
//!
//! `CARGO_BIN_EXE_sysmedic` is set by cargo for integration tests, so no
//! helper crate is needed to locate the build.

use std::process::{Command, Output};

fn sysmedic(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sysmedic"))
        .args(args)
        // Pin the language so assertions on English text hold regardless of
        // the machine running the suite.
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env("LC_MESSAGES", "C")
        .output()
        .expect("failed to run the sysmedic binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn checkup_json_carries_the_documented_shape() {
    let out = sysmedic(&["checkup", "--format", "json"]);
    assert!(out.status.success(), "checkup exited {:?}", out.status);

    let report: serde_json::Value =
        serde_json::from_str(&stdout(&out)).expect("--format json must emit parseable JSON");

    // The fields any consumer of this output relies on.
    assert!(report["score"].is_u64(), "missing numeric score");
    assert!((0..=100).contains(&report["score"].as_u64().unwrap()));
    assert!(report["grade"].is_string());
    assert!(report["generated_at"].is_string());
    assert!(report["findings"].is_array());
    assert!(report["category_scores"].is_array());
    assert_eq!(report["category_scores"].as_array().unwrap().len(), 12);

    // Coverage travels with the score: a consumer must be able to tell how
    // much of the system that number is based on.
    assert!(report["coverage"]["measured"].is_u64());
    assert_eq!(report["coverage"]["total"], 12);

    // Every finding is self-describing enough to act on.
    for finding in report["findings"].as_array().unwrap() {
        assert!(finding["id"].is_string(), "finding without an id");
        assert!(finding["category"].is_string());
        assert!(finding["severity"].is_string());
        assert!(finding["title"].is_string());
    }
}

#[test]
fn json_output_is_not_polluted_by_progress_messages() {
    // "Running SysMedic checkup..." goes to stderr on purpose, so
    // `sysmedic checkup --format json | jq` works.
    let out = sysmedic(&["checkup", "--format", "json"]);
    assert!(stdout(&out).trim_start().starts_with('{'));
}

#[test]
fn checks_lists_every_rule() {
    let out = sysmedic(&["checks"]);
    assert!(out.status.success());
    let listing = stdout(&out);
    let names: Vec<&str> = listing.lines().map(str::trim).collect();
    assert_eq!(
        names.len(),
        sysmedic_diagnostics::FINDING_IDS.len(),
        "`sysmedic checks` and the declared rule set disagree"
    );
    assert!(names.contains(&"disk-nearly-full"));
}

#[test]
fn explain_rejects_an_unknown_id() {
    let out = sysmedic(&["explain", "not.a.real.finding"]);
    assert!(!out.status.success(), "an unknown id must not exit 0");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unknown finding id"),
        "unhelpful error: {stderr}"
    );
}

#[test]
fn explain_answers_the_five_questions_in_both_languages() {
    let en = sysmedic(&["explain", "storage.disk_nearly_full", "--lang", "en"]);
    assert!(en.status.success());
    let en = stdout(&en);
    for label in ["Cause:", "Dangerous?", "Impact:", "Remedy:", "If ignored:"] {
        assert!(en.contains(label), "missing {label} in: {en}");
    }

    // The Arabic answers must not arrive under English labels.
    let ar = stdout(&sysmedic(&[
        "explain",
        "storage.disk_nearly_full",
        "--lang",
        "ar",
    ]));
    assert!(ar.contains("السبب:"), "Arabic labels missing: {ar}");
    assert!(ar.contains("العلاج:"));
    assert!(!ar.contains("Cause:"));
}

#[test]
fn fix_refuses_an_unknown_id() {
    let out = sysmedic(&["fix", "fix.definitely_not_real", "--yes"]);
    assert!(!out.status.success(), "an unknown fix id must not exit 0");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unknown or not applicable"),
        "unhelpful error: {stderr}"
    );
}

#[test]
fn fix_without_yes_changes_nothing() {
    // The default is a preview. Listing applicable fixes and previewing one
    // must never reach the apply path.
    let list = sysmedic(&["fix"]);
    assert!(list.status.success());

    let dry = sysmedic(&["fix", "fix.apt_clean", "--dry-run"]);
    // Either the fix does not apply on this machine (non-zero, explained) or
    // it previews and says nothing was changed — never "applied".
    let combined = stdout(&dry) + &String::from_utf8_lossy(&dry.stderr);
    assert!(
        !combined.contains("applied fix."),
        "a dry run reported an application: {combined}"
    );
}

#[test]
fn exit_code_is_zero_by_default_and_opt_in_otherwise() {
    // Scripts have always been able to rely on `checkup` exiting 0.
    let default = sysmedic(&["checkup", "--format", "json"]);
    assert_eq!(default.status.code(), Some(0));

    // With --exit-code the code reflects the worst finding: 0 healthy,
    // 1 High, 2 Critical. The machine running CI is not controlled, so assert
    // the contract (one of the three, and consistent with the JSON) rather
    // than a fixed value.
    let opted = sysmedic(&["checkup", "--format", "json", "--exit-code"]);
    let code = opted.status.code().expect("terminated by a signal");
    assert!((0..=2).contains(&code), "unexpected exit code {code}");

    let report: serde_json::Value = serde_json::from_str(&stdout(&opted)).unwrap();
    let worst = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| match f["severity"].as_str().unwrap() {
            "critical" => 2,
            "high" => 1,
            _ => 0,
        })
        .max()
        .unwrap_or(0);
    assert_eq!(
        code, worst,
        "exit code disagrees with the reported findings"
    );
}

#[test]
fn help_documents_every_subcommand() {
    let out = sysmedic(&["--help"]);
    assert!(out.status.success());
    let help = stdout(&out);
    for command in [
        "checkup", "checks", "explain", "fix", "undo", "disk", "network", "monitor", "history",
        "schedule",
    ] {
        assert!(help.contains(command), "`{command}` missing from --help");
    }
}
