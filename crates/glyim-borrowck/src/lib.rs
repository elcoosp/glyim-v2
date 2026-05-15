//! Borrow checker using non-lexical lifetimes (NLL) with Polonius-style
//! region inference.
//!
//! This implementation tracks borrows across basic block boundaries using
//! a CFG-aware liveness analysis. Loans are tracked as sets, and conflicts
//! are detected between active borrows and place accesses.
//!
//! The analysis proceeds in three phases:
//! 1. **Loan collection**: Scan the MIR body for `Rvalue::Ref` assignments
//!    and record each as a `Loan` with the borrowed place, borrow kind,
//!    and the local holding the reference.
//! 2. **Liveness analysis**: Compute which locals are live at each program
//!    point using a standard backward dataflow analysis on the CFG.
//! 3. **Conflict detection**: For each statement, determine which loans are
//!    active (their dest local is live) and check for conflicts with
//!    place accesses in the statement.

use fixedbitset::FixedBitSet as BitSet;
use glyim_diag::{DiagSeverity, GlyimDiagnostic, MultiSpan, SubDiagnostic};
use glyim_mir::{
    Place,
