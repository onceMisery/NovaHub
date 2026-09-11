#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CalculatorError {
    Empty,
    UnexpectedToken,
    DivisionByZero,
    TrailingInput,
    UnknownFunction,
    DomainError,
    NonFinite,
}

/// Evaluates a bounded arithmetic expression without a script runtime.
///
/// Supported syntax is numeric literals, `+`, `-`, `*`, `/`, parentheses,
/// constants (`pi` and `e`), common math functions, and a postfix percent
/// operator. The parser is deliberately bounded by the input length and does
/// not evaluate arbitrary code.
///
/// # Errors
///
/// Returns an error for malformed input, division by zero, or trailing tokens.
pub fn evaluate(input: &str) -> Result<f64, CalculatorError> {
    let mut parser = Parser::new(input);
    parser.skip_whitespace();
    if parser.at_end() {
        return Err(CalculatorError::Empty);
    }
    let value = parser.expression()?;
    parser.skip_whitespace();
    if parser.at_end() {
        ensure_finite(value)
    } else {
        Err(CalculatorError::TrailingInput)
    }
}

/// Converts one bounded value between common offline units.
///
/// Units are grouped by dimension. Length (`m`, `km`, `cm`, `mm`, `in`,
/// `ft`, `mi`), mass (`g`, `kg`, `mg`, `lb`), and time (`ms`, `s`, `min`,
/// `h`, `day`) are supported. Temperature supports `c`, `f`, and `k`.
///
/// # Errors
///
/// Returns [`CalculatorError::UnexpectedToken`] for an unknown or
/// dimension-mismatched unit, and [`CalculatorError::NonFinite`] for a value
/// that cannot be represented as a finite result.
pub fn convert(value: f64, from: &str, to: &str) -> Result<f64, CalculatorError> {
    let from = normalize_unit(from);
    let to = normalize_unit(to);
    if from.is_empty() || to.is_empty() {
        return Err(CalculatorError::UnexpectedToken);
    }
    let result = if let (Some(from), Some(to)) = (length_unit(from), length_unit(to)) {
        if from.1 != to.1 {
            return Err(CalculatorError::UnexpectedToken);
        }
        value * from.0 / to.0
    } else if let (Some(from), Some(to)) = (mass_unit(from), mass_unit(to)) {
        if from.1 != to.1 {
            return Err(CalculatorError::UnexpectedToken);
        }
        value * from.0 / to.0
    } else if let (Some(from), Some(to)) = (time_unit(from), time_unit(to)) {
        if from.1 != to.1 {
            return Err(CalculatorError::UnexpectedToken);
        }
        value * from.0 / to.0
    } else if temperature_unit(from).is_some() && temperature_unit(to).is_some() {
        let celsius = to_celsius(value, from)?;
        from_celsius(celsius, to)?
    } else {
        return Err(CalculatorError::UnexpectedToken);
    };
    ensure_finite(result)
}

fn normalize_unit(value: &str) -> &str {
    match value.trim().to_ascii_lowercase().as_str() {
        "m" | "meters" | "metres" => "m",
        "km" | "kilometers" | "kilometres" => "km",
        "cm" | "centimeters" | "centimetres" => "cm",
        "mm" | "millimeters" | "millimetres" => "mm",
        "in" | "inches" => "in",
        "ft" | "feet" => "ft",
        "mi" | "miles" => "mi",
        "g" | "grams" => "g",
        "kg" | "kilograms" => "kg",
        "mg" | "milligrams" => "mg",
        "lb" | "pounds" => "lb",
        "ms" | "milliseconds" => "ms",
        "s" | "seconds" => "s",
        "min" | "minutes" => "min",
        "h" | "hours" => "h",
        "day" | "days" => "day",
        "c" => "c",
        "f" => "f",
        "k" => "k",
        _ => "",
    }
}

fn length_unit(unit: &str) -> Option<(f64, &'static str)> {
    Some((
        match unit {
            "m" => 1.0,
            "km" => 1_000.0,
            "cm" => 0.01,
            "mm" => 0.001,
            "in" => 0.0254,
            "ft" => 0.3048,
            "mi" => 1_609.344,
            _ => return None,
        },
        "length",
    ))
}

