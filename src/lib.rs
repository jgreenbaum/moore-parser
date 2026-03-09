// Copyright (c) 2016-2021 Fabian Schuiki

//! A hardware description language compiler.

#![allow(dead_code)]
#![allow(unused_variables)]

#[macro_use]
extern crate log;
#[macro_use]
pub extern crate moore_common as _;

use std::path::Path;

use moore_common::errors::DiagBuilder2;
pub use moore_common as common;
pub use moore_common::*;
pub use moore_svlog as svlog;
pub use moore_vhdl as vhdl;

pub mod score;

#[derive(Debug)]
enum Language {
    Verilog,
    SystemVerilog,
    Vhdl,
}

pub fn parse_string<'a>(filename: &str, content: String, 
                         include_paths: &[&Path], defines: &[(&str, Option<&str>)]) 
    -> Result<score::Ast<'a>, DiagBuilder2>
{
    // Detect the file type.
    let language = match Path::new(&filename).extension().and_then(|s| s.to_str()) {
        Some("sv") | Some("svh") => Language::SystemVerilog,
        Some("v") | Some("vh") => Language::Verilog,
        Some("vhd") | Some("vhdl") => Language::Vhdl,
        Some(ext) => {
            return Err(
                DiagBuilder2::warning(format!("ignoring `{}`", filename)).add_note(format!(
                    "Cannot determine language from extension `.{}`",
                    ext
                )));
        }
        None => {
            return Err(
                DiagBuilder2::warning(format!("ignoring `{}`", filename)).add_note(format!(
                    "No file extension that can be used to guess language"
                )));
        }
    };

    // Add the file to the source manager.
    let sm = source::get_source_manager();
    let source = sm.add(filename, &content.as_str());

    // Parse the file.
    match language {
        Language::SystemVerilog | Language::Verilog => {
            let preproc = svlog::preproc::Preprocessor::new(source, &include_paths, &defines);
            let lexer = svlog::lexer::Lexer::new(preproc);
            match svlog::parser::parse(lexer) {
                Ok(x) => return Ok(score::Ast::Svlog(x)),
            Err(()) => return Err(DiagBuilder2::error(format!("Failed to parse {}", filename))),
            }
        }
        Language::Vhdl => match vhdl::syntax::parse(source) {
            Ok(x) => return Ok(score::Ast::Vhdl(x)),
            Err(()) => return Err(DiagBuilder2::error(format!("Failed to parse {}", filename))),
        },
    }
}