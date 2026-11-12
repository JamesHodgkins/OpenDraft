use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use super::{evaluate, CalcError, EXPRESSION_MAX, STEPS_MAX};

fn one(text: &str) -> Result<f64, CalcError> {
    evaluate(&[("v".to_owned(), text.to_owned())]).map(|v| v[0].1)
}

fn ok(text: &str) -> f64 {
    one(text).unwrap_or_else(|e| panic!("{text}: {e}"))
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-12
}

fn refused(text: &str) -> String {
    one(text).expect_err(text).0
}

#[test]
fn arithmetic_follows_the_usual_precedence() {
    assert_eq!(ok("2+3*4"), 14.0);
    assert_eq!(ok("(2+3)*4"), 20.0);
    assert_eq!(ok("10-4-3"), 3.0);
    assert_eq!(ok("2*3/4"), 1.5);
    assert_eq!(ok("100/10/5"), 2.0);
    assert_eq!(ok("  7 -  2 "), 5.0);
}

#[test]
fn powers_are_right_to_left_and_bind_tighter_than_a_minus() {
    assert_eq!(ok("2^3^2"), 512.0);
    assert_eq!(ok("-2^2"), -4.0);
    assert_eq!(ok("(-2)^2"), 4.0);
    assert_eq!(ok("2^-1"), 0.5);
    assert_eq!(ok("--3"), 3.0);
    assert_eq!(ok("+5"), 5.0);
}

#[test]
fn numbers_may_have_a_fraction_and_an_exponent() {
    assert_eq!(ok("1e3"), 1000.0);
    assert!(near(ok("1.5e-2"), 0.015));
    assert_eq!(ok(".5"), 0.5);
    assert_eq!(ok("2E2"), 200.0);
}

#[test]
fn the_constants_and_the_trigonometry_are_exact_to_the_double() {
    assert!(near(ok("pi"), PI));
    assert!(near(ok("tau"), 2.0 * PI));
    assert!(near(ok("e"), std::f64::consts::E));
    assert!(near(ok("sin(pi/2)"), 1.0));
    assert!(near(ok("cos(pi)"), -1.0));
    assert!(near(ok("atan2(1, 1)"), FRAC_PI_4));
    assert!(near(ok("atan2(1, 0)"), FRAC_PI_2), "y first, then x");
    assert!(near(ok("atan2(0, -1)"), PI));
    assert!(near(ok("asin(1)"), FRAC_PI_2));
    assert!(near(ok("rad(180)"), PI));
    assert!(near(ok("deg(pi)"), 180.0));
    assert!(near(ok("tan(rad(45))"), 1.0));
}

#[test]
fn the_other_functions_do_what_they_say() {
    assert_eq!(ok("sqrt(16)"), 4.0);
    assert_eq!(ok("hypot(3, 4)"), 5.0);
    assert_eq!(ok("min(3, 1, 2)"), 1.0);
    assert_eq!(ok("max(3, 1, 2)"), 3.0);
    assert_eq!(ok("abs(-2.5)"), 2.5);
    assert_eq!(ok("floor(2.7)"), 2.0);
    assert_eq!(ok("ceil(2.1)"), 3.0);
    assert_eq!(ok("round(2.5)"), 3.0);
    assert_eq!(ok("round_to(1.23456, 3)"), 1.235);
    assert_eq!(ok("pow(2, 10)"), 1024.0);
    assert!(near(ok("ln(e)"), 1.0));
    assert!(near(ok("log10(1000)"), 3.0));
    assert!(near(ok("exp(0)"), 1.0));
}

#[test]
fn a_step_can_use_the_names_before_it() {
    let steps: Vec<(String, String)> = [
        ("r", "37.5"),
        ("a", "rad(45)"),
        ("x", "r*cos(a)"),
        ("y", "r*sin(a)"),
    ]
    .into_iter()
    .map(|(n, e)| (n.to_owned(), e.to_owned()))
    .collect();
    let values = evaluate(&steps).unwrap();
    assert_eq!(
        values.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
        ["r", "a", "x", "y"]
    );
    assert!(near(values[2].1, 37.5 * FRAC_PI_4.cos()));
    assert!(near(values[3].1, 37.5 * FRAC_PI_4.sin()));
}

