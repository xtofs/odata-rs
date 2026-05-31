use super::*;
use itertools::Itertools;
use std::fmt::{Debug, Display, Formatter, Result};

const MIN_PRECEDENCE: u8 = 0;
const UNARY_PRECEDENCE: u8 = 5;

impl Debug for FilterSpan {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}:{}", self.start, self.end)
    }
}

impl Display for FilterClause {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", Pretty(self, MIN_PRECEDENCE))
    }
}

/// internal trait for formatting with precedence to be able to place parenthesis
/// conditionally based on parent precedence
/// this will only be implemented for structs that introduce recursion
trait FormatWithPrecedence {
    fn fmtp(&self, parent_precedence: u8, f: &mut Formatter<'_>) -> Result;
}

/// Wrapper to carry the parent precedence down the formatting tree.
struct Pretty<'a, T: FormatWithPrecedence>(&'a T, u8);

/// blanket implementation of DIsplay for any type that
/// implements FormatWithPrecedence
impl<'a, T: FormatWithPrecedence> Display for Pretty<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        self.0.fmtp(self.1, f)
    }
}

impl FormatWithPrecedence for FilterClause {
    fn fmtp(&self, pp: u8, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", Pretty(&self.expression, pp))
    }
}

impl FormatWithPrecedence for FilterExpression {
    fn fmtp(&self, pp: u8, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", Pretty(&self.kind, pp))
    }
}

impl FormatWithPrecedence for FilterExpressionKind {
    fn fmtp(&self, pp: u8, f: &mut Formatter<'_>) -> Result {
        match self {
            FilterExpressionKind::Literal(filter_literal) => write!(f, "{}", filter_literal),
            FilterExpressionKind::Member(filter_member_path) => write!(f, "{}", filter_member_path),
            FilterExpressionKind::FunctionCall(filter_function_call) => {
                write!(f, "{}", Pretty(filter_function_call, pp))
            }
            FilterExpressionKind::Unary { operator, operand } => {
                write!(
                    f,
                    "{}{}",
                    operator,
                    Pretty(operand.as_ref(), UNARY_PRECEDENCE)
                )
            }
            FilterExpressionKind::Binary {
                left,
                operator,
                right,
            } => {
                let prec = operator_precedence(operator);
                let needs_parens = prec < pp;
                if needs_parens {
                    write!(
                        f,
                        "({} {} {})",
                        Pretty(left.as_ref(), prec),
                        operator,
                        Pretty(right.as_ref(), prec)
                    )
                } else {
                    write!(
                        f,
                        "{} {} {}",
                        Pretty(left.as_ref(), prec),
                        operator,
                        Pretty(right.as_ref(), prec)
                    )
                }
            }
        }
    }
}

impl Display for FilterLiteral {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            FilterLiteral::Null => write!(f, "null"),
            FilterLiteral::Boolean(value) => write!(f, "{}", value),
            FilterLiteral::Number(value) => write!(f, "{}", value),
            FilterLiteral::String(value) => write!(f, "'{}'", value.replace('\'', "''")),
        }
    }
}

impl FormatWithPrecedence for FilterFunctionCall {
    fn fmtp(&self, _pp: u8, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}({})",
            self.name,
            self.arguments
                .iter()
                .map(|argument| Pretty(argument, MIN_PRECEDENCE))
                .format(", ")
        )
    }
}

impl Display for FilterMemberPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.segments.iter().format("/"))
    }
}

impl Display for FilterUnaryOperator {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            FilterUnaryOperator::Not => write!(f, "not "),
            FilterUnaryOperator::Negate => write!(f, "-"),
        }
    }
}

impl Display for FilterBinaryOperator {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let operator = match self {
            FilterBinaryOperator::Or => "or",
            FilterBinaryOperator::And => "and",
            FilterBinaryOperator::Equal => "eq",
            FilterBinaryOperator::NotEqual => "ne",
            FilterBinaryOperator::GreaterThan => "gt",
            FilterBinaryOperator::GreaterThanOrEqual => "ge",
            FilterBinaryOperator::LessThan => "lt",
            FilterBinaryOperator::LessThanOrEqual => "le",
            FilterBinaryOperator::Add => "add",
            FilterBinaryOperator::Subtract => "sub",
            FilterBinaryOperator::Multiply => "mul",
            FilterBinaryOperator::Divide => "div",
            FilterBinaryOperator::Modulo => "mod",
        };

        write!(f, "{}", operator)
    }
}

fn operator_precedence(operator: &FilterBinaryOperator) -> u8 {
    match operator {
        FilterBinaryOperator::Or => 0,
        FilterBinaryOperator::And => 1,
        FilterBinaryOperator::Equal => 2,
        FilterBinaryOperator::NotEqual => 2,
        FilterBinaryOperator::GreaterThan => 2,
        FilterBinaryOperator::GreaterThanOrEqual => 2,
        FilterBinaryOperator::LessThan => 2,
        FilterBinaryOperator::LessThanOrEqual => 2,
        FilterBinaryOperator::Add => 3,
        FilterBinaryOperator::Subtract => 3,
        FilterBinaryOperator::Multiply => 4,
        FilterBinaryOperator::Divide => 4,
        FilterBinaryOperator::Modulo => 4,
    }
}
