#[cfg(feature = "bincode")]
use bincode::{Decode, Encode};

/// Bet size candidates for the first bets and raises.
///
/// In the `try_from()` method, multiple bet sizes can be specified using a comma-separated string.
/// Each element must be a string ending in one of the following characters: %, x, c, e, a.
///
/// - %: Percentage of the pot. Example: "70%"
/// - x: Multiple of the previous bet. Valid for only raises. Example: "2.5x"
/// - c: Constant value. Must be an integer. Example: "100c"
/// - e: Geometric size.
///   - e: Same as "3e" for the flop, "2e" for the turn, and "1e" (equivalent to "a") for the river.
///   - Xe: The geometric size with X streets remaining. X must be a positive integer. Example: "2e"
///   - XeY%: Same as Xe, but the maximum size is Y% of the pot. Example: "3e200%".
/// - a: All-in. Example: "a"
///
/// # Examples
/// ```
/// use postflop_solver::BetSize::*;
/// use postflop_solver::BetSizeCandidates;
///
/// let bet_size = BetSizeCandidates::try_from(("50%, 100c, 2e, a", "2.5x")).unwrap();
///
/// assert_eq!(
///     bet_size.bet,
///     vec![
///         PotRelative(0.5),
///         Additive(100),
///         Geometric(2, f64::INFINITY),
///         AllIn
///    ]
/// );
///
/// assert_eq!(bet_size.raise, vec![PrevBetRelative(2.5)]);
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "bincode", derive(Decode, Encode))]
pub struct BetSizeCandidates {
    /// Bet size candidates for first bet.
    pub bet: Vec<BetSize>,

    /// Bet size candidates for raise.
    pub raise: Vec<BetSize>,
}

/// Bet size candidates for the donk bets.
///
/// See the [`BetSizeCandidates`] struct for the description and examples.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "bincode", derive(Decode, Encode))]
pub struct DonkSizeCandidates {
    pub donk: Vec<BetSize>,
}

/// Bet size specification.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(feature = "bincode", derive(Decode, Encode))]
pub enum BetSize {
    /// Bet size relative to the current pot size.
    PotRelative(f64),

    /// Bet size relative to the previous bet size (only valid for raise actions).
    PrevBetRelative(f64),

    /// Bet size specifying constant addition.
    Additive(i32),

    /// Geometric bet size for `i32` streets with maximum pot-relative size of `f64`.
    ///
    /// If `i32 == 0`, the number of streets is as follows: flop = 3, turn = 2, river = 1.
    Geometric(i32, f64),

    /// Bet size representing all-in.
    AllIn,
}

impl TryFrom<(&str, &str)> for BetSizeCandidates {
    type Error = String;

    /// Attempts to convert comma-separated strings into bet sizes.
    ///
    /// See the [`BetSizeCandidates`] struct for the description and examples.
    fn try_from((bet_str, raise_str): (&str, &str)) -> Result<Self, Self::Error> {
        let mut bet_sizes = bet_str.split(',').map(str::trim).collect::<Vec<_>>();
        let mut raise_sizes = raise_str.split(',').map(str::trim).collect::<Vec<_>>();

        if bet_sizes.last().unwrap().is_empty() {
            bet_sizes.pop();
        }

        if raise_sizes.last().unwrap().is_empty() {
            raise_sizes.pop();
        }

        let mut bet = Vec::new();
        let mut raise = Vec::new();

        for bet_size in bet_sizes {
            bet.push(bet_size_from_str(bet_size, false)?);
        }

        for raise_size in raise_sizes {
            raise.push(bet_size_from_str(raise_size, true)?);
        }

        bet.sort_unstable_by(|l, r| l.partial_cmp(r).unwrap());
        raise.sort_unstable_by(|l, r| l.partial_cmp(r).unwrap());

        Ok(BetSizeCandidates { bet, raise })
    }
}

impl TryFrom<&str> for DonkSizeCandidates {
    type Error = String;

    /// Attempts to convert comma-separated strings into bet sizes.
    ///
    /// See the [`BetSizeCandidates`] struct for the description and examples.
    fn try_from(donk_str: &str) -> Result<Self, Self::Error> {
        let mut donk_sizes = donk_str.split(',').map(str::trim).collect::<Vec<_>>();

        if donk_sizes.last().unwrap().is_empty() {
            donk_sizes.pop();
        }

        let mut donk = Vec::new();

        for donk_size in donk_sizes {
            donk.push(bet_size_from_str(donk_size, false)?);
        }

        donk.sort_unstable_by(|l, r| l.partial_cmp(r).unwrap());

        Ok(DonkSizeCandidates { donk })
    }
}

fn parse_float(s: &str) -> Option<f64> {
    if s.contains('+') || s.contains('-') || s.contains(|c: char| c.is_ascii_alphabetic()) {
        None
    } else {
        s.parse::<f64>().ok()
    }
}

