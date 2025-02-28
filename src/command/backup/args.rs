//! Arguments that can be passed to the [`Backup`](crate::command::Command::Backup) command.

use std::path::PathBuf;

use clap::{Args, ValueEnum};
use mini_exercism::api::v2::iteration::Iteration;
use mini_exercism::api::v2::solution::Solution;
use mini_exercism::api::v2::{iteration, solution};

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
    #[arg(short, long, value_enum, default_value_t = SolutionStatus::Any)]
    pub status: SolutionStatus,

    /// How to handle solutions that already exist on disk
    #[arg(short, long, value_enum, default_value_t = OverwritePolicy::IfNewer)]
    pub overwrite: OverwritePolicy,

    /// Whether to also back up iterations and how
    #[arg(short, long = "iterations", value_enum, default_value_t = IterationsSyncPolicy::DoNotSync)]
    pub iterations_sync_policy: IterationsSyncPolicy,

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

    /// Determines if the given [`Iteration`] should be backed up.
    ///
    /// # Notes
    ///
    /// There are currently no filters applied when fetching iterations,
    /// but we'll only keep the [published](mini_exercism::api::v2::iteration::Iteration::is_published)
    /// ones if our [status filter](Self::status) tells us to.
    pub fn iteration_matches(&self, iteration: &Iteration) -> bool {
        iteration.status != iteration::Status::Deleted
            && (self.status < SolutionStatus::Published || iteration.is_published)
    }

    fn track_matches(&self, track_name: &str) -> bool {
        self.track.is_empty() || self.track.iter().any(|t| t == track_name)
    }

    fn exercise_matches(&self, exercise_name: &str) -> bool {
        self.exercise.is_empty() || self.exercise.iter().any(|e| e == exercise_name)
    }

    fn solution_status_matches(&self, solution_status: Option<SolutionStatus>) -> bool {
        solution_status.is_some_and(|st| st >= self.status)
    }
}

/// Possible solution status to filter for (see [`BackupArgs::status`]).
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum SolutionStatus {
    /// Do not filter solutions based on their status
    #[value(alias = "started")]
    Any,

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
            solution::Status::Started => Ok(Self::Any),
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
    #[value(alias = "if-new")]
    IfNewer,

    /// Never overwrite existing solutions
    Never,
}

/// Policy used to decide whether to also back up iterations (see [`BackupArgs::iterations_sync_policy`]).
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum IterationsSyncPolicy {
    /// Do not back up iterations
    #[value(alias = "no")]
    DoNotSync,

    /// Back up new iterations, do not touch existing iterations on disk
    New,

    /// Back up new iterations and remove existing iterations on disk that no longer exist
    #[value(aliases = ["f", "full"])]
    FullSync,

    /// Remove existing iterations on disk
    CleanUp,
}

impl IterationsSyncPolicy {
    /// Whether this policy implies synchronizing iterations at all, regardless of how.
    pub fn sync(&self) -> bool {
        self != &Self::DoNotSync
    }

    /// Whether this policy implies backing up new iterations.
    pub fn backup_new(&self) -> bool {
        self == &Self::New || self == &Self::FullSync
    }

