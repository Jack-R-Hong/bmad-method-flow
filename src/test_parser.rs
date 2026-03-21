//! Test output parser — transforms raw test runner output into structured results.
//!
//! Supports: cargo test, Jest/Mocha, pytest, JUnit XML.
//! Used by the workflow executor to provide targeted failure context for retry loops
//! and review steps via `context_from`.

use serde::{Deserialize, Serialize};

// ── Data structures ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParsedTestResults {
    pub framework: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub failures: Vec<TestFailure>,
    pub raw_output: String,
    pub format_detected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestFailure {
    pub test_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_number: Option<u32>,
    pub assertion_message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Parse test output using the specified framework, or auto-detect.
pub fn parse_test_output(output: &str, framework: Option<&str>) -> ParsedTestResults {
    match framework {
        Some("cargo-test") => parse_cargo_test(output),
        Some("jest") => parse_jest(output),
        Some("pytest") => parse_pytest(output),
        Some("junit-xml") => parse_junit_xml(output),
        Some(_) | None => auto_detect(output),
    }
}

// ── Auto-detection ─────────────────────────────────────────────────────────

fn auto_detect(output: &str) -> ParsedTestResults {
    // Try cargo test first (most common in this project)
    if output.contains("test result:")
        && (output.contains("... ok") || output.contains("... FAILED"))
    {
        return parse_cargo_test(output);
    }
    // Jest/Mocha
    if output.contains("Test Suites:") || output.contains("Tests:") && output.contains("passed") {
        return parse_jest(output);
    }
    // Pytest
    if output.contains("passed")
        && output.contains("failed")
        && output.contains("=")
        && output.contains("::")
    {
        return parse_pytest(output);
    }
    // JUnit XML
    if output.contains("<testsuite") || output.contains("<testsuites") {
        return parse_junit_xml(output);
    }
    // Fallback
    fallback_result(output)
}

fn fallback_result(output: &str) -> ParsedTestResults {
    ParsedTestResults {
        framework: "unknown".to_string(),
        total: 0,
        passed: 0,
        failed: 0,
        skipped: 0,
        failures: Vec::new(),
        raw_output: output.to_string(),
        format_detected: false,
    }
}

// ── Cargo test parser ──────────────────────────────────────────────────────

fn parse_cargo_test(output: &str) -> ParsedTestResults {
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let mut failures = Vec::new();
    let mut in_failures_section = false;
    let mut current_failure_name = String::new();
    let mut current_failure_msg = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();

        // Count individual test results
        if trimmed.starts_with("test ") && trimmed.ends_with("... ok") {
            passed += 1;
        } else if trimmed.starts_with("test ") && trimmed.ends_with("... FAILED") {
            failed += 1;
        } else if trimmed.starts_with("test ") && trimmed.ends_with("... ignored") {
            skipped += 1;
        }

        // Parse failures section
        if trimmed == "failures:" && !in_failures_section {
            in_failures_section = true;
            continue;
        }

        if in_failures_section {
            if trimmed == "failures:" {
                // Second "failures:" header — list of failed test names (not stdout details)
                // Flush any pending failure from the stdout section
                if !current_failure_name.is_empty() {
                    failures.push(build_cargo_failure(
                        &current_failure_name,
                        &current_failure_msg,
                    ));
                    current_failure_name.clear();
                    current_failure_msg.clear();
                }
                continue;
            }
            if trimmed.starts_with("test result:") {
                // End of failures section
                in_failures_section = false;
                continue;
            }

            if trimmed.starts_with("---- ") && trimmed.ends_with(" stdout ----") {
                // Save previous failure
                if !current_failure_name.is_empty() {
                    failures.push(build_cargo_failure(
                        &current_failure_name,
                        &current_failure_msg,
                    ));
                }
                // Start new failure
                current_failure_name = trimmed
                    .trim_start_matches("---- ")
                    .trim_end_matches(" stdout ----")
                    .to_string();
                current_failure_msg.clear();
            } else if !current_failure_name.is_empty() && !trimmed.is_empty() {
                current_failure_msg.push(trimmed.to_string());
            }
        }

        // Parse summary line: "test result: FAILED. X passed; Y failed; Z ignored"
        if trimmed.starts_with("test result:") {
            if let Some(counts) = parse_cargo_summary(trimmed) {
                passed = counts.0;
                failed = counts.1;
                skipped = counts.2;
            }
        }
    }

    // Flush last failure
    if !current_failure_name.is_empty() {
        failures.push(build_cargo_failure(
            &current_failure_name,
            &current_failure_msg,
        ));
    }

    let total = passed + failed + skipped;

    ParsedTestResults {
        framework: "cargo-test".to_string(),
        total,
        passed,
        failed,
        skipped,
        failures,
        raw_output: output.to_string(),
        format_detected: true,
    }
}

