//! Arguments that can be passed to the [`Backup`](crate::command::Command::Backup) command.

use std::path::PathBuf;

use clap::{Args, ValueEnum};
use mini_exercism::api::v2::solution;
use mini_exercism::api::v2::solution::Solution;

/// Command-line arguments accepted by the [`Backup`](crate::command::Command::Backup) command.
#[derive(Debug, Clone, Args)]
pub struct BackupArgs {
    /// Path where to store the downloaded solutions
    pub path: PathBuf,

    /// Exercism.org API token; if unspecified, CLI token will be used instead
    #[arg(long)]
    pub token: Option<String>,

    /// Only download solutions in the given track(s) (can be used multiple times)
    #[arg(short, long)]
    pub track: Vec<String>,

    /// Only download solutions for the given exercise(s) (can be used multiple times)
    #[arg(short, long)]
    pub exercise: Vec<String>,

    /// Only download solutions with the given status (or greater)
    #[arg(short, long, value_enum, default_value_t = SolutionStatus::Submitted)]
    pub status: SolutionStatus,

    /// How to handle solutions that already exist on disk
    #[arg(short, long, value_enum, default_value_t = OverwritePolicy::IfNewer)]
    pub overwrite: OverwritePolicy,

    /// Determine what solutions to back up without downloading them
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// Maximum number of concurrent downloads
    #[arg(short, long, default_value_t = 4)]
    pub max_downloads: usize,
}

impl BackupArgs {
    /// Determines if the given [`Solution`] should be backed up.
    pub fn solution_matches(&self, solution: &Solution) -> bool {
        self.track_matches(&solution.track.name)
            && self.exercise_matches(&solution.exercise.name)
            && self.solution_status_matches(solution.status.try_into().ok())
    }

    fn track_matches(&self, track_name: &str) -> bool {
        self.track.is_empty() || self.track.iter().any(|t| t == track_name)
    }

    fn exercise_matches(&self, exercise_name: &str) -> bool {
        self.exercise.is_empty() || self.exercise.iter().any(|e| e == exercise_name)
    }

    fn solution_status_matches(&self, solution_status: Option<SolutionStatus>) -> bool {
        self.status == SolutionStatus::Submitted
            || solution_status.map_or(false, |st| st >= self.status)
    }
}

/// Possible solution status to filter for (see [`BackupArgs::status`]).
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum SolutionStatus {
    /// At least one iteration has been submitted, but exercise has not been marked as complete
    Submitted,

    /// Exercise has been marked as complete
    Completed,

    /// Exercise has been marked as complete and a solution has been published
    Published,
}

impl TryFrom<solution::Status> for SolutionStatus {
    type Error = solution::Status;

    fn try_from(value: solution::Status) -> Result<Self, Self::Error> {
        match value {
            solution::Status::Iterated => Ok(Self::Submitted),
            solution::Status::Completed => Ok(Self::Completed),
            solution::Status::Published => Ok(Self::Published),
            unsupported_solution_status => Err(unsupported_solution_status),
        }
    }
}

/// Policy used to decide what to do if a solution already exists on disk (see [`BackupArgs::overwrite`]).
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum OverwritePolicy {
    /// Always overwrite existing solutions
    Always,

    /// Overwrite existing solutions if there is a newer version
    IfNewer,

    /// Never overwrite existing solutions
    Never,
}

#[cfg(test)]
mod tests {
    mod backup_args {
        mod solution_matches {
            use std::path::PathBuf;

            use mini_exercism::api::v2::solution;
            use mini_exercism::api::v2::solution::Solution;

            use crate::command::backup::args::{BackupArgs, OverwritePolicy, SolutionStatus};

            fn get_solution(status: Option<solution::Status>) -> Solution {
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

                let mut solution: Solution = serde_json::from_str(json).unwrap();
                if let Some(status) = status {
                    solution.status = status;
                }
                solution
            }

            fn get_args(
                tracks: &[&str],
                exercises: &[&str],
                status: Option<SolutionStatus>,
            ) -> BackupArgs {
                BackupArgs {
                    path: PathBuf::default(),
                    token: None,
                    track: tracks.iter().map(|&t| t.into()).collect(),
                    exercise: exercises.iter().map(|&e| e.into()).collect(),
                    status: status.unwrap_or(SolutionStatus::Submitted),
                    overwrite: OverwritePolicy::IfNewer,
                    dry_run: false,
                    max_downloads: 4,
                }
            }

