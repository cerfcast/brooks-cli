use std::io::Read;

#[allow(
    redundant_imports,
    unused_imports,
    clippy::single_component_path_imports
)]
use brooks_lib;

#[cfg(test)]
mod test;

use brooks_lib::{
    ast::AstVisitorDriver,
    compiler::{CompilerError, MELCompilerContext, SyntaxError::EmptyContext, compile},
    expect_expr,
    serializer::{AstTextSerializer, AstTextSerializerContext},
};
use clap::{CommandFactory, Parser, Subcommand};

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
}

#[derive(Debug, Clone)]
pub enum CliError {
    BadPath,
    CouldNotRead,
}
pub type CliResult<T> = Result<T, CliError>;

fn compile_and_serialize(path: clio::ClioPath) -> CliResult<()> {
    let mut f = path.open().map_err(|_| CliError::BadPath)?;

    let mut to_parse: Vec<u8> = vec![];
    f.read_to_end(&mut to_parse)
        .map_err(|_| CliError::BadPath)?;

    let compile_result = compile(&String::from_utf8_lossy(&to_parse));
    let compiled = compile_result.expect("Compilation error");
    let ast = expect_expr!(compiled)
        .ok_or(CompilerError::SyntaxError(EmptyContext))
        .expect("Missing AST");

    let driver = AstVisitorDriver {};
    let visitor = AstTextSerializer {};
    let context = AstTextSerializerContext {
        serialized: "".to_string(),
        indent: 0,
    };
    let result = driver
        .visit(ast, &visitor, context)
        .expect("Could not serialize");
    println!("{}", result.serialized);
    Ok(())
}
fn main() {
    let result = match Cli::parse() {
        Cli {
            command: Commands::Compile { path },
        } => compile_and_serialize(path),
    };

    if let Err(e) = result {
        println!("Error: {e:?}");
        let mut cli = Cli::command();
        println!("{}", cli.render_help());
    }
}