fn build_cargo_failure(name: &str, msg_lines: &[String]) -> TestFailure {
    // Try to extract file path and line number from assertion messages
    // Format: "thread 'test_name' panicked at 'assertion failed', src/lib.rs:42:5
    let mut file_path = None;
    let mut line_number = None;
    let mut assertion_msg = msg_lines.join("\n");

    for line in msg_lines {
        if line.contains("panicked at") {
            if let Some(loc) = line.rsplit_once(", ") {
                let loc_str = loc.1.trim().trim_end_matches(':');
                let parts: Vec<&str> = loc_str.split(':').collect();
                if parts.len() >= 2 {
                    file_path = Some(parts[0].to_string());
                    line_number = parts[1].parse().ok();
                }
            }
        }
        // Also check for "at file:line" pattern
        if let Some(rest) = line.strip_prefix("thread '") {
            if let Some((_name, location)) = rest.split_once("' panicked at ") {
                assertion_msg = location.to_string();
            }
        }
    }

    TestFailure {
        test_name: name.to_string(),
        file_path,
        line_number,
        assertion_message: if assertion_msg.is_empty() {
            "test failed".to_string()
        } else {
            assertion_msg
        },
        stdout: if msg_lines.is_empty() {
            None
        } else {
            Some(msg_lines.join("\n"))
        },
    }
}

fn parse_cargo_summary(line: &str) -> Option<(usize, usize, usize)> {
    // "test result: ok. 42 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out"
    let mut passed = 0;
    let mut failed = 0;
    let mut ignored = 0;

    for part in line.split(';') {
        let trimmed = part
            .trim()
            .trim_start_matches("test result: ok. ")
            .trim_start_matches("test result: FAILED. ");
        if trimmed.ends_with("passed") {
            passed = trimmed
                .trim_end_matches(" passed")
                .trim()
                .parse()
                .unwrap_or(0);
        } else if trimmed.ends_with("failed") {
            failed = trimmed
                .trim_end_matches(" failed")
                .trim()
                .parse()
                .unwrap_or(0);
        } else if trimmed.ends_with("ignored") {
            ignored = trimmed
                .trim_end_matches(" ignored")
                .trim()
                .parse()
                .unwrap_or(0);
        }
    }

    Some((passed, failed, ignored))
}

// ── Jest/Mocha parser ──────────────────────────────────────────────────────

