// brooks-cli, Copyright 2026, Will Hawkins
//
// This file is part of brooks-cli.

// This file is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use std::fmt::Display;
use std::io::Read;

use ansi_term::{Color::Red, Style};
#[allow(
    redundant_imports,
    unused_imports,
    clippy::single_component_path_imports
)]
use brooks_lib;

#[cfg(test)]
mod test;

use brooks_lib::logging::{LogLevel::Trace, LogMsgFormatter, LogMsgs};

use brooks_lib::mel::compiler::compile::{MelCompilerError, MelCompilerLocatableError};
use brooks_lib::mel::interpreter::builtins::builtin_builtin_function_interpreters;
use brooks_lib::mel::scope::{Scope, builtin_function_types, minimal_core_variable_types};
use brooks_lib::mel::{
    analysis::{self, MelAnalysisError, MelAnalysisLocatableError},
    ast::AstVisitorDriver,
    compiler::compile,
    interpreter::{
        self,
        interpret::{MelInterpContext, MelInterpLocatableError, TypedValue},
    },
    scope::Scopes,
    serializer::{AstTextSerializer, AstTextSerializerContext},
    tvs::Type,
};
use brooks_lib::ps::spec::{TypedGenericStage, TypedStage};
use brooks_lib::ps::verify::{PsVerificationError, PsVerificationKey, verify_ps_request_stage};
use clap::{ArgAction, CommandFactory, Parser, Subcommand};
use clio::ClioPath;
use flexi_logger::{FileSpec, LogSpecification, Logger};
use log::{LevelFilter, info};

use crate::CliError::{ParseError, VerificationError};

mod hmds;
mod proxy;
mod serve;

#[derive(Debug, Clone)]
enum DebugLevel {
    Error,
    Warn,
    Info,
    Debug,
}

impl From<u8> for DebugLevel {
    fn from(value: u8) -> Self {
        if value >= 3 {
            Self::Debug
        } else if value >= 2 {
            Self::Info
        } else if value >= 1 {
            Self::Warn
        } else {
            Self::Error
        }
    }
}

impl From<DebugLevel> for LevelFilter {
    fn from(value: DebugLevel) -> Self {
        match value {
            DebugLevel::Error => LevelFilter::Error,
            DebugLevel::Warn => LevelFilter::Warn,
            DebugLevel::Info => LevelFilter::Info,
            DebugLevel::Debug => LevelFilter::Debug,
        }
    }
}

#[derive(Parser)]
struct Cli {
    #[arg(long, action=ArgAction::Count)]
    debug: u8,

    #[arg(long)]
    log_file: Option<ClioPath>,

    #[command(subcommand)]
    command: Commands,
}

pub fn parse_timeout_duration(given_duration: &str) -> Result<chrono::Duration, clap::Error> {
    if given_duration.ends_with("s") {
        let time: i64 = given_duration[0..given_duration.len() - 1]
            .parse()
            .map_err(|_| clap::error::Error::new(clap::error::ErrorKind::ValueValidation))?;
        Ok(chrono::Duration::seconds(time))
    } else if given_duration.ends_with("ns") {
        let time: i64 = given_duration[0..given_duration.len() - 2]
            .parse()
            .map_err(|_| clap::error::Error::new(clap::error::ErrorKind::ValueValidation))?;
        Ok(chrono::Duration::nanoseconds(time))
    } else {
        Err(clap::error::Error::new(
            clap::error::ErrorKind::ValueValidation,
        ))
    }
}

