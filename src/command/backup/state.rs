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
    pub last_iteration_marker: LastIterationMarker,
}

impl BackupState {
    pub fn for_solution_uuid<U>(solution_uuid: U) -> Self
    where
        U: Into<String>,
    {
        Self { uuid: solution_uuid.into(), ..Self::default() }
    }

    pub fn for_solution(solution: Solution) -> Self {
        Self {
            uuid: solution.uuid,
            last_iteration_marker: solution
                .last_iterated_at
                .map(Into::into)
                .unwrap_or_else(|| solution.num_iterations.into()),
        }
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

    pub fn needs_update(&self, solution: &Solution) -> Result<bool> {
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

        match (&self.last_iteration_marker, solution.last_iterated_at.as_ref(), solution.num_iterations) {
            (LastIterationMarker::None, _, _) => Ok(true),
            (LastIterationMarker::LastIteratedAt(state_lia), Some(sol_lia), _) => Ok(state_lia != sol_lia),
            (&LastIterationMarker::NumIterations(state_ni), _, sol_ni) if state_ni > sol_ni => Err(
                anyhow!(
                    "solution to {}/{} has less iterations ({}) than what we last saw ({}): did you choose the wrong output directory?",
                    solution.track.name,
                    solution.exercise.name,
                    sol_ni,
                    state_ni,
                )
            ),
            (&LastIterationMarker::NumIterations(state_ni), _, sol_ni) => Ok(state_ni != sol_ni),
            (LastIterationMarker::LastIteratedAt(state_lia), None, _) => Err(
                anyhow!(
                    "solution to {}/{} used to have 'last iterated at' timestamp ({}) but no longer has one: did you choose the wrong output directory?",
                    solution.track.name,
                    solution.exercise.name,
                    state_lia,
                )
            ),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LastIterationMarker {
    #[default]
    None,
    LastIteratedAt(String),
    NumIterations(i32),
}

impl From<String> for LastIterationMarker {
    fn from(value: String) -> Self {
        Self::LastIteratedAt(value)
    }
}

impl From<i32> for LastIterationMarker {
    fn from(value: i32) -> Self {
        Self::NumIterations(value)
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
            last_iteration_marker: value.iterations.last().copied().unwrap_or(0).into(),
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
    use assert_matches::assert_matches;

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
            use super::*;

            #[test]
            fn test_for_solution_uuid() {
                let solution = get_solution();
                let state = BackupState::for_solution_uuid(&solution.uuid);

                assert_eq!(solution.uuid, state.uuid);
                assert_eq!(LastIterationMarker::None, state.last_iteration_marker);
                assert_matches!(state.needs_update(&solution), Ok(true));
            }
        }

        mod for_solution {
            use super::*;

            mod with_last_iterated_at {
                use super::*;

                #[test]
                fn test_all() {
                    let mut solution = get_solution();
                    let state = BackupState::for_solution(get_solution());

                    assert_eq!(solution.uuid, state.uuid);
                    assert_matches!(&state.last_iteration_marker, LastIterationMarker::LastIteratedAt(state_lia) => {
                        assert_eq!(solution.last_iterated_at.as_ref(), Some(state_lia));
                    });
                    assert_matches!(state.needs_update(&solution), Ok(false));

                    solution.last_iterated_at = Some("2024-05-07T05:35:43Z".into());
                    assert_matches!(state.needs_update(&solution), Ok(true));
                }
            }

            mod without_last_iterated_at {
                use super::*;

                #[test]
                fn test_all() {
                    let get_solution = || {
                        let mut solution = get_solution();
                        solution.last_iterated_at = None;
                        solution
                    };

                    let mut solution = get_solution();
                    let state = BackupState::for_solution(get_solution());

                    assert_eq!(solution.uuid, state.uuid);
                    assert_matches!(state.last_iteration_marker, LastIterationMarker::NumIterations(state_ni) => {
                        assert_eq!(solution.num_iterations, state_ni);
                    });
                    assert_matches!(state.needs_update(&solution), Ok(false));

                    solution.num_iterations += 1;
                    assert_matches!(state.needs_update(&solution), Ok(true));
                }
            }

            mod errors {
                use super::*;

                #[test]
                fn test_changing_uuid() {
                    let mut solution = get_solution();
                    solution.uuid = "cdbb19fc-5061-47a0-9e5f-e78c72e31fc1".into();
                    let state = BackupState::for_solution(get_solution());

                    assert!(state.needs_update(&solution).is_err());
                }

                #[test]
                fn test_disappearing_last_iterated_at() {
                    let mut solution = get_solution();
                    solution.last_iterated_at = None;
                    let state = BackupState::for_solution(get_solution());

                    assert!(state.needs_update(&solution).is_err());
                }

                #[test]
                fn test_time_traveling_iterations() {
                    let get_solution = || {
                        let mut solution = get_solution();
                        solution.last_iterated_at = None;
                        solution
                    };

                    let mut solution = get_solution();
                    solution.num_iterations -= 1;
                    let state = BackupState::for_solution(get_solution());

                    assert!(state.needs_update(&solution).is_err());
                }
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

            macro_rules! with_backup_state_tests {
                ($manifest_path:ident) => {
                    mod $manifest_path {
                        use super::*;

                        #[tokio::test]
                        async fn test_all_matching() {
                            let solution = get_solution();
                            let path = test_manifest_path(stringify!($manifest_path));
                            let state = BackupState::for_backup(&solution, &path).await;

                            assert_matches!(state.needs_update(&solution), Ok(false));
                        }

                        #[tokio::test]
                        async fn test_with_wrong_uuid() {
                            let solution = get_solution();
                            let path = test_manifest_path(stringify!($manifest_path));
                            let mut state = BackupState::for_backup(&solution, &path).await;
                            state.uuid = "7966df35-bdfc-4f83-9791-2996548160f4".into();

                            assert_matches!(state.needs_update(&solution), Err(_));
                        }

                        #[tokio::test]
                        async fn test_with_previous_iteration() {
                            let solution = get_solution();
                            let path = test_manifest_path(stringify!($manifest_path));
                            let mut state = BackupState::for_backup(&solution, &path).await;
                            state.last_iteration_marker = (solution.num_iterations - 1).into();

                            assert_matches!(state.needs_update(&solution), Ok(true));
                        }

                        #[tokio::test]
                        async fn test_with_future_iteration() {
                            let solution = get_solution();
                            let path = test_manifest_path(stringify!($manifest_path));
                            let mut state = BackupState::for_backup(&solution, &path).await;
                            state.last_iteration_marker = (solution.num_iterations + 1).into();

                            assert_matches!(state.needs_update(&solution), Err(_));
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

                assert_matches!(state.needs_update(&solution), Ok(true));
            }
        }
    }
}
