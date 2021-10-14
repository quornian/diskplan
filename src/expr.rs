use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alphanumeric1, char},
    combinator::map,
    multi::many0,
    sequence::{delimited, preceded},
};

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum ExprError {
    #[error("Syntax error")]
    SyntaxError, //(#[from] nom::error::Error),
}

#[derive(Debug, PartialEq)]
pub enum Token<'a> {
    Text(&'a str),
    AtVar(&'a str),
    Global(Global),
}

#[derive(Debug, PartialEq)]
pub enum Global {
    Parent,
    Path,
    Name,
}

pub fn parse(s: &str) -> Result<Vec<Token<'_>>, ExprError> {
    match expr(s) {
        Ok((remains, tokens)) => {
            if remains.is_empty() {
                Ok(tokens)
            } else {
                Err(ExprError::SyntaxError)
            }
        }
        // TODO: Provide feedback
        Err(_) => Err(ExprError::SyntaxError),
    }
}

fn expr(input: &str) -> nom::IResult<&str, Vec<Token<'_>>> {
    many0(alt((non_special, at_var, braced_var)))(input)
}

fn non_special(input: &str) -> nom::IResult<&str, Token<'_>> {
    nom::bytes::complete::is_not("{}@")(input).map(|(rem, text)| (rem, Token::Text(text)))
}

fn at_var(input: &str) -> nom::IResult<&str, Token<'_>> {
    preceded(char('@'), alphanumeric1)(input).map(|(rem, name)| (rem, Token::AtVar(name)))
}

fn global(input: &str) -> nom::IResult<&str, Token<'_>> {
    map(
        alt((tag("PARENT"), tag("PATH"), tag("NAME"))),
        |s| match s {
            "PARENT" => Token::Global(Global::Parent),
            "PATH" => Token::Global(Global::Path),
            "NAME" => Token::Global(Global::Name),
            _ => panic!("Match with unaccounted token: {}", s),
        },
    )(input)
}

fn braced_var(input: &str) -> nom::IResult<&str, Token<'_>> {
    delimited(tag("{"), alt((at_var, global)), tag("}"))(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_special() {
        use Token::Text;
        assert_eq!(Ok(("", Text("all_of_it"))), non_special("all_of_it"));
        assert_eq!(
            Ok(("@some_more", Text("just_this_bit"))),
            non_special("just_this_bit@some_more")
        );
    }

    #[test]
    fn test_at_var() {
        use Token::AtVar;
        assert_eq!(Ok(("", AtVar("varname"))), at_var("@varname"));
        assert_eq!(
            Ok(("_without_underscores", AtVar("varname"))),
            at_var("@varname_without_underscores")
        );
        assert!(at_var("prefix@varname").is_err());
    }

    #[test]
    fn test_braced_at_var() {
        use Token::AtVar;
        assert_eq!(Ok(("", AtVar("varname"))), braced_var("{@varname}"));
        assert_eq!(
            Ok(("_without_underscores", AtVar("varname"))),
            braced_var("{@varname}_without_underscores")
        );
        assert!(braced_var("prefix{@varname}").is_err());
    }

    #[test]
    fn test_expr() {
        use self::Global::{Name, Parent, Path};
        use Token::{AtVar, Global, Text};
        assert_eq!(Ok(("", vec![])), expr(""));
        assert_eq!(Ok(("", vec![AtVar("varname")])), expr("@varname"));
        assert_eq!(
            Ok(("", vec![AtVar("varname"), Text("_no")])),
            expr("@varname_no")
        );
        assert_eq!(
            Ok(("", vec![AtVar("varname"), Text("_no")])),
            expr("@varname_no")
        );
        assert_eq!(
            Ok(("", vec![AtVar("varname"), Text("_no")])),
            expr("{@varname}_no")
        );
        assert_eq!(
            Ok(("", vec![AtVar("mount"), Text("/"), Global(Path)])),
            expr("{@mount}/{PATH}")
        );
        assert_eq!(
            Ok((
                "",
                vec![
                    AtVar("mount"),
                    Text("/"),
                    Global(Parent),
                    Text("/"),
                    Global(Name)
                ]
            )),
            expr("{@mount}/{PARENT}/{NAME}")
        );
    }
}
