use async_trait::async_trait;
use crate::types::{ContainerInput, ContainerOutput};
use crate::error::Result;

#[async_trait]
pub trait Runtime: Send + Sync {
    async fn run(&self, input: ContainerInput) -> Result<ContainerOutput>;
}

pub struct DockerRuntime;

#[async_trait]
impl Runtime for DockerRuntime {
    async fn run(&self, input: ContainerInput) -> Result<ContainerOutput> {
        crate::container_runner::run_container(input).await
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::Mutex;
    use std::sync::Arc;

    #[derive(Clone)]
    pub struct MockRuntime {
        pub invocations: Arc<Mutex<Vec<ContainerInput>>>,
        pub mock_result: ContainerOutput,
    }

    impl MockRuntime {
        pub fn new(mock_result: ContainerOutput) -> Self {
            Self {
                invocations: Arc::new(Mutex::new(Vec::new())),
                mock_result,
            }
        }
    }

    #[async_trait]
    impl Runtime for MockRuntime {
        async fn run(&self, input: ContainerInput) -> Result<ContainerOutput> {
            self.invocations.lock().unwrap().push(input);
            Ok(self.mock_result.clone())
        }
    }
}