fn parse_jest(output: &str) -> ParsedTestResults {
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut failures = Vec::new();
    let mut current_suite = String::new();

    for line in output.lines() {
        let trimmed = line.trim();

        // Suite name
        if !trimmed.starts_with('✓')
            && !trimmed.starts_with('✕')
            && !trimmed.starts_with("PASS")
            && !trimmed.starts_with("FAIL")
            && !trimmed.is_empty()
            && !trimmed.starts_with("Test Suites:")
            && !trimmed.starts_with("Tests:")
            && !trimmed.starts_with("Snapshots:")
            && !trimmed.starts_with("Time:")
            && !trimmed.starts_with("Ran ")
        {
            // Could be a describe block
            if !trimmed.starts_with("●") && !trimmed.starts_with("×") && !trimmed.starts_with("at ")
            {
                current_suite = trimmed.to_string();
            }
        }

        // Passing test: ✓ test name (Xms)
        if trimmed.starts_with('✓') || trimmed.starts_with("✓") {
            passed += 1;
        }

        // Failing test: ✕ test name or × test name
        if trimmed.starts_with('✕') || trimmed.starts_with("✕") || trimmed.starts_with('×') {
            failed += 1;
            let test_name = trimmed.trim_start_matches(['✕', '×', ' '].as_ref()).trim();
            failures.push(TestFailure {
                test_name: if current_suite.is_empty() {
                    test_name.to_string()
                } else {
                    format!("{} > {}", current_suite, test_name)
                },
                file_path: None,
                line_number: None,
                assertion_message: "test failed".to_string(),
                stdout: None,
            });
        }

        // Summary line: Tests: X failed, Y passed, Z total
        if trimmed.starts_with("Tests:") {
            let after_colon = trimmed.strip_prefix("Tests:").unwrap_or(trimmed).trim();
            for part in after_colon.split(',') {
                let p = part.trim();
                if p.ends_with("failed") {
                    failed = p
                        .trim_end_matches(" failed")
                        .trim()
                        .parse()
                        .unwrap_or(failed);
                } else if p.ends_with("passed") {
                    passed = p
                        .trim_end_matches(" passed")
                        .trim()
                        .parse()
                        .unwrap_or(passed);
                }
            }
        }
    }

    let total = passed + failed;

    ParsedTestResults {
        framework: "jest".to_string(),
        total,
        passed,
        failed,
        skipped: 0,
        failures,
        raw_output: output.to_string(),
        format_detected: passed > 0 || failed > 0,
    }
}

// ── Pytest parser ──────────────────────────────────────────────────────────

fn parse_pytest(output: &str) -> ParsedTestResults {
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let mut failures = Vec::new();
    let mut in_short_summary = false;

    for line in output.lines() {
        let trimmed = line.trim();

        // Individual test results
        if trimmed.contains("PASSED") {
            passed += 1;
        } else if trimmed.contains("FAILED") && trimmed.contains("::") {
            failed += 1;
        } else if trimmed.contains("SKIPPED") {
            skipped += 1;
        }

        // Short test summary
        if trimmed.contains("short test summary info") {
            in_short_summary = true;
            continue;
        }

        if in_short_summary {
            if trimmed.starts_with("=") && trimmed.len() > 3 {
                in_short_summary = false;
                // Parse final summary: "= 2 failed, 8 passed ="
                for part in trimmed.split(',') {
                    let p = part.trim().trim_matches('=').trim();
                    if p.ends_with("failed") {
                        failed = p
                            .split_whitespace()
                            .next()
                            .and_then(|n| n.parse().ok())
                            .unwrap_or(failed);
                    } else if p.ends_with("passed") {
                        passed = p
                            .split_whitespace()
                            .next()
                            .and_then(|n| n.parse().ok())
                            .unwrap_or(passed);
                    } else if p.ends_with("skipped") {
                        skipped = p
                            .split_whitespace()
                            .next()
                            .and_then(|n| n.parse().ok())
                            .unwrap_or(skipped);
                    }
                }
                continue;
            }
            if trimmed.starts_with("FAILED ") {
                // "FAILED tests/test_auth.py::test_login - AssertionError: ..."
                let rest = trimmed.strip_prefix("FAILED ").unwrap_or(trimmed);
                let parts: Vec<&str> = rest.splitn(2, " - ").collect();
                let test_path = parts[0];
                let msg = parts.get(1).unwrap_or(&"test failed");

                let (file_path, test_name) = if let Some((fp, tn)) = test_path.split_once("::") {
                    (Some(fp.to_string()), tn.to_string())
                } else {
                    (None, test_path.to_string())
                };

                failures.push(TestFailure {
                    test_name,
                    file_path,
                    line_number: None,
                    assertion_message: msg.to_string(),
                    stdout: None,
                });
            }
        }
    }

    let total = passed + failed + skipped;

    ParsedTestResults {
        framework: "pytest".to_string(),
        total,
        passed,
        failed,
        skipped,
        failures,
        raw_output: output.to_string(),
        format_detected: total > 0,
    }
}

