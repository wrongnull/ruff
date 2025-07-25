use ast::FStringFlags;
use itertools::Itertools;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Arguments, Expr};
use ruff_text_size::{Ranged, TextRange};

use crate::checkers::ast::Checker;
use crate::fix::edits::pad;
use crate::fix::snippet::SourceCodeSnippet;
use crate::{AlwaysFixableViolation, Edit, Fix};

use crate::rules::flynt::helpers;

/// ## What it does
/// Checks for `str.join` calls that can be replaced with f-strings.
///
/// ## Why is this bad?
/// f-strings are more readable and generally preferred over `str.join` calls.
///
/// ## Example
/// ```python
/// " ".join((foo, bar))
/// ```
///
/// Use instead:
/// ```python
/// f"{foo} {bar}"
/// ```
///
/// ## Fix safety
/// The fix is always marked unsafe because the evaluation of the f-string
/// expressions will default to calling the `__format__` method of each
/// object, whereas `str.join` expects each object to be an instance of
/// `str` and uses the corresponding string. Therefore it is possible for
/// the values of the resulting strings to differ, or for one expression
/// to raise an exception while the other does not.
///
/// ## References
/// - [Python documentation: f-strings](https://docs.python.org/3/reference/lexical_analysis.html#f-strings)
#[derive(ViolationMetadata)]
pub(crate) struct StaticJoinToFString {
    expression: SourceCodeSnippet,
}

impl AlwaysFixableViolation for StaticJoinToFString {
    #[derive_message_formats]
    fn message(&self) -> String {
        let StaticJoinToFString { expression } = self;
        if let Some(expression) = expression.full_display() {
            format!("Consider `{expression}` instead of string join")
        } else {
            "Consider f-string instead of string join".to_string()
        }
    }

    fn fix_title(&self) -> String {
        let StaticJoinToFString { expression } = self;
        if let Some(expression) = expression.full_display() {
            format!("Replace with `{expression}`")
        } else {
            "Replace with f-string".to_string()
        }
    }
}

fn is_static_length(elts: &[Expr]) -> bool {
    elts.iter().all(|e| !e.is_starred_expr())
}

/// Build an f-string consisting of `joinees` joined by `joiner` with `flags`.
fn build_fstring(joiner: &str, joinees: &[Expr], flags: FStringFlags) -> Option<Expr> {
    // If all elements are string constants, join them into a single string.
    if joinees.iter().all(Expr::is_string_literal_expr) {
        let mut flags = None;
        let node = ast::StringLiteral {
            value: joinees
                .iter()
                .filter_map(|expr| {
                    if let Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) = expr {
                        if flags.is_none() {
                            // take the flags from the first Expr
                            flags = Some(value.first_literal_flags());
                        }
                        Some(value.to_str())
                    } else {
                        None
                    }
                })
                .join(joiner)
                .into_boxed_str(),
            flags: flags?,
            range: TextRange::default(),
            node_index: ruff_python_ast::AtomicNodeIndex::dummy(),
        };
        return Some(node.into());
    }

    let mut f_string_elements = Vec::with_capacity(joinees.len() * 2);
    let mut first = true;

    for expr in joinees {
        if expr.is_f_string_expr() {
            // Oops, already an f-string. We don't know how to handle those
            // gracefully right now.
            return None;
        }
        if !std::mem::take(&mut first) {
            f_string_elements.push(helpers::to_interpolated_string_literal_element(joiner));
        }
        f_string_elements.push(helpers::to_interpolated_string_element(expr)?);
    }

    let node = ast::FString {
        elements: f_string_elements.into(),
        range: TextRange::default(),
        node_index: ruff_python_ast::AtomicNodeIndex::dummy(),
        flags,
    };
    Some(node.into())
}

/// FLY002
pub(crate) fn static_join_to_fstring(checker: &Checker, expr: &Expr, joiner: &str) {
    let Expr::Call(ast::ExprCall {
        arguments: Arguments { args, keywords, .. },
        ..
    }) = expr
    else {
        return;
    };

    // If there are kwargs or more than one argument, this is some non-standard
    // string join call.
    if !keywords.is_empty() {
        return;
    }
    let [arg] = &**args else {
        return;
    };

    // Get the elements to join; skip (e.g.) generators, sets, etc.
    let joinees = match &arg {
        Expr::List(ast::ExprList { elts, .. }) if is_static_length(elts) => elts,
        Expr::Tuple(ast::ExprTuple { elts, .. }) if is_static_length(elts) => elts,
        _ => return,
    };

    // Try to build the fstring (internally checks whether e.g. the elements are
    // convertible to f-string elements).
    let Some(new_expr) = build_fstring(joiner, joinees, checker.default_fstring_flags()) else {
        return;
    };

    let contents = checker.generator().expr(&new_expr);

    let mut diagnostic = checker.report_diagnostic(
        StaticJoinToFString {
            expression: SourceCodeSnippet::new(contents.clone()),
        },
        expr.range(),
    );
    diagnostic.set_fix(Fix::unsafe_edit(Edit::range_replacement(
        pad(contents, expr.range(), checker.locator()),
        expr.range(),
    )));
}
