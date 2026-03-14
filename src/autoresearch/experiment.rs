use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Metric {
    ValBpb,
    ValLoss,
    TrainLoss,
}

impl Metric {
    pub fn name(&self) -> &str {
        match self {
            Metric::ValBpb => "val_bpb",
            Metric::ValLoss => "val_loss",
            Metric::TrainLoss => "train_loss",
        }
    }

    pub fn lower_is_better(&self) -> bool {
        true
    }
}

impl Default for Metric {
    fn default() -> Self {
        Metric::ValBpb
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentConfig {
    pub time_budget_secs: u64,
    pub max_iterations: u32,
    pub metric: Metric,
    pub program_path: PathBuf,
    pub train_script_path: PathBuf,
    pub early_stop_patience: u32,
    pub output_dir: PathBuf,
}

impl Default for ExperimentConfig {
    fn default() -> Self {
        Self {
            time_budget_secs: 300,
            max_iterations: 100,
            metric: Metric::default(),
            program_path: PathBuf::from("program.md"),
            train_script_path: PathBuf::from("train.py"),
            early_stop_patience: 5,
            output_dir: PathBuf::from("experiments"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentResult {
    pub iteration: u32,
    pub metric_value: f64,
    pub is_improvement: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub duration_secs: u64,
    pub train_script: String,
    pub metric_name: String,
}

impl ExperimentResult {
    pub fn new(
        iteration: u32,
        metric_value: f64,
        is_improvement: bool,
        duration_secs: u64,
        train_script: String,
        metric_name: &str,
    ) -> Self {
        Self {
            iteration,
            metric_value,
            is_improvement,
            timestamp: chrono::Utc::now(),
            duration_secs,
            train_script,
            metric_name: metric_name.to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExperimentHistory {
    results: Vec<ExperimentResult>,
    max_size: usize,
}

impl ExperimentHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            results: Vec::with_capacity(max_size),
            max_size,
        }
    }

    pub fn push(&mut self, result: ExperimentResult) {
        if self.results.len() >= self.max_size {
            self.results.remove(0);
        }
        self.results.push(result);
    }

    pub fn results(&self) -> &[ExperimentResult] {
        &self.results
    }

    pub fn best(&self) -> Option<&ExperimentResult> {
        self.results
            .iter()
            .min_by(|a, b| a.metric_value.partial_cmp(&b.metric_value).unwrap())
    }

    pub fn latest(&self) -> Option<&ExperimentResult> {
        self.results.last()
    }

    pub fn count(&self) -> usize {
        self.results.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_default() {
        let metric = Metric::default();
        assert_eq!(metric.name(), "val_bpb");
    }

    #[test]
    fn test_experiment_config_default() {
        let config = ExperimentConfig::default();
        assert_eq!(config.time_budget_secs, 300);
        assert_eq!(config.max_iterations, 100);
    }

    #[test]
    fn test_experiment_history() {
        let mut history = ExperimentHistory::new(3);
        history.push(ExperimentResult::new(
            1,
            2.5,
            true,
            100,
            "code1".to_string(),
            "val_bpb",
        ));
        history.push(ExperimentResult::new(
            2,
            2.3,
            true,
            100,
            "code2".to_string(),
            "val_bpb",
        ));

        assert_eq!(history.count(), 2);
        assert_eq!(history.best().unwrap().metric_value, 2.3);
    }
}
