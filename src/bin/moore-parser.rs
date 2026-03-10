// Copyright (c) 2016-2021 Fabian Schuiki

//! A hardware description language compiler.

extern crate log;

use clap::{App, Arg, ArgMatches};
// use llhd;
// use moore_circt::{self as circt, mlir, prelude::*, sys::*};
// use llhd::opt::{Pass, PassContext};
// use moore::common::score::NodeRef;
use moore_parser::errors::*;
// use moore_parser::name::Name;
// use moore_parser::score::{ScoreBoard, ScoreContext};
// use moore_parser::source::Span;
use moore_parser::svlog::{/*ast::AcceptVisitor as _, hir::Visitor as _,*/ QueryDatabase as _};
use moore_parser::*;
use std::fs::OpenOptions;
use std::io::Read;

fn main() {
    // Configure the logger.
    let mut builder = pretty_env_logger::formatted_builder();
    builder.parse_filters(
        std::env::var("MOORE_LOG")
            .ok()
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("off"),
    );
    builder.try_init().unwrap();

    // Parse the command-line arguments.
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(clap::crate_version!())
        .author(clap::crate_authors!())
        .about(clap::crate_description!())
        .arg(
            Arg::with_name("trace_scoreboard")
                .long("trace-scoreboard")
                .global(true),
        )
        .arg(
            Arg::with_name("verbosity-opts")
                .short("V")
                .help("Sets verbosity settings")
                .takes_value(true)
                .multiple(true)
                .number_of_values(1)
                .possible_values(&[
                    "types",
                    "expr-types",
                    "type-contexts",
                    "typeck",
                    "names",
                    "casts",
                    "ports",
                    "consts",
                    "insts",
                    "func-args",
                    "call-args",
                ])
                .global(true),
        )
        .arg(
            Arg::with_name("inc")
                .short("I")
                .value_name("DIR")
                .help("Add a search path for SystemVerilog includes")
                .multiple(true)
                .takes_value(true)
                .number_of_values(1),
        )
        .arg(
            Arg::with_name("def")
                .short("D")
                .value_name("DEFINE")
                .help("Define a preprocesor macro")
                .multiple(true)
                .takes_value(true)
                .number_of_values(1),
        )
        .arg(
            Arg::with_name("preproc")
                .short("E")
                .help("Write preprocessed input files to stdout"),
        )
        .arg(
            Arg::with_name("dump-ast")
                .long("dump-ast")
                .help("Dump the parsed abstract syntax tree"),
        )
        .arg(
            Arg::with_name("check-syntax")
                .long("syntax")
                .help("Preprocess and check the input for syntax errors"),
        )
        .arg(
            Arg::with_name("emit_pkgs")
                .long("emit-pkgs")
                .help("Dump VHDL packages for debugging"),
        )
        .arg(
            Arg::with_name("opt-level")
                .short("O")
                .long("opt-level")
                .help("Sets optimization level applied to the output")
                .default_value("1")
                .takes_value(true)
                .number_of_values(1),
        )
        .arg(
            Arg::with_name("lib")
                .short("l")
                .long("lib")
                .value_name("LIB")
                .help("Name of the library to compile into")
                .takes_value(true)
                .number_of_values(1),
        )
        .arg(
            Arg::with_name("elaborate")
                .short("e")
                .long("elaborate")
                .value_name("ENTITY")
                .help("Elaborate an entity or module")
                .multiple(true)
                .takes_value(true)
                .number_of_values(1),
        )
        .arg(
            Arg::with_name("output")
                .short("o")
                .long("output")
                .help("Output file (`-` for stdout)")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("output-format")
                .short("f")
                .long("format")
                .help("Output format")
                .takes_value(true)
                .possible_values(&["llhd", "mlir", "mlir-native"]),
        )
        .arg(
            Arg::with_name("debug-info")
                .short("g")
                .long("debug-info")
                .help("Emit location information as part of the output"),
        )
        .arg(
            Arg::with_name("INPUT")
                .help("The input files to compile")
                .multiple(true)
                .required(true),
        )
        .get_matches();

    // Configure the session.
    let mut session = Session::new();
    session.opts.trace_scoreboard = matches.is_present("trace_scoreboard");
    for v in matches
        .values_of("verbosity-opts")
        .into_iter()
        .flat_map(|v| v)
    {
        session.opts.verbosity |= match v {
            "types" => Verbosity::TYPES,
            "expr-types" => Verbosity::EXPR_TYPES,
            "type-contexts" => Verbosity::TYPE_CONTEXTS,
            "typeck" => Verbosity::TYPECK,
            "names" => Verbosity::NAMES,
            "casts" => Verbosity::CASTS,
            "ports" => Verbosity::PORTS,
            "consts" => Verbosity::CONSTS,
            "insts" => Verbosity::INSTS,
            "func-args" => Verbosity::FUNC_ARGS,
            "call-args" => Verbosity::CALL_ARGS,
            _ => unreachable!(),
        };
    }
    session.opts.opt_level = matches.value_of("opt-level").unwrap().parse().unwrap();

    // Invoke the compiler.
    score(&session, &matches);
}

