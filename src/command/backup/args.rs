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

    /// Determine what solutions to backup without downloading them
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
