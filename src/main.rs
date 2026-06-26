use std::{collections::HashMap, io::Read, sync::Arc};

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
    interp::{
        self, BuiltinFunction, MelInterpLocatableError, PathElementBuiltin, StructValue,
        TypedValue, Value,
    },
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

#[derive(Debug)]
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
fn compile_and_interpret(path: clio::ClioPath) -> CliResult<()> {
    let mut f = path.open().map_err(|_| CliError::BadPath)?;

    let mut to_parse: Vec<u8> = vec![];
    f.read_to_end(&mut to_parse)
        .map_err(|_| CliError::BadPath)?;

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

    let path_element_builtin = PathElementBuiltin {};

    analysis_scopes = analysis_scopes.insert(
        &path_element_builtin.name(),
        Type::Function(
            Arc::new(path_element_builtin.return_type()),
            path_element_builtin.parameters(),
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

    let compiled =
        analysis::compile_and_analyze(&String::from_utf8_lossy(&to_parse), analysis_scopes)
            .map_err(CliError::AnalysisError)?;
    let value = interp::interpret(&compiled, interp_scopes).map_err(CliError::InterpreterError)?;

    println!("{}", value);

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let result = match Cli::parse() {
        Cli {
            command: Commands::Compile { path },
        } => compile_and_serialize(path),
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
