use std::path::{Path, PathBuf};

use mini_exercism::api::v2::solution::Solution;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupState {
    pub uuid: String,
    pub iterations: Vec<i32>,
}

pub const BACKUP_STATE_FILE_NAME: &str = ".auxiliaire/backup_state.json";
pub const BACKUP_STATE_TEMP_FILE_NAME: &str = ".auxiliaire/backup_state.json.tmp";

impl BackupState {
    pub fn for_solution(solution: &Solution) -> Self {
        Self { uuid: solution.uuid.clone(), iterations: vec![] }
    }

    pub async fn for_backup(solution: &Solution, solution_output_path: &Path) -> Self {
        let mut state_file_path: PathBuf = solution_output_path.into();
        state_file_path.push(BACKUP_STATE_FILE_NAME);

        fs::read_to_string(state_file_path)
            .await
            .map_err(|_| ())
            .and_then(|state_str| serde_json::from_str(&state_str).map_err(|_| ()))
            .unwrap_or_else(|_| Self::for_solution(solution))
    }

    pub fn needs_backup(&self, solution: &Solution) -> bool {
        self.uuid != solution.uuid
            || self
                .iterations
                .last()
                .map(|&iteration_idx| iteration_idx != solution.num_iterations)
                .unwrap_or(true)
    }
}
