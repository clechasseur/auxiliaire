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

#[cfg(test)]
mod tests {
    use mini_exercism::api::v2::solution::Solution;

    fn get_solution() -> Solution {
        let json = r#"{
            "uuid": "00c717b68e1b4213b316df82636f5e0f",
            "private_url": "https://exercism.org/tracks/rust/exercises/poker",
            "public_url": "https://exercism.org/tracks/rust/exercises/poker/solutions/clechasseur",
            "status": "published",
            "mentoring_status": "finished",
            "published_iteration_head_tests_status": "passed",
            "has_notifications": false,
            "num_views": 0,
            "num_stars": 0,
            "num_comments": 0,
            "num_iterations": 13,
            "num_loc": 252,
            "is_out_of_date": false,
            "published_at": "2023-05-08T00:02:21Z",
            "completed_at": "2023-05-08T00:02:21Z",
            "updated_at": "2023-08-27T07:06:01Z",
            "last_iterated_at": "2023-05-07T05:35:43Z",
            "exercise": {
                "slug": "poker",
                "title": "Poker",
                "icon_url": "https://assets.exercism.org/exercises/poker.svg"
            },
            "track": {
                "slug": "rust",
                "title": "Rust",
                "icon_url": "https://assets.exercism.org/tracks/rust.svg"
            }
        }"#;

        serde_json::from_str(json).unwrap()
    }

    mod backup_state {
        use super::*;
        use crate::command::backup::state::BackupState;

        mod for_solution {
            use super::*;

            #[test]
            fn test_for_solution() {
                let solution = get_solution();
                let state = BackupState::for_solution(&solution);

                assert!(state.needs_backup(&solution));
            }
        }

        mod for_backup {
            use std::path::PathBuf;

            use super::*;

            fn test_manifest_path(part: &str) -> PathBuf {
                [env!("CARGO_MANIFEST_DIR"), "resources", "tests", part, "rust", "poker"]
                    .iter()
                    .collect()
            }

            #[tokio::test]
            async fn test_all_matching() {
                let solution = get_solution();
                let path = test_manifest_path("with_backup_state");
                let state = BackupState::for_backup(&solution, &path).await;

                assert!(!state.needs_backup(&solution));
            }

            #[tokio::test]
            async fn test_with_wrong_uuid() {
                let solution = get_solution();
                let path = test_manifest_path("with_backup_state");
                let mut state = BackupState::for_backup(&solution, &path).await;
                state.uuid = "foo".into();

                assert!(state.needs_backup(&solution));
            }

            #[tokio::test]
            async fn test_with_previous_iteration() {
                let solution = get_solution();
                let path = test_manifest_path("with_backup_state");
                let mut state = BackupState::for_backup(&solution, &path).await;
                state.iterations = vec![solution.num_iterations - 1];

                assert!(state.needs_backup(&solution));
            }

            #[tokio::test]
            async fn test_with_future_iteration() {
                let solution = get_solution();
                let path = test_manifest_path("with_backup_state");
                let mut state = BackupState::for_backup(&solution, &path).await;
                state.iterations = vec![solution.num_iterations + 1];

                assert!(state.needs_backup(&solution));
            }

            #[tokio::test]
            async fn test_without_backup_state() {
                let solution = get_solution();
                let path = test_manifest_path("without_backup_state");
                let state = BackupState::for_backup(&solution, &path).await;

                assert!(state.needs_backup(&solution));
            }
        }
    }
}
