use std::convert::TryFrom;

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alphanumeric1, char},
    combinator::{eof, value},
    multi::many0,
    sequence::{delimited, preceded, terminated},
};

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum ExprError {
    #[error("Syntax error in expression at {0} looking for {1:?}")]
    SyntaxError(String, nom::error::ErrorKind),

    #[error("Parse error: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr<'a>(Vec<Token<'a>>);

#[derive(Debug, Clone, PartialEq)]
pub enum Token<'a> {
    Text(&'a str),
    AtVar(&'a str),
    Builtin(Builtin),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Builtin {
    Parent,
    Path,
    Name,
}

impl<'a> TryFrom<&'a str> for Expr<'a> {
    type Error = ExprError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        expr(value)
            .map(|(_, tokens)| Expr(tokens))
            .map_err(|err| match err {
                nom::Err::Error(nom::error::Error { input, code }) => {
                    ExprError::SyntaxError(input.to_owned(), code)
                }
                _ => ExprError::ParseError(err.to_string()),
            })
    }
}

impl<'a> Expr<'a> {
    pub fn tokens(&self) -> &Vec<Token<'a>> {
        &self.0
    }
}

/// Expression, such as "static/@varA_{@varB}v2/{NAME}"
///
fn expr(input: &str) -> nom::IResult<&str, Vec<Token<'_>>> {
    terminated(many0(alt((non_special, at_var, braced_var))), eof)(input)
}

/// A sequence of characters that are not part of any variable
///
fn non_special(input: &str) -> nom::IResult<&str, Token<'_>> {
    nom::bytes::complete::is_not("{}@")(input).map(|(rem, text)| (rem, Token::Text(text)))
}

/// A variable between braces, such as "{@example}" or "{PATH}"
fn braced_var(input: &str) -> nom::IResult<&str, Token<'_>> {
    delimited(tag("{"), alt((at_var, builtin)), tag("}"))(input)
}

/// A variable name prefixed with an at-sign, such as "@example"
///
fn at_var(input: &str) -> nom::IResult<&str, Token<'_>> {
    preceded(char('@'), alphanumeric1)(input).map(|(rem, name)| (rem, Token::AtVar(name)))
}

/// A builtin variable
///
fn builtin(input: &str) -> nom::IResult<&str, Token<'_>> {
    alt((
        value(Token::Builtin(Builtin::Parent), tag("PARENT")),
        value(Token::Builtin(Builtin::Path), tag("PATH")),
        value(Token::Builtin(Builtin::Name), tag("NAME")),
    ))(input)
}

#[cfg(test)]
mod tests {
    use super::Builtin::{Name, Parent, Path};
    use super::Token::{AtVar, Builtin, Text};
    use super::*;
    use nom::error::ErrorKind;

    #[test]
    fn test_non_special() {
        assert_eq!(Ok(("", Text("all_of_it"))), non_special("all_of_it"));
        assert_eq!(
            Ok(("@some_more", Text("just_this_bit"))),
            non_special("just_this_bit@some_more")
        );
    }

    #[test]
    fn test_at_var() {
        assert_eq!(Ok(("", AtVar("varname"))), at_var("@varname"));
        assert_eq!(
            Ok(("_without_underscores", AtVar("varname"))),
            at_var("@varname_without_underscores")
        );
        assert!(at_var("prefix@varname").is_err());
    }

    #[test]
    fn test_braced_at_var() {
        assert_eq!(Ok(("", AtVar("varname"))), braced_var("{@varname}"));
        assert_eq!(
            Ok(("_without_underscores", AtVar("varname"))),
            braced_var("{@varname}_without_underscores")
        );
        assert!(braced_var("prefix{@varname}").is_err());
    }

    #[test]
    fn test_expr() {
        assert_eq!(expr(""), Ok(("", vec![])));
        assert_eq!(expr("@varname"), Ok(("", vec![AtVar("varname")])));
        assert_eq!(
            expr("@varname_no"),
            Ok(("", vec![AtVar("varname"), Text("_no")])),
        );
        assert_eq!(
            expr("{@varname}_no"),
            Ok(("", vec![AtVar("varname"), Text("_no")]))
        );
        assert_eq!(
            expr("{@mount}/{PATH}"),
            Ok(("", vec![AtVar("mount"), Text("/"), Builtin(Path)]))
        );
        assert_eq!(
            expr("{@mount}/{PARENT}/{NAME}"),
            Ok((
                "",
                vec![
                    AtVar("mount"),
                    Text("/"),
                    Builtin(Parent),
                    Text("/"),
                    Builtin(Name)
                ]
            ))
        );
    }

    #[test]
    fn test_expr_incomplete_at() {
        assert_eq!(expr("@"), Err(nom_error("@", ErrorKind::Eof)));
    }

    #[test]
    fn test_expr_incomplete_brace() {
        assert_eq!(expr("something{"), Err(nom_error("{", ErrorKind::Eof)))
    }

    #[test]
    fn test_parse_text() {
        assert_eq!(
            Expr::try_from("some/text+with.other-=chars"),
            Ok(Expr(vec![Text("some/text+with.other-=chars")]))
        )
    }

    #[test]
    fn test_parse_var() {
        assert_eq!(Expr::try_from("@var"), Ok(Expr(vec![AtVar("var")])))
    }

    #[test]
    fn test_parse_var_trailing_text() {
        assert_eq!(
            Expr::try_from("@var_not_this"),
            Ok(Expr(vec![AtVar("var"), Text("_not_this")]))
        )
    }

    #[test]
    fn test_parse_empty_var() {
        assert_eq!(
            Expr::try_from("@"),
            Err(ExprError::SyntaxError("@".to_owned(), ErrorKind::Eof))
        )
    }

    #[test]
    fn test_parse_incomplete_at() {
        assert_eq!(
            Expr::try_from("before@"),
            Err(ExprError::SyntaxError("@".to_owned(), ErrorKind::Eof))
        );
    }

    #[test]
    fn test_parse_invalid_at() {
        assert_eq!(
            Expr::try_from("before@_invalid"),
            Err(ExprError::SyntaxError(
                "@_invalid".to_owned(),
                ErrorKind::Eof
            ))
        );
    }

    #[test]
    fn test_parse_incomplete_brace() {
        assert_eq!(
            Expr::try_from("something{else"),
            Err(ExprError::SyntaxError("{else".to_owned(), ErrorKind::Eof))
        );
    }

    #[test]
    fn test_parse_complete_brace_match() {
        assert_eq!(
            Expr::try_from("{PARENT}/{NAME}={PATH}"),
            Ok(Expr(vec![
                Builtin(Parent),
                Text("/"),
                Builtin(Name),
                Text("="),
                Builtin(Path)
            ]))
        );
    }

    #[test]
    fn test_parse_complete_brace_no_match() {
        assert_eq!(
            Expr::try_from("{parent}/{name}={path}"),
            Err(ExprError::SyntaxError(
                "{parent}/{name}={path}".to_owned(),
                ErrorKind::Eof
            ))
        );
    }

    fn nom_error(input: &'static str, code: ErrorKind) -> nom::Err<nom::error::Error<&str>> {
        nom::Err::Error(nom::error::Error { input, code })
    }
}
