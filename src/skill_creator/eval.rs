//! Basic Eval Runner Module
//!
//! Runs evaluation tests on skills and generates reports

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

use crate::config::nuclaw_home;
use crate::error::{NuClawError, Result};

/// A single evaluation test case
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCase {
    /// Unique identifier
    pub id: usize,
    /// Test prompt
    pub prompt: String,
    /// Expected output description
    pub expected: Option<String>,
    /// Files to include (optional)
    pub files: Vec<String>,
}

impl EvalCase {
    pub fn new(id: usize, prompt: &str) -> Self {
        Self {
            id,
            prompt: prompt.to_string(),
            expected: None,
            files: vec![],
        }
    }

    pub fn with_expected(mut self, expected: &str) -> Self {
        self.expected = Some(expected.to_string());
        self
    }
}

/// Result of a single eval run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRunResult {
    pub case_id: usize,
    pub with_skill: bool,
    pub output: String,
    pub duration_ms: u64,
    pub tokens: Option<u64>,
    pub passed: Option<bool>,
}

/// Result of evaluating a test case (with and without skill)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub case_id: usize,
    pub prompt: String,
    pub with_skill: EvalRunResult,
    pub without_skill: Option<EvalRunResult>,
    /// Overall pass/fail (comparison based)
    pub improvement: bool,
}

/// Eval runner configuration
#[derive(Debug, Clone)]
pub struct EvalConfig {
    /// Workspace directory for eval outputs
    pub workspace: PathBuf,
    /// Maximum concurrent runs
    pub max_concurrent: usize,
    /// Timeout per run (ms)
    pub timeout_ms: u64,
}

impl Default for EvalConfig {
    fn default() -> Self {
        Self {
            workspace: nuclaw_home().join("skill-workspaces"),
            max_concurrent: 3,
            timeout_ms: 120_000,
        }
    }
}

/// Basic eval runner that stores results and generates markdown reports
pub struct EvalRunner {
    config: EvalConfig,
}

impl EvalRunner {
    pub fn new() -> Self {
        Self {
            config: EvalConfig::default(),
        }
    }

    pub fn with_config(config: EvalConfig) -> Self {
        Self { config }
    }

    /// Run a single eval case (placeholder - actual execution depends on agent runner)
    pub async fn run_case(&self, case: &EvalCase, _skill_path: Option<&str>) -> Result<EvalRunResult> {
        let start = Instant::now();
        
        // Placeholder: in real implementation, this would call AgentRunner
        // For now, simulate a run
        let output = format!(
            "[Eval {}] Processed: {}",
            case.id,
            &case.prompt[..case.prompt.len().min(50)]
        );

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(EvalRunResult {
            case_id: case.id,
            with_skill: true,
            output,
            duration_ms,
            tokens: None,
            passed: None,
        })
    }

    /// Run eval cases and return results
    pub async fn run(&self, cases: &[EvalCase], skill_path: Option<&str>) -> Result<Vec<EvalResult>> {
        let mut results = Vec::new();

        for case in cases {
            let result = self.run_case(case, skill_path).await?;
            
            let eval_result = EvalResult {
                case_id: case.id,
                prompt: case.prompt.clone(),
                with_skill: result,
                without_skill: None,
                improvement: false, // Would be determined by comparison
            };
            
            results.push(eval_result);
        }

        Ok(results)
    }