// ── JUnit XML parser ───────────────────────────────────────────────────────

fn parse_junit_xml(output: &str) -> ParsedTestResults {
    // Simple XML parsing without external crate — handles basic JUnit format
    let mut total = 0usize;
    let mut failures_count = 0usize;
    let mut skipped = 0usize;
    let mut failures = Vec::new();

    // Parse <testsuite> attributes for summary
    if let Some(suite_start) = output.find("<testsuite") {
        let suite_tag = &output[suite_start
            ..output[suite_start..]
                .find('>')
                .map(|i| suite_start + i + 1)
                .unwrap_or(output.len())];
        total = extract_xml_attr(suite_tag, "tests")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        failures_count = extract_xml_attr(suite_tag, "failures")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        skipped = extract_xml_attr(suite_tag, "skipped")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
    }

    // Parse individual <testcase> elements with <failure> children
    let mut pos = 0;
    while let Some(tc_start) = output[pos..].find("<testcase") {
        let tc_abs = pos + tc_start;
        let tc_end = output[tc_abs..]
            .find("/>")
            .or_else(|| output[tc_abs..].find("</testcase>"))
            .map(|i| tc_abs + i)
            .unwrap_or(output.len());
        let tc_block = &output[tc_abs..tc_end];

        let name = extract_xml_attr(tc_block, "name").unwrap_or_default();
        let classname = extract_xml_attr(tc_block, "classname").unwrap_or_default();

        if tc_block.contains("<failure") {
            let fail_msg = extract_xml_content(tc_block, "failure")
                .unwrap_or_else(|| "test failed".to_string());
            failures.push(TestFailure {
                test_name: if classname.is_empty() {
                    name
                } else {
                    format!("{}::{}", classname, name)
                },
                file_path: None,
                line_number: None,
                assertion_message: fail_msg,
                stdout: None,
            });
        }

        pos = tc_end + 1;
    }

    let passed = total.saturating_sub(failures_count).saturating_sub(skipped);

    let detected = total > 0 || !failures.is_empty();

    ParsedTestResults {
        framework: "junit-xml".to_string(),
        total,
        passed,
        failed: failures_count,
        skipped,
        failures,
        raw_output: output.to_string(),
        format_detected: detected,
    }
}

fn extract_xml_attr(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    tag.find(&pattern).map(|start| {
        let val_start = start + pattern.len();
        let val_end = tag[val_start..]
            .find('"')
            .map(|i| val_start + i)
            .unwrap_or(tag.len());
        tag[val_start..val_end].to_string()
    })
}

