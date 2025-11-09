//! Definition of the [`Backup`](crate::command::Command::Backup) command.

pub mod args;
#[macro_use]
mod detail;
mod iterations;
mod state;

use std::collections::HashSet;
use std::fmt::Debug;
use std::io;
use std::panic::resume_unwind;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, anyhow};
use itertools::Itertools;
use mini_exercism::api::v2::iteration::Iteration;
use mini_exercism::api::v2::solution::Solution;
use mini_exercism::api::v2::{solution, solutions};
use mini_exercism::cli::get_cli_credentials;
use mini_exercism::core::Credentials;
use mini_exercism::stream::StreamExt;
use mini_exercism::{api, http};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::{fs, spawn};
use tracing::{Level, debug, enabled, error, info, trace, warn};

use crate::Result;
use crate::command::backup::args::{BackupArgs, OverwritePolicy, SolutionStatus};
use crate::command::backup::iterations::{
    ITERATIONS_DIR_ENV_VAR_NAME, SyncOps, get_iterations_dir_name,
};
use crate::command::backup::state::{
    AUXILIAIRE_STATE_DIR_NAME, BACKUP_STATE_FILE_NAME, BACKUP_STATE_TEMP_FILE_NAME, BackupState,
};
use crate::limiter::Limiter;
use crate::task_pool::TaskPool;

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
    iterations_dir_name: String,
    iterations_dir_filter: String,
}

impl BackupCommand {
    /// Creates a new [`BackupCommand`] using the provided [`args`](BackupArgs).
    ///
    /// The `api_base_url` parameter should only be set to test using a different Exercism local endpoint.
    pub fn new(args: BackupArgs, api_base_url: Option<&str>) -> Result<Arc<Self>> {
        let http_client = http::Client::builder()
            .cookie_store(true)
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
        let iterations_dir_name = get_iterations_dir_name();
        let iterations_dir_filter = format!("{iterations_dir_name}/");

        Ok(Arc::new(Self {
            args,
            v1_client,
            v2_client,
            limiter,
            iterations_dir_name,
            iterations_dir_filter,
        }))
    }

    /// Execute the backup operation.
    ///
    /// See [struct description](Self) for details on how to call this method.
    #[cfg_attr(not(coverage_nightly), tracing::instrument(skip_all))]
    pub async fn execute(this: Arc<Self>) -> Result<()> {
        info!("Starting Exercism solutions backup to {}", this.args.path.display());
        trace!(?this.args);

        this.create_output_directory(&this.args.path).await?;

        let output_path = this.args.path.canonicalize().with_context(|| {
            format!("failed to get absolute path for output directory {}", this.args.path.display())
        })?;
        trace!(output_path = %output_path.display());

        match spawn(Self::backup_solutions(Arc::clone(&this), output_path)).await {
            Ok(Ok(())) => {
                info!("Exercism solutions backup complete");
                Ok(())
            },
            Ok(Err(task_error)) => Err(task_error),
            Err(join_error) => resume_unwind(join_error.into_panic()),
        }
    }

    #[cfg_attr(not(coverage_nightly), tracing::instrument(skip_all))]
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
                            Arc::clone(&this),
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

