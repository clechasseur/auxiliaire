use std::path::Path;

use anyhow::anyhow;
use mini_exercism::api::v2::solution::Solution;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::Result;

pub const AUXILIAIRE_STATE_DIR_NAME: &str = ".auxiliaire";
pub const BACKUP_STATE_FILE_NAME: &str = ".auxiliaire/backup_state.json";
pub const BACKUP_STATE_TEMP_FILE_NAME: &str = ".auxiliaire/backup_state.json.tmp";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BackupState {
    pub uuid: String,
    pub num_iterations: i32,
    pub saved_iterations: Vec<i32>,
}

impl BackupState {
    pub fn for_solution_uuid<U>(solution_uuid: U) -> Self
    where
        U: Into<String>,
    {
        Self { uuid: solution_uuid.into(), ..Self::default() }
    }

    pub async fn for_backup(solution: &Solution, solution_output_path: &Path) -> Self {
        let mut state_file_path = solution_output_path.to_path_buf();
        state_file_path.push(BACKUP_STATE_FILE_NAME);

        fs::read_to_string(state_file_path)
            .await
            .map_err(|_| ())
            .and_then(|state_str| {
                serde_json::from_str::<PersistedBackupState>(&state_str)
                    .map(PersistedBackupState::revise)
                    .map_err(|_| ())
            })
            .unwrap_or_else(|_| Self::for_solution_uuid(&solution.uuid))
    }

    pub fn needs_backup(&self, solution: &Solution) -> Result<bool> {
        if self.uuid != solution.uuid {
            return Err(
                anyhow!(
                    "solution to {}/{} has a different uuid ({}) that what we last saw ({}): did you choose the wrong output directory?",
                    solution.track.name,
                    solution.exercise.name,
                    solution.uuid,
                    self.uuid,
                )
            );
        }

        if self.num_iterations > solution.num_iterations {
            return Err(
                anyhow!(
                    "solution to {}/{} has less iterations ({}) than what we last saw ({}): did you choose the wrong output directory?",
                    solution.track.name,
                    solution.exercise.name,
                    solution.num_iterations,
                    self.num_iterations,
                )
            );
        }

        Ok(self.num_iterations != solution.num_iterations)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct V1BackupState {
    pub uuid: String,
    pub iterations: Vec<i32>,
}

impl From<V1BackupState> for BackupState {
    fn from(value: V1BackupState) -> Self {
        Self {
            uuid: value.uuid,
            num_iterations: value.iterations.last().copied().unwrap_or(0),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum PersistedBackupState {
    Latest(BackupState),
    V1(V1BackupState),
}

impl PersistedBackupState {
    fn revise(self) -> BackupState {
        match self {
            Self::Latest(state) => state,
            Self::V1(state) => state.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        mod for_solution_uuid {
            use assert_matches::assert_matches;

            use super::*;

            #[test]
            fn test_for_solution() {
                let solution = get_solution();
                let state = BackupState::for_solution_uuid(&solution.uuid);

                assert_matches!(state.needs_backup(&solution), Ok(true));
            }
        }

        mod for_backup {
            use std::path::PathBuf;

            use assert_matches::assert_matches;

            use super::*;

            fn test_manifest_path(part: &str) -> PathBuf {
                [env!("CARGO_MANIFEST_DIR"), "resources", "tests", part, "rust", "poker"]
                    .iter()
                    .collect()
            }

            macro_rules! with_backup_state_tests {
                ($manifest_path:ident) => {
                    mod $manifest_path {
                        use super::*;

                        #[tokio::test]
                        async fn test_all_matching() {
                            let solution = get_solution();
                            let path = test_manifest_path(stringify!($manifest_path));
                            let state = BackupState::for_backup(&solution, &path).await;

                            assert_matches!(state.needs_backup(&solution), Ok(false));
                        }

                        #[tokio::test]
                        async fn test_with_wrong_uuid() {
                            let solution = get_solution();
                            let path = test_manifest_path(stringify!($manifest_path));
                            let mut state = BackupState::for_backup(&solution, &path).await;
                            state.uuid = "foo".into();

                            assert_matches!(state.needs_backup(&solution), Err(_));
                        }

                        #[tokio::test]
                        async fn test_with_previous_iteration() {
                            let solution = get_solution();
                            let path = test_manifest_path(stringify!($manifest_path));
                            let mut state = BackupState::for_backup(&solution, &path).await;
                            state.num_iterations = solution.num_iterations - 1;

                            assert_matches!(state.needs_backup(&solution), Ok(true));
                        }

                        #[tokio::test]
                        async fn test_with_future_iteration() {
                            let solution = get_solution();
                            let path = test_manifest_path(stringify!($manifest_path));
                            let mut state = BackupState::for_backup(&solution, &path).await;
                            state.num_iterations = solution.num_iterations + 1;

                            assert_matches!(state.needs_backup(&solution), Err(_));
                        }
                    }
                };
            }

            with_backup_state_tests!(with_backup_state);
            with_backup_state_tests!(with_v1_backup_state);

            #[tokio::test]
            async fn test_without_backup_state() {
                let solution = get_solution();
                let path = test_manifest_path("without_backup_state");
                let state = BackupState::for_backup(&solution, &path).await;

                assert_matches!(state.needs_backup(&solution), Ok(true));
            }
        }
    }
}