#[derive(Subcommand, Debug)]
enum Commands {
    Compile {
        #[arg(long)]
        path: clio::ClioPath,
    },
    Analyze {
        #[arg(long)]
        path: clio::ClioPath,
    },
    Interpret {
        #[arg(long)]
        path: clio::ClioPath,
    },
    Serve {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value = "8080")]
        port: u16,
    },
    Proxy {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value = "8080")]
        port: u16,
        #[arg(long)]
        path: clio::ClioPath,
    },
    HmdsServer {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value = "8080")]
        port: u16,

        #[cfg(feature = "domain")]
        #[arg(long, default_value = "/tmp/brooks/server")]
        path: clio::ClioPath,

        #[arg(long, default_value = "300", value_parser=clap::builder::ValueParser::new(parse_timeout_duration))]
        timeout: chrono::Duration,

        #[cfg(feature = "domain")]
        #[arg(long)]
        user: Option<String>,
        #[cfg(feature = "domain")]
        #[arg(long)]
        group: Option<String>,
    },
}

#[allow(clippy::result_large_err)]
fn compile_and_analyze(path: clio::ClioPath) -> CliResult<()> {
    let mut f = path
        .clone()
        .open()
        .map_err(|_| CliError::BadPath(path.clone()))?;

    let mut to_parse: Vec<u8> = vec![];
    f.read_to_end(&mut to_parse)
        .map_err(|_| CliError::BadPath(path.clone()))?;

    let source = &String::from_utf8_lossy(&to_parse);

    let type_scopes = Scopes::<Type> {
        scopes: vec![&minimal_core_variable_types() + &builtin_function_types()],
    };

    let result = match compile(source) {
        Ok(expr) => expr,
        Err(e) => {
            println!("{}", format_compiler_error(e, source, &path.to_string()));
            return Ok(());
        }
    };

    let result = analysis::analyze(&result, &type_scopes);

    match result {
        Ok(r) => println!("Expression Type: {}", r.tipe()),
        Err(e) => println!("{}", format_error(*e, source, &path.to_string())),
    };
    Ok(())
}

#[allow(clippy::result_large_err)]
fn compile_and_interpret(path: clio::ClioPath) -> CliResult<()> {
    let mut f = path
        .clone()
        .open()
        .map_err(|_| CliError::BadPath(path.clone()))?;

    let mut to_parse: Vec<u8> = vec![];
    f.read_to_end(&mut to_parse)
        .map_err(|_| CliError::BadPath(path.clone()))?;

    let types_scopes = Scopes::<Type> {
        scopes: vec![&minimal_core_variable_types() + &builtin_function_types()],
    };

    let values_scopes = Scopes::<TypedValue> {
        scopes: vec![
            &Into::<Scope<TypedValue>>::into(http::Request::new("body"))
                + &builtin_builtin_function_interpreters(),
        ],
    };
    let source = &String::from_utf8_lossy(&to_parse);

    let result = match compile(source) {
        Ok(expr) => expr,
        Err(e) => {
            println!("{}", format_compiler_error(e, source, &path.to_string()));
            return Ok(());
        }
    };

    let analyzed = analysis::analyze(&result, &types_scopes).map_err(CliError::AnalysisError)?;

    let mut interp_context = MelInterpContext::default();

    interp_context = interp_context
        .update_log(LogMsgs::new(Trace))
        .update_scopes(&values_scopes);
    match interpreter::interpret(&analyzed, interp_context) {
        Ok(o) => {
            match o.val {
                Some(o) => println!("{}", o),
                None => println!("Value missing"),
            }
            println!("Log:");
            println!(
                "{}",
                o.log.msgs(&LogMsgFormatter {
                    newline: true,
                    show_level: false
                })
            );
        }
        Err(e) => {
            print!("Error: {e}");
        }
    };
    Ok(())
}

#[allow(clippy::result_large_err)]
fn parse_and_analyze_processing_stages(
    path: clio::ClioPath,
) -> CliResult<TypedStage<PsVerificationKey>> {
    let mut f = path
        .clone()
        .open()
        .map_err(|_| CliError::BadPath(path.clone()))?;

    let mut to_parse: Vec<u8> = vec![];
    f.read_to_end(&mut to_parse)
        .map_err(|_| CliError::BadPath(path.clone()))?;

    let source = &String::from_utf8_lossy(&to_parse);

    let result =
        serde_json::from_str::<TypedGenericStage>(source).map_err(|e| ParseError(e.to_string()))?;

    let types_scope = Scopes::<Type> {
        scopes: vec![minimal_core_variable_types()],
    };
    let result = verify_ps_request_stage(&result, types_scope).map_err(VerificationError)?;

    Ok(result)
}

