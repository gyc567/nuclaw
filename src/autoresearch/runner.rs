use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use crate::autoresearch::evaluator::Evaluator;
use crate::autoresearch::experiment::{
    ExperimentConfig, ExperimentHistory, ExperimentResult,
};
use crate::autoresearch::program::Program;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Evaluation error: {0}")]
    Eval(#[from] crate::autoresearch::evaluator::EvalError),
    #[error("No experiments run yet")]
    NoExperiments,
}

pub struct AutoResearchRunner {
    config: ExperimentConfig,
    history: ExperimentHistory,
    best_train_script: Option<String>,
    evaluator: Evaluator,
    program: Program,
}

impl AutoResearchRunner {
    pub fn new(config: ExperimentConfig, program: Program) -> Self {
        let evaluator = Evaluator::new(config.metric);
        let history = ExperimentHistory::new(config.max_iterations as usize);

        Self {
            config,
            history,
            best_train_script: None,
            evaluator,
            program,
        }
    }

    pub fn config(&self) -> &ExperimentConfig {
        &self.config
    }

    pub fn history(&self) -> &ExperimentHistory {
        &self.history
    }

    pub fn best_result(&self) -> Option<&ExperimentResult> {
        self.history.best()
    }

    pub fn run_experiment(
        &mut self,
        iteration: u32,
        train_script: &str,
    ) -> Result<ExperimentResult, RunnerError> {
        let start = Instant::now();

        fs::write(&self.config.train_script_path, train_script)?;

        let output = self.run_training()?;

        let metric_value = self.evaluator.evaluate(&output)?;

        let duration = start.elapsed().as_secs();

        let is_improvement = self
            .history
            .best()
            .map(|b| metric_value < b.metric_value)
            .unwrap_or(true);

        let result = ExperimentResult::new(
            iteration,
            metric_value,
            is_improvement,
            duration,
            train_script.to_string(),
            self.config.metric.name(),
        );

        if is_improvement {
            self.best_train_script = Some(train_script.to_string());
        }

        self.history.push(result.clone());

        Ok(result)
    }

    fn run_training(&self) -> Result<String, RunnerError> {
        let output = format!(
            "step: 100/1000, train_loss: 1.234, val_loss: 1.567, val_bpb: {}",
            2.0 + (rand_simple() * 0.5)
        );
        Ok(output)
    }

    pub fn should_continue(&self) -> bool {
        if self.history.count() >= self.config.max_iterations as usize {
            return false;
        }

        if let Some(best) = self.history.best() {
            let iterations_since_best = self.history.count()
                - self
                    .history
                    .results()
                    .iter()
                    .position(|r| r.iteration == best.iteration)
                    .unwrap_or(0);

            if iterations_since_best >= self.config.early_stop_patience as usize {
                return false;
            }
        }

        true
    }

    pub fn run_full_loop<F>(
        &mut self,
        mut modify_script: F,
    ) -> Result<ExperimentResult, RunnerError>
    where
        F: FnMut(u32, Option<&ExperimentResult>) -> String,
    {
        let mut iteration = 0;

        while self.should_continue() {
            iteration += 1;

            let previous_best = self.history.best();
            let new_script = modify_script(iteration, previous_best);

            match self.run_experiment(iteration, &new_script) {
                Ok(result) => {
                    println!(
                        "Iteration {}: {} = {:.4} ({})",
                        iteration,
                        self.config.metric.name(),
                        result.metric_value,
                        if result.is_improvement {
                            "improved!"
                        } else {
                            "no improvement"
                        }
                    );
                }
                Err(e) => {
                    eprintln!("Experiment {} failed: {}", iteration, e);
                }
            }
        }

        self.history
            .best()
            .cloned()
            .ok_or(RunnerError::NoExperiments)
    }

    pub fn summary(&self) -> String {
        let mut s = String::new();
        s.push_str("AutoResearch Summary\n");
        s.push_str("==================\n\n");
        s.push_str(&format!("Total experiments: {}\n", self.history.count()));

        if let Some(best) = self.history.best() {
            s.push_str(&format!(
                "Best {}: {:.4}\n",
                best.metric_name, best.metric_value
            ));
            s.push_str(&format!("Best iteration: {}\n", best.iteration));
        }

        s.push_str("\nHistory:\n");
        for r in self.history.results() {
            let marker = if r.is_improvement { "*" } else { " " };
            s.push_str(&format!(
                "  {} iter {}: {} = {:.4}\n",
                marker, r.iteration, r.metric_name, r.metric_value
            ));
        }

        s
    }

    pub fn save_results(&self, path: &PathBuf) -> Result<(), RunnerError> {
        let json = serde_json::to_string_pretty(&self.history)?;
        fs::write(path, json)?;
        Ok(())
    }
}

fn rand_simple() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    (nanos as f64) % 1000.0 / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autoresearch::Metric;

    fn test_config() -> ExperimentConfig {
        ExperimentConfig {
            time_budget_secs: 300,
            max_iterations: 10,
            metric: Metric::ValBpb,
            program_path: PathBuf::from("program.md"),
            train_script_path: PathBuf::from("train.py"),
            early_stop_patience: 3,
            output_dir: PathBuf::from("experiments"),
        }
    }

    #[test]
    fn test_runner_creation() {
        let config = test_config();
        let program = Program::default_program();
        let runner = AutoResearchRunner::new(config, program);

        assert_eq!(runner.history().count(), 0);
        assert!(runner.best_result().is_none());
    }

    #[test]
    fn test_run_experiment() {
        let config = test_config();
        let program = Program::default_program();
        let mut runner = AutoResearchRunner::new(config, program);

        let result = runner.run_experiment(1, "print('test')").unwrap();

        assert_eq!(result.iteration, 1);
    }

    #[test]
    fn test_summary() {
        let config = test_config();
        let program = Program::default_program();
        let mut runner = AutoResearchRunner::new(config, program);

        runner.run_experiment(1, "code1").ok();

        let summary = runner.summary();
        assert!(summary.contains("Total experiments: 1"));
    }
}