#[test]
fn what_does_not_come_out_as_a_number_is_an_error_not_inf_or_nan() {
    assert!(refused("1/0").contains("division by zero"));
    assert!(refused("sqrt(0-1)").contains("not a number"));
    assert!(refused("ln(0)").contains("too large"));
    assert!(refused("9^999").contains("too large"));
    assert!(refused("acos(2)").contains("not a number"));
}

#[test]
fn a_badly_formed_expression_says_where_it_went_wrong() {
    assert!(refused("(1+2").contains("expected ')'"));
    assert!(refused("1+").contains("ends too soon"));
    assert!(
        refused("1 2").contains("unexpected 2 (at 3)"),
        "{}",
        refused("1 2")
    );
    assert!(refused("2 $ 3").contains("unexpected '$' (at 3)"));
    assert!(refused("1.2.3").contains("not a number"));
    assert!(refused("*3").contains("unexpected '*'"));
    assert!(refused("").contains("empty"));
    assert!(refused("   ").contains("empty"));
}

#[test]
fn a_call_must_name_a_function_and_give_it_the_right_number_of_arguments() {
    assert!(refused("sin()").contains("takes 1 argument"));
    assert!(refused("sin(1, 2)").contains("takes 1 argument"));
    assert!(refused("atan2(1)").contains("takes 2 arguments"));
    assert!(refused("max()").contains("at least one"));
    assert!(refused("foo(1)").contains("not a function"));
    assert!(
        refused("SIN(1)").contains("not a function"),
        "names are lower case"
    );
    assert!(refused("sin(1").contains("expected"));
    assert!(refused("round_to(1, 13)").contains("0 to 12"));
}

#[test]
fn a_name_that_is_not_defined_is_refused_and_so_is_a_forward_reference() {
    assert!(refused("width*2").contains("width is not defined"));
    let steps = vec![
        ("a".to_owned(), "b+1".to_owned()),
        ("b".to_owned(), "2".to_owned()),
    ];
    let error = evaluate(&steps).unwrap_err();
    assert!(
        error.0.contains("step 1 (a)") && error.0.contains("b is not defined"),
        "{error}"
    );
}

#[test]
fn names_must_be_plain_and_not_reuse_a_constant_a_function_or_each_other() {
    let bad = |name: &str| {
        evaluate(&[(name.to_owned(), "1".to_owned())])
            .expect_err(name)
            .0
    };
    assert!(bad("").contains("a name is"));
    assert!(bad("1x").contains("a name is"));
    assert!(bad("a b").contains("a name is"));
    assert!(bad(&"n".repeat(33)).contains("a name is"));
    assert!(bad("pi").contains("already a constant or function"));
    assert!(bad("sin").contains("already a constant or function"));
    let twice = vec![
        ("a".to_owned(), "1".to_owned()),
        ("a".to_owned(), "2".to_owned()),
    ];
    assert!(evaluate(&twice).unwrap_err().0.contains("defined twice"));
}

#[test]
fn the_size_of_a_calculation_is_limited() {
    assert!(evaluate(&[]).is_err());
    let many: Vec<(String, String)> = (0..=STEPS_MAX)
        .map(|i| (format!("n{i}"), "1".to_owned()))
        .collect();
    assert!(evaluate(&many).is_err());
    let long = format!("1{}", "+1".repeat(EXPRESSION_MAX));
    assert!(refused(&long).contains("longer than"));
    let deep = format!("{}1{}", "(".repeat(60), ")".repeat(60));
    assert!(refused(&deep).contains("nested too deeply"));
    let deep_minus = format!("{}1", "-".repeat(100));
    assert!(refused(&deep_minus).contains("nested too deeply"));
}