    /// Generate markdown report from results
    pub fn generate_report(&self, results: &[EvalResult], skill_name: &str) -> String {
        let mut report = String::new();

        // Header
        report.push_str(&format!("# Eval Report: {}\n\n", skill_name));
        report.push_str(&format!(
            "**Total Cases:** {}  \n",
            results.len()
        ));
        report.push_str(&format!(
            "**Date:** {}\n\n",
            chrono_lite_date()
        ));

        // Summary
        report.push_str("## Summary\n\n");
        let total_time: u64 = results.iter().map(|r| r.with_skill.duration_ms).sum();
        let avg_time = if results.is_empty() {
            0
        } else {
            total_time / results.len() as u64
        };
        
        report.push_str(&format!("| Metric | Value |\n"));
        report.push_str(&format!("|--------|-------|\n"));
        report.push_str(&format!("| Total Cases | {} |\n", results.len()));
        report.push_str(&format!("| Total Time | {}ms |\n", total_time));
        report.push_str(&format!("| Avg Time/Case | {}ms |\n\n", avg_time));

        // Results table (compact for Telegram/WhatsApp)
        report.push_str("## Results\n\n");
        report.push_str("| ID | Prompt | Time |\n");
        report.push_str("|----|--------|------|\n");
        
        for result in results {
            let prompt_preview = if result.prompt.len() > 30 {
                format!("{}...", &result.prompt[..30])
            } else {
                result.prompt.clone()
            };
            
            report.push_str(&format!(
                "| {} | {} | {}ms |\n",
                result.case_id,
                prompt_preview.replace('|', "\\|"),
                result.with_skill.duration_ms
            ));
        }

        // Detailed section
        report.push_str("\n## Details\n\n");
        
        for result in results {
            report.push_str(&format!("### Case {}\n", result.case_id));
            report.push_str(&format!("**Prompt:** {}\n\n", result.prompt));
            report.push_str(&format!("**Output:**\n```\n{}\n```\n\n", result.with_skill.output));
        }

        report
    }

    /// Generate compact report for messaging (Telegram/WhatsApp)
    pub fn generate_compact_report(&self, results: &[EvalResult], skill_name: &str) -> String {
        let mut report = format!("📊 *Eval: {}*\n\n", skill_name);

        let total_cases = results.len();
        let total_time: u64 = results.iter().map(|r| r.with_skill.duration_ms).sum();
        let avg_time = if total_cases == 0 { 0 } else { total_time / total_cases as u64 };

        report.push_str(&format!(
            "✅ {} cases | ⏱ {}ms avg\n\n",
            total_cases, avg_time
        ));

        for result in results {
            let prompt_preview = if result.prompt.len() > 25 {
                format!("{}...", &result.prompt[..25])
            } else {
                result.prompt.clone()
            };
            
            report.push_str(&format!(
                "{} • {}ms\n",
                prompt_preview.replace('\n', " "),
                result.with_skill.duration_ms
            ));
        }

        report
    }

    /// Save results to JSON file
    pub fn save_results(&self, results: &[EvalResult], iteration: usize) -> Result<PathBuf> {
        use std::fs;
        
        let dir = self.config.workspace.join(format!("iteration-{}", iteration));
        fs::create_dir_all(&dir).map_err(|e| NuClawError::FileSystem {
            message: format!("Failed to create workspace: {}", e),
        })?;

        let file = dir.join("results.json");
        let json = serde_json::to_string_pretty(results).map_err(|e| NuClawError::FileSystem {
            message: format!("Failed to serialize results: {}", e),
        })?;

        fs::write(&file, json).map_err(|e| NuClawError::FileSystem {
            message: format!("Failed to write results: {}", e),
        })?;

        Ok(file)
    }
}