fn extract_xml_content(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    if let Some(start) = block.find(&open) {
        let content_start = block[start..].find('>').map(|i| start + i + 1)?;
        let content_end = block[content_start..]
            .find(&close)
            .map(|i| content_start + i)?;
        Some(block[content_start..content_end].trim().to_string())
    } else {
        None
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Cargo test parser ────────────────────────────────────────────

    #[test]
    fn cargo_test_all_passing() {
        let output = "\
running 3 tests
test tests::test_add ... ok
test tests::test_sub ... ok
test tests::test_mul ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out";

        let result = parse_test_output(output, Some("cargo-test"));
        assert_eq!(result.framework, "cargo-test");
        assert_eq!(result.total, 3);
        assert_eq!(result.passed, 3);
        assert_eq!(result.failed, 0);
        assert!(result.failures.is_empty());
        assert!(result.format_detected);
    }

    #[test]
    fn cargo_test_with_failures() {
        let output = "\
running 3 tests
test tests::test_add ... ok
test tests::test_sub ... FAILED
test tests::test_mul ... ok

failures:

---- tests::test_sub stdout ----
thread 'tests::test_sub' panicked at 'assertion failed: `(left == right)`
  left: `3`,
 right: `2`', src/lib.rs:42:5

failures:
    tests::test_sub

test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out";

        let result = parse_test_output(output, Some("cargo-test"));
        assert_eq!(result.total, 3);
        assert_eq!(result.passed, 2);
        assert_eq!(result.failed, 1);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].test_name, "tests::test_sub");
        assert!(result.failures[0]
            .assertion_message
            .contains("assertion failed"));
    }

    #[test]
    fn cargo_test_with_ignored() {
        let output = "\
running 3 tests
test tests::test_add ... ok
test tests::test_slow ... ignored
test tests::test_mul ... ok

test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out";

        let result = parse_test_output(output, Some("cargo-test"));
        assert_eq!(result.total, 3);
        assert_eq!(result.passed, 2);
        assert_eq!(result.skipped, 1);
    }

    // ── Jest parser ──────────────────────────────────────────────────

    #[test]
    fn jest_summary_line() {
        let output = "\
PASS src/utils.test.js
FAIL src/auth.test.js

Test Suites: 1 failed, 1 passed, 2 total
Tests:       2 failed, 5 passed, 7 total
Snapshots:   0 total
Time:        3.45 s";

        let result = parse_test_output(output, Some("jest"));
        assert_eq!(result.framework, "jest");
        assert_eq!(result.passed, 5);
        assert_eq!(result.failed, 2);
    }

    // ── Pytest parser ────────────────────────────────────────────────

    #[test]
    fn pytest_short_summary() {
        let output = "\
tests/test_auth.py::test_login PASSED
tests/test_auth.py::test_register FAILED
tests/test_auth.py::test_logout PASSED

=== short test summary info ===
FAILED tests/test_auth.py::test_register - AssertionError: expected 200 got 401
=== 1 failed, 2 passed ===";

        let result = parse_test_output(output, Some("pytest"));
        assert_eq!(result.framework, "pytest");
        assert_eq!(result.passed, 2);
        assert_eq!(result.failed, 1);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].test_name, "test_register");
        assert_eq!(
            result.failures[0].file_path.as_deref(),
            Some("tests/test_auth.py")
        );
        assert!(result.failures[0]
            .assertion_message
            .contains("expected 200"));
    }

    // ── JUnit XML parser ─────────────────────────────────────────────

    #[test]
    fn junit_xml_basic() {
        let output = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuite name="tests" tests="3" failures="1" skipped="0">
  <testcase name="test_add" classname="math"/>
  <testcase name="test_sub" classname="math">
    <failure message="expected 3 got 2">assertion failed</failure>
  </testcase>
  <testcase name="test_mul" classname="math"/>
</testsuite>"#;

        let result = parse_test_output(output, Some("junit-xml"));
        assert_eq!(result.framework, "junit-xml");
        assert_eq!(result.total, 3);
        assert_eq!(result.failed, 1);
        assert_eq!(result.passed, 2);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].test_name, "math::test_sub");
        assert!(result.failures[0]
            .assertion_message
            .contains("assertion failed"));
    }

    // ── Auto-detection ───────────────────────────────────────────────

    #[test]
    fn auto_detect_cargo() {
        let output = "test tests::test_add ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored";
        let result = parse_test_output(output, None);
        assert_eq!(result.framework, "cargo-test");
    }

    #[test]
    fn auto_detect_unknown() {
        let output = "some random output that doesn't match anything";
        let result = parse_test_output(output, None);
        assert_eq!(result.framework, "unknown");
        assert!(!result.format_detected);
    }

    // ── Fallback ─────────────────────────────────────────────────────

    #[test]
    fn fallback_preserves_raw_output() {
        let output = "nonsense output here";
        let result = parse_test_output(output, Some("nonexistent-framework"));
        assert!(!result.format_detected);
        assert_eq!(result.raw_output, output);
    }
}
