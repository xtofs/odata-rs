use super::*;
use itertools::Itertools;
use std::fmt::{Debug, Display, Formatter, Result};

impl Debug for FilterSpan {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}:{}", self.start, self.end)
    }
}

impl Display for FilterClause {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", Pretty(self, 0))
    }
}

trait FormatWithPrecedence {
    fn fmt(&self, parent_precedence: u8, f: &mut Formatter<'_>) -> Result;
}

// wrapper to cary the precedence
struct Pretty<'a, T: FormatWithPrecedence>(&'a T, u8);

// and blanket implementation for Display to unwrap precedence
impl<'a, T: FormatWithPrecedence> Display for Pretty<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        self.0.fmt(self.1, f)
    }
}

impl FormatWithPrecedence for FilterClause {
    fn fmt(&self, pp: u8, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", Pretty(&self.expression, pp))
    }
}

impl FormatWithPrecedence for FilterExpression {
    fn fmt(&self, pp: u8, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", Pretty(&self.kind, pp))
    }
}

impl FormatWithPrecedence for FilterExpressionKind {
    fn fmt(&self, pp: u8, f: &mut Formatter<'_>) -> Result {
        match self {
            FilterExpressionKind::Literal(filter_literal) => {
                write!(f, "{}", Pretty(filter_literal, pp))
            }
            FilterExpressionKind::Member(filter_member_path) => {
                // use itertools' format
                write!(f, "{}", filter_member_path.segments.iter().format("/"))
            }
            FilterExpressionKind::FunctionCall(filter_function_call) => {
                write!(f, "{}", Pretty(filter_function_call, pp))
            }
            FilterExpressionKind::Unary { operator, operand } => {
                write!(
                    f,
                    "{}{}",
                    Pretty(operator, pp),
                    Pretty(operand.as_ref(), pp)
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
                        Pretty(operator, pp),
                        Pretty(right.as_ref(), prec)
                    )
                } else {
                    write!(
                        f,
                        "{} {} {}",
                        Pretty(left.as_ref(), prec),
                        Pretty(operator, pp),
                        Pretty(right.as_ref(), prec)
                    )
                }
            }
        }
    }
}

impl FormatWithPrecedence for FilterLiteral {
    fn fmt(&self, _pp: u8, f: &mut Formatter<'_>) -> Result {
        match self {
            FilterLiteral::Null => write!(f, "null"),
            FilterLiteral::Boolean(b) => write!(f, "{}", b),
            FilterLiteral::Number(n) => write!(f, "{}", n),
            FilterLiteral::String(s) => write!(f, "'{}'", s),
        }
    }
}

impl FormatWithPrecedence for FilterFunctionCall {
    fn fmt(&self, pp: u8, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}({})",
            self.name,
            self.arguments.iter().map(|a| Pretty(a, pp)).format(", ")
        )
    }
}

impl FormatWithPrecedence for FilterUnaryOperator {
    fn fmt(&self, _pp: u8, f: &mut Formatter<'_>) -> Result {
        match self {
            FilterUnaryOperator::Not => write!(f, "!"),
            FilterUnaryOperator::Negate => write!(f, "-"),
        }
    }
}
impl FormatWithPrecedence for FilterBinaryOperator {
    fn fmt(&self, _pp: u8, f: &mut Formatter<'_>) -> Result {
        let s = match self {
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

        write!(f, "{}", s)
    }
}

fn operator_precedence(operator: &FilterBinaryOperator) -> u8 {
    match operator {
        FilterBinaryOperator::Or => 0,
        FilterBinaryOperator::And => 0,
        FilterBinaryOperator::Equal => 1,
        FilterBinaryOperator::NotEqual => 1,
        FilterBinaryOperator::GreaterThan => 1,
        FilterBinaryOperator::GreaterThanOrEqual => 1,
        FilterBinaryOperator::LessThan => 1,
        FilterBinaryOperator::LessThanOrEqual => 1,
        FilterBinaryOperator::Add => 2,
        FilterBinaryOperator::Subtract => 2,
        FilterBinaryOperator::Multiply => 3,
        FilterBinaryOperator::Divide => 3,
        FilterBinaryOperator::Modulo => 3,
    }
}
