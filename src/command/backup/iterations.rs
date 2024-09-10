use std::env;

use mini_exercism::api::v2::iteration::Iteration;

pub fn get_iterations_dir_name() -> String {
    env::var(ITERATIONS_DIR_ENV_VAR_NAME).unwrap_or_else(|_| DEFAULT_ITERATIONS_DIR_NAME.into())
}

pub const ITERATIONS_DIR_ENV_VAR_NAME: &str = "AUXILIAIRE_ITERATIONS_DIR";
pub const DEFAULT_ITERATIONS_DIR_NAME: &str = "_iterations";

#[derive(Debug, Default, Clone)]
pub struct SyncOps {
    pub existing_iterations_to_clean_up: Vec<i32>,
    pub iterations_to_backup: Vec<Iteration>,
}
