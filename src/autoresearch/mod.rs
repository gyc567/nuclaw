pub mod evaluator;
pub mod experiment;
pub mod program;
pub mod runner;

pub use evaluator::{EvalError, Evaluator};
pub use experiment::{ExperimentConfig, ExperimentHistory, ExperimentResult, Metric};
pub use program::{Program, ProgramError};
pub use runner::{AutoResearchRunner, RunnerError};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        let _ = Evaluator::default();
        let _ = ExperimentConfig::default();
        let _ = Metric::default();
        let _ = Program::default_program();
    }
}