    /// Whether this policy implies cleaning up old iterations on disk that no
    /// longer exist or are no longer published.
    pub fn clean_up_old(&self) -> bool {
        self == &Self::FullSync || self == &Self::CleanUp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod backup_args {
        use super::*;

        mod solution_matches {
            use super::*;

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
                    track: tracks.iter().copied().map(Into::into).collect(),
                    exercise: exercises.iter().copied().map(Into::into).collect(),
                    status: status.unwrap_or(SolutionStatus::Any),
                    overwrite: OverwritePolicy::IfNewer,
                    iterations_sync_policy: IterationsSyncPolicy::DoNotSync,
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
                perform_simple_test(&[], &[], Some(SolutionStatus::Any), true);
                perform_simple_test(&[], &[], Some(SolutionStatus::Submitted), true);
                perform_simple_test(&[], &[], Some(SolutionStatus::Completed), true);
                perform_simple_test(&[], &[], Some(SolutionStatus::Published), true);

                perform_test(
                    &[],
                    &[],
                    Some(SolutionStatus::Any),
                    Some(solution::Status::Started),
                    true,
                );
                perform_test(
                    &[],
                    &[],
                    Some(SolutionStatus::Submitted),
                    Some(solution::Status::Started),
                    false,
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
                    Some(SolutionStatus::Any),
                    Some(solution::Status::Iterated),
                    true,
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
                    Some(SolutionStatus::Any),
                    Some(solution::Status::Completed),
                    true,
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
                    Some(SolutionStatus::Any),
                    Some(solution::Status::Published),
                    true,
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

        mod iteration_matches {
            use super::*;

            fn get_iteration(
                status: Option<iteration::Status>,
                is_published: Option<bool>,
            ) -> Iteration {
                let json = r#"{
                    "uuid": "e44cbc866b1d42e5b276fd2afabb8fe0",
                    "submission_uuid": "f19960cbe3b344a58f7728db53ce47f9",
                    "idx": 13,
                    "status": "no_automated_feedback",
                    "num_essential_automated_comments": 0,
                    "num_actionable_automated_comments": 0,
                    "num_non_actionable_automated_comments": 0,
                    "num_celebratory_automated_comments": 0,
                    "submission_method": "cli",
                    "created_at": "2023-05-07T05:35:43Z",
                    "tests_status": "passed",
                    "is_published": true,
                    "is_latest": true,
                    "links": {
                        "self": "https://exercism.org/tracks/rust/exercises/poker/iterations?idx=13",
                        "automated_feedback": "https://exercism.org/api/v2/solutions/00c717b68e1b4213b316df82636f5e0f/iterations/e44cbc866b1d42e5b276fd2afabb8fe0/automated_feedback",
                        "delete": "https://exercism.org/api/v2/solutions/00c717b68e1b4213b316df82636f5e0f/iterations/e44cbc866b1d42e5b276fd2afabb8fe0",
                        "solution": "https://exercism.org/tracks/rust/exercises/poker",
                        "test_run": "https://exercism.org/api/v2/solutions/00c717b68e1b4213b316df82636f5e0f/submissions/f19960cbe3b344a58f7728db53ce47f9/test_run",
                        "files": "https://exercism.org/api/v2/solutions/00c717b68e1b4213b316df82636f5e0f/submissions/f19960cbe3b344a58f7728db53ce47f9/files"
                    }
                }"#;

                let mut iteration: Iteration = serde_json::from_str(json).unwrap();
                if let Some(status) = status {
                    iteration.status = status;
                }
                if let Some(is_published) = is_published {
                    iteration.is_published = is_published;
                }
                iteration
            }

            fn get_args(status: Option<SolutionStatus>) -> BackupArgs {
                BackupArgs {
                    path: PathBuf::default(),
                    token: None,
                    track: vec![],
                    exercise: vec![],
                    status: status.unwrap_or(SolutionStatus::Any),
                    overwrite: OverwritePolicy::IfNewer,
                    iterations_sync_policy: IterationsSyncPolicy::FullSync,
                    dry_run: false,
                    max_downloads: 4,
                }
            }

            fn perform_test(
                status: Option<SolutionStatus>,
                iteration_status: Option<iteration::Status>,
                iteration_is_published: Option<bool>,
                should_match: bool,
            ) {
                let args = get_args(status);
                let iteration = get_iteration(iteration_status, iteration_is_published);
                assert_eq!(should_match, args.iteration_matches(&iteration));
            }

            #[test]
            fn test_iteration_status() {
                perform_test(None, Some(iteration::Status::Untested), None, true);
                perform_test(None, Some(iteration::Status::Testing), None, true);
                perform_test(None, Some(iteration::Status::TestsFailed), None, true);
                perform_test(None, Some(iteration::Status::Analyzing), None, true);
                perform_test(None, Some(iteration::Status::EssentialAutomatedFeedback), None, true);
                perform_test(
                    None,
                    Some(iteration::Status::ActionableAutomatedFeedback),
                    None,
                    true,
                );
                perform_test(
                    None,
                    Some(iteration::Status::CelebratoryAutomatedFeedback),
                    None,
                    true,
                );
                perform_test(
                    None,
                    Some(iteration::Status::NonActionableAutomatedFeedback),
                    None,
                    true,
                );
                perform_test(None, Some(iteration::Status::NoAutomatedFeedback), None, true);
                perform_test(None, Some(iteration::Status::Deleted), None, false);
            }

            #[test]
            fn test_iteration_is_published() {
                perform_test(Some(SolutionStatus::Any), None, Some(true), true);
                perform_test(Some(SolutionStatus::Submitted), None, Some(true), true);
                perform_test(Some(SolutionStatus::Completed), None, Some(true), true);
                perform_test(Some(SolutionStatus::Published), None, Some(true), true);

                perform_test(Some(SolutionStatus::Any), None, Some(false), true);
                perform_test(Some(SolutionStatus::Submitted), None, Some(false), true);
                perform_test(Some(SolutionStatus::Completed), None, Some(false), true);
                perform_test(Some(SolutionStatus::Published), None, Some(false), false);
            }
        }
    }

    mod solution_status {
        use super::*;

        mod try_into {
            use mini_exercism::api::v2::solution;

            use super::*;

            #[test]
            fn test_all() {
                assert_eq!(Ok(SolutionStatus::Any), solution::Status::Started.try_into());
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

    mod iterations_sync_policy {
        use super::*;

        fn perform_checks(
            policy: IterationsSyncPolicy,
            expect_sync: bool,
            expect_backup_new: bool,
            expect_clean_up_old: bool,
        ) {
            assert_eq!(expect_sync, policy.sync());
            assert_eq!(expect_backup_new, policy.backup_new());
            assert_eq!(expect_clean_up_old, policy.clean_up_old());
        }

        #[test]
        fn test_all() {
            let expectations = [
                (IterationsSyncPolicy::DoNotSync, false, false, false),
                (IterationsSyncPolicy::New, true, true, false),
                (IterationsSyncPolicy::FullSync, true, true, true),
                (IterationsSyncPolicy::CleanUp, true, false, true),
            ];

            for (policy, expect_sync, expect_backup_new, expect_clean_up_old) in expectations {
                perform_checks(policy, expect_sync, expect_backup_new, expect_clean_up_old);
            }
        }
    }
}
