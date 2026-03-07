// Copyright (c) 2016-2021 Fabian Schuiki

//! This module implements LLHD code generation.
#![allow(unreachable_code)]

// use moore_circt::{self as circt, comb::CmpPred, mlir, prelude::*};
// use num::{BigInt, FromPrimitive, One, ToPrimitive, Zero};
use std::{
    ops::Deref,
};

/* pub type HybridValue = (llhd::ir::Value, mlir::Value);
pub type HybridType = (llhd::Type, mlir::Type);
pub type HybridBlock = (llhd::ir::Block, mlir::Block); */

/// A code generator.
///
/// Use this struct to emit LLHD code for nodes in a [`Context`].
pub struct CodeGenerator<C> {
    /// The compilation context.
    cx: C,
    /// The MLIR compilation context.
    // mcx: mlir::Context,
    /// The LLHD module to be populated.
    into: llhd::ir::Module,
}

impl<C> CodeGenerator<C> {
    /// Create a new code generator.
    pub fn new(cx: C /*, into_mlir: circt::ModuleOp*/) -> Self {
        CodeGenerator {
            cx,
            // mcx: into_mlir.context(),
            into: llhd::ir::Module::new(),
            // into_mlir,
        }
    }

    /// Finalize code generation and return the generated LLHD module.
    pub fn finalize(self) -> llhd::ir::Module {
        self.into
    }
}

impl<C> Deref for CodeGenerator<C> {
    type Target = C;

    fn deref(&self) -> &C {
        &self.cx
    }
}

