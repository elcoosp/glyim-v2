//! Expression checking logic for FnCtxt.

use std::collections::HashMap;

use glyim_core::def_id::{AdtId, FnDefId};
use glyim_core::primitives::*;
use glyim_diag::GlyimDiagnostic;
use glyim_hir::*;
use glyim_type::{GenericArg, Region, Ty, TyKind};
use glyim_core::interner::Name;
use glyim_span::Span;