    #[cfg_attr(not(coverage_nightly), tracing::instrument(level = "debug", skip_all, fields(solution.track.name, solution.exercise.name)))]
    async fn backup_solution(
        this: Arc<Self>,
        mut output_path: PathBuf,
        solution: Solution,
    ) -> Result<()> {
        trace!(?solution);

        output_path.push(&solution.track.name);
        output_path.push(&solution.exercise.name);
        trace!(output_path = %output_path.display());

        let files = this.get_solution_files(&solution).await.with_context(|| {
            format!(
                "failed to get list of files for solution to {}/{}",
                solution.track.name, solution.exercise.name,
            )
        })?;

        let (needs_backup, solution_exists) =
            this.solution_needs_backup(&solution, &output_path).await?;
        if this.args.dry_run && needs_backup {
            debug!("Files to back up: {}", files.join(", "));
        }

        if this.args.iterations_sync_policy.sync() && this.has_iterations_dir_collision(&files) {
            let warning = format!(
                "solution to {}/{} contains a file whose name collides with the iterations backup directory name ({}); consider setting the {} environment variable to change the directory name",
                solution.track.name,
                solution.exercise.name,
                this.iterations_dir_name,
                ITERATIONS_DIR_ENV_VAR_NAME,
            );

            warn!("{}", warning);
            if !this.args.dry_run {
                return Err(anyhow!("{}", warning));
            }
        }

        let matching_iterations = this.get_matching_solution_iterations(&solution).await?;
        let existing_iterations = this
            .get_existing_iterations(&solution, &output_path)
            .await?;
        let iteration_ops = this.get_iteration_sync_ops(matching_iterations, existing_iterations);

        if this.args.iterations_sync_policy.clean_up_old()
            && !iteration_ops.existing_iterations_to_clean_up.is_empty()
        {
            debug!(
                "Existing iterations to clean up: {}",
                iteration_ops.existing_iterations_to_clean_up.len()
            );
        }
        if this.args.iterations_sync_policy.backup_new()
            && !iteration_ops.iterations_to_backup.is_empty()
        {
            debug!("Iterations to back up: {}", iteration_ops.iterations_to_backup.len());
        }

        if !needs_backup && iteration_ops.is_empty() {
            // No need to log something here, user has already been notified that we're
            // skipping this solution in `solution_needs_backup`.
            return Ok(());
        }

        if !this.args.dry_run {
            this.create_solution_directories(
                needs_backup,
                solution_exists,
                &solution,
                &output_path,
            )
            .await?;
        }

        if !this.args.dry_run || enabled!(Level::DEBUG) {
            let mut task_pool = TaskPool::new();

            if needs_backup {
                for file in files {
                    task_pool.spawn(Self::backup_one_file(
                        Arc::clone(&this),
                        solution.clone(),
                        file,
                        output_path.clone(),
                    ));
                }
            }

            let mut iterations_output_path = output_path.clone();
            iterations_output_path.push(&this.iterations_dir_name);

            if !iteration_ops.is_empty() {
                for existing_iteration in iteration_ops.existing_iterations_to_clean_up {
                    task_pool.spawn(Self::remove_one_existing_iteration(
                        Arc::clone(&this),
                        solution.clone(),
                        existing_iteration,
                        iterations_output_path.clone(),
                    ));
                }
                for new_iteration in iteration_ops.iterations_to_backup {
                    task_pool.spawn(Self::backup_one_iteration(
                        Arc::clone(&this),
                        solution.clone(),
                        new_iteration,
                        iterations_output_path.clone(),
                    ));
                }
            }

            task_pool
                .join(|| {
                    format!(
                        "errors detected while backing up solution for {}/{}",
                        solution.track.name, solution.exercise.name
                    )
                })
                .await?;

            // If we removed all iterations from the iterations directory, we should
            // delete it. The easiest way is to try to delete it and if it's not empty,
            // simply skip and move on.
            match fs::remove_dir(&iterations_output_path).await {
                Ok(()) => (),
                Err(err) if err.kind() == io::ErrorKind::DirectoryNotEmpty => (),
                err => {
                    return err.with_context(|| {
                        format!(
                            "error removing empty iterations directory for {}/{}",
                            solution.track.name, solution.exercise.name
                        )
                    });
                },
            }
        }

        if !this.args.dry_run {
            let _permit = this.limiter.get_permit().await;
            this.save_backup_state(&solution, &output_path).await?;
        }

        info!("Solution to {}/{} downloaded", solution.track.name, solution.exercise.name);

        Ok(())
    }

