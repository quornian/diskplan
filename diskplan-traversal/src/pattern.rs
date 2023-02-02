use anyhow::Result;
use regex::Regex;

use diskplan_filesystem::PlantedPath;
use diskplan_schema::Expression;

use super::{eval::evaluate, stack};

#[derive(Debug)]
pub(super) enum CompiledPattern {
    Any,
    Regex(regex::Regex),
    RegexWithExclusions(regex::Regex, regex::Regex),
}

impl CompiledPattern {
    pub fn compile(
        match_pattern: Option<&Expression>,
        avoid_pattern: Option<&Expression>,
        stack: &stack::StackFrame,
        path: &PlantedPath,
    ) -> Result<CompiledPattern> {
        let match_pattern = match match_pattern {
            Some(expr) => Some(evaluate(expr, stack, path)?),
            None => None,
        };
        let avoid_pattern = match avoid_pattern {
            Some(expr) => Some(evaluate(expr, stack, path)?),
            None => None,
        };
        Ok(match (&match_pattern, &avoid_pattern) {
            (None, None) => CompiledPattern::Any,
            (Some(pattern), None) => {
                Regex::new(pattern)?; // Ensure it's valid before encasing to avoid injection
                CompiledPattern::Regex(Regex::new(&format!("^(?:{pattern})$"))?)
            }
            (_, Some(avoiding)) => {
                let pattern = match_pattern.as_deref().unwrap_or(".*");
                Regex::new(pattern)?;
                Regex::new(avoiding)?;
                CompiledPattern::RegexWithExclusions(
                    Regex::new(&format!("^(?:{pattern})$"))?,
                    Regex::new(&format!("^(?:{avoiding})$"))?,
                )
            }
        })
    }

    pub fn matches(&self, text: &str) -> bool {
        match self {
            Self::Any => true,
            Self::Regex(ref regex) => regex.is_match(text),
            Self::RegexWithExclusions(ref regex, ref excl) => {
                regex.is_match(text) && !excl.is_match(text)
            }
        }
    }
}