            fn perform_test(
                tracks: &[&str],
                exercises: &[&str],
                status: Option<SolutionStatus>,
                solution_status: Option<solution::Status>,
                should_match: bool,
            ) {
                let args = get_args(tracks, exercises, status);
                let solution = get_solution(solution_status);
                assert_eq!(should_match, args.solution_matches(&solution));
            }

            fn perform_simple_test(
                tracks: &[&str],
                exercises: &[&str],
                status: Option<SolutionStatus>,
                should_match: bool,
            ) {
                perform_test(tracks, exercises, status, None, should_match);
            }

            #[test]
            fn test_no_filter() {
                perform_simple_test(&[], &[], None, true);
            }

            #[test]
            fn test_track_filter() {
                perform_simple_test(&["rust"], &[], None, true);
                perform_simple_test(&["rust", "clojure"], &[], None, true);
                perform_simple_test(&["clojure"], &[], None, false);
            }

            #[test]
            fn test_exercise_filter() {
                perform_simple_test(&[], &["poker"], None, true);
                perform_simple_test(&[], &["poker", "zebra-puzzle"], None, true);
                perform_simple_test(&[], &["zebra-puzzle"], None, false);
            }

            #[test]
            fn test_solution_filter() {
                perform_simple_test(&[], &[], Some(SolutionStatus::Submitted), true);
                perform_simple_test(&[], &[], Some(SolutionStatus::Completed), true);
                perform_simple_test(&[], &[], Some(SolutionStatus::Published), true);

                // Note: currently, if we pass "Submitted" for the solution status filter
                // (or leave it blank, since it's the default), then "Started" exercises
                // are also downloaded. Maybe that's wrong? Might want to revisit.
                perform_test(
                    &[],
                    &[],
                    Some(SolutionStatus::Submitted),
                    Some(solution::Status::Started),
                    true,
                );
                perform_test(
                    &[],
                    &[],
                    Some(SolutionStatus::Completed),
                    Some(solution::Status::Started),
                    false,
                );
                perform_test(
                    &[],
                    &[],
                    Some(SolutionStatus::Published),
                    Some(solution::Status::Started),
                    false,
                );

                perform_test(
                    &[],
                    &[],
                    Some(SolutionStatus::Submitted),
                    Some(solution::Status::Iterated),
                    true,
                );
                perform_test(
                    &[],
                    &[],
                    Some(SolutionStatus::Completed),
                    Some(solution::Status::Iterated),
                    false,
                );
                perform_test(
                    &[],
                    &[],
                    Some(SolutionStatus::Published),
                    Some(solution::Status::Iterated),
                    false,
                );

                perform_test(
                    &[],
                    &[],
                    Some(SolutionStatus::Submitted),
                    Some(solution::Status::Completed),
                    true,
                );
                perform_test(
                    &[],
                    &[],
                    Some(SolutionStatus::Completed),
                    Some(solution::Status::Completed),
                    true,
                );
                perform_test(
                    &[],
                    &[],
                    Some(SolutionStatus::Published),
                    Some(solution::Status::Completed),
                    false,
                );

                perform_test(
                    &[],
                    &[],
                    Some(SolutionStatus::Submitted),
                    Some(solution::Status::Published),
                    true,
                );
                perform_test(
                    &[],
                    &[],
                    Some(SolutionStatus::Completed),
                    Some(solution::Status::Published),
                    true,
                );
                perform_test(
                    &[],
                    &[],
                    Some(SolutionStatus::Published),
                    Some(solution::Status::Published),
                    true,
                );
            }
        }
    }

    mod solution_status {
        mod try_into {
            use mini_exercism::api::v2::solution;

            use crate::command::backup::args::SolutionStatus;

            #[test]
            fn test_all() {
                assert_eq!(
                    Err::<SolutionStatus, _>(solution::Status::Started),
                    solution::Status::Started.try_into()
                );
                assert_eq!(Ok(SolutionStatus::Submitted), solution::Status::Iterated.try_into());
                assert_eq!(Ok(SolutionStatus::Completed), solution::Status::Completed.try_into());
                assert_eq!(Ok(SolutionStatus::Published), solution::Status::Published.try_into());
                assert_eq!(
                    Err::<SolutionStatus, _>(solution::Status::Unknown),
                    solution::Status::Unknown.try_into()
                );
            }
        }
    }
}