fn mass_unit(unit: &str) -> Option<(f64, &'static str)> {
    Some((
        match unit {
            "g" => 1.0,
            "kg" => 1_000.0,
            "mg" => 0.001,
            "lb" => 453.592_37,
            _ => return None,
        },
        "mass",
    ))
}

fn time_unit(unit: &str) -> Option<(f64, &'static str)> {
    Some((
        match unit {
            "ms" => 0.001,
            "s" => 1.0,
            "min" => 60.0,
            "h" => 3_600.0,
            "day" => 86_400.0,
            _ => return None,
        },
        "time",
    ))
}

fn temperature_unit(unit: &str) -> Option<&'static str> {
    matches!(unit, "c" | "f" | "k").then_some("temperature")
}

fn to_celsius(value: f64, unit: &str) -> Result<f64, CalculatorError> {
    match unit {
        "c" => Ok(value),
        "f" => Ok((value - 32.0) * 5.0 / 9.0),
        "k" if value >= 0.0 => Ok(value - 273.15),
        "k" => Err(CalculatorError::DomainError),
        _ => Err(CalculatorError::UnexpectedToken),
    }
}

fn from_celsius(value: f64, unit: &str) -> Result<f64, CalculatorError> {
    let result = match unit {
        "c" => value,
        "f" => value * 9.0 / 5.0 + 32.0,
        "k" if value >= -273.15 => value + 273.15,
        "k" => return Err(CalculatorError::DomainError),
        _ => return Err(CalculatorError::UnexpectedToken),
    };
    Ok(result)
}

fn ensure_finite(value: f64) -> Result<f64, CalculatorError> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(CalculatorError::NonFinite)
}