#[allow(clippy::result_large_err)]
fn compile_and_serialize(path: clio::ClioPath) -> CliResult<()> {
    let mut f = path
        .clone()
        .open()
        .map_err(|_| CliError::BadPath(path.clone()))?;

    let mut to_parse: Vec<u8> = vec![];
    f.read_to_end(&mut to_parse)
        .map_err(|_| CliError::BadPath(path.clone()))?;

    let compile_result = compile(&String::from_utf8_lossy(&to_parse));
    let ast = compile_result.expect("Compilation error");

    let driver = AstVisitorDriver {};
    let visitor = AstTextSerializer {};
    let context = AstTextSerializerContext {
        serialized: "".to_string(),
        indent: 0,
    };
    let result = driver
        .visit(&ast, &visitor, context)
        .expect("Could not serialize");
    println!("{}", result.serialized);
    Ok(())
}

#[derive(Debug)]
pub enum CliError {
    BadPath(clio::ClioPath),
    AnalysisError(Box<MelAnalysisLocatableError>),
    InterpreterError(Box<MelInterpLocatableError>),
    VerificationError(Box<PsVerificationError>),
    ParseError(String),
    ServerError(std::io::Error),
    SocketError(std::io::Error),
}
pub type CliResult<T> = Result<T, CliError>;

impl Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::BadPath(path) => write!(f, "Bad path: {path}"),
            CliError::AnalysisError(mel_analysis_locatable_error) => {
                write!(f, "Analysis error: {mel_analysis_locatable_error}")
            }
            CliError::InterpreterError(mel_interp_locatable_error) => {
                write!(f, "Interpreter error: {mel_interp_locatable_error}")
            }
            VerificationError(ps_verification_error) => write!(
                f,
                "Processing Stages verification error: {ps_verification_error}"
            ),
            ParseError(pe) => write!(f, "Parsing error: {pe}"),
            CliError::ServerError(error) => write!(f, "Server error: {error}"),
            CliError::SocketError(error) => write!(f, "UNIX Socket error: {error}"),
        }
    }
}

fn format_compiler_error(error: MelCompilerLocatableError, source: &str, path: &str) -> String {
    match error {
        MelCompilerLocatableError {
            error: MelCompilerError::SyntaxError(msg),
            location: l,
        } => {
            let mut result =
                Style::new().underline().paint("Error:").to_string() + &format!(" {}:\n", msg);

            let context_len = 3usize;
            let source_len = source.len();
            let semantic_source_len = if source.ends_with("\n") {
                source_len - 1
            } else {
                source_len
            };

            let error_start = l.start;
            let error_end = error_start + l.extent;

            let pre_error_start =
                std::cmp::max(0, error_start as i64 - context_len as i64) as usize;
            let pre_error_end = l.start;

            let post_error_start = std::cmp::min(source_len, error_end);
            let post_error_end = std::cmp::min(source_len, error_end + context_len);

            let pre_context = &source[pre_error_start..pre_error_end];
            let erroneous = &source[error_start..error_end];
            let post_context = &source[post_error_start..post_error_end].trim_end_matches("\n");

            // Print the error in context.
            result += "\t";
            if pre_error_start != 0 {
                result += "...";
            }
            result += pre_context;

            result += &Red.underline().paint(erroneous).to_string();

            result += post_context;
            if post_error_end <= semantic_source_len {
                result += "...";
            }
            result += "\n";
            result += &format!("\tat {}:{},{}", path, error_start, error_end);

            result
        }
        _ => todo!(),
    }
}