impl Default for EvalRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple date function without chrono dependency
fn chrono_lite_date() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    
    // Simple YYYY-MM-DD (not accurate but sufficient for tests)
    let days = secs / 86400;
    let years = 1970 + days / 365;
    let remaining_days = days % 365;
    let month = remaining_days / 30 + 1;
    let day = remaining_days % 30 + 1;
    
    format!("{:04}-{:02}-{:02}", years, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_case_new() {
        let case = EvalCase::new(1, "Test prompt");
        assert_eq!(case.id, 1);
        assert_eq!(case.prompt, "Test prompt");
        assert!(case.expected.is_none());
    }

    #[test]
    fn test_eval_case_with_expected() {
        let case = EvalCase::new(1, "Test").with_expected("Expected output");
        assert_eq!(case.expected, Some("Expected output".to_string()));
    }

    #[test]
    fn test_eval_runner_default_config() {
        let runner = EvalRunner::new();
        assert_eq!(runner.config.max_concurrent, 3);
        assert_eq!(runner.config.timeout_ms, 120_000);
    }

    #[tokio::test]
    async fn test_run_single_case() {
        let runner = EvalRunner::new();
        let case = EvalCase::new(1, "Parse this JSON");
        
        let result = runner.run_case(&case, Some("json-parser")).await.unwrap();
        
        assert_eq!(result.case_id, 1);
        assert!(result.with_skill);
    }

    #[tokio::test]
    async fn test_run_multiple_cases() {
        let runner = EvalRunner::new();
        let cases = vec![
            EvalCase::new(1, "First test"),
            EvalCase::new(2, "Second test"),
        ];
        
        let results = runner.run(&cases, Some("test-skill")).await.unwrap();
        
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].case_id, 1);
        assert_eq!(results[1].case_id, 2);
    }

    #[test]
    fn test_generate_report() {
        let runner = EvalRunner::new();
        let results = vec![
            EvalResult {
                case_id: 1,
                prompt: "Test prompt 1".to_string(),
                with_skill: EvalRunResult {
                    case_id: 1,
                    with_skill: true,
                    output: "Output 1".to_string(),
                    duration_ms: 100,
                    tokens: None,
                    passed: None,
                },
                without_skill: None,
                improvement: false,
            },
            EvalResult {
                case_id: 2,
                prompt: "Test prompt 2".to_string(),
                with_skill: EvalRunResult {
                    case_id: 2,
                    with_skill: true,
                    output: "Output 2".to_string(),
                    duration_ms: 200,
                    tokens: None,
                    passed: None,
                },
                without_skill: None,
                improvement: false,
            },
        ];

        let report = runner.generate_report(&results, "test-skill");
        
        assert!(report.contains("Eval Report: test-skill"));
        assert!(report.contains("| Total Cases | 2 |"));
        assert!(report.contains("100ms"));
        assert!(report.contains("200ms"));
    }

    #[test]
    fn test_generate_compact_report() {
        let runner = EvalRunner::new();
        let results = vec![
            EvalResult {
                case_id: 1,
                prompt: "Parse JSON".to_string(),
                with_skill: EvalRunResult {
                    case_id: 1,
                    with_skill: true,
                    output: "Result".to_string(),
                    duration_ms: 150,
                    tokens: None,
                    passed: None,
                },
                without_skill: None,
                improvement: false,
            },
        ];

        let report = runner.generate_compact_report(&results, "json-parser");
        
        assert!(report.contains("📊"));
        assert!(report.contains("1 cases"));
        assert!(report.contains("150ms avg"));
    }

    #[test]
    fn test_save_results() {
        let temp_dir = std::env::temp_dir().join("nuclaw-eval-test");
        let config = EvalConfig {
            workspace: temp_dir.clone(),
            ..Default::default()
        };
        
        let runner = EvalRunner::with_config(config);
        let results = vec![
            EvalResult {
                case_id: 1,
                prompt: "Test".to_string(),
                with_skill: EvalRunResult {
                    case_id: 1,
                    with_skill: true,
                    output: "Output".to_string(),
                    duration_ms: 100,
                    tokens: None,
                    passed: None,
                },
                without_skill: None,
                improvement: false,
            },
        ];

        let path = runner.save_results(&results, 1).unwrap();
        assert!(path.exists());

        // Clean up
        std::fs::remove_dir_all(temp_dir).ok();
    }

    #[test]
    fn test_eval_result_serialization() {
        let result = EvalResult {
            case_id: 1,
            prompt: "Test prompt".to_string(),
            with_skill: EvalRunResult {
                case_id: 1,
                with_skill: true,
                output: "Output".to_string(),
                duration_ms: 100,
                tokens: Some(500),
                passed: Some(true),
            },
            without_skill: None,
            improvement: true,
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: EvalResult = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.case_id, 1);
        assert_eq!(parsed.with_skill.duration_ms, 100);
        assert!(parsed.improvement);
    }
}
