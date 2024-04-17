//! Definition of the [`Backup`](crate::command::Command::Backup) command.

pub mod args;
#[macro_use]
mod detail;
mod state;

use std::collections::HashSet;
use std::panic::resume_unwind;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use futures::StreamExt;
use mini_exercism::api;
use mini_exercism::api::v2::solution::Solution;
use mini_exercism::api::v2::solutions;
use mini_exercism::cli::get_cli_credentials;
use mini_exercism::core::Credentials;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::{fs, spawn};
use tracing::{debug, enabled, info, instrument, trace, Level};

use crate::command::backup::args::{BackupArgs, OverwritePolicy};
use crate::command::backup::state::{
    BackupState, BACKUP_STATE_FILE_NAME, BACKUP_STATE_TEMP_FILE_NAME,
};
use crate::limiter::Limiter;
use crate::task_pool::TaskPool;
use crate::Result;

/// Command wrapper used for the [`Backup`](crate::command::Command::Backup) command.
///
/// # Notes
///
/// The [`new`](BackupCommand::new) method returns a [`BackupCommand`] wrapped in an [`Arc`],
/// because it is needed to adequately create asynchronous task. To use:
///
/// ```no_run
/// # use auxiliaire::command::backup::args::BackupArgs;
/// use auxiliaire::command::backup::BackupCommand;
///
/// # async fn perform_backup(args: BackupArgs) -> auxiliaire::Result<()> {
/// let backup_command = BackupCommand::new(args, None)?;
/// BackupCommand::execute(backup_command).await
/// # }
/// ```
#[derive(Debug)]
pub struct BackupCommand {
    args: BackupArgs,
    v1_client: api::v1::Client,
    v2_client: api::v2::Client,
    limiter: Limiter,
}

impl BackupCommand {
    /// Creates a new [`BackupCommand`] using the provided [`args`](BackupArgs).
    ///
    /// The `api_base_url` parameter should only be set to test using a different Exercism local endpoint.
    pub fn new(args: BackupArgs, api_base_url: Option<&str>) -> Result<Arc<Self>> {
        let http_client = reqwest::Client::builder()
            .build()
            .with_context(|| "failed to create HTTP client")?;
        let credentials = args
            .token
            .as_ref()
            .map(|token| Ok(Credentials::from_api_token(token)))
            .unwrap_or_else(|| {
                get_cli_credentials().with_context(|| "failed to get Exercism CLI credentials")
            })?;

        let v1_client = build_client!(api::v1::Client, http_client, credentials, api_base_url);
        let v2_client = build_client!(api::v2::Client, http_client, credentials, api_base_url);
        let limiter = Limiter::new(args.max_downloads);

        Ok(Arc::new(Self { args, v1_client, v2_client, limiter }))
    }

    /// Execute the backup operation.
    ///
    /// See [struct description](Self) for details on how to call this method.
    #[instrument(skip_all)]
    pub async fn execute(this: Arc<Self>) -> Result<()> {
        info!("Starting Exercism solutions backup to {}", this.args.path.display());
        trace!(?this.args);

        this.create_output_directory(&this.args.path).await?;

        let output_path = this.args.path.canonicalize().with_context(|| {
            format!("failed to get absolute path for output directory {}", this.args.path.display())
        })?;
        trace!(output_path = %output_path.display());

        match spawn(Self::backup_solutions(this.clone(), output_path)).await {
            Ok(Ok(())) => {
                info!("Exercism solutions backup complete");
                Ok(())
            },
            Ok(Err(task_error)) => Err(task_error),
            Err(join_error) => resume_unwind(join_error.into_panic()),
        }
    }

