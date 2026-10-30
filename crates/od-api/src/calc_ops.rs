//! `calculate.evaluate`: exact arithmetic and trigonometry for dimensions.
//! The work is `od_core::calc`; this is only its wire form.

use od_core::calc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CalculateArgs {
    /// At most 100, worked out in order; an expression may use the names of
    /// the steps before it.
    pub steps: Vec<CalcStep>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CalcStep {
    /// Letters, digits and underscores, not starting with a digit.
    pub name: String,
    /// Numbers, `+ - * / ^`, brackets, constants and functions.
    pub expression: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CalculateResult {
    pub values: Vec<CalcValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CalcValue {
    pub name: String,
    pub value: f64,
}

/// Work the steps out, or say what is wrong with them.
pub fn evaluate(args: &CalculateArgs) -> Result<CalculateResult, calc::CalcError> {
    let steps: Vec<(String, String)> = args
        .steps
        .iter()
        .map(|s| (s.name.clone(), s.expression.clone()))
        .collect();
    Ok(CalculateResult {
        values: calc::evaluate(&steps)?
            .into_iter()
            .map(|(name, value)| CalcValue { name, value })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn args(steps: serde_json::Value) -> CalculateArgs {
        serde_json::from_value(json!({"steps": steps})).expect("arguments")
    }

    #[test]
    fn steps_come_back_as_named_values_in_order() {
        let result = evaluate(&args(json!([
            {"name": "d", "expression": "75"},
            {"name": "r", "expression": "d/2"},
        ])))
        .unwrap();
        assert_eq!(
            result.values,
            [
                CalcValue {
                    name: "d".into(),
                    value: 75.0
                },
                CalcValue {
                    name: "r".into(),
                    value: 37.5
                },
            ]
        );
    }

    #[test]
    fn an_error_names_the_step() {
        let error = evaluate(&args(json!([{"name": "x", "expression": "1/0"}]))).unwrap_err();
        assert!(error.0.contains("step 1 (x)"), "{error}");
    }

    #[test]
    fn unknown_fields_are_refused() {
        let bad = json!({"steps": [{"name": "a", "expression": "1", "units": "mm"}]});
        assert!(serde_json::from_value::<CalculateArgs>(bad).is_err());
    }

    #[test]
    fn every_function_is_named_in_the_tool_description() {
        let tool = crate::catalogue::find("opendraft_calculate").expect("the tool");
        for (name, _) in calc::FUNCTIONS {
            assert!(tool.description.contains(name), "{name}");
        }
    }
}