    #[cfg_attr(not(coverage_nightly), tracing::instrument(level = "trace", skip_all, fields(solution.track.name, solution.exercise.name, file)))]
    async fn backup_one_file(
        this: Arc<Self>,
        solution: Solution,
        file: String,
        mut destination_path: PathBuf,
    ) -> Result<()> {
        destination_path.extend(file.split('/'));
        trace!(destination_path = %destination_path.display());

        let _permit = this.limiter.get_permit().await;
        let mut file_stream = this.v1_client.get_file(&solution.uuid, &file).await;

        if !this.args.dry_run {
            this.create_file_parent_directory(&destination_path).await?;

            let destination_file = fs::File::create(&destination_path).await?;
            let mut destination_file = BufWriter::new(destination_file);

            while let Some(bytes) = file_stream.next().await {
                let bytes = bytes.with_context(|| {
                    format!(
                        "failed to download file {file} in solution to exercise {}/{}",
                        solution.track.name, solution.exercise.name,
                    )
                })?;
                destination_file.write_all(&bytes).await?;
            }

            destination_file.flush().await?;
        }

        Ok(())
    }

    #[cfg_attr(not(coverage_nightly), tracing::instrument(level = "trace", skip_all, fields(solution.track.name, solution.exercise.name, iteration.index = iteration)))]
    async fn remove_one_existing_iteration(
        this: Arc<Self>,
        solution: Solution,
        iteration: i32,
        mut destination_path: PathBuf,
    ) -> Result<()> {
        destination_path.push(iteration.to_string());
        trace!(destination_path = %destination_path.display());

        if !this.args.dry_run {
            let context = || {
                format!(
                    "failed to remove existing iteration {} of solution to {}/{}",
                    iteration, solution.track.name, solution.exercise.name,
                )
            };

            let _permit = this.limiter.get_permit().await;
            this.remove_directory_content(&destination_path)
                .await
                .with_context(context)?;
            fs::remove_dir(&destination_path)
                .await
                .with_context(context)?;
        }

        debug!(
            "Iteration {} of solution to {}/{} removed from disk",
            iteration, solution.track.name, solution.exercise.name,
        );

        Ok(())
    }

    #[cfg_attr(not(coverage_nightly), tracing::instrument(level = "trace", skip_all, fields(solution.track.name, solution.exercise.name, iteration.index)))]
    async fn backup_one_iteration(
        this: Arc<Self>,
        solution: Solution,
        iteration: Iteration,
        mut destination_path: PathBuf,
    ) -> Result<()> {
        destination_path.push(iteration.index.to_string());
        trace!(destination_path = %destination_path.display());

        match iteration.submission_uuid {
            Some(submission_uuid) => {
                let _permit = this.limiter.get_permit().await;
                let files = this
                    .v2_client
                    .get_submission_files(&solution.uuid, &submission_uuid)
                    .await
                    .with_context(|| {
                        format!(
                            "failed to fetch files for iteration {} of solution to {}/{}",
                            iteration.index, solution.track.name, solution.exercise.name,
                        )
                    })?
                    .files;

                for file in files {
                    let mut file_path = destination_path.clone();
                    file_path.push(&file.filename);

                    this.create_file_parent_directory(&file_path).await?;
                    if !this.args.dry_run {
                        fs::write(&file_path, file.content).await.with_context(|| {
                            format!(
                                "failed to save file {} of iteration {} of solution to {}/{}",
                                file.filename,
                                iteration.index,
                                solution.track.name,
                                solution.exercise.name,
                            )
                        })?;
                    }
                }

                debug!(
                    "Iteration {} of solution to {}/{} downloaded",
                    iteration.index, solution.track.name, solution.exercise.name,
                );
            },
            None => {
                let error = format!(
                    "Iteration {} of solution to {}/{} is not marked as deleted but does not have a submission UUID",
                    iteration.index, solution.track.name, solution.exercise.name,
                );

                error!("{}", error);
                if !this.args.dry_run {
                    return Err(anyhow!(error));
                }
            },
        }

        Ok(())
    }

