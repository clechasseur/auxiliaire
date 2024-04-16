//! Definition of supported CLI commands.

pub mod backup;

use clap::Subcommand;

use crate::command::backup::args::BackupArgs;
use crate::command::backup::BackupCommand;
use crate::Result;

/// Possible commands supported by our CLI application.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Download Exercism.org solutions for backup
    ///
    /// By default, this command will attempt to download backups of all solutions to exercises
    /// submitted to the Exercism.org website, for all language tracks, and will store them in
    /// the specified directory. See options for ways to filter solutions/exercises to download, etc.
    ///
    /// If an exercise has had multiple iterations submitted, the latest iteration is always downloaded.
    ///
    /// To download solutions, an Exercism API token is needed. If not specified via the --token option,
    /// by default, the API token configured for the local installation of the Exercism CLI application
    /// will be used. The command does not require the Exercism CLI to work, but if it's not installed,
    /// then the API token will have to be specified (see --token).
    Backup(BackupArgs),
}

impl Command {
    /// Execute this [`Command`].
    ///
    /// This method is provided explicitly in order to make it `async`.
    pub async fn execute(self) -> Result<()> {
        match self {
            Command::Backup(args) => {
                let backup_command = BackupCommand::new(args, None)?;
                BackupCommand::execute(backup_command).await
            },
        }
    }
}