fn bet_size_from_str(s: &str, allow_prev_bet_rel: bool) -> Result<BetSize, String> {
    let s_lower = s.to_lowercase();
    let err_msg = format!("Invalid bet size: {s}");

    if let Some(prev_bet_rel) = s_lower.strip_suffix('x') {
        // Previous bet relative
        if !allow_prev_bet_rel {
            let err_msg = format!("Relative size to the previous bet is not allowed: {s}");
            Err(err_msg)
        } else {
            let float = parse_float(prev_bet_rel).ok_or(&err_msg)?;
            if float <= 1.0 {
                let err_msg = format!("Multiplier must be greater than 1.0: {s}");
                Err(err_msg)
            } else {
                Ok(BetSize::PrevBetRelative(float))
            }
        }
    } else if let Some(add) = s_lower.strip_suffix('c') {
        // Additive
        let float = parse_float(add).ok_or(&err_msg)?;
        if float.trunc() != float {
            Err(format!("Additional size must be an integer: {s}"))
        } else if float > i32::MAX as f64 {
            Err(format!("Additional size must be less than 2^31: {s}"))
        } else {
            Ok(BetSize::Additive(float as i32))
        }
    } else if s_lower.contains('e') {
        // Geometric
        let mut split = s_lower.split('e');
        let num_streets_str = split.next().ok_or(&err_msg)?;
        let max_pot_rel_str = split.next().ok_or(&err_msg)?;

        let num_streets = if num_streets_str.is_empty() {
            0
        } else {
            let float = parse_float(num_streets_str).ok_or(&err_msg)?;
            if float.trunc() != float || float == 0.0 {
                let err_msg = format!("Number of streets must be a positive integer: {s}");
                return Err(err_msg);
            } else if float > 100.0 {
                let err_msg = format!("Number of streets must be less than or equal to 100: {s}");
                return Err(err_msg);
            } else {
                float as i32
            }
        };

        let max_pot_rel = if max_pot_rel_str.is_empty() {
            f64::INFINITY
        } else {
            let max_pot_rel_str = max_pot_rel_str.strip_suffix('%').ok_or(&err_msg)?;
            parse_float(max_pot_rel_str).ok_or(&err_msg)? / 100.0
        };

        if split.next().is_some() {
            Err(err_msg)
        } else {
            Ok(BetSize::Geometric(num_streets, max_pot_rel))
        }
    } else if let Some(pot_rel) = s_lower.strip_suffix('%') {
        // Pot relative (must be after the geometric check)
        let float = parse_float(pot_rel).ok_or(&err_msg)?;
        Ok(BetSize::PotRelative(float / 100.0))
    } else if s_lower == "a" {
        // All-in
        Ok(BetSize::AllIn)
    } else {
        // Parse error
        Err(err_msg)
    }
}

#[cfg(test)]
mod tests {
    use super::BetSize::*;
    use super::*;

    #[test]
    fn test_bet_size_from_str() {
        let tests = [
            ("0%", PotRelative(0.0)),
            ("75%", PotRelative(0.75)),
            ("112.5%", PotRelative(1.125)),
            ("1.001x", PrevBetRelative(1.001)),
            ("3.5X", PrevBetRelative(3.5)),
            ("0c", Additive(0)),
            ("123C", Additive(123)),
            ("e", Geometric(0, f64::INFINITY)),
            ("E", Geometric(0, f64::INFINITY)),
            ("2e", Geometric(2, f64::INFINITY)),
            ("E37.5%", Geometric(0, 0.375)),
            ("100e.5%", Geometric(100, 0.005)),
            ("a", AllIn),
            ("A", AllIn),
        ];

        for (s, expected) in tests {
            assert_eq!(bet_size_from_str(s, true), Ok(expected));
        }

        let error_tests = [
            "", "0", "1.23", "%", "+42%", "-30%", "x", "0x", "1x", "c", "12.3c", "0e", "2.7e",
            "101e", "3e7", "E%", "1e2e3", "bet", "1a", "a1",
        ];

        for s in error_tests {
            assert!(bet_size_from_str(s, true).is_err());
        }
    }

    #[test]
    fn test_bet_sizes_from_str() {
        let tests = [
            (
                "40%, 70%",
                "",
                BetSizeCandidates {
                    bet: vec![PotRelative(0.4), PotRelative(0.7)],
                    raise: Vec::new(),
                },
            ),
            (
                "50c, e, a,",
                "25%, 2.5x, e200%",
                BetSizeCandidates {
                    bet: vec![Additive(50), Geometric(0, f64::INFINITY), AllIn],
                    raise: vec![PotRelative(0.25), PrevBetRelative(2.5), Geometric(0, 2.0)],
                },
            ),
        ];

        for (bet, raise, expected) in tests {
            assert_eq!((bet, raise).try_into(), Ok(expected));
        }

        let error_tests = [("2.5x", ""), (",", "")];

        for (bet, raise) in error_tests {
            assert!(BetSizeCandidates::try_from((bet, raise)).is_err());
        }
    }

    #[test]
    fn test_donk_sizes_from_str() {
        let tests = [
            (
                "40%, 70%",
                DonkSizeCandidates {
                    donk: vec![PotRelative(0.4), PotRelative(0.7)],
                },
            ),
            (
                "50c, e, a,",
                DonkSizeCandidates {
                    donk: vec![Additive(50), Geometric(0, f64::INFINITY), AllIn],
                },
            ),
        ];

        for (donk, expected) in tests {
            assert_eq!(donk.try_into(), Ok(expected));
        }

        let error_tests = ["2.5x", ","];

        for donk in error_tests {
            assert!(DonkSizeCandidates::try_from(donk).is_err());
        }
    }
}
