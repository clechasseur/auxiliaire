use std::env;

pub fn get_iterations_dir_name() -> String {
    env::var(ITERATIONS_DIR_ENV_VAR_NAME).unwrap_or_else(|_| DEFAULT_ITERATIONS_DIR_NAME.into())
}

pub const ITERATIONS_DIR_ENV_VAR_NAME: &str = "AUXILIAIRE_ITERATIONS_DIR";
pub const DEFAULT_ITERATIONS_DIR_NAME: &str = "_iterations";