fn score(sess: &Session, matches: &ArgMatches) {
    // Prepare a list of include paths.
    let include_paths: Vec<_> = match matches.values_of("inc") {
        Some(args) => args.map(|x| std::path::Path::new(x)).collect(),
        None => Vec::new(),
    };

    let defines: Vec<_> = match matches.values_of("def") {
        Some(args) => args
            .map(|x| {
                let mut iter = x.split("=");
                (iter.next().unwrap(), iter.next())
            })
            .collect(),
        None => Vec::new(),
    };

    // Parse the input files.
    let mut failed = false;
    let mut asts = Vec::new();
    for filename in matches.values_of("INPUT").unwrap() {
        if filename.is_empty() {
            continue;
        }
        let mut file = match OpenOptions::new()
                            .create(false)
                            .read(true)
                            .open(filename) {
            Ok(f) => f,
            Err(_) => {
                sess.emit(
                    DiagBuilder2::warning(format!("Failed to open input file `{}`", filename))
                );
                failed = true;
                continue;
            }
        };
        let mut content = String::new();
        match file.read_to_string(&mut content) {
            Ok(_) => (),
            Err(_) => {
                sess.emit(
                    DiagBuilder2::warning(format!("Failed to read from `{}`", filename))
                );
                failed = true;
                continue;
            }
        }
        
        match parse_string(filename, content, include_paths.as_slice(), defines.as_slice()) {
            Ok(ast) => asts.push(ast),
            Err(diag) => sess.emit(diag),
        };
    }
    if failed || sess.failed() {
        std::process::exit(1);
    }
    if matches.is_present("preproc") {
        return;
    }

    // Dump the AST if so requested.
    if matches.is_present("dump-ast") {
        println!("{:#99?}", asts);
    }

    if matches.is_present("emit_pkgs") {
        vhdl::debug::emit_pkgs(
            sess,
            asts.iter()
                .flat_map(|ast| match *ast {
                    score::Ast::Vhdl(ref x) => x.iter(),
                    _ => [].iter(),
                })
                .collect(),
        );
    }

    // Stop processing if requested.
    if matches.is_present("check-syntax") {
        std::process::exit(0);
    }

    if sess.failed() {
        std::process::exit(1);
    }
}

/// A visitor that emits detailed type information to stdout.
pub struct TypeVerbosityVisitor<'a, 'gcx>(&'a svlog::GlobalContext<'gcx>, svlog::ParamEnv);

impl<'a, 'gcx> svlog::hir::Visitor<'gcx> for TypeVerbosityVisitor<'a, 'gcx> {
    type Context = svlog::GlobalContext<'gcx>;

    fn context(&self) -> &Self::Context {
        self.0
    }

    fn visit_expr(&mut self, expr: &'gcx svlog::hir::Expr<'gcx>, lvalue: bool) {
        self.print(expr.id);
        svlog::hir::walk_expr(self, expr, lvalue);
    }

    fn visit_var_decl(&mut self, decl: &'gcx svlog::hir::VarDecl) {
        self.print(decl.id);
        svlog::hir::walk_var_decl(self, decl);
    }
}

impl<'a, 'gcx> TypeVerbosityVisitor<'a, 'gcx> {
    fn print(&mut self, id: NodeId) {
        use svlog::Context;
        let span = self.0.span(id);
        let ext = span.extract();
        let line = span.begin().human_line();

        // Report the type.
        if let Ok(ty) = self.0.type_of(id, self.1) {
            println!("{}: type({}) = {}", line, ext, ty);
        }

        // Report the cast type.
        if let Some(cast) = self.0.cast_type(id, self.1) {
            println!("{}: cast_type({}) = {}", line, ext, cast.ty);
            println!("{}: cast_chain({}) = {}", line, ext, cast);
        }

        // Report the self-determined type.
        if let Some(ty) = self.0.self_determined_type(id, self.1) {
            println!("{}: self_type({}) = {}", line, ext, ty);
        }

        // Report the operation type.
        if let Some(ty) = self.0.operation_type(id, self.1) {
            println!("{}: operation_type({}) = {}", line, ext, ty);
        }

        // Report the type context.
        if let Some(expr) = self.0.ast_for_id(id).as_all().get_expr() {
            if let Some(ty) = self.0.type_context(svlog::Ref(expr), self.1) {
                println!(
                    "{}: type_context({}) = {}",
                    line,
                    ext,
                    match ty {
                        svlog::typeck::TypeContext::Type(ty) => format!("{}", ty),
                        svlog::typeck::TypeContext::Bool => "<bool>".to_string(),
                    }
                );
            }
        }
    }
}
