use std::{collections::HashMap, io::Read, sync::Arc};

use ansi_term::Color::Red;
#[allow(
    redundant_imports,
    unused_imports,
    clippy::single_component_path_imports
)]
use brooks_lib;

#[cfg(test)]
mod test;

use brooks_lib::{
    analysis::{self, MelAnalysisLocatableError},
    ast::AstVisitorDriver,
    compiler::{CompilerError, MELCompilerContext, SyntaxError::EmptyContext, compile},
    expect_expr,
    interpreter::{
        self,
        builtins::{BooleanBuiltin, BuiltinFunction, Path_ElementBuiltin},
        interpret::{MelInterpContext, MelInterpLocatableError, StructValue, TypedValue, Value},
    },
    logging::{LogLevel::Trace, LogMsgFormatter, LogMsgs},
    scope::Scopes,
    serializer::{AstTextSerializer, AstTextSerializerContext},
    tvs::{Struct, Type},
};
use clap::{CommandFactory, Parser, Subcommand};

mod serve;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
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
}

fn common_scopes() -> (Scopes<Type>, Scopes<TypedValue>) {
    // Set up the built-in variables for type checking.
    let mut analysis_scopes = Scopes::<Type>::default();
    let mut headers = Struct {
        name: "headers".to_string(),
        fields: HashMap::new(),
    };

    headers.fields.insert("method".to_string(), Type::String);

    let mut reqs = Struct {
        name: "req".to_string(),
        fields: HashMap::new(),
    };
    reqs.fields
        .insert("incoming".to_string(), Type::Struct(headers.clone()));

    analysis_scopes = analysis_scopes.insert("req", Type::Struct(reqs.clone()));

    let path_element_builtin = Path_ElementBuiltin {};
    let boolean_builtin = BooleanBuiltin {};

    analysis_scopes = analysis_scopes.insert(
        &path_element_builtin.name(),
        Type::Function(
            Arc::new(path_element_builtin.return_type()),
            path_element_builtin.parameters(),
        ),
    );

    analysis_scopes = analysis_scopes.insert(
        &boolean_builtin.name(),
        Type::Function(
            Arc::new(boolean_builtin.return_type()),
            boolean_builtin.parameters(),
        ),
    );

    // Set up the built-in variables for interpreting.
    let mut interp_scopes = Scopes::<TypedValue>::default();

    let mut reqsv = StructValue {
        fields: HashMap::new(),
        tpe: reqs.clone(),
    };

    let mut headersv = StructValue {
        fields: HashMap::new(),
        tpe: headers.clone(),
    };

    headersv.fields.insert(
        "method".to_string(),
        TypedValue {
            value: Value::String("GET".to_string()),
            tipe: Type::String,
        },
    );
    reqsv.fields.insert(
        "incoming".to_string(),
        TypedValue {
            value: Value::Struct(headersv),
            tipe: Type::Struct(headers.clone()),
        },
    );

    interp_scopes = interp_scopes.insert(
        "req",
        TypedValue {
            value: Value::Struct(reqsv),
            tipe: Type::Struct(reqs),
        },
    );

    interp_scopes = interp_scopes.insert(
        &path_element_builtin.name(),
        TypedValue {
            value: Value::Function(Arc::new(path_element_builtin.clone())),
            tipe: Type::Function(
                Arc::new(path_element_builtin.return_type()),
                path_element_builtin.parameters(),
            ),
        },
    );

    interp_scopes = interp_scopes.insert(
        &boolean_builtin.name(),
        TypedValue {
            value: Value::Function(Arc::new(boolean_builtin.clone())),
            tipe: Type::Function(
                Arc::new(boolean_builtin.return_type()),
                boolean_builtin.parameters(),
            ),
        },
    );

    (analysis_scopes, interp_scopes)
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum CliError {
    BadPath,
    CouldNotRead,
    AnalysisError(MelAnalysisLocatableError),
    InterpreterError(MelInterpLocatableError),
    ServerError(std::io::Error),
}
pub type CliResult<T> = Result<T, CliError>;

#[allow(clippy::result_large_err)]
fn compile_and_serialize(path: clio::ClioPath) -> CliResult<()> {
    let mut f = path.open().map_err(|_| CliError::BadPath)?;

    let mut to_parse: Vec<u8> = vec![];
    f.read_to_end(&mut to_parse)
        .map_err(|_| CliError::BadPath)?;

    let compile_result = compile(&String::from_utf8_lossy(&to_parse));
    let compiled = compile_result.expect("Compilation error");
    let ast = expect_expr!(MELCompilerContext, compiled)
        .ok_or(CompilerError::SyntaxError(EmptyContext))
        .expect("Missing AST");

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

#[allow(clippy::result_large_err)]
fn compile_and_analyze(path: clio::ClioPath) -> CliResult<()> {
    let mut f = path.clone().open().map_err(|_| CliError::BadPath)?;

    let mut to_parse: Vec<u8> = vec![];
    f.read_to_end(&mut to_parse)
        .map_err(|_| CliError::BadPath)?;

    let source = &String::from_utf8_lossy(&to_parse);
    let (analysis_scopes, _) = common_scopes();
    let result = analysis::compile_and_analyze(source, analysis_scopes);

    match result {
        Ok(r) => println!("Expression Type: {}", r.tipe().to_string()),
        Err(e) => {
            println!("Could not analyze:");
            let context_len = 3usize;
            let source_len = source.len();
            let semantic_source_len = if source.ends_with("\n") {
                source_len - 1
            } else {
                source_len
            };

            let error_start = e.location.start;
            let error_end = error_start + e.location.extent;

            let pre_error_start =
                std::cmp::max(0, error_start as i64 - context_len as i64) as usize;
            let pre_error_end = e.location.start;

            let post_error_start = std::cmp::min(source_len, error_end);
            let post_error_end = std::cmp::min(source_len, error_end + context_len);

            let pre_context = &source[pre_error_start..pre_error_end];
            let erroneous = &source[error_start..error_end];
            let post_context = &source[post_error_start..post_error_end].trim_end_matches("\n");

            if pre_error_start != 0 {
                print!("...");
            }
            print!("{pre_context}");

            print!("{}", Red.underline().paint(erroneous));

            print!("{post_context}");
            if post_error_end <= semantic_source_len {
                print!("...");
            }
            println!();
            println!("at {}:{},{}", path, error_start, error_end);
            println!("{}", e.error);
        }
    };
    Ok(())
}

#[allow(clippy::result_large_err)]
fn compile_and_interpret(path: clio::ClioPath) -> CliResult<()> {
    let mut f = path.open().map_err(|_| CliError::BadPath)?;

    let mut to_parse: Vec<u8> = vec![];
    f.read_to_end(&mut to_parse)
        .map_err(|_| CliError::BadPath)?;

    let (analysis_scopes, interp_scopes) = common_scopes();

    let compiled =
        analysis::compile_and_analyze(&String::from_utf8_lossy(&to_parse), analysis_scopes)
            .map_err(CliError::AnalysisError)?;

    let mut interp_context = MelInterpContext::default();

    interp_context = interp_context
        .update_log(LogMsgs::new(Trace))
        .update_scopes(interp_scopes);
    match interpreter::interpret(&compiled, interp_context) {
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

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let result = match Cli::parse() {
        Cli {
            command: Commands::Compile { path },
        } => compile_and_serialize(path),
        Cli {
            command: Commands::Analyze { path },
        } => compile_and_analyze(path),
        Cli {
            command: Commands::Interpret { path },
        } => compile_and_interpret(path),
        Cli {
            command: Commands::Serve { host, port },
        } => serve::serve(host, port)
            .await
            .map_err(CliError::ServerError),
    };

    if let Err(e) = result {
        println!("Error: {e:?}");
        let mut cli = Cli::command();
        println!("{}", cli.render_help());
    }
}
