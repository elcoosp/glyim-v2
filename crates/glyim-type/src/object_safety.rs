//! Object safety checks for traits used as `dyn Trait`.
//!
//! A trait is object-safe if:
//! 1. It does not require `Self: Sized` (explicitly or implicitly).
//! 2. All methods have receivers that can be dispatched (i.e., take `self` by reference
//!    or by value where `Self: Sized` is not required).
//! 3. No method has generic type parameters.
//! 4. No associated constants (future: Glyim doesn't have these yet).

use glyim_core::Name;
use glyim_span::Span;

/// Reasons a trait is not object-safe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectSafetyViolation {
    /// The trait requires `Self: Sized` (either directly or via a bound).
    SelfSized,
    /// A method has a generic type parameter, which can't be monomorphized through a vtable.
    GenericMethod { method: Name, span: Span },
    /// A method does not take `self` (no receiver) — static methods cannot be dispatched.
    StaticMethod { method: Name, span: Span },
    /// A method takes `self` by value on a trait that does not have `Self: Sized`.
    ByValueSelf { method: Name, span: Span },
    /// An associated function is not callable through a trait object.
    AssociatedFunction { name: Name, span: Span },
    /// The trait has an associated type that is not constrained (future).
    UnconstrainedAssociatedType { name: Name, span: Span },
}

/// HIR-level representation of a method signature for object safety checking.
/// This avoids depending on glyim-hir from glyim-type.
#[derive(Debug, Clone)]
pub struct MethodSignature {
    /// Name of the method
    pub name: Name,
    /// Span for error reporting
    pub span: Span,
    /// Whether the method takes `self` by value (`self`), reference (`&self`), or has no self.
    pub self_kind: MethodSelfKind,
    /// Whether the method has generic type parameters (excluding lifetime params).
    pub has_generic_params: bool,
    /// Whether the method returns `Self` (which would make it non-object-safe if by-value).
    pub returns_self: bool,
}

/// How a method takes the `self` parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodSelfKind {
    /// `self` by value: the method takes ownership.
    ByValue,
    /// `&self` or `&mut self`: the method takes a reference.
    ByReference,
    /// No `self` parameter: a static method or associated function.
    None,
}

/// Checks whether a trait is object-safe given its methods.
///
/// Returns a list of violations. An empty list means the trait is object-safe.
pub fn check_object_safety(
    requires_self_sized: bool,
    methods: &[MethodSignature],
) -> Vec<ObjectSafetyViolation> {
    let mut violations = Vec::new();

    if requires_self_sized {
        violations.push(ObjectSafetyViolation::SelfSized);
    }

    for method in methods {
        // Generic methods can't be put in a vtable
        if method.has_generic_params {
            violations.push(ObjectSafetyViolation::GenericMethod {
                method: method.name,
                span: method.span,
            });
        }

        match method.self_kind {
            MethodSelfKind::ByValue => {
                // Taking self by value is only allowed if the trait requires Self: Sized,
                // but we already flagged that. If not, it's a separate violation.
                if !requires_self_sized {
                    violations.push(ObjectSafetyViolation::ByValueSelf {
                        method: method.name,
                        span: method.span,
                    });
                }
            }
            MethodSelfKind::None => {
                // Static methods / associated functions without self cannot be dispatched.
                // However, they can still exist on an object-safe trait; they just can't be
                // called through the trait object. Glyim might allow this with a warning,
                // but for now we treat it as a violation.
                violations.push(ObjectSafetyViolation::StaticMethod {
                    method: method.name,
                    span: method.span,
                });
            }
            MethodSelfKind::ByReference => {
                // Fine: &self or &mut self
            }
        }
    }

    violations
}