struct Parser {
    chars: Vec<char>,
    position: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            position: 0,
        }
    }

    fn at_end(&self) -> bool {
        self.position >= self.chars.len()
    }

    fn skip_whitespace(&mut self) {
        while self
            .chars
            .get(self.position)
            .is_some_and(|character| character.is_whitespace())
        {
            self.position += 1;
        }
    }

    fn expression(&mut self) -> Result<f64, CalculatorError> {
        let mut value = self.term()?;
        loop {
            self.skip_whitespace();
            match self.chars.get(self.position).copied() {
                Some('+') => {
                    self.position += 1;
                    value += self.term()?;
                }
                Some('-') => {
                    self.position += 1;
                    value -= self.term()?;
                }
                _ => return Ok(value),
            }
        }
    }

    fn term(&mut self) -> Result<f64, CalculatorError> {
        let mut value = self.unary()?;
        loop {
            self.skip_whitespace();
            match self.chars.get(self.position).copied() {
                Some('*') => {
                    self.position += 1;
                    value *= self.unary()?;
                }
                Some('/') => {
                    self.position += 1;
                    let divisor = self.unary()?;
                    if divisor == 0.0 {
                        return Err(CalculatorError::DivisionByZero);
                    }
                    value /= divisor;
                }
                _ => return Ok(value),
            }
        }
    }

    fn unary(&mut self) -> Result<f64, CalculatorError> {
        self.skip_whitespace();
        match self.chars.get(self.position).copied() {
            Some('+') => {
                self.position += 1;
                self.unary()
            }
            Some('-') => {
                self.position += 1;
                Ok(-self.unary()?)
            }
            _ => self.primary(),
        }
    }

    fn primary(&mut self) -> Result<f64, CalculatorError> {
        self.skip_whitespace();
        if self.chars.get(self.position) == Some(&'(') {
            self.position += 1;
            let value = self.expression()?;
            self.skip_whitespace();
            if self.chars.get(self.position) != Some(&')') {
                return Err(CalculatorError::UnexpectedToken);
            }
            self.position += 1;
            return Ok(self.percent(value));
        }
        if self
            .chars
            .get(self.position)
            .is_some_and(char::is_ascii_alphabetic)
        {
            let name = self.identifier();
            self.skip_whitespace();
            if name == "pi" {
                return Ok(self.percent(std::f64::consts::PI));
            }
            if name == "e" {
                return Ok(self.percent(std::f64::consts::E));
            }
            if self.chars.get(self.position) != Some(&'(') {
                return Err(CalculatorError::UnknownFunction);
            }
            self.position += 1;
            let first = self.expression()?;
            self.skip_whitespace();
            let second = if self.chars.get(self.position) == Some(&',') {
                self.position += 1;
                Some(self.expression()?)
            } else {
                None
            };
            self.skip_whitespace();
            if self.chars.get(self.position) != Some(&')') {
                return Err(CalculatorError::UnexpectedToken);
            }
            self.position += 1;
            return Self::apply_function(name, first, second).map(|value| self.percent(value));
        }
        let start = self.position;
        while self
            .chars
            .get(self.position)
            .is_some_and(|character| character.is_ascii_digit() || *character == '.')
        {
            self.position += 1;
        }
        if start == self.position {
            return Err(CalculatorError::UnexpectedToken);
        }
        let number: String = self.chars[start..self.position].iter().collect();
        let value = number
            .parse::<f64>()
            .map_err(|_| CalculatorError::UnexpectedToken)?;
        Ok(self.percent(value))
    }

    fn identifier(&mut self) -> &'static str {
        let start = self.position;
        while self
            .chars
            .get(self.position)
            .is_some_and(char::is_ascii_alphabetic)
        {
            self.position += 1;
        }
        match self.chars[start..self.position]
            .iter()
            .collect::<String>()
            .to_ascii_lowercase()
            .as_str()
        {
            "sqrt" => "sqrt",
            "sin" => "sin",
            "cos" => "cos",
            "tan" => "tan",
            "abs" => "abs",
            "ln" => "ln",
            "log" | "log10" => "log10",
            "exp" => "exp",
            "floor" => "floor",
            "ceil" => "ceil",
            "round" => "round",
            "min" => "min",
            "max" => "max",
            "pi" => "pi",
            "e" => "e",
            _ => "?",
        }
    }

    fn apply_function(name: &str, first: f64, second: Option<f64>) -> Result<f64, CalculatorError> {
        let value = match name {
            "sqrt" if first >= 0.0 => first.sqrt(),
            "sin" => first.sin(),
            "cos" => first.cos(),
            "tan" => first.tan(),
            "abs" => first.abs(),
            "ln" if first > 0.0 => first.ln(),
            "log10" if first > 0.0 => first.log10(),
            "exp" => first.exp(),
            "floor" => first.floor(),
            "ceil" => first.ceil(),
            "round" => first.round(),
            "min" => first.min(second.ok_or(CalculatorError::UnexpectedToken)?),
            "max" => first.max(second.ok_or(CalculatorError::UnexpectedToken)?),
            "sqrt" | "ln" | "log10" => return Err(CalculatorError::DomainError),
            _ => return Err(CalculatorError::UnknownFunction),
        };
        ensure_finite(value)
    }

    fn percent(&mut self, value: f64) -> f64 {
        self.skip_whitespace();
        if self.chars.get(self.position) == Some(&'%') {
            self.position += 1;
            value / 100.0
        } else {
            value
        }
    }
}

#[cfg(test)]
mod conversion_tests {
    use super::{CalculatorError, convert, evaluate};

    #[test]
    fn calculator_supports_common_math_functions_and_constants() {
        assert!((evaluate("sqrt(9) + sin(pi / 2)").expect("math") - 4.0).abs() < 1e-9);
        assert!((evaluate("max(2, 5) + min(4, 1)").expect("min max") - 6.0).abs() < 1e-9);
        assert!(matches!(
            evaluate("sqrt(-1)"),
            Err(CalculatorError::DomainError)
        ));
    }

    #[test]
    fn unit_conversion_stays_offline_and_dimension_checked() {
        assert!((convert(1.0, "km", "m").expect("length") - 1_000.0).abs() < 1e-9);
        assert!((convert(32.0, "f", "c").expect("temperature")).abs() < 1e-9);
        assert!(convert(1.0, "kg", "m").is_err());
    }
}