    #[instrument(skip_all)]
    async fn backup_solutions(this: Arc<Self>, output_path: PathBuf) -> Result<()> {
        let mut task_pool = TaskPool::new();

        let mut page = 1;
        loop {
            let (solutions, meta) = this.get_solutions_for_page(page).await?;

            if solutions.is_empty() {
                info!("No solutions to backup in page {page}");
            } else {
                if this.args.dry_run && enabled!(Level::INFO) {
                    let solutions_list = solutions
                        .iter()
                        .map(|solution| {
                            format!("{}/{}", solution.track.name, solution.exercise.name)
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    info!("Solutions to backup in page {page}: {solutions_list}");
                } else {
                    info!("Number of solutions to backup in page {page}: {}", solutions.len());
                }

                // Create track directories right away so that concurrent tasks don't end up trying
                // to create a directory multiple times.
                this.create_track_directories(&output_path, &solutions)
                    .await?;

                if !this.args.dry_run || enabled!(Level::DEBUG) {
                    for solution in solutions {
                        task_pool.spawn(Self::backup_solution(
                            this.clone(),
                            output_path.clone(),
                            solution,
                        ));
                    }
                }
            }

            if meta.current_page == meta.total_pages {
                break;
            }
            page += 1;
        }

        task_pool
            .join(|| "errors detected while backing up solutions")
            .await
    }

    #[instrument(level = "debug", skip_all, fields(%solution.track.name, %solution.exercise.name))]
    async fn backup_solution(
        this: Arc<Self>,
        mut output_path: PathBuf,
        solution: Solution,
    ) -> Result<()> {
        if !this.args.dry_run {
            debug!("Starting solution backup");
        }
        trace!(?solution);

        output_path.push(&solution.track.name);
        output_path.push(&solution.exercise.name);
        trace!(output_path = %output_path.display());

        let files = {
            let _permit = this.limiter.get_permit().await;

            // Note: this is not a download, but can cause "Too many open files" errors
            // if we have too many concurrent calls, so we'll limit it also.
            if !this.can_backup_solution(&solution, &output_path).await? {
                return Ok(());
            }

            this.v1_client
                .get_solution(&solution.uuid)
                .await?
                .solution
                .files
        };
        if this.args.dry_run {
            debug!("Files to backup: {}", files.join(", "));
        }

        if !this.args.dry_run || enabled!(Level::TRACE) {
            let mut task_pool = TaskPool::new();

            for file in files {
                task_pool.spawn(Self::backup_one_file(
                    this.clone(),
                    solution.clone(),
                    file,
                    output_path.clone(),
                ));
            }

            task_pool
                .join(|| {
                    format!(
                        "errors detected while backing up solution for {}/{}",
                        solution.track.name, solution.exercise.name
                    )
                })
                .await?;

            // See above for why we limit this.
            let _permit = this.limiter.get_permit().await;
            this.save_backup_state(&solution, &output_path).await?;
        }

        info!("Solution to {}/{} downloaded", solution.track.name, solution.exercise.name);

        Ok(())
    }

    #[instrument(level = "trace", skip_all, fields(%solution.track.name, %solution.exercise.name, file))]
    async fn backup_one_file(
        this: Arc<Self>,
        solution: Solution,
        file: String,
        mut destination_path: PathBuf,
    ) -> Result<()> {
        let _permit = this.limiter.get_permit().await;
        let mut file_stream = this.v1_client.get_file(&solution.uuid, &file).await;

        destination_path.extend(file.split('/'));
        trace!(destination_path = %destination_path.display());

        if !this.args.dry_run {
            this.create_file_parent_directory(&destination_path).await?;

            let destination_file =
                fs::File::create(&destination_path).await.with_context(|| {
                    format!("failed to create local file {}", destination_path.display())
                })?;
            let mut destination_file = BufWriter::new(destination_file);

            while let Some(bytes) = file_stream.next().await {
                let bytes = bytes.with_context(|| {
                    format!(
                        "failed to download file {} in solution to exercise {}/{}",
                        file, solution.track.name, solution.exercise.name,
                    )
                })?;
                destination_file.write_all(&bytes).await.with_context(|| {
                    format!("failed to write data to file {}", destination_path.display())
                })?;
            }

            destination_file.flush().await.with_context(|| {
                format!("failed to flush data to file {}", destination_path.display())
            })?;
        }

        Ok(())
    }

    #[instrument(level = "trace", skip(self, solution), fields(%solution.track.name, %solution.exercise.name))]
    async fn save_backup_state(
        &self,
        solution: &Solution,
        solution_output_path: &Path,
    ) -> Result<()> {
        let mut state = BackupState::for_solution(solution);
        state.iterations = vec![solution.num_iterations];
        let state = serde_json::to_string_pretty(&state).with_context(|| {
            format!(
                "failed to persist backup state for solution to {}/{} to JSON",
                solution.track.name, solution.exercise.name
            )
        })?;

        let mut temp_state_file_path: PathBuf = solution_output_path.into();
        temp_state_file_path.push(BACKUP_STATE_TEMP_FILE_NAME);
        self.create_file_parent_directory(&temp_state_file_path)
            .await?;
        fs::write(&temp_state_file_path, state)
            .await
            .with_context(|| {
                format!(
                    "failed to save backup state for solution to {}/{} to {}",
                    solution.track.name,
                    solution.exercise.name,
                    temp_state_file_path.display()
                )
            })?;

        let mut state_file_path: PathBuf = solution_output_path.into();
        state_file_path.push(BACKUP_STATE_FILE_NAME);
        fs::rename(&temp_state_file_path, &state_file_path)
            .await
            .with_context(|| {
                format!(
                    "failed to rename backup state for solution to {}/{}, from {} to {}",
                    solution.track.name,
                    solution.exercise.name,
                    temp_state_file_path.display(),
                    state_file_path.display()
                )
            })
    }

    #[instrument(level = "trace", skip(self))]
    async fn create_output_directory(&self, output_path: &Path) -> Result<()> {
        match self.args.dry_run {
            true => Ok(()),
            false => fs::create_dir_all(output_path).await.with_context(|| {
                format!("failed to create output directory {}", output_path.display())
            }),
        }
    }

    #[instrument(level = "debug", skip(self))]
    async fn get_solutions_for_page(
        &self,
        page: i64,
    ) -> Result<(Vec<Solution>, solutions::ResponseMeta)> {
        let filters = self.get_solutions_filters();
        let paging = solutions::Paging::for_page(page);

        let _permit = self.limiter.get_permit().await;
        let response = self
            .v2_client
            .get_solutions(Some(filters), Some(paging), Some(solutions::SortOrder::NewestFirst))
            .await
            .with_context(|| format!("failed to fetch solutions for page {page}"))?;
        let solutions = response
            .results
            .into_iter()
            .filter(|solution| self.args.solution_matches(solution))
            .collect();
        Ok((solutions, response.meta))
    }

    #[instrument(level = "trace", skip_all, ret(level = "trace"))]
    fn get_solutions_filters(&self) -> solutions::Filters {
        let mut builder = solutions::Filters::builder();

        // These are more optimizations - it works even if we don't specify them since the
        // filtering performed later will catch all invalid solutions, but it's faster to iterate
        // on the solutions if we pre-filter them on Exercism's side.
        if self.args.track.len() == 1 {
            builder.track(self.args.track.first().map(|track| track.as_str()).unwrap());
        }
        if self.args.exercise.len() == 1 {
            builder.criteria(
                self.args
                    .exercise
                    .first()
                    .map(|exercise| exercise.as_str())
                    .unwrap(),
            );
        }

        builder.build()
    }

    #[instrument(level = "trace", skip(self, solutions))]
    async fn create_track_directories(
        &self,
        output_path: &Path,
        solutions: &[Solution],
    ) -> Result<()> {
        if !self.args.dry_run {
            let track_names = solutions
                .iter()
                .map(|solution| solution.track.name.as_str())
                .collect::<HashSet<_>>();

            for track_name in track_names {
                let mut destination_path = output_path.to_path_buf();
                destination_path.push(track_name);

                fs::create_dir_all(&destination_path)
                    .await
                    .with_context(|| {
                        format!("failed to create directory for track {track_name}")
                    })?;
            }
        }

        Ok(())
    }

    #[instrument(level = "trace", skip(self, solution), fields(%solution.track.name, %solution.exercise.name))]
    async fn can_backup_solution(
        &self,
        solution: &Solution,
        solution_output_path: &Path,
    ) -> Result<bool> {
        let solution_exists = self.directory_exists(solution_output_path).await;
        let solution_needs_update = BackupState::for_backup(solution, solution_output_path)
            .await
            .needs_backup(solution);

        match (solution_exists, solution_needs_update, self.args.overwrite) {
            (true, false, OverwritePolicy::Always) => {
                trace!("Solution to {}/{} already up-to-date on disk, but needs to be overwritten; cleaning up...",
                    solution.track.name, solution.exercise.name);
                self.remove_directory(solution_output_path).await?;
            },
            (true, false, _) => {
                trace!(
                    "Solution to {}/{} already exists on disk and is up-to-date; skipping",
                    solution.track.name,
                    solution.exercise.name
                );
                return Ok(false);
            },
            (true, true, OverwritePolicy::Never) => {
                trace!(
                    "Solution to {}/{} already exists on disk and cannot be overwritten; skipping",
                    solution.track.name,
                    solution.exercise.name
                );
                return Ok(false);
            },
            (true, true, _) => {
                trace!(
                    "Solution to {}/{} already exists on disk but needs updating; cleaning up...",
                    solution.track.name,
                    solution.exercise.name
                );
                self.remove_directory(solution_output_path).await?;
            },
            (false, _, _) => (),
        }

        if !self.args.dry_run {
            fs::create_dir_all(solution_output_path)
                .await
                .with_context(|| {
                    format!(
                        "failed to create destination directory for solution to {}/{}: {}",
                        solution.track.name,
                        solution.exercise.name,
                        solution_output_path.display(),
                    )
                })?;
        }

        Ok(true)
    }

    #[instrument(level = "trace", skip(self))]
    async fn create_file_parent_directory(&self, destination_path: &Path) -> Result<()> {
        match (self.args.dry_run, destination_path.parent()) {
            (false, Some(parent)) => fs::create_dir_all(parent).await.with_context(|| {
                format!("failed to make sure parent of file {} exists", destination_path.display())
            }),
            _ => Ok(()),
        }
    }

    #[instrument(level = "trace", skip(self), ret(level = "trace"))]
    async fn directory_exists(&self, dir_path: &Path) -> bool {
        fs::metadata(dir_path)
            .await
            .map(|meta| meta.is_dir())
            .unwrap_or(false)
    }

    #[instrument(level = "trace", skip(self))]
    async fn remove_directory(&self, dir_path: &Path) -> Result<()> {
        if !self.args.dry_run {
            fs::remove_dir_all(dir_path)
                .await
                .with_context(|| format!("failed to remove directory {}", dir_path.display()))
        } else {
            Ok(())
        }
    }
}
