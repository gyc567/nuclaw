use crate::autoresearch::experiment::Metric;
use regex::Regex;
use std::fs;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvalError {
    #[error("Failed to read output: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("No metric found in output")]
    NoMetricFound,
    #[error("Failed to parse metric: {0}")]
    ParseError(String),
}

pub struct Evaluator {
    metric: Metric,
}

impl Evaluator {
    pub fn new(metric: Metric) -> Self {
        Self { metric }
    }

    pub fn evaluate(&self, output: &str) -> Result<f64, EvalError> {
        match self.metric {
            Metric::ValBpb => self.parse_val_bpb(output),
            Metric::ValLoss => self.parse_val_loss(output),
            Metric::TrainLoss => self.parse_train_loss(output),
        }
    }

    fn parse_val_bpb(&self, output: &str) -> Result<f64, EvalError> {
        let patterns = [
            r"val_bpb:\s*([0-9.]+)",
            r"validation bits per byte:\s*([0-9.]+)",
            r"val_bpb\s*=\s*([0-9.]+)",
        ];
        self.parse_with_patterns(output, &patterns)
    }

    fn parse_val_loss(&self, output: &str) -> Result<f64, EvalError> {
        let patterns = [r"val_loss:\s*([0-9.]+)", r"validation loss:\s*([0-9.]+)"];
        self.parse_with_patterns(output, &patterns)
    }

    fn parse_train_loss(&self, output: &str) -> Result<f64, EvalError> {
        let patterns = [r"train_loss:\s*([0-9.]+)", r"training loss:\s*([0-9.]+)"];
        self.parse_with_patterns(output, &patterns)
    }

    fn parse_with_patterns(&self, output: &str, patterns: &[&str]) -> Result<f64, EvalError> {
        for pattern in patterns {
            if let Ok(v) = self.try_parse(output, pattern) {
                return Ok(v);
            }
        }
        Err(EvalError::NoMetricFound)
    }

    fn try_parse(&self, output: &str, pattern: &str) -> Result<f64, EvalError> {
        let re = Regex::new(pattern).map_err(|e| EvalError::ParseError(e.to_string()))?;
        if let Some(caps) = re.captures(output) {
            if let Some(m) = caps.get(1) {
                return m
                    .as_str()
                    .parse::<f64>()
                    .map_err(|e| EvalError::ParseError(e.to_string()));
            }
        }
        Err(EvalError::NoMetricFound)
    }

    pub fn evaluate_file(&self, path: &str) -> Result<f64, EvalError> {
        let content = fs::read_to_string(path)?;
        self.evaluate(&content)
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new(Metric::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_val_bpb() {
        let evaluator = Evaluator::new(Metric::ValBpb);
        let output = "Step 100, train_loss: 1.234, val_bpb: 2.345";
        let result = evaluator.evaluate(output).unwrap();
        assert!((result - 2.345).abs() < 0.001);
    }

    #[test]
    fn test_parse_val_loss() {
        let evaluator = Evaluator::new(Metric::ValLoss);
        let output = "val_loss: 1.567";
        let result = evaluator.evaluate(output).unwrap();
        assert!((result - 1.567).abs() < 0.001);
    }

    #[test]
    fn test_no_metric() {
        let evaluator = Evaluator::new(Metric::ValBpb);
        let output = "some random output";
        assert!(evaluator.evaluate(output).is_err());
    }
}