fn format_analysis_error(error: MelAnalysisLocatableError, source: &str, path: &str) -> String {
    let mut result =
        Style::new().underline().paint("Error:").to_string() + &format!(" {}:\n", error.error);

    let context_len = 3usize;
    let source_len = source.len();
    let semantic_source_len = if source.ends_with("\n") {
        source_len - 1
    } else {
        source_len
    };

    let error_start = error.location.start;
    let error_end = error_start + error.location.extent;

    let pre_error_start = std::cmp::max(0, error_start as i64 - context_len as i64) as usize;
    let pre_error_end = error.location.start;

    let post_error_start = std::cmp::min(source_len, error_end);
    let post_error_end = std::cmp::min(source_len, error_end + context_len);

    let pre_context = &source[pre_error_start..pre_error_end];
    let erroneous = &source[error_start..error_end];
    let post_context = &source[post_error_start..post_error_end].trim_end_matches("\n");

    // Print the error in context.
    result += "\t";
    if pre_error_start != 0 {
        result += "...";
    }
    result += pre_context;

    result += &Red.underline().paint(erroneous).to_string();

    result += post_context;
    if post_error_end <= semantic_source_len {
        result += "...";
    }
    result += "\n";
    result += &format!("\tat {}:{},{}", path, error_start, error_end);

    result
}

fn format_error(error: MelAnalysisLocatableError, source: &str, path: &str) -> String {
    if let MelAnalysisError::CompilerError(e) = *error.error {
        format_compiler_error(e, source, path)
    } else {
        format_analysis_error(error, source, path)
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let Cli {
        debug: raw_debug,
        log_file: maybe_log_file,
        command,
    } = Cli::parse();

    let (debug, command) = (Into::<DebugLevel>::into(raw_debug), command);

    // First, setup logging.
    let mut log_builder = LogSpecification::builder();
    log_builder.default(From::<DebugLevel>::from(debug));
    let mut logger = Logger::with(log_builder.build());
    logger = if let Some(log_file) = maybe_log_file {
        logger.log_to_file(FileSpec::default().directory(log_file.path()))
    } else {
        logger
    };

    let logger = logger
        .start()
        .unwrap_or_else(|e| panic!("Logger initialization failed with {}", e));

    info!(
        "Logging at {:?} level",
        logger
            .current_max_level()
            .unwrap_or_else(|e| panic!("Logger interrogation failed with {}", e))
    );

    info!("Executing requested command: {:?}", command);

    let result = match command {
        Commands::Compile { path } => compile_and_serialize(path),
        Commands::Analyze { path } => compile_and_analyze(path),
        Commands::Interpret { path } => compile_and_interpret(path),
        Commands::Serve { host, port } => serve::serve(host, port)
            .await
            .map_err(CliError::ServerError),
        Commands::Proxy { host, port, path } => match parse_and_analyze_processing_stages(path) {
            Ok(crs) => proxy::proxy(host, port, crs)
                .await
                .map_err(CliError::ServerError),
            Err(e) => Err(e),
        },

        #[cfg(feature = "domain")]
        Commands::HmdsServer {
            host,
            port,
            path,
            timeout,
            user,
            group,
        } => match hmds::server(host, port, path, timeout, user, group).await {
            Ok(_) => Ok(()),
            Err(e) => Err(CliError::SocketError(e)),
        },
        #[cfg(not(feature = "domain"))]
        Commands::HmdsServer {
            host,
            port,
            timeout,
        } => match hmds::server(
            host,
            port,
            #[cfg(feature = "domain")]
            path,
            timeout,
            None,
            None,
        )
        .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(CliError::SocketError(e)),
        },
    };

    if let Err(e) = result {
        println!("Error: {e}");
        let mut cli = Cli::command();
        println!("{}", cli.render_help());
    }
}