    #[cfg_attr(not(coverage_nightly), tracing::instrument(level = "trace", skip(self, solution), fields(solution.track.name, solution.exercise.name)))]
    async fn save_backup_state(
        &self,
        solution: &Solution,
        solution_output_path: &Path,
    ) -> Result<()> {
        let state = BackupState::for_solution(solution.clone());
        let state = serde_json::to_string_pretty(&state).with_context(|| {
            format!(
                "failed to persist backup state for solution to {}/{} to JSON",
                solution.track.name, solution.exercise.name
            )
        })?;

        let mut temp_state_file_path = solution_output_path.to_path_buf();
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

        let mut state_file_path = solution_output_path.to_path_buf();
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

    #[cfg_attr(not(coverage_nightly), tracing::instrument(level = "trace", skip(self)))]
    async fn create_output_directory(&self, output_path: &Path) -> Result<()> {
        if !self.args.dry_run {
            fs::create_dir_all(output_path).await?;
        }

        Ok(())
    }

    #[cfg_attr(not(coverage_nightly), tracing::instrument(level = "debug", skip(self)))]
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

    #[cfg_attr(
        not(coverage_nightly),
        tracing::instrument(level = "trace", skip_all, ret(level = "trace"))
    )]
    fn get_solutions_filters(&self) -> solutions::Filters<'_> {
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
        if self.args.status == SolutionStatus::Published {
            // Published is the only status we can actually pass as a filter,
            // because otherwise we only get solutions with that specific status
            // (and not any status that is higher).
            builder.status(solution::Status::Published);
        }

        builder.build()
    }

    #[cfg_attr(not(coverage_nightly), tracing::instrument(level = "trace", skip(self, solutions)))]
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
                fs::create_dir_all(&destination_path).await?;
            }
        }

        Ok(())
    }

    #[cfg_attr(not(coverage_nightly), tracing::instrument(level = "trace", skip_all, fields(solution.track.name, solution.exercise.name)))]
    async fn get_solution_files(&self, solution: &Solution) -> Result<Vec<String>> {
        let _permit = self.limiter.get_permit().await;
        Ok(self
            .v1_client
            .get_solution(&solution.uuid)
            .await
            .with_context(|| {
                format!(
                    "failed to get list of files for solution to {}/{}",
                    solution.track.name, solution.exercise.name,
                )
            })?
            .solution
            .files)
    }

    #[cfg_attr(not(coverage_nightly), tracing::instrument(
        level = "trace",
        skip(self, solution),
        fields(solution.track.name, solution.exercise.name),
        ret(level = "trace")
    ))]
    async fn solution_needs_backup(
        &self,
        solution: &Solution,
        solution_output_path: &Path,
    ) -> Result<(bool, bool)> {
        let _permit = self.limiter.get_permit().await;
        let state = BackupState::for_backup(solution, solution_output_path).await;

        let solution_exists = self.directory_exists(solution_output_path).await;
        let solution_needs_update = state.needs_update(solution)?;

        let needs_backup = match (solution_exists, solution_needs_update, self.args.overwrite) {
            (true, false, OverwritePolicy::Always) => {
                trace!(
                    "Solution to {}/{} already up-to-date on disk, but needs to be overwritten; will be cleaned up",
                    solution.track.name, solution.exercise.name
                );
                true
            },
            (true, false, OverwritePolicy::IfNewer) | (true, false, OverwritePolicy::Never) => {
                trace!(
                    "Solution to {}/{} already exists on disk and is up-to-date; skipping",
                    solution.track.name, solution.exercise.name
                );
                false
            },
            (true, true, OverwritePolicy::Never) => {
                trace!(
                    "Solution to {}/{} already exists on disk and cannot be overwritten; skipping",
                    solution.track.name, solution.exercise.name
                );
                false
            },
            (true, true, OverwritePolicy::IfNewer) | (true, true, OverwritePolicy::Always) => {
                trace!(
                    "Solution to {}/{} already exists on disk but needs updating; will be cleaned up",
                    solution.track.name, solution.exercise.name
                );
                true
            },
            (false, _, _) => true,
        };

        Ok((needs_backup, solution_exists))
    }

    #[cfg_attr(not(coverage_nightly), tracing::instrument(level = "trace", skip(self, solution), fields(solution.track.name, solution.exercise.name)))]
    async fn create_solution_directories(
        &self,
        needs_backup: bool,
        solution_exists: bool,
        solution: &Solution,
        solution_output_path: &Path,
    ) -> Result<()> {
        if needs_backup {
            if solution_exists {
                self.remove_directory_content(solution_output_path)
                    .await
                    .with_context(|| {
                        format!(
                            "failed to clean up existing directory for solution to {}/{}",
                            solution.track.name, solution.exercise.name,
                        )
                    })?;
            }

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

        if self.args.iterations_sync_policy.sync() {
            let mut iterations_output_path = solution_output_path.to_path_buf();
            iterations_output_path.push(&self.iterations_dir_name);

            fs::create_dir_all(&iterations_output_path)
                .await
                .with_context(|| {
                    format!(
                        "failed to create destination directory for iterations of solution to {}/{}: {}",
                        solution.track.name,
                        solution.exercise.name,
                        iterations_output_path.display(),
                    )
                })?;
        }

        Ok(())
    }

    #[cfg_attr(not(coverage_nightly), tracing::instrument(level = "trace", skip_all, fields(solution.track.name, solution.exercise.name)))]
    async fn get_matching_solution_iterations(
        &self,
        solution: &Solution,
    ) -> Result<Vec<Iteration>> {
        if !self.args.iterations_sync_policy.backup_new() && !self.args.dry_run {
            return Ok(vec![]);
        }

        let iterations = {
            let _permit = self.limiter.get_permit().await;
            self.v2_client
                .get_solution(&solution.uuid, true)
                .await
                .with_context(|| {
                    format!(
                        "failed to get list of iterations for solution to {}/{}",
                        solution.track.name, solution.exercise.name,
                    )
                })?
                .iterations
        };

        Ok(iterations
            .into_iter()
            .filter(|iter| self.args.iteration_matches(iter))
            .sorted_unstable_by_key(|iter| iter.index)
            .collect_vec())
    }

    #[cfg_attr(not(coverage_nightly), tracing::instrument(level = "trace", skip(self, solution), fields(solution.track.name, solution.exercise.name)))]
    async fn get_existing_iterations(
        &self,
        solution: &Solution,
        solution_output_path: &Path,
    ) -> Result<Vec<i32>> {
        if !self.args.iterations_sync_policy.sync() && !self.args.dry_run {
            return Ok(vec![]);
        }

        let mut iterations_path = solution_output_path.to_path_buf();
        iterations_path.push(&self.iterations_dir_name);
        if !self.directory_exists(&iterations_path).await {
            return Ok(vec![]);
        }

        let _permit = self.limiter.get_permit().await;
        let mut iterations_dir_content =
            fs::read_dir(&iterations_path).await.with_context(|| {
                format!(
                    "failed to list existing backed up iterations for solution to {}/{}",
                    solution.track.name, solution.exercise.name,
                )
            })?;

        let mut iterations = Vec::new();
        loop {
            match iterations_dir_content.next_entry().await {
                Ok(Some(entry)) => {
                    let iteration = entry
                        .file_type()
                        .await
                        .ok()
                        .and_then(|file_type| {
                            file_type.is_dir().then(|| entry.file_name().into_string().ok())
                        })
                        .flatten()
                        .and_then(|file_name| {
                            file_name.parse::<i32>().ok()
                        });
                    if let Some(iteration) = iteration {
                        iterations.push(iteration);
                    }
                },
                Ok(None) => break,
                Err(err) => return Err(err).with_context(|| {
                    format!(
                        "failed to scan existing iterations back up directory for solution to {}/{}",
                        solution.track.name,
                        solution.exercise.name,
                    )
                }),
            }
        }

        iterations.sort_unstable();
        Ok(iterations)
    }

    #[cfg_attr(
        not(coverage_nightly),
        tracing::instrument(level = "trace", skip_all, ret(level = "trace"))
    )]
    fn get_iteration_sync_ops<M, E>(
        &self,
        matching_iterations: M,
        existing_iterations: E,
    ) -> SyncOps
    where
        M: IntoIterator<Item = Iteration>,
        E: IntoIterator<Item = i32>,
    {
        let mut existing_it = existing_iterations.into_iter().peekable();

        let mut ops = SyncOps::default();
        for matching in matching_iterations {
            while let Some(existing) = existing_it.next_if(|&ne| ne < matching.index) {
                ops.existing_iterations_to_clean_up.push(existing);
            }
            if existing_it.next_if_eq(&matching.index).is_none() {
                ops.iterations_to_backup.push(matching.clone());
            }
        }
        ops.existing_iterations_to_clean_up.extend(existing_it);

        // Existing iterations are fetched even if we don't want to clean them up, because
        // we need them to compute which iterations are new. However, if we don't want to
        // clean them up, remove them here.
        if !self.args.iterations_sync_policy.clean_up_old() {
            ops.existing_iterations_to_clean_up.clear();
        }

        ops
    }

    #[cfg_attr(not(coverage_nightly), tracing::instrument(level = "trace", skip(self)))]
    async fn create_file_parent_directory(&self, destination_path: &Path) -> Result<()> {
        match (self.args.dry_run, destination_path.parent()) {
            (false, Some(parent)) => fs::create_dir_all(parent).await.with_context(|| {
                format!("failed to make sure parent of file {} exists", destination_path.display())
            }),
            _ => Ok(()),
        }
    }

    #[cfg_attr(
        not(coverage_nightly),
        tracing::instrument(level = "trace", skip(self), ret(level = "trace"))
    )]
    async fn directory_exists(&self, dir_path: &Path) -> bool {
        fs::metadata(dir_path)
            .await
            .map(|meta| meta.is_dir())
            .unwrap_or(false)
    }

    #[cfg_attr(not(coverage_nightly), tracing::instrument(level = "trace", skip(self)))]
    async fn remove_directory_content(&self, dir_path: &Path) -> Result<()> {
        if !self.args.dry_run {
            let mut dir_content = fs::read_dir(dir_path).await?;

            loop {
                match dir_content.next_entry().await {
                    Ok(Some(entry)) if !self.should_skip_dir_entry(&entry.path()) => {
                        if entry.file_type().await?.is_dir() {
                            // We won't use this function recursively to delete directories,
                            // because we currently filter entries in the root directory only.
                            fs::remove_dir_all(&entry.path()).await?;
                        } else {
                            fs::remove_file(&entry.path()).await?;
                        }
                    },
                    Ok(Some(entry)) => {
                        trace!(
                            "Skipping {} while removing directory {}",
                            entry.path().display(),
                            dir_path.display(),
                        );
                    },
                    Ok(None) => break,
                    Err(err) => return Err(anyhow!(err)),
                }
            }
        }

        Ok(())
    }

    #[cfg_attr(
        not(coverage_nightly),
        tracing::instrument(level = "trace", skip(self), ret(level = "trace"))
    )]
    fn should_skip_dir_entry(&self, entry_path: &Path) -> bool {
        entry_path
            .file_name()
            .map(|name| {
                name == self.iterations_dir_name.as_str() || name == AUXILIAIRE_STATE_DIR_NAME
            })
            .unwrap_or(true)
    }

    #[cfg_attr(
        not(coverage_nightly),
        tracing::instrument(level = "trace", skip(self), ret(level = "trace"))
    )]
    fn has_iterations_dir_collision(&self, files: &[String]) -> bool {
        files.iter().any(|file| {
            file.starts_with(&self.iterations_dir_name)
                || file.starts_with(&self.iterations_dir_filter)
        })
    }
}